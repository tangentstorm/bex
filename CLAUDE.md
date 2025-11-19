# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Bex is a Rust crate for working with binary (Boolean) expressions. It provides tools to build abstract syntax trees (ASTs) and solve them by converting into canonical representations like reduced, ordered, binary decision diagrams (ROBDDs) and algebraic normal form (ANF).

## Common Commands

### Building and Testing
```bash
# Build the library and all binaries
cargo build

# Build with optimizations (important for performance testing)
cargo build --release

# Run all tests
cargo test

# Run tests including slow tests
cargo test --features slowtests

# Run a specific test
cargo test <test_name>

# Run benchmarks
cargo bench
```

### Running Binaries
The project includes several executable binaries:

```bash
# Interactive BDD shell (default binary)
cargo run

# Swarm shell (parallel computing framework)
cargo run --bin swarm-shell

# BDD solver example (factoring problems)
cargo run --bin bdd-solve

# Factor primorial example
cargo run --bin factor-p

# Run with release optimizations for better performance
cargo run --release --bin bdd-solve
```

### Documentation
```bash
# Generate and open documentation
cargo doc --open
```

## Architecture

### Core Data Types

**NID (Node ID)**: The fundamental identifier for nodes in the system. Packed into a u64 for performance:
- Contains a VID (variable ID) and index
- Has special bits for: inversion (INV), variable (VAR), constant (T), real variable (RVAR), function (F)
- Constants: `O` (always false), `I` (always true)
- See `src/nid.rs`

**VID (Variable ID)**: Represents variables in the BDD structure with ordering:
- Four types: `T` (top/constant), `NoV` (no variable), `Var(u32)` (real variables), `Vir(u32)` (virtual variables)
- Ordering determines BDD structure efficiency
- See `src/vid.rs`

### Base Trait

The `Base` trait (`src/base.rs`) defines the common interface for all expression databases:
- Logical operations: `and`, `xor`, `or`, `ite` (if-then-else)
- Variable operations: `when_hi`, `when_lo` (cofactors)
- Node management: `def`, `tag`, `get` (naming/lookup)
- Evaluation: `eval`, `eval_all` (substitution)
- Visualization: `dot`, `show` (GraphViz integration)

### Main Implementations

**BddBase** (`src/bdd.rs`): The primary BDD implementation
- Uses ITE (if-then-else) triples normalized according to Bryant's algorithm
- Contains a `BddSwarm` for parallel operations
- Implements the `Base` trait for Boolean operations
- Supports variable reordering (sifting, FORCE algorithm)

**RawASTBase** (`src/ast.rs`): Abstract syntax tree representation
- Stores operations as `Ops` structs
- Simpler than BDD, useful for building expressions before solving
- Uses expression cache for deduplication

### Parallel Computing Framework

**Swarm** (`src/swarm.rs`): Generic multicore programming framework
- Worker threads communicate via channels
- Query-response pattern with `QID` (query ID) and `WID` (worker ID)
- Used by `BddSwarm` and `VhlSwarm`
- Workers have lifecycle: `work_init` → `work_step` (loop) → `work_done`

### Key Modules

- `src/solve.rs`: Solving/factoring logic using BDDs
- `src/swap.rs`: Swap-based BDD solver with validation
- `src/anf.rs`: Algebraic Normal Form (XOR of ANDs)
- `src/naf.rs`: Negation Algebraic Normal Form variant
- `src/int.rs`: Integer/vector operations on bits
- `src/reg.rs`: Register type for representing bit vectors
- `src/fun.rs`: Function trait for truth tables
- `src/ops.rs`: Operations on expressions
- `src/simp.rs`: Simplification rules
- `src/io.rs`: Input/output utilities

## Workspace Structure

The repository is a Cargo workspace with three members:

1. **py/** - Python bindings using PyO3
   - Compatible with `dd` package interface
   - Install: `pip install tangentstorm-bex`

2. **api/** - HTTP REST API server
   - Provides web interface to BDD operations
   - Default: `http://127.0.0.1:3030`
   - Endpoints: `/ite`, `/and`, `/xor`, `/or`, `/nid`

3. **ffi/** - C FFI bindings
   - Exposes solvers and utilities to C
   - Uses cbindgen for header generation

## Testing

Tests are located in:
- Inline `#[cfg(test)]` modules within source files
- Separate test files: `src/test-bdd.rs`, `src/test-swap.rs`, `src/test-swap-scaffold.rs`
- Example programs in `examples/` that demonstrate solving problems

The `slowtests` feature flag enables computationally intensive tests.

## Performance Notes

- The `release` profile uses aggressive optimizations (LTO, single codegen unit)
- Debug info uses `line-tables-only` for profiling release builds
- Recent commits show focus on parallel processing and solver performance
- Use `--release` for meaningful performance measurements

## Visualization

GraphViz integration allows visualizing BDDs:
- Uses `dot` command to generate SVG files
- `show()` method renders and opens in Firefox
- HTML viewer available at `viewbex.html`
