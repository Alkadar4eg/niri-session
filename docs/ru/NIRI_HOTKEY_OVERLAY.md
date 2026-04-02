# Оверлей хоткеев niri (`show-hotkey-overlay`)

В niri есть действие **`show-hotkey-overlay`**: показывает подсказку по привязкам из `config.kdl`.

## Привязка в `config.kdl`

Если в `~/.config/niri/config.kdl` ещё нет сочетания, добавьте в блок `binds` (пример — **Super+Shift+/**, как часто делают для «справки по клавишам»):

```kdl
binds {
    Mod+Shift+Slash { show-hotkey-overlay; }
}
```

Имя модификатора (`Mod`, `Super` и т.д.) должно соответствовать вашему стилю конфига niri.

## Без правки файла

Проверить оверлей из терминала:

```sh
niri msg action show-hotkey-overlay
```

(выполняйте **внутри** сессии niri, с работающим IPC.)
