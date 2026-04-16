# Causal profiling with coz

Bex is instrumented for use with the [coz][coz] causal profiler. Coz is
*not* a sampling profiler — instead of telling you where a program spends
time, it tells you which lines would actually speed up the program if
optimized. For parallel code (which bex has a lot of: `BddSwarm`,
`VhlSwarm`, `AnfSwarm`, the parallel regroup solver) those are frequently
different answers.

See issue [#9][issue9] for background on why we want this.

## Build

```sh
cargo build --release --features coz --example bdd-solve
```

The `coz` feature is off by default; with it disabled all of the progress
points defined in `src/coz_profile.rs` expand to no-ops, so normal builds
pay nothing.

## Profile

Profile a workload by prefixing it with `coz run`:

```sh
coz run --- target/release/examples/bdd-solve
```

This produces a `profile.coz` file in the working directory. View it in
the [coz web viewer][viewer].

## Constraints

- **Linux only** — coz uses `perf_event_open`.
- Needs **debug symbols** in release builds. The existing
  `debug = "line-tables-only"` in `Cargo.toml` is already enough.
- Expect a **2-5× slowdown** while profiling, so this is a "when you
  have a perf question" tool, not continuous.
- Only meaningful on **realistic workloads**. Toy inputs won't expose
  contention patterns.

## Progress points

These are the named progress points currently emitted:

| Name        | Where                                           | Meaning                                                    |
| ----------- | ----------------------------------------------- | ---------------------------------------------------------- |
| `bdd-ite`   | `src/bdd/bdd_swarm.rs` `BddJobHandler::work_job`| One ITE job processed by a BDD swarm worker.               |
| `anf-job`   | `src/anf_swarm.rs` `AnfWorker::work_job`        | One ANF job (xor / and / sub) processed by a swarm worker. |
| `row-swap`  | `src/swap.rs` `swap()` and `swarm_put_rd()`     | One row-swap committed (serial or parallel regroup).       |

## Adding more progress points

Use one of the macros in `src/coz_profile.rs`:

```rust
crate::coz_progress!("some-name");       // named progress point
crate::coz_begin!("span");               // throughput span start
crate::coz_end!("span");                 // throughput span end
crate::coz_scope!("span");               // RAII scope guard
```

When the `coz` feature is disabled, all four expand to `()`.

[coz]: https://github.com/plasma-umass/coz
[issue9]: https://github.com/tangentstorm/bex/issues/9
[viewer]: https://plasma-umass.org/coz/
