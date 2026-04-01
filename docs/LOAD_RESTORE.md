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

## Параметры `--load` (CLI, env, конфиг)

Все значения таймингов — в **миллисекундах**.

**Приоритет:** аргумент CLI → переменная окружения → секция **`[load]`** в `niri-session.conf` → встроенное значение по умолчанию.

| Флаг | Env | Дефолт (без конфига) | Назначение |
|------|-----|----------------------|------------|
| `--spawn-poll-ms` | `NIRI_SESSION_SPAWN_POLL_MS` | `50` | Интервал опроса `Request::Windows` при ожидании нового окна после spawn. |
| `--spawn-timeout-ms` | `NIRI_SESSION_SPAWN_TIMEOUT_MS` | **`2000`** (2 с) | Ожидание появления окна с PID запущенного процесса. Для тяжёлых приложений увеличьте здесь, в `[load]` или в env. |
| `--ipc-settle-ms` | `NIRI_SESSION_IPC_SETTLE_MS` | `80` | Пауза после IPC, меняющих фокус/раскладку, и между шагами выравнивания. |
| `--spawn-start-delay-ms` | `NIRI_SESSION_SPAWN_START_DELAY_MS` | `0` | Задержка перед первым опросом после spawn. |
| `--no-notify-on-spawn-failure` | `NIRI_SESSION_NOTIFY_ON_SPAWN_FAILURE` (`true`/`false`/`0`/`1`) | уведомления **вкл.** | Не вызывать `notify-send`, если не удалось выполнить `spawn` или окно не появилось за `spawn_timeout_ms`. |
| `--config` | — | — | Путь к TOML (`[load]` + `[[launch]]`). Без флага: `~/.config/niri/niri-session.conf`, если есть. |

Уведомления: при ошибке запуска или таймауте окна вызывается **`notify-send`** (пакет `libnotify`), если не отключено. Текст на русском в теле уведомления.

Пример:

```sh
niri-session --load ~/session.json --spawn-poll-ms 100 --spawn-timeout-ms 8000
```

Постоянные настройки удобно держать в `[load]` в `niri-session.conf` (см. [CONFIG.md](CONFIG.md)).

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
