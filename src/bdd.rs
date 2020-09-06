///! A module for efficient implementation of binary decision diagrams.
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

extern crate num_cpus;

use serde::{Serialize, Serializer, Deserialize, Deserializer};
use bincode;
use base::Base;
use io;
use reg::Reg;
use {vhl, vhl::{HiLo, HiLoPart, HiLoBase, VHLParts}};
use nid::{NID,O,I};
use vid::{VID,VidOrdering,topmost_of3};
use cur::{Cursor, CursorPlan};
use {wip, wip::{QID,Dep,WIP,WorkState}};

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
        return if g==I { return Norm::Nid(f) } else { Norm::Nid(!f) }}
      else {
        let nf = !f;
        if      g==nf { g=O }
        else if h==f  { h=O }
        else if h==nf { h=I }
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
  /// number of variables
  nvars: usize,
  /// cache of hi,lo pairs.
  hilos: vhl::HiLoCache,
  /// arbitrary memoization. These record normalized (f,g,h) lookups.
  xmemo: BDDHashMap<ITE, NID> }


impl BddState {

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] fn tup(&self, n:NID)-> (NID, NID) {
    if n.is_const() { if n==I { (I, O) } else { (O, I) } }
    else if n.is_var() { if n.is_inv() { (O, I) } else { (I, O) }}
    else { let hilo = self.get_hilo(n); (hilo.hi, hilo.lo) }}

  /// fetch or create a "simple" node, where the hi and lo branches are both
  /// already fully computed pointers to existing nodes.
  #[inline] fn simple_node(&mut self, v:VID, hilo:HiLo)->NID {
    match self.get_simple_node(v, hilo) {
      Some(n) => n,
      None => { self.put_simple_node(v, hilo) }}}

  /// constructor
  fn new(nvars:usize)->BddState {
    BddState {
      nvars,
      hilos: vhl::HiLoCache::new(),
      xmemo: BDDHashMap::default() }}

  #[inline] fn put_xmemo(&mut self, ite:ITE, new_nid:NID) {
    self.xmemo.insert(ite, new_nid); }

  /// load the memoized NID if it exists
  #[inline] fn get_memo(&self, ite:&ITE) -> Option<NID> {
    if ite.i.is_var() {
      debug_assert!(!ite.i.is_inv()); // because it ought to be normalized by this point.
      let hilo = if ite.i.is_inv() { HiLo::new(ite.e,ite.t) } else { HiLo::new(ite.t,ite.e) };
      self.get_simple_node(ite.i.vid(), hilo) }
    else { self.xmemo.get(&ite).copied() }}

  #[inline] fn get_hilo(&self, n:NID)->HiLo {
    self.hilos.get_hilo(n) }

  #[inline] fn get_simple_node(&self, v:VID, hl:HiLo)-> Option<NID> {
    self.hilos.get_node(v, hl)}

  #[inline] fn put_simple_node(&mut self, v:VID, hl:HiLo)->NID {
    self.hilos.insert(v, hl) }}


// ----------------------------------------------------------------
// Helper types for BddSwarm
// ----------------------------------------------------------------

/// Query message for BddSwarm.
enum QMsg { Ite(QID, ITE), Cache(Arc<BddState>) }
impl std::fmt::Debug for QMsg {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      QMsg::Ite(qid, ite) => { write!(f, "Ite(q{}, {:?})", qid, ite) }
      QMsg::Cache(_) => { write!(f, "QMsg::Cache") } } }}

type RMsg = wip::RMsg<Norm>;

/// Sender for QMsg
type QTx = Sender<QMsg>;
/// Receiver for QMsg
type QRx = Receiver<QMsg>;
/// Sender for RMsg
type RTx = Sender<(QID, RMsg)>;
/// Receiver for RMsg
type RRx = Receiver<(QID, RMsg)>;


