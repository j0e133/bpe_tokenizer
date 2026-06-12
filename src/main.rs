
use std::{collections::HashMap, fs::{self, read_to_string, write}, path::Path, sync::{atomic::{AtomicUsize, Ordering}}, time::Instant};

use rustc_hash::{FxBuildHasher, FxHashMap};
use itertools::Itertools;
use rayon::prelude::*;


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Token(usize);


struct Rule {
    a: Token,
    b: Token,
    replacement: Token
}

impl Rule {
    fn new(a: Token, b: Token, replacement: Token) -> Self {
        Self { a, b, replacement }
    }

    fn apply_ip(&self, tokens: &mut Vec<Token>) {
        let mut i = 0;
        let mut j = 0;

        while i < tokens.len() {
            // combine two tokens
            if i < tokens.len() - 1 && self.a == tokens[i] && self.b == tokens[i + 1] {
                tokens[j] = self.replacement;
                i += 2;
            }
            // keep token
            else {
                tokens[j] = tokens[i];
                i += 1;
            }

            j += 1;
        }

        tokens.truncate(j);
    }
}


struct Tokenization {
    tokens: Box<[Token]>
}

impl Tokenization {
    fn from_vec(vec: Vec<Token>) -> Self {
        Self {
            tokens: vec.into_boxed_slice()
        }
    }

    /// Creates a JSON representation of the tokenization in a String
    fn to_json(&self) -> String {
        let tokens =
            self.tokens.iter()
                       .map(|Token(tok)| tok.to_string())
                       .join(",");

        format!("[{}]", tokens)
    }
}


struct CorpusTokenization {
    tokenizations: Vec<Tokenization>
}

impl CorpusTokenization {
    fn new(tokenizations: Vec<Tokenization>) -> Self {
        Self { tokenizations }
    }

    /// Creates a JSON representation of the corpus in a String
    fn to_json(&self) -> String {
        let tokenizations =
            self.tokenizations.iter()
                              .map(|tok| tok.to_json())
                              .join(",");

        format!("[{}]", tokenizations)
    }

    /// Saves a JSON representation of the corpus to a file
    fn save<T: AsRef<Path>>(&self, filename: T) -> std::io::Result<()> {
        let json = self.to_json();

        write(filename, json)?;

        Ok(())
    }
}


struct BPETokenizer {
    vocab: Vec<String>,
    rules: Vec<Rule>,
    encoding_table: FxHashMap<char, Token>
}

impl BPETokenizer {
    fn new(vocab: Vec<String>, rules: Vec<Rule>, encoding_table: FxHashMap<char, Token>) -> Self {
        Self {
            vocab,
            rules,
            encoding_table
        }
    }

    /// Creates a tokenizer from a corpus of Strings. Returns the tokenizer and the encoded corpus
    /// 
    /// `vocab_size` is the maximum number of vocab chunks the tokenizer will create, unlimited if 0.
    /// 
    /// `min_frequency` is the lowest frequency in the text at which pairs will still be combined. Should be >= 2
    /// 
    /// `low_memory` is whether or not to use low memory mode, which is slower
    fn from_corpus(corpus: &Vec<String>, vocab_size: usize, min_frequency: usize, low_memory: bool) -> (Self, CorpusTokenization) {
        let mut vocab: Vec<String> =
            corpus.iter()
                  .flat_map(|s| s.chars())
                  .unique()
                  .sorted()
                  .map(|ch| ch.to_string())
                  .collect();

        let encoding_table: FxHashMap<char, Token> = 
            vocab.iter()
                 .enumerate()
                 .map(|(i, ch)| (ch.chars().next().unwrap(), Token(i)))
                 .collect();

        if 0 < vocab_size && vocab_size < encoding_table.len() { panic!("Parameter `vocab_size` (={}) is less than the text's vocab length of {}", vocab_size, encoding_table.len()) }

        let mut rules = Vec::with_capacity(vocab_size);
        let mut token = vocab.len();

        let mut encodings: Vec<Vec<Token>> =
            corpus.iter()
                  .map(|text|
                      text.chars()
                          .map(|ch| *encoding_table.get(&ch).unwrap())
                          .collect()
                      )
                  .collect();
        
        let counts_len =
            if low_memory { 0 }                           // don't initialize
            else if vocab_size == 0 { token * token * 4 } // initialize to double length of vocab 
            else { vocab_size * vocab_size };             // initialize normally

        let mut counts: Vec<usize> = (0..counts_len).map(|_| Default::default()).collect();

        while vocab_size == 0 || token < vocab_size {
            if !low_memory && token * token > counts.len() {
                counts = (0..counts.len() * 4).map(|_| Default::default()).collect(); // double allocation length
            }

            let ([a, b], n) = if low_memory {
                Self::most_common_pair_low_memory(&encodings)
            } else {
                Self::most_common_pair(&encodings, token, &mut counts)
            };

            // stop combining if frequency of pair < min_frequency
            if n < min_frequency { break }

            let rule = Rule::new(a, b, Token(token));

            encodings.par_iter_mut().for_each(|encoding| {
                rule.apply_ip(encoding);
            });

            let combined_str = format!("{}{}", vocab[a.0], vocab[b.0]);

            vocab.push(combined_str);
            rules.push(rule);
            token = vocab.len();
        }

        let tokenizer = Self::new(vocab, rules, encoding_table);
        let encoded_corpus = CorpusTokenization::new(
            encodings.into_iter()
                                    .map(|encoding| Tokenization::from_vec(encoding))
                                    .collect()
        );

        (tokenizer, encoded_corpus)
    }

