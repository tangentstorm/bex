//! this benchmark programs splits various primorials
//! into two factors x,y where that x<y.
//! primorial n is the product of the first n primes.

/// Product of the first 4 primes: 2 3 5 7 (8 bits, but treat as 16-bit)
const P4 : usize = 210;
fn p4_factors()->Vec<(u64,u64)> {
  vec![(1,210), (2,105), ( 3,70), ( 5,42),
       (6, 35), (7, 30), (10,21), (14,15)]}

/// Product of the first 5 primes: 2 3 5 7 11  (12 bits, treat as 16-bit)
const P5 : usize = 2_310;
fn p5_factors()->Vec<(u64,u64)> {
  vec![(10, 231), (11, 210), (14, 165), (15, 154), (21, 110),
       (22, 105), (30, 77), (33, 70), (35,66), (42,55)]}

/// Product of the first 6 primes: 2 3 5 7 11 13   (15 bits, treat as 16-bit)
const P6 : usize = 30_030;
fn p6_factors()->Vec<(u64,u64)> {
  vec![(130,231), (143,210), (154,195), (165,182)]}


extern crate bex;
use bex::{solve::find_factors, bdd::BddBase, int::{X8,X16}, swap::SwapSolver};

include!(concat!(env!("OUT_DIR"), "/bex-build-info.rs"));

pub fn main() {
  // -- parse arguments ----
  let mut use_swap = false;
  let mut get_which = false; let mut which = 4;
  let mut get_threads = false; let mut num_threads = 0;
  for a in std::env::args() {
    if get_threads { num_threads = a.parse().expect("bad -t parameter"); get_threads=false; }
    else if get_which { which = a.parse().expect("bad -p parameter"); get_which=false; }
    else { match a.as_str() {
      "-t" => get_threads = true,
      "-p" => get_which = true,
      "swap" => use_swap = true,
      _ => { /* ignore for now */} }}}

  let (k, expected) = match which {
    4 => (P4, p4_factors()),
    5 => (P5, p5_factors()),
    6 => (P6, p6_factors()),
    _ => { panic!("the available primorials are: 4,5,6") }};

  // -- print current configuration ---
  println!("[bex {BEX_VERSION} -O{BEX_OPT_LEVEL}] factor-p4 -t {num_threads} -p {which} ({})",
    if use_swap { "swap solver" } else { "sub solver" });

  // ---- run the requested solver
  if use_swap {
    if num_threads != 0 { println!("note: swap solver ignores -t parameter"); }
    find_factors::<X8, X16, SwapSolver>(&mut SwapSolver::new(), k, expected); }
  else { find_factors::<X8, X16, BddBase>(&mut BddBase::new_with_threads(num_threads), k, expected); }}
