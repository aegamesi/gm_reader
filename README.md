# GM Reader

This is a project to read and extract file formats associated with Game Maker (primarily focusing on 8.1 and earlier).


## Supported Formats
* Game Maker 6.0 - 8.1 EXEs
* Game Maker 5.3/5.3A EXEs (detection only)


## CLI

A small CLI is provided for demo purposes.

`cargo run <path to input file> [<optional path to output file>]`

The given input file will be read, its format detected, and it will be decoded into memory.

If an output file path is given, the decoded game will be written to the path in MessagePack
format, following the internal schema (see `game.rs`). **Note**: all resources are stored uncompressed,
including resources such as sprites, sounds, and backgrounds, and included files. As a result,
the output file size may be fairly large. Compress it with a program such as `gzip` to get a file
size that's similar in size to the input file.


## License

This project is dual-licensed under the MIT and Apache licenses.
