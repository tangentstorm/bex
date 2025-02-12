//! A module for efficient implementation of binary decision diagrams.
extern crate num_cpus;
use std::collections::{HashMap,HashSet};
use crate::base::Base;
use crate::reg::Reg;
use crate::vhl::Walkable;
use crate::nid::{NID,O,I};
use crate::vid::{VID,VidOrdering,topmost_of3};
use crate::wip;

mod bdd_sols;
pub mod bdd_swarm; use self::bdd_swarm::*;



/// An if/then/else triple. Like VHL, but all three slots are NIDs.
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone, Copy)]
pub struct ITE {pub i:NID, pub t:NID, pub e:NID}  // nopub!! only public for WorkState
impl ITE {
  /// shorthand constructor
  pub fn new (i:NID, t:NID, e:NID)-> ITE { ITE { i, t, e } }
  pub fn top_vid(&self)->VID {
    let (i,t,e) = (self.i.vid(), self.t.vid(), self.e.vid());
    topmost_of3(i,t,e) }}

/// This represents the result of normalizing an ITE. There are three conditions:
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Norm {
  /// used when the ITE simplifies to a single NID.
  Nid(NID),
  /// a normalized ITE.
  Ite(NormIteKey),
  /// a normalized, inverted ITE.
  Not(NormIteKey)}

impl Norm {
  pub fn to_key(&self)->NormIteKey {
    match self {
      Norm::Nid(_) => panic!("Norm::Nid cannot be a key!"),
      Norm::Not(_) => panic!("Norm::Not cannot be a key!"),
      Norm::Ite(ite) => *ite}}
  pub fn is_inv(&self)->bool {
    match self {
      Norm::Nid(x) => x.is_inv(),
      Norm::Not(_) => true,
      Norm::Ite(_) => false}}}

/// a normalized ITE suitable for use as a key in the computed cache
#[derive(Eq,PartialEq,Hash,Debug,Default,Clone,Copy)]
pub struct NormIteKey(pub ITE); // nopub



impl ITE {
  /// choose normal form for writing this triple. Algorithm based on:
  /// "Efficient Implementation of a BDD Package"
  /// <http://www.cs.cmu.edu/~emc/15817-f08/bryant-bdd-1991.pdf>
  pub fn norm(f0:NID, g0:NID, h0:NID)->Norm {
    let mut f = f0; let mut g = g0; let mut h = h0;
    loop {
      if f.is_const() { return Norm::Nid(if f==I { g } else { h }) }  // (I/O, _, _)
      if g==h { return Norm::Nid(g) }                                 // (_, g, g)
      if g==f { if h.is_const() {
                return Norm::Nid(if h==I { I } else { f }) } // (f, f, I/O)
                else { g=I }}
      else if g.is_const() && h.is_const() { // both const, and we know g!=h
        return if g==I { Norm::Nid(f) } else { Norm::Nid(!f) }}
      else {
        let nf = !f;
        if      g==nf { g=O }
        else if h==nf { h=I }
        else if h==f  { h=O }
        else {
          let (fv, fi) = (f.vid(), f.idx());
          macro_rules! cmp { ($x0:expr,$x1:expr) => {
            { let x0=$x0; ((x0.is_above(&fv)) || ((x0==fv) && ($x1<fi))) }}}
          if g.is_const() && cmp!(h.vid(),h.idx()) {
            if g==I { g = f;  f = h;  h = g;  g = I; }
            else    { f = !h; g = O;  h = nf; }}
          else if h.is_const() && cmp!(g.vid(),g.idx()) {
            if h==I { f = !g; g = nf; h = I; }
            else    { h = f;  f = g;  g = h;  h = O; }}
          else {
            let ng = !g;
            if (h==ng) && cmp!(g.vid(), g.idx()) { h=f; f=g; g=h; h=nf; }
            // choose form where first 2 slots are NOT inverted:
            // from { (f,g,h), (¬f,h,g), ¬(f,¬g,¬h), ¬(¬f,¬g,¬h) }
            else if f.is_inv() { f=g; g=h; h=f; f=nf; }
            else if g.is_inv() { return match ITE::norm(f,ng,!h) {
              Norm::Nid(nid) => Norm::Nid(!nid),
              Norm::Not(ite) => Norm::Ite(ite),
              Norm::Ite(ite) => Norm::Not(ite)}}
            else { return Norm::Ite(NormIteKey(ITE::new(f,g,h))) }}}}}} }


/// Finally, we put everything together. This is the top-level type for this crate.
#[derive(Debug)]
pub struct BddBase {
  /// allows us to give user-friendly names to specific nodes in the base.
  pub tags: HashMap<String, NID>,
  pub swarm: BddSwarm} // TODO: nopub

impl BddBase {

  pub fn new()->BddBase { BddBase{swarm: BddSwarm::new(), tags:HashMap::new()}}

