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


## Changes in 0.1.4 (latest release)

This version introduces the ANFBase for working with
algebraic normal form using a BDD-like graph structure.
This version also introduces Cursors, which provide the
ability to iterate through BDD solutions and ANF terms.

It also includes a major refactoring effort: the BDD, AST,
and ANF bases now all use the same NID/VID types for node
and variable identifiers.

Finally, BDD and ANF graphs are now arranged so that
variables with the smallest identifiers now appear at
the bottom (so that subgraphs are more likely to be
shared across functions with different numbers of inputs,
and also so that the size of a node's truth table is
immediately apparent from its topmost variable.)

For full changelog, see [CHANGELOG.md](https://github.com/tangentstorm/bex/blob/main/CHANGELOG.md).
