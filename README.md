# bex : binary expression toolkit

Bex is a rust crate for working with binary (Boolean) expressions.

This crate lets you build a complicated abstract syntax tree (AST) by working with individual Bit structs, or vectors that act like integers.

You can also "solve" these AST structures by converting them into various canonical representations:

  - **reduced, ordered, binary decision diagrams (ROBDDs)**
   -- a normal form consisting of if-then-else triples that
   essentially act like compressed truth tables
  - **algebraic nomal form**
    -- an "xor of ands" polynomial form
  - (more coming in the future)

## Video introduction

[J and Bex vs Primorial 15](https://www.youtube.com/watch?v=gtEGiq04E4Q&list=PLMVwLeG3bKmniOWnZUM2mcYKphm0ggS-C)
is about converting "simple" factoring problems into
boolean expressions and solving them with bex.

It covers the large factoring problems in [examples/solve/bdd-solve.rs](examples/solve/bdd-solve.rs)
and the smaller tests in [src/solve.rs](src/solve.rs)


## Benchmarking

The central cross-solver metric is **Time to Cover (TTC)** — the projected
wall-clock time for a solver to fully classify every point in the `2^N`
search space of a problem. See [doc/time-to-cover.md](doc/time-to-cover.md)
for the definition, formula, and how it applies to both branching and
top-down (BDD/swap) solvers.

## Profiling

Bex has optional [coz][coz] causal-profiler integration behind the `coz`
cargo feature. See [doc/coz-profiling.md](doc/coz-profiling.md) for
instructions.

[coz]: https://github.com/plasma-umass/coz

## Changes in main branch (upcoming version)

- **Table NIDs with named variables:** truth tables of up to 5 inputs are now stored directly in the NID, with the variable set encoded via a combinadic index. `BddBase::ite()` automatically uses truth table operations for small subexpressions before entering the BDD swarm. New modules: `comb` (combinadic encoding), `tbl` (table alignment and operations). New display format: `T{x3,x7:1110}`.
- add C ffi for use with https://github.com/SSoelvsten/bdd-benchmark (bex adapter is at https://github.com/tangentforks/bdd-benchmark for now)
- add `ite` to the Base trait
- Removed `XID` type from `swap.rs`, as it is equivalent to `NID::ixn()` and required duplicating (or genericizing) `Hilo` and `Vhl` types.
- Added a new SQLite persistence module (`sql` feature, enabled by default) with an `ast_node`/`ast_edge` schema, `tag`/`keep` tables, an `edge_src_bits` view, and stored format-version metadata.
- Removed the (unused) `io` module.
- Expose AST/BDD operations, solvers, and `NID` utilities via the C FFI.
- Breaking: remove `ITE::new` (use the normal constructor instead).
- Swarm: when `num_workers=0`, leave one core free for the main thread.
- Swap: disable `validate()` checks except in test mode.
- FFI build/profile updates: move FFI release profile to the workspace root, enable optimizations, and allow configurable profiles.
- Profiling: set debug info to line-labels-only for profiling release builds.
- Notation: standardize and tighten NID string forms
  - Use `@` for indexed nodes; remove legacy `#` form.
  - Enforce uppercase hex for all hex segments (`xN`, `vN`, `.MMMM`, `@MMMM`, `fN.M`).
  - Allow binary tables `t` with lengths 2/4/8/16/32 bits (arity 1..5).
  - Restrict hex tables shorthand to a single digit: `fX` == `f2.X`; multi-digit `fXX` shorthand is rejected (full `fN.M` remains).
  - Fix parsing of virtual variables (`v…`).
- Shell: align with spec
  - Only `!` for negation (remove `~` and `not`).
  - Only `O` and `I` for constants (remove lowercase aliases).
- API: no response format change; endpoints still return plain text, but paths now accept the updated NID syntax (uppercase hex, `@…`, extended `t…`, `fX`).
- Performance: ~22% speedup on BDD factoring benchmark (`small`):
  - Replace `HiLoCache` `Mutex` with `RwLock` + combined `get_or_insert` method
  - Increase DashMap shard count from 16 to 128
  - Pre-size hash tables to reduce rehash cascades
  - See [doc/optimization-ideas.md](doc/optimization-ideas.md) for full profiling notes

## Release versions

See [CHANGELOG.md](CHANGELOG.md) for detailed notes on published releases, including [0.3.0](CHANGELOG.md#030-2025-02-16).
