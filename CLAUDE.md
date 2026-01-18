# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust ecosystem learning project for the Geektime Rust Language Bootcamp (极客时间 Rust 语言训练营). It demonstrates modern Rust patterns and ecosystem tools through practical examples with extensive Chinese-language documentation.

## Development Commands

### Building and Quality Checks
```bash
# Format check (enforced by pre-commit hooks)
cargo fmt -- --check

# Code analysis
cargo check --all

# Linting with strict warnings
cargo clippy --all-targets --all-features --tests --benches -- -D warnings

# Run tests (uses nextest for better performance)
cargo nextest run --all-features

# Run a single test
cargo nextest run --all-features <test_name>

# Dependency security and license checking
cargo deny check

# Spelling check
typos
```

### Pre-commit Hooks
The project uses pre-commit hooks configured in `.pre-commit-config.yaml`. Hooks automatically run:
- `cargo fmt -- --check` - Format verification
- `cargo check --all` - Compile check
- `cargo deny check -d` - Dependency analysis
- `typos` - Spelling correction
- `cargo clippy` - Linting
- `cargo nextest run` - Test execution

## Project Structure

- **Single package lib crate** (not a workspace)
- **`src/lib.rs`**: Public API hub (currently minimal)
- **`examples/`**: Primary demonstration code for patterns and concepts
  - `serde.rs`, `serde1.rs` - Serialization/deserialization patterns
  - `err.rs` - Error handling with anyhow/thiserror
  - `builder.rs` - Builder pattern with derive_builder
  - `enum.rs` - Enum utilities with strum
  - `axum_*.rs` - Web framework integration examples
  - `more.rs` - Additional patterns

## Key Dependencies and Patterns

### Observability (tracing ecosystem)
- **tracing**: Core instrumentation API
- **tracing-subscriber**: Layering, filtering, formatting
- **tracing-appender**: Non-blocking file writer
- **tracing-opentelemetry**: OpenTelemetry bridge

### OpenTelemetry
- **opentelemetry**: API traits for telemetry
- **opentelemetry_sdk**: SDK implementation
- **opentelemetry-otlp**: OTLP exporter with tonic gRPC

### Error Handling
- **anyhow**: Error propagation for application errors
- **thiserror**: Error derivation for library errors

### Build and Test Dependencies
- **axum**: Web framework for examples
- **tokio**: Async runtime
- **serde**: Serialization framework (dev dependency)
- **derive_builder**: Builder pattern derivation
- **strum**: Enum enhancements and string conversion
- **base64**: Base64 encoding
- **http**: HTTP types

## Coding Conventions

- **Formatting**: rustfmt (enforced via pre-commit)
- **Linting**: clippy with `-D warnings` (treats warnings as errors)
- **Testing**: Uses nextest instead of built-in `cargo test`
- **Comments**: Extensive Chinese comments explaining concepts and patterns
- **Documentation**: Educational focus with detailed inline explanations

## CI/CD

GitHub Actions workflow (`.github/workflows/build.yml`):
- Triggers: push to master, tags (v*), PRs to master
- Jobs: format check, cargo check, clippy, nextest tests
- On tags: Generates changelog with git-cliff, creates GitHub release
- Tools: cargo-llvm-cov for coverage, nextest for testing

## Important Notes

- This is an **educational codebase** - prioritize clarity and documentation when making changes
- The codebase uses **conventional commits** for automatic changelog generation
- Pre-commit hooks enforce code quality before commits
- Dependency changes are checked for security and license compliance via cargo-deny
