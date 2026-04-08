/// Quick single-run benchmark for the "small" factoring problem.
/// Usage: cargo run --release --example bench-small
use std::time::Instant;

use bex::{BddBase, solve::find_factors, int::GBASE};
use bex::int::{X8, X16};
use bex::ast::ASTBase;

fn main() {
  let threads: usize = std::env::var("BEX_THREADS").ok()
    .and_then(|s| s.parse().ok()).unwrap_or(0); // 0 = default (num_cpus - 1)
  let expected = vec![(1,210), (2,105), ( 3,70), ( 5,42),
                      (6, 35), (7, 30), (10,21), (14,15)];
  let t = Instant::now();
  let mut base = if threads > 0 { BddBase::new_with_threads(threads) } else { BddBase::new() };
  find_factors::<X8, X16, BddBase>(&mut base, 210, expected);
  let elapsed = t.elapsed();
  GBASE.with(|gb| gb.replace(ASTBase::empty()));
  eprintln!("small: {:.3}s (threads: {})", elapsed.as_secs_f64(),
    if threads == 0 { "default".to_string() } else { threads.to_string() });
}
