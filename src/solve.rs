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

use ::{apl, ops};
use ast::{RawASTBase};
use base::{Base};
use {nid, nid::NID};
use {vid::VID};
use ops::Ops;
use reg::Reg;
use hashbrown::HashSet;
use std::path::Path;


/// protocol used by solve.rs. These allow the base to prepare itself for different steps
/// in a substitution solver.
pub trait SubSolver {
  /// Initialize the solver by constructing the node corresponding to the final
  /// virtual variable in the expression. Return its nid.
  fn init(&mut self, top: VID)->NID { NID::from_vid(top) }
  /// tell the implementation to perform a substitution step.
  /// context NIDs are passed in and out so the implementation
  /// itself doesn't have to remember it.
  fn subst(&mut self, ctx:NID, vid:VID, ops:&Ops)->NID;
  /// fetch a solution, (if one exists)
  fn get_one(&self, ctx:NID, nvars:usize)->Option<Reg> {
    println!("Warning: default SubSolver::get_one() calls get_all(). Override this!");
    self.get_all(ctx, nvars).iter().next().cloned() }
  /// fetch all solutions
  fn get_all(&self, ctx:NID, nvars:usize)->HashSet<Reg>;
  // a status message for the progress report
  fn status(&self)->String { "".to_string() }
  /// Dump the current internal state for inspection by some external process.
  /// Generally this means writing to a graphviz (*.dot) file.
  /// The step number, status note, and a copy of the arguments to the
  /// previous subst(), and the result are provided, in case the dump format
  /// can make use of them in some way.
  fn dump(&self, _path:&Path, _note:&str, _step:usize, _old:NID, _vid:VID, _ops:&Ops, _new:NID); }

impl<B:Base> SubSolver for B {

  fn subst(&mut self, ctx:NID, v:VID, ops:&Ops) ->NID {
    let def = match ops {
      Ops::RPN(x) => if x.len() == 3 {
        match x[2] {
          ops::AND => self.and(x[0], x[1]),
          ops::XOR => self.xor(x[0], x[1]),
          ops::VEL => self.or(x[0], x[1]),
          _ => panic!("don't know how to translate {:?}", ops)}}
        else { todo!("SubSolver impl for Base can only handle simple dyadic ops for now.") }};
      //_ => { todo!("SubSolver impl for Base can only handle RPN for now")}};
    self.sub(v, def, ctx)}

  fn get_all(&self, ctx:NID, nvars:usize)->HashSet<Reg> { self.solution_set(ctx, nvars) }

  fn dump(&self, _path:&Path, _note:&str, _step:usize, _old:NID, _vid:VID, _ops:&Ops, _new:NID) {}}

pub trait Progress<S:SubSolver> {
  fn on_start(&self, ctx:&DstNid) { println!("INITIAL ctx: {:?}", ctx) }
  fn on_step(&mut self, src:&RawASTBase, dest: &mut S, step:usize, millis:u128, oldtop:DstNid, newtop:DstNid);
  fn on_done(&self, src:&RawASTBase, dest: &mut S, newtop:DstNid); }

pub struct ProgressReport<'a> {
  pub millis: u128,
  pub save_dot: bool,
  pub save_dest: bool,
  pub prefix: &'a str }

/// these are wrappers so the type system can help us keep the src and dest nids separate
#[derive(Clone, Copy, Debug, PartialEq)] pub struct SrcNid { pub n: NID }
#[derive(Clone, Copy, Debug, PartialEq)] pub struct DstNid { pub n: NID }


