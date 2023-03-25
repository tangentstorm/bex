//! this program factors primorial 4 (210 = */2 3 5 7)
//! into two bytes x,y where that x<y.

/// Product of the first 4 primes: 2 3 5 7
const K:usize = 210;

fn factors()->Vec<(u64,u64)> {
  vec![(1,210), (2,105), ( 3,70), ( 5,42),
       (6, 35), (7, 30), (10,21), (14,15)]}

extern crate bex;
use bex::{find_factors,{reg::Reg},bdd::BDDBase, int::{X8,X16}, Base};

include!(concat!(env!("OUT_DIR"), "/bex-build-info.rs"));

pub fn main() {
  println!("bex {BEX_VERSION} opt-level: {BEX_OPT_LEVEL}");
  let expected = factors();
  find_factors!(BDDBase::new_with_threads(0), X8, X16, K, expected);
  //  find_factors!(SwapSolver, X8, X16, K, expected);
 }
