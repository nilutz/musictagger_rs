.PHONY: help release-patch release-minor release-major install-hooks build test fmt clippy clean

help:
	@echo "Available targets:"
	@echo "  make build          - Build the project"
	@echo "  make test           - Run tests"
	@echo "  make fmt            - Format code with cargo fmt"
	@echo "  make clippy         - Run clippy linter"
	@echo "  make clean          - Clean build artifacts"
	@echo ""
	@echo "  make install-hooks  - Install git hooks"
	@echo ""
	@echo "  make release-patch  - Create a patch release (0.1.13 -> 0.1.14)"
	@echo "  make release-minor  - Create a minor release (0.1.13 -> 0.2.0)"
	@echo "  make release-major  - Create a major release (0.1.13 -> 1.0.0)"

build:
	cargo build --release

test:
	cargo test

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets --all-features

clean:
	cargo clean

install-hooks:
	@echo "Installing git hooks..."
	@ln -sf ../../hooks/pre-commit .git/hooks/pre-commit
	@echo "✓ Pre-commit hook installed"

release-patch:
	@./scripts/release.sh patch

release-minor:
	@./scripts/release.sh minor

release-major:
	@./scripts/release.sh major
