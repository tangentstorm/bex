/// "solve" ast-based expressions by converting to another form.
///
/// the tests in this module use command line options to show or hide diagrams.
///     -a show AST (problem statement)
///     -r show result (BDD, ANF, etc)
///
/// note that you need to use two '--' parameters to pass arguments to a test.
///
/// syntax:
///     cargo test pattern -- test_engine_args -- actual_args
/// example:
///     cargo test nano_bdd -- --nocapture -- -a -r
///
/// (the --nocapture is an optional argument to the test engine. it turns off
/// capturing of stdout so that you can see debug lines from the solver)

use apl;
use ast::{Op,RawASTBase};
use base::{Base, GraphViz, SubSolver};
use {nid, nid::NID};
use {vid::VID};

pub trait Progress<B:Base> {
  fn on_start(&self);
  fn on_step(&mut self, src:&RawASTBase, dest: &mut B, step:usize, millis:u128, oldtop:DstNid, newtop:DstNid);
  fn on_done(&self, src:&RawASTBase, dest: &mut B, newtop:DstNid); }

pub struct ProgressReport<'a> {
  pub millis: u128,
  pub save_dot: bool,
  pub save_dest: bool,
  pub prefix: &'a str }

/// these are wrappers so the type system can help us keep the src and dest nids separate
#[derive(Clone, Copy, Debug, PartialEq)] pub struct SrcNid { pub n: NID }
#[derive(Clone, Copy, Debug, PartialEq)] pub struct DstNid { pub n: NID }


impl<'a, B:Base> Progress<B> for ProgressReport<'a> {
  fn on_start(&self) { } //println!("step, millis, topnid, oldtopvar, newtopvar"); }
  fn on_step(&mut self, src:&RawASTBase, dest: &mut B, step:usize, millis:u128, oldtop:DstNid, newtop:DstNid) {
    self.millis += millis;
    let DstNid{ n: new } = newtop;
    println!("{:4}, {:8} ms, {:45?} → {:45?}, {:45?}",
             step, millis, oldtop.n,
             if new.vid().is_vir() {
               format!("{:?}", src.get_op(nid::ixn(new.vid().vir_ix() as u32))) }
             else { format!("{:?}", new)},
             newtop.n);
    // dest.show_named(newtop.n, format!("step-{}", step).as_str());
    if step.trailing_zeros() >= 3 { // every so often, save the state
      // !! TODO: expected number of steps only works if sort_by_cost was called.
      { let expected_steps = src.len() as f64;
        let percent_done = 100.0 * (step as f64) / expected_steps as f64;
        println!("\n# newtop: {:?}  step:{}/{} ({:.2}%)",
                 newtop.n.vid(), step, src.len(), percent_done); }
      if self.save_dest {
        dest.tag(new, "top".to_string()); dest.tag(NID::var(step as u32), "step".to_string());
        // TODO: remove the 'bdd' suffix
        dest.save(format!("{}-{:04}.bdd", self.prefix, step).as_str())
          .expect("failed to save"); }}
    if step.trailing_zeros() >= 5 { println!("step, millis, change, newtop"); }
    if self.save_dot && (step.trailing_zeros() >= 5) || (step==446)
    { // on really special occasions, output a diagram
      dest.save_dot(new, format!("{}-{:04}.dot", self.prefix, step).as_str()); } }

  fn on_done(&self, _src:&RawASTBase, _dest: &mut B, _newtop:DstNid) {
    println!("total time: {} ms", self.millis) }}


fn default_bitmask(_src:&RawASTBase, v:VID) -> u64 { v.bitmask() }

/// This function renumbers the NIDs so that nodes with higher IDs "cost" more.
/// Sorting your AST this way dramatically reduces the cost of converting to
/// another form. (For example, the test_tiny benchmark drops from 5282 steps to 111 for BDDBase)
#[allow(clippy::needless_range_loop)]
pub fn sort_by_cost(src:&RawASTBase, top:SrcNid)->(RawASTBase,SrcNid) {
  let (mut src0,kept0) = src.repack(vec![top.n]);
  src0.tag(kept0[0], "-top-".to_string());

  // m:mask (which input vars are required?); c:cost (in steps before we can calculate)
  let (_m0,c0) = src0.masks_and_costs(default_bitmask);
  let p = apl::gradeup(&c0); // p[new idx] = old idx
  let ast = src0.permute(&p);
  let n = ast.get("-top-").expect("what? I just put it there.");
  (ast,SrcNid{n}) }


