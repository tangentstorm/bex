//! Zero-suppressed Decision Diagrams (ZDDs) for families of sets.
//!
//! ZDD reduction rule: eliminate nodes whose hi branch is O.
//! Skipped variables are implicitly absent from every set in the family.
//!
//! Constants: O = empty family, I = family containing only the empty set.
//! NID::from_vid(v) = family {{v}} (the singleton set containing v).
use std::collections::{HashMap, HashSet};
use crate::base::Base;
use crate::simp;
use crate::nid::{NID,I,O};
use crate::vid::{VID,VidOrdering};
use crate::vhl::{Vhl, HiLo, HiLoBase, Walkable};
use crate::cur::CursorPlan;
use crate::reg::Reg;
use crate::bdd::BddBase;

#[repr(u8)]
#[derive(Clone, Copy)]
enum OpTag { Union=1, Intersect=2, Diff=3, Product=4, Quotient=5 }

pub struct ZddBase {
  hilos: crate::vhl::HiLoCache,
  universe: Vec<VID>,
  universe_set: HashSet<VID>,
  memo: HashMap<(u8,NID,NID),NID>,
  power_set_cache: Option<NID>,
  tags: HashMap<String,NID>,
}

// -- core helpers --
impl ZddBase {

  pub fn new()->Self {
    ZddBase {
      hilos: crate::vhl::HiLoCache::new(),
      universe: vec![],
      universe_set: HashSet::new(),
      memo: HashMap::new(),
      power_set_cache: None,
      tags: HashMap::new() }}

  /// Canonical node constructor. Enforces ZDD reduction: hi==O => lo.
  fn mk(&self, v:VID, hi:NID, lo:NID)->NID {
    if hi == O { return lo; }
    if hi == I && lo == O { return NID::from_vid(v); }
    self.hilos.get_or_insert(v, HiLo{hi, lo}) }

  fn fetch(&self, n:NID)->Vhl {
    if n.is_vid() { Vhl{ v:n.vid(), hi:I, lo:O } }
    else {
      let hl = self.hilos.get_hilo(n);
      Vhl{ v:n.vid(), hi:hl.hi, lo:hl.lo } }}

  fn register_vid(&mut self, v:VID) {
    if (v.is_var() || v.is_vir()) && self.universe_set.insert(v) {
      let pos = self.universe.iter()
        .position(|u| v.is_above(u))
        .unwrap_or(self.universe.len());
      self.universe.insert(pos, v);
      self.power_set_cache = None; }}

  fn register_nid(&mut self, n:NID) {
    if n.is_const() { return; }
    let v = n.vid();
    self.register_vid(v);
    if !n.is_vid() && !n.is_inv() {
      let mut vids = vec![];
      self.walk_dn(n, &mut |_,v,_,_| { vids.push(v); });
      for v in vids { self.register_vid(v); }}}

  fn contains_empty(&self, n:NID)->bool {
    if n == O { false } else if n == I { true }
    else { self.contains_empty(self.fetch(n).lo) }}

  fn add_empty(&mut self, n:NID)->NID {
    if n == O || n == I { return I; }
    let Vhl{v, hi, lo} = self.fetch(n);
    let lo1 = self.add_empty(lo);
    self.mk(v, hi, lo1) }

  fn remove_empty(&mut self, n:NID)->NID {
    if n == O || n == I { return O; }
    let Vhl{v, hi, lo} = self.fetch(n);
    let lo1 = self.remove_empty(lo);
    self.mk(v, hi, lo1) }

  pub fn reset_memo(&mut self) { self.memo.clear(); }

  fn cofactor_on(&self, n:NID, v:VID)->(NID,NID) {
    if n == O { return (O, O); }
    if n == I { return (O, I); }
    let nv = n.vid();
    if nv == v { let Vhl{v:_,hi,lo} = self.fetch(n); (hi, lo) }
    else { (O, n) } // v not at this level: subset1=O, subset0=n
  }
}

// -- family-of-sets API --
impl ZddBase {

