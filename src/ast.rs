// a concrete implemetation:
use std::collections::{HashMap,HashSet};
use std::cmp::Ordering;
use vid::VidOrdering;

use io;
use base::*;
use {nid, nid::NID};
use {vid, vid::VID};



pub type SID = usize; // canned substition
type SUB = HashMap<VID,NID>;


/// this is only so I can order ops. VID should otherwise always be
/// compared with is_above / iS_below or cmp_depth, for clarity.
impl Ord for VID {
  fn cmp(&self, other: &Self)-> Ordering {
    match self.cmp_depth(other) {
      VidOrdering::Above => Ordering::Less,
      VidOrdering::Level => Ordering::Equal,
      VidOrdering::Below => Ordering::Greater
    }
  }
}
impl PartialOrd for VID {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
      Some(self.cmp(other))
  }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Op {
  O, I, Var(VID), Not(NID), And(NID,NID), Or(NID,NID), Xor(NID,NID),
  // Eql(NID,NID), LT(NID,NID),
  Ch(NID, NID, NID), Mj(NID, NID, NID) }

// !! TODO: move subs/subc into external structure
#[derive(Serialize, Deserialize)]
pub struct ASTBase {
  bits: Vec<Op>,                    // all known bits (simplified)
  nvars: usize,
  tags: HashMap<String, NID>,       // support for naming/tagging bits.
  hash: HashMap<Op, NID>,           // expression cache (simple+complex)
  subs: Vec<SUB>,                   // list of substitution dicts
  subc: Vec<HashMap<NID,NID>>       // cache of substiution results
}

type VarMaskFn = fn(&ASTBase,vid::VID)->u64;

impl ASTBase {


  fn new(bits:Vec<Op>, tags:HashMap<String, NID>, nvars:usize)->ASTBase {
    ASTBase{bits, nvars, tags,
            hash: HashMap::new(),
            subs: vec![],
            subc: vec![]}}

  pub fn empty()->ASTBase { ASTBase::new(vec![], HashMap::new(), 0) }
  pub fn len(&self)->usize { self.bits.len() }
  pub fn is_empty(&self)->bool { self.bits.is_empty() }

  fn nid(&mut self, op:Op)->NID {
    if op == Op::O { nid::O }
    else if op == Op::I { nid::I }
    else if let Op::Var(x) = op { NID::from_vid(x) }
    else if let Op::Not(x) = op { !x }
    else { match self.hash.get(&op) {
      Some(&n) => n,
      None => {
        let nid = nid::ixn(self.bits.len() as u32);
        self.bits.push(op);
        self.hash.insert(op, nid);
        nid }}}}

  pub fn load(path:&str)->::std::io::Result<ASTBase> {
    let s = io::get(path)?;
    Ok(bincode::deserialize(&s).unwrap()) }


/*  fn sid(&mut self, kv:SUB)->SID {
    let res = self.subs.len();
    self.subs.push(kv); self.subc.push(HashMap::new());
    res } */

  pub fn sub(&mut self, x:NID, s:SID)->NID {
    macro_rules! op {
      [not $x:ident] => {{ let x1 = self.sub($x, s); !x1 }};
      [$f:ident $x:ident $y:ident] => {{
        let x1 = self.sub($x, s);
        let y1 = self.sub($y, s);
        self.$f(x1,y1) }}}
    match self.subc[s].get(&x) {
      Some(&n) => n,
      None => {
        let n = match self.op(x) {
          Op::O | Op::I => x,
          Op::Var(v) => {
            match self.subs[s].get(&v) {
              Some(&y) => y,
              None => x }},
          Op::Not(a) => op![not a],
          Op::And(a,b) => op![and a b],
          Op::Xor(a,b) => op![xor a b],
          other => { panic!("huh?! sub({:?},{})", other, s) }};
        self.subc[s].insert(x, n);
        n }}}


