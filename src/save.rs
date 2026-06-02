
use std::{collections::HashMap, fs::read_to_string, io, time::Instant};

use itertools::Itertools;



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
    vocab: HashMap<char, Token>,
    token_map: HashMap<Token, String>,
    rules: Vec<Rule>
}

impl BPETokenizer {
    pub fn new(vocab: HashMap<char, Token>, token_map: HashMap<Token, String>, rules: Vec<Rule>) -> Self {
        Self {
            vocab,
            token_map,
            rules
        }
    }

    pub fn from_file(filename: &String) -> Result<Self, io::Error> {
        Self::from_file_with_vocab_size(filename, 0)
    }

    pub fn from_file_with_vocab_size(filename: &String, vocab_size: usize) -> Result<Self, io::Error> {
        Ok(Self::from_text_with_vocab_size(&read_to_string(filename)?, vocab_size))
    }

    pub fn from_text(text: &String) -> Self {
        Self::from_text_with_vocab_size(text, 0)
    }

    pub fn from_text_with_vocab_size(text: &String, vocab_size: usize) -> Self {
        let vocab: HashMap<char, Token> = 
            text.chars()
                .unique()
                .sorted()
                .enumerate()
                .map(|(i, ch)| (ch, Token(i)))
                .collect();

        let mut token_map: HashMap<Token, String> =
            vocab.iter()
                 .map(|(ch, &tok)| (tok, ch.to_string()))
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

            let combined_str = format!("{}{}", token_map.get(&a).unwrap(), token_map.get(&b).unwrap());
            
            token_map.insert(Token(token), combined_str);
            rules.push(rule);
            token += 1;
        }

        Self::new(vocab, token_map, rules)
    }

    fn most_common_pair(tokens: &Vec<Token>, n: usize) -> ((Token, Token), usize) {
        let mut counts = vec![0; n * n];

        for i in 0..tokens.len() - 1 {
            counts[tokens[i].0 + tokens[i + 1].0 * n] += 1
        }

        let most_common_i = counts.iter().position_max().unwrap();

        ((Token(most_common_i % n), Token(most_common_i / n)), counts[most_common_i])
    }

    pub fn num_tokens(&self) -> usize {
        self.token_map.len()
    }

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

    pub fn decode(&self, tokenization: Box<[Token]>) -> String {
        tokenization.iter()
                    .map(|token| self.token_map.get(token)
                    .expect("Invalid token encountered in decoding!"))
                    .join("")
    }
}



fn main() {
    let text = read_to_string("texts/tiny-shakespeare.txt").unwrap();

    let start = Instant::now();
    let tokenizer = BPETokenizer::from_text_with_vocab_size(&text, 1024);
    println!("Time: {:?}", start.elapsed());

    let start = Instant::now();
    let encoded = tokenizer.encode(&text);
    println!("Encoding time: {:?}", start.elapsed());

    let start = Instant::now();
    let decoded = tokenizer.decode(encoded.clone());
    println!("Decoding time: {:?}", start.elapsed());

    // println!("Original text: {}", text);
    // println!("Encoded text: {:?}", encoded);
    // println!("Encoded text: {:?}", encoded.iter().map(|tok| tokenizer.token_map.get(tok).unwrap()).collect::<Vec<&String>>());
    // println!("Decoded text: {:?}", decoded);

    println!("Vocab ({}): {:?}", tokenizer.vocab.len(), tokenizer.vocab.keys().sorted().collect::<Vec<&char>>());
    println!("Number of tokens: {}", tokenizer.num_tokens());
    println!("Space use: {} -> {} -> {}", text.len(), encoded.len(), decoded.len());
    println!();
    println!("Encodings:");
    for rule in tokenizer.rules.iter().take(25) {
        println!("\"{}\" + \"{}\" -> \"{}\"", tokenizer.token_map.get(&rule.a).unwrap().replace("\n", "\\n"), tokenizer.token_map.get(&rule.b).unwrap().replace("\n", "\\n"), tokenizer.token_map.get(&rule.replacement).unwrap().replace("\n", "\\n"))
    }
}
