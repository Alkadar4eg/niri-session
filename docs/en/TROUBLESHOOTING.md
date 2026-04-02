# Troubleshooting

## `NIRI_SOCKET is not set`

The tool must run **inside a niri session** where `NIRI_SOCKET` is set. Check:

```sh
echo "$NIRI_SOCKET"
```

If empty, you are not on niri or the session does not export the variable (rare). Run from a terminal on niri.

## IPC errors / version mismatch

Messages like “unexpected IPC response” or niri errors on `Action` often mean **version mismatch**: your niri build does not match the `niri-ipc` API **niri-session-manage** was compiled against. Build the project with `niri-ipc` matching your niri (`niri --version`), or install niri from the same release as the dependency in `Cargo.toml`.

## `--graceful-shutdown` closed all windows

**`niri-session-manage --graceful-shutdown`** saves JSON then calls IPC **`CloseWindow`** for every window: no confirmation and no distinction for “important” apps. Unsaved data in applications may be lost — use deliberately (e.g. before `niri msg action quit` or leaving the session).

## Slow or stuttering `--load`

By default load **waits** for new windows after each spawn (until **`spawn_deadline`**), with pauses from **`[load]`** / CLI. If niri or the disk cannot keep up, increase `ipc_settle_ms` and optionally `spawn_start_delay_ms` (or `--ipc-settle-ms`, `--spawn-start-delay-ms`). See [LOAD_RESTORE.md](LOAD_RESTORE.md).

On **launch** failure (or missing `[[launch]]` rule), the log shows `window skipped`; with notifications enabled, `notify-send` is used (needs `libnotify`).

## Empty or invalid JSON

- Ensure the file was written completely and is valid JSON.
- The `schema` field must be supported by your `niri-session-manage` version.

## Empty window list after `--save`

Only windows with a known **PID** and a non-empty command line in `/proc` are saved. Windows without PID (e.g. some portals) are not included in the MVP.

## X11 apps (Chrome, etc.) and `xwayland-satellite`

On niri, X11 clients go through **xwayland-satellite**. The session JSON may contain a `command` like:

```text
xwayland-satellite :1 -listenfd …
```

That is **not** a command to relaunch the app (session file descriptors). On **`--save`** those strings stay in JSON with `app_id` and `title` — layout is preserved. On **`--load`** you need a TOML **`[[launch]]`** rule: `app_id` / `title_contains`, **`resolve`** (`-listenfd` or a basename like `xwayland-satellite`), and the real `command`. See [CONFIG.md](CONFIG.md).

If there is no rule — an error with a hint to check `app_id`/`title` and add a section to `~/.config/niri-session/niri-session.conf` or pass `--config /path`.