// ----------------------------------------------------------------
/// BddSwarm: a multi-threaded swarm implementation
// ----------------------------------------------------------------
#[derive(Debug)]
pub struct BddSwarm {
  /// receives messages from the threads
  rx: RRx,
  /// send messages to myself (so we can put them back in the queue.
  me: RTx,
  /// QMsg senders for each thread, so we can send queries to work on.
  swarm: Vec<QTx>,
  /// read-only version of the state shared by all threads.
  stable: Arc<BddState>,
  /// mutable version of the state kept by the main thread.
  recent: BddState,
  // work in progress
  work: WorkState<ITE>}

impl Serialize for BddSwarm {
  fn serialize<S:Serializer>(&self, ser: S)->Result<S::Ok, S::Error> {
    // all we really care about is the state:
    self.stable.serialize::<S>(ser) } }

impl<'de> Deserialize<'de> for BddSwarm {
  fn deserialize<D:Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
    let mut res = Self::new(0);
    res.stable = Arc::new(BddState::deserialize(d)?);
    Ok(res) }}


impl BddSwarm {

  fn new(nvars:usize)->Self {
    let (me, rx) = channel::<(QID, RMsg)>();
    let swarm = vec![];
    let stable = Arc::new(BddState::new(nvars));
    let recent = BddState::new(nvars);
    Self{ me, rx, swarm, stable, recent, work:WorkState::new() }}

  fn get_state(&self)->&BddState { &self.recent }

  fn nvars(&self)->usize { self.recent.nvars }

  fn tup(&self, n:NID)->(NID,NID) { self.recent.tup(n) }

  /// all-purpose if-then-else node constructor. For the swarm implementation,
  /// we push all the normalization and tree traversal work into the threads,
  /// while this function puts all the parts together.
  fn ite(&mut self, i:NID, t:NID, e:NID)->NID { self.run_swarm(i,t,e) } }


impl BddSwarm {

  /// add a new task for the swarm to work on. (if it's a duplicate, we just
  /// add the dependencies to the original task (unless it's already finished,
  /// in which case we resolve immediately))
  fn add_task(&mut self, opt_dep:Option<Dep>, ite:ITE) {
    trace!("add_task({:?}, {:?})", opt_dep, ite);
    let (qid, is_dup) = {
      if let Some(&dup) = self.work.qid.get(&ite) { (dup, true) }
      else { (self.work.wip.len(), false) }};
    if is_dup {
      if let Some(dep) = opt_dep {
        trace!("*** task {:?} is dup of q{} invert: {}", ite, qid, dep.invert);
        if let WIP::Done(nid) = self.work.wip[qid] {
          self.resolve_part(dep.qid, dep.part, nid, dep.invert); }
        else { self.work.deps[qid].push(dep) }}
      else { panic!("Got duplicate request, but no dep. This should never happen!") }}
    else {
      self.work.qid.insert(ite, qid); self.work.qs.push(ite);
      let w:usize = qid % self.swarm.len();
      self.swarm[w].send(QMsg::Ite(qid, ite)).expect("send to swarm failed");
      self.work.wip.push(WIP::Fresh);
      if let Some(dep) = opt_dep {
        trace!("*** added task #{}: {:?} invert:{}", qid, ite, dep.invert);
        self.work.deps.push(vec![dep]) }
      else if qid == 0 {
        trace!("*** added task #{}: {:?} (no deps!)", qid, ite);
        self.work.deps.push(vec![]) }
      else { panic!("non 0 qid with no deps!?") }}}

  /// called whenever the wip resolves to a single nid
  fn resolve_nid(&mut self, qid:QID, nid:NID) {
    trace!("resolve_nid(q{}, {})", qid, nid);
    if let WIP::Done(old) = self.work.wip[qid] {
      warn!("resolving already resolved nid for q{}", qid);
      assert_eq!(old, nid, "old and new resolutions didn't match!") }
    else {
      trace!("resolved_nid: q{}=>{}. deps: {:?}", qid, nid, self.work.deps[qid].clone());
      self.work.wip[qid] = WIP::Done(nid);
      let ite = self.work.qs[qid];
      self.recent.put_xmemo(ite, nid);
      for &dep in self.work.deps[qid].clone().iter() {
        self.resolve_part(dep.qid, dep.part, nid, dep.invert) }
      if qid == 0 { self.me.send((0, RMsg::Ret(nid))).expect("failed to send Ret"); }}}

