# Building

## Requirements

- Rust **1.74+** (see `rust-version` in `Cargo.toml`).
- The **`niri-ipc`** dependency is pinned to an **exact** version in `Cargo.toml` (`=25.11.0`, etc.) and must match your installed **niri**.

## Commands

| Command | Result |
|---------|--------|
| `cargo build` | Debug build |
| `cargo build --release` | Release build |
| `make` / `make release` | Same as `cargo build --locked --release` |
| `make install` | Installs the binary to `$(DESTDIR)$(PREFIX)/bin` |
| `make clippy` | clippy with `-D warnings` |
| `make test` / `cargo test --locked --all-targets` | Unit and integration tests (CLI smoke, session format, `/proc` on Linux) |
| `make fmt` | `cargo fmt --all` |

## Makefile variables

| Variable | Default | Description |
|----------|---------|---------------|
| `PREFIX` | `/usr/local` | Install root |
| `DESTDIR` | empty | Staging prefix (packaging) |
| `CARGO` | `cargo` | cargo executable |

Example:

```sh
make install DESTDIR=/tmp/stage PREFIX=/usr
```

## Changing the niri version

1. Check niri: `niri --version`.
2. Find a matching `niri-ipc` release on [crates.io](https://crates.io/crates/niri-ipc).
3. Update `niri-ipc = "=…"` in `Cargo.toml`.
4. Run `cargo update -p niri-ipc`, rebuild, and commit the new `Cargo.lock`.
