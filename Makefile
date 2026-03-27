.PHONY: all build build-debug run run-debug test-rust clean up down logs help

RUST_DIR = rust

all: build

# Build Rust gateway binary (release)
build:
	cargo build --release --manifest-path=$(RUST_DIR)/Cargo.toml

# Build Rust gateway binary (debug)
build-debug:
	cargo build --manifest-path=$(RUST_DIR)/Cargo.toml

# Run the Rust gateway directly (DeerFlow must be running separately)
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

# Start full stack via docker compose (Rust Gateway + DeerFlow)
up:
	docker compose up -d --build

# Stop full stack
down:
	docker compose down

# View logs
logs:
	docker compose logs -f

# Run Rust tests
test-rust:
	cargo test --manifest-path=$(RUST_DIR)/Cargo.toml

# Clean all builds
clean:
	cargo clean --manifest-path=$(RUST_DIR)/Cargo.toml

# Full setup
setup: build
	@echo "Setup complete."
	@echo "1. Copy .env.example to .env and fill in values"
	@echo "2. Start DeerFlow: docker compose up deerflow -d"
	@echo "3. Run gateway: make run"

help:
	@echo "Codex AI — Rust Gateway + DeerFlow Backend"
	@echo ""
	@echo "  make build      - Build Rust gateway (release)"
	@echo "  make run        - Build and run gateway"
	@echo "  make up         - Start full stack (docker compose)"
	@echo "  make down       - Stop full stack"
	@echo "  make logs       - View docker compose logs"
	@echo "  make test-rust  - Run Rust tests"
	@echo "  make clean      - Clean build artifacts"
	@echo "  make setup      - Full setup with instructions"