impl<'a, S:SubSolver> Progress<S> for ProgressReport<'a> {
  fn on_step(&mut self, src:&RawASTBase, dest: &mut S, step:usize, millis:u128, oldtop:DstNid, newtop:DstNid) {
    self.millis += millis;
    let DstNid{ n: new } = newtop;
    println!("{:4}, {:8} ms, {:45?} → {:45?}, {:45?}",
             step, millis, oldtop.n,
             if new.vid().is_vir() {
               format!("{:?}", src.get_ops(nid::ixn(new.vid().vir_ix() as u32))) }
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
        println!("TODO: save_dest for SwapSolver instead of Base")
        // dest.tag(new, "top".to_string()); dest.tag(NID::var(step as u32), "step".to_string());
        // TODO: remove the 'bdd' suffix
        // dest.save(format!("{}-{:04}.bdd", self.prefix, step).as_str()).expect("failed to save");
       }}
    if step.trailing_zeros() >= 5 { println!("step, millis, change, newtop"); }
    if self.save_dot && (step.trailing_zeros() >= 5) || (step==446)
    { // on really special occasions, output a diagram
      let note = &dest.status();
      let path = Path::new("."); // todo
      let ops = &Ops::RPN(vec![]); // todo
      dest.dump(path, note, step, oldtop.n, newtop.n.vid(), ops, newtop.n); }}

  fn on_done(&self, _src:&RawASTBase, _dest: &mut S, _newtop:DstNid) {
    println!("total time: {} ms", self.millis) }}


fn default_bitmask(_src:&RawASTBase, v:VID) -> u64 { v.bitmask() }

/// This function renumbers the NIDs so that nodes with higher IDs "cost" more.
/// Sorting your AST this way dramatically reduces the cost of converting to
/// another form. (For example, the test_tiny benchmark drops from 5282 steps to 111 for BDDBase)
pub fn sort_by_cost(src:&RawASTBase, top:SrcNid)->(RawASTBase,SrcNid) {
  let (mut src0,kept0) = src.repack(vec![top.n]);
  src0.tag(kept0[0], "-top-".to_string());
  // m:mask (which input vars are required?); c:cost (in steps before we can calculate)
  let (_m0,c0) = src0.masks_and_costs(default_bitmask);
  let p = apl::gradeup(&c0); // p[new idx] = old idx
  let ast = src0.permute(&p);
  let n = ast.get("-top-").expect("what? I just put it there.");
  (ast,SrcNid{n}) }


/// map a nid from the source to a (usually virtual) variable in the destination
pub fn convert_nid(sn:SrcNid)->DstNid {
  let SrcNid{ n } = sn;
  let r = if nid::is_const(n) { n }
  else {
    let r0 = if n.is_vid() { NID::var(nid::vid(n) as u32) } // TODO: probably want
    else if nid::no_var(n) { NID::vir(nid::idx(n) as u32) }
    else { todo!("convert_nid({:?})", n) };
    if nid::is_inv(n) { !r0 } else { r0 }};
  DstNid{ n: r } }

/// replace node in destination with its definition form source
fn refine_one(dst: &mut dyn SubSolver, v:VID, src:&RawASTBase, d:DstNid)->DstNid {
  // println!("refine_one({:?})", d)
  let ctx = d.n;
  let ops = src.get_ops(nid::ixn(v.vir_ix() as u32));
  let cn = |x0:&NID|->NID { if x0.is_fun() { *x0 } else { convert_nid(SrcNid{n:*x0}).n }};
  let def:Ops = Ops::RPN( ops.to_rpn().map(cn).collect() );
  DstNid{n: dst.subst(ctx, v, &def) }}