  /// called whenever the wip resolves to a new simple (v/hi/lo) node.
  fn resolve_vhl(&mut self, qid:QID, v:VID, hilo:HiLo, invert:bool) {
    trace!("resolve_vhl(q{}, {:?}, {:?}, invert:{}", qid, v, hilo, invert);
    let HiLo{hi:h0,lo:l0} = hilo;
    // we apply invert first so it normalizes correctly.
    let (h1,l1) = if invert { (!h0, !l0) } else { (h0, l0) };
    let nid = match ITE::norm(NID::from_vid(v), h1, l1) {
      Norm::Nid(n) => n,
      Norm::Ite(ITE{i:vv,t:hi,e:lo}) =>  self.recent.simple_node(vv.vid(), HiLo{hi,lo}),
      Norm::Not(ITE{i:vv,t:hi,e:lo}) => !self.recent.simple_node(vv.vid(), HiLo{hi,lo})};
    trace!("resolved vhl: q{}=>{}. #deps: {}", qid, nid, self.work.deps[qid].len());
    self.resolve_nid(qid, nid); }

  fn resolve_part(&mut self, qid:QID, part:HiLoPart, nid:NID, invert:bool) {
    self.work.resolve_part(qid, part, nid, invert);
    if let WIP::Parts(wip) = self.work.wip[qid] {
      if let Some(hilo) = wip.hilo() { self.resolve_vhl(qid, wip.v, hilo, wip.invert) }}}

  /// initialization logic for running the swarm. spawns threads and copies latest cache.
  fn init_swarm(&mut self) {
    self.work.wip = vec![]; self.work.qs = vec![]; self.work.deps = vec![]; self.work.qid = BDDHashMap::new();
    // wipe out and replace the channels so un-necessary work from last iteration
    // (that was still going on when we returned a value) gets ignored..
    let (me, rx) = channel::<(QID, RMsg)>(); self.me = me; self.rx = rx;
    self.swarm = vec![];
    while self.swarm.len() < num_cpus::get() {
      let (tx, rx) = channel::<QMsg>();
      let me_clone = self.me.clone();
      let state = self.stable.clone();
      thread::spawn(move || swarm_loop(me_clone, rx, state));
      self.swarm.push(tx); }
    self.stable = Arc::new(self.recent.clone());
    for tx in self.swarm.iter() {
      tx.send(QMsg::Cache(self.stable.clone())).expect("failed to send QMsg::Cache"); }}

  /// distrubutes the standard ite() operatation across a swarm of threads
  fn run_swarm(&mut self, i:NID, t:NID, e:NID)->NID {
    macro_rules! run_swarm_ite { ($ite:expr) => {{
      self.init_swarm(); self.add_task(None, $ite);
      let mut result:Option<NID> = None;
      // each response can lead to up to two new ITE queries, and we'll relay those to
      // other workers too, until we get back enough info to solve the original query.
      while result.is_none() {
        let (qid, rmsg) = self.rx.recv().expect("failed to read RMsg from queue!");
        trace!("===> run_swarm got RMsg {}: {:?}", qid, rmsg);
        match rmsg {
          RMsg::Nid(nid) =>  { self.resolve_nid(qid, nid); }
          RMsg::Vhl{v,hi,lo,invert} => { self.resolve_vhl(qid, v, HiLo{hi, lo}, invert); }
          RMsg::Wip{v,hi,lo,invert} => {
            // by the time we get here, the task for this node was already created.
            // (add_task already filled in the v for us, so we don't need it.)
            assert_eq!(self.work.wip[qid], WIP::Fresh);
            self.work.wip[qid] = WIP::Parts(VHLParts{ v, hi:None, lo:None, invert });
            macro_rules! handle_part { ($xx:ident, $part:expr) => {
              match $xx {
                Norm::Nid(nid) => self.resolve_part(qid, $part, nid, false),
                Norm::Ite(ite) => self.add_task(Some(Dep::new(qid, $part, false)), ite),
                Norm::Not(ite) => self.add_task(Some(Dep::new(qid, $part, true)), ite)}}}
            handle_part!(hi, HiLoPart::HiPart); handle_part!(lo, HiLoPart::LoPart); }
          RMsg::Ret(n) => { result = Some(n) }}}
      result.unwrap() }}}
    // TODO: at some point, we should kill all the child threads.
    match ITE::norm(i,t,e) {
      Norm::Nid(n) => n,
      Norm::Ite(ite) => { run_swarm_ite!(ite) }
      Norm::Not(ite) => { !run_swarm_ite!(ite) }}}
} // end bddswarm

