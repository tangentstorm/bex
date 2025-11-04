//! Abstract syntax trees (simple logic combinators).

use std::collections::{HashMap,HashSet};

use crate::base::*;
use crate::{nid, NID, Fun};
use crate::{vid, vid::VID};
use crate::{ops, ops::Ops};
use crate::apl;
use crate::simp;


#[derive(Debug)]
pub struct RawASTBase {
  pub bits: Vec<Ops>,               // all known bits (simplified)
  // TODO: redesign tags. (only used externally)
  pub tags: HashMap<String, NID>,   // support for naming/tagging bits.
  hash: HashMap<Ops, NID>,          // expression cache (simple+complex)
}

type VarMaskFn = fn(&RawASTBase,vid::VID)->u64;

/// An ASTBase that does not use extra simplification rules.
impl RawASTBase {

  pub fn empty()->RawASTBase { RawASTBase{ bits:vec![], tags:HashMap::new(), hash:HashMap::new() }}
  pub fn len(&self)->usize { self.bits.len() }
  pub fn is_empty(&self)->bool { self.bits.is_empty() }

  pub fn push_raw_ops(&mut self, ops:Ops)->NID {
    let nid = NID::ixn(self.bits.len());
    self.bits.push(ops.clone());
    self.hash.insert(ops, nid);
    nid
  }

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
  pub fn node_masks(&self, vm:VarMaskFn)->Vec<u64> {
    let mut masks:Vec<u64> = Vec::with_capacity(self.bits.len());
    for bit in self.bits.iter() {
      let mut mask = 0u64;
      for &op in bit.to_rpn() {
        if op.is_fun() { continue }
        let op_mask =
          if op.is_const() { 0 }
          else if op.is_vid() { vm(self, op.vid()) }
          else if op.is_ixn() { masks[op.idx()] }
          else { todo!("mask({:?})", op) };
        mask |= op_mask; }
      masks.push(mask); }
    masks }

  /// Calculate the cost of each bit, where constants have cost 0, inputs have cost 1,
  /// and everything else is 1 + max(cost of input bits).
  pub fn node_costs(&self)->Vec<u64> {
    use std::cmp::max;
    let mut costs:Vec<u64> = Vec::with_capacity(self.bits.len());
    for bit in self.bits.iter() {
      let mut cost = 0u64;
      for &op in bit.to_rpn() {
        if op.is_fun() { continue }
        let op_cost =
          if op.is_const() { 0 }
          else if op.is_vid() { 1 }
          else if op.is_ixn() { costs[op.idx()] }
          else { todo!("cost({:?})", op) };
        cost = max(cost, op_cost); }
      costs.push(cost + 1); }
    costs }

  /// this returns a ragged 2d vector of direct references for each bit in the base
  pub fn reftable(&self) -> Vec<Vec<NID>> {
    //todo!("test case for reftable!");
    let bits = &self.bits;
    let mut res:Vec<Vec<NID>> = vec![vec![]; bits.len()];
    bits.iter().enumerate().for_each(|(i, bit)| {
      let n = NID::ixn(i);
      let f = |x:&NID| res[x.idx()].push(n);
      bit.to_rpn().rev().skip(1).for_each(f); });
    res }

  /// this is part of the garbage collection system. keep is the top level nid to keep.
  /// seen gets marked true for every nid that is a dependency of keep.
  /// TODO:: use a HashSet for 'seen' in markdeps()
  fn markdeps(&self, keep:NID, seen:&mut Vec<bool>) {
    // TODO: there should be a 'has_ixn'
    if keep.is_ixn() && !seen[keep.idx()] {
      seen[keep.idx()] = true;
      for &op in self.bits[keep.idx()].to_rpn() { self.markdeps(op, seen) }}}


  /// Construct a copy of the base, with the nodes reordered according to
  /// permutation vector pv. That is, pv is a vector of unique node indices
  /// that we want to keep, in the order we want them. (It might actually be
  /// shorter than bits.len() and thus not technically a permutation vector,
  /// but I don't have a better name for this concept.)
  pub fn permute(&self, pv:&[usize])->RawASTBase {
    // map each kept node in self.bits to Some(new position)
    let new:Vec<Option<usize>> = {
      let mut res = vec![None; self.bits.len()];
      for (ix,&old) in pv.iter().enumerate() { res[old] = Some(ix) }
      res };
    let nn = |x:NID|{
      assert!(x.is_ixn());
      let r = NID::ixn(new[x.idx()].unwrap_or_else(|| {
        println!("trying to find index from: {x}. index: {} (hex: {:X})", x.idx(), x.idx());
        println!("new.len() = {} (hex {:X})", new.len(), new.len());
        let rt = self.reftable();
        for r in rt[x.idx()].clone() { println!("  ref: {r:?} -> {:?} ({:?})", self.get_ops(r), new[r.idx()]); }
        panic!("?! {x}"); }));
      if x.is_inv() { !r } else { r }};
    let nnix = |x:NID| { if x.is_ixn() { nn(x) } else { x }};
    let bits = pv.iter().map(|&old| {
      let res:Vec<NID> = self.bits[old].to_rpn().map(|&x|nnix(x)).collect();
      ops::rpn(&res) }).collect();
    let mut tags = HashMap::new();
    for (key, &nid) in &self.tags {
      if nid.is_ixn() && new[nid.idx()].is_none() { continue }
      else { tags.insert(key.clone(), nnix(nid)); }}
    RawASTBase{ bits, tags, hash:HashMap::new() }}

