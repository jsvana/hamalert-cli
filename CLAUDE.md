# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

hamalert-cli is a Rust CLI tool for managing HamAlert.org triggers. It allows amateur radio operators to create alerts for specific callsigns and import callsign lists from Ham2K PoLo notes.

## Build Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Run all tests
cargo fmt --all -- --check     # Check formatting
cargo clippy --all-targets --all-features -- -D warnings  # Lint (warnings are errors)
```

## CI Requirements

All PRs must pass:
- `cargo fmt` formatting check
- `cargo clippy` with warnings as errors
- `cargo test` with all features

## Architecture

This is a single-file Rust application (`src/main.rs`) with ~420 lines. The monolithic design is intentional for a simple CLI tool.

**Key components in main.rs:**
- `Config` struct: Loads credentials from `~/.config/hamalert/config.toml`
- `Cli` / `Commands` enums: clap-based CLI parsing with two subcommands
- `login()`: Authenticates with HamAlert.org, returns session-enabled client
- `add_trigger()`: POSTs trigger data to HamAlert API
- `parse_polo_notes_content()`: Extracts callsigns from Ham2K PoLo format (well-tested)
- `fetch_polo_notes()`: HTTP GET for remote PoLo notes files

**Data flow:**
1. Parse CLI args → load config → login to HamAlert.org
2. For `add-trigger`: POST all callsigns as a single comma-separated trigger
3. For `import-polo-notes`: Fetch URL → parse callsigns → POST each as trigger (or dry-run)

## API Integration

The tool authenticates via POST to `https://hamalert.org/ajax/user_login` and creates triggers via `https://hamalert.org/ajax/trigger_update`. Session cookies are maintained in the reqwest client.

## Testing

Tests are unit tests for `parse_polo_notes_content()` only (13 tests). No integration tests for API calls exist to avoid external dependencies.

```bash
cargo test                           # Run all tests
cargo test test_parse_polo_notes     # Run specific test group
```

## Configuration

Users must create `~/.config/hamalert/config.toml`:
```toml
username = "your_username"
password = "your_password"
```

## Release Process

Git tags matching `v*` trigger automated multi-platform builds (Linux x64/ARM64, macOS x64/ARM64, Windows x64) via GitHub Actions.
