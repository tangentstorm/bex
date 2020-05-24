#[macro_use]
extern crate bencher;
use bencher::Bencher;

extern crate bex;
use bex::bdd;
use bex::int::{BInt, BaseBit, GBASE, gbase_def};
use bex::{find_factors, solve::{ProgressReport, bdd_refine, sort_by_cost}};
use bex::ast::ASTBase;

pub fn tiny(b: &mut Bencher) {
  use bex::int::{X4,X8};
  b.iter(|| {
    find_factors!(X4,X8, 210, vec![(14,15)], false);
  }); }

pub fn small(b: &mut Bencher) {
  use bex::int::{X8,X16};
  b.iter(|| {
    let expected = vec![(1,210), (2,105), ( 3,70), ( 5,42),
                        (6, 35), (7, 30), (10,21), (14,15)];
    find_factors!(X8,X16, 210, expected, false);
    GBASE.with(|gb| gb.replace(bex::ast::ASTBase::empty()));
  }); }



benchmark_group!(both, tiny, small);
benchmark_main!(both);