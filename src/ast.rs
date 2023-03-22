//! Abstract syntax trees (simple logic combinators).

use std::collections::{HashMap,HashSet};

use base::*;
use {nid, nid::NID};
use {vid, vid::VID};
use {ops, ops::Ops};
use simp;


#[derive(Debug)]
pub struct RawASTBase {
  bits: Vec<Ops>,                   // all known bits (simplified)
  tags: HashMap<String, NID>,       // support for naming/tagging bits.
  hash: HashMap<Ops, NID>,          // expression cache (simple+complex)
}

type VarMaskFn = fn(&RawASTBase,vid::VID)->u64;

/// An ASTBase that does not use extra simplification rules.
impl RawASTBase {

  pub fn empty()->RawASTBase { RawASTBase{ bits:vec![], tags:HashMap::new(), hash:HashMap::new() }}
  pub fn len(&self)->usize { self.bits.len() }
  pub fn is_empty(&self)->bool { self.bits.is_empty() }

  fn nid(&mut self, ops:Ops)->NID {
    match self.hash.get(&ops) {
      Some(&n) => n,
      None => {
        let nid = NID::ixn(self.bits.len());
        self.bits.push(ops.clone());
        self.hash.insert(ops, nid);
        nid }}}


  fn when(&mut self, v:vid::VID, val:NID, nid:NID)->NID {
    if nid.is_vid() && nid.vid() == v { val }
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
    if !seen.contains(&n.raw()) {
      seen.insert(n.raw());
      f(n);
      if !n.is_lit() {
        for op in self.get_ops(n).to_rpn() {
          if !op.is_fun() {
            self.step(*op, f, seen) }}}}}

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
    for bit in self.bits.iter() {
      let (mask, cost) = {
        let cost = |x:NID| {
          if x.is_const() { 0 }
          else if x.is_vid() { 1 }
          else if x.is_ixn() { costs[x.idx()] }
          else { todo!("cost({:?})", x) }};
        let mask = |x:NID| {
          if x.is_const() { 0 }
          else if x.is_vid() { vm(self, x.vid()) }
          else if x.is_ixn() { masks[x.idx()] }
          else { todo!("mask({:?})", x) }};
        let mut m = 0u64;
        let mut c = 0u32;
        for &op in bit.to_rpn() {
          if ! op.is_fun() {
            m |= mask(op);
            c = max(c, cost(op)); }}
        (m, c+1) };
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
    if keep.is_lit() { return }
    if !keep.is_ixn() { todo!("markdeps({:?})", keep) }
    if !seen[keep.idx()] {
      seen[keep.idx()] = true;
      let mut f = |x:&NID| { self.markdeps(*x, seen) };
      for op in self.bits[keep.idx()].to_rpn() { if !op.is_fun() { f(op) }}}}


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
      if x.is_lit() { x }
      else {
        let r = NID::ixn(new[x.idx()].expect("bad index in AST::permute"));
        if x.is_inv() { !r } else { r }}};
    let newbits = pv.iter().map(|&old| {
      let new:Vec<NID> = self.bits[old].to_rpn().map(|&x| { if x.is_fun() { x } else { nn(x) }}).collect();
      ops::rpn(&new) }).collect();
    let mut newtags = HashMap::new();
    for (key, &val) in &self.tags { newtags.insert(key.clone(), nn(val)); }
    RawASTBase{ bits:newbits, tags:newtags, hash:HashMap::new() }}

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
      NID::ixn(new[i.idx()].expect("?!"))).collect()) }

  pub fn get_ops(&self, n:NID)->&Ops {
    if n.is_ixn() { &self.bits[n.idx()] } else { panic!("don't know how to op({:?})", n) }}
} // impl RawASTBase

impl Base for RawASTBase {

  fn new()->Self { RawASTBase::empty() }

  fn when_hi(&mut self, v:vid::VID, n:NID)->NID { self.when(v, nid::I, n) }
  fn when_lo(&mut self, v:vid::VID, n:NID)->NID { self.when(v, nid::O, n) }

  fn def(&mut self, s:String, v:vid::VID)->NID {
    let nid = NID::from_vid(v);
    self.tag(nid, format!("{}{:?}", s, v)) }

  fn tag(&mut self, n:NID, s:String)->NID {
    let n = n;
    self.tags.insert(s, n); n }

  fn and(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::and(x,y) { nid }
    else {
      let (lo, hi) = if x<y {(x,y)} else {(y,x)};
      self.nid(ops::and(lo, hi)) }}

  fn xor(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::xor(x,y) { nid }
    else {
      let (lo, hi) = if x<y {(x,y)} else {(y,x)};
      self.nid(ops::xor(lo, hi)) }}

  fn or(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::or(x,y) { nid }
    else if x.is_inv() && y.is_inv() { !self.and(x, y) }
    else {
      let (lo, hi) = if x<y {(x,y)} else {(y,x)};
      self.nid(ops::vel(lo, hi)) }}

  fn sub(&mut self, _v:vid::VID, _n:NID, _ctx:NID)->NID { todo!("ast::sub") }

  fn get(&self, s:&str)->Option<NID> { Some(*self.tags.get(s)?) }

  // generate dot file (graphviz)
  fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write) {
    macro_rules! w {
      ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}
    macro_rules! dotop {
      ($s:expr, $n:expr $(,$xs:expr)*) => {{
        w!("  \"{}\"[label={}];", $n.raw(), $s); // draw the node
        $({ if ($xs).is_inv() { w!("edge[style=dashed];"); }
            else { w!("edge[style=solid];"); }
            w!(" \"{}\"->\"{}\";", $xs.raw(), $n.raw()); })* }}}

    w!("digraph bdd {{");
    w!("  bgcolor=\"#3399cc\"; pad=0.225");
    w!("  rankdir=BT;"); // put root on top
    w!("  node[shape=circle, style=filled, fillcolor=\"#dddddd\", fontname=calibri];");
    w!("  edge[style=solid]");
    self.walk(n, &mut |n| {
      match n {
        nid::O => w!(" \"{}\"[label=⊥];", n),
        nid::I => w!(" \"{}\"[label=⊤];", n),
        _ if n.is_vid() => w!("\"{}\"[fillcolor=\"#bbbbbb\",label=\"{}\"];", n.raw(), n.vid()),
        _ => {
          let rpn: Vec<NID> = self.get_ops(n).to_rpn().cloned().collect();
          let fun = rpn.last().unwrap();
          if let Some(2) = fun.arity() {
            let (x, y) = (rpn[0], rpn[1]);
            match *fun {
              ops::AND => dotop!("∧",n,x,y),
              ops::XOR => dotop!("≠",n,x,y),
              ops::VEL => dotop!("∨",n,x,y),
              _ => panic!("unexpected op in dot(): {:?}", n) }}
          else { panic!("can't dot arbitrary ops yet: {:?}", rpn) }}}});
    w!("}}"); }
} // impl Base for RawASTBase

pub struct ASTBase { base: Simplify<RawASTBase> }
impl Base for ASTBase {
  inherit![when_hi, when_lo, and, xor, or, def, tag, get, sub, dot ];
  fn new()->Self { ASTBase{ base: Simplify{ base: <RawASTBase as Base>::new() }}}}

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
