
use std::{collections::HashMap, fs::read_to_string, path::Path, time::Instant};

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

    fn to_string(&self) -> String {
        format!("[{}, {}, {}]", self.a.0, self.b.0, self.replacement.0)
    }
}


pub struct BPETokenizer {
    vocab: HashMap<char, Token>,
    token_map: Vec<String>,
    rules: Vec<Rule>
}

impl BPETokenizer {
    pub fn new(vocab: HashMap<char, Token>, token_map: Vec<String>, rules: Vec<Rule>) -> Self {
        Self {
            vocab,
            token_map,
            rules
        }
    }

    /// Creates a tokenizer from the text contained in a file with a given max vocab size. if `vocab_size` is 0, it will continue until no pairs remain (not recommended for large input strings).
    pub fn from_file(filename: &String, vocab_size: usize) -> Self {
        Self::from_text(&read_file(filename), vocab_size)
    }

    /// Creates a tokenizer from a String with a given max vocab size. if `vocab_size` is 0, it will continue until no pairs remain (not recommended for large input strings).
    pub fn from_text(text: &String, vocab_size: usize) -> Self {
        let vocab: HashMap<char, Token> = 
            text.chars()
                .unique()
                .sorted()
                .enumerate()
                .map(|(i, ch)| (ch, Token(i)))
                .collect();

        if 0 < vocab_size && vocab_size < vocab.len() { panic!("Parameter `vocab_size` (={}) is less than the text's vocab length of {}", vocab_size, vocab.len()) }

        let mut token_map: Vec<String> =
            vocab.iter()
                 .sorted_by_key(|&(_, tok)| tok.0)
                 .map(|(ch, _)| ch.to_string())
                 .collect();

        let mut rules = Vec::with_capacity(vocab_size);
        let mut token = token_map.len();

        let mut buffer: Vec<Token> = Vec::with_capacity(text.len());
        let mut encoding: Vec<Token> =
            text.chars()
                .map(|ch| *vocab.get(&ch).unwrap())
                .collect();

        while vocab_size == 0 || token < vocab_size {
            let ((a, b), n) = Self::most_common_pair(&encoding, token);

            // if combining only one pair done
            if n < 2 { break }

            let rule = Rule::new(a, b, Token(token));
            rule.apply_into(&encoding, &mut buffer);

            // switch buffer to encoding and clear the new buffer
            std::mem::swap(&mut encoding, &mut buffer);
            buffer.clear();

            let combined_str = format!("{}{}", token_map[a.0], token_map[b.0]);

            token_map.push(combined_str);
            rules.push(rule);
            token += 1;
        }

        Self::new(vocab, token_map, rules)
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
    fn _most_common_pair_low_memory(tokens: &Vec<Token>, _: usize) -> ((Token, Token), usize) {
        let mut counts = HashMap::new();

        for i in 0..tokens.len() - 1 {
            counts.entry((tokens[i], tokens[i + 1]))
                  .and_modify(|i| *i += 1)
                  .or_insert(1usize);
        }

        let most_common = counts.into_iter().max_by_key(|&(_, i)| i).unwrap();

        most_common
    }

    /// Returns the number of tokens in the tokenizer's vocabulary.
    pub fn num_tokens(&self) -> usize {
        self.token_map.len()
    }

    /// Encodes a String into a `Box<[Token]>`.
    pub fn encode(&self, text: &String) -> Box<[Token]> {
        let mut buffer: Vec<Token> = Vec::with_capacity(text.len());
        let mut encoding: Vec<Token> =
            text.chars()
                .map(|ch| *self.vocab.get(&ch).expect("Character encountered in input string isn't in tokenizer vocab!"))
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
                    .map(|token| &self.token_map[token.0])
                    .join("")
    }
}



fn main() {
    let text = read_file("texts/tiny-shakespeare.txt");

    let start = Instant::now();
    let tokenizer = BPETokenizer::from_text(&text, 1024);
    println!("Time: {:?}", start.elapsed());

    let start = Instant::now();
    let encoded = tokenizer.encode(&text);
    println!("Encoding time: {:?}", start.elapsed());

    let start = Instant::now();
    let decoded = tokenizer.decode(encoded.clone());
    println!("Decoding time: {:?}", start.elapsed());

    println!("Vocab ({}): {:?}", tokenizer.vocab.len(), tokenizer.vocab.keys().sorted().collect::<Vec<&char>>());
    println!("Number of tokens: {}", tokenizer.num_tokens());
    println!("Space use: {} -> {} -> {}", text.len(), encoded.len(), decoded.len());
    println!();
    println!("Encodings:");
    for rule in tokenizer.rules.iter().take(25) {
        println!("\"{}\" + \"{}\" -> \"{}\"", &tokenizer.token_map[rule.a.0].replace("\n", "\\n"), &tokenizer.token_map[rule.b.0].replace("\n", "\\n"), &tokenizer.token_map[rule.replacement.0].replace("\n", "\\n"))
    }
}
