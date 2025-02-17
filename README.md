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

It covers the large factoring problems in [examples/solve/bdd-solve.rs](https://github.com/tangentstorm/bex/blob/main/examples/solve/bdd-solve.rs)
and the smaller tests in [src/solve.rs](https://github.com/tangentstorm/bex/blob/main/src/solve.rs)


## Changes in main branch (upcoming version)

- TBD

## What's new in 0.3.0

- Greatly expanded and fleshed out the python integration, including support for [@tulip-control/dd](https://github.com/tulip-control/dd)
- Added a variety of new functions to `BddBase`:
  - `reorder` for arbitrary reorderings
  - `reorder_by_force` for the FORCE algorith, a fast (but not always as effective) alternative to variable sifting
  - `to_json` and `from_json` to serialize and restore a set of nids
- Added a simple [HTTP API](https://github.com/tangentstorm/bex/tree/main/api) for integrating with other languages.
- Added new `Fun` trait and `NidFun` struct, refining the idea of storing truth tables of up to 5 inputs in a NID.
- Added `ASTBase::{apply,eval}`
- `naf.rs` (a variation of ANF)
- VhlSwarm (extracted a generic VHL swarm framework from BddSwarm, to re-use on other VHL-based mods)
- Began standardizing the formatting/parsing of NIDs (`FromStr` and `fmt::Display` should now round-trip)
- Many other small fixes and cleanups.

For full changelog, see [CHANGELOG.md](https://github.com/tangentstorm/bex/blob/main/CHANGELOG.md).
