# Loading a session (`--load`)

**`niri-session-manage --load-last`** loads from the file set in config as **`[session].graceful_shutdown_name`** (by default file **`last`** in the [session directory](CONFIG.md#session-directory)); it is **`--load`** with a known path (the same snapshot **`--graceful-shutdown`** writes). Timings, **`[[launch]]`**, and the flag table below apply the same as for **`--load`**.

---

`niri-session-manage --load [file]` (no argument — **`session.json`** in the [session directory](CONFIG.md#session-directory)):

1. Reads JSON (see [SESSION_FORMAT.md](SESSION_FORMAT.md)).
2. Sorts windows by `(output, workspace_idx, column, tile)`.
3. Merges consecutive **tiled** windows with the same `(output, workspace_idx, column)` into groups (**column with multiple tiles**).
4. For each group (or single window), in order:
   - focuses the right monitor (`FocusMonitor`) and workspace (`FocusWorkspace`);
   - if the group has **multiple** windows: before each spawn — `FocusColumn` with the saved **1-based** column index; with **waiting** enabled (default), after spawn `Request::Windows` is polled until a new window id appears or **`spawn_deadline`** expires; after the **second and later** launches **`ConsumeWindowIntoColumn`** is called (the new window on the right is pulled into this column);
   - picks **argv for launch** (`[[launch]]` when needed — see [CONFIG.md](CONFIG.md));
   - runs the process with `std::process::Command` (not IPC `Spawn`);
   - pauses from `[load]` / CLI (`ipc_settle_ms`, `spawn_start_delay_ms`).
   - after a **tiled** group is placed: if JSON has **`column_width`** / **`window_height`**, applies **`SetColumnWidth`** (focused column) and **`SetWindowHeight`** per window (matched by workspace slot and `app_id` / `title`), same semantics as `niri msg action set-column-width` / `set-window-height` with a fixed logical size.

**By default**, after each successful spawn load **waits** for a new window in niri’s list (up to **`spawn_deadline`** ms), then continues — startup order stays consistent. **`--no-await`** or **`[load].no_await = true`** disable waiting (“fire and forget”; for columns with several tiles **`ConsumeWindowIntoColumn`** may run too early).

Single windows and floating modes do not use `ConsumeWindowIntoColumn`. Order in JSON and pauses determine which workspace has focus before each start.

Before each `spawn`, `niri-session-manage` checks whether a **matching window already exists** on the target workspace: for tiled windows, same output/workspace index, tile position `(column, tile)`, and `app_id`; for floating windows, same `app_id` and `title` (substring match). If so, that step is **skipped** (no duplicate launch). Use **`--open-forcefully`** or **`[load].open_forcefully = true`** to always spawn anyway.

**Per-window errors** (no `[[launch]]`, `spawn` failure, empty command, IPC error when focusing) **do not abort** load: that window is skipped, stderr logs `window skipped: …`, other windows continue. If any error occurred, a summary is printed at the end and **exit code 1**; full success is **0**.

## `--load` parameters (CLI, env, config)

All values are in **milliseconds**.

**Priority:** CLI argument → environment variable → **`[load]`** in `niri-session.conf` → built-in default.

| Flag | Env | Default (no config) | Purpose |
|------|-----|---------------------|---------|
| `--ipc-settle-ms` | `NIRI_SESSION_IPC_SETTLE_MS` | `80` | Pause after IPC focus (monitor/workspace) and **after each successful spawn** before the next step. |
| `--spawn-start-delay-ms` | `NIRI_SESSION_SPAWN_START_DELAY_MS` | `0` | Extra pause after spawn before the next window (reduce CPU/disk load when starting many apps). |
| `--no-await` | — | waiting **on** | Do not wait for a new window after spawn; next step right after `ipc_settle` / `spawn_start_delay` pauses. |
| `--spawn-deadline` | `NIRI_SESSION_SPAWN_DEADLINE_MS` | `10000` | Max milliseconds to wait for a new window id in niri after spawn (while waiting is enabled). |
| `--no-notify-on-spawn-failure` | `NIRI_SESSION_NOTIFY_ON_SPAWN_FAILURE` (`true`/`false`/`0`/`1`) | notifications **on** | Do not run `notify-send` on launch errors (missing `[[launch]]`, `spawn` failure, empty command). |
| `--open-forcefully` | — | off | Do not skip when a matching window already exists; always run `spawn` (see also `[load].open_forcefully` in [CONFIG.md](CONFIG.md)). |
| `--config` | — | — | Path to TOML (`[load]` + `[[launch]]`). Without the flag: `~/.config/niri-session/niri-session.conf` if it exists. |
| `-d` / `--debug` | — | off | Verbose log to **stderr**: `NIRI_SOCKET`, each IPC request/response, windows on `--save`, command parsing and `spawn` on `--load`, pauses. |
| `--load-last` | — | — | Load from **`[session].graceful_shutdown_name`**; see [CONFIG.md](CONFIG.md) (subsection **`--graceful-shutdown` and `--load-last`**). |

Notifications: on command preparation or `spawn` failure **`notify-send`** is used (`libnotify` package) unless disabled.

Example:

```sh
niri-session-manage --load ~/session.json --ipc-settle-ms 120 --spawn-start-delay-ms 200
```

Persistent settings fit well in `[load]` in `niri-session.conf` (see [CONFIG.md](CONFIG.md)).

## `[[launch]]` config (required for xwayland-satellite, etc.)

If the saved `command` is **non-portable** (built-in check: `-listenfd`; for bridges like **xwayland-satellite** set **`resolve`** in a rule), `--load` without a matching TOML rule errors. See `resolve` and matching order in [CONFIG.md](CONFIG.md). Example: [niri-session.conf.example](../niri-session.conf.example).

Alternative: manually edit `command` in JSON — but the next `--save` will no longer match “as in `/proc`”.

## PWA, Flatpak

Prefer `[[launch]]` with `command = ["flatpak", "run", …]` or `app_id` + `title_contains` for the window. See [CONFIG.md](CONFIG.md).

## X11 and xwayland-satellite

X11 windows on niri go through **xwayland-satellite**; JSON often has `argv` like `xwayland-satellite … -listenfd …`. Add config rules with **`resolve = "xwayland-satellite"`** (and if needed `resolve = "-listenfd"`) and `app_id` — see [TROUBLESHOOTING.md](TROUBLESHOOTING.md) and [CONFIG.md](CONFIG.md).

## Limitations

- **Multiple windows in one column** (`column` + different `tile` in JSON): on `--load` windows start in ascending `tile` order; after each subsequent launch IPC **`ConsumeWindowIntoColumn`** is called (the new window on the right is pulled into the column with index **`column`**). Before each spawn **`FocusColumn`** runs with that index. Stack order inside the column and column mode (tabs, etc.) may **not** match the snapshot; slow windows can hit the poll deadline — increase **`spawn_deadline`** (or **`ipc_settle_ms`** as poll interval). Column width and per-tile heights from JSON (**`column_width`**, **`window_height`**) are applied **after** the group is built; old session files without those fields skip this step.
- **Floating windows**: positions and sizes are not carried over after launch.
- **PID / fork:** windows are not matched to the process after spawn; the new window is found by **set difference** of ids from `Request::Windows`.
