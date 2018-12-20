# bex
A rust library for working with boolean expressions (expression trees, decision diagrams, etc.)

This crate lets you build a complicated abstract syntax tree (or logic circuit schematic, if you prefer) by working with individual Bit structs, or vectors that act like integers. You can also solve these AST structures by converting them into reduced, ordered, binary decision diagrams (ROBDDs) - a normal form consisting of if-then-else triples that essentially act like compressed truth tables. You can also construct and manipulate BDDs directly.


## changelog

### 0.1.3 (in progress)

- `solve::ProgressReport` can now simply save the final result instead of showing it (as dot can take a very long time to render it into a png).
- added `solve::sort_by_cost` which optimizes the ast→bdd conversion to take only one `bdd_refine_one` step per AST node (improved my still-external benchmark script by an order of magnitude).


### 0.1.2 (2018-12-17)

- added Cargo.toml documentation link to [docs.rs/bex](https://docs.rs/bex/)
- added this changelog

### 0.1.1 (2018-12-17)

- Renamed `bex::x32` to `bex::int`, used macros to generalize number of bits, added `times`, `lt`, and `eq` functions
- Added `bex::solve` for converting between ast and bdd representations.
- Added distinction between `real` (input) and `virtual` (intermediate) variables in `bdd::NID`
- Added graphviz (`*.dot`) output for `base::Base` and improved formatting for `bdd::BDDBase`
- Various performance enhancements for `bex::bdd`. Most notably:
  - switched caches to use the `hashbrown` crate (for about a 40% speedup!)
  - added inlining hints for many functions
  - re-ordered logic in bottleneck functions (`norm`, `ite_norm`) to minimize work
  - `bdd::NID` is now a single u64 with redundant information packed into the NID itself. This way, decisions can be made looking at the NID directly, without fetching the actual node.
  - Disabled bounds checking for internal node lookups. (unsafe)
- Refactored `bex::bdd` in preparation for multi-threading.
  - Grouped the internal node lists and the caches by branching variable (VID). This isn't actually an optimization, but I expect(ed?) it to make concurrent solving easier in the future.
  - moved all the unsafe, data-mutating operations into a handful of isolated functions on a single source page. These will likely be factored out into a new `Worker` struct, eventually.

### 0.1.0 (2018-11-30)

Initial public version. Work-in-progress code imported from a private repo.