  /// Construct a new RawASTBase with only the nodes necessary to define the given nodes.
  /// The relative order of the bits is preserved.
  pub fn gc(&self, keep:Vec<NID>) -> (RawASTBase, Vec<NID>) {
    // garbage collection: mark dependencies of the bits we want to keep
    let mut deps = vec!(false;self.bits.len());
    for &nid in keep.iter() { self.markdeps(nid, &mut deps) }

    let mut new:Vec<Option<usize>> = vec![None; self.bits.len()];
    let mut kept:Vec<usize> = vec![];
    for i in 0..self.bits.len() {
      if deps[i] { new[i]=Some(kept.len()); kept.push(i); }}

    (self.permute(&kept), keep.iter().map(|&i| {
      if let Some(ix) = new[i.idx()] { NID::ixn(ix) }
      else { panic!("gc(): failed to find new index for kept nid: {i:?}."); }}).collect())}

  /// Construct a new RawASTBase with only the nodes necessary to define the given nodes,
  /// then reorder those nodes by increasing cost (cheapest nodes first).
  pub fn repack(&self, keep:Vec<NID>) -> (RawASTBase, Vec<NID>) {
    let (base, kept) = self.gc(keep);
    if base.is_empty() { return (base, kept); }
    let costs = base.node_costs();
    let perm = apl::gradeup(&costs);
    let mut inv = vec![0usize; perm.len()];
    for (new_idx, &old_idx) in perm.iter().enumerate() { inv[old_idx] = new_idx; }
    let new_keep = kept.iter().map(|&nid|{
      if nid.is_ixn() {
        let mapped = NID::ixn(inv[nid.idx()]);
        if nid.is_inv() { !mapped } else { mapped }}
      else { nid }}).collect();
    (base.permute(&perm), new_keep) }

  pub fn get_ops(&self, n:NID)->&Ops {
    if n.is_ixn() { &self.bits[n.idx()] }
    else { panic!("nid {n} is not an ixn...") }}


  // apply a function nid to a list of arguments
  pub fn apply(&mut self, n:NID, args0:Vec<NID>)->NID {
    // for table nids:
    //   - make sure #args == arity
    //   - handle constant inputs.
    let (f, args) =
      if let Some(mut f) = n.to_fun() {
        // !! TODO: move this to NidFun
        assert_eq!(f.arity() as usize, args0.len());
        // first pass: handle constant inputs
        let mut i = 0; let mut args1 = vec![];
        for &arg in args0.iter() {
          if arg.is_const() { f=f.when(i, arg==nid::I); }
          else { args1.push(arg); i+=1 }}
        // second pass: merge similar inputs
        let mut matches : HashMap<NID,u8> = HashMap::new();
        let mut i = 0;
        for &arg in args1.iter() {
          if let Some(&ix) = matches.get(&arg.raw()) {
            if arg == args1[ix as usize] { f = f.when_same(ix, i)}
            else { f = f.when_diff(ix, i)} }
          else { matches.insert(arg.raw(), i); i+=1; }}
        (f.to_nid(), args1) }
      else { (n, args0) };
    let env:HashMap<VID,NID> = args.iter().enumerate()
      .map(|(i,&x)|(VID::var(i as u32), x)).collect();
    self.eval(f, &env) }
} // impl RawASTBase

impl Base for RawASTBase {

  fn new()->Self { RawASTBase::empty() }

  fn when_hi(&mut self, v:vid::VID, n:NID)->NID { self.when(v, nid::I, n) }
  fn when_lo(&mut self, v:vid::VID, n:NID)->NID { self.when(v, nid::O, n) }

  fn def(&mut self, s:String, v:vid::VID)->NID {
    let nid = NID::from_vid(v);
    self.tag(nid, format!("{}{:?}", s, v)) }

