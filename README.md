
# BPE Tokenizer

A byte-pair encoding tokenizer written in Rust.

Can be trained on a corpus of text with parameters for limiting vocabulary size and/or compression level.

Intended to be run as a CLI.

## Usage

Once compiled, the executable can be run in command line.

```
./target/release/bpe_tokenizer.exe [-f | --files] text1.txt text2.txt ...
                                   [-d | --dirs] /dir1 /dir2 ...
                                   [-v | --vocab-size] 1024
                                   --min-freq
                                   --low-mem
```

### Arguments

| Argument | Meaning | Constraints | Default |
| --- | --- | --- | :---: |
| -f or --files | The files to be included. | Must have at least one file or dir. | N/A |
| -d or --dirs | The directories to search for files to include. | Must have at least one file or dir. | N/A |
| -v or --vocab-size | The maximum vocabulary size produced. A value of 0 will stop once --min-freq is fulfilled. | Must be >= the number of unique tokens in the corpus. | 0 |
| --min-freq | The lowest frequency in the text at which byte pairs will be combined. | Must be >= 2 | 2 |
| --low-mem | Turns on low memory mode, saving RAM at the cost of speed. Can be undeterministic. | N/A | off |

## Credits

Made by Joe Hopkins