/// Code run by each thread in the swarm. Isolated as a function without channels for testing.
fn swarm_ite(state: &Arc<BddState>, ite0:ITE)->RMsg {
  let ITE { i, t, e } = ite0;
  match ITE::norm(i,t,e) {
      Norm::Nid(n) => RMsg::Nid(n),
      Norm::Ite(ite) => swarm_ite_norm(state, ite),
      Norm::Not(ite) => !swarm_ite_norm(state, ite) }}

fn swarm_vhl_norm(state: &Arc<BddState>, ite:ITE)->RMsg {
  let ITE{i:vv,t:hi,e:lo} = ite; let v = vv.vid();
  if let Some(n) = state.get_simple_node(v, HiLo{hi,lo}) { RMsg::Nid(n) }
  else { RMsg::Vhl{ v, hi, lo, invert:false } }}

fn swarm_ite_norm(state: &Arc<BddState>, ite:ITE)->RMsg {
  let ITE { i, t, e } = ite;
  let (vi, vt, ve) = (i.vid(), t.vid(), e.vid());
  let v = ite.top_vid();
  match state.get_memo(&ite) {
    Some(n) => RMsg::Nid(n),
    None => {
      let (hi_i, lo_i) = if v == vi {state.tup(i)} else {(i,i)};
      let (hi_t, lo_t) = if v == vt {state.tup(t)} else {(t,t)};
      let (hi_e, lo_e) = if v == ve {state.tup(e)} else {(e,e)};
      // now construct and normalize the queries for the hi/lo branches:
      let hi = ITE::norm(hi_i, hi_t, hi_e);
      let lo = ITE::norm(lo_i, lo_t, lo_e);
      // if they're both simple nids, we're guaranteed to have a vhl, so check cache
      if let (Norm::Nid(hn), Norm::Nid(ln)) = (hi,lo) {
        match ITE::norm(NID::from_vid(v), hn, ln) {
          // first, it might normalize to a nid directly:
          Norm::Nid(n) => { RMsg::Nid(n) }
          // otherwise, the normalized triple might already be in cache:
          Norm::Ite(ite) => swarm_vhl_norm(state, ite),
          Norm::Not(ite) => !swarm_vhl_norm(state, ite)}}
      // otherwise at least one side is not a simple nid yet, and we have to defer
      else { RMsg::Wip{ v, hi, lo, invert:false } }}}}


/// This is the loop run by each thread in the swarm.
fn swarm_loop(tx:RTx, rx:QRx, mut state:Arc<BddState>) {
  for qmsg in rx.iter() {
    match qmsg {
      QMsg::Cache(s) => { state = s }
      QMsg::Ite(qid, ite) => {
        trace!("--->   thread worker got qmsg {}: {:?}", qid, qmsg);
        let rmsg = swarm_ite(&state, ite);
        if tx.send((qid, rmsg)).is_err() { break } }}}}


/// Finally, we put everything together. This is the top-level type for this crate.
#[derive(Debug, Serialize, Deserialize)]
pub struct BDDBase {
  /// allows us to give user-friendly names to specific nodes in the base.
  pub tags: HashMap<String, NID>,
  swarm: BddSwarm}