pub fn refine<B:Base+SubSolver, P:Progress<B>>(dest: &mut B, src:&RawASTBase, end:DstNid, mut pr:P)->DstNid {
  // end is the root of the expression to simplify, as a nid in the src ASTbase.
  // we want its equivalent expression in the dest base:
  let mut top = end;
  println!("INITIAL TOPNID: {:?}", top);
  // step is just a number. we're packing it in a nid as a kludge
  let step_node = dest.get(&"step".to_string()).unwrap_or_else(||NID::var(0));
  let mut step:usize = step_node.vid().var_ix();
  pr.on_start();
  while !(nid::is_rvar(top.n) || nid::is_const(top.n)) {
    let now = std::time::SystemTime::now();
    let old = top;
    top = refine_one(dest, &src, top);
    assert!(old != top, "top should have changed!");
    let millis = now.elapsed().expect("elapsed?").as_millis();
    pr.on_step(src, dest, step, millis, old, top);
    step += 1; }
  pr.on_done(src, dest, top);
  top }

/// map a nid from the source to a (usually virtual) variable in the destination
pub fn convert_nid(sn:SrcNid)->DstNid {
  let SrcNid{ n } = sn;
  let r = if nid::is_const(n) { n }
  else {
    let r0 = if nid::is_var(n) { NID::var(nid::var(n) as u32) }
    else if nid::no_var(n) { NID::vir(nid::idx(n) as u32) }
    else { todo!("convert_nid({:?})", n) };
    if nid::is_inv(n) { !r0 } else { r0 }};
  DstNid{ n: r } }

/// replace node in destintation with its definition form source
fn refine_one<B:Base+SubSolver>(dst: &mut B, src:&RawASTBase, d:DstNid)->DstNid {
  // println!("refine_one({:?})", d);
  if let Some((otv, ctx)) = dst.next_sub(d.n) { // ctx nid, old top var
    let op = src.get_op(nid::ixn(otv.vir_ix() as u32));
    let cn = |x0:NID|->NID { convert_nid(SrcNid{n:x0}).n };
    // println!("op: {:?}", op);
    let newdef:NID = match op {
      Op::O | Op::I | Op::Var(_) | Op::Not(_) => panic!("Src base should not contain {:?}", op),
      // the VIDs on the right here are because we're treating each step in the
      // calculation as a 'virtual' input variable, and just slowly simplifying
      // until the virtual variables are all gone.
      Op::And(x,y) => dst.and(cn(x), cn(y)),
      Op::Xor(x,y) => dst.xor(cn(x), cn(y)),
      Op::Or(x,y) => dst.or(cn(x), cn(y)),
      _ => { panic!("don't know how to translate {:?}", op ) }};
    DstNid{n: dst.sub(otv, newdef, ctx) }}
  else { d } }


pub fn solve<B:Base+SubSolver>(dst:&mut B, src0:&RawASTBase, n:NID)->DstNid {
  let (src, top) = sort_by_cost(&src0, SrcNid{n});
  refine(dst, &src, DstNid{n: NID::vir(nid::idx(top.n) as u32)},
        ProgressReport{ save_dot: false, save_dest: false, prefix:"x", millis: 0 }) }