  pub fn subset1(&mut self, n:NID, v:VID)->NID {
    if n == O || n == I { return O; }
    let Vhl{v:nv, hi, lo} = self.fetch(n);
    match v.cmp_depth(&nv) {
      VidOrdering::Above => O,
      VidOrdering::Level => hi,
      VidOrdering::Below => {
        let h1 = self.subset1(hi, v);
        let l1 = self.subset1(lo, v);
        self.mk(nv, h1, l1) }}}

  pub fn subset0(&mut self, n:NID, v:VID)->NID {
    if n == O { return O; }
    if n == I { return I; }
    let Vhl{v:nv, hi, lo} = self.fetch(n);
    match v.cmp_depth(&nv) {
      VidOrdering::Above => n,
      VidOrdering::Level => lo,
      VidOrdering::Below => {
        let h1 = self.subset0(hi, v);
        let l1 = self.subset0(lo, v);
        self.mk(nv, h1, l1) }}}

  pub fn change(&mut self, n:NID, v:VID)->NID {
    if n == O { return O; }
    if n == I { return self.mk(v, I, O); }
    let Vhl{v:nv, hi, lo} = self.fetch(n);
    match v.cmp_depth(&nv) {
      VidOrdering::Above => self.mk(v, n, O),
      VidOrdering::Level => self.mk(nv, lo, hi),
      VidOrdering::Below => {
        let h1 = self.change(hi, v);
        let l1 = self.change(lo, v);
        self.mk(nv, h1, l1) }}}

  pub fn onset(&mut self, n:NID, v:VID)->NID { self.subset1(n, v) }
  pub fn offset(&mut self, n:NID, v:VID)->NID { self.subset0(n, v) }

  pub fn union(&mut self, p:NID, q:NID)->NID {
    if p == O { return q; }
    if q == O { return p; }
    if p == q { return p; }
    let (a,b) = if p < q { (p,q) } else { (q,p) };
    let key = (OpTag::Union as u8, a, b);
    if let Some(&r) = self.memo.get(&key) { return r; }
    let r = self.union_rec(p, q);
    self.memo.insert(key, r);
    r }

  fn union_rec(&mut self, p:NID, q:NID)->NID {
    if p == I { return self.add_empty(q); }
    if q == I { return self.add_empty(p); }
    let pv = p.vid(); let qv = q.vid();
    match pv.cmp_depth(&qv) {
      VidOrdering::Above => {
        let Vhl{v, hi, lo} = self.fetch(p);
        let lo1 = self.union(lo, q);
        self.mk(v, hi, lo1) }
      VidOrdering::Below => self.union_rec(q, p),
      VidOrdering::Level => {
        let a = self.fetch(p); let b = self.fetch(q);
        let hi = self.union(a.hi, b.hi);
        let lo = self.union(a.lo, b.lo);
        self.mk(pv, hi, lo) }}}

  pub fn intersect(&mut self, p:NID, q:NID)->NID {
    if p == O || q == O { return O; }
    if p == q { return p; }
    if p == I { return if self.contains_empty(q) { I } else { O }; }
    if q == I { return if self.contains_empty(p) { I } else { O }; }
    let (a,b) = if p < q { (p,q) } else { (q,p) };
    let key = (OpTag::Intersect as u8, a, b);
    if let Some(&r) = self.memo.get(&key) { return r; }
    let pv = p.vid(); let qv = q.vid();
    let r = match pv.cmp_depth(&qv) {
      VidOrdering::Above => {
        let Vhl{v:_, hi:_, lo} = self.fetch(p);
        self.intersect(lo, q) }
      VidOrdering::Below => {
        let Vhl{v:_, hi:_, lo} = self.fetch(q);
        self.intersect(p, lo) }
      VidOrdering::Level => {
        let pa = self.fetch(p); let qa = self.fetch(q);
        let hi = self.intersect(pa.hi, qa.hi);
        let lo = self.intersect(pa.lo, qa.lo);
        self.mk(pv, hi, lo) }};
    self.memo.insert(key, r);
    r }

