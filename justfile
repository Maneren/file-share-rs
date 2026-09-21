# Common project commands. Run `just` with no arguments to list them.
# Requires: cargo-leptos, leptosfmt (plus the nightly toolchain in rust-toolchain.toml).

default:
    @just --list

# Build the server and the WASM client (dev profile).
build:
    cargo leptos build

# Build the server and the WASM client with optimizations.
build-release:
    cargo leptos build --release

# Serve the app (dev profile). Extra args go to the server binary: `just serve files/ -p 3000`.
serve *args:
    cargo leptos serve {{ args }}

# Serve the app with optimizations. Extra args are passed to the server binary.
serve-release *args:
    cargo leptos serve --release {{ args }}

# Serve and automatically rebuild/reload when files change (dev profile).
watch *args:
    cargo leptos watch --clear --hot-reload {{ args }}

# Serve and automatically rebuild/reload when files change, with optimizations.
watch-release *args:
    cargo leptos watch --clear --release {{ args }}

# Format view! macros (leptosfmt) and all Rust code (rustfmt).
fmt:
    leptosfmt app frontend server
    cargo fmt --all

# Check formatting without changing files (e.g. for CI).
fmt-check:
    leptosfmt --check app frontend server
    cargo fmt --all -- --check

# Run the full test suite.
test *args:
    cargo test --workspace {{ args }}

# Lint the workspace, including tests.
clippy:
    cargo clippy --workspace --all-targets --features=hydrate
    cargo clippy --workspace --all-targets --features=ssr
