#![macro_use]

/// "solve" ast-based expressions by converting to another form.
//use apl;
use base::Base;
use ast::{Op,ASTBase};
use nid;

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
    let DstNid{ n: old } = oldtop;
    let DstNid{ n: new } = newtop;
    println!("{:4}, {:4}, {:4?}→{:3?}, {:8?}",
             step, secs, oldtop, new, /*src.get_op(nid::nvi(nid::NOVAR, nid::var(new) as u32)),*/ newtop);
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


fn default_bitmask(_src:&ASTBase, v:nid::VID) -> u64 {
  if v < 64 { 1u64 << v } else { 0 }}

/// This function renumbers the NIDs so that nodes with higher IDs "cost" more.
/// Sorting your AST this way dramatically reduces the cost of converting to
/// another form. (For example, the test_tiny benchmark drops from 5282 steps to 111 for BDDBase)
#[allow(clippy::needless_range_loop)]
pub fn sort_by_cost(src:&ASTBase, top:SrcNid)->(ASTBase,SrcNid) {
  todo!("rebuild sort_by_cost()")
  /*
  let (mut src0,kept0) = src.repack(vec![top]);
  src0.tag(kept0[0], "-top-".to_string());

  // m:mask (which input vars are required?); c:cost (in steps before we can calculate)
  let (_m0,c0) = src0.masks_and_costs(default_bitmask);
  let mut p = apl::gradeup(&c0); // p[new idx] = old idx
  let src1 = src0.permute(&p);

  // now permute so that vars are on bottom and everything else is flipped
  // this is purely so that the node we want to replace remains on top in the destination
  let max = p.len()-1; let min = src1.num_vars()+1;
  for i in 0..min { p[i] = i }
  for i in min..p.len() { p[i] = min + (max-i) }
  let ast = src1.permute(&p);
  let nid = ast.get("-top-").expect("what? I just put it there.");
  (ast,nid)*/ }


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
  if nid::is_const(n) { DstNid{ n } }
  else if nid::is_var(n) { DstNid{ n: nid::nvr(nid::var(n)) } }
  else if nid::var(n) == nid::NOVAR { DstNid{ n: nid::nv(nid::idx(n)) }}
  else { todo!("convert_nid({:?})", n) }}

/// replace a
fn refine_one(dst: &mut B, src:&ASTBase, d:DstNid)->DstNid {
  // println!("refine_one({:?})", d);
  if nid::is_const(d.n) { d }
  else if nid::is_rvar(d.n) { d }
  else {
    let otv = nid::var(d.n);
    let op = src.get_op(nid::nvi(nid::NOVAR, otv as u32));
    let cn = |x0:nid::NID|->nid::NID { convert_nid(SrcNid{n:x0}).n };
    // println!("op: {:?}", op);
    let newdef:nid::NID = match op {
      // Op::Not should only occur once at the very top, if at all:
      Op::Not(x) => dst.not(cn(x)),
      // the VIDs on the right here are because we're treating each step in the
      // calculation as a 'virtual' input variable, and just slowly simplifying
      // until the virtual variables are all gone.
      Op::And(x,y) => dst.and(cn(x), cn(y)),
      Op::Xor(x,y) => dst.xor(cn(x), cn(y)),
      Op::Or(x,y) => dst.or(cn(x), cn(y)),
      // !! 'Var' should only appear in leaves, so don't need it here.
      //Op::Var(x) => nid::nvr(x as nid::VID),
      _ => { panic!("don't know how to translate {:?}", op ) }};
    // println!("sub(otv:{:?}, new:{:?}, old:{:?})", otv, newdef, d);
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
    let (x, y) = ($T0::def("x"), $T0::def("y"));
    let xy:$T1 = x.times(&y); let k = $T1::new($k); let lt = x.lt(&y); let eq = xy.eq(&k);
    if $show {
      GBASE.with(|gb| { gb.borrow().show_named(lt.clone().n, "lt") });
      GBASE.with(|gb| { gb.borrow().show_named(eq.clone().n, "eq") }); }
    let top:BaseBit = lt & eq;
    let mut dest = $TDEST::new(nid::idx(top.n));
    let answer:DstNid = GBASE.with(|gb| {
      let src = gb.borrow();
      // The diagram looks exactly the same before and after sort_by_cost, so I
      // only generate it once. The only difference is the internal numbering.
      // However: this sorting dramatically reduces the cost of the conversion.
      // For example, test_tiny drops from to 111 steps.
      if $show { src.show_named(top.n, "ast"); }
      dest = $TDEST::new(src.len());
      assert!(nid::var(top.n) == nid::NOVAR, "top nid seems to be a literal. (TODO: handle these already solved cases)");
      refine(&mut dest, &src, DstNid{n: nid::nv(nid::idx(top.n))},
             ProgressReport{ save_dot: $show, save_dest: false, prefix: "x",
                             show_result: $show, save_result: $show }) });
    println!("done with refinement!");
    let expect = $expect;
    let answer = answer.n;
    let actual:Vec<(u64, u64)> = dest.nidsols_trunc(answer, 2*$T0::n() as usize).map(|nids| {
      let mut res = (0, 0);
      let mut it = nids.iter();
      for (i, &n) in it.by_ref().take($T0::n() as usize).enumerate() {
        if !::bex::nid::is_inv(n) {  res.0 |= (1 << i) }}
      for (i, &n) in it.take($T0::n() as usize).enumerate() {
        if !::bex::nid::is_inv(n) { res.1 |= (1 << i) }}
      res }).collect();
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
  let xy:X4 = x.times(&y); let k = X4::new(6);
  let lt = x.lt(&y); let eq = xy.eq(&k);
  let top = lt.clone() & eq.clone();
  //GBASE.with(|gb| { gb.borrow().show_named(lt.clone().n, "lt") });
  //GBASE.with(|gb| { gb.borrow().show_named(eq.clone().n, "eq") });
  //GBASE.with(|gb| { gb.borrow().show_named(top.clone().n, "top") });
}
