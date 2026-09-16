PREFIX ?= /opt/homebrew
BIN = target/release/ct

all: $(BIN)

$(BIN): src/*.rs src/clipboard/*.rs Cargo.toml
	cargo build --release

test:
	cargo test --release

install: $(BIN)
	install -d $(PREFIX)/bin
	install -m 755 $(BIN) $(PREFIX)/bin/ct

uninstall:
	rm -f $(PREFIX)/bin/ct

windows-check:
	cargo check --release --target x86_64-pc-windows-msvc

clean:
	cargo clean

.PHONY: all test install uninstall windows-check clean