  pub fn diff(&mut self, p:NID, q:NID)->NID {
    if p == O || p == q { return O; }
    if q == O { return p; }
    if p == I { return if self.contains_empty(q) { O } else { I }; }
    if q == I { return self.remove_empty(p); }
    let key = (OpTag::Diff as u8, p, q);
    if let Some(&r) = self.memo.get(&key) { return r; }
    let pv = p.vid(); let qv = q.vid();
    let r = match pv.cmp_depth(&qv) {
      VidOrdering::Above => {
        let Vhl{v, hi, lo} = self.fetch(p);
        let lo1 = self.diff(lo, q);
        self.mk(v, hi, lo1) }
      VidOrdering::Below => {
        let Vhl{v:_, hi:_, lo} = self.fetch(q);
        self.diff(p, lo) }
      VidOrdering::Level => {
        let pa = self.fetch(p); let qa = self.fetch(q);
        let hi = self.diff(pa.hi, qa.hi);
        let lo = self.diff(pa.lo, qa.lo);
        self.mk(pv, hi, lo) }};
    self.memo.insert(key, r);
    r }

  pub fn product(&mut self, p:NID, q:NID)->NID {
    if p == O || q == O { return O; }
    if p == I { return q; }
    if q == I { return p; }
    let (a,b) = if p < q { (p,q) } else { (q,p) };
    let key = (OpTag::Product as u8, a, b);
    if let Some(&r) = self.memo.get(&key) { return r; }
    let pv = p.vid(); let qv = q.vid();
    let r = match pv.cmp_depth(&qv) {
      VidOrdering::Above => {
        let pa = self.fetch(p);
        let hi = self.product(pa.hi, q);
        let lo = self.product(pa.lo, q);
        self.mk(pv, hi, lo) }
      VidOrdering::Below => {
        let qa = self.fetch(q);
        let hi = self.product(p, qa.hi);
        let lo = self.product(p, qa.lo);
        self.mk(qv, hi, lo) }
      VidOrdering::Level => {
        let pa = self.fetch(p); let qa = self.fetch(q);
        let p1q1 = self.product(pa.hi, qa.hi);
        let p1q0 = self.product(pa.hi, qa.lo);
        let p0q1 = self.product(pa.lo, qa.hi);
        let p0q0 = self.product(pa.lo, qa.lo);
        let u = self.union(p1q1, p1q0);
        let hi = self.union(u, p0q1);
        self.mk(pv, hi, p0q0) }};
    self.memo.insert(key, r);
    r }

  pub fn quotient(&mut self, f:NID, g:NID)->NID {
    if g == O { panic!("ZDD division by empty family"); }
    if f == O { return O; }
    if g == I { return f; }
    if f == g { return I; }
    let key = (OpTag::Quotient as u8, f, g);
    if let Some(&r) = self.memo.get(&key) { return r; }
    let gv = g.vid();
    let r = {
      let (f1, f0) = self.cofactor_on(f, gv);
      let (g1, g0) = self.cofactor_on(g, gv);
      // if g branches on gv but f doesn't contain gv, then f1==O
      // quotient(O, g1) = O, so result is O (can't divide)
      let q1 = self.quotient(f1, g1);
      if q1 == O { O }
      else if g0 == I { q1 }
      else {
        let q0 = self.quotient(f0, g0);
        self.intersect(q1, q0) }};
    self.memo.insert(key, r);
    r }

  pub fn remainder(&mut self, f:NID, g:NID)->NID {
    let q = self.quotient(f, g);
    let qg = self.product(q, g);
    self.diff(f, qg) }

  pub fn count(&self, n:NID)->u64 {
    fn aux(b:&ZddBase, n:NID, memo:&mut HashMap<NID,u64>)->u64 {
      if n == O { return 0; }
      if n == I { return 1; }
      if let Some(&c) = memo.get(&n) { return c; }
      let Vhl{v:_, hi, lo} = b.fetch(n);
      let c = aux(b, hi, memo) + aux(b, lo, memo);
      memo.insert(n, c);
      c }
    aux(self, n, &mut HashMap::new()) }
}

