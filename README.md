# niri-session

Installed command: **`niri-session-manage`** (avoids clashing with niri’s own session-related naming).

A utility to **save** and **restore** a set of windows in [niri](https://github.com/niri-wm/niri): monitors, workspaces, column order, and stack order within a column. Data comes from the official IPC (`niri-ipc`); launch commands are read from `/proc/<pid>/cmdline`, in spirit similar to [hyprsession](https://github.com/joshurtree/hyprsession) for Hyprland.

**License:** GNU GPL v3 or later — see [LICENSE](LICENSE).

**Russian documentation:** [docs/ru/README.md](docs/ru/README.md)

## Dependencies

- **Rust:** toolchain **1.74** or newer (current stable recommended).
- **niri:** the niri binary version must **match** the `niri-ipc` version **niri-session-manage** was built with. This project pins `niri-ipc = "=25.11.0"` — use niri **25.11.x** or rebuild **niri-session-manage** for your niri version (see [docs/en/BUILD.md](docs/en/BUILD.md)).
- Environment variable **`NIRI_SOCKET`**: path to niri’s IPC socket. It is usually set automatically inside a niri session; without it, save and load are unavailable.

## Installation

### From source (`make`)

```sh
git clone <URL> niri-session
cd niri-session
make release
sudo make install PREFIX=/usr/local
```

`PREFIX` defaults to `/usr/local`; for packaging use `DESTDIR`, for example:

```sh
make install DESTDIR=/tmp/pkg PREFIX=/usr
```

### Script

```sh
./scripts/install.sh PREFIX=/usr/local
```

(equivalent to `make install` from the repository root.)

### Via Cargo

```sh
cargo install --locked --path .
```

The **`niri-session-manage`** binary ends up in `~/.cargo/bin` (with a default rustup setup).

## Tests

Minimal check that the build and CLI work:

```sh
make test
# or: cargo test --locked --all-targets
```

There are unit tests (window ordering in a session, JSON roundtrip, reading `/proc/1/cmdline` on Linux) and integration smoke tests for the binary (`--help`, `--version`, errors without a mode and without `NIRI_SOCKET`). Details: [docs/en/BUILD.md](docs/en/BUILD.md).

## Quick start

Save the current layout to a file:

```sh
niri-session-manage --save ~/session.json
```

The default directory for session files is **`[session].default_session_dir`** in `~/.config/niri-session/niri-session.conf`, or **`NIRI_SESSION_DIR`**; otherwise `~/.config/niri-session/sessions`. A bare filename (`foo.json`) is saved/loaded in that directory; **`--save`** / **`--load`** with no argument use **`session.json`** there (see [docs/en/CONFIG.md](docs/en/CONFIG.md)).

Restore (sequential workspace focus and **process launch without waiting for windows** — see [docs/en/LOAD_RESTORE.md](docs/en/LOAD_RESTORE.md)):

```sh
niri-session-manage --load ~/session.json
```

**Graceful shutdown:** save the session to the file from **`[session].graceful_shutdown_name`** (by default the name **`last`** in the session directory) and close all windows:

```sh
niri-session-manage --graceful-shutdown
```

Restore that snapshot later:

```sh
niri-session-manage --load-last
```

The **`graceful_shutdown_name`** field, path resolution, and incompatibility with **`--save`/`--load`** are covered in [docs/en/CONFIG.md](docs/en/CONFIG.md).

For windows whose `command` in JSON is not portable (e.g. X11 via `xwayland-satellite`), set **`resolve`** in `[[launch]]` (basename of the problematic binary or `-listenfd`), `app_id` / title, and the real `command` in `~/.config/niri-session/niri-session.conf` or via `--config`. The **`[load]`** section sets pauses between steps and notifications (`notify-send` on launch failure; enabled by default). Details: [docs/en/CONFIG.md](docs/en/CONFIG.md).

Load timing (ms) and environment variables are described in [docs/en/LOAD_RESTORE.md](docs/en/LOAD_RESTORE.md). For debugging, **`-d` / `--debug`** writes a verbose log to stderr (IPC, windows, commands, pauses).

## Documentation

| Document | Contents |
|----------|----------|
| [docs/en/SESSION_FORMAT.md](docs/en/SESSION_FORMAT.md) | JSON session format, `schema` field |
| [docs/en/LOAD_RESTORE.md](docs/en/LOAD_RESTORE.md) | `--load` behavior, timings, limitations |
| [docs/en/TROUBLESHOOTING.md](docs/en/TROUBLESHOOTING.md) | Common issues |
| [docs/en/BUILD.md](docs/en/BUILD.md) | Build, Makefile, niri versions |
| [docs/en/CONFIG.md](docs/en/CONFIG.md) | TOML `[[launch]]`, `[session]`, `--graceful-shutdown` / `--load-last`, `--config` |

## Limitations (MVP)

No background auto-save. Non-portable commands in JSON use the config ([CONFIG.md](docs/en/CONFIG.md)), not a separate “bridge” like hyprsession. Layout restoration is **heuristic**; difficult cases (Chromium forks, windows without PID) are in [docs/en/TROUBLESHOOTING.md](docs/en/TROUBLESHOOTING.md).
