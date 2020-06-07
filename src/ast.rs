// a concrete implemetation:
use std::collections::{HashMap,HashSet};
use std::fs::File;
use std::io::Write;
use std::ops::Index;
use std::process::Command;      // for creating and viewing digarams

use io;
use base::*;
use nid;
pub use nid::{VID};
// pub type VID = usize;
pub type NID = usize;
const GONE:usize = 1<<63;
//pub const GONE:NID = NID{ n:1<<59 >> } // only used in ast.

// temporary scaffolding while I replace usize with nid::NID
type Old = usize;
type New = nid::NID;
const NOVAR:usize = 1<<31;
fn no1(old:Old)->New {
  if old == 0 { nid::O }
  else if old ==1 { nid::I }
  else { nid::nvi(NOVAR, old as u32) }}
fn on1(new:New)->Old {
  if new == nid::O { 0 }
  else if new == nid::I { 1 }
  else { nid::idx(new) as Old }}
fn on(old:Old)->Old{ old }
fn no(old:Old)->Old{ old }



pub type SID = usize; // canned substition
type SUB = HashMap<VID,Old>;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Op {
  O, I, Var(VID), Not(Old), And(Old,Old), Or(Old,Old), Xor(Old,Old),
  // Eql(Old,Old), LT(Nid,Nid),
  Ch(Old, Old, Old), Mj(Old, Old, Old) }

// !! TODO: move subs/subc into external structure
#[derive(Serialize, Deserialize)]
pub struct ASTBase {
  pub bits: Vec<Op>,                // all known bits (simplified)     TODO: make private
  nvars: usize,
  tags: HashMap<String, Old>,       // support for naming/tagging bits.  TODO: make private
  hash: HashMap<Op, Old>,           // expression cache (simple+complex)
  vars: Vec<Old>,                   // quick index of Var(n) in bits
  subs: Vec<SUB>,                   // list of substitution dicts
  subc: Vec<HashMap<Old,Old>>       // cache of substiution results
}

type VarMaskFn = fn(&ASTBase,VID)->u64;

impl ASTBase {


  pub fn new(bits:Vec<Op>, tags:HashMap<String, Old>, nvars:usize)->ASTBase {
    ASTBase{bits, nvars, tags,
            hash: HashMap::new(),
            vars: vec![],
            subs: vec![],
            subc: vec![]}}

  pub fn empty()->ASTBase { ASTBase::new(vec![Op::O, Op::I], HashMap::new(), 0) }

  fn nid(&mut self, op:Op)->Old {
    match self.hash.get(&op) {
      Some(&n) => n,
      None => {
        let n = self.bits.len();
        self.bits.push(op);
        self.hash.insert(op, n);
        n }}}

  // TODO: extract a Trait? These are almost exactly the same in bdd.rs
  pub fn save(&self, path:&str)->::std::io::Result<()> {
    let s = bincode::serialize(&self).unwrap();
    io::put(path, &s) }

  pub fn load(path:&str)->::std::io::Result<ASTBase> {
    let s = io::get(path)?;
    Ok(bincode::deserialize(&s).unwrap()) }


/*  fn sid(&mut self, kv:SUB)->SID {
    let res = self.subs.len();
    self.subs.push(kv); self.subc.push(HashMap::new());
    res } */

