# ---------------------------------------------------------------------------
#  Biopass – Root Build Orchestrator
#
#  Drives both sub-projects from the repository root:
#    auth/rust/ → Rust PAM module + helper + auth core
#    app/   → Tauri desktop application
#
#  Prerequisites
#    auth : rustup/cargo, V4L2 headers/libs, fprintd
#    app  : bun, rustup/cargo, tauri-cli v2, webkit2gtk, libssl-dev …
# ---------------------------------------------------------------------------

.PHONY: all build build-auth build-app \
        package package-app \
        clean clean-auth clean-app \
        install-deps

# ── Configurable defaults ──────────────────────────────────────────────────
CARGO_PROFILE ?= release
AUTH_RUST_DIR := auth/rust
AUTH_PAM_DIR  := auth/rust/pam
APP_DIR       := app
VERSION       ?= $(shell git describe --tags --always 2>/dev/null || echo "0.1.0")

ifeq ($(CARGO_PROFILE),release)
CARGO_BUILD_FLAGS := --release
else
CARGO_BUILD_FLAGS :=
endif

# ── Top-level aliases ──────────────────────────────────────────────────────
all: build

build: build-auth build-app

package: package-app

# ── auth (Rust / Cargo) ───────────────────────────────────────────────────
build-auth:
	@echo "==> [auth] Building Rust helper and auth core (CARGO_PROFILE=$(CARGO_PROFILE))…"
	cargo build --manifest-path $(AUTH_RUST_DIR)/Cargo.toml $(CARGO_BUILD_FLAGS)
	@echo "==> [auth] Building Rust PAM module (CARGO_PROFILE=$(CARGO_PROFILE))…"
	cargo build --manifest-path $(AUTH_PAM_DIR)/Cargo.toml $(CARGO_BUILD_FLAGS)


clean-auth:
	@echo "==> [auth] Cleaning Rust build artifacts…"
	cargo clean --manifest-path $(AUTH_RUST_DIR)/Cargo.toml
	cargo clean --manifest-path $(AUTH_PAM_DIR)/Cargo.toml

# ── app (Tauri / Bun / Rust) ──────────────────────────────────────────────
# build-app depends on build-auth so the PAM module and helper exist before
# Tauri packages them into the combined Linux bundles.
build-app: build-auth
	@echo "==> [app] Installing JS dependencies…"
	cd $(APP_DIR) && bun install --frozen-lockfile
	@echo "==> [app] Building Tauri application…"
	cd $(APP_DIR) && bun run tauri build

# package-app produces the combined Linux bundles (Tauri app + auth libs bundled inside)
package-app: build-app
	@echo "==> [app] Tauri packages are in app/src-tauri/target/release/bundle/"
	@ls app/src-tauri/target/release/bundle/deb/*.deb \
	     app/src-tauri/target/release/bundle/rpm/*.rpm 2>/dev/null || true

clean-app:
	@echo "==> [app] Cleaning build artifacts…"
	cd $(APP_DIR) && cargo clean --manifest-path src-tauri/Cargo.toml 2>/dev/null || true
	rm -rf $(APP_DIR)/dist

# ── Combined clean ────────────────────────────────────────────────────────
clean: clean-auth clean-app

# ── Help ──────────────────────────────────────────────────────────────────
help:
	@echo ""
	@echo "  Usage: make [target] [VAR=value …]"
	@echo ""
	@echo "  Targets:"
	@echo "    build          Build both auth and app (default)"
	@echo "    build-auth     Build the Rust auth helper and PAM module only"
	@echo "    build-app      Install JS deps + build the Tauri app only"
	@echo "    package        Build + package everything into combined Linux bundles"
	@echo "    package-app    Show Tauri combined bundle output paths"
	@echo "    clean          Remove all build output"
	@echo "    clean-auth     Remove Rust auth build artifacts"
	@echo "    clean-app      Remove app build artifacts"
	@echo ""
	@echo "  Variables:"
	@echo "    CARGO_PROFILE  Cargo profile for auth builds (default: release)"
	@echo "    VERSION        Package version (default: git tag or 0.1.0)"
	@echo ""