// -- universe tracking, complement --
impl ZddBase {

  pub fn power_set(&mut self)->NID {
    if let Some(ps) = self.power_set_cache { return ps; }
    let mut r = I;
    // iterate bottom-to-top so mk builds correctly
    for &v in self.universe.iter().rev() {
      r = self.mk(v, r, r); }
    self.power_set_cache = Some(r);
    r }

  fn power_set_without(&mut self, exclude:VID)->NID {
    let mut r = I;
    for &v in self.universe.iter().rev() {
      if v != exclude { r = self.mk(v, r, r); }}
    r }

  pub fn complement(&mut self, n:NID)->NID {
    let u = self.power_set();
    self.diff(u, n) }

  fn resolve_inv(&mut self, n:NID)->NID {
    if n.is_const() { return n; } // I and O are constants, pass through
    if !n.is_inv() { return n; }
    // n is inverted non-const: complement its raw form
    let raw = n.raw();
    self.complement(raw) }
}

// -- trait impls --

impl Walkable for ZddBase {
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>, topdown:bool)
  where F: FnMut(NID,VID,NID,NID) {
    if n.is_const() || n.is_vid() { return; }
    if !seen.insert(n) { return; }
    let Vhl{v, hi, lo} = self.fetch(n);
    if topdown { f(n, v, hi, lo); }
    if !lo.is_const() { self.step(lo, f, seen, topdown); }
    if !hi.is_const() { self.step(hi, f, seen, topdown); }
    if !topdown { f(n, v, hi, lo); }}}

impl HiLoBase for ZddBase {
  fn get_hilo(&self, n:NID)->Option<HiLo> {
    if n.is_const() || n.is_inv() { return None; }
    if n.is_vid() { return Some(HiLo{hi:I, lo:O}); }
    let Vhl{v:_, hi, lo} = self.fetch(n);
    Some(HiLo{hi, lo}) }}

impl CursorPlan for ZddBase {}

impl Base for ZddBase {
  fn new()->Self { ZddBase::new() }

  fn when_hi(&mut self, v:VID, n:NID)->NID {
    self.register_vid(v); self.register_nid(n);
    let n = self.resolve_inv(n);
    let s1 = self.subset1(n, v);
    let s0 = self.subset0(n, v);
    self.union(s1, s0) }

  fn when_lo(&mut self, v:VID, n:NID)->NID {
    self.register_vid(v); self.register_nid(n);
    let n = self.resolve_inv(n);
    self.subset0(n, v) }

  fn and(&mut self, x:NID, y:NID)->NID {
    if let Some(n) = simp::and(x,y) { return n; }
    self.register_nid(x); self.register_nid(y);
    let x = self.resolve_inv(x);
    let y = self.resolve_inv(y);
    self.intersect(x, y) }

  fn xor(&mut self, x:NID, y:NID)->NID {
    if let Some(n) = simp::xor(x,y) { return n; }
    self.register_nid(x); self.register_nid(y);
    let x = self.resolve_inv(x);
    let y = self.resolve_inv(y);
    let u = self.union(x, y);
    let i = self.intersect(x, y);
    self.diff(u, i) }

  fn or(&mut self, x:NID, y:NID)->NID {
    if let Some(n) = simp::or(x,y) { return n; }
    self.register_nid(x); self.register_nid(y);
    let x = self.resolve_inv(x);
    let y = self.resolve_inv(y);
    self.union(x, y) }

  fn ite(&mut self, i:NID, t:NID, e:NID)->NID {
    if let Some(n) = simp::ite(i,t,e) { return n; }
    let it = self.and(i, t);
    let ni = !i;
    let ne = self.and(ni, e);
    self.or(it, ne) }

  fn def(&mut self, s:String, v:VID)->NID {
    self.register_vid(v);
    let ps = self.power_set_without(v);
    let n = self.mk(v, ps, O);
    self.tag(n, s) }