impl BDDBase {

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] fn tup(&self, n:NID)->(NID,NID) { self.swarm.tup(n) }

  /// walk node recursively, without revisiting shared nodes
  pub fn walk<F>(&self, n:NID, f:&mut F) where F: FnMut(NID,VID,NID,NID) {
    let mut seen = HashSet::new();
    self.step(n,f,&mut seen)}

  /// internal helper: one step in the walk.
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>)
  where F: FnMut(NID,VID,NID,NID) {
    if !seen.contains(&n) {
      seen.insert(n); let (hi,lo) = self.tup(n); f(n,n.vid(),hi,lo);
      if !hi.is_const() { self.step(hi, f, seen); }
      if !lo.is_const() { self.step(lo, f, seen); }}}

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
  fn tt_aux(&mut self, res:&mut Vec<u8>, v:VID, n:NID, i:usize) {
    let o = v.var_ix();
    if o == self.num_vars() { match self.when_lo(v, n) {
      O => {} // res[i] = 0; but this is already the case.
      I => { res[i] = 1; }
      x => panic!("expected a leaf nid, got {}", x) }}
    else {
      let lo = self.when_lo(v,n); self.tt_aux(res, VID::var(1+o as u32), lo, i*2);
      let hi = self.when_hi(v,n); self.tt_aux(res, VID::var(1+o as u32), hi, i*2+1); }}

  /// Truth table. Could have been Vec<bool> but this is mostly for testing
  /// and the literals are much smaller when you type '1' and '0' instead of
  /// 'true' and 'false'.
  pub fn tt(&mut self, n0:NID)->Vec<u8> {
    // !! once the high vars are at the top, we can compare to nid.vid().u() and count down instead of up
    if !n0.vid().is_var() { todo!("tt only works for actual variables. got {:?}", n0); }
    if self.num_vars() > 16 {
      panic!("refusing to generate a truth table of 2^{} bytes", self.num_vars()) }
    let mut res = vec![0;(1 << self.num_vars()) as usize];
    self.tt_aux(&mut res, VID::var(0), n0, 0);
    res }

} // end impl BDDBase


impl Base for BDDBase {

  fn new(nvars:usize)->BDDBase {
    BDDBase{swarm: BddSwarm::new(nvars), tags:HashMap::new()}}

  /// accessor for number of variables
  fn num_vars(&self)->usize { self.swarm.nvars() }

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

  #[cfg(todo)] fn mj(&mut self, x:NID, y:NID, z:NID)->NID { self.xor(x, self.xor(y, z)) }  // TODO: normalize order. make this the default impl.
  #[cfg(todo)] fn ch(&mut self, x:NID, y:NID, z:NID)->NID { self.ite(x, y, z) }

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
    w!("}}"); }}

// generic Base test suite
test_base_consts!(BDDBase);
test_base_when!(BDDBase);

// basic test suite

#[test] fn test_base() {
  let mut base = BDDBase::new(3);
  let (v1, v2, v3) = (NID::var(1), NID::var(2), NID::var(3));
  assert_eq!(base.num_vars(), 3);
  assert_eq!((I,O), base.tup(I));
  assert_eq!((O,I), base.tup(O));
  assert_eq!((I,O), base.tup(v1));
  assert_eq!((I,O), base.tup(v2));
  assert_eq!((I,O), base.tup(v3));
  assert_eq!(I, base.when_hi(VID::var(3),v3));
  assert_eq!(O, base.when_lo(VID::var(3),v3))}

#[test] fn test_and() {
  let mut base = BDDBase::new(3);
  let (v1, v2) = (NID::var(1), NID::var(2));
  let a = base.and(v1, v2);
  assert_eq!(O,  base.when_lo(VID::var(1),a));
  assert_eq!(v2, base.when_hi(VID::var(1),a));
  assert_eq!(O,  base.when_lo(VID::var(2),a));
  assert_eq!(v1, base.when_hi(VID::var(2),a));
  assert_eq!(a,  base.when_hi(VID::var(3),a));
  assert_eq!(a,  base.when_lo(VID::var(3),a))}

#[test] fn test_xor() {
  let mut base = BDDBase::new(3);
  let (v1, v2) = (NID::var(1), NID::var(2));
  let x = base.xor(v1, v2);
  assert_eq!(v2,  base.when_lo(VID::var(1),x));
  assert_eq!(!v2, base.when_hi(VID::var(1),x));
  assert_eq!(v1,  base.when_lo(VID::var(2),x));
  assert_eq!(!v1, base.when_hi(VID::var(2),x));
  assert_eq!(x,   base.when_lo(VID::var(3),x));
  assert_eq!(x,   base.when_hi(VID::var(3),x))}

