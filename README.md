# bex
A rust library for working with boolean expressions
(expression trees, decision diagrams, etc.)

This crate lets you build a complicated abstract syntax tree (or logic
circuit schematic, if you prefer) by working with individual Bit
structs, or vectors that act like integers. You can also solve these
AST structures by converting them into reduced, ordered, binary
decision diagrams (ROBDDs) - a normal form consisting of if-then-else
triples that essentially act like compressed truth tables. You can
also construct and manipulate BDDs directly.

## changelog

### 0.1.4 (upcoming)

** BDDBase::nidsols() **
- You can now call nidsols() on a BDDBase to iterate through solutions of the BDD.
  Each solution is presented as a Vec<NID> of length nvars(). The nid::var() of each
  NID tells you which input variable it describes, and the nid::inv() is 1 if the
  input should be LO. (I'll probably add a more ergonomic representation in the future.)

**ANFBase**
- The new `anf` module contains the beginnings of a BDD-like structure for working
  with expressions in algebraic normal form (XOR-of-ANDs). These two operations
  plus the constant `I` give a complete functional base. The implementation does
  not yet take advantage of multiple cores.

**code cleanup**
- Swarming is now enabled by default for BDDBase. (#7)
  For now, you can disable with `--feature noswarm`, but that's probably going to go away.
- `cargo test` now runs quickly, without generating diagrams (#3)
- Unify the AST and BDD "Base" interfaces. (#2, #5)
  - `base` now contains only the abstract trait `Base` (formerly `TBase`)
  - `Base` methods now act on associated types `Self::N` and `Self::V`,
    rather than `NID` and `VID` directly.
  - The old `struct base::Base` is now `ast::ASTBase`. Methods `sid`, `sub`,
    and `when` (which might not apply to other implementations) have been
    moved out of `trait Base` and into `struct ASTBase` directly.
  - The old `base::{Op,SID,SUB,NID,VID}` types have also moved to the `ast` module.
  - `bdd::BddBase` now implements `base::Base`.
  - Some of the tests for `ast` and `bdd` have been macro-fied and moved into `base`.
    These macros allow re-using the same test code for each `Base` implementation.
- The `bdd::NID` type and associated helper functions have been moved into `nid`
  so the same scheme can be reused for other `Base` implementations.

**documentation**
- Began writing/collecting more documentation in the
  [doc/](https://github.com/tangentstorm/bex/tree/master/doc) directory.

### 0.1.3 (2019-09-24)

I got most this working back in December and then put it all aside for a while.
It's still pretty messy, but I'm starting to work on it again, so I figured I
would ship what I have, and then aim for more frequent, small releases as I
continue to tinker with it.

**multi-threaded workers**
- refactored `bdd` so that the `BddState` is now owned by a `BddWorker`.
  Further, both `BddState` and `BddWorker` are now traits.
- Moved `BddWorker` implementation into `SimpleBddWorker`.
- Provided multiple implementations for `BddState` -- (so far,
  one with and one without array bounds checking).
- Added a multi-core bdd worker: `BddSwarm`. Between threading and an
  out-of-order execution model that results in potential short circuiting,
  `ite()` calls that once took 30 or more seconds on my low-end 2-core
  laptop now run in 0 seconds!

**code tuning**
- added `solve::sort_by_cost` which optimizes the ast→bdd conversion
  to take only one `bdd_refine_one` step per AST node
  (improved my still-external benchmark script by an order of magnitude).
- in `bdd`, `ite_norm` now constructs hi/lo nodes directly from
  input rather than calling `when_xx`. This resulted in about a 23% speedup.

**(rudimentary) example programs**
- `examples/bdd-solve.rs` demonstrates one method of using bex
  to solve arbitrary problems. (Albeit very very slowly, still...)
- `examples/bex-shell.rs` is a tiny forth-like interpreter for
  manipulating expressions interactively.
- See [examples/README.md](https://github.com/tangentstorm/bex/tree/master/examples)
  for more details.

**other improvements**
- `solve::ProgressReport` can now simply save the final result instead
  of showing it (as `dot` can take a very long time to render it into a png).
  It also now shows progress as a percentage (though only currently accurate
  when `sort_by_cost` was called)

### 0.1.2 (2018-12-17)

- added `Cargo.toml` documentation link to [docs.rs/bex](https://docs.rs/bex/)
- added this changelog

### 0.1.1 (2018-12-17)

- Renamed `bex::x32` to `bex::int`, used macros to generalize number of bits,
  added `times`, `lt`, and `eq` functions
- Added `bex::solve` for converting between ast and bdd representations.
- Added distinction between `real` (input) and `virtual` (intermediate)
  variables in `bdd::NID`
- Added graphviz (`*.dot`) output for `base::Base` and improved formatting
  for `bdd::BDDBase`
- Various performance enhancements for `bex::bdd`. Most notably:
  - switched caches to use the `hashbrown` crate (for about a 40% speedup!)
  - added inlining hints for many functions
  - re-ordered logic in bottleneck functions (`norm`, `ite_norm`) to minimize work
  - `bdd::NID` is now a single u64 with redundant information packed into the NID itself.
    This way, decisions can be made looking at the NID directly, without fetching the
	actual node.
  - Disabled bounds checking for internal node lookups. (unsafe)
- Refactored `bex::bdd` in preparation for multi-threading.
  - Grouped the internal node lists and the caches by branching variable (VID).
    This isn't actually an optimization, but I expect(ed?) it to make concurrent
	solving easier in the future.
  - moved all the unsafe, data-mutating operations into a handful of
    isolated functions on a single source page. These will likely be
    factored out into a new `Worker` struct, eventually.

### 0.1.0 (2018-11-30)

Initial public version. Work-in-progress code imported from a private repo.
