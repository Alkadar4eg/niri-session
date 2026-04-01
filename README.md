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

Восстановить (запуск процессов и раскладка через IPC):

```sh
niri-session --load ~/session.json
```

Параметры задержек при загрузке (мс) и переменные окружения описаны в [docs/LOAD_RESTORE.md](docs/LOAD_RESTORE.md).

## Документация

| Документ | Содержание |
|----------|------------|
| [docs/SESSION_FORMAT.md](docs/SESSION_FORMAT.md) | Формат JSON-сессии, поле `schema` |
| [docs/LOAD_RESTORE.md](docs/LOAD_RESTORE.md) | Поведение `--load`, тайминги, ограничения |
| [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | Типичные ошибки |
| [docs/BUILD.md](docs/BUILD.md) | Сборка, Makefile, версии niri |

## Ограничения (MVP)

Нет фонового авто-сохранения, нет отдельного конфига «мостов» для Flatpak/PWA — при необходимости правьте JSON вручную (см. [docs/LOAD_RESTORE.md](docs/LOAD_RESTORE.md)). Восстановление раскладки **эвристическое**; тяжёлые случаи (форки процессов вроде Chromium, окна без PID) описаны в [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md).
