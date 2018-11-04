/// bex: a boolean expression library for rust

use std::collections::HashMap;
use std::ops::Index;
extern crate std;
pub mod apl;
pub mod x32;
pub mod io;



// abstract bits and bit base types (trait TBase)
pub type VID = usize;
pub type NID = usize;
type SID = usize; // canned substition
type SUB = HashMap<VID,NID>;

pub const GONE:usize = 1<<63;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Op {
  O, I, Var(VID), Not(NID), And(NID,NID), Or(NID,NID), Xor(NID,NID),
  Ch(NID, NID, NID), Mj(NID, NID, NID) }

/// outside the base, you deal only with opaque references.
/// inside, it could be stored any way we like.
pub trait TBase {
  fn o(&self)->NID;
  fn i(&self)->NID;
  fn var(&mut self, v:VID)->NID;
  fn def(&mut self, s:String, i:u32)->NID;
  fn tag(&mut self, n:NID, s:String)->NID;
  fn not(&mut self, x:NID)->NID;
  fn and(&mut self, x:NID, y:NID)->NID;
  fn xor(&mut self, x:NID, y:NID)->NID;
  fn or(&mut self, x:NID, y:NID)->NID;
  fn mj(&mut self, x:NID, y:NID, z:NID)->NID;
  fn ch(&mut self, x:NID, y:NID, z:NID)->NID;
  fn when(&mut self, v:VID, val:NID, nid:NID)->NID;
  fn sid(&mut self, kv:SUB) -> SID;
  fn sub(&mut self, x:NID, s:SID)->NID; // set many inputs at once
  fn nid(&mut self, op:Op)->NID;   // given an op, return a nid
}

// a concrete implemetation:

// !! TODO: move subs/subc into external structure
pub struct Base {
  pub bits: Vec<Op>,               // all known bits (simplified)     TODO: make private
  pub tags: HashMap<String, NID>,       // support for naming/tagging bits.  TODO: make private
  hash: HashMap<Op, NID>,      // expression cache (simple+complex)
  vars: Vec<NID>,                   // quick index of Var(n) in bits
  subs: Vec<SUB>,                   // list of substitution dicts
  subc: Vec<HashMap<NID,NID>>       // cache of substiution results
}

type VarMaskFn = fn(&Base,VID)->u64;

impl Base {

  fn new(bits:Vec<Op>, tags:HashMap<String, NID>)->Base {
    Base{bits: bits,
         tags: tags,
         hash: HashMap::new(),
         vars: vec![],
         subs: vec![],
         subc: vec![]}}

  fn empty()->Base { Base::new(vec![Op::O, Op::I], HashMap::new()) }

  fn len(&self)->usize { self.bits.len() }

  /// given a function that maps input bits to 64-bit masks, color each node
  /// in the base according to its inputs (thus tracking the spread of influence
  /// of up to 64 bits (or groups of bits).
  ///
  /// while we're at it, calculate the cost of each bit, where constants have cost 0,
  /// inputs have a cost of 1, and everything else is 1 + max(cost of input bits)
  /// (TOOD: break masks_and_costs into two functions)
  pub fn masks_and_costs(&self, vm:VarMaskFn)->(Vec<u64>, Vec<u32>) {
    use std::cmp::max;
    let mut masks = vec![];
    let mut costs = vec![];
    for (i,&bit) in self.bits.iter().enumerate() {
      let (mask, cost) = {
        let mc = |x,y| {
          let m = masks[x] | masks[y];
          (m, if m < 32 { 1 } else { max(costs[x], costs[y]) + 1 })};
        match bit {
          Op::I | Op::O => (0, 0),
          Op::Var(v)    => (vm(self, v), 1),
          Op::Not(x)    => mc(x,0),
          Op::And(x,y)  => mc(x,y),
          Op::Xor(x,y)  => mc(x,y),
          Op::Or (x,y)  => mc(x,y),
          _ => { println!("TODO: cost({}: {:?})", i, bit); (!0, 0) }}};
      masks.push(mask);
      costs.push(cost)}
    (masks, costs)}

  /// this returns a raggod 2d vector of direct references for each bit in the base
  pub fn reftable(&self) -> Vec<Vec<NID>> {
    let bits = &self.bits;
    let mut res:Vec<Vec<NID>> = vec![vec![]; bits.len()];
    for n in 0..bits.len() {
      let mut f = |x:NID| res[x].push(n);
      match bits[n] {
        Op::O | Op::I | Op::Var(_) => {}
        Op::Not(x)    => { f(x); }
        Op::And(x,y)  => { f(x); f(y); }
        Op::Xor(x,y)  => { f(x); f(y); }
        Op::Or(x,y)   => { f(x); f(y); }
        Op::Ch(x,y,z) => { f(x); f(y); f(z); }
        Op::Mj(x,y,z) => { f(x); f(y); f(z); } } }
    res }