// swarm test suite
#[test] fn test_swarm_xor() {
  let mut base = BDDBase::new(2);
  let (x0, x1) = (NID::var(0), NID::var(1));
  let x = base.xor(x0, x1);
  assert_eq!(x1,  base.when_lo(VID::var(0),x));
  assert_eq!(!x1, base.when_hi(VID::var(0),x));
  assert_eq!(x0,  base.when_lo(VID::var(1),x));
  assert_eq!(!x0, base.when_hi(VID::var(1),x));
  assert_eq!(x,   base.when_lo(VID::var(2),x));
  assert_eq!(x,   base.when_hi(VID::var(2),x))}

#[test] fn test_swarm_and() {
  let mut base = BDDBase::new(2);
  let (x0, x1) = (NID::var(0), NID::var(1));
  let a = base.and(x0, x1);
  assert_eq!(O,  base.when_lo(VID::var(0),a));
  assert_eq!(x1, base.when_hi(VID::var(0),a));
  assert_eq!(O,  base.when_lo(VID::var(1),a));
  assert_eq!(x0, base.when_hi(VID::var(1),a));
  assert_eq!(a,  base.when_hi(VID::var(2),a));
  assert_eq!(a,  base.when_lo(VID::var(2),a))}

/// slightly harder test case that requires ite() to recurse
#[test] fn test_swarm_ite() {
  //use simplelog::*;  TermLogger::init(LevelFilter::Trace, Config::default()).unwrap();
  let mut base = BDDBase::new(3);
  let (x0,x1,x2) = (NID::var(0), NID::var(1), NID::var(2));
  assert_eq!(vec![0,0,0,0,1,1,1,1], base.tt(x0));
  assert_eq!(vec![0,0,1,1,0,0,1,1], base.tt(x1));
  assert_eq!(vec![0,1,0,1,0,1,0,1], base.tt(x2));
  let x = base.xor(x0, x1);
  assert_eq!(vec![0,0,1,1,1,1,0,0], base.tt(x));
  let a = base.and(x1, x2);
  assert_eq!(vec![0,0,0,1,0,0,0,1], base.tt(a));
  let i = base.ite(x, a, !a);
  assert_eq!(vec![1,1,0,1,0,0,1,0], base.tt(i))}


/// slightly harder test case that requires ite() to recurse
#[test] fn test_swarm_another() {
  use simplelog::*;  TermLogger::init(LevelFilter::Trace, Config::default()).unwrap();
  let mut base = BDDBase::new(4);
  let (a,b) = (NID::var(0), NID::var(1));
  let anb = base.and(a,!b);
  assert_eq!(vec![0,0,0,0,0,0,0,0,1,1,1,1,0,0,0,0], base.tt(anb));

  let anb_nb = base.xor(anb,!b);
  assert_eq!(vec![1,1,1,1,0,0,0,0,0,0,0,0,0,0,0,0], base.tt(anb_nb));
  let anb2 = base.xor(!b, anb_nb);
  assert_eq!(vec![0,0,0,0,0,0,0,0,1,1,1,1,0,0,0,0], base.tt(anb2));
  assert_eq!(anb, anb2)}


use  std::iter::FromIterator; use std::hash::Hash;
pub fn hs<T: Eq+Hash>(xs: Vec<T>)->HashSet<T> { <HashSet<T>>::from_iter(xs) }

/// Test cases for SolutionIterator
#[test] fn test_bdd_solutions_o() {
  let mut base = BDDBase::new(2);  let mut it = base.solutions(O);
  assert_eq!(it.next(), None, "const O should yield no solutions.") }

#[test] fn test_bdd_solutions_i() {
  let mut base = BDDBase::new(2);
  let actual:HashSet<usize> = base.solutions(I).map(|r| r.as_usize()).collect();
  assert_eq!(actual, hs(vec![0b00, 0b01, 0b10, 0b11]),
     "const true should yield all solutions"); }

