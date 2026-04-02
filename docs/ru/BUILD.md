# Сборка

## Требования

- Rust **1.74+** (см. `rust-version` в `Cargo.toml`).
- Зависимость **`niri-ipc`** зафиксирована **точной** версией в `Cargo.toml` (`=25.11.0` и т.д.) и должна соответствовать установленному **niri**.

## Команды

| Команда | Результат |
|---------|-----------|
| `cargo build` | Отладочная сборка |
| `cargo build --release` | Релизная сборка |
| `make` / `make release` | То же, что `cargo build --locked --release` |
| `make install` | Установка бинарника в `$(DESTDIR)$(PREFIX)/bin` |
| `make clippy` | Проверка clippy с `-D warnings` |
| `make test` / `cargo test --locked --all-targets` | Юнит- и интеграционные тесты (CLI smoke, формат сессии, `/proc` на Linux) |
| `make fmt` | `cargo fmt --all` |

## Makefile: переменные

| Переменная | Значение по умолчанию | Описание |
|------------|------------------------|----------|
| `PREFIX` | `/usr/local` | Корень установки |
| `DESTDIR` | пусто | Префикс для staging (пакеты) |
| `CARGO` | `cargo` | Исполняемый файл cargo |

Пример:

```sh
make install DESTDIR=/tmp/stage PREFIX=/usr
```

## Смена версии niri

1. Узнайте версию niri: `niri --version`.
2. Найдите на [crates.io](https://crates.io/crates/niri-ipc) совпадающий релиз `niri-ipc`.
3. Обновите строку `niri-ipc = "=…"` в `Cargo.toml`.
4. Выполните `cargo update -p niri-ipc` и пересоберите; закоммитьте новый `Cargo.lock`.