    /// Returns the most common common pair of consecutive tokens and how many times it appears.
    fn most_common_pair(encodings: &Vec<Vec<Token>>, n: usize, counts: &mut Vec<usize>) -> ([Token; 2], usize) {
        // fill preallocated vec
        counts[0..n * n].fill(0);

        encodings.into_iter()
                 .flat_map(|encoding| encoding.array_windows::<2>())
                 .for_each(|&[a, b]| {
                    counts[a.0 + b.0 * n] += 1;
                 });

        let (i, &count) =
            counts.par_iter()
                  .enumerate()
                  .max_by_key(|&(_, count)| count)
                  .unwrap();

        ([Token(i % n), Token(i / n)], count)
    }

    /// A slower but more memory efficient `most_common_pair` function.
    fn most_common_pair_low_memory(encodings: &Vec<Vec<Token>>) -> ([Token; 2], usize) {
        let mut counts = HashMap::with_hasher(FxBuildHasher);

        encodings.into_iter()
                 .flat_map(|encoding| encoding.array_windows::<2>())
                 .for_each(|&pair| {
                     *counts.entry(pair).or_insert(0) += 1;
                 });

        let out =
            counts.into_par_iter()
                  .max_by_key(|&(_, i)| i)
                  .unwrap();

        out
    }

    /// Slower than single threaded on a Ryzen 5 9600X with all 12 threads, even for large corpuses
    fn _most_common_pair_multithread(encodings: &Vec<Vec<Token>>, n: usize, counts: &mut Vec<AtomicUsize>) -> ([Token; 2], usize) {
        encodings.into_par_iter()
                 .flat_map(|encoding| encoding.par_array_windows::<2>())
                 .for_each(|&[Token(a), Token(b)]| {
                     counts[a + b * n].fetch_add(1, Ordering::Relaxed);
                 });

        let (i, count) =
            (0..n * n).into_par_iter()
                      .map(|i| (i, counts[i].swap(0, Ordering::Relaxed))) // also resets value to 0
                      .max_by_key(|&(_, count)| count)
                      .unwrap();

        ([Token(i % n), Token(i / n)], count)
    }

    /// Encodes a String into a `Box<[Token]>`.
    fn encode(&self, text: &String) -> Tokenization {
        let mut encoding: Vec<Token> =
            text.chars()
                .map(|ch| *self.encoding_table.get(&ch).expect("Character encountered in input string isn't in tokenizer vocab!"))
                .collect();

        for rule in &self.rules {
            rule.apply_ip(&mut encoding);
        }

        Tokenization::from_vec(encoding)
    }

    /// Decodes a `Box<[Token]>` into a String.
    fn decode(&self, tokenization: Tokenization) -> String {
        tokenization.tokens
                    .iter()
                    .map(|token| &self.vocab[token.0])
                    .join("")
    }

    /// Creates a JSON representation of the tokenizer in a String
    fn to_json(&self) -> String {
        let encoding_table =
            self.encoding_table.iter()
                               .map(|(ch, tok)| format!("\"{}\":{}", ch.escape_default().collect::<String>(), tok.0))
                               .join(",");

        let vocab =
            self.vocab.iter()
                      .map(|voc| format!("\"{}\"", voc.escape_default().collect::<String>()))
                      .join(",");

        let rules =
            self.rules.iter()
                      .map(|rule| format!("[{},{},{}]", rule.a.0, rule.b.0, rule.replacement.0))
                      .join(",");

        format!("{{\"vocab\":[{}],\"rules\":[{}],\"encoding_table\":{{{}}}}}", vocab, rules, encoding_table)
    }

    /// Saves a JSON representation of the tokenizer to a file
    fn save<T: AsRef<Path>>(&self, filename: T) -> std::io::Result<()> {
        let json = self.to_json();

        write(filename, json)?;

        Ok(())
    }
}


enum Argument {
    None,
    Files,
    Dirs,
    VocabSize,
    MinFrequency,
    LowMemory,
    TokenizerPath,
    TokenizationPath,
}