#[test] fn test_bdd_solutions_simple() {
  let mut base = BDDBase::new(1); let a = NID::var(0);
  let mut it = base.solutions(a);
  // it should be sitting on first solution, which is a=1
  assert_eq!(it.next().expect("expected solution!").as_usize(), 0b1);
  assert_eq!(it.next(), None);}


#[test] fn test_bdd_solutions_extra() {
  let mut base = BDDBase::new(5);
  let (b, d) = (NID::var(1), NID::var(3));
  // the idea here is that we have "don't care" above, below, and between the used vars:
  let n = base.and(b,d);
  let actual:Vec<_> = base.solutions(n).map(|r| r.as_usize()).collect();
                          //abcde
  assert_eq!(actual, vec![0b01010,
                          0b01011,
                          0b01110,
                          0b01111,
                          0b11010,
                          0b11011,
                          0b11110,
                          0b11111])}

#[test] fn test_bdd_solutions_xor() {
  let mut base = BDDBase::new(3);
  let (a, b) = (NID::var(0), NID::var(1));
  let n = base.xor(a, b);
  // base.show(n);
  let actual:Vec<usize> = base.solutions(n).map(|x|x.as_usize()).collect();
  let expect = vec![0b001, 0b010, 0b101, 0b110 ]; // bits cba
  assert_eq!(actual, expect); }

impl BDDBase {
  pub fn solutions(&mut self, n:NID)->BDDSolIterator {
    self.solutions_trunc(n, self.num_vars())}

  pub fn solutions_trunc(&self, n:NID, nvars:usize)->BDDSolIterator {
    assert!(nvars <= self.num_vars(), "nvars arg to solutions_trunc must be <= self.num_vars");
    BDDSolIterator::from_bdd(self, n, nvars)}}


/// helpers for solution cursor
impl HiLoBase for BDDBase {
  fn get_hilo(&self, n:NID)->Option<HiLo> {
    let (hi, lo) = self.swarm.get_state().tup(n);
    Some(HiLo{ hi, lo }) }}

impl CursorPlan for BDDBase {}

impl BDDBase {
  pub fn first_solution(&self, n:NID, nvars:usize)->Option<Cursor> {
    if n== O || nvars == 0 { None }
    else {
      let mut cur = Cursor::new(nvars, n);
      cur.descend(self);
      debug_assert!(cur.node.is_const());
      debug_assert!(self.in_solution(&cur), format!("{:?}", cur.scope));
      Some(cur) }}

  pub fn next_solution(&self, cur:Cursor)->Option<Cursor> {
    self.log(&cur, "advance>"); self.log_indent(1);
    let res = self.advance0(cur); self.log_indent(-1);
    res }

  /// is the cursor currently pointing at a span of 1 or more solutions?
  pub fn in_solution(&self, cur:&Cursor)->bool {
    self.includes_leaf(cur.node) }