  fn tag(&mut self, n:NID, s:String)->NID { self.tags.insert(s, n); n }
  fn get(&self, s:&str)->Option<NID> { self.tags.get(s).copied() }

  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID {
    if !ctx.might_depend_on(v) { return ctx; }
    let c1 = self.when_hi(v, ctx);
    let c0 = self.when_lo(v, ctx);
    self.ite(n, c1, c0) }

  fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write) {
    macro_rules! w {
      ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}
    w!("digraph zdd {{");
    w!("  bgcolor=\"#336699\"; pad=0.225");
    w!("  node[shape=circle, style=filled, fillcolor=\"#cccccc\", fontname=calibri]");
    w!("  edge[arrowhead=none]");
    w!("subgraph head {{ h1[shape=plaintext, fillcolor=none, label=\"ZDD\"] }}");
    w!("  I[label=⊤, shape=square, fillcolor=white]");
    w!("  O[label=⊥, shape=square, fontcolor=white, fillcolor=\"#333333\"]");
    w!("{{rank = same; I; O;}}");
    self.walk_dn(n, &mut |n,_,_,_| w!("  \"{}\"[label=\"{:?}\"];", n, n.vid()));
    w!("edge[style=solid];");
    self.walk_dn(n, &mut |n,_,hi,_| w!("  \"{}\"->\"{}\";", n, hi));
    w!("edge[style=dashed];");
    self.walk_dn(n, &mut |n,_,_,lo| w!("  \"{}\"->\"{}\";", n, lo));
    w!("}}"); }

  fn solution_set(&self, n:NID, nvars:usize)->HashSet<Reg> {
    self.solutions_pad(n, nvars).collect() }
}

// -- conversion and solution iteration --
impl ZddBase {

  /// Convert ZDD node to another Base (typically BDD).
  /// Interprets the ZDD as a characteristic function over the current universe.
  pub fn to_base(&self, n:NID, dest:&mut dyn Base)->NID {
    let mut memo: HashMap<NID,NID> = HashMap::new();
    memo.insert(O, O);
    // I = {∅} means all universe vars are 0
    let mut i_val = I;
    for &v in self.universe.iter().rev() {
      let vn = NID::from_vid(v);
      let not_v = dest.xor(vn, I);
      i_val = dest.and(i_val, not_v); // AND NOT v
    }
    memo.insert(I, i_val);
    self.to_base_rec(n, dest, &mut memo) }

  fn to_base_rec(&self, n:NID, dest:&mut dyn Base, memo:&mut HashMap<NID,NID>)->NID {
    if let Some(&r) = memo.get(&n) { return r; }
    let Vhl{v, hi, lo} = self.fetch(n);
    // recursively convert children
    let hi_d = self.to_base_rec(hi, dest, memo);
    let lo_d = self.to_base_rec(lo, dest, memo);
    // force skipped universe vars to 0 between v and hi's level
    let hi_forced = self.force_absent(v, hi.vid(), hi_d, dest);
    let lo_forced = self.force_absent(v, lo.vid(), lo_d, dest);
    let vn = NID::from_vid(v);
    let r = dest.ite(vn, hi_forced, lo_forced);
    // force skipped vars above v (between top of universe and v)
    let r = self.force_above(v, r, dest);
    memo.insert(n, r);
    r }

  /// AND NOT each universe var strictly between parent_v and child_v onto dest_nid
  fn force_absent(&self, parent_v:VID, child_v:VID, dest_nid:NID, dest:&mut dyn Base)->NID {
    let mut r = dest_nid;
    for &uv in &self.universe {
      if uv == parent_v { continue; }
      if !uv.is_below(&parent_v) { continue; } // above parent: skip
      if uv == child_v { break; }
      if child_v.is_below(&uv) || child_v == VID::top() {
        // uv is between parent and child (or child is const)
        let vn = NID::from_vid(uv);
        let not_uv = dest.xor(vn, I);
        r = dest.and(r, not_uv); // AND NOT uv
      }
    }
    r }

  /// Force absent universe vars above node_v (for root-level gaps)
  fn force_above(&self, _node_v:VID, dest_nid:NID, _dest:&mut dyn Base)->NID {
    // Only needed at the very top call. We handle this in to_base by
    // wrapping the root result. For recursive calls this is identity.
    dest_nid }

