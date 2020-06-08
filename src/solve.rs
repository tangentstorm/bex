#![macro_use]

/// "solve" ast-based expressions by converting to another form.
//use apl;
use base::Base;
use ast;
use ast::{Op,ASTBase};
use nid;

type B = dyn Base<N=nid::NID>;


pub trait Progress {
  fn on_start(&self);
  fn on_step(&self, src:&ASTBase, dest: &mut B, step:usize, secs:u64,
             oldtop:nid::NID, newtop:nid::NID);
  fn on_done(&self, src:&ASTBase, dest: &mut B, newtop:nid::NID); }

pub struct ProgressReport<'a> {
  pub save_dot: bool,
  pub save_dest: bool,
  pub prefix: &'a str,
  pub show_result: bool,
  pub save_result: bool }


impl<'a> Progress for ProgressReport<'a> {
  fn on_start(&self) { } //println!("step, seconds, topnid, oldtopvar, newtopvar"); }
  fn on_step(&self, src:&ASTBase, dest: &mut B, step:usize, secs:u64,
             oldtop:nid::NID, newtop:nid::NID) {
    println!("{:4}, {:4}, {:4}→{:3?}, {:8}",
             step, secs, oldtop, src.get_op(oldtop), newtop);
    if step.trailing_zeros() >= 3 { // every so often, save the state
      // !! TODO: expected number of steps only works if sort_by_cost was called.
      { let expected_steps = src.len() as f64;
        let percent_done = 100.0 * (step as f64) / expected_steps as f64;
        println!("\n# newtop: {}  step:{}/{} ({:.2}%)",
                 newtop, step, src.len(), percent_done); }
      if self.save_dest {
        dest.tag(newtop, "top".to_string()); dest.tag(nid::nv(step), "step".to_string());
        // TODO: remove the 'bdd' suffix
        dest.save(format!("{}-{:04}.bdd", self.prefix, step).as_str())
          .expect("failed to save"); }}
    if step.trailing_zeros() >= 5 { println!("step, seconds, change, newtop"); }
    if self.save_dot && (step.trailing_zeros() >= 5) || (step==446)
    { // on really special occasions, output a diagram
      dest.save_dot(newtop, format!("{}-{:04}.dot", self.prefix, step).as_str()); } }

  fn on_done(&self, _src:&ASTBase, dest: &mut B, newtop:nid::NID) {
    if self.show_result {
      dest.show_named(newtop, format!("{}-final", self.prefix).as_str()); }
    else if self.save_result {
      dest.save_dot(newtop, format!("{}-final.dot", self.prefix).as_str()); }
    else {}}}


fn default_bitmask(_src:&ASTBase, v:nid::VID) -> u64 {
  if v < 64 { 1u64 << v } else { 0 }}

/// This function renumbers the NIDs so that nodes with higher IDs "cost" more.
/// Sorting your AST this way dramatically reduces the cost of converting to
/// another form. (For example, the test_tiny benchmark drops from 5282 steps to 111 for BDDBase)
#[allow(clippy::needless_range_loop)]
pub fn sort_by_cost(src:&ASTBase, top:ast::NID)->(ASTBase,ast::NID) {
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


pub fn refine<P:Progress>(dest: &mut B, src:&ASTBase, end:nid::NID, pr:P)->nid::NID {
  let mut topnid = end;
  // step is just a number. we're packing it in a nid as a kludge
  let mut step = nid::var(dest.get(&"step".to_string()).unwrap_or_else(||nid::nv(0)));
  pr.on_start();
  while !(nid::is_rvar(topnid) || nid::is_const(topnid)) {
    let now = std::time::SystemTime::now();
    let oldtop = topnid;
    topnid = refine_one(dest, &src, oldtop);
    let secs = now.elapsed().expect("elapsed?").as_secs();
    pr.on_step(src, dest, step, secs, oldtop, topnid);
    step += 1; }
  pr.on_done(src, dest, topnid);
  topnid }

/// map a nid from the source to a (usually virtual) variable in the destination
fn convert_nid(src:&ASTBase, n:ast::NID)->nid::NID {
  match src.get_op(n) {
    Op::O => nid::O,
    Op::I => nid::I,
    Op::Var(x) => nid::nvr(x as nid::VID),
    _ => if nid::var(n) == nid::NOVAR { n }
         else { todo!("convert_nid({:?})", n) }}}

fn refine_one(dst: &mut B, src:&ASTBase, oldtop:nid::NID)->nid::NID {
  let otv = nid::var(oldtop);
  let op = src.get_op(oldtop);
  let v = |x0:ast::NID|->nid::NID { convert_nid(src, x0) };
  let newdef:nid::NID = match op {
    // Op::Not should only occur once at the very top, if at all:
    Op::Not(x) => dst.not(v(x)),
    // the VIDs on the right here are because we're treating each step in the
    // calculation as a 'virtual' input variable, and just slowly simplifying
    // until the virtual variables are all gone.
    Op::And(x,y) => dst.and(v(x), v(y)),
    Op::Xor(x,y) => dst.xor(v(x), v(y)),
    Op::Or(x,y) => dst.or(v(x), v(y)),
    // !! 'Var' should only appear in leaves, so don't need it here.
    // Op::Var(x) => nid::nvr(x as nid::VID),
    _ => { panic!("don't know how to translate {:?}", op ) }};
  dst.sub(otv, newdef, oldtop) }

/// This is an example solver used by the bdd-solve example and the bench-solve benchmark.
/// It finds all pairs of type $T0 that multiply to $k as a $T1. ($T0 and $T1 are
/// BInt types. Generally $T0 would have half as many bits as $T1) $TDEST is destination type.
#[macro_export]
macro_rules! find_factors {
  ($TDEST:ident, $T0:ident, $T1:ident, $k:expr, $expect:expr, $show:expr) => {{
    use bex::Base;
    // reset gbase on each test
    GBASE.with(|gb| gb.replace(ASTBase::empty()));
    let x = $T0::from_vec((0..$T0::n())
                          .map(|i| gbase_def('x'.to_string(), i as bex::nid::VID)).collect());
    let y = $T0::from_vec((0..$T0::n())
                          .map(|i| gbase_def('y'.to_string(), i as bex::nid::VID)).collect());
    let xy:$T1 = x.times(&y); let k = $T1::new($k); let lt = x.lt(&y); let eq = xy.eq(&k);
    if $show {
      GBASE.with(|gb| { gb.borrow().show_named(lt.clone().n, "lt") });
      GBASE.with(|gb| { gb.borrow().show_named(eq.clone().n, "eq") }); }
    let top:BaseBit = lt & eq;
    let mut dest = $TDEST::new(8);
    let answer = GBASE.with(|gb| {
      //let (src, newtop) = sort_by_cost(&gb.borrow(), nid::un(top.n));
      let (src, newtop) = (gb.borrow(), top.n);
      // The diagram looks exactly the same before and after sort_by_cost, so I
      // only generate it once. The only difference is the internal numbering.
      // However: this sorting dramatically reduces the cost of the conversion.
      // For example, test_tiny drops from to 111 steps.
      if $show { src.show_named(newtop, "ast"); }
      dest = $TDEST::new(src.len());
      refine(&mut dest, &src, newtop,
             ProgressReport{ save_dot: $show, save_dest: false, prefix: "x",
                             show_result: $show, save_result: $show }) });
    let expect = $expect;
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