/// This is the core algorithm for solving by substitution. We are given a (presumably empty)
/// destination (the `SubSolver`), a source ASTBase (`src0`), and a source nid (`sn`),
/// pointing to a node inside the ASTBase.
///
/// The source nids we encounter are indices into the ASTBase. We begin by sorting/rewriting
/// the ASTBase in terms of "cost", so that a node at index k is only dependent on nodes
/// with indices < k. We also filter out any nodes that are not actually used (for example,
/// there may be nodes in the middle of the AST that are expensive to calculate on their own,
/// but get canceled out later on (perhaps by XORing with itself, or ANDing with 0) -- there's
/// no point including these at all as we work backwards.
///
/// After this sorting and filtering, we map each nid in the AST to a `VID::vir` with
/// the corresponding index. We then initialize `dst` with the highest vid (the one
/// corresponding to the topmost/highest cost node in the AST).
///
/// We then replace each VID in turn with its definition. The definition of each intermediate
/// node is always in terms of either other AST nodes (mapped to `VID::vir` in the destination,
/// or actual input variables (`VID::var`), which are added to the destination directly).
///
/// The dependency ordering ensures that we never re-introduce a node after substitution,
/// so the number of substitution steps is equal to the number of AST nodes.
///
/// Of course, the cost of each substitution is likely to increase as the destination
/// becomes more and more detailed. Depending on the implementation, this cost may even
/// grow exponentially. However, the hope is that by working "backward" from the final
/// result, we will have access to the maximal number of constraints, and there
/// will be opportunities to streamline and cancel out even more nodes. The hope is that
/// no matter how slow this process is, it will be less slow that trying to fully solve
/// each intermediate node by working "forward".
pub fn solve<S:SubSolver>(dst:&mut S, src0:&RawASTBase, sn:NID)->DstNid {
  // AST nids don't contain VIR nodes (they "are" vir nodes).
  // If it's already a const or a VID::var, though, there's nothing to do.
  if sn.is_lit() { DstNid{n:sn} }
  else {
    dst.init(sn.vid());
    // renumber and garbage collect, leaving only the AST nodes reachable from sn
    let (src, top) = sort_by_cost(&src0, SrcNid{n:sn});

    // step is just a number that counts downward.
    let mut step:usize = nid::idx(top.n);

    // !! These lines were a kludge to allow storing the step number in the dst,
    //    with the idea of persisting the destination to disk to resume later.
    //    The current solvers are so slow that I'm not actually using them for
    //    anything but testing, though, so I don't need this yet.
    // TODO: re-enable the ability to save and load the destination mid-run.
    // let step_node = dst.get(&"step".to_string()).unwrap_or_else(||NID::var(0));
    // let mut step:usize = step_node.vid().var_ix();

    // v is the next virtual variable to replace.
    let mut v = VID::vir(step as u32);

    // The context is the evolving top-level node in the destination.
    // It begins with just the vir representing the top node in the AST.
    let mut ctx = DstNid{n: dst.init(v)};

    // This just lets us record timing info. TODO: pr probably should be an input parameter.
    let mut pr = ProgressReport{ save_dot: false, save_dest: false, prefix:"x", millis: 0 };
    <dyn Progress<S>>::on_start(&pr, &ctx);

    // main loop:
    while !(ctx.n.is_var() || ctx.n.is_const()) {
      let now = std::time::SystemTime::now();
      let old = ctx; ctx = refine_one(dst, v, &src, ctx);
      let millis = now.elapsed().expect("elapsed?").as_millis();
      pr.on_step(&src, dst, step, millis, old, ctx);
      if step == 0 { break } else { step -= 1; v=VID::vir(step as u32) }}
    pr.on_done(&src, dst, ctx);
    ctx}}