impl Argument {
    fn from_string(string: String) -> Self {
        match string.as_str() {
            "-f" | "--files"      => Argument::Files,
            "-d" | "--dirs"       => Argument::Dirs,
            "-v" | "--vocab-size" => Argument::VocabSize,
            "--min-freq"          => Argument::MinFrequency,
            "--low-mem"           => Argument::LowMemory,
            "--tokenizer-path"    => Argument::TokenizerPath,
            "--tokenization-path" => Argument::TokenizationPath,
            other               => error_exit(format!("Invalid argument: {}", other))
        }
    }
}


fn read_file<T: AsRef<Path>>(filename: T) -> String
{
    read_to_string(filename).unwrap().chars().filter(|&ch| ch != '\r').collect()
}


fn error_exit(message: String) -> ! {
    println!("\n{message}\n");

    std::process::exit(0);
}


fn format_commas(number: usize) -> String {
    number.to_string()
          .chars()
          .rev()
          .chunks(3)
          .into_iter()
          .map(|chunk| chunk.collect::<String>())
          .join(",")
          .chars()
          .rev()
          .collect()
}



fn main() {
    // parse command line arguments
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut argument = Argument::None;
    let mut filenames = Vec::new();
    let mut vocab_size = 0;
    let mut min_frequency = 2;
    let mut low_memory = false;
    let mut tokenizer_path = None;
    let mut tokenization_path = None;

    for arg in args {
        match argument {
            Argument::None => {
                argument = Argument::from_string(arg);

                match argument {
                    Argument::LowMemory => {
                        low_memory = true;
                        argument = Argument::None;
                    },
                    _ => {}
                }
            }
            Argument::Files => {
                if arg.starts_with('-') {
                    argument = Argument::from_string(arg);

                } else {
                    filenames.push(arg);
                }
            }
            Argument::Dirs => {
                if arg.starts_with('-') {
                    argument = Argument::from_string(arg);

                } else {
                    for entry in fs::read_dir(&arg).unwrap_or_else(|_| error_exit(format!("Failure reading directory: {}", arg))) {
                        let entry = entry.unwrap_or_else(|_| error_exit(format!("Failure reading directory: {}", arg)));
                        let path = entry.path();

                        if path.is_file() {
                            filenames.push(path.to_string_lossy().to_string());
                        }
                    }
                }
            }
            Argument::VocabSize => {
                vocab_size = str::parse::<usize>(arg.as_str()).unwrap_or_else(|_| error_exit(format!("Invalid value for --vocab-size: {}", arg)));

                argument = Argument::None;
            }
            Argument::MinFrequency => {
                min_frequency = str::parse::<usize>(arg.as_str()).unwrap_or_else(|_| error_exit(format!("Invalid value for --min-freq: {}", arg)));
                
                if min_frequency < 2 {
                    error_exit(format!("Invalid value for --min-freq: {}, value must be >= 2", arg));
                }

                argument = Argument::None;
            }
            Argument::LowMemory => {
                // should be unreachable
                panic!("ERROR: Invalid state reached. --low-mem flag didn't reset argument");
            }
            Argument::TokenizerPath => {
                tokenizer_path = Some(arg);

                argument = Argument::None;
            }
            Argument::TokenizationPath => {
                tokenization_path = Some(arg);

                argument = Argument::None;
            }
        }
    }

    let total = Instant::now();

    if filenames.is_empty() { error_exit(format!("One of -f and -d is required!")) }

    let start = Instant::now();
    let corpus: Vec<String> = filenames.iter().map(read_file).collect();
    let size = corpus.iter().map(|text| text.len()).sum();
    println!("Loaded corpus ({} files) in {:?}", filenames.len(), start.elapsed());

    let start = Instant::now();
    let (tokenizer, tokenization) = BPETokenizer::from_corpus(&corpus, vocab_size, min_frequency, low_memory);
    let tokenization_size: usize = tokenization.tokenizations.iter().map(|tokz| tokz.tokens.len()).sum();
    println!("Trained tokenizer on {} tokens in {:?}", format_commas(size), start.elapsed());

    let start = Instant::now();
    if let Some(path) = tokenizer_path {
        tokenizer.save(path).unwrap();
    }
    if let Some(path) = tokenization_path {
        tokenization.save(path).unwrap();
    }
    println!("Saved in {:?}", start.elapsed());

    println!("Finished in {:?}", total.elapsed());
    println!();
    println!("Number of chars: {}", tokenizer.encoding_table.len());
    println!("Vocab size: {}", tokenizer.vocab.len());
    println!("Compression: {} -> {} ({:.2}%)", format_commas(size), format_commas(tokenization_size), 100.0 - (tokenization_size as f32) / (size as f32) * 100.0);
}
