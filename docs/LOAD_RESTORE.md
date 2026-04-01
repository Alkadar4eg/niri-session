# Загрузка сессии (`--load`)

`niri-session --load <файл>`:

1. Читает JSON (см. [SESSION_FORMAT.md](SESSION_FORMAT.md)).
2. Сортирует окна по `(output, workspace_idx, column, tile)`.
3. Для каждого окна по очереди:
   - фокусирует нужный монитор (`FocusMonitor`);
   - фокусирует рабочий стол по индексу на этом мониторе (`FocusWorkspace`);
   - выбирает **argv для запуска**: если сохранённая `command` «переносимая», она используется как есть; иначе ищется подходящая секция `[[launch]]` в TOML (`~/.config/niri/niri-session.conf` или `--config`), см. [CONFIG.md](CONFIG.md);
   - запускает локально выбранный `command[0]` с аргументами `command[1..]` через `std::process::Command` (не через IPC `Spawn`, чтобы получить **PID** и сопоставить с окном в niri);
   - ждёт появления окна с этим PID в `Request::Windows` (с таймаутом и опросом);
   - при необходимости переносит окно на монитор и рабочий стол (`MoveWindowToMonitor`, `MoveWindowToWorkspace`);
   - для плавающих окон: `MoveWindowToFloating`;
   - для тайловых: пытается выровнять колонку и плитку итерациями `MoveColumnLeft`/`MoveColumnRight` и `MoveWindowUp`/`MoveWindowDown` до совпадения с сохранённой `(column, tile)`.

Раскладка **не гарантируется** на 100% для сложных сценариев (см. ниже).

## Параметры таймингов (CLI)

Все значения в **миллисекундах**. Есть соответствующие переменные окружения (удобно для постоянных настроек).

| Флаг | Env | По умолчанию | Назначение |
|------|-----|----------------|------------|
| `--spawn-poll-ms` | `NIRI_SESSION_SPAWN_POLL_MS` | `50` | Интервал опроса `Request::Windows` при ожидании нового окна после запуска. |
| `--spawn-timeout-ms` | `NIRI_SESSION_SPAWN_TIMEOUT_MS` | `120000` | Максимальное время ожидания появления окна с нужным PID. |
| `--ipc-settle-ms` | `NIRI_SESSION_IPC_SETTLE_MS` | `80` | Пауза после действий IPC, меняющих фокус/раскладку, и между шагами выравнивания. |
| `--spawn-start-delay-ms` | `NIRI_SESSION_SPAWN_START_DELAY_MS` | `0` | Задержка перед **первым** опросом после `spawn` (для медленных клиентов). |
| `--config` | — | — | Путь к TOML с `[[launch]]` (переопределение `command` по `app_id` / `title_contains`). Без флага читается `~/.config/niri/niri-session.conf`, если файл есть. |

Приоритет: **аргументы командной строки** переопределяют переменные окружения, те переопределяют значения по умолчанию.

Пример:

```sh
niri-session --load ~/session.json --spawn-poll-ms 100 --ipc-settle-ms 150
```

## Конфиг `[[launch]]` (обязателен для xwayland-satellite и т.п.)

Если сохранённая `command` **непереносимая** (например `xwayland-satellite` с `-listenfd`), при `--load` без подходящего правила в TOML будет ошибка. Задайте соответствие `app_id` / `title_contains` → реальная `command` в [CONFIG.md](CONFIG.md). Пример файла: [niri-session.conf.example](niri-session.conf.example).

Альтернатива: вручную заменить `command` у окна в JSON — но тогда теряется автоматическое «как в /proc» при следующем `--save`.

## PWA, Flatpak

Предпочтительно прописать `[[launch]]` с `command = ["flatpak", "run", …]` или `app_id` + `title_contains` для нужного окна. См. [CONFIG.md](CONFIG.md).

## X11 и xwayland-satellite

Окна X11 в niri идут через **xwayland-satellite**; в JSON часто сохраняется `argv` вида `xwayland-satellite … -listenfd …` при этом **геометрия** в JSON корректна. Для загрузки добавьте в конфиг правила по `app_id` (например `Google-chrome`) — см. [TROUBLESHOOTING.md](TROUBLESHOOTING.md) и [CONFIG.md](CONFIG.md).

## Ограничения

- **PID:** приложения, которые форкают процесс и открывают Wayland из другого PID (частый случай Chromium), могут не совпасть с PID запуска — окно не будет найдено по таймауту. Увеличьте таймауты или поправьте способ запуска/сопоставления вручную в будущих версиях.
- **Плавающие окна** и **таб-колонки** — лучшее усилие; возможны расхождения.
