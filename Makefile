PREFIX ?= /usr/local
DESTDIR ?=
CARGO ?= cargo

.PHONY: all release debug install clean fmt clippy test

all: release

release:
	$(CARGO) build --locked --release

debug:
	$(CARGO) build --locked

install: release
	install -d "$(DESTDIR)$(PREFIX)/bin"
	install -m 755 target/release/niri-session "$(DESTDIR)$(PREFIX)/bin/niri-session"

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --all-targets --locked -- -D warnings

test:
	$(CARGO) test --locked --all-targets

clean:
	$(CARGO) clean
