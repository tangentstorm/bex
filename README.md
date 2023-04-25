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


## Changes in main branch (upcoming version 0.3.0)

- Added new `Fun` trait and `NidFun` struct, refining the idea of storing truth tables of up to 5 inputs in a NID.
- Added `ASTBase::{apply,eval}`


## Changes in 0.2.0

The main change here is that `BddBase` is now 100 times faster, or more, depending on your CPU count.

The `BddSwarm` structure has been heavily refactored, making use of the `swarm` module, and also
extracting `wip::WorkState` for tracking dependencies between concurrent work-in-progress operations.

For full changelog, see [CHANGELOG.md](https://github.com/tangentstorm/bex/blob/main/CHANGELOG.md).