  pub fn solutions_pad(&self, n:NID, nvars:usize)->ZddSolIterator<'_> {
    ZddSolIterator::from_zdd_base(self, n, nvars) }
}

// -- ZddSolIterator (via BDD conversion) --

pub struct ZddSolIterator<'a> {
  _zdd: &'a ZddBase,
  bdd: BddBase,
  bcur: Option<crate::cur::Cursor> }

impl<'a> ZddSolIterator<'a> {
  pub fn from_zdd_base(zdd: &'a ZddBase, nid:NID, nvars:usize)->Self {
    let mut bdd = BddBase::new();
    let bnid = zdd.to_base(nid, &mut bdd);
    let bcur = bdd.first_solution(bnid, nvars);
    ZddSolIterator { _zdd:zdd, bdd, bcur } }}

impl Iterator for ZddSolIterator<'_> {
  type Item = Reg;
  fn next(&mut self)->Option<Self::Item> {
    if let Some(cur) = self.bcur.take() {
      let res = Some(cur.scope.clone());
      self.bcur = self.bdd.next_solution(cur);
      res }
    else { None } }}

// -- ZddSetIterator (native family enumeration) --

pub struct ZddSetIterator<'a> {
  zdd: &'a ZddBase,
  /// Stack of (node, took_hi). We DFS: try lo first, then hi.
  stack: Vec<(NID, bool)>,
  /// Current set being built (VIDs included via hi branches)
  current: Vec<VID>,
  done: bool }

impl<'a> ZddSetIterator<'a> {
  pub fn from_zdd_base(zdd: &'a ZddBase, root:NID)->Self {
    if root == O {
      ZddSetIterator { zdd, stack:vec![], current:vec![], done:true }
    } else {
      ZddSetIterator { zdd, stack:vec![(root, false)], current:vec![], done:false } }}}

impl Iterator for ZddSetIterator<'_> {
  type Item = Reg;
  fn next(&mut self)->Option<Self::Item> {
    while !self.done {
      if self.stack.is_empty() { self.done = true; return None; }
      let &mut (n, ref mut took_hi) = self.stack.last_mut().unwrap();
      if n == I {
        // yield current set
        let nvars = if self.current.is_empty() { 1 }
          else { self.current.iter().map(|v| v.var_ix()).max().unwrap() + 1 };
        let mut reg = Reg::new(nvars);
        for &v in &self.current { reg.put(v.var_ix(), true); }
        self.stack.pop();
        return Some(reg); }
      if n == O { self.stack.pop(); continue; }
      if !*took_hi {
        // first visit: go down lo branch
        *took_hi = true;
        let Vhl{v:_, hi:_, lo} = self.zdd.fetch(n);
        self.stack.push((lo, false));
      } else {
        // second visit: go down hi branch
        let Vhl{v, hi, lo:_} = self.zdd.fetch(n);
        self.stack.pop();
        self.current.push(v);
        self.stack.push((hi, false));
      }}
    None }}

impl ZddBase {
  pub fn sets(&self, n:NID)->ZddSetIterator<'_> {
    ZddSetIterator::from_zdd_base(self, n) }
}

// -- tests --
test_base_consts!(ZddBase);
test_base_when!(ZddBase);

#[test] fn test_zdd_mk_reduction() {
  let z = ZddBase::new();
  let v = VID::var(0);
  assert_eq!(z.mk(v, O, I), I, "hi==O should reduce to lo");
  assert_eq!(z.mk(v, O, O), O, "hi==O should reduce to lo"); }

#[test] fn test_zdd_interning() {
  let z = ZddBase::new();
  let v = VID::var(0);
  let a = z.mk(v, I, I);
  let b = z.mk(v, I, I);
  assert_eq!(a, b, "same (v,hi,lo) should give same NID"); }