/// This is an example solver used by the bdd-solve example and the bench-solve benchmark.
/// It finds all pairs of type $T0 that multiply to $k as a $T1. ($T0 and $T1 are
/// BInt types. Generally $T0 would have half as many bits as $T1) $TDEST is destination type.
#[macro_export]
macro_rules! find_factors {
  ($TDEST:ident, $T0:ident, $T1:ident, $k:expr, $expect:expr) => {{
    use std::env;
    use $crate::{Base, SubSolver, GraphViz, nid, solve::*, ast::ASTBase, int::{GBASE,BInt,BaseBit}, bdd};
    bdd::COUNT_XMEMO_TEST.with(|c| *c.borrow_mut()=0 ); bdd::COUNT_XMEMO_TEST.with(|c| *c.borrow_mut()=0 ); // TODO: other bases
    GBASE.with(|gb| gb.replace(ASTBase::empty()));   // reset on each test
    let (y, x) = ($T0::def("y"), $T0::def("x")); let lt = x.lt(&y);
    let xy:$T1 = x.times(&y); let k = $T1::new($k); let eq = xy.eq(&k);
    let mut show_ast = false; let mut show_res = false;
    for arg in env::args() { match arg.as_str() {
      "-a" => { show_ast = true }
      "-r" => { show_res = true }
      _ => {} }}
    if show_ast {
      GBASE.with(|gb| { gb.borrow().show_named(lt.clone().n, "lt") });
      GBASE.with(|gb| { gb.borrow().show_named(eq.clone().n, "eq") }); }
    let top0:BaseBit = lt & eq;
    let gb = GBASE.with(|gb| gb.replace(ASTBase::empty())); // swap out the thread-local one
    let (src, top) = sort_by_cost(gb.raw_ast(), SrcNid{n:top0.n});
    assert!(nid::no_var(top.n), "top nid seems to be a literal. (TODO: handle these already solved cases)");
    if show_ast { src.show_named(top.n, "ast"); }
    // --- now we have the ast, so solve ----
    let mut dest = $TDEST::new(src.len()); dest.init_sub(top.n);
    let answer:DstNid = solve(&mut dest, &src, top.n);
    if show_res { dest.show_named(answer.n, "result") }
    let expect = $expect;
    let answer = answer.n;
    let actual:Vec<(u64, u64)> = dest.solutions_trunc(answer, 2*$T0::n() as usize).map(|r|{
      let t = r.as_usize();
      let x = t & ((1<<$T0::n())-1);
      let y = t >> $T0::n();
      (y as u64, x as u64) }).collect();
    assert_eq!(actual, expect);
    assert_eq!(actual.len(), expect.len(), "check number of solutions");
    for i in 0..expect.len() { assert_eq!(actual[i], expect[i], "mismatch at i={}", i) }
    let tests = bdd::COUNT_XMEMO_TEST.with(|c| *c.borrow() );
    let fails = bdd::COUNT_XMEMO_FAIL.with(|c| *c.borrow() );
    println!("TOTAL XMEMO STATS: tests: {} fails: {} hits: {}", tests, fails, tests-fails); }}
}


/// nano test case for BDD: factor (*/2 3)=6 into two bitpairs. The only answer is 2,3.
#[test] pub fn test_nano_bdd() {
  use {bdd::BDDBase, int::{X2,X4}};
  find_factors!(BDDBase, X2, X4, 6, vec![(2,3)]); }

/// nano test case for ANF: factor (*/2 3)=6 into two bitpairs. The only answer is 2,3.
#[test] pub fn test_nano_anf() {
    use {anf::ANFBase, int::{X2,X4}};
    find_factors!(ANFBase, X2, X4, 6, vec![(2,3)]); }

    /// tiny test case: factor (*/2 3 5 7)=210 into 2 nibbles. The only answer is 14,15.
#[test] pub fn test_tiny_bdd() {
  use {bdd::BDDBase, int::{X4,X8}};
  find_factors!(BDDBase, X4, X8, 210, vec![(14,15)]); }

/// tiny test case: factor (*/2 3 5 7)=210 into 2 nibbles. The only answer is 14,15.
#[test] pub fn test_tiny_anf() {
  use {anf::ANFBase, int::{X4,X8}};
  find_factors!(ANFBase, X4, X8, 210, vec![(14,15)]); }

/// multi: factor (*/2 3 5)=30 into 2 nibbles. There are three answers.
#[test] pub fn test_multi_bdd() {
  use {bdd::BDDBase, int::{X4,X8}};
  find_factors!(BDDBase, X4, X8, 30, vec![(2,15), (3,10), (5,6)]); }

/// multi: factor (*/2 3 5)=30 into 2 nibbles. There are three answers.
#[test] pub fn test_multi_anf() {
  use {anf::ANFBase, int::{X4,X8}};
  find_factors!(ANFBase, X4, X8, 30, vec![(2,15), (3,10), (5,6)]); }

/// same as tiny test, but multiply 2 bytes to get 210. There are 8 distinct answers.
/// this was intended as a unit test but is *way* too slow.
/// (11m17.768s on rincewind (hex-core Intel i7-8700K @ 3.70 GHz with 16GB ram) as of 6/16/2020)
/// (that's with debug information and no optimizations enabled in rustc)
#[cfg(feature="slowtests")]
#[test] pub fn test_small_bdd() {
  use {bdd::BDDBase, int::{X8,X16}};
  let expected = vec![(1,210), (2,105), ( 3,70), ( 5,42),
                      (6, 35), (7, 30), (10,21), (14,15)];
  find_factors!(BDDBase, X8, X16, 210, expected); }
