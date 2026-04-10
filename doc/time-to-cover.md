# Time to Cover (TTC)

## Motivation

When comparing search/solve strategies for boolean satisfaction problems, the
metric most people reach for first — **wall-clock time to a solution** — is
surprisingly bad. A solver that gets lucky and stumbles on an answer early
looks fast; the same solver on a different instance looks slow. The metric
measures luck as much as it measures the strategy.

The next obvious metric, **tries per second** (throughput), is an improvement
because it factors luck out. But it still doesn't answer the question you
actually want to ask when comparing strategies:

> *How long would this strategy take to fully decide the entire N-bit search
> space — whether it finds a solution or proves none exists?*

That is the **Time to Cover** metric. TTC is meant to be the central
benchmarking number across all solvers in the bex ecosystem.

## Definition

> **TTC** is the projected wall-clock time for a solver to reach a state where
> every point in `{0,1}^N` has been classified as *solution* or *non-solution*,
> given a problem with `N` free variables.

"Classified" is the key word. Both brute-force search and symbolic (BDD)
reasoning end up classifying every point, just by very different routes.

## The formula for branching search

For a brute-force / branching solver:

```
total_covered   = branching_tries + table_checks     // effective configs
throughput      = total_covered / elapsed_secs       // configs/sec
search_space    = 1 << N                             // 2^N configs
ttc_seconds     = search_space / throughput          // projected wall time
coverage_pct    = 100 * total_covered / search_space
```

`branching_tries` must weight each short-circuit by the size of the subtree it
eliminates. If the solver backtracks at depth `d` (with `k` variables still
unbound), that single skip covers `2^k` leaf configurations. A well-instrumented
branching solver already tracks this as effective coverage, not raw skip
count. `table_checks` adds `2^remaining_vars` for each subtree resolved via
truth-table evaluation.

### Worked example

8-bit search space, 256 configurations. A tactic prunes the space down to 64
live configurations. Each live config takes 10 seconds to check. Then:

```
total_covered   = 64 * 1 (live) + 192 (pruned as effective coverage) = 256
elapsed         = 64 * 10 = 640s
throughput      = 256 / 640 = 0.4 configs/sec
ttc             = 256 / 0.4 = 640s
```

TTC is 640 seconds — regardless of whether the solution turns up on try 3 or
try 63. That's the point.

## Applying TTC to top-down solvers (BDD, swap, ANF)

A top-down solver like `SwapSolver` or `BddBase` substitution doesn't enumerate
leaves at all. It builds a BDD that symbolically represents the answer
function over the entire `2^N` input space. When such a solver completes, it
has — by construction — classified every point in the space. So:

- **Completed run:** `ttc = elapsed`. The work wasn't pointwise, but the
  classification is. Throughput in `configs/sec` is implied but not meaningful
  on its own (the solver doesn't do "tries").
- **Timed out / OOM:** TTC is **undefined and known to be greater than the
  timeout**. Unlike a branching solver, a top-down solver can't be
  extrapolated from partial progress because its work isn't pointwise — a
  BDD that's 50% built doesn't cover 50% of the search space. Report
  `ttc > <timeout> (DNF)`.

The payoff is cross-strategy comparability. Whoever finishes classifying
`{0,1}^N` first wins, whether they did it by pruning a search tree or by
building a BDD. No more "my solver found the answer faster" / "but mine would
have caught more cases" apples-to-oranges arguments.

## Display conventions

Solvers and benchmark tools across the bex ecosystem should include a TTC
line in their output when they know `N` (the search space bit-width):

```
search space: 2^24 = 16777216
covered:      16777216 (100.00%)
time-to-cover: 112.7s
```

For a timed-out run:

```
search space: 2^32 = 4294967296
covered:      8500000 (0.20%)
time-to-cover: ≈ 28600s (extrapolated, 0.20% covered)
```

For a top-down solver that completed:

```
search space: 2^16 = 65536
time-to-cover: 3.1s (full BDD constructed)
```

For a top-down solver that timed out:

```
time-to-cover: > 30s (DNF — BDD construction did not complete)
```

## Reference implementations

- `woslbimi/src/bin/def-worker.rs` — the branching case. Uses
  `DefBase::search_parallel_timed` which already returns
  `(tries, table_checks, elapsed, outcome)`. TTC is derived from `db.arity()`
  and those values. A `--timeout` flag lets you cap runs so TTC extrapolation
  is possible for spaces the solver can't finish.
- `woslbimi/src/bin/benchmark.rs` — same, in benchmark-runner form.
- `bex/src/solve.rs::find_factors` — the top-down case. Knows the search
  space from `2 * T0::n()` bits and prints `ttc = elapsed` after the solver's
  own `print_stats`.

## See also

- `doc/optimization-ideas.md` — profiling and optimization notes that
  should be measured against TTC going forward.
- `woslbimi/docs/benchmark-datasets.md` — the `block-N` datasets
  (`block-12`, `block-16`, `block-24`, `block-32`) used as canonical TTC
  benchmarks for the def-worker.
