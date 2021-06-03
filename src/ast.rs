// a concrete implemetation:
use std::collections::{HashMap,HashSet};

use io;
use base::*;
use {nid, nid::NID};
use {vid, vid::VID};
use {ops, ops::Ops};


#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord, Serialize, Deserialize)]
enum Op {
  And(NID,NID), Or(NID,NID), Xor(NID,NID),
  Ch(NID, NID, NID), Mj(NID, NID, NID) }

// !! TODO: move subs/subc into external structure
#[derive(Serialize, Deserialize)]
pub struct RawASTBase {
  bits: Vec<Ops>,                   // all known bits (simplified)
  nvars: usize,
  tags: HashMap<String, NID>,       // support for naming/tagging bits.
  hash: HashMap<Ops, NID>,          // expression cache (simple+complex)
}

type VarMaskFn = fn(&RawASTBase,vid::VID)->u64;

/// An ASTBase that does not use extra simplification rules.
impl RawASTBase {

  pub fn empty()->RawASTBase { RawASTBase{ bits:vec![], nvars:0, tags:HashMap::new(), hash:HashMap::new() }}
  pub fn len(&self)->usize { self.bits.len() }
  pub fn is_empty(&self)->bool { self.bits.is_empty() }

  fn nid(&mut self, ops:Ops)->NID {
    match self.hash.get(&ops) {
      Some(&n) => n,
      None => {
        let nid = nid::ixn(self.bits.len() as u32);
        self.bits.push(ops.clone());
        self.hash.insert(ops, nid);
        nid }}}

  pub fn load(path:&str)->::std::io::Result<RawASTBase> {
    let s = io::get(path)?;
    Ok(bincode::deserialize(&s).unwrap()) }


  fn when(&mut self, v:vid::VID, val:NID, nid:NID)->NID {
    // if var is outside the base, it can't affect the expression
    if v.vid_ix() >= self.num_vars() { nid }
    else if nid.is_vid() && nid.vid() == v { val }
    else if nid.is_lit() { nid }
    else {
      let ops = self.get_ops(nid).clone();
      let rpn:Vec<NID> = ops.to_rpn().map(|&nid|{
        if nid.is_fun() { nid }
        else { self.when(v, val, nid) }}).collect();
      self.nid(ops::rpn(&rpn)) }}



  fn walk<F>(&self, n:NID, f:&mut F) where F: FnMut(NID) {
    let mut seen = HashSet::new();
    self.step(n,f,&mut seen)}

  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>) where F:FnMut(NID) {
    if !seen.contains(&nid::raw(n)) {
      seen.insert(nid::raw(n));
      f(n);
      let mut s = |x| self.step(x, f, seen);
      match self.old_op(n) {
        Op::And(x,y)  => { s(x); s(y); }
        Op::Xor(x,y)  => { s(x); s(y); }
        Op::Or(x,y)   => { s(x); s(y); }
        Op::Ch(x,y,z) => { s(x); s(y); s(z); }
        Op::Mj(x,y,z) => { s(x); s(y); s(z); }
        other => panic!("unexpected op: {:?}", other) }}}

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
    for i in  0..self.bits.len() {
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
        let bit = self.old_op(nid::ixn(i as u32));
        match bit {
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
      match &self.old_op(keep) {
        Op::And(x,y)  => { f(x); f(y); }
        Op::Xor(x,y)  => { f(x); f(y); }
        Op::Or(x,y)   => { f(x); f(y); }
        Op::Ch(x,y,z) => { f(x); f(y); f(z); }
        Op::Mj(x,y,z) => { f(x); f(y); f(z); }
       other => panic!("bad op in markdeps: {:?}", other) } } }

  /// Construct a copy of the base, with the nodes reordered according to
  /// permutation vector pv. That is, pv is a vector of unique node indices
  /// that we want to keep, in the order we want them. (It might actually be
  /// shorter than bits.len() and thus not technically a permutation vector,
  /// but I don't have a better name for this concept.)
  pub fn permute(&self, pv:&[usize])->RawASTBase {
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
      match self.old_op(nid::ixn(old as u32)) {
        Op::And(x,y)  => ops::and(nn(x), nn(y)),
        Op::Xor(x,y)  => ops::xor(nn(x), nn(y)),
        Op::Or(x,y)   => ops::vel(nn(x), nn(y)),
        // Op::Ch(x,y,z) => Op::Ch(nn(x), nn(y), nn(z)),
        // Op::Mj(x,y,z) => Op::Mj(nn(x), nn(y), nn(z)),
       other => panic!("permute op: {:?}", other) }})
      .collect();
    let mut newtags = HashMap::new();
    for (key, &val) in &self.tags { newtags.insert(key.clone(), nn(val)); }
    RawASTBase{ bits:newbits, tags:newtags, nvars: self.nvars, hash:HashMap::new() }}

  /// Construct a new RawASTBase with only the nodes necessary to define the given nodes.
  /// The relative order of the bits is preserved.
  pub fn repack(&self, keep:Vec<NID>) -> (RawASTBase, Vec<NID>) {
    // garbage collection: mark dependencies of the bits we want to keep
    let mut deps = vec!(false;self.bits.len());
    for &nid in keep.iter() { self.markdeps(nid, &mut deps) }

    let mut new:Vec<Option<usize>> = vec![None; self.bits.len()];
    let mut old:Vec<usize> = vec![];
    for i in 0..self.bits.len() {
      if deps[i] { new[i]=Some(old.len()); old.push(i); }}

    (self.permute(&old), keep.iter().map(|&i|
      nid::ixn(new[nid::idx(i) as usize].expect("?!") as u32)).collect()) }

  pub fn get_ops(&self, n:NID)->&Ops {
    if nid::no_var(n) { &self.bits[nid::idx(n)] } else { panic!("don't know how to op({:?})", n) }}

  fn old_op(&self, nid:NID)->Op {
    let ops::Ops::RPN(rpn) = self.get_ops(nid);
    let &fun = rpn.last().unwrap();
    assert!(fun.is_fun());
    match fun {
      ops::AND => Op::And(rpn[0], rpn[1]),
      ops::XOR => Op::Xor(rpn[0], rpn[1]),
      ops::VEL => Op::Or(rpn[0], rpn[1]),
      _ => panic!("get_op -> fun = {:?}", fun) }}

} // impl RawASTBase