/// This is an example solver used by the bdd-solve example and the bench-solve benchmark.
/// It finds all pairs of type $T0 that multiply to $k as a $T1. ($T0 and $T1 are
/// BInt types. Generally $T0 would have half as many bits as $T1) $TDEST is destination type.
#[macro_export]
macro_rules! find_factors {
  ($TDEST:ident, $T0:ident, $T1:ident, $k:expr, $expect:expr) => {{
    use std::env;
    use $crate::{GraphViz, nid, solve::*, ast::ASTBase, int::{GBASE,BInt,BaseBit}, bdd};
    bdd::COUNT_XMEMO_TEST.with(|c| c.replace(0) );
    bdd::COUNT_XMEMO_FAIL.with(|c| c.replace(0) ); // TODO: other bases
    GBASE.with(|gb| gb.replace(ASTBase::empty()));   // reset on each test
    let (y, x) = ($T0::def("y", 0), $T0::def("x", $T0::n())); let lt = x.lt(&y);
    let xy:$T1 = x.times(&y); let k = $T1::new($k); let eq = xy.eq(&k);
    let mut show_ast = false; // let mut show_res = false;
    for arg in env::args() { match arg.as_str() {
      "-a" => { show_ast = true }
      "-r" => { /*show_res = true*/ }
      _ => {} }}
    if show_ast {
      GBASE.with(|gb| { gb.borrow().show_named(lt.clone().n, "lt") });
      GBASE.with(|gb| { gb.borrow().show_named(eq.clone().n, "eq") }); }
    let top:BaseBit = lt & eq;
    assert!(nid::no_var(top.n), "top nid seems to be a literal. (TODO: handle these already solved cases)");
    let gb = GBASE.with(|gb| gb.replace(ASTBase::empty())); // swap out the thread-local one
    let src = gb.raw_ast();
    if show_ast { src.show_named(top.n, "ast"); }
    // --- now we have the ast, so solve ----
    let mut dest = $TDEST::new();
    let answer:DstNid = solve(&mut dest, &src, top.n);
    // if show_res { dest.show_named(answer.n, "result") }
    type Factors = (u64,u64);
    let to_factors = |r:&Reg|->Factors {
      let t = r.as_usize();
      let x = t & ((1<<$T0::n())-1);
      let y = t >> $T0::n();
      (y as u64, x as u64) };
    let actual_regs:HashSet<Reg> = dest.get_all(answer.n, 2*$T0::n() as usize);
    let actual:HashSet<Factors> = actual_regs.iter().map(to_factors).collect();
    let expect:HashSet<Factors> = $expect.iter().map(|&(x,y)| (x as u64, y as u64)).collect();
    assert_eq!(actual, expect);
    let tests = bdd::COUNT_XMEMO_TEST.with(|c| *c.borrow() );
    let fails = bdd::COUNT_XMEMO_FAIL.with(|c| *c.borrow() );
    println!("XMEMO: tests: {} fails: {} hits: {}", tests, fails, tests-fails); }}
}


/// nano test case for BDD: factor (*/2 3)=6 into two bitpairs. The only answer is 2,3.
#[test] pub fn test_nano_bdd() {
  use {bdd::BDDBase, int::{X2,X4}};
  find_factors!(BDDBase, X2, X4, 6, vec![(2,3)]); }

/// nano test case for ANF: factor (*/2 3)=6 into two bitpairs. The only answer is 2,3.
#[test] pub fn test_nano_anf() {
    use {anf::ANFBase, int::{X2,X4}};
    find_factors!(ANFBase, X2, X4, 6, vec![(2,3)]); }

/// nano test case for swap solver: factor (*/2 3)=6 into two bitpairs. The only answer is 2,3.
#[test] pub fn test_nano_swap() {
  use {swap::SwapSolver, int::{X2,X4}};
  find_factors!(SwapSolver, X2, X4, 6, vec![(2,3)]); }

/// tiny test case: factor (*/2 3 5 7)=210 into 2 nibbles. The only answer is 14,15.
#[test] pub fn test_tiny_bdd() {
  use {bdd::BDDBase, int::{X4,X8}};
  find_factors!(BDDBase, X4, X8, 210, vec![(14,15)]); }

/// tiny test case: factor (*/2 3 5 7)=210 into 2 nibbles. The only answer is 14,15.
#[test] pub fn test_tiny_anf() {
  use {anf::ANFBase, int::{X4,X8}};
  find_factors!(ANFBase, X4, X8, 210, vec![(14,15)]); }

/// tiny test case: factor (*/2 3 5 7)=210 into 2 nibbles. The only answer is 14,15.
#[test] pub fn test_tiny_swap() {
  use {swap::SwapSolver, int::{X4,X8}};
  find_factors!(SwapSolver, X4, X8, 210, vec![(14,15)]); }

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

/// same test using the swap solver
/// `time cargo test --lib --features slowtests test_small_swap`
/// timing on rincewind is 5m13.901s as of 4/23/2021, so the swap
/// solver running on 1 core is more than 2x faster than old solver on 6!
#[cfg(feature="slowtests")]
#[test] pub fn test_small_swap() {
  use {swap::SwapSolver, int::{X8,X16}};
  let expected = vec![(1,210), (2,105), ( 3,70), ( 5,42),
                      (6, 35), (7, 30), (10,21), (14,15)];
  find_factors!(SwapSolver, X8, X16, 210, expected); }