  /// this is part of the garbage collection system. keep is the top level nid to keep.
  /// seen gets marked true for every nid that is a dependency of keep.
  fn markdeps(&self, keep:NID, seen:&mut Vec<bool>) {
    if !seen[keep] {
      seen[keep] = true;
      let mut f = |x:&NID| { self.markdeps(*x, seen) };
      match &self.bits[keep] {
        Op::O | Op::I | Op::Var(_) => { }
        Op::Not(x)    => { f(x); }
        Op::And(x,y)  => { f(x); f(y); }
        Op::Xor(x,y)  => { f(x); f(y); }
        Op::Or(x,y)   => { f(x); f(y); }
        Op::Ch(x,y,z) => { f(x); f(y); f(z); }
        Op::Mj(x,y,z) => { f(x); f(y); f(z); } } } }

  /// Construct a copy of the base, with the selected nodes in the given order.
  /// The nodes will be re-numbered according to their position in the vector.
  /// NOTE: if the `oldnids` parameter is not a proper permutation vector, it is
  /// possible to create an invalid Base. In particular, if the vector is shorter
  /// than the number of bits in self, then unaccounted-for bits will be discarded
  /// in the result. This is intentional, as this function is used by the garbage
  /// collector, but if a node whose nid is in `oldnids` references a node that
  /// is not in `oldnids`, the resulting generated node will reference GONE (2^64).
  pub fn permute(&self, oldnids:Vec<NID>)->Base {
    let newnid = {
      let mut result = vec![GONE; self.bits.len()];
      for (i,&n) in oldnids.iter().enumerate() { result[n] = i; }
      result };
    let newbits = oldnids.iter().map(|&old| {
      match self.bits[old] {
        Op::O | Op::I | Op::Var(_) => self.bits[old], // nid might change, but vid won't.
        Op::Not(x)    => Op::Not(newnid[x]),
        Op::And(x,y)  => Op::And(newnid[x], newnid[y]),
        Op::Xor(x,y)  => Op::Xor(newnid[x], newnid[y]),
        Op::Or(x,y)   => Op::Or(newnid[x], newnid[y]),
        Op::Ch(x,y,z) => Op::Ch(newnid[x], newnid[y], newnid[z]),
        Op::Mj(x,y,z) => Op::Mj(newnid[x], newnid[y], newnid[z]) }})
      .collect();
    let mut newtags = HashMap::new();
    for (key, &val) in &self.tags {
      if newnid[val] != GONE { newtags.insert(key.clone(), newnid[val]); }}

    Base::new(newbits, newtags) }

  /// Construct a new Base with only the nodes necessary to define the given nodes.
  /// The relative order of the bits is preserved.
  pub fn repack(&self, keep:Vec<NID>) -> (Base, Vec<NID>) {

    // garbage collection: mark dependencies of the bits we want to keep
    let mut deps = vec!(false;self.bits.len());
    for &nid in keep.iter() { self.markdeps(nid, &mut deps) }

    let mut newnids = vec![GONE; self.bits.len()];
    let mut oldnids:Vec<usize> = vec![];
    for (i, bit) in self.bits.iter().enumerate() {
      if deps[i] { newnids[i]=oldnids.len(); oldnids.push(i as usize); }}

    return (self.permute(oldnids), keep.iter().map(|&i| newnids[i]).collect()); }

} // end impl Base

impl Index<NID> for Base {
  type Output = Op;
  fn index(&self, index:NID) -> &Self::Output { &self.bits[index] } }

impl std::fmt::Debug for Base {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f,"Base[{}]", self.bits.len()) } }

#[test]
fn base_basics(){
  let mut b = Base::empty();
  assert_eq!(b.len(), 2);

  // constants
  assert!(0==b.o(), "o");      assert!(1==b.i(), "i");

  // not
  assert!(1==b.not(0), "¬o");  assert!(0==b.not(1), "¬i");

  // and
  assert!(0==b.and(0,0),"o∧o");  assert!(0==b.and(1,0), "i∧o");
  assert!(0==b.and(0,1),"o∧i");  assert!(1==b.and(1,1), "i∧i");

  // xor
  assert!(0==b.xor(0,0),"o≠o");  assert!(1==b.xor(1,0), "i≠o");
  assert!(1==b.xor(0,1),"o≠i");  assert!(0==b.xor(1,1), "i≠i"); }

#[test]
fn base_vars(){
  let mut b = Base::empty(); let n = b.len();
  let x0 = b.var(0); let x02 = b.var(0); let x1 = b.var(1);
  assert!(x0 == n, "var() should create a node. expected {}, got {}", n, x0);
  assert!(x0 == x02, "var(0) should always return the same nid.");
  assert!(x1 == x0+1);
  let nx0 = b.not(x0);
  assert!(x0 == b.not(nx0), "expected x0=¬¬x0");
  assert!(nx0 == b.nid(Op::Not(x0))) }

#[test]
fn base_when(){
  let mut b = Base::empty(); let x0 = b.var(0);
  assert!(b[x0] == Op::Var(0), "expect var(0) to return nid for Var(0)");
  assert!(b.when(0,0,x0)==  0, "x0 when x0 == 0 should be O");
  assert!(b.when(0,1,x0)==  1, "x0 when x0 == 1 should be I");
  assert!(b.when(1,1,x0)== x0, "x0 when x1 == 1 should be x0");
}


