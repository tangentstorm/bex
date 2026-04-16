# bex changelog

Bex is a rust crate for working with binary expressions.

## 0.4.0 (upcoming)

- **ZDD (Zero-suppressed Decision Diagram) module.** New `src/zdd.rs` adds
  `ZddBase`, implementing both a family-of-sets API and the `Base` trait
  for Boolean-function compatibility with the swap solver and other tooling.
  - Family-of-sets ops: `union`, `intersect`, `diff`, `product`, `quotient`,
    `remainder` (Minato's unate cube set algebra), plus `change`, `onset`,
    `offset`, `subset0`, `subset1`, `count`, `complement`, `power_set`.
  - `Base` trait: `and`/`or`/`xor`/`ite` mapped to family ops with lazy
    universe tracking for complement semantics.
  - `ZddSetIterator` for native family enumeration; `ZddSolIterator` for
    Boolean solution enumeration via BDD conversion.
  - Graphviz rendering via `dot`.
- **Direct single-threaded ITE path for `BddBase`.** Opt in with
  `BddBase::set_direct_ite(true)` (or `bex_bdd_set_direct_ite(bdd, true)` from C) to
  bypass swarm dispatch and recurse directly with a local `FxHashMap` computed table.
  Designed for workloads that build BDDs bottom-up via many small sequential ITE
  calls, where channel-dispatch overhead dominates. Default is unchanged (swarm).
  - Reduced the `bdd-benchmark` queens N=8 runtime from "hangs" to ~580 ms in the
    bdd-benchmark adapter; unblocks tic-tac-toe, hamiltonian, and game-of-life too.
- New FFI `bex_swap_copy_to_bdd(swap, bdd, n)` so C callers can transfer a
  swap-solver result into a separate `BddBase` and use the normal
  `bex_bdd_node_count` / `bex_bdd_solution_count` on it.
- `VhlSwarm` now exposes `get_done` / `put_done` / `vhl_to_nid` so callers can
  check the shared computed table or construct nodes without dispatching a job.

### Bug fixes
- `tbl::merge_small` wrote past the end of a 5-element array before returning
  `None` on 6-variable ITEs, causing panics in the truth-table fast path. The
  `len > 5` guards are now `len >= 5`.
- `tbl::nid_to_small` accepted real-variable NIDs with an index >= `MAX_VAR`
  and fed them into the combinatorial encoder, which panicked with an array
  bounds error. Now it returns `None` so the caller falls back to the regular
  BDD path. This unblocked the `bdd-benchmark` hamiltonian workload, which
  allocates more than 110 variables.

- **Table NIDs with named variables.** Functions of up to 5 input variables are now stored
  directly in the NID as a truth table, with the variable set encoded using a combinatorial
  number system (combinadic). This avoids allocating BDD nodes for small subexpressions.
  - New `NID::fun_with_vars(&[u32], tbl)` constructor for explicit variable sets.
  - `NidFun` gains `vars()`, `top_vid()`, `contains_var()`, `var_position()`.
  - New `src/comb.rs` module: combinadic encode/decode for variable subsets of up to 110 variables.
  - New `src/tbl.rs` module: truth table alignment, expansion, and bitwise operations
    (`table_and`, `table_xor`, `table_or`, `table_ite`) with zero-allocation fast path.
  - `BddBase::ite()` now automatically resolves small operations via truth tables before
    entering the BDD swarm (zero overhead on large problems, avoids node allocation for small ones).
  - `VhlBase::tup()` decomposes table NIDs into hi/lo branches transparently.
  - Display format `T{x3,x7:1110}` for table NIDs with non-default variable sets;
    `FromStr` round-trips the new format.

Older upcoming changes for 0.4.0 live in the README section titled \"Changes in main branch (upcoming version)\".

### Performance
- ~22% speedup on BDD factoring benchmark (`small`: factor 210 into 8x16-bit integers)
  - Replace `HiLoCache` `Mutex` with `RwLock` + combined `get_or_insert` to reduce lock contention
  - Increase `DashMap` shard count from 16 to 128 for better concurrent access
  - Pre-size `HiLoCache` HashMap and `DashMap` to 256K entries, eliminating rehash cascades
    (Massif profiling showed 90% of heap allocations went to hash table resizing)
  - Added `bench-small` example for quick single-run benchmarking
  - Added [doc/optimization-ideas.md](doc/optimization-ideas.md) with 24 profiling-driven ideas (3 applied, 12 tested/rejected with rationale)

### Persistence
- **Snapshot persistence for `VhlScaffold`, `BddBase`, and `ANFBase`** (bex#6).
  Resolves the long-standing request to load and save intermediate
  solver state. A new `sql_snap` module adds four SQLite tables
  (`snapshot`, `snapshot_vid`, `snapshot_node`, `snapshot_root`)
  alongside the existing AST schema; each snapshot captures the
  graph plus its variable permutation (critical for `SwapSolver`,
  whose scaffold reordering changes at every `subst` step).
  Snapshots chain via `parent_id` to form a replay trace.
  - New public API: `sql_snap::{write_scaffold, write_bdd, write_anf,
    read_scaffold, read_bdd_into, read_anf_into, list_snapshots}`
    plus path-based wrappers and `ensure_snap_schema`.
  - `VhlScaffold`: new `iter_nodes`, `from_raw`, `is_mid_regroup`.
  - `SwapSolver`: new `dx`, `rv`, `from_parts` for resume.
  - `ANFBase`: new `nodes`, `tags`, `tags_mut`, `insert_vhl`.
  - `sql`: new `ensure_schema_pub`, `ensure_schema_tx` helpers so
    callers can share a transaction with snapshot writes.
  - Schema is additive — existing AST-only `.sdb` files load
    unchanged; `list_snapshots` returns empty for files without
    snapshot tables.

### Tooling
- **New binary `bex-sdb`** — CLI for inspecting snapshot databases.
  Subcommands: `list`, `info`, `dump`, `ast`, `replay`.
- **New binary `bex-mkproblem`** — generates AST `.sdb` files for
  primorial factoring problems (`bex-mkproblem -p 4 -o primorial-4.sdb`).
- **New binary `bex-solve`** — drives `SwapSolver` / `BddBase` /
  `ANFBase` through the substitution solve loop, auto-committing a
  snapshot to the same `.sdb` file after every N steps. Supports
  `--solver swap|bdd|anf`, `--save-every N`, `--resume <snap-id>`,
  `--timeout <secs>`, and `-o <output.sdb>`.
- `solve::refine_one` is now `pub` so external drivers can call it.

- Migrated `benches/bench-solve.rs` from [`bencher`](https://crates.io/crates/bencher)
  to [`divan`](https://crates.io/crates/divan) (bex#4). Benchmark output now
  includes median / mean / stddev on a tree-structured terminal report, plus
  per-iteration allocation byte and count statistics (via
  `divan::AllocProfiler`) — useful for catching silent allocation regressions
  in `ite` / `and` / `xor` on `BddBase`. Filtering and sample-count control
  work via the standard `cargo bench -- <filter> --sample-count N` CLI.
  - Factoring benches are defined via a `factor_benches!` macro table: one
    row per size (`tiny`, `small`, ...), each with its own `sample_count`.
    Adding a new size is a one-line addition.
  - New `ops::{and_chain, xor_chain, ite_chain}` alloc benches sweep
    N ∈ {8, 16, 32} variables and report the heap-allocation cost of
    reducing a chain of inputs via each primitive. `BddBase` setup and
    teardown are excluded from the measurement (via `with_inputs` +
    `bench_refs`), so the reported count is the marginal per-op cost.


## 0.3.0 (2025-02-16)

- Greatly expanded and fleshed out the python integration, including support for [@tulip-control/dd](https://github.com/tulip-control/dd)
- Added a variety of new functions to `BddBase`:
  - `reorder` for arbitrary reorderings
  - `reorder_by_force` for the FORCE algorith, a fast (but not always as effective) alternative to variable sifting
  - `to_json` and `from_json` to serialize and restore a set of nids
- Added a simple [HTTP API](api) for integrating with other languages.
- Added new `Fun` trait and `NidFun` struct, refining the idea of storing truth tables of up to 5 inputs in a NID.
- Added `ASTBase::{apply,eval}`
- `naf.rs` (a variation of ANF)
- VhlSwarm (extracted a generic VHL swarm framework from BddSwarm, to re-use on other VHL-based mods)
- Began standardizing the formatting/parsing of NIDs (`FromStr` and `fmt::Display` should now round-trip)
- Many other small fixes and cleanups.

## 0.2.0 (2023-04-22)

`BddBase` is now 100 times faster (or more, depending on your CPU count!)

- worker threads are no longer killed and respawned for each top-level query.

- Extract `wip:WorkState` from `bdd_swarm`, introducing a shared queue
  and concurrent hashmaps so workers can share work without supervision
  from the main thread.

- the workers now use concurrent queues and hashmaps (thanks to `boxcar`
  and `dashmap`) to share the cache state.

- Dropped `hashbrown` crate for non-shared hashmaps, since it is now the
  implementation that comes with rust standard library.

- Added `fxhash` as the hasher for all hashmaps.

- Removed top level functions in `nid::`. Use the corresponding `nid::NID::`
  methods instead. (ex: `nid::raw(n)` is now `n.raw()`) In particular,
  `nid::not(n)` should be written `!n`.

- `solve::find_factors` is now a generic function rather than a macro.

## 0.1.7 (2023-03-27)

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

## 0.1.6 (2023-03-27)

Same as 0.1.7 except I forgot to update the readme. :D

## 0.1.5 (2020-05-20)

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

## 0.1.4 (2020-06-13)

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

**vid::VID**
- `VID` is now an explicit custom type rather than a simple usize.
  It accounts for both "real" variables (`var()`) and virtual ones (`vir()`), as
  well as the meta-constant `T`  (true) which fills the branch variable
  slots for the `I` and `O` BDD nodes.

**smaller variables now appear at the bottom of BDD, ANF graphs.**
- A new type for `VID` comparison was introduced - `vid::VidOrdering`. This
  lets you `cmp_depth` using terms `Above`, `Level`, and `Below` rather than
  the `Less`, `Equal`, `Greater` you get with `cmp`. There was no technical
  reason for this, but I found it much easier to reason about the code in these terms.
- `VidOrdering` is set up so that branch variables with smaller
  numbers move to the *bottom* of a BDD. There are numerous benefits to doing
  this - cross-function cache hits, immediate knowledge of the width of a
  node's truth table, and (most importantly) a much simpler time converting
  from ANF to BDD.
- There is currently no support for the "industry standard" ordering - the
  plan in the future is just to make sure the graphviz output shows variable
  names, and then you can just re-arrange the labels.

**NID as universal ID**
- `ASTBase` now uses `nid::VID` for input variable identifiers, and `nid::NID`
  for node identifiers, rather than using simple `usize` indices. This means
  we no longer need to store explicit entries for constants and literals.
- The `Base` trait no longer takes type arguments `N` and `V`, since all
  implementations now use `nid::NID` and `vid::VID`.

**Reg type**
- `Reg` provides an general purpose register containing an arbitrary number of bits.
- Bits in a Reg can be accessed individually either by number (with `get(ix)`
  and `put(ix,bool))`, or using a `VID` (`var_get`, `var_put`). Indexing by virtual
  variables is not supported.
- `Reg` also provides a simple `increment()` method, as well as the more general
  `ripple(start,end)`. These treat the register as a binary number, "add 1" at a
  specified location, and ripple-carry the result until a 0 is encountered, or
  the carry overflows the end position. This is all intended to support `Cursor`.

**Cursor**
- `cur::Cursor` combines a `Reg` with a stack of `NID`s to provide a tool for
  navigating through the terms or solutions in a BDD/ANF-like graph structure.

**BDDBase**
- You can now call `solutions()` on a `BDDBase` to iterate through solutions of
  the BDD. Each solution is presented as a `Reg` of length `nvars()`.

**ANFBase**
- The new `anf` module contains the beginnings of a BDD-like structure for working
  with expressions in algebraic normal form (XOR-of-ANDs). These two operations
  plus the constant `I` give a complete functional base. The implementation does
  not yet take advantage of multiple cores.

**code cleanup**
- Swarming is now the only implementation for BDDBase. (#7)
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
  [doc/](doc/) directory.

## 0.1.3 (2019-09-24)

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
- See [examples/README.md](examples/README.md)
  for more details.

**other improvements**
- `solve::ProgressReport` can now simply save the final result instead
  of showing it (as `dot` can take a very long time to render it into a png).
  It also now shows progress as a percentage (though only currently accurate
  when `sort_by_cost` was called)

## 0.1.2 (2018-12-17)

- added `Cargo.toml` documentation link to [docs.rs/bex](https://docs.rs/bex/)
- added this changelog

## 0.1.1 (2018-12-17)

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

## 0.1.0 (2018-11-30)

Initial public version. Work-in-progress code imported from a private repo.
