
# BPE Tokenizer

A byte-pair encoding tokenizer written in Rust.

Can be trained on a corpus of text with parameters for limiting vocabulary size and/or compression level.

Intended to be run as a CLI.

## Usage

Once compiled, the executable can be run in command line.

```
./target/release/bpe_tokenizer.exe [-f | --files] text1.txt text2.txt ...
                                   [-v | --vocab-size] 1024
                                   --min-freq
                                   --low-mem
```

### Arguments

| Argument | Meaning | Constraints | Default |
| --- | --- | --- | :---: |
| -f or --files | The corpus of files to be included. | Must have at least one file. | N/A |
| -v or --vocab-size | The maximum vocabulary size produced. A value of 0 will stop once --min-freq is fulfilled. | Must be >= the number of unique tokens in the corpus. | 0 |
| --min-freq | The lowest frequency in the text at which byte pairs will be combined. | Must be >= 2 | 2 |
| --low-mem | Whether or not to use low memory mode, saving RAM at the cost of speed. | Must be "true" or "false" | false |

## Credits

Made by Joe Hopkins
