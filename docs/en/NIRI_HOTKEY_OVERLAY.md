# Niri hotkey overlay (`show-hotkey-overlay`)

niri exposes **`show-hotkey-overlay`**: a cheat sheet of bindings from `config.kdl`.

## Binding in `config.kdl`

If `~/.config/niri/config.kdl` does not yet map it, add inside the `binds` block (example — **Super+Shift+/**, a common “keyboard help” chord):

```kdl
binds {
    Mod+Shift+Slash { show-hotkey-overlay; }
}
```

Use the modifier naming (`Mod`, `Super`, etc.) that matches the rest of your niri config.

## Without editing the file

Try the overlay from a terminal:

```sh
niri msg action show-hotkey-overlay
```

(Run **inside** a niri session with working IPC.)
