# ViewIt — discoverable dev/release task runner.
# Thin wrapper over npm scripts + scripts/*.sh + cargo. `make <target>`.

SHELL := /bin/sh
.PHONY: help setup dev-web dev-desktop build-web build-android lint format \
	check test test-rust test-ts clean deny fmt cargo-fmt clippy misuse install-apk smoke

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) | sort | \
	awk 'BEGIN {FS = ":.*## "}; {printf "\033[36m%-14s\033[0m %s\n", $$1, $$2}'

setup: ## Install workspace + Rust deps
	npm install
	cargo fetch

dev-web: ## Run the web app (fastest loop)
	npm run dev:web

dev-desktop: ## Run the Tauri desktop dev build
	npm run dev:desktop

build-web: ## Build web + WASM format parsers
	npm run build:web

build-android: ## Build + install the Android release APK
	bash scripts/android-release.sh

lint: ## Prettier check across TS/Svelte/JS
	npm run lint

format: ## Auto-format TS/Svelte/JS (prettier)
	npm run format

check: ## TypeScript typecheck of packages/platform
	npm run check

test: test-rust test-ts ## Run Rust + vitest unit tests

test-rust: ## Host-side Rust tests (no Android SDK required)
	cargo test --workspace --exclude viewit-mobile --exclude viewit-desktop

test-ts: ## Vitest unit tests for packages/platform
	npm run test:ts

fmt: cargo-fmt ## Rust formatting (cargo fmt)
cargo-fmt: ## Rust formatting (cargo fmt)
	cargo fmt

clippy: ## Rust lints (-D warnings) across host crates
	cargo clippy --workspace --exclude viewit-mobile --exclude viewit-desktop -- -D warnings

deny: ## cargo-deny license/advisory/ban check (if installed)
	cargo deny check

smoke: ## On-device format smoke (requires installed APK + device)
	python3 scripts/android-format-smoke.py

install-apk: ## Install signed APK over USB
	adb install -r dist/viewit-android-arm64-release.apk

clean: ## Remove derived build artifacts
	npm run clean || true
	cargo clean