  pub fn new_with_threads(n:usize)->BddBase {
    BddBase{swarm: BddSwarm::new_with_threads(n), tags:HashMap::new()}}

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] fn tup(&self, n:NID)->(NID,NID) { self.swarm.tup(n) }

  pub fn get_vhl(&self, n:NID)->(VID,NID,NID) {
    let (hi, lo) = self.tup(n); (n.vid(), hi, lo) }

  // clear all data from the cache (mostly for benchmarks)
  pub fn reset(&mut self) { self.swarm.reset(); }

  pub fn len(&self)->usize { self.swarm.len() }
  #[must_use] pub fn is_empty(&self) -> bool { self.len() == 0 }


  // public node constructors

  pub fn  gt(&mut self, x:NID, y:NID)->NID { self.ite(x, !y, O) }
  pub fn  lt(&mut self, x:NID, y:NID)->NID { self.ite(x, O, y) }

  /// all-purpose node creation/lookup
  #[inline] pub fn ite(&mut self, f:NID, g:NID, h:NID)->NID { self.swarm.ite(f,g,h) }


  /// swap input variables x and y within bdd n
  pub fn swap(&mut self, n:NID, x:VID, y:VID)-> NID {
    if x.is_below(&y) { return self.swap(n,y,x) }
    /*
        x ____                        x'____
        :     \                       :     \
        y __    y __      =>          y'__    y'__
        :   \    :  \                 :   \    :   \
        ll   lh  hl  hh               ll   hl  lh   hh
     */
    let (xlo, xhi) = (self.when_lo(x,n), self.when_hi(x,n));
    let (xlo_ylo, xlo_yhi) = (self.when_lo(y,xlo), self.when_hi(y,xlo));
    let (xhi_ylo, xhi_yhi) = (self.when_lo(y,xhi), self.when_hi(y,xhi));
    let lo = self.ite(NID::from_vid(x), xlo_ylo, xhi_ylo);
    let hi = self.ite(NID::from_vid(y), xlo_yhi, xhi_yhi);
    self.ite(NID::from_vid(x), lo, hi) }

  pub fn node_count(&self, n:NID)->usize {
    let mut c = 0; self.walk_dn(n, &mut |_,_,_,_| c+=1); c }

  // Add solution_count method
  pub fn solution_count(&mut self, n: NID) -> u64 {
    let mut counts = std::collections::HashMap::new();
    self.walk_up(n, &mut |nid, vid, hi, lo| {
      let level = vid.var_ix();
      let hi_count = if hi.is_const() {
        if hi == I { 1 << level } else { 0 }
      } else {
        let hi_level = hi.vid().var_ix();
        counts[&hi] << ((level-1) - hi_level)
      };
      let lo_count = if lo.is_const() {
        if lo == I { 1 << level } else { 0 }
      } else {
        let lo_level = lo.vid().var_ix();
        counts[&lo] << ((level-1) - lo_level)
      };
      counts.insert(nid, hi_count + lo_count);
    });
    counts[&n]
  }


  /// helper for truth table builder
  fn tt_aux(&mut self, res:&mut Vec<u8>, n:NID, i:usize, level:u32) {
    if level == 0 { match n {
      O => {} // res[i] = 0; but this is already the case.
      I => { res[i] = 1; }
      x => panic!("expected a leaf nid, got {}", x) }}
    else {
      let v = VID::var(level-1);
      let lo = self.when_lo(v,n); self.tt_aux(res, lo, i*2, level-1);
      let hi = self.when_hi(v,n); self.tt_aux(res, hi, i*2+1, level-1); }}

  /// Truth table. Could have been `Vec<bool>` but this is mostly for testing
  /// and the literals are much smaller when you type `1` and `0` instead of
  /// `true` and `false`.
  pub fn tt(&mut self, n0:NID, num_vars:u32)->Vec<u8> {
    // !! once the high vars are at the top, we can compare to nid.vid().u() and count down instead of up
    if !n0.vid().is_var() { todo!("tt only works for actual variables. got {:?}", n0); }
    if num_vars > 16 { panic!("refusing to generate a truth table of 2^{} bytes", num_vars) }
    if num_vars == 0 { panic!("num_vars should be > 0")}
    let mut res = vec![0;(1 << num_vars) as usize];
    self.tt_aux(&mut res, n0, 0, num_vars);
    res }

  pub fn get_stats(&mut self)->(u64, u64) {
    self.swarm.get_stats();
    let tests = wip::COUNT_CACHE_TESTS.with(|c| *c.borrow());
    let hits = wip::COUNT_CACHE_HITS.with(|c| *c.borrow());
    (tests, hits)}
}

impl Default for BddBase { fn default() -> Self { Self::new() }}


impl Base for BddBase {

  fn new()->BddBase { BddBase{swarm: BddSwarm::new(), tags:HashMap::new()}}

