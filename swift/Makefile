PREFIX ?= /opt/homebrew
BIN = .build/release/ct

all: $(BIN)

$(BIN): Sources/ct/*.swift Package.swift
	swift build -c release

install: $(BIN)
	install -d $(PREFIX)/bin
	install -m 755 $(BIN) $(PREFIX)/bin/ct

uninstall:
	rm -f $(PREFIX)/bin/ct

clean:
	rm -rf .build

.PHONY: all install uninstall clean
