# File Share

Simple utility to share selected folder over HTTP

## Features

- web page GUI
- downloading files
- downloading folders as on-the-fly created archives (zip, tar, tar.gz, tar.zst)
- creating folders
- uploading files

## Usage

```txt
file-share [OPTIONS] [TARGET_DIR]

Arguments:
  [TARGET_DIR]  Target directory to serve [default: .]

Options:
  -p, --port <PORT>                 Port to serve on [default: 8080]
  -q, --qr                          Show QR code with link to the site
  -i, --interfaces <INTERFACES>...  IP address(es) on which file-share will be available [default: 0.0.0.0,::]
  -h, --help                        Print help
  -V, --version                     Print version
```

## Installation

Download the binary from Github Releases and put it in `$PATH`.

## Compilation

You'll need [`cargo-leptos`](https://github.com/leptos-rs/cargo-leptos). You can
get it either by compiling it from source or downloading a binary using
[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall).

```txt
cargo install cargo-leptos
# or
cargo binstall cargo-leptos
```

Then run `cargo leptos build --release` and the binary will be under `target/release/file-share`.

## License

The source code is licensed under the MIT license.

## Credits

The files and folders icons are [Material Design Icons](https://pictogrammers.com/library/mdi)
licensed under Apache License 2.0 from <https://pictogrammers.com/>.

The app icon is from
[File sharing icons created by smashingstocks - Flaticon](https://www.flaticon.com/free-icons/file-sharing "file sharing icons")
