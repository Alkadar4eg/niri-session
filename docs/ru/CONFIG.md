# Конфиг (`niri-session.conf`)

Один TOML-файл: секция **`[session]`** (каталог файлов сессий по умолчанию и имя файла для «мягкого» выключения), **`[load]`** (тайминги и уведомления при `--load`), таблицы **`[[launch]]`** (переопределение команды для непереносимых `command` в JSON).

## Расположение

| Способ | Путь |
|--------|------|
| По умолчанию | `$XDG_CONFIG_HOME/niri-session/niri-session.conf` (обычно `~/.config/niri-session/niri-session.conf`) |
| Явно | `niri-session-manage --load session.json --config /путь/к/файлу.conf` |

Если **не** передан `--config` и файла по умолчанию нет — правил нет: непереносимые окна при загрузке завершатся ошибкой `MissingLaunchOverride` с подсказкой.

Раньше путь по умолчанию был `~/.config/niri/niri-session.conf`; при обновлении перенесите файл в **`~/.config/niri-session/niri-session.conf`** (или укажите старый путь через `--config`). Ранее в **`[session]`** использовалось поле `default_save_path` (один файл); теперь задаётся **каталог** `default_session_dir`, а имя файла при отсутствии аргумента — `session.json`.

Если передан **`--config`** и файла нет — ошибка «config file not found».

## Секция `[session]` (каталог сессий)

| Поле | Тип | Смысл |
|------|-----|--------|
| `default_session_dir` | string | Каталог, где лежат JSON-сессии. Строка с префиксом **`~/`** разворачивается в домашний каталог. |
| `graceful_shutdown_name` | string | Имя файла (или путь — см. ниже) для режимов **`--graceful-shutdown`** и **`--load-last`**. По умолчанию **`last`**: в каталоге сессий получается файл **`…/sessions/last`** (при желании задайте, например, **`last.json`**). |

**Приоритет** каталога: переменная окружения **`NIRI_SESSION_DIR`** → затем **`[session].default_session_dir`** в конфиге → иначе встроенный каталог **`$XDG_CONFIG_HOME/niri-session/sessions`** (обычно `~/.config/niri-session/sessions`).

Как используется путь к файлу в **`--save`** / **`--load`**:

- **Абсолютный путь** — используется как есть (полный путь к файлу).
- **Одно имя файла** без `/` (например `work.json`) — файл **каталог/work.json**, где каталог берётся из приоритета выше.
- **Относительный путь с подкаталогами** (например `backup/foo.json`) — от текущей рабочей директории.

**`niri-session-manage --save`** без аргумента и **`niri-session-manage --load`** без аргумента читают/пишут файл **`session.json`** в этом каталоге.

### `--graceful-shutdown` и `--load-last`

- **`niri-session-manage --graceful-shutdown`** — сначала сохраняет текущую сессию в JSON по пути, вычисленному из **`[session].graceful_shutdown_name`** (те же правила, что для одного имени файла: **`каталог/graceful_shutdown_name`**), затем **закрывает все окна** через IPC (`CloseWindow` по id каждого окна). Удобно перед выходом из сессии: состояние остаётся в файле «последней» сессии, после чего столы пустые.
- **`niri-session-manage --load-last`** — то же, что **`niri-session-manage --load`** с этим путём (без отдельного аргумента файла). Тайминги и **`[[launch]]`** — как у обычной загрузке (секция **`[load]`**, флаги CLI).

Режимы **несовместимы** с **`--save`** и **`--load`**; нужен **`NIRI_SOCKET`**, как для сохранения и загрузки.

Пример:

```toml
[session]
default_session_dir = "~/sessions/niri"
graceful_shutdown_name = "last.json"
```

## Секция `[load]` (параметры `--load`)

Все поля необязательны. Приоритет: **аргументы CLI** или **переменные окружения** (см. [LOAD_RESTORE.md](LOAD_RESTORE.md)) → затем значения из `[load]` → затем встроенные значения по умолчанию.