  fn when(&mut self, v:vid::VID, val:NID, nid:NID)->NID {
    // print!(":{}",nid);
    macro_rules! op {
      [not $x:ident] => {{ let x1 = self.when(v, val, $x); !x1 }};
      [$f:ident $x:ident $y:ident] => {{
        let x1 = self.when(v, val, $x);
        let y1 = self.when(v, val, $y);
        self.$f(x1,y1) }}}
    // if var is outside the base, it can't affect the expression
    if v.vid_ix() >= self.num_vars() { nid }
    else { match self.op(nid) {
      Op::Var(x) if x==v => val,
      Op::O | Op::I | Op::Var(_) => nid,
      Op::Not(x)    => op![not x],
      Op::And(x, y) => op![and x y],
      Op::Xor(x, y) => op![xor x y],
      other => { println!("unhandled match: {:?}", other); nid }}}}



  fn walk<F>(&self, n:NID, f:&mut F) where F: FnMut(NID) {
    let mut seen = HashSet::new();
    self.step(n,f,&mut seen)}

  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>) where F:FnMut(NID) {
    if !seen.contains(&nid::raw(n)) {
      seen.insert(nid::raw(n));
      f(n);
      let mut s = |x| self.step(x, f, seen);
      match self.op(n) {
        Op::O | Op::I | Op::Var(_) => {  } // we already called f(n) so nothing to do
        Op::Not(x)    => { s(x); }
        Op::And(x,y)  => { s(x); s(y); }
        Op::Xor(x,y)  => { s(x); s(y); }
        Op::Or(x,y)   => { s(x); s(y); }
        Op::Ch(x,y,z) => { s(x); s(y); s(z); }
        Op::Mj(x,y,z) => { s(x); s(y); s(z); } }}}

  pub fn show(&self, n:NID) { self.show_named(n, "+ast+") }


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
        let cost = |x:NID| {
          if nid::is_const(x) { 0 }
          else if nid::is_var(x) { 1 }
          else if nid::no_var(x) { costs[nid::idx(x)] }
          else { todo!("cost({:?})", x) }};
        let mask = |x:NID| {
          if nid::is_const(x) { 0 }
          else if nid::is_var(x) { vm(self, x.vid()) }
          else if nid::no_var(x) { masks[nid::idx(x)] }
          else { todo!("mask({:?})", x) }};
        let mc = |x,y| {
          let m = mask(x) | mask(y);
          (m, max(cost(x), cost(y)) + 1 )};
        match bit {
          Op::I | Op::O => (0, 0),
          Op::Var(v)    => (vm(self, v), 1),
          Op::Not(x)    => mc(x,nid::O),
          Op::And(x,y)  => mc(x,y),
          Op::Xor(x,y)  => mc(x,y),
          Op::Or (x,y)  => mc(x,y),
          _ => { println!("TODO: cost({}: {:?})", i, bit); (!0, 0) }}};
      masks.push(mask);
      costs.push(cost)}
    (masks, costs)}

  /// this returns a ragged 2d vector of direct references for each bit in the base
  pub fn reftable(&self) -> Vec<Vec<NID>> {
    todo!("test case for reftable!"); /*
    let bits = &self.bits;
    let mut res:Vec<Vec<NID>> = vec![vec![]; bits.len()];
    for (n, &bit) in bits.iter().enumerate() {
      let mut f = |x:NID| res[nid::idx(x)].push(n);
      match bit {
        Op::O | Op::I | Op::Var(_) => {}
        Op::Not(x)    => { f(x); }
        Op::And(x,y)  => { f(x); f(y); }
        Op::Xor(x,y)  => { f(x); f(y); }
        Op::Or(x,y)   => { f(x); f(y); }
        Op::Ch(x,y,z) => { f(x); f(y); f(z); }
        Op::Mj(x,y,z) => { f(x); f(y); f(z); } } }
    res*/ }

  /// this is part of the garbage collection system. keep is the top level nid to keep.
  /// seen gets marked true for every nid that is a dependency of keep.
  /// TODO:: use a HashSet for 'seen' in markdeps()
  fn markdeps(&self, keep:NID, seen:&mut Vec<bool>) {
    if nid::is_lit(keep) { return }
    if !nid::no_var(keep) { todo!("markdeps({:?})", keep) }
    if !seen[nid::idx(keep)] {
      seen[nid::idx(keep)] = true;
      let mut f = |x:&NID| { self.markdeps(*x, seen) };
      match &self.bits[nid::idx(keep)] {
        Op::O | Op::I | Op::Var(_) => { }
        Op::Not(x)    => { f(x); }
        Op::And(x,y)  => { f(x); f(y); }
        Op::Xor(x,y)  => { f(x); f(y); }
        Op::Or(x,y)   => { f(x); f(y); }
        Op::Ch(x,y,z) => { f(x); f(y); f(z); }
        Op::Mj(x,y,z) => { f(x); f(y); f(z); } } } }

  /// Construct a copy of the base, with the nodes reordered according to
  /// permutation vector pv. That is, pv is a vector of unique node indices
  /// that we want to keep, in the order we want them. (It might actually be
  /// shorter than bits.len() and thus not technically a permutation vector,
  /// but I don't have a better name for this concept.)
  pub fn permute(&self, pv:&[usize])->ASTBase {
    // map each kept node in self.bits to Some(new position)
    let new:Vec<Option<usize>> = {
      let mut res = vec![None; self.bits.len()];
      for (i,&n) in pv.iter().enumerate() { res[n] = Some(i) }
      res };
    let nn = |x:NID|{
      if nid::is_lit(x) { x }
      else {
        let r = nid::ixn(new[nid::idx(x) as usize].expect("bad index in AST::permute") as u32);
        if nid::is_inv(x) { !r } else { r }}};
    let newbits = pv.iter().map(|&old| {
      match self.bits[old] {
        Op::O | Op::I | Op::Var(_) | Op::Not(_) => panic!("o,i,var,not should never be in self.bits"),
        Op::And(x,y)  => Op::And(nn(x), nn(y)),
        Op::Xor(x,y)  => Op::Xor(nn(x), nn(y)),
        Op::Or(x,y)   => Op::Or(nn(x), nn(y)),
        Op::Ch(x,y,z) => Op::Ch(nn(x), nn(y), nn(z)),
        Op::Mj(x,y,z) => Op::Mj(nn(x), nn(y), nn(z)) }})
      .collect();
    let mut newtags = HashMap::new();
    for (key, &val) in &self.tags { newtags.insert(key.clone(), nn(val)); }
    ASTBase::new(newbits, newtags, self.nvars) }

  /// Construct a new ASTBase with only the nodes necessary to define the given nodes.
  /// The relative order of the bits is preserved.
  pub fn repack(&self, keep:Vec<NID>) -> (ASTBase, Vec<NID>) {
    // garbage collection: mark dependencies of the bits we want to keep
    let mut deps = vec!(false;self.bits.len());
    for &nid in keep.iter() { self.markdeps(nid, &mut deps) }

    let mut new:Vec<Option<usize>> = vec![None; self.bits.len()];
    let mut old:Vec<usize> = vec![];
    for i in 0..self.bits.len() {
      if deps[i] { new[i]=Some(old.len()); old.push(i); }}

    (self.permute(&old), keep.iter().map(|&i|
      nid::ixn(new[nid::idx(i) as usize].expect("?!") as u32)).collect()) }

  fn op(&self, n:NID)->Op {
    if n == nid::O { Op::O }
    else if n == nid::I { Op::I }
    else if nid::is_var(n) { Op::Var(n.vid()) }
    else if nid::no_var(n) { self.bits[nid::idx(n)] }
    else { panic!("don't know how to op({:?})", n) }}

  pub fn get_op(&self, nid:NID)->Op { self.op(nid) }

} // impl ASTBase

