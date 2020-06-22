  #![macro_use]

/// "solve" ast-based expressions by converting to another form.
use apl;
use ast::{Op,ASTBase};
use base::Base;
use nid;
use vid;

type B = dyn Base;

pub trait Progress {
  fn on_start(&self);
  fn on_step(&self, src:&ASTBase, dest: &mut B, step:usize, secs:u64, oldtop:DstNid, newtop:DstNid);
  fn on_done(&self, src:&ASTBase, dest: &mut B, newtop:DstNid); }

pub struct ProgressReport<'a> {
  pub save_dot: bool,
  pub save_dest: bool,
  pub prefix: &'a str,
  pub show_result: bool,
  pub save_result: bool }

/// these are wrappers so the type system can help us keep the src and dest nids separate
#[derive(Clone, Copy, Debug, PartialEq)] pub struct SrcNid { pub n: nid::NID }
#[derive(Clone, Copy, Debug, PartialEq)] pub struct DstNid { pub n: nid::NID }


impl<'a> Progress for ProgressReport<'a> {
  fn on_start(&self) { } //println!("step, seconds, topnid, oldtopvar, newtopvar"); }
  fn on_step(&self, src:&ASTBase, dest: &mut B, step:usize, secs:u64, oldtop:DstNid, newtop:DstNid) {
    let DstNid{ n: new } = newtop;
    println!("{:4}, {:4}, {:4?}→{:3?}, {:8?}",
             step, secs, oldtop, new, /*src.get_op(nid::nvi(nid::NOVAR, nid::var(new) as u32)),*/ newtop);
    // dest.show_named(newtop.n, format!("step-{}", step).as_str());
    if step.trailing_zeros() >= 3 { // every so often, save the state
      // !! TODO: expected number of steps only works if sort_by_cost was called.
      { let expected_steps = src.len() as f64;
        let percent_done = 100.0 * (step as f64) / expected_steps as f64;
        println!("\n# newtop: {:?}  step:{}/{} ({:.2}%)",
                 newtop, step, src.len(), percent_done); }
      if self.save_dest {
        dest.tag(new, "top".to_string()); dest.tag(nid::nv(step), "step".to_string());
        // TODO: remove the 'bdd' suffix
        dest.save(format!("{}-{:04}.bdd", self.prefix, step).as_str())
          .expect("failed to save"); }}
    if step.trailing_zeros() >= 5 { println!("step, seconds, change, newtop"); }
    if self.save_dot && (step.trailing_zeros() >= 5) || (step==446)
    { // on really special occasions, output a diagram
      dest.save_dot(new, format!("{}-{:04}.dot", self.prefix, step).as_str()); } }

  fn on_done(&self, _src:&ASTBase, dest: &mut B, newtop:DstNid) {
    if self.show_result {
      dest.show_named(newtop.n, format!("{}-final", self.prefix).as_str()); }
    else if self.save_result {
      dest.save_dot(newtop.n, format!("{}-final.dot", self.prefix).as_str()); }
    else {}}}


fn default_bitmask(_src:&ASTBase, v0:vid::VID) -> u64 {
  let v = v0.u();
  if v < 64 { 1u64 << v } else { 0 }}

/// This function renumbers the NIDs so that nodes with higher IDs "cost" more.
/// Sorting your AST this way dramatically reduces the cost of converting to
/// another form. (For example, the test_tiny benchmark drops from 5282 steps to 111 for BDDBase)
#[allow(clippy::needless_range_loop)]
pub fn sort_by_cost(src:&ASTBase, top:SrcNid)->(ASTBase,SrcNid) {
  let (mut src0,kept0) = src.repack(vec![top.n]);
  src0.tag(kept0[0], "-top-".to_string());

  // m:mask (which input vars are required?); c:cost (in steps before we can calculate)
  let (_m0,c0) = src0.masks_and_costs(default_bitmask);
  let mut p = apl::gradeup(&c0); // p[new idx] = old idx
  let src1 = src0.permute(&p);

  // now permute so that vars are on bottom and everything else is flipped
  // this is purely so that the node we want to replace remains on top in the destination
  let max = p.len()-1;
  for i in 0..p.len() { p[i] = max-i }
  let ast = src1.permute(&p);
  let n = ast.get("-top-").expect("what? I just put it there.");
  (ast,SrcNid{n}) }


pub fn refine<P:Progress>(dest: &mut B, src:&ASTBase, end:DstNid, pr:P)->DstNid {
  // end is the root of the expression to simplify, as a nid in the src ASTbase.
  // we want its equivalent expression in the dest base:
  let mut top = end;
  println!("INITIAL TOPNID: {:?}", top);
  // step is just a number. we're packing it in a nid as a kludge
  let mut step = nid::var(dest.get(&"step".to_string()).unwrap_or_else(||nid::nv(0)));
  pr.on_start();
  while !(nid::is_rvar(top.n) || nid::is_const(top.n)) {
    let now = std::time::SystemTime::now();
    let old = top;
    top = refine_one(dest, &src, top);
    assert!(old != top, "top should have changed!");
    let secs = now.elapsed().expect("elapsed?").as_secs();
    pr.on_step(src, dest, step, secs, old, top);
    step += 1; }
  pr.on_done(src, dest, top);
  top }

