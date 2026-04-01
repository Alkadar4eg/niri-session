# Конфиг запуска (`niri-session.conf`)

При `--load` для окон с **непереносимой** командной строкой (например `xwayland-satellite … -listenfd …`) нужно указать, **что именно запускать**. Это задаётся TOML-файлом с таблицами `[[launch]]`.

## Расположение

| Способ | Путь |
|--------|------|
| По умолчанию | `$XDG_CONFIG_HOME/niri/niri-session.conf` (обычно `~/.config/niri/niri-session.conf`) |
| Явно | `niri-session --load session.json --config /путь/к/файлу.conf` |

Если **не** передан `--config` и файла по умолчанию нет — правил нет: непереносимые окна при загрузке завершатся ошибкой `MissingLaunchOverride` с подсказкой.

Если передан **`--config`** и файла нет — ошибка «config file not found».

## Формат

Каждая секция `[[launch]]` — одно правило. **Первое подходящее** правило выигрывает; ставьте более узкие правила (и `app_id`, и `title_contains`) **выше** общих.

Поля:

| Поле | Обязательно | Смысл |
|------|-------------|--------|
| `app_id` | нет* | Должен совпасть с `app_id` окна из JSON (точное совпадение). |
| `title_contains` | нет* | Подстрока в заголовке окна (`title` из JSON). |
| `command` | да | `argv` для `exec` (массив строк). |

\* У каждой секции должен быть задан **хотя бы один** из `app_id` или `title_contains`.

Пример (`~/.config/niri/niri-session.conf`):

```toml
# Узкое правило — первым
[[launch]]
app_id = "Google-chrome"
title_contains = "VK Messenger"
command = ["google-chrome-stable"]

# Общий запуск Chrome, если заголовок другой
[[launch]]
app_id = "Google-chrome"
command = ["google-chrome-stable"]

[[launch]]
app_id = "org.mozilla.firefox"
command = ["flatpak", "run", "org.mozilla.firefox"]
```

Геометрия (монитор, рабочий стол, колонка, плитка) по-прежнему берётся из JSON сессии; конфиг задаёт только **команду запуска**.

См. также [LOAD_RESTORE.md](LOAD_RESTORE.md), [TROUBLESHOOTING.md](TROUBLESHOOTING.md).
