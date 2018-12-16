/// solve ast-based expressions by converting to BDDs.
use bdd;
use base::{Op,Base};

pub trait Progress {
  fn on_start(&self);
  fn on_step(&self, bdds: &mut bdd::BDDBase, step:u32, secs:u64,
             topnid:bdd::NID, topvar:bdd::NID); }

pub struct ProgressReport<'a> { pub save_dot: bool, pub save_bdd: bool, pub prefix: &'a str }
impl<'a> Progress for ProgressReport<'a> {
  fn on_start(&self) { println!("step, seconds, topnid, oldtopvar, newtopvar"); }
  fn on_step(&self, bdds: &mut bdd::BDDBase, step:u32, secs:u64,
             topnid:bdd::NID, topvar:bdd::NID) {
    println!("{:4}, {:4}, {:8}, {:8}", step, secs, topnid, topvar);
    if step&7 == 0 { // every so often, save the state
      println!("# top: {}  step:{}", topnid, step);
      if self.save_bdd {
        bdds.tag("top".to_string(), topnid); bdds.tag("step".to_string(), bdd::nv(step));
        bdds.save(format!("{}-{:04}.bdd", self.prefix, step).as_str())
          .expect("failed to save"); }}

    if self.save_dot && step&31 == 0 { // on really special occasions, output a diagram
      bdds.save_dot(topnid, format!("{}-{:04}.dot", self.prefix, step).as_str()); } }}


pub fn bdd_refine<P:Progress>(bdds: &mut bdd::BDDBase, base:&Base, end:bdd::NID, pr:P) {
  let mut topnid = end;
  // step is just a number. we're packing it in a nid as a kludge
  let mut step = bdd::var(bdds.get(&"step".to_string()).unwrap_or(bdd::nv(0)));
  pr.on_start();
  while !bdd::is_rvar(topnid) {
    let now = std::time::SystemTime::now();
    let (tn, newtopvar) = bdd_refine_one(bdds, &base, topnid); topnid = tn;
    let secs = now.elapsed().expect("elapsed?").as_secs();
    pr.on_step(bdds, step, secs, topnid, newtopvar);
    step += 1; }}

fn bdd_refine_one(bdds: &mut bdd::BDDBase, base:&Base,
                  oldtop:bdd::NID)->(bdd::NID,bdd::NID) {
  let otv = bdd::var(oldtop);
  let op = base[otv as usize];
  let v = |x0| { let x = x0 as bdd::VID;
                 if x<(2+base.nvars) as u32 { bdd::nvr(x-2)}
                 else { bdd::nv(x-2) }};
  let newdef:bdd::NID = match op {
    // the VIDs on the right here are because we're treating each step in the
    // calculation as a 'virtual' input variable, and just slowly simplifying
    // until the virtual variables are all gone.
    Op::And(x,y) => bdds.and(v(x), v(y)),
    Op::Xor(x,y) => bdds.xor(v(x), v(y)),
    Op::Or(x,y) => bdds.or(v(x), v(y)),
    Op::Not(x) => bdd::not(v(x)),
    // here is where we get to the real input variables:
    Op::Var(x) => bdd::nvr(x as bdd::VID),
    _ => { println!("don't know how to translate {:?}", op ); oldtop }};
  let res = bdds.replace(otv, newdef, oldtop);
  (res, bdd::nv(bdd::var(res))) }