/// map a nid from the source to a (usually virtual) variable in the destination
pub fn convert_nid(sn:SrcNid)->DstNid {
  let SrcNid{ n } = sn;
  let r = if nid::is_const(n) { n }
  else {
    let r0 = if nid::is_var(n) { nid::nvr(nid::var(n)) }
    else if nid::no_var(n) { nid::nv(nid::idx(n)) }
    else { todo!("convert_nid({:?})", n) };
    if nid::is_inv(n) { nid::not(r0)} else { r0 }};
  DstNid{ n: r } }

/// replace a
fn refine_one(dst: &mut B, src:&ASTBase, d:DstNid)->DstNid {
  // println!("refine_one({:?})", d);
  if nid::is_const(d.n) || nid::is_rvar(d.n) { d }
  else {
    let otv = d.n.vid();
    let op = src.get_op(nid::ixn(otv.u() as u32));
    let cn = |x0:nid::NID|->nid::NID { convert_nid(SrcNid{n:x0}).n };
    // println!("op: {:?}", op);
    let newdef:nid::NID = match op {
      Op::O | Op::I | Op::Var(_) | Op::Not(_) => panic!("Src base should not contain {:?}", op),
      // the VIDs on the right here are because we're treating each step in the
      // calculation as a 'virtual' input variable, and just slowly simplifying
      // until the virtual variables are all gone.
      Op::And(x,y) => dst.and(cn(x), cn(y)),
      Op::Xor(x,y) => dst.xor(cn(x), cn(y)),
      Op::Or(x,y) => dst.or(cn(x), cn(y)),
      _ => { panic!("don't know how to translate {:?}", op ) }};
    DstNid{n: dst.sub(otv, newdef, d.n) }}}


/// This is an example solver used by the bdd-solve example and the bench-solve benchmark.
/// It finds all pairs of type $T0 that multiply to $k as a $T1. ($T0 and $T1 are
/// BInt types. Generally $T0 would have half as many bits as $T1) $TDEST is destination type.
#[macro_export]
macro_rules! find_factors {
  ($TDEST:ident, $T0:ident, $T1:ident, $k:expr, $expect:expr, $show:expr) => {{
    use bex::{Base,nid, solve::{SrcNid, DstNid, convert_nid}};
    // reset gbase on each test
    GBASE.with(|gb| gb.replace(ASTBase::empty()));
    let (x, y) = ($T0::def("x"), $T0::def("y")); let lt = x.lt(&y);
    let xy:$T1 = x.times(&y); let k = $T1::new($k); let eq = xy.eq(&k);
    if $show {
      GBASE.with(|gb| { gb.borrow().show_named(lt.clone().n, "lt") });
      GBASE.with(|gb| { gb.borrow().show_named(eq.clone().n, "eq") }); }
    let mut top:BaseBit = lt & eq;
    let mut dest = $TDEST::new(nid::idx(top.n));
    let answer:DstNid = GBASE.with(|gb| {
      let (src, top) = sort_by_cost(&gb.borrow(), SrcNid{n:top.n});
      if $show { src.show_named(top.n, "ast"); }
      dest = $TDEST::new(src.len());
      assert!(nid::no_var(top.n), "top nid seems to be a literal. (TODO: handle these already solved cases)");
      refine(&mut dest, &src, DstNid{n: nid::nv(nid::idx(top.n))},
             ProgressReport{ save_dot: $show, save_dest: false, prefix: "x",
                             show_result: $show, save_result: $show }) });
    let expect = $expect;
    let answer = answer.n;
    let actual:Vec<(u64, u64)> = dest.solutions_trunc(answer, 2*$T0::n() as usize).map(|r|{
      let t = r.as_usize_rev();
      let x = t & ((1<<$T0::n())-1);
      let y = t >> $T0::n();
      (x as u64, y as u64)
    }).collect();
    assert_eq!(actual.len(), expect.len());
    for i in 0..expect.len() {
      assert_eq!(actual[i], expect[i], "mismatch at i={}", i) }
  }}
}

/// First step of the solve procedure: do a calculation that results in a bit.
#[test] fn test_nano_expanded() {
  use int::*;
  GBASE.with(|gb| gb.replace(ASTBase::empty()));
  let x = X2::def("x"); let y = X2::def("y");
  let lt = x.lt(&y);
  let xy:X4 = x.times(&y); let k = X4::new(6);
  let eq = xy.eq(&k);
  let top = lt.clone() & eq.clone();
  //GBASE.with(|gb| { gb.borrow().show_named(lt.clone().n, "lt") });
  //GBASE.with(|gb| { gb.borrow().show_named(eq.clone().n, "eq") });
  //GBASE.with(|gb| { gb.borrow().show_named(top.clone().n, "top") });
  println!("lt: {:?}", lt);
  println!("eq: {:?}", eq);
  println!("top: {:?}", top);
}