impl ::std::fmt::Debug for ASTBase {
  fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
    write!(f,"ASTBase[{}]", self.bits.len()) } }

impl Base for ASTBase {

  fn new(n:usize)->Self {
    let mut res = ASTBase::empty();
    for i in 0..n { println!("var({})",i); res.var(i as u32); }
    res }
  fn num_vars(&self)->usize { self.nvars }

  fn var(&mut self, v:u32)->NID {
    for _ in self.nvars as u32 ..= v { self.nvars += 1 }
    NID::var(v) }

  fn when_hi(&mut self, v:vid::VID, n:NID)->NID { self.when(v, nid::I, n) }
  fn when_lo(&mut self, v:vid::VID, n:NID)->NID { self.when(v, nid::O, n) }

  fn def(&mut self, s:String, i:vid::VID)->NID {
    let next = self.num_vars() as u32;
    let nid = self.var(next);
    self.tag(nid, format!("{}{:?}", s, i)) }

  fn tag(&mut self, n:NID, s:String)->NID {
    let n = n;
    self.tags.insert(s, n); n }

  fn and(&mut self, x:NID, y:NID)->NID {
    if x == y { x }
    else {
      let (lo,hi) = if self.op(x) < self.op(y) { (x,y) } else { (y,x) };
      match (self.op(lo), self.op(hi)) {
        (Op::O,_) => self.o(),
        (Op::I,_) => hi,
        (Op::Not(n),_) if n==hi => self.o(),
        (_,Op::Not(n)) if n==lo => self.o(),
        _ => self.nid(Op::And(lo,hi)) }}}

