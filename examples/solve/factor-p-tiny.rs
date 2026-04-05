//! Smaller primorial factoring benchmark using X4/X8 widths.
//! This is meant for quicker iteration than factor-p.

/// Product of the first 2 primes: 2 3 (3 bits, fits in X8)
const P2 : usize = 6;
fn p2_factors()->Vec<(u64,u64)> {
  vec![(1,6), (2,3)]}

/// Product of the first 3 primes: 2 3 5 (5 bits, fits in X8)
const P3 : usize = 30;
fn p3_factors()->Vec<(u64,u64)> {
  vec![(1,30), (2,15), (3,10), (5,6)]}

/// Product of the first 4 primes: 2 3 5 7 (8 bits, fits in X8)
const P4 : usize = 210;
fn p4_factors()->Vec<(u64,u64)> {
  vec![(1,210), (2,105), ( 3,70), ( 5,42),
       (6, 35), (7, 30), (10,21), (14,15)]}

extern crate bex;
use bex::{Base, solve::find_factors, anf::ANFBase, bdd::BddBase, int::{X4,X8}, swap::SwapSolver};
use bex::anf_swarm::AnfSwarmBase;

include!(concat!(env!("OUT_DIR"), "/bex-build-info.rs"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolverKind {
  Bdd,
  Swap,
  Anf,
  AnfSwarm,
}

impl SolverKind {
  fn label(self)->&'static str {
    match self {
      Self::Bdd => "sub solver",
      Self::Swap => "swap solver",
      Self::Anf => "anf solver",
      Self::AnfSwarm => "anf swarm solver",
    }
  }

  fn ignores_threads(self)->bool {
    matches!(self, Self::Swap | Self::Anf)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Config {
  solver: SolverKind,
  which: usize,
  num_threads: usize,
}

fn parse_args<I, S>(args:I)->Config
where
  I: IntoIterator<Item=S>,
  S: AsRef<str>,
{
  let mut use_swap = false;
  let mut use_anf = false;
  let mut use_anf_swarm = false;
  let mut get_which = false; let mut which = 4;
  let mut get_threads = false; let mut num_threads = 0;
  for a in args {
    let a = a.as_ref();
    if get_threads { num_threads = a.parse().expect("bad -t parameter"); get_threads=false; }
    else if get_which { which = a.parse().expect("bad -p parameter"); get_which=false; }
    else { match a {
      "-t" => get_threads = true,
      "-p" => get_which = true,
      "swap" => use_swap = true,
      "anf" => use_anf = true,
      "anf-swarm" => use_anf_swarm = true,
      _ => { /* ignore for now */} }}}

  let mode_count = [use_swap, use_anf, use_anf_swarm].into_iter().filter(|x| *x).count();
  if mode_count > 1 { panic!("choose only one of 'swap', 'anf', or 'anf-swarm'"); }
  Config {
    solver: if use_swap { SolverKind::Swap }
            else if use_anf { SolverKind::Anf }
            else if use_anf_swarm { SolverKind::AnfSwarm }
            else { SolverKind::Bdd },
    which,
    num_threads,
  }
}

pub fn main() {
  let Config { solver, which, num_threads } = parse_args(std::env::args());

  let (k, expected) = match which {
    2 => (P2, p2_factors()),
    3 => (P3, p3_factors()),
    4 => (P4, p4_factors()),
    _ => { panic!("the available primorials are: 2,3,4") }};

  println!("[bex {BEX_VERSION} -O{BEX_OPT_LEVEL}] factor-p-tiny -t {num_threads} -p {which} ({})",
    solver.label());

  if solver.ignores_threads() && num_threads != 0 {
    println!("note: {} ignores -t parameter", solver.label());
  }
  match solver {
    SolverKind::Swap =>
      find_factors::<X4, X8, SwapSolver>(&mut SwapSolver::new(), k, expected),
    SolverKind::Anf =>
      find_factors::<X4, X8, ANFBase>(&mut ANFBase::new(), k, expected),
    SolverKind::AnfSwarm =>
      find_factors::<X4, X8, AnfSwarmBase>(&mut AnfSwarmBase::new_with_threads(num_threads), k, expected),
    SolverKind::Bdd =>
      find_factors::<X4, X8, BddBase>(&mut BddBase::new_with_threads(num_threads), k, expected),
  }}

#[cfg(test)]
mod tests {
  use super::{Config, SolverKind, parse_args};

  #[test]
  fn parse_defaults_to_bdd() {
    assert_eq!(parse_args(["factor-p-tiny"]),
      Config { solver: SolverKind::Bdd, which: 4, num_threads: 0 });
  }

  #[test]
  fn parse_anf_mode() {
    assert_eq!(parse_args(["factor-p-tiny", "anf", "-p", "3"]),
      Config { solver: SolverKind::Anf, which: 3, num_threads: 0 });
  }

  #[test]
  fn parse_swap_mode_with_threads() {
    assert_eq!(parse_args(["factor-p-tiny", "swap", "-t", "2"]),
      Config { solver: SolverKind::Swap, which: 4, num_threads: 2 });
  }

  #[test]
  #[should_panic(expected = "choose only one")]
  fn parse_rejects_multiple_solver_flags() {
    parse_args(["factor-p-tiny", "swap", "anf"]);
  }
}
