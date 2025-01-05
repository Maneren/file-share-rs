# File Share

Fast Rust-powered HTTP file server with beautiful web-based GUI

## Features

- Viewing files and folders in the web browser
- Downloading individual files
- Downloading folders as on-the-fly created archives (zip, tar, tar.gz, tar.zst)
- Creating new folders
- Uploading files
- Material Design Icons
- Blazingly fast thanks to async Rust and the [Leptos framework](https://leptos.dev/)
- Multiple instances can be run at the same time
- Allows picking the directory to share with a native GUI picker

## Preview

![Screenshot](.github/assets/screenshot.png)

## Usage

```txt
Fast Rust-powered HTTP file server with beautiful web-based GUI

Usage: file-share [OPTIONS] [TARGET_DIR]

Arguments:
  [TARGET_DIR]
          Path to the directory to share
          
          [default: .]

Options:
  -p, --port <PORT>
          Port to listen on
          
          [default: 3000]

  -q, --qr
          Show QR codes that link to the site

  -i, --interfaces <INTERFACES>...
          IP address(es) of interfaces on which file-share will be available
          
          Accepts comma separated list of both IPv4 and IPv6 addresses
          
          [default: 0.0.0.0,::]

  -P, --picker
          Open a GUI file picker to choose the target directory
          
          Overrides `TARGET_DIR`

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Installation

Download the binary from GitHub Releases and put it in `$PATH`.

## Compilation

You'll need [`cargo-leptos`](https://github.com/leptos-rs/cargo-leptos). You can
get it either by compiling it from the source or downloading a binary using
[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall).

```sh
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