  fn tag(&mut self, n:NID, s:String)->NID {
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

  fn ite(&mut self, i:NID, t:NID, e:NID)->NID {
    if let Some(nid) = simp::ite(i,t,e) { nid }
    else {
      self.nid(ops::ite(i, t, e)) }}

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
          let fun = rpn.last().unwrap().to_fun().unwrap();
          if 2 == fun.arity() {
            let (x, y) = (rpn[0], rpn[1]);
            match fun {
              ops::AND => dotop!("∧",n,x,y),
              ops::XOR => dotop!("≠",n,x,y),
              ops::VEL => dotop!("∨",n,x,y),
              _ => panic!("unexpected op in dot(): {:?}", n) }}
          else { panic!("can't dot arbitrary ops yet: {:?}", rpn) }}}});
    w!("}}"); }

  /// recursively evaluate an AST, caching shared sub-expressions
  fn _eval_aux(&mut self, n:NID, kvs:&HashMap<VID, NID>, cache:&mut HashMap<NID,NID>)->NID {
    let raw = n.raw();
    let res =
      if n.is_vid() {
        if let Some(&nid) = kvs.get(&n.vid()) { nid }
        else { n }}
      else if n.is_lit() { raw }
      else if n.is_fun() {
        let mut f = n.to_fun().unwrap();
        let res:NID;
        loop {
          let i = f.arity();
          if i == 0 { res = if f.tbl()==0 { nid::O } else { nid::I}; break; }
          else {
            let &arg = kvs.get(&VID::var((i as u32)-1))
              .expect("don't have enough args to fully evaluate!");
            f = f.when(i-1, arg==nid::I); }};
        res }
      else if let Some(&vn) = cache.get(&raw) { vn }
      else {
        let (f, args0) = self.get_ops(raw).to_app();
        let args:Vec<NID> = args0.iter().map(|&x| self._eval_aux(x, kvs, cache)).collect();
        let t = self.apply(f, args); cache.insert(n, t); t };
    if n.is_inv() { !res } else { res }}

} // impl Base for RawASTBase

pub struct ASTBase { base: Simplify<RawASTBase> }
impl ASTBase {
  pub fn from_raw(raw:RawASTBase)->Self { ASTBase{ base: Simplify{ base: raw } }}
  pub fn new()->Self { ASTBase::from_raw(RawASTBase::new()) }}

impl Default for ASTBase {
    fn default() -> Self {Self::new()}}

impl Base for ASTBase {
  inherit![when_hi, when_lo, and, xor, or, ite, def, tag, get, sub, dot ];
  fn new()->Self { ASTBase::new() }}

impl ASTBase {
  pub fn empty()->Self { ASTBase { base: Simplify{ base: RawASTBase::empty() }}}
  pub fn raw_ast(&self)->&RawASTBase { &self.base.base }
  pub fn raw_ast_mut(&mut self)->&mut RawASTBase { &mut self.base.base }}

test_base_consts!(ASTBase);
test_base_when!(ASTBase);

#[test] fn ast_and(){
  let mut b = ASTBase::empty();
  let x0 = NID::var(0); let x1 = NID::var(1);
  let x01 = b.and(x0,x1);
  let x10 = b.and(x1,x0);
  assert_eq!(x01, x10, "expect $0 & $1 == $1 & $0"); }


#[test] fn ast_eval_full(){
  use crate::{I, O, vid::named::{x0, x1}, nid::named::{x0 as nx0, x1 as nx1}};
  let mut b = RawASTBase::empty();
  let and = expr![b, (nx0 & nx1)];
  assert_eq!(b.eval(and, &vid_map![x0: O, x1: O]), O, "O and O => O");
  assert_eq!(b.eval(and, &vid_map![x0: O, x1: I]), O, "O and I => O");
  assert_eq!(b.eval(and, &vid_map![x0: I, x1: O]), O, "I and O => O");
  assert_eq!(b.eval(and, &vid_map![x0: I, x1: I]), I, "I and I => I"); }

// TODO: #[test] fn ast_eval_partial(){
// (for now you have to assign all variables)
//   use crate::{I, O, vid::named::{x0, x1}};
//   let mut b = RawASTBase::empty();
//   let and = expr![b, (x0 & x1)];
//   assert_eq!(b.eval(and, &vid_map![x1: O]), O, "expect  x0 & O == O");
//   assert_eq!(b.eval(and, &vid_map![x1: !x0]), O, "expect  x0 & ~x0 == O");
//   assert_eq!(b.eval(and, &vid_map![x1: I]), x0, "expect x0 & I == x0");
//   assert_eq!(b.eval(and, &vid_map![x1: x0]), x0, "expect  x0 & x0 == x0"); }

#[test] fn test_repack() {
  let mut b = RawASTBase::empty();
  use crate::nid::named::{x0, x1, x2, x3, x4};
  let and = b.and(x0, x1);
  let or = b.or(x2, x3);
  b.tag(or, "or".to_string());
  let xor = b.xor(x4, and);
  let (b2, keep) = b.repack(vec![xor]);
  assert_eq!(b2.len(), 2);
  assert_eq!(keep, vec![NID::ixn(1)]);
  assert_eq!(b2.get_ops(keep[0]), b.get_ops(xor)); }