  pub fn sub(&mut self, x:Old, s:SID)->Old {
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


  pub fn when(&mut self, v:VID, val:Old, nid:Old)->Old {
    // print!(":{}",nid);
    macro_rules! op {
      [not $x:ident] => {{ let x1 = self.when(v, val, $x); self.not(x1) }};
      [$f:ident $x:ident $y:ident] => {{
        let x1 = self.when(v, val, $x);
        let y1 = self.when(v, val, $y);
        self.$f(x1,y1) }}}
    if (v as usize) >= self.vars.len() { nid }
    else { match self[nid] {
      Op::Var(x) if x==v => val,
      Op::O | Op::I | Op::Var(_) => nid,
      Op::Not(x)    => op![not x],
      Op::And(x, y) => op![and x y],
      Op::Xor(x, y) => op![xor x y],
      other => { println!("unhandled match: {:?}", other); nid }}}}



  pub fn walk<F>(&self, n:Old, f:&mut F) where F: FnMut(Old) {
    let mut seen = HashSet::new();
    self.step(n,f,&mut seen)}

  fn step<F>(&self, n:Old, f:&mut F, seen:&mut HashSet<Old>) where F:FnMut(Old) {
    if !seen.contains(&n) {
      seen.insert(n);
      f(n);
      let mut s = |x| self.step(x, f, seen);
      match self.bits[n] {
        Op::O | Op::I | Op::Var(_) => {  } // we already called f(n) so nothing to do
        Op::Not(x)    => { s(x); }
        Op::And(x,y)  => { s(x); s(y); }
        Op::Xor(x,y)  => { s(x); s(y); }
        Op::Or(x,y)   => { s(x); s(y); }
        Op::Ch(x,y,z) => { s(x); s(y); s(z); }
        Op::Mj(x,y,z) => { s(x); s(y); s(z); } }}}


  // generate dot file (graphviz)
  pub fn dot<T>(&self, n:Old, wr: &mut T) where T : ::std::fmt::Write {
    macro_rules! w {
      ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}
    macro_rules! dotop {
      ($s:expr, $n:expr $(,$xs:expr)*) => {{
        w!("  {}[label={}];", $n, $s); // draw the node
        $( w!(" {}->{};", $xs, $n); )* }}}  // draw the edges leading into it

    w!("digraph bdd {{");
    w!("rankdir=BT;"); // put root on top
    w!("node[shape=circle];");
    w!("edge[style=solid];");
    self.walk(n, &mut |n| {
      match &self.bits[n] {
        Op::O => w!(" {}[label=⊥];", n),
        Op::I => w!(" {}[label=⊤];", n),
        Op::Var(x)  => w!("{}[label=\"${}\"];", n, x),
        Op::And(x,y) => dotop!("∧",n,x,y),
        Op::Xor(x,y) => dotop!("≠",n,x,y),
        Op::Or(x,y)  => dotop!("∨",n,x,y),
        Op::Not(x)  => dotop!("¬",n,x),
        _ => w!("  \"{}\"[label={}];", n, n) }});
    w!("}}"); }

  pub fn save_dot(&self, n:Old, path:&str) { // !! taken from bdd.rs
    let mut s = String::new(); self.dot(n, &mut s);
    let mut txt = File::create(path).expect("couldn't create dot file");
    txt.write_all(s.as_bytes()).expect("failet to write text to dot file"); }


  pub fn show_named(&self, n:Old, s:&str) {   // !! almost exactly the same as in bdd.rs
    self.save_dot(n, format!("{}.dot", s).as_str());
    let out = Command::new("dot").args(&["-Tpng",format!("{}.dot",s).as_str()])
      .output().expect("failed to run 'dot' command");
    let mut png = File::create(format!("{}.png",s).as_str()).expect("couldn't create png");
    png.write_all(&out.stdout).expect("couldn't write png");
    Command::new("firefox").args(&[format!("{}.png",s).as_str()])
      .spawn().expect("failed to launch firefox"); }

  pub fn show(&self, n:Old) { self.show_named(n, "+ast+") }


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
  pub fn reftable(&self) -> Vec<Vec<Old>> {
    let bits = &self.bits;
    let mut res:Vec<Vec<Old>> = vec![vec![]; bits.len()];
    for (n, &bit) in bits.iter().enumerate() {
      let mut f = |x:Old| res[x].push(n);
      match bit {
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
  fn markdeps(&self, keep:Old, seen:&mut Vec<bool>) {
    if !seen[keep] {
      seen[keep] = true;
      let mut f = |x:&Old| { self.markdeps(*x, seen) };
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
  pub fn permute(&self, oldnids:&[Old])->ASTBase {
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

    ASTBase::new(newbits, newtags, self.nvars) }

  /// Construct a new ASTBase with only the nodes necessary to define the given nodes.
  /// The relative order of the bits is preserved.
  pub fn repack(&self, keep:Vec<Old>) -> (ASTBase, Vec<Old>) {

    // garbage collection: mark dependencies of the bits we want to keep
    let mut deps = vec!(false;self.bits.len());
    for &nid in keep.iter() { self.markdeps(nid, &mut deps) }

    let mut newnids = vec![GONE; self.bits.len()];
    let mut oldnids:Vec<usize> = vec![];
    for i in 0..self.bits.len() {
      if deps[i] { newnids[i]=oldnids.len(); oldnids.push(i as usize); }}

    (self.permute(&oldnids), keep.iter().map(|&i| newnids[i]).collect()) }

} // impl ASTBase

impl Index<Old> for ASTBase {
  type Output = Op;
  fn index(&self, index:Old) -> &Self::Output { &self.bits[index] } }

impl ::std::fmt::Debug for ASTBase {
  fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
    write!(f,"ASTBase[{}]", self.bits.len()) } }

impl Base for ASTBase {

  type N = Old;

  fn new(n:usize)->Self {
    let mut res = ASTBase::empty();
    for i in 0..n { println!("var({})",i); res.var(i as VID); }
    res }
  fn num_vars(&self)->usize { self.nvars }

  fn o(&self)->Old { on(no(0)) }
  fn i(&self)->Old { on(no(1)) }

  fn var(&mut self, v:VID)->Old {
    let bits = &mut self.bits;
    let vars = &mut self.vars;
    let known = self.nvars;
    if v >= known {
      for i in known ..= v {
        self.nvars += 1;
        vars.push(bits.len());
        bits.push(Op::Var(i as usize)) }}
    on(vars[v as usize]) }

  fn when_hi(&mut self, v:VID, n:Old)->Old { on(self.when(v,no(1),no(n))) }
  fn when_lo(&mut self, v:VID, n:Old)->Old { on(self.when(v,no(0),no(n))) }

  fn def(&mut self, s:String, i:VID)->Old {
    let next = self.vars.len() as VID;
    let nid = no(self.var(next));
    on(self.tag(nid, format!("{}{}", s, i))) }

  fn tag(&mut self, n:Old, s:String)->Old {
    let n = no(n);
    self.tags.insert(s, n); on(n) }

  fn not(&mut self, x:Old)->Old {
    match self[x] {
      Op::O => self.i(),
      Op::I => self.o(),
      Op::Not(n) => n,
      _ => on(self.nid(Op::Not(x))) } }



  fn and(&mut self, x:Old, y:Old)->Old {
    let (x,y) = (no(x),no(y));
    if x == y { x }
    else {
      let (lo,hi) = if self[x] < self[y] { (x,y) } else { (y,x) };
      match (self[lo], self[hi]) {
        (Op::O,_) => self.o(),
        (Op::I,_) => hi,
        (Op::Not(n),_) if n==hi => self.o(),
        (_,Op::Not(n)) if n==lo => self.o(),
        _ => self.nid(Op::And(lo,hi)) }}}

  fn xor(&mut self, x:Old, y:Old)->Old {
    let (x,y) = (no(x),no(y));
    if x == y { self.o() }
    else {
      let (lo,hi) = if self[x] < self[y] { (x,y) } else { (y,x) };
      match (self[lo], self[hi]) {
        (Op::O, _) => hi,
        (Op::I, _) => self.not(hi),
        (Op::Var(_), Op::Not(n)) if n==lo => self.i(),
        _ => self.nid(Op::Xor(lo,hi)) }}}

  fn or(&mut self, x:Old, y:Old)->Old {
    let (x,y) = (no(x),no(y));
    if x == y { x }
    else {
      let (lo,hi) = if self[x] < self[y] { (x,y) } else { (y,x) };
      match (self[lo], self[hi]) {
        (Op::O, _) => hi,
        (Op::I, _) => self.i(),
        (Op::Var(_), Op::Not(n)) if n==lo => self.i(),
        (Op::Not(m), Op::Not(n)) => {
          let a = self.and(m,n); self.not(a)},
        _ => self.nid(Op::Or(lo,hi)) }}}



  #[cfg(todo)]
  fn mj(&mut self, x:Old, y:Old, z:Old)->Old {
    let (a,b,c) = order3(x,y,z);
    self.nid(Op::Mj(x,y,z)) }

  #[cfg(todo)]
  fn ch(&mut self, x:Old, y:Old, z:Old)->Old { self.o() }


  fn sub(&mut self, _v:VID, _n:Old, _ctx:Old)->Old { todo!("ast::sub") }

  fn get(&self, s:&str)->Option<Old> { Some(*self.tags.get(s)?) }
  fn save(&self, _path:&str)->::std::io::Result<()> { todo!("ast::save") }
  fn save_dot(&self, _n:Old, _path:&str) { todo!("ast::save_dot") }
  fn show_named(&self, _n:Old, _path:&str) { todo!("ast::show_named") }

  fn solutions(&self)->&dyn Iterator<Item=Vec<bool>> { todo!("ast::solutions") }
} // impl Base for ASTBase


test_base_consts!(ASTBase);
test_base_vars!(ASTBase);
test_base_when!(ASTBase);

#[test]
fn ast_vars(){
  let mut b = ASTBase::empty();
  let x0 = b.var(0); let x1 = b.var(1);
  assert!(x1 == x0+1);
  let nx0 = b.not(x0);
  assert!(nx0 == b.nid(Op::Not(x0))) }