#[test] fn test_zdd_union() {
  let mut z = ZddBase::new();
  let v = VID::var(0);
  let sv = z.mk(v, I, O); // {{v}}
  // union with O
  assert_eq!(z.union(sv, O), sv);
  assert_eq!(z.union(O, sv), sv);
  // union with self
  assert_eq!(z.union(sv, sv), sv);
  // union with I = add empty set
  let both = z.union(sv, I); // {{v}, {}}
  assert_eq!(z.count(both), 2);
  // commutativity
  assert_eq!(z.union(I, sv), both); }

#[test] fn test_zdd_intersect() {
  let mut z = ZddBase::new();
  let v0 = VID::var(0); let v1 = VID::var(1);
  let s0 = z.mk(v0, I, O); // {{v0}}
  let s1 = z.mk(v1, I, O); // {{v1}}
  assert_eq!(z.intersect(s0, s1), O, "disjoint families");
  assert_eq!(z.intersect(s0, s0), s0);
  assert_eq!(z.intersect(s0, O), O);
  // I ∩ F = I if F contains ∅
  let f = z.mk(v0, I, I); // {{v0}, {}}
  assert_eq!(z.intersect(I, f), I); }

#[test] fn test_zdd_diff() {
  let mut z = ZddBase::new();
  let v = VID::var(0);
  let sv = z.mk(v, I, O);
  assert_eq!(z.diff(sv, O), sv);
  assert_eq!(z.diff(sv, sv), O);
  assert_eq!(z.diff(O, sv), O);
  let both = z.union(sv, I);
  assert_eq!(z.diff(both, I), sv); }

#[test] fn test_zdd_count() {
  let z = ZddBase::new();
  assert_eq!(z.count(O), 0);
  assert_eq!(z.count(I), 1);
  let v = VID::var(0);
  let both = z.mk(v, I, I); // {{v}, {}}
  assert_eq!(z.count(both), 2); }

#[test] fn test_zdd_change() {
  let mut z = ZddBase::new();
  let v = VID::var(0);
  let sv = z.change(I, v); // {{}} -> {{v}}
  assert_eq!(sv, z.mk(v, I, O));
  let back = z.change(sv, v); // {{v}} -> {{}}
  assert_eq!(back, I); }

#[test] fn test_zdd_product() {
  let mut z = ZddBase::new();
  let v0 = VID::var(0); let v1 = VID::var(1);
  let s0 = z.mk(v0, I, O); // {{v0}}
  let s1 = z.mk(v1, I, O); // {{v1}}
  let p = z.product(s0, s1); // {{v0,v1}}
  assert_eq!(z.count(p), 1);
  // product with I is identity
  assert_eq!(z.product(s0, I), s0);
  assert_eq!(z.product(I, s0), s0);
  // product with O is empty
  assert_eq!(z.product(s0, O), O); }

#[test] fn test_zdd_complement() {
  let mut z = ZddBase::new();
  let v0 = VID::var(0); let v1 = VID::var(1);
  z.register_vid(v0); z.register_vid(v1);
  let s0 = z.mk(v0, I, O); // {{v0}}
  let ps = z.power_set();
  assert_eq!(z.count(ps), 4); // 2^2 = 4 subsets
  let c = z.complement(s0);
  assert_eq!(z.count(c), 3); // 4 - 1
  let all = z.union(s0, c);
  assert_eq!(all, ps); }

#[test] fn test_zdd_quotient_remainder() {
  let mut z = ZddBase::new();
  let v0 = VID::var(0); let v1 = VID::var(1); let v2 = VID::var(2);
  // f = {{v0,v1}, {v0,v2}, {v0}}
  let s01 = z.product(z.mk(v0,I,O), z.mk(v1,I,O));
  let s02 = z.product(z.mk(v0,I,O), z.mk(v2,I,O));
  let s0 = z.mk(v0, I, O);
  let tmp = z.union(s02, s0);
  let f = z.union(s01, tmp);
  // g = {{v0}}
  let g = z.mk(v0, I, O);
  let q = z.quotient(f, g);
  let r = z.remainder(f, g);
  let qg = z.product(q, g);
  let reconstructed = z.union(qg, r);
  assert_eq!(f, reconstructed, "f should equal q*g + r"); }