fn order<T:PartialOrd>(x:T, y:T)->(T,T) { if x < y { (x,y) } else { (y,x) }}
fn order3<T:Ord+Clone>(x:T, y:T, z:T)->(T,T,T) {
  let mut res = [x,y,z];
  res.sort();
  (res[0].clone(), res[1].clone(), res[2].clone())}

impl TBase for Base {

  fn o(&self)->NID { 0 }
  fn i(&self)->NID { 1 }

  fn var(&mut self, v:VID)->NID {
    let bits = &mut self.bits;
    let vars = &mut self.vars;
    let known = vars.len();
    if v >= known {
      for i in known .. v+1 {
        vars.push(bits.len());
        bits.push(Op::Var(i)) }}
    vars[v] }

  fn def(&mut self, s:String, i:u32)->NID {
    let next = self.vars.len();
    let nid = self.var(next);
    self.tag(nid, format!("{}{}", s, i).to_string()) }

  fn tag(&mut self, n:NID, s:String)->NID {
    self.tags.insert(s, n); n }

  fn not(&mut self, x:NID)->NID {
    match self[x] {
      Op::O => self.i(),
      Op::I => self.o(),
      Op::Not(n) => n,
      _ => self.nid(Op::Not(x)) } }




  fn and(&mut self, x:NID, y:NID)->NID {
    if x == y { x }
    else { let (lo,hi) = order(x, y);
           match order(self[x], self[y]) {
             (Op::O,_) => self.o(),
             (Op::I,_) => hi,
             (Op::Not(n),_) if n==hi => self.o(),
             (_,Op::Not(n)) if n==lo => self.o(),
             _ => self.nid(Op::And(lo,hi)) }}}

  fn xor(&mut self, x:NID, y:NID)->NID {
    if x == y { self.o() }
    else { let (lo,hi) = order(x, y);
           match (self[lo], self[hi]) {
             (Op::O, _) => hi,
             (Op::I, _) => self.not(hi),
             (Op::Var(v), Op::Not(n)) if n==lo => self.i(),
             _ => self.nid(Op::Xor(lo,hi)) }}}

  fn or(&mut self, x:NID, y:NID)->NID {
    if x == y { x }
    else { let (lo,hi) = order(x, y);
           match (self[lo], self[hi]) {
             (Op::O, _) => hi,
             (Op::I, _) => self.i(),
             (Op::Var(v), Op::Not(n)) if n==lo => self.i(),
             (Op::Not(m), Op::Not(n)) => {
               let a = self.and(m,n); self.not(a)},
             _ => self.nid(Op::Or(lo,hi)) }}}



  fn mj(&mut self, x:NID, y:NID, z:NID)->NID {
    let (a,b,c) = order3(x,y,z);
    self.nid(Op::Mj(x,y,z)) }

  fn ch(&mut self, x:NID, y:NID, z:NID)->NID { self.o() }


  fn sid(&mut self, kv:SUB)->SID {
    let res = self.subs.len();
    self.subs.push(kv); self.subc.push(HashMap::new());
    res }

  fn sub(&mut self, x:NID, s:SID)->NID {
    macro_rules! op {
      [not $x:ident] => {{ let x1 = self.sub($x, s); self.not(x1) }};
      [$f:ident $x:ident $y:ident] => {{
        let x1 = self.sub($x, s);
        let y1 = self.sub($y, s);
        self.$f(x1,y1) }}}
    match self.subc[s].get(&x) {
      Some(&n) => n,
      None => {
        let n = match self[x] {
          Op::O | Op::I => x,
          Op::Var(v) => match self.subs[s].get(&v) {
            Some(&y) => y,
            None => x },
          Op::Not(a) => op![not a],
          Op::And(a,b) => op![and a b],
          Op::Xor(a,b) => op![xor a b],
          other => { panic!("huh?! sub({:?},{})", other, s) }};
        self.subc[s].insert(x, n);
        n }}}


  fn when(&mut self, v:VID, val:NID, nid:NID)->NID {
    // print!(":{}",nid);
    macro_rules! op {
      [not $x:ident] => {{ let x1 = self.when(v, val, $x); self.not(x1) }};
      [$f:ident $x:ident $y:ident] => {{
        let x1 = self.when(v, val, $x);
        let y1 = self.when(v, val, $y);
        self.$f(x1,y1) }}}
    if v >= self.vars.len() { nid }
    else { match self[nid] {
      Op::Var(x) if x==v => val,
      Op::O | Op::I | Op::Var(_) => nid,
      Op::Not(x)    => op![not x],
      Op::And(x, y) => op![and x y],
      Op::Xor(x, y) => op![xor x y],
      other => { println!("unhandled match: {:?}", other); nid }}}}

  fn nid(&mut self, op:Op)->NID {
    match self.hash.get(&op) {
      Some(&n) => n,
      None => {
        let n = self.bits.len();
        self.bits.push(op);
        self.hash.insert(op, n);
        n }}}

} // impl TBase for Base

