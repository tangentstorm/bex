/// solve ast-based expressions by converting to BDDs.
use apl;
use bdd;
use base;
use base::{Op,Base,TBase,VID,NID};

pub trait Progress {
  fn on_start(&self);
  fn on_step(&self, base:&Base, bdds: &mut bdd::BDDBase, step:u32, secs:u64,
             oldtop:bdd::NID, newtop:bdd::NID);
  fn on_done(&self, base:&Base, bdds: &mut bdd::BDDBase, newtop:bdd::NID); }

pub struct ProgressReport<'a> {
  pub save_dot: bool,
  pub save_bdd: bool,
  pub prefix: &'a str,
  pub show_result: bool }

impl<'a> Progress for ProgressReport<'a> {
  fn on_start(&self) { } //println!("step, seconds, topnid, oldtopvar, newtopvar"); }
  fn on_step(&self, base:&Base, bdds: &mut bdd::BDDBase, step:u32, secs:u64,
             oldtop:bdd::NID, newtop:bdd::NID) {
    println!("{:4}, {:4}, {:4}→{:3?}, {:8}",
             step, secs, oldtop, base[bdd::var(oldtop) as usize], newtop);
    if step&7 == 0 { // every so often, save the state
      // !! TODO: expected number of steps only works if sort_by_cost was called.
      { let expected_steps = base.bits.len() as f64;
        let percent_done = 100.0 * (step as f64) / expected_steps as f64;
        println!("\n# newtop: {}  step:{}/{} ({:.2}%)", newtop, step, base.bits.len(), percent_done); }
      if self.save_bdd {
        bdds.tag("top".to_string(), newtop); bdds.tag("step".to_string(), bdd::nv(step));
        bdds.save(format!("{}-{:04}.bdd", self.prefix, step).as_str())
          .expect("failed to save"); }}
    if step &31 == 0  { println!("step, seconds, change, newtop"); }
    if self.save_dot && (step&31 == 0) || (step==446)
    { // on really special occasions, output a diagram
      bdds.save_dot(newtop, format!("{}-{:04}.dot", self.prefix, step).as_str()); } }

  fn on_done(&self, _base:&Base, bdds: &mut bdd::BDDBase, newtop:bdd::NID) {
    if self.show_result {
      bdds.show_named(newtop, format!("{}-final", self.prefix).as_str()); }
    else {
      bdds.save_dot(newtop, format!("{}-final.dot", self.prefix).as_str()); }}}


fn default_bitmask(_base:&Base, v:VID) -> u64 {
  if v < 64 { 1u64 << v } else { 0 }}

/// This function renumbers the NIDs so that nodes with higher IDs "cost" more.
/// Sorting your AST this way dramatically reduces the cost of converting to
/// BDD form. (For example, the test_tiny benchmark drops from 5282 steps to 111)_
pub fn sort_by_cost(base:&Base, top:NID)->(Base,NID) {

  let (mut base0,kept0) = base.repack(vec![top]);
  base0.tag(kept0[0], "-top-".to_string());

  // m:mask (which input vars are required?); c:cost (in steps before we can calculate)
  let (_m0,c0) = base0.masks_and_costs(default_bitmask);
  let mut p = apl::gradeup(&c0); // p[new idx] = old idx
  let base1 = base0.permute(&p);

  // now permute so that vars are on bottom and everything else is flipped
  // this is purely so that the node we want to replace remains on top in the bdd
  let max = p.len()-1; let min = base1.nvars+1;
  for i in 0..min { p[i] = i }
  for i in min..p.len() { p[i] = min + (max-i) }
  let ast = base1.permute(&p);
  let &nid = ast.tags.get("-top-").expect("what? I just put it there.");
  (ast,nid) }




pub fn bdd_refine<P:Progress>(bdds: &mut bdd::BDDBase, base:&Base, end:bdd::NID, pr:P) {
  let mut topnid = end;
  // step is just a number. we're packing it in a nid as a kludge
  let mut step = bdd::var(bdds.get(&"step".to_string()).unwrap_or(bdd::nv(0)));
  let mut newtop = topnid;
  pr.on_start();
  while !bdd::is_rvar(topnid) {
    let now = std::time::SystemTime::now();
    let oldtop = topnid;
    newtop = bdd_refine_one(bdds, &base, oldtop); topnid=newtop;
    let secs = now.elapsed().expect("elapsed?").as_secs();
    pr.on_step(base, bdds, step, secs, oldtop, newtop);
    step += 1; }
  pr.on_done(base, bdds, newtop); }

/// map a nid from the base to a (usually virtual) variable in the bdd
fn convert_nid(base:&Base, n:base::NID)->bdd::NID {
  match base[n as usize] {
    Op::O => bdd::O,
    Op::I => bdd::I,
    Op::Var(x) => bdd::nvr(x as bdd::VID),
    _ => bdd::nv(n as u32) }}

fn bdd_refine_one(bdds: &mut bdd::BDDBase, base:&Base, oldtop:bdd::NID)->bdd::NID {
  let otv = bdd::var(oldtop);
  let op = base[otv as usize];
  let v = |x0:base::NID|->bdd::NID { convert_nid(base, x0) };
  let newdef:bdd::NID = match op {
    // Op::Not should only occur once at the very top, if at all:
    Op::Not(x) => bdd::not(v(x)),
    // the VIDs on the right here are because we're treating each step in the
    // calculation as a 'virtual' input variable, and just slowly simplifying
    // until the virtual variables are all gone.
    Op::And(x,y) => bdds.and(v(x), v(y)),
    Op::Xor(x,y) => bdds.xor(v(x), v(y)),
    Op::Or(x,y) => bdds.or(v(x), v(y)),
    // !! 'Var' should only appear in leaves, so don't need it here.
    // Op::Var(x) => bdd::nvr(x as bdd::VID),
    _ => { panic!("don't know how to translate {:?}", op ) }};
  return bdds.replace(otv, newdef, oldtop) }
