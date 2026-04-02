# Configuration (`niri-session.conf`)

A single TOML file: **`[session]`** section (default session directory and filename for “graceful shutdown”), **`[load]`** section (timings and notifications for `--load`), and **`[[launch]]`** tables (override command for non-portable `command` values in JSON).

## Location

| Method | Path |
|--------|------|
| Default | `$XDG_CONFIG_HOME/niri-session/niri-session.conf` (usually `~/.config/niri-session/niri-session.conf`) |
| Explicit | `niri-session --load session.json --config /path/to/file.conf` |

If **`--config`** is **not** passed and the default file is missing — there are no rules: non-portable windows will fail on load with `MissingLaunchOverride` and a hint.

The default path used to be `~/.config/niri/niri-session.conf`; when upgrading, move the file to **`~/.config/niri-session/niri-session.conf`** (or pass the old path with `--config`). Previously **`[session]`** used `default_save_path` (a single file); now you set a **directory** `default_session_dir`, and the filename when no argument is given is `session.json`.

If **`--config`** is passed but the file does not exist — error “config file not found”.

## Session directory

| Field | Type | Meaning |
|-------|------|---------|
| `default_session_dir` | string | Directory containing JSON session files. Strings with a **`~/`** prefix expand to the home directory. |
| `graceful_shutdown_name` | string | Filename (or path — see below) for **`--graceful-shutdown`** and **`--load-last`**. Default is **`last`**: in the session directory this becomes **`…/sessions/last`** (you can set e.g. **`last.json`**). |

**Priority** for the directory: environment **`NIRI_SESSION_DIR`** → then **`[session].default_session_dir`** in config → otherwise built-in **`$XDG_CONFIG_HOME/niri-session/sessions`** (usually `~/.config/niri-session/sessions`).

How paths work for **`--save`** / **`--load`**:

- **Absolute path** — used as-is (full path to the file).
- **Single filename** without `/` (e.g. `work.json`) — **`directory/work.json`** where `directory` comes from the priority above.
- **Relative path with subdirs** (e.g. `backup/foo.json`) — relative to the current working directory.

**`niri-session --save`** with no argument and **`niri-session --load`** with no argument read/write **`session.json`** in that directory.

### `--graceful-shutdown` and `--load-last`

- **`niri-session --graceful-shutdown`** — first saves the current session to JSON at the path derived from **`[session].graceful_shutdown_name`** (same rules as a single filename: **`directory/graceful_shutdown_name`**), then **closes all windows** via IPC (`CloseWindow` for each id). Handy before leaving the session: state is stored in the “last” session file, then workspaces are empty.
- **`niri-session --load-last`** — same as **`niri-session --load`** with that path (no separate file argument). Timings and **`[[launch]]`** behave like normal load ( **`[load]`** section, CLI flags).

These modes are **incompatible** with **`--save`** and **`--load`**; **`NIRI_SOCKET`** is required, as for save and load.

Example:

```toml
[session]
default_session_dir = "~/sessions/niri"
graceful_shutdown_name = "last.json"
```

## `[load]` section (`--load` parameters)

All fields are optional. Priority: **CLI arguments** or **environment variables** (see [LOAD_RESTORE.md](LOAD_RESTORE.md)) → then values from `[load]` → then built-in defaults.

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `ipc_settle_ms` | integer | `80` | Pause after IPC focus and after each successful spawn (see [LOAD_RESTORE.md](LOAD_RESTORE.md)). |
| `spawn_start_delay_ms` | integer | `0` | Extra pause after spawn before the next window. |
| `no_await` | boolean | `false` | If `true`, do not wait for a window after spawn (like `--no-await` in CLI; CLI wins). |
| `spawn_deadline` | integer | `10000` | Max milliseconds to wait for a new window in niri after spawn (see `--spawn-deadline` in [LOAD_RESTORE.md](LOAD_RESTORE.md)). |
| `notify_on_spawn_failure` | boolean | `true` | Call `notify-send` on command preparation or `spawn` failure. Disable with `false` here or `--no-notify-on-spawn-failure` in CLI. |

Example at the top of the file:

```toml
[load]
ipc_settle_ms = 100
spawn_start_delay_ms = 150
notify_on_spawn_failure = true

[[launch]]
app_id = "Google-chrome"
resolve = "xwayland-satellite"
command = ["google-chrome-stable"]
```

## `[[launch]]` format

Each `[[launch]]` section is one rule. The **first matching** rule wins; put narrower rules (both `app_id` and `title_contains`) **above** broader ones.

For each window on `--load`, rules are checked in order. First **`app_id`** / **`title_contains`** must match (if set). Then:

- if **`resolve`** is set — by default the saved command must match: **basename** of the first JSON `command` argument (e.g. `xwayland-satellite`), or the **`-listenfd`** flag in `argv` when **`resolve = "-listenfd"`** (or `listenfd`). **Exception:** if the rule sets **`title_contains`** and the window title already matches, the rule applies **even** when `argv[0]` does not match `resolve` (e.g. Chrome PWA with a direct `chrome` path while the rule mentions `xwayland-satellite` for another scenario);
- if **`resolve`** is **not** set — the rule matches on **`-listenfd`** in the saved command **or** if **`title_contains`** is set in the rule (narrow title-only rule without tying to `argv[0]`).

If no rule matches and the saved `command` is still considered non-portable (only `-listenfd` with no matching rule), load fails with an error and a hint.

When parsing the saved command, a single JSON element that looks like a full shell command line **is split** like a shell ([shlex](https://docs.rs/shlex/)) so `spawn` gets a correct `argv`.

Fields:

| Field | Required | Meaning |
|-------|----------|---------|
| `app_id` | no* | Must match the window `app_id` from JSON (exact). |
| `title_contains` | no* | Substring in the window title (`title` from JSON). |
| `resolve` | no | Refine match on `argv[0]` / `-listenfd` (see above); with **`title_contains`** in the same rule, override may apply without matching `resolve`. |
| `command` | yes | Command and arguments: **array of strings** in TOML (`["a", "b"]`) or **one string** parsed like a shell (spaces, quotes via [shlex](https://docs.rs/shlex/), e.g. `command = "flatpak run org.app --opt value"`). |

\* Each section must have **at least one** of `app_id` or `title_contains`.

Example (`~/.config/niri-session/niri-session.conf`):

```toml
# Narrow rule first (X11 via xwayland-satellite in JSON)
[[launch]]
app_id = "Google-chrome"
title_contains = "VK Messenger"
resolve = "xwayland-satellite"
command = ["google-chrome-stable"]

# General Chrome launch if title differs
[[launch]]
app_id = "Google-chrome"
resolve = "xwayland-satellite"
command = ["google-chrome-stable"]

[[launch]]
app_id = "org.mozilla.firefox"
command = ["flatpak", "run", "org.mozilla.firefox"]
```

`[[launch]]` sections set the **launch command** for non-portable `command` in JSON; `[load]` sets **pauses**, **waiting for windows**, and notifications. Exact tile geometry from JSON is **not** restored on `--load`; by default steps run **in order while waiting** for each new window (see [LOAD_RESTORE.md](LOAD_RESTORE.md)), or “fire and forget” via `no_await` / `--no-await`.

See also [LOAD_RESTORE.md](LOAD_RESTORE.md), [TROUBLESHOOTING.md](TROUBLESHOOTING.md).