impl ::std::fmt::Debug for RawASTBase {
  fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
    write!(f,"RawASTBase[{}]", self.bits.len()) } }

impl Base for RawASTBase {

  fn new(n:usize)->Self {
    let mut res = RawASTBase::empty();
    res.nvars = n;
    res }
  fn num_vars(&self)->usize { self.nvars }

  fn when_hi(&mut self, v:vid::VID, n:NID)->NID { self.when(v, nid::I, n) }
  fn when_lo(&mut self, v:vid::VID, n:NID)->NID { self.when(v, nid::O, n) }

  fn def(&mut self, s:String, i:vid::VID)->NID {
    let next = self.num_vars() as u32;
    let nid = NID::var(next);
    self.nvars += 1;
    self.tag(nid, format!("{}{:?}", s, i)) }

  fn tag(&mut self, n:NID, s:String)->NID {
    let n = n;
    self.tags.insert(s, n); n }

  fn and(&mut self, x:NID, y:NID)->NID {
    match (x, y) {
      (nid::O, _) => nid::O,
      (_, nid::O) => nid::O,
      (nid::I, y) => y,
      (x, nid::I) => x,
      _ if x == y => x,
      _ if x == !y => nid::O,
      _ => { let (lo, hi) = if x<y {(x,y)} else {(y,x)};  self.nid(ops::and(lo, hi)) }}}

  fn xor(&mut self, x:NID, y:NID)->NID {
    match (x, y) {
      (nid::O, y) => y,
      (x, nid::O) => x,
      (nid::I, y) => !y,
      (x, nid::I) => !x,
      _ if x == y => nid::O,
      _ if x == !y => nid::I,
      _ => { let (lo, hi) = if x<y {(x,y)} else {(y,x)};  self.nid(ops::xor(lo, hi)) }}}

  fn or(&mut self, x:NID, y:NID)->NID {
    match (x, y) {
      (nid::O, y) => y,
      (x, nid::O) => x,
      (nid::I, _) => nid::I,
      (_, nid::I) => nid::I,
      _ if x == y => x,
      _ if x == !y => nid::I,
      _ if x.is_inv() && y.is_inv() => !self.and(x, y),
      _ => { let (lo, hi) = if x<y {(x,y)} else {(y,x)};  self.nid(ops::vel(lo, hi)) }}}

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
      match n {
        nid::O => w!(" \"{}\"[label=⊥];", n),
        nid::I => w!(" \"{}\"[label=⊤];", n),
        _ if n.is_vid() => w!("\"{}\"[label=\"{}\"];", nid::raw(n), n.vid()),
        _ => match &self.old_op(n) {
          Op::And(x,y) => dotop!("∧",n,x,y),
          Op::Xor(x,y) => dotop!("≠",n,x,y),
          Op::Or(x,y)  => dotop!("∨",n,x,y),
          _ => panic!("unexpected op in dot(): {:?}", n) }}});
    w!("}}"); }
} // impl Base for RawASTBase

pub struct ASTBase { base: Simplify<RawASTBase> }
impl Base for ASTBase {
  inherit![num_vars, when_hi, when_lo, and, xor, or, def, tag, get, sub, save, dot ];
  fn new(n:usize)->Self { ASTBase{ base: Simplify{ base: <RawASTBase as Base>::new(n) }}}}

impl ASTBase {
  pub fn empty()->Self { ASTBase { base: Simplify{ base: RawASTBase::empty() }}}
  pub fn raw_ast(&self)->&RawASTBase { &self.base.base }}

test_base_consts!(ASTBase);
test_base_when!(ASTBase);

#[test]
fn ast_and(){
  let mut b = ASTBase::empty();
  let x0 = NID::var(0); let x1 = NID::var(1);
  let x01 = b.and(x0,x1);
  let x10 = b.and(x1,x0);
  assert_eq!(x01, x10, "expect $0 & $1 == $1 & $0"); }
