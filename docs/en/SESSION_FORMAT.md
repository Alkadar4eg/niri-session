# Session file format (JSON)

The file is written by `niri-session-manage --save` and read by `niri-session-manage --load`.

## Schema version

The **`schema`** field (integer): current format version is **`1`**. Breaking structural changes will bump this; old files may need migration.

## Root object

| Field | Type | Description |
|-------|------|-------------|
| `schema` | `number` | Format version (see above). |
| `niri_version` | `string` | niri version string from IPC (`Request::Version`). |
| `outputs` | `object` | Output names → [`Output`](https://docs.rs/niri-ipc/25.11.0/niri_ipc/struct.Output.html) objects from `niri-ipc` (as in `Request::Outputs`). |
| `workspaces` | `array` | Snapshot of workspaces at save time. |
| `windows` | `array` | Windows to restore. |

## `workspaces[]` element

| Field | Type | Description |
|-------|------|-------------|
| `id` | `number` | Internal workspace id in niri (not used as a key on load). |
| `idx` | `number` | Workspace index **on its monitor** (as in IPC). |
| `name` | `string` \| `null` | Workspace name if set. |
| `output` | `string` \| `null` | Output (monitor) name. |

## `windows[]` element

| Field | Type | Description |
|-------|------|-------------|
| `command` | `array` of `string` | Process arguments from `/proc/<pid>/cmdline` at save time (`argv`), with possible tweaks (see below). |
| `app_id` | `string` \| `null` | Wayland `app_id` if present. |
| `title` | `string` \| `null` | Window title (for reference). |
| `output` | `string` | Output name where the window was. |
| `workspace_idx` | `number` | Workspace index on that output (`u8` in IPC). |
| `column` | `number` | Column index in the scrolling layout, **1-based** (like `pos_in_scrolling_layout.0`). |
| `tile` | `number` | Position in the column stack, **1-based** (top tile is 1). |
| `is_floating` | `boolean` | Floating vs tiled. |
| `was_focused` | `boolean` | Present when the window had focus at save time; used to refocus on `--load` (default `false` if omitted). |
| `column_width` | `number` \| omitted | Tiled windows only: column width in **logical pixels** (from IPC `WindowLayout.tile_size` width at save). On `--load`, applied with `SetColumnWidth` after the column is formed. |
| `window_height` | `number` \| omitted | Tiled windows only: tile height in **logical pixels** (from `tile_size` height). On `--load`, applied with `SetWindowHeight` per matched window. |

Windows with unknown PID or no command line in `/proc` at save time are **skipped** (cannot be restored with this model).

**Chrome/Chromium installed PWA** windows may share one browser PID: `/proc` may then have the wrong `--app-id=` for a given window. On **`--save`** and **`--load`**, `niri-session-manage` aligns **`--app-id=`** with **`app_id`** when it looks like `chrome-<id>-…` / `chromium-<id>-…` (opaque id up to the first `-` after the prefix).

For windows like **xwayland-satellite** with **`-listenfd`**, JSON keeps the real `command` from `/proc` (not suitable to relaunch). On **`--load`**, the real `argv` comes from TOML [`[[launch]]`](CONFIG.md) matching `app_id` / `title_contains` and **`resolve`**. Fields `output`, `workspace_idx`, `column`, `tile`, `is_floating` drive **focus order** before each launch. For **tiled** windows, optional **`column_width`** / **`window_height`** restore sizes via IPC after placement (see [LOAD_RESTORE.md](LOAD_RESTORE.md)).

## Minimal example (fragment)

```json
{
  "schema": 1,
  "niri_version": "25.11",
  "outputs": {},
  "workspaces": [],
  "windows": [
    {
      "command": ["foot"],
      "app_id": "foot",
      "title": "foot",
      "output": "HDMI-A-1",
      "workspace_idx": 1,
      "column": 1,
      "tile": 1,
      "is_floating": false
    }
  ]
}
```

In a real file, `outputs` is fully populated; it is shortened here for readability.

## Compatibility

- **niri version** should match the `niri-ipc` dependency of your built `niri-session-manage`.
- Manual edits to `command` are allowed (e.g. to launch Flatpak instead of the binary from `/proc`).
