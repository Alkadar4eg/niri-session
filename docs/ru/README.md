# niri-session

Команда в `$PATH`: **`niri-session-manage`** (не пересекается с настройками niri вокруг имени `niri-session`).

Утилита для **сохранения** и **восстановления** набора окон в [niri](https://github.com/niri-wm/niri): мониторы, рабочие столы, порядок колонок и стеков в колонке. Данные берутся через официальный IPC (`niri-ipc`), команды запуска — из `/proc/<pid>/cmdline`, по идее близкой к [hyprsession](https://github.com/joshurtree/hyprsession) для Hyprland.

**Лицензия:** GNU GPL v3 или новее — см. файл [LICENSE](../../LICENSE).

**English documentation:** [README.md](../../README.md) · [docs/en/README.md](../en/README.md)

## Зависимости

- **Rust:** toolchain не ниже **1.74** (рекомендуется актуальный stable).
- **niri:** версия бинарника должна **совпадать** с версией крейта `niri-ipc`, с которым собран `niri-session-manage`. В проекте зафиксировано `niri-ipc = "=25.11.0"` — используйте niri **25.11.x** или пересоберите `niri-session-manage` под свою версию niri (см. [BUILD.md](BUILD.md)).
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

Бинарник **`niri-session-manage`** окажется в `~/.cargo/bin` (при стандартной настройке rustup).

## Тесты

Минимальная проверка, что сборка и CLI в порядке:

```sh
make test
# или: cargo test --locked --all-targets
```

Есть юнит-тесты (сортировка окон в сессии, JSON roundtrip, чтение `/proc/1/cmdline` на Linux) и интеграционные smoke-тесты бинарника (`--help`, `--version`, ошибки без режима и без `NIRI_SOCKET`). Подробнее — [BUILD.md](BUILD.md).

## Быстрый старт

Сохранить текущую раскладку в файл:

```sh
niri-session-manage --save ~/session.json
```

Каталог для файлов сессий по умолчанию — **`[session].default_session_dir`** в `~/.config/niri-session/niri-session.conf` или **`NIRI_SESSION_DIR`**; иначе `~/.config/niri-session/sessions`. Имя без пути (`foo.json`) сохраняется/загружается в этом каталоге; **`--save`** / **`--load`** без аргумента используют **`session.json`** там (см. [CONFIG.md](CONFIG.md)).

Восстановить (последовательный фокус столов и **запуск процессов без ожидания окон** — см. [LOAD_RESTORE.md](LOAD_RESTORE.md)):

```sh
niri-session-manage --load ~/session.json
```

**«Мягкое» завершение:** сохранить сессию в файл из **`[session].graceful_shutdown_name`** (по умолчанию имя **`last`** в каталоге сессий) и закрыть все окна:

```sh
niri-session-manage --graceful-shutdown
```

Позже восстановить именно этот снимок:

```sh
niri-session-manage --load-last
```

Поле **`graceful_shutdown_name`**, разрешение пути и несовместимость с **`--save`/`--load`** — в [CONFIG.md](CONFIG.md).

Для окон с непереносимой `command` в JSON (например X11 через `xwayland-satellite`) задайте в `[[launch]]` поле **`resolve`** (basename проблемной программы или `-listenfd`), `app_id` / заголовок и реальную `command` в `~/.config/niri-session/niri-session.conf` или через `--config`. Секция **`[load]`** задаёт паузы между шагами и уведомления (`notify-send` при ошибке запуска; по умолчанию включено). Подробно: [CONFIG.md](CONFIG.md).

Параметры задержек при загрузке (мс) и переменные окружения описаны в [LOAD_RESTORE.md](LOAD_RESTORE.md). Для отладки: **`-d` / `--debug`** — подробный журнал в stderr (IPC, окна, команды, паузы).

## Документация

| Документ | Содержание |
|----------|------------|
| [SESSION_FORMAT.md](SESSION_FORMAT.md) | Формат JSON-сессии, поле `schema` |
| [LOAD_RESTORE.md](LOAD_RESTORE.md) | Поведение `--load`, тайминги, ограничения |
| [TROUBLESHOOTING.md](TROUBLESHOOTING.md) | Типичные ошибки |
| [BUILD.md](BUILD.md) | Сборка, Makefile, версии niri |
| [CONFIG.md](CONFIG.md) | TOML `[[launch]]`, `[session]`, `--graceful-shutdown` / `--load-last`, `--config` |

## Ограничения (MVP)

Нет фонового авто-сохранения. Для непереносимых команд в JSON используется конфиг [CONFIG.md](CONFIG.md), а не отдельный «bridge» как в hyprsession. Восстановление раскладки **эвристическое**; тяжёлые случаи (форки Chromium, окна без PID) — в [TROUBLESHOOTING.md](TROUBLESHOOTING.md).
