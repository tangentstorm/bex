///! A module for efficient implementation of binary decision diagrams.
use std::collections::HashMap;
use std::collections::HashSet;
use std::cell::RefCell;

extern crate num_cpus;

use bincode;
use base::{Base};
use io;
use reg::Reg;
use {vhl, vhl::{HiLo, Walkable}};
use nid::{NID,O,I};
use vid::{VID,VidOrdering,topmost_of3};

mod bdd_sols;
mod bdd_swarm; use self::bdd_swarm::*;

/// Type alias for whatever HashMap implementation we're curretly using -- std,
/// fnv, hashbrown... Hashing is an extremely important aspect of a BDD base, so
/// it's useful to have a single place to configure this.
pub type BDDHashMap<K,V> = vhl::VHLHashMap<K,V>;


/// An if/then/else triple. Like VHL, but all three slots are NIDs.
#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone, Copy)]
pub struct ITE {i:NID, t:NID, e:NID}
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
  Ite(ITE),
  /// a normalized, inverted ITE.
  Not(ITE)}


impl ITE {
  /// choose normal form for writing this triple. Algorithm based on:
  /// "Efficient Implementation of a BDD Package"
  /// http://www.cs.cmu.edu/~emc/15817-f08/bryant-bdd-1991.pdf
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
            else { return Norm::Ite(ITE::new(f,g,h)) }}}}}} }


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BddState {
  /// cache of hi,lo pairs.
  hilos: vhl::HiLoCache,
  /// arbitrary memoization. These record normalized (f,g,h) lookups.
  xmemo: BDDHashMap<ITE, NID> }

// cache lookup counters:
thread_local!{
  pub static COUNT_XMEMO_TEST: RefCell<u64> = RefCell::new(0);
  pub static COUNT_XMEMO_FAIL: RefCell<u64> = RefCell::new(0); }


impl BddState {
  /// constructor
  fn new()->BddState { BddState { hilos: vhl::HiLoCache::new(), xmemo: BDDHashMap::default() }}

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] fn tup(&self, n:NID)-> (NID, NID) {
    if n.is_const() { if n==I { (I, O) } else { (O, I) } }
    else if n.is_vid() { if n.is_inv() { (O, I) } else { (I, O) }}
    else { let hilo = self.hilos.get_hilo(n); (hilo.hi, hilo.lo) }}

  /// fetch or create a "simple" node, where the hi and lo branches are both
  /// already fully computed pointers to existing nodes.
  #[inline] fn simple_node(&mut self, v:VID, hilo:HiLo)->NID {
    match self.get_simple_node(v, hilo) {
      Some(n) => n,
      None => { self.hilos.insert(v, hilo) }}}

  /// load the memoized NID if it exists
  #[inline] fn get_memo(&self, ite:&ITE) -> Option<NID> {
    if ite.i.is_vid() {
      debug_assert!(!ite.i.is_inv()); // because it ought to be normalized by this point.
      let hilo = if ite.i.is_inv() { HiLo::new(ite.e,ite.t) } else { HiLo::new(ite.t,ite.e) };
      self.get_simple_node(ite.i.vid(), hilo) }
    else {
      COUNT_XMEMO_TEST.with(|c| *c.borrow_mut() += 1 );
      let test = self.xmemo.get(&ite).copied();
      if test == None { COUNT_XMEMO_FAIL.with(|c| *c.borrow_mut() += 1 ); }
      test }}

  #[inline] fn get_simple_node(&self, v:VID, hl:HiLo)-> Option<NID> {
    self.hilos.get_node(v, hl) }}


/// Finally, we put everything together. This is the top-level type for this crate.
#[derive(Debug, Serialize, Deserialize)]
pub struct BDDBase {
  /// allows us to give user-friendly names to specific nodes in the base.
  pub tags: HashMap<String, NID>,
  swarm: BddSwarm}

impl BDDBase {

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] fn tup(&self, n:NID)->(NID,NID) { self.swarm.tup(n) }

  pub fn load(path:&str)->::std::io::Result<BDDBase> {
    let s = io::get(path)?;
    Ok(bincode::deserialize(&s).unwrap()) }

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
    let mut c = 0; self.walk(n, &mut |_,_,_,_| c+=1); c }

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

  /// Truth table. Could have been Vec<bool> but this is mostly for testing
  /// and the literals are much smaller when you type '1' and '0' instead of
  /// 'true' and 'false'.
  pub fn tt(&mut self, n0:NID, num_vars:u32)->Vec<u8> {
    // !! once the high vars are at the top, we can compare to nid.vid().u() and count down instead of up
    if !n0.vid().is_var() { todo!("tt only works for actual variables. got {:?}", n0); }
    if num_vars > 16 { panic!("refusing to generate a truth table of 2^{} bytes", num_vars) }
    if num_vars == 0 { panic!("num_vars should be > 0")}
    let mut res = vec![0;(1 << num_vars) as usize];
    self.tt_aux(&mut res, n0, 0, num_vars);
    res }

} // end impl BDDBase


impl Base for BDDBase {

  fn new()->BDDBase { BDDBase{swarm: BddSwarm::new(), tags:HashMap::new()}}

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
  fn def(&mut self, _s:String, _i:VID)->NID { todo!("BDDBase::def()") }
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

  fn save(&self, path:&str)->::std::io::Result<()> {
    let s = bincode::serialize(&self).unwrap();
    io::put(path, &s) }

  // generate dot file (graphviz)
  fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write) {
    macro_rules! w { ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap(); }}
    w!("digraph bdd {{");
    w!("subgraph head {{ h1[shape=plaintext; label=\"BDD\"] }}");
    w!("  I[label=⊤; shape=square];");
    w!("  O[label=⊥; shape=square];");
    w!("node[shape=circle];");
    self.walk(n, &mut |n,_,_,_| w!("  \"{}\"[label=\"{}\"];", n, n.vid()));
    w!("edge[style=solid];");
    self.walk(n, &mut |n,_,t,_| w!("  \"{}\"->\"{}\";", n, t));
    w!("edge[style=dashed];");
    self.walk(n, &mut |n,_,_,e| w!("  \"{}\"->\"{}\";", n, e));
    w!("}}"); }

  fn solution_set(&self, n: NID, nvars: usize)->hashbrown::HashSet<Reg> {
    self.solutions_pad(n, nvars).collect() }}



include!("test-bdd.rs");