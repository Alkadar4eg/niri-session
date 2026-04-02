# niri-session

Утилита для **сохранения** и **восстановления** набора окон в [niri](https://github.com/niri-wm/niri): мониторы, рабочие столы, порядок колонок и стеков в колонке. Данные берутся через официальный IPC (`niri-ipc`), команды запуска — из `/proc/<pid>/cmdline`, по идее близкой к [hyprsession](https://github.com/joshurtree/hyprsession) для Hyprland.

**Лицензия:** GNU GPL v3 или новее — см. файл [LICENSE](LICENSE).

## Зависимости

- **Rust:** toolchain не ниже **1.74** (рекомендуется актуальный stable).
- **niri:** версия бинарника должна **совпадать** с версией крейта `niri-ipc`, с которым собран `niri-session`. В проекте зафиксировано `niri-ipc = "=25.11.0"` — используйте niri **25.11.x** или пересоберите `niri-session` под свою версию niri (см. [docs/BUILD.md](docs/BUILD.md)).
- Переменная окружения **`NIRI_SOCKET`**: путь к сокету IPC niri. Обычно выставляется автоматически внутри сессии niri; без неё сохранение и загрузка недоступны.

## Установка

### Из исходников (`make`)

```sh
git clone <URL> niri-session
cd niri-session
make release
sudo make install PREFIX=/usr/local
```

`PREFIX` по умолчанию `/usr/local`; для пакетирования используйте `DESTDIR`, например:

```sh
make install DESTDIR=/tmp/pkg PREFIX=/usr
```

### Скрипт

```sh
./scripts/install.sh PREFIX=/usr/local
```

(эквивалентно `make install` из корня репозитория).

### Через Cargo

```sh
cargo install --locked --path .
```

Бинарник окажется в `~/.cargo/bin` (при стандартной настройке rustup).

## Тесты

Минимальная проверка, что сборка и CLI в порядке:

```sh
make test
# или: cargo test --locked --all-targets
```

Есть юнит-тесты (сортировка окон в сессии, JSON roundtrip, чтение `/proc/1/cmdline` на Linux) и интеграционные smoke-тесты бинарника (`--help`, `--version`, ошибки без режима и без `NIRI_SOCKET`). Подробнее — [docs/BUILD.md](docs/BUILD.md).

## Быстрый старт

Сохранить текущую раскладку в файл:

```sh
niri-session --save ~/session.json
```

Каталог для файлов сессий по умолчанию — **`[session].default_session_dir`** в `~/.config/niri-session/niri-session.conf` или **`NIRI_SESSION_DIR`**; иначе `~/.config/niri-session/sessions`. Имя без пути (`foo.json`) сохраняется/загружается в этом каталоге; **`--save`** / **`--load`** без аргумента используют **`session.json`** там (см. [docs/CONFIG.md](docs/CONFIG.md)).

Восстановить (последовательный фокус столов и **запуск процессов без ожидания окон** — см. [docs/LOAD_RESTORE.md](docs/LOAD_RESTORE.md)):

```sh
niri-session --load ~/session.json
```

**«Мягкое» завершение:** сохранить сессию в файл из **`[session].graceful_shutdown_name`** (по умолчанию имя **`last`** в каталоге сессий) и закрыть все окна:

```sh
niri-session --graceful-shutdown
```

Позже восстановить именно этот снимок:

```sh
niri-session --load-last
```

Поле **`graceful_shutdown_name`**, разрешение пути и несовместимость с **`--save`/`--load`** — в [docs/CONFIG.md](docs/CONFIG.md).

Для окон с непереносимой `command` в JSON (например X11 через `xwayland-satellite`) задайте в `[[launch]]` поле **`resolve`** (basename проблемной программы или `-listenfd`), `app_id` / заголовок и реальную `command` в `~/.config/niri-session/niri-session.conf` или через `--config`. Секция **`[load]`** задаёт паузы между шагами и уведомления (`notify-send` при ошибке запуска; по умолчанию включено). Подробно: [docs/CONFIG.md](docs/CONFIG.md).

Параметры задержек при загрузке (мс) и переменные окружения описаны в [docs/LOAD_RESTORE.md](docs/LOAD_RESTORE.md). Для отладки: **`-d` / `--debug`** — подробный журнал в stderr (IPC, окна, команды, паузы).

Подсказка по хоткеям niri (оверлей `show-hotkey-overlay`): если в `~/.config/niri/config.kdl` ещё нет этой привязки, см. [docs/NIRI_HOTKEY_OVERLAY.md](docs/NIRI_HOTKEY_OVERLAY.md). Строку для вставки в `binds { }` можно вывести командой `niri-session --print-niri-hotkey-overlay-bind`.

## Документация

| Документ | Содержание |
|----------|------------|
| [docs/SESSION_FORMAT.md](docs/SESSION_FORMAT.md) | Формат JSON-сессии, поле `schema` |
| [docs/LOAD_RESTORE.md](docs/LOAD_RESTORE.md) | Поведение `--load`, тайминги, ограничения |
| [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | Типичные ошибки |
| [docs/BUILD.md](docs/BUILD.md) | Сборка, Makefile, версии niri |
| [docs/CONFIG.md](docs/CONFIG.md) | TOML `[[launch]]`, `[session]`, `--graceful-shutdown` / `--load-last`, `--config` |
| [docs/NIRI_HOTKEY_OVERLAY.md](docs/NIRI_HOTKEY_OVERLAY.md) | Хоткей оверлея niri, фрагмент KDL, `niri msg action` |

## Ограничения (MVP)

Нет фонового авто-сохранения. Для непереносимых команд в JSON используется конфиг [CONFIG.md](docs/CONFIG.md), а не отдельный «bridge» как в hyprsession. Восстановление раскладки **эвристическое**; тяжёлые случаи (форки Chromium, окна без PID) — в [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md).