  fn log_indent(&self, _d:i8) { /*self.indent += d;*/ }
  fn log(&self, _c:&Cursor, _msg: &str) {
    #[cfg(test)]{
      print!(" {}", if _c.invert { '¬' } else { ' ' });
      print!("{:>10}", format!("{}", _c.node));
      print!(" {:?}{}", _c.scope, if self.in_solution(&_c) { '.' } else { ' ' });
      let s = format!("{}", /*"{}", "  ".repeat(self.indent as usize),*/ _msg,);
      println!(" {:50} {:?}", s, _c.nstack);}}

  /// walk depth-first from lo to hi until we arrive at the next solution
  fn find_next_leaf(&self, cur:&mut Cursor)->Option<NID> {
    self.log(cur, "find_next_leaf"); self.log_indent(1);
    let res = self.find_next_leaf0(cur);
    self.log(cur, format!("^ next leaf: {:?}", res.clone()).as_str());
    self.log_indent(-1); res }

  fn find_next_leaf0(&self, cur:&mut Cursor)->Option<NID> {
    // we always start at a leaf and move up, with the one exception of root=I
    assert!(cur.node.is_const(), "find_next_leaf should always start by looking at a leaf");
    if cur.nstack.is_empty() { assert!(cur.node == I); return None }

    // now we are definitely at a leaf node with a branch above us.
    cur.step_up();

    let tv = cur.node.vid(); // branching var for current twig node
    let mut rippled = false;
    // if we've already walked the hi branch...
    if cur.scope.var_get(tv) {
      cur.go_next_lo_var();
      // if we've cleared the stack and already explored the hi branch...
      { let iv = cur.node.vid();
        if cur.nstack.is_empty() && cur.scope.var_get(iv) {
          // ... then first check if there are any variables above us on which
          // the node doesn't actually depend. ifso: ripple add. else: done.
          let top = cur.nvars-1;
          if let Some(x) = cur.scope.ripple(iv.var_ix(), top) {
            rippled = true;
            self.log(cur, format!("rippled top to {}. restarting.", x).as_str()); }
          else { self.log(cur, "no next leaf!"); return None }}} }

    if rippled { cur.clear_trailing_bits() }
    else if cur.var_get() { self.log(cur, "done with node."); return None }
    else { cur.put_step(self, true); }
    cur.descend(self);
    Some(cur.node) }

  /// walk depth-first from lo to hi until we arrive at the next solution
  fn advance0(&self, mut cur:Cursor)->Option<Cursor> {
    assert!(cur.node.is_const(), "advance should always start by looking at a leaf");
    if self.in_solution(&cur) {
      // if we're in the solution, we're going to increment the "counter".
      if let Some(zpos) = cur.increment() {
        self.log(&cur, format!("rebranch on {:?}",zpos).as_str());
        // The 'zpos' variable exists in the solution space, but there might or might
        // not be a branch node for that variable in the current bdd path.
        // Whether we follow the hi or lo branch depends on which variable we're looking at.
        if cur.node.is_const() { return Some(cur) } // special case for topmost I (all solutions)
        cur.put_step(self, cur.var_get());
        cur.descend(self); }
      else { // overflow. we've counted all the way to 2^nvars-1, and we're done.
        self.log(&cur, "$ found all solutions!"); return None }}
    // If still here, we are looking at a leaf that isn't a solution (out=0 in truth table)
    while !self.in_solution(&cur) { self.find_next_leaf(&mut cur)?; }
    Some(cur) }
}

pub struct BDDSolIterator<'a> {
  bdd: &'a BDDBase,
  next: Option<Cursor>}

impl<'a> BDDSolIterator<'a> {
  pub fn from_bdd(bdd: &'a BDDBase, n:NID, nvars:usize)->BDDSolIterator<'a> {
    // init scope with all variables assigned to 0
    let next = bdd.first_solution(n, nvars);
    BDDSolIterator{ bdd, next }}}


impl<'a> Iterator for BDDSolIterator<'a> {
  type Item = Reg;
  fn next(&mut self)->Option<Self::Item> {
    if let Some(cur) = self.next.take() {
      assert!(self.bdd.in_solution(&cur));
      let result = cur.scope.clone();
      self.next = self.bdd.next_solution(cur);
      Some(result)}
    else { None }}}



#[test] fn test_simple_nodes() {
  let mut state = BddState::new(8);
  let hl = HiLo::new(NID::var(5), NID::var(6));
  let x0 = VID::var(0);
  let v0 = VID::vir(0);
  let v1 = VID::vir(1);
  assert!(state.get_simple_node(v0, hl).is_none());
  let nv0 = state.put_simple_node(v0, hl);
  assert_eq!(nv0, NID::from_vid_idx(v0, 0));

  // I want the following to just work, but it doesn't:
  // let nv1 = state.get_simple_node(v1, hl).expect("nv1");

  let nv1 = state.put_simple_node(v1, hl);
  assert_eq!(nv1, NID::from_vid_idx(v1, 0));

  // this node is "malformed" because the lower number is on top,
  // but the concept should still work:
  let nx0 = state.put_simple_node(x0, hl);
  assert_eq!(nx0, NID::from_vid_idx(x0, 0));
}
