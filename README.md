# bex : an exercise in optimization

Bex is a rust library for working with boolean expressions
(expression trees, decision diagrams, etc.)

This crate lets you build a complicated abstract syntax tree (or logic circuit schematic, if you prefer) by working with individual Bit structs, or vectors that act like integers.

You can also "solve" these AST structures by converting them into various canonical represations:

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

## Current work (upcoming for version 0.2.0)

- actually use semantic versioning for releases :)
- better metrics collection for benchmarking
- generalize the BDD swarm implementation to work for ANF
- various solver optimizations
- better graphviz rendering
- other general improvements

For more detail, see [plans.org](https://github.com/tangentstorm/bex/blob/main/plans.org).


## Changes in 0.1.5 (latest release)

- Added `SwapSolver`, a new substitution solver that (like any `SubSolver`)
  works by iteratively replacing virtual variables (representing AST nodes)
  with their definitions inside a BDD. What's new here is that `SwapSolver`
  continuously re-orders the variables (rows) in the BDD at each step so
  that the substitution is as efficient as possible.

- Added `XVHLScaffold`, a data structure for decision-diagram-like graphs,
  that allows accessing each row individually. This structure should be
  considered extremely experimental, and may change in the future (as it
  does not currently use `NID` for node references).

- Added `swarm` module that contains a small framework for distributing work
  across threads. It is used by the `SwapSolver` to swap BDD rows in parallel,
  and follows the same design as `BddSwarm`, which will likely be ported over
  to this framework in the future.

- Added `ops` module for representing boolean expressions in something like
  reverse Polish notation. The `Ops::RPN` constructor will likely replace
  `ast::Op` as the representation of nodes in `ast::ASTBase` in a future
  version, since `Ops::RPN` can represent arbitrary boolean functions
  with any number of inputs.

For full changelog, see [CHANGELOG.md](https://github.com/tangentstorm/bex/blob/main/CHANGELOG.md).