  fn xor(&mut self, x:NID, y:NID)->NID {
    if x == y { self.o() }
    else {
      let (lo,hi) = if self.op(x) < self.op(y) { (x,y) } else { (y,x) };
      match (self.op(lo), self.op(hi)) {
        (Op::O, _) => hi,
        (Op::I, _) => !hi,
        (Op::Var(_), Op::Not(n)) if n==lo => self.i(),
        _ => self.nid(Op::Xor(lo,hi)) }}}

  fn or(&mut self, x:NID, y:NID)->NID {
    if x == y { x }
    else {
      let (lo,hi) = if self.op(x) < self.op(y) { (x,y) } else { (y,x) };
      match (self.op(lo), self.op(hi)) {
        (Op::O, _) => hi,
        (Op::I, _) => self.i(),
        (Op::Var(_), Op::Not(n)) if n==lo => self.i(),
        (Op::Not(m), Op::Not(n)) => { !self.and(m,n) },
        _ => self.nid(Op::Or(lo,hi)) }}}

  #[cfg(todo)]
  fn mj(&mut self, x:NID, y:NID, z:NID)->NID {
    let (a,b,c) = order3(x,y,z);
    self.nid(Op::Mj(x,y,z)) }

  #[cfg(todo)]
  fn ch(&mut self, x:NID, y:NID, z:NID)->NID { self.o() }


  fn sub(&mut self, _v:vid::VID, _n:NID, _ctx:NID)->NID { todo!("ast::sub") }

  fn get(&self, s:&str)->Option<NID> { Some(*self.tags.get(s)?) }
  fn save(&self, path:&str)->::std::io::Result<()> {
    let s = bincode::serialize(&self).unwrap();
    io::put(path, &s) }

  // generate dot file (graphviz)
  fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write) {
    macro_rules! w {
      ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}
    macro_rules! dotop {
      ($s:expr, $n:expr $(,$xs:expr)*) => {{
        w!("  \"{}\"[label={}];", nid::raw($n), $s); // draw the node
        $({ if nid::is_inv(*$xs) { w!("edge[style=dashed];"); }
            else { w!("edge[style=solid];"); }
            w!(" \"{}\"->\"{}\";", nid::raw(*$xs), nid::raw($n)); })* }}}

    w!("digraph bdd {{");
    w!("rankdir=BT;"); // put root on top
    w!("node[shape=circle];");
    w!("edge[style=solid];");
    self.walk(n, &mut |n| {
      match &self.op(n) {
        Op::O => w!(" \"{}\"[label=⊥];", n),
        Op::I => w!(" \"{}\"[label=⊤];", n),
        Op::Var(x)  => w!("\"{}\"[label=\"{}\"];", nid::raw(n), x),
        Op::And(x,y) => dotop!("∧",n,x,y),
        Op::Xor(x,y) => dotop!("≠",n,x,y),
        Op::Or(x,y)  => dotop!("∨",n,x,y),
        _ => panic!("unexpected node: {:?}", n) }});
    w!("}}"); }
} // impl Base for ASTBase


test_base_consts!(ASTBase);
test_base_vars!(ASTBase);
test_base_when!(ASTBase);

#[test]
fn ast_vars(){
  let mut b = ASTBase::empty();
  let x0 = b.var(0); let x1 = b.var(1);
  assert_eq!(x0.vid().var_ix(), 0);
  assert_eq!(x1.vid().var_ix(), 1);
  assert_eq!(!x0, b.nid(Op::Not(x0))); }

#[test]
fn ast_and(){
  let mut b = ASTBase::empty();
  let x0 = b.var(0); let x1 = b.var(1);
  let x01 = b.and(x0,x1);
  let x10 = b.and(x1,x0);
  assert_eq!(x01, x10, "expect $0 & $1 == $1 & $0"); }
