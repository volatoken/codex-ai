.PHONY: all build build-rust setup-python run test clean help

RUST_DIR = rust
PYTHON_DIR = python

all: build

# Build Rust binary (release)
build:
	cargo build --release --manifest-path=$(RUST_DIR)/Cargo.toml

# Build Rust binary (debug)
build-debug:
	cargo build --manifest-path=$(RUST_DIR)/Cargo.toml

# Setup Python virtual environment and install dependencies
setup-python:
ifeq ($(OS),Windows_NT)
	cd $(PYTHON_DIR) && python -m venv .venv && .venv\Scripts\pip install -r requirements.txt
else
	cd $(PYTHON_DIR) && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
endif

# Run the application (Rust binary)
run: build
ifeq ($(OS),Windows_NT)
	.\$(RUST_DIR)\target\release\codex-ai.exe
else
	./$(RUST_DIR)/target/release/codex-ai
endif

# Run in debug mode
run-debug: build-debug
ifeq ($(OS),Windows_NT)
	.\$(RUST_DIR)\target\debug\codex-ai.exe
else
	./$(RUST_DIR)/target/debug/codex-ai
endif

# Run Python tests
test:
	cd $(PYTHON_DIR) && python -m pytest tests/ -v

# Run Rust tests
test-rust:
	cargo test --manifest-path=$(RUST_DIR)/Cargo.toml

# Clean all builds
clean:
	cargo clean --manifest-path=$(RUST_DIR)/Cargo.toml
	find $(PYTHON_DIR) -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true

# Full setup
setup: setup-python build
	@echo "Setup complete. Edit .env then run: make run"

help:
	@echo "Codex AI - Hybrid Rust+Python"
	@echo ""
	@echo "  make build        - Build Rust binary (release)"
	@echo "  make setup-python - Setup Python venv + deps"
	@echo "  make setup        - Full setup (python + rust)"
	@echo "  make run          - Build and run"
	@echo "  make test         - Run Python tests"
	@echo "  make test-rust    - Run Rust tests"
	@echo "  make clean        - Clean build artifacts"
