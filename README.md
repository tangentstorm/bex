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


## Changes in 0.1.6 (2023-03-27)

Aside from the addition of the `ops` module, this is primarily
a benchmark release to make it easier to compare the 0.1.5
algorithms with 0.2.0.

- Rename `BDDBase` to `BddBase`, and add `reset()` method.

- Add `BddBase::reset(&mut self)` to clear bdd state.

- Cleaned up all compiler warnings.

- Removed all debug output.

- Fixed test failures that appeared with different threading configurations.

- Remove `nvars` from all `Base` implementations. This member was
  only really useful when the height of a node wasn't obvious from
  the variable index. Because of this,  `Base::new()` no longer takes
  a parameter.

- Remove obsolete "substitution" concept from `ast.rs`, and replace
  `ast::Op` with the more flexible `ops::Ops`.



For full changelog, see [CHANGELOG.md](https://github.com/tangentstorm/bex/blob/main/CHANGELOG.md).
