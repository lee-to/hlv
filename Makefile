.PHONY: build release test clippy lint check install uninstall clean help

# Default target
all: install

# Debug build
build:
	cargo build

# Release build (optimized)
release:
	cargo build --release

# Run all tests
test:
	cargo test

# Clippy lint
clippy:
	cargo clippy -- -D warnings

# Full lint: clippy + fmt check
lint: clippy
	cargo fmt -- --check

# Format code
fmt:
	cargo fmt

# Run hlv check on example project fixture
check: build
	./target/debug/hlv check --root tests/fixtures/example-project

# Install release binary to /usr/local/bin
install: release
	cp target/release/hlv /usr/local/bin/hlv
	@echo "Installed hlv to /usr/local/bin/hlv"

# Uninstall
uninstall:
	rm -f /usr/local/bin/hlv
	@echo "Removed /usr/local/bin/hlv"

# Clean build artifacts
clean:
	cargo clean

# Help
help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  build     Debug build"
	@echo "  release   Release build (optimized)"
	@echo "  test      Run all tests"
	@echo "  clippy    Run clippy linter"
	@echo "  lint      Clippy + fmt check"
	@echo "  fmt       Auto-format code"
	@echo "  check     Build + run hlv check on example project"
	@echo "  install   Release build + copy to /usr/local/bin"
	@echo "  uninstall Remove from /usr/local/bin"
	@echo "  clean     Remove build artifacts"
