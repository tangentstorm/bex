//! this program factors primorial 4 (210 = */2 3 5 7)
//! into two bytes x,y where that x<y.

/// Product of the first 4 primes: 2 3 5 7
const K:usize = 210;

fn factors()->Vec<(u64,u64)> {
  vec![(1,210), (2,105), ( 3,70), ( 5,42),
       (6, 35), (7, 30), (10,21), (14,15)]}

extern crate bex;
use bex::{solve::find_factors, bdd::BddBase, int::{X8,X16}, swap::SwapSolver};

include!(concat!(env!("OUT_DIR"), "/bex-build-info.rs"));

pub fn main() {
  let expected = factors();
  let mut use_swap = false;
  let mut get_threads = false; let mut num_threads = 0;
  for a in std::env::args() {
    if get_threads { num_threads = a.parse().expect("bad -t parameter"); get_threads=false; }
    else { match a.as_str() {
      "-t" => get_threads = true,
      "swap" => use_swap = true,
      _ => { /* ignore for now */} }}}
  println!("[bex {BEX_VERSION} -O{BEX_OPT_LEVEL}] factor-p4 -t {num_threads} ({})",
    if use_swap { "swap solver" } else { "sub solver" });
  if use_swap {
    if num_threads != 0 { println!("note: swap solver ignores -t parameter"); }
    find_factors::<X8, X16, SwapSolver>(&mut SwapSolver::new(), K, expected); }
  else { find_factors::<X8, X16, BddBase>(&mut BddBase::new_with_threads(num_threads), K, expected); }}
