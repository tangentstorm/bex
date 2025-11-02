# Repository Guidelines

## Project Structure & Module Organization
The Rust crate lives in `src/`, with solver-focused modules such as `swap.rs`, `solve.rs`, and `bdd/` helpers. Example binaries sit in `examples/shell/` and `examples/solve/`. Performance experiments reside in `benches/bench-solve.rs`. Workspace members `api/`, `ffi/`, and `py/` provide the HTTP service, FFI bindings, and Python tools; keep their build outputs in `target/`. Background reading, including design notes, lives under `doc/` and `plans.org`.

## Build, Test, and Development Commands
- `cargo build` compiles the library and binaries for quick validation.
- `cargo run --release --bin bex-shell` launches the interactive shell with optimized settings.
- `cargo test` executes the in-module suites such as `src/test-swap.rs`.
- `cargo test --features slowtests` unlocks exhaustive solver and swarm scenarios.
- `cargo bench` runs `benches/bench-solve.rs` via `bencher` to track regressions.
- `cargo clippy --all-targets --all-features` surfaces lint issues before review.

## Coding Style & Naming Conventions
We intentionally do not auto-format with `rustfmt`; preserve the existing hand-tuned layout, two-space indentation, and column alignment used in solver matches. Functions and modules stay `snake_case`, types and traits use `CamelCase`, and constants remain `UPPER_SNAKE`. Group related helpers near their primary module (for example, scaffolding structs at the bottom of `swap.rs`). Prefer explicit imports and keep doc comments focused on solver intent.

## Testing Guidelines
Unit tests live beside implementations (`test-*.rs` files and `#[cfg(test)]` modules). Name tests after the behavior under scrutiny, e.g., `test_xsdebug` in `test-swap.rs`. Use `cargo test -- --nocapture` when debugging traces. Gate expensive solver sweeps behind the `slowtests` feature and mention any required runtime when requesting review. Record noteworthy benchmark deltas if `cargo bench` exposes regressions.

## Commit & Pull Request Guidelines
Follow the `area: imperative summary` subject style seen in history (`swap: disable validate() checks`). Keep subjects under 72 characters, add details in the body (link issues with `Refs #id`), and explain any solver performance trade-offs. Pull requests should outline affected modules, note API or CLI impacts, and include logs or screenshots when touching the HTTP or shell tooling.

## Tooling Notes
- Do not run `cargo fmt`; formatting is hand-tuned.
