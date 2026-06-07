
use std::{collections::HashMap, fs::{read_to_string, write}, path::Path, time::Instant};

use itertools::Itertools;


fn read_file<T: AsRef<Path>>(filename: T) -> String
{
    read_to_string(filename).unwrap().chars().filter(|&ch| ch != '\r').collect()
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token(usize);


pub struct Rule {
    a: Token,
    b: Token,
    replacement: Token
}

impl Rule {
    fn new(a: Token, b: Token, replacement: Token) -> Self {
        Self { a, b, replacement }
    }

    fn apply_into(&self, tokens: &Vec<Token>, out: &mut Vec<Token>) {
        let mut i = 0;

        while i < tokens.len() {
            // combine two tokens
            if i < tokens.len() - 1 && self.a == tokens[i] && self.b == tokens[i + 1] {
                out.push(self.replacement);
                i += 2;
            }
            
            // keep token
            else {
                out.push(tokens[i]);
                i += 1;
            }
        }
    }
}


pub struct BPETokenizer {
    vocab: Vec<String>,
    rules: Vec<Rule>,
    encoding_table: HashMap<char, Token>
}

impl BPETokenizer {
    pub fn new(vocab: Vec<String>, rules: Vec<Rule>, encoding_table: HashMap<char, Token>) -> Self {
        Self {
            vocab,
            rules,
            encoding_table
        }
    }

    /// Creates a tokenizer from a String.
    /// 
    /// `vocab_size` is the maximum number of vocab chunks the tokenizer will create, unlimited if 0.
    /// 
    /// `min_frequency` is the lowest frequency in the text at which pairs will still be combined. Should be >= 2
    pub fn from_text(text: &String, vocab_size: usize, min_frequency: usize, low_memory: bool) -> Self {
        let mut vocab: Vec<String> =
            text.chars()
                .unique()
                .sorted()
                .map(|ch| ch.to_string())
                .collect();

        let encoding_table: HashMap<char, Token> = 
            vocab.iter()
                 .enumerate()
                 .map(|(i, ch)| (ch.chars().next().unwrap(), Token(i)))
                 .collect();

        if 0 < vocab_size && vocab_size < encoding_table.len() { panic!("Parameter `vocab_size` (={}) is less than the text's vocab length of {}", vocab_size, encoding_table.len()) }

        let mut rules = Vec::with_capacity(vocab_size);
        let mut token = vocab.len();

        let mut buffer: Vec<Token> = Vec::with_capacity(text.len());
        let mut encoding: Vec<Token> =
            text.chars()
                .map(|ch| *encoding_table.get(&ch).unwrap())
                .collect();

        while vocab_size == 0 || token < vocab_size {
            let ((a, b), n) = if low_memory {
                Self::most_common_pair_low_memory(&encoding, token)
            } else {
                Self::most_common_pair(&encoding, token)
            };

            // stop combining if frequency of pair < min_frequency
            if n < min_frequency { break }

            let rule = Rule::new(a, b, Token(token));
            rule.apply_into(&encoding, &mut buffer);

            // switch buffer to encoding and clear the new buffer
            std::mem::swap(&mut encoding, &mut buffer);
            buffer.clear();

            let combined_str = format!("{}{}", vocab[a.0], vocab[b.0]);

            vocab.push(combined_str);
            rules.push(rule);
            token += 1;
        }

        Self::new(vocab, rules, encoding_table)
    }

    /// Returns the most common common pair of consecutive tokens and how many times it appears.
    fn most_common_pair(tokens: &Vec<Token>, n: usize) -> ((Token, Token), usize) {
        let mut counts = vec![0; n * n];

        for i in 0..tokens.len() - 1 {
            counts[tokens[i].0 + tokens[i + 1].0 * n] += 1
        }

        let most_common_i = counts.iter().position_max().unwrap();

        ((Token(most_common_i % n), Token(most_common_i / n)), counts[most_common_i])
    }

    /// A slower but more memory efficient `most_common_pair` function.
    fn most_common_pair_low_memory(tokens: &Vec<Token>, _: usize) -> ((Token, Token), usize) {
        let mut counts = HashMap::new();

        for i in 0..tokens.len() - 1 {
            counts.entry((tokens[i], tokens[i + 1]))
                  .and_modify(|i| *i += 1)
                  .or_insert(1usize);
        }

        let most_common = counts.into_iter().max_by_key(|&(_, i)| i).unwrap();

        most_common
    }

    /// Encodes a String into a `Box<[Token]>`.
    pub fn encode(&self, text: &String) -> Box<[Token]> {
        let mut buffer: Vec<Token> = Vec::with_capacity(text.len());
        let mut encoding: Vec<Token> =
            text.chars()
                .map(|ch| *self.encoding_table.get(&ch).expect("Character encountered in input string isn't in tokenizer vocab!"))
                .collect();

        for rule in &self.rules {
            rule.apply_into(&encoding, &mut buffer);

            // switch buffer to encoding and clear the new buffer
            std::mem::swap(&mut encoding, &mut buffer);
            buffer.clear();
        }

        encoding.into_boxed_slice()
    }

    /// Decodes a `Box<[Token]>` into a String.
    pub fn decode(&self, tokenization: Box<[Token]>) -> String {
        tokenization.iter()
                    .map(|token| &self.vocab[token.0])
                    .join("")
    }

    fn to_json(&self) -> String {
        let encoding_table =
            self.encoding_table.iter()
                               .map(|(ch, tok)| format!("\"{}\": {}", ch.escape_default().collect::<String>(), tok.0))
                               .join(", ");

        let vocab =
            self.vocab.iter()
                      .map(|voc| format!("\"{}\"", voc.escape_default().collect::<String>()))
                      .join(", ");

        let rules =
            self.rules.iter()
                      .map(|rule| format!("[{}, {}, {}]", rule.a.0, rule.b.0, rule.replacement.0))
                      .join(", ");

        format!("{{\"vocab\": [{}], \"rules\": [{}], \"encoding_table\": {{{}}}}}", vocab, rules, encoding_table)
    }

    fn save<T: AsRef<Path>>(&self, filename: T) -> std::io::Result<()> {
        let json = self.to_json();

        write(filename, json)?;

        Ok(())
    }
}



fn error_exit(message: String) -> ! {
    println!("\n{message}\n");

    std::process::exit(0);
}



fn main() {
    enum Ident {
        None,
        File,
        VocabSize,
        MinFrequency,
        LowMemory
    }

    // parse command line arguments
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut ident = Ident::None;
    let mut filename = None;
    let mut vocab_size = 0;
    let mut min_frequency = 2;
    let mut low_memory = false;

    for arg in args {
        match ident {
            Ident::None => {
                ident = match arg.as_str() {
                    "-f" | "--file"       => Ident::File,
                    "-v" | "--vocab-size" => Ident::VocabSize,
                    "--min-freq"          => Ident::MinFrequency,
                    "--low-mem"           => Ident::LowMemory,
                    other           => error_exit(format!("Invalid argument: {}", other))
                }
            }
            Ident::File => {
                filename = Some(arg);

                ident = Ident::None;
            }
            Ident::VocabSize => {
                vocab_size = str::parse::<usize>(arg.as_str()).unwrap_or_else(|_| error_exit(format!("Invalid value for --vocab-size: {}", arg)));

                ident = Ident::None;
            }
            Ident::MinFrequency => {
                min_frequency = str::parse::<usize>(arg.as_str()).unwrap_or_else(|_| error_exit(format!("Invalid value for --min-freq: {}", arg)));
                
                if min_frequency < 2 {
                    error_exit(format!("Invalid value for --min-freq: {}, value must be >= 2", arg));
                }

                ident = Ident::None;
            }
            Ident::LowMemory => {
                low_memory = str::parse::<bool>(arg.as_str()).unwrap_or_else(|_| error_exit(format!("Invalid value for --low-mem: {} (should be \"true\" or \"false\")", arg)));

                ident = Ident::None;
            }
        }
    }

    let start = Instant::now();

    let filename = filename.unwrap_or_else(|| error_exit(format!("Argument -f is required!")));
    
    println!("Tokenizing {filename}");

    let text = read_file(filename);
    let tokenizer = BPETokenizer::from_text(&text, vocab_size, min_frequency, low_memory);
    let tokenized = tokenizer.encode(&text);

    println!("Tokenization created in {:?}", start.elapsed());
    println!();
    println!("Vocab size: {}", tokenizer.vocab.len());
    println!("Compression: {} -> {} ({:.2}%)", text.len(), tokenized.len(), 100.0 - tokenized.len() as f32 / text.len() as f32 * 100.0);

    tokenizer.save("tokenization.tokr").unwrap();
}
