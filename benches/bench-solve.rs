//! Benchmarks for the BDD factoring solver.
//!
//! Uses [`divan`](https://crates.io/crates/divan) for measurement. Each bench
//! reports wall-clock statistics (median / mean / stddev) by default, and —
//! via the `AllocProfiler` global allocator — bytes and count of heap
//! allocations per iteration. The latter is particularly useful for catching
//! silent allocation regressions in a BDD crate.
//!
//! Run with:
//! ```text
//! cargo bench
//! cargo bench -- tiny          # filter by name
//! cargo bench -- --sample-count 30
//! ```

use bex::{BddBase, ast::ASTBase, int::GBASE, solve::find_factors};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() { divan::main(); }

/// Factor 210 into two 4×8-bit operands (one expected solution).
#[divan::bench]
fn tiny(bencher: divan::Bencher) {
  use bex::int::{X4, X8};
  bencher.bench_local(|| {
    find_factors::<X4, X8, BddBase>(&mut BddBase::new(), 210, vec![(14, 15)]);
    GBASE.with(|gb| gb.replace(ASTBase::empty()));
  });
}

/// Factor 210 into two 8×16-bit operands (full set of solutions).
#[divan::bench(sample_count = 10)]
fn small(bencher: divan::Bencher) {
  use bex::int::{X8, X16};
  bencher.bench_local(|| {
    let expected = vec![(1, 210), (2, 105), (3, 70), (5, 42),
                        (6,  35), (7,  30), (10, 21), (14, 15)];
    find_factors::<X8, X16, BddBase>(&mut BddBase::new(), 210, expected);
    GBASE.with(|gb| gb.replace(ASTBase::empty()));
  });
}
