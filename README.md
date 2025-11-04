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


## Changes in main branch (upcoming version)

- add C ffi for use with https://github.com/SSoelvsten/bdd-benchmark (bex adapter is at https://github.com/tangentforks/bdd-benchmark for now)
- add `ite` to the Base trait
- Removed `XID` type from `swap.rs`, as it is equivalent to `NID::ixn()` and required duplicating (or genericizing) `Hilo` and `Vhl` types.
- Added a new SQLite persistence module (`sql` feature, enabled by default) with an `ast_node`/`ast_edge` schema, `tag`/`keep` tables, an `edge_src_bits` view, and stored format-version metadata.
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

## Release versions

See [CHANGELOG.md](CHANGELOG.md) for detailed notes on published releases, including [0.3.0](CHANGELOG.md#030-2025-02-16).