| Поле | Тип | По умолчанию | Смысл |
|------|-----|----------------|--------|
| `ipc_settle_ms` | integer | `80` | Пауза после IPC фокуса и после каждого успешного spawn (см. [LOAD_RESTORE.md](LOAD_RESTORE.md)). |
| `spawn_start_delay_ms` | integer | `0` | Дополнительная пауза после spawn перед следующим окном. |
| `no_await` | boolean | `false` | Если `true` — не ждать появления окна после spawn (аналог `--no-await` в CLI; CLI имеет приоритет). |
| `spawn_deadline` | integer | `10000` | Лимит миллисекунд ожидания нового окна в niri после spawn (см. `--spawn-deadline` в [LOAD_RESTORE.md](LOAD_RESTORE.md)). |
| `notify_on_spawn_failure` | boolean | `true` | Вызывать `notify-send` при ошибке подготовки команды или `spawn`. Отключить: `false` здесь или `--no-notify-on-spawn-failure` в CLI. |
| `open_forcefully` | boolean | `false` | Если `true`, при `--load` всегда выполнять `spawn`, даже если подходящее окно уже есть (как флаг `--open-forcefully`). |

Пример в начале файла:

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

## Формат `[[launch]]`

Каждая секция `[[launch]]` — одно правило. **Первое подходящее** правило выигрывает; ставьте более узкие правила (и `app_id`, и `title_contains`) **выше** общих.

Для каждого окна при `--load` проверяются правила по порядку. Сначала должны совпасть **`app_id`** / **`title_contains`** (если заданы). Затем:

- если **`resolve`** задан — по умолчанию требуется совпадение с сохранённой командой: **basename** первого аргумента `command` из JSON (например `xwayland-satellite`), либо флаг **`-listenfd`** в `argv` при **`resolve = "-listenfd"`** (или `listenfd`). **Исключение:** если в правиле задан **`title_contains`** и заголовок окна ему уже соответствует, правило применяется **даже** когда `argv[0]` не совпадает с `resolve` (например PWA Chrome с прямым путём к `chrome`, а в правиле указан `xwayland-satellite` для другого сценария);
- если **`resolve` не задан** — правило срабатывает при **`-listenfd`** в сохранённой команде **или** если в правиле задан **`title_contains`** (узкое правило по заголовку без привязки к `argv[0]`).

Если ни одно правило не подошло, а сохранённая `command` всё ещё считается непереносимой (только `-listenfd` без подходящего правила), загрузка завершится с ошибкой и подсказкой.

При разборе сохранённой команды один элемент JSON, похожий на целую командную строку с пробелами, **разбивается** как в shell ([shlex](https://docs.rs/shlex/)), чтобы `spawn` получил корректный `argv`.

Поля:

| Поле | Обязательно | Смысл |
|------|-------------|--------|
| `app_id` | нет* | Должен совпасть с `app_id` окна из JSON (точное совпадение). |
| `title_contains` | нет* | Подстрока в заголовке окна (`title` из JSON). |
| `resolve` | нет | Уточнение по `argv[0]` / `-listenfd` (см. список выше); при **`title_contains`** в том же правиле допускается переопределение и без совпадения с `resolve`. |
| `command` | да | Команда и аргументы: **массив строк** TOML (`["a", "b"]`) или **одна строка**, разбираемая как в shell (пробелы, кавычки через [shlex](https://docs.rs/shlex/), например `command = "flatpak run org.app --opt value"`). |

\* У каждой секции должен быть задан **хотя бы один** из `app_id` или `title_contains`.

Пример (`~/.config/niri-session/niri-session.conf`):

```toml
# Узкое правило — первым (X11 через xwayland-satellite в JSON)
[[launch]]
app_id = "Google-chrome"
title_contains = "VK Messenger"
resolve = "xwayland-satellite"
command = ["google-chrome-stable"]

# Общий запуск Chrome, если заголовок другой
[[launch]]
app_id = "Google-chrome"
resolve = "xwayland-satellite"
command = ["google-chrome-stable"]

[[launch]]
app_id = "org.mozilla.firefox"
command = ["flatpak", "run", "org.mozilla.firefox"]
```

Секции `[[launch]]` задают **команду запуска** для непереносимых `command` в JSON; `[load]` — **паузы**, **ожидание появления окон** и уведомления. Точная геометрия тайлов из JSON при `--load` **не восстанавливается**; по умолчанию шаги идут **по очереди с ожиданием** нового окна (см. [LOAD_RESTORE.md](LOAD_RESTORE.md)), либо режим «запустил и забыл» через `no_await` / `--no-await`.

См. также [LOAD_RESTORE.md](LOAD_RESTORE.md), [TROUBLESHOOTING.md](TROUBLESHOOTING.md).