  /// nid of y when x is high
  fn when_hi(&mut self, x:VID, y:NID)->NID {
    let yv = y.vid();
    match x.cmp_depth(&yv) {
      VidOrdering::Level => self.tup(y).0,  // x ∧ if(x,th,_) → th
      VidOrdering::Above => y,              // y independent of x, so no change. includes yv = I
      VidOrdering::Below => {               // y may depend on x, so recurse.
        let (yt, ye) = self.tup(y);
        let (th,el) = (self.when_hi(x,yt), self.when_hi(x,ye));
        self.ite(NID::from_vid(yv), th, el) }}}

  /// nid of y when x is low
  fn when_lo(&mut self, x:VID, y:NID)->NID {
    let yv = y.vid();
    match x.cmp_depth(&yv) {
      VidOrdering::Level => self.tup(y).1,  // ¬x ∧ if(x,_,el) → el
      VidOrdering::Above => y,              // y independent of x, so no change. includes yv = I
      VidOrdering::Below => {               // y may depend on x, so recurse.
        let (yt, ye) = self.tup(y);
        let (th,el) = (self.when_lo(x,yt), self.when_lo(x,ye));
        self.ite(NID::from_vid(yv), th, el) }}}

  // TODO: these should be moved into seperate struct
  fn def(&mut self, _s:String, _i:VID)->NID { todo!("BddBase::def()") }
  fn tag(&mut self, n:NID, s:String)->NID { self.tags.insert(s, n); n }
  fn get(&self, s:&str)->Option<NID> { Some(*self.tags.get(s)?) }

  fn and(&mut self, x:NID, y:NID)->NID { self.ite(x, y, O) }
  fn xor(&mut self, x:NID, y:NID)->NID { self.ite(x, !y, y) }
  fn  or(&mut self, x:NID, y:NID)->NID { self.ite(x, I, y) }

  /// replace var v with n in ctx
  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID {
    if ctx.might_depend_on(v) {
      let (zt,ze) = self.tup(ctx); let zv = ctx.vid();
      if v==zv { self.ite(n, zt, ze) }
      else {
        let th = self.sub(v, n, zt);
        let el = self.sub(v, n, ze);
        self.ite(NID::from_vid(zv), th, el) }}
    else { ctx }}

  fn _eval_aux(&mut self, n:NID, kv:&HashMap<VID, NID>, cache:&mut HashMap<NID,NID>)->NID {
    if n.is_const() { n }
    else if n.is_vid() { if let Some(&nid) = kv.get(&n.vid()) { nid.inv_if(n.is_inv()) } else { n } }
    else if let Some(&nid) = cache.get(&n.raw()) { nid.inv_if(n.is_inv()) }
    else {
      let (v, hi, lo) = self.get_vhl(n.raw());
      let hi_val = self._eval_aux(hi, kv, cache);
      let lo_val = self._eval_aux(lo, kv, cache);
      let branch = if let Some(&nid) = kv.get(&v) { nid } else { NID::from_vid(v) };
      let mut res = self.ite(branch, hi_val, lo_val);
      cache.insert(n.raw(), res);
      if n.is_inv() { res = !res }
      res }}

  // generate dot file (graphviz)
  fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write) {
    macro_rules! w { ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}
    macro_rules! we { ($src:expr, $dst:expr) => {
      w!("  \"{}\"->\"{}\"{}",$src, $dst,
        (if $dst.is_inv() & !$dst.is_const() { "[arrowhead=dot]" } else {""})) }}
    w!("digraph bdd {{");
    w!("  bgcolor=\"#3399cc\"; pad=0.225");
    w!("  node[shape=circle, style=filled, fillcolor=\"#bbbbbb\", fontname=calibri]");
    w!("  edge[arrowhead=none]");
    w!("  subgraph head {{ h1[shape=plaintext, fillcolor=none, label=\"BDD\"] }}");
    w!("  I[label=⊤, shape=square, fillcolor=white]");
    w!("  O[label=⊥, shape=square, fontcolor=white, fillcolor=\"#333333\"]");
    if n.is_inv() {
      w!("hook[label=\"\",shape=plain,style=invis]; hook->{}:n[arrowhead=dot,penwidth=0,minlen=0,constraint=false]", n); }
    self.walk_dn(n, &mut |n,_,_,_| w!("  \"{}\"[label=\"{}\"];", n, n.vid()));
    w!("edge[style=solid];");
    self.walk_dn(n, &mut |n,_,t,_| we!(n, t));
    w!("edge[style=dashed];");
    self.walk_dn(n, &mut |n,_,_,e| we!(n, e));
    w!("}}"); }

  fn init_stats(&mut self) {
    wip::COUNT_CACHE_TESTS.with(|c| c.replace(0));
    wip::COUNT_CACHE_HITS.with(|c| c.replace(0)); }

  fn print_stats(&mut self) {
    let (tests, hits) = self. get_stats();
    println!("Cache stats: {hits} hits / {tests} tests ({:.1}%).",
      (hits as f64/tests as f64) * 100.0); }

  fn solution_set(&self, n: NID, nvars: usize)->HashSet<Reg> {
    self.solutions_pad(n, nvars).collect() }}



include!("test-bdd.rs");