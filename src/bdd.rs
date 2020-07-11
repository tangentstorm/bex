/// A module for efficient implementation of binary decision diagrams.
use std::clone::Clone;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

extern crate num_cpus;

use serde::{Serialize, Serializer, Deserialize, Deserializer};
use bincode;
use base;
use io;
use reg::Reg;
use vhl::{HiLo, HiLoPart, HiLoBase, VHLParts};
use {nid, nid::{NID,O,I,idx,is_var,is_const,IDX,is_inv}};
use vid::{VID,VidOrdering,topmost_of3};
use cur::{Cursor, CursorPlan};


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

/// Type alias for whatever HashMap implementation we're curretly using -- std,
/// fnv, hashbrown... Hashing is an extremely important aspect of a BDD base, so
/// it's useful to have a single place to configure this.
pub type BDDHashMap<K,V> = hashbrown::hash_map::HashMap<K,V>;


impl ITE {
  /// choose normal form for writing this triple. Algorithm based on:
  /// "Efficient Implementation of a BDD Package"
  /// http://www.cs.cmu.edu/~emc/15817-f08/bryant-bdd-1991.pdf
  /// (This is one of the biggest bottlenecks so we inline a lot,
  /// do our own tail call optimization, etc...)
  pub fn norm(f0:NID, g0:NID, h0:NID)->Norm {
    let mut f = f0; let mut g = g0; let mut h = h0;
    loop {
      if is_const(f) { return Norm::Nid(if f==I { g } else { h }) }           // (I/O, _, _)
      if g==h { return Norm::Nid(g) }                                         // (_, g, g)
      if g==f { if is_const(h) { return Norm::Nid(if h==I { I } else { f }) } // (f, f, I/O)
                else { g=I }}
      else if is_const(g) && is_const(h) { // both const, and we know g!=h
        return if g==I { return Norm::Nid(f) } else { Norm::Nid(!f) }}
      else {
        let nf = !f;
        if      g==nf { g=O } // bounce!(f,O,h)
        else if h==f  { h=O } // bounce!(f,g,O)
        else if h==nf { h=I } // bounce!(f,g,I)
        else {
          let (fv, fi) = (f.vid(), idx(f));
          macro_rules! cmp { ($x0:expr,$x1:expr) => {
            { let x0=$x0; ((x0.is_above(&fv)) || ((x0==fv) && ($x1<fi))) }}}
          if is_const(g) && cmp!(h.vid(),idx(h)) {
            if g==I { g=f; f=h; h=g;  g=I; }     // bounce!(h,I,f)
            else    { f=!h; g=O;  h=nf; }}   // bounce(not(h),O,nf)
          else if is_const(h) && cmp!(g.vid(),idx(g)) {
            if h==I { f=!g; g=nf; h=I; }     // bounce!(not(g),nf,I)
            else    { h=f; f=g; g=h;  h=O; }}    // bounce!(g,f,O)
          else {
            let ng = !g;
            if (h==ng) && cmp!(g.vid(), idx(g)) { h=f; f=g; g=h; h=nf; } // bounce!(g,f,nf)
            // choose form where first 2 slots are NOT inverted:
            // from { (f,g,h), (¬f,h,g), ¬(f,¬g,¬h), ¬(¬f,¬g,¬h) }
            else if is_inv(f) { f=g; g=h; h=f; f=nf; } // bounce!(nf,h,g)
            else if is_inv(g) { return match ITE::norm(f,ng,!h) {
              Norm::Nid(nid) => Norm::Nid(!nid),
              Norm::Not(ite) => Norm::Ite(ite),
              Norm::Ite(ite) => Norm::Not(ite)}}
            else { return Norm::Ite(ITE::new(f,g,h)) }}}}}} }


/// trait allowing multiple implementations of the in-memory storage layer.
pub trait BddState : Sized + Serialize + Clone + Sync + Send {

  fn new(nvars: usize)->Self;

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] fn tup(&self, n:NID)-> (NID, NID) {
    if is_const(n) { if n==I { (I, O) } else { (O, I) } }
    else if is_var(n) { if is_inv(n) { (O, I) } else { (I, O) }}
    else { let hilo = self.get_hilo(n);
           (hilo.hi, hilo.lo) }}

  /// fetch or create a "simple" node, where the hi and lo branches are both
  /// already fully computed pointers to existing nodes.
  #[inline] fn simple_node(&mut self, v:VID, hilo:HiLo)->NID {
    match self.get_simple_node(v, hilo) {
      Some(n) => n,
      None => { self.put_simple_node(v, hilo) }}}

  // --- implement these --------------------------------------------

  fn nvars(&self)->usize;

  fn get_hilo(&self, n:NID)->HiLo;
  /// load the memoized NID if it exists
  fn get_memo(&self, ite:&ITE) -> Option<NID>;
  fn put_xmemo(&mut self, ite:ITE, new_nid:NID);
  fn get_simple_node(&self, v:VID, hilo:HiLo)-> Option<NID>;
  fn put_simple_node(&mut self, v:VID, hilo:HiLo)->NID; }


// TODO: remove vindex. There's no reason to store (x1,y,z) separately from (y,z).
// !! Previously, in test_nano_bdd, I wind up with a node branching on x2
//      to another node also branching on x2.
//    As of 2020-07-10, the new problem is just that test_multi_bdd
//      and test_nano_bdd start taking minutes to run.
//    I can't currently think of a reason vindex[(vX,hilo)] shouldn't behave
//      exactly the same as vindex[(vY,hilo)] and thus == index[hilo], but I'm
//      obviously missing something. :/
//    It could be a bug in replace(), but that's a simple function.
//    More likely, it's something to do with the recent/stable dichotomy in BddSwarm,
//      or simply the fact that each worker has its own recent state and they're getting
//      out of sync.

/// Groups everything by variable. I thought this would be useful, but it probably is not.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SafeBddState {
  /// number of variables
  nvars: usize,
  /// variable-agnostic hi/lo pairs for individual bdd nodes.
  hilos: Vec<HiLo>,
  /// reverse map for hilos.
  index: BDDHashMap<HiLo, IDX>,
  /// variable-specific memoization. These record (v,hilo) lookups.
  /// There shouldn't be any need for this, but an undiagnosed
  /// bug prevents me from removing it.
  vindex: BDDHashMap<(VID,HiLo), IDX>,
  /// arbitrary memoization. These record normalized (f,g,h) lookups.
  xmemo: BDDHashMap<ITE, NID> }


impl BddState for SafeBddState {

  /// constructor
  fn new(nvars:usize)->SafeBddState {
    SafeBddState {
      nvars,
      hilos: vec![],
      index: BDDHashMap::default(),
      vindex: BDDHashMap::default(),
      xmemo: BDDHashMap::default() }}

  /// return the number of variables
  fn nvars(&self)->usize { self.nvars }

  #[inline] fn put_xmemo(&mut self, ite:ITE, new_nid:NID) {
    self.xmemo.insert(ite, new_nid); }

  /// load the memoized NID if it exists
  #[inline] fn get_memo(&self, ite:&ITE) -> Option<NID> {
    if is_var(ite.i) {
      debug_assert!(!is_inv(ite.i)); // because it ought to be normalized by this point.
      let hilo = if is_inv(ite.i) { HiLo::new(ite.e,ite.t) } else { HiLo::new(ite.t,ite.e) };
      self.get_simple_node(ite.i.vid(), hilo) }
    else { self.xmemo.get(&ite).copied() }}

  /// the "put" for this one is put_simple_node
  #[inline] fn get_hilo(&self, n:NID)->HiLo {
    assert!(!nid::is_lit(n));
    let res = self.hilos[idx(n) as usize];
    if is_inv(n) { res.invert() } else { res }}

  #[inline] fn get_simple_node(&self, v:VID, hl0:HiLo)-> Option<NID> {
    let inv = nid::is_inv(hl0.lo);
    let hl1 = if inv { hl0.invert() } else { hl0 };
    let to_nid = |&ix| NID::from_vid_idx(v, ix);
    let res = self.vindex.get(&(v, hl1)).map(to_nid);
    //let res = self.index.get(&hl1).map(to_nid);
    if inv { res.map(|nid| !nid ) } else { res }}

  #[inline] fn put_simple_node(&mut self, v:VID, hl0:HiLo)->NID {
    let inv = nid::is_inv(hl0.lo);
    let hilo = if inv { hl0.invert() } else { hl0 };
    let ix:IDX =
      if let Some(&ix) = self.index.get(&hilo) { ix }
      else {
        let ix = self.hilos.len() as IDX;
        self.hilos.push(hilo);
        self.index.insert(hilo, ix);
        self.vindex.insert((v,hilo), ix);
        ix };
    let res = NID::from_vid_idx(v, ix);
    if inv { !res } else { res } }}


pub trait BddWorker<S:BddState> : Sized + Serialize {
  fn new(nvars:usize)->Self;
  fn new_with_state(state: S)->Self;
  fn nvars(&self)->usize;
  fn tup(&self, n:NID)->(NID,NID);
  fn ite(&mut self, f:NID, g:NID, h:NID)->NID;
  fn get_state(&self)->&S;
}


// ----------------------------------------------------------------
// Helper types for BddSwarm
// ----------------------------------------------------------------
/// Query ID for BddSwarm.
type QID = usize;

/// Query message for BddSwarm.
#[derive(PartialEq)]
enum QMsg<S:BddState> { Ite(QID, ITE), Cache(Arc<S>) }
impl<S:BddState> std::fmt::Debug for QMsg<S> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      QMsg::Ite(qid, ite) => { write!(f, "Ite(q{}, {:?})", qid, ite) }
      QMsg::Cache(_) => { write!(f, "QMsg::Cache") } } }}

/// Response message for BddSwarm.
#[derive(PartialEq,Debug)]
enum RMsg {
  /// resolved to a nid
  Nid(NID),
  /// a simple node needs to be constructed:
  Vhl{v:VID, hi:NID, lo:NID, invert:bool},
  /// other work in progress
  Wip{v:VID, hi:Norm, lo:Norm, invert:bool},
  /// We've solved the whole problem, so exit the loop and return this nid.
  Ret(NID)}

fn rmsg_not(rmsg:RMsg)->RMsg {
  match rmsg {
    RMsg::Nid(n) => RMsg::Nid(!n),
    RMsg::Vhl{v,hi,lo,invert} => RMsg::Vhl{v,hi,lo,invert:!invert},
    RMsg::Wip{v,hi,lo,invert} => RMsg::Wip{v,hi,lo,invert:!invert},
    RMsg::Ret(n) => RMsg::Ret(!n)}}


/// Sender for QMsg
type QTx<S> = Sender<QMsg<S>>;
/// Receiver for QMsg
type QRx<S> = Receiver<QMsg<S>>;
/// Sender for RMsg
type RTx = Sender<(QID, RMsg)>;
/// Receiver for RMsg
type RRx = Receiver<(QID, RMsg)>;


/// Work in progress for BddSwarm.
#[derive(PartialEq,Debug,Copy,Clone)]
enum BddWIP { Fresh, Done(NID), Parts(VHLParts) }

/// Helps track dependencies between WIP tasks
#[derive(Debug,Copy,Clone)]
struct BddDep { qid: QID, part: HiLoPart, invert: bool }
impl BddDep{
  fn new(qid: QID, part: HiLoPart, invert: bool)->BddDep { BddDep{qid, part, invert} }}



// ----------------------------------------------------------------
/// BddSwarm: a multi-threaded worker implementation
// ----------------------------------------------------------------
#[derive(Debug)]
pub struct BddSwarm <S:BddState+'static> {
  /// receives messages from the threads
  rx: RRx,
  /// send messages to myself (so we can put them back in the queue.
  me: RTx,
  /// QMsg senders for each thread, so we can send queries to work on.
  swarm: Vec<QTx<S>>,
  /// read-only version of the state shared by all threads.
  stable: Arc<S>,
  /// mutable version of the state kept by the main thread.
  recent: S,

  // !! maybe these should be moved to a different struct, since they're specific to a run?

  /// track new ites that aren't in the cache, so we can memoize once we solve them.
  ites: Vec<ITE>,
  /// stores work in progress during a run:
  wip:Vec<BddWIP>,
  /// track ongoing tasks so we don't duplicate work in progress:
  qid: BDDHashMap<ITE, QID>,
  /// stores dependencies during a run. The bool specifies whether to invert.
  deps: Vec<Vec<BddDep>> }

impl<TState:BddState> Serialize for BddSwarm<TState> {
  fn serialize<S:Serializer>(&self, ser: S)->Result<S::Ok, S::Error> {
    // all we really care about is the state:
    self.stable.serialize::<S>(ser) } }

impl<'de:'a, 'a, S:BddState + Deserialize<'de>> Deserialize<'de> for BddSwarm<S> {
  fn deserialize<D:Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
    let mut res = Self::new(0);
    res.stable = Arc::new(S::deserialize(d)?);
    Ok(res) }}


impl<S:BddState> BddWorker<S> for BddSwarm<S> {

  fn new(nvars:usize)->Self {
    let (me, rx) = channel::<(QID, RMsg)>();
    let swarm = vec![];
    let stable = Arc::new(S::new(nvars));
    let recent = S::new(nvars);
    Self{ me, rx, swarm, stable, recent,
          ites:vec![], deps:vec![], wip:vec![], qid:BDDHashMap::new() }}

  fn new_with_state(state: S)->Self {
    println!("warning: new_with_state probably doesn't work so well yet..."); // TODO!
    let mut res = Self::new(state.nvars());
    res.stable = Arc::new(state.clone());
    res.recent = state;
    res }

  fn get_state(&self)->&S { &self.recent }

  fn nvars(&self)->usize { self.recent.nvars() }

  fn tup(&self, n:NID)->(NID,NID) { self.recent.tup(n) }

  /// all-purpose if-then-else node constructor. For the swarm implementation,
  /// we push all the normalization and tree traversal work into the threads,
  /// while this function puts all the parts together.
  fn ite(&mut self, i:NID, t:NID, e:NID)->NID { self.run_swarm(i,t,e) } }


impl<S:BddState> BddSwarm<S> {

  /// add a new task for the swarm to work on. (if it's a duplicate, we just
  /// add the dependencies to the original task (unless it's already finished,
  /// in which case we resolve immediately))
  fn add_task(&mut self, opt_dep:Option<BddDep>, ite:ITE) {
    trace!("add_task({:?}, {:?})", opt_dep, ite);
    let (qid, is_dup) = {
      if let Some(&dup) = self.qid.get(&ite) { (dup, true) }
      else { (self.wip.len(), false) }};
    if is_dup {
      if let Some(dep) = opt_dep {
        trace!("*** task {:?} is dup of q{} invert: {}", ite, qid, dep.invert);
        if let BddWIP::Done(nid) = self.wip[qid] {
          self.resolve_part(dep.qid, dep.part, nid, dep.invert); }
        else { self.deps[qid].push(dep) }}
      else { panic!("Got duplicate request, but no dep. This should never happen!") }}
    else {
      self.qid.insert(ite, qid); self.ites.push(ite);
      let w:usize = qid % self.swarm.len();
      self.swarm[w].send(QMsg::Ite(qid, ite)).expect("send to swarm failed");
      self.wip.push(BddWIP::Fresh);
      if let Some(dep) = opt_dep {
        trace!("*** added task #{}: {:?} invert:{}", qid, ite, dep.invert);
        self.deps.push(vec![dep]) }
      else if qid == 0 {
        trace!("*** added task #{}: {:?} (no deps!)", qid, ite);
        self.deps.push(vec![]) }
      else { panic!("non 0 qid with no deps!?") }}}

  /// called whenever the wip resolves to a single nid
  fn resolve_nid(&mut self, qid:QID, nid:NID) {
    trace!("resolve_nid(q{}, {})", qid, nid);
    if let BddWIP::Done(old) = self.wip[qid] {
      warn!("resolving already resolved nid for q{}", qid);
      assert_eq!(old, nid, "old and new resolutions didn't match!") }
    else {
      trace!("resolved_nid: q{}=>{}. deps: {:?}", qid, nid, self.deps[qid].clone());
      self.wip[qid] = BddWIP::Done(nid);
      let ite = self.ites[qid];
      self.recent.put_xmemo(ite, nid);
      for &dep in self.deps[qid].clone().iter() {
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
    trace!("resolved vhl: q{}=>{}. #deps: {}", qid, nid, self.deps[qid].len());
    self.resolve_nid(qid, nid); }

  fn resolve_part(&mut self, qid:QID, part:HiLoPart, nid0:NID, invert:bool) {
    if let BddWIP::Parts(ref mut parts) = self.wip[qid] {
      let nid = if invert { !nid0 } else { nid0 };
      trace!("   !! set {:?} for q{} to {}", part, qid, nid);
      if part == HiLoPart::HiPart { parts.hi = Some(nid) } else { parts.lo = Some(nid) }}
    else { warn!("???? got a part for a qid #{} that was already done!", qid) }
    if let BddWIP::Parts(wip) = self.wip[qid] {
      if let Some(hilo) = wip.hilo() { self.resolve_vhl(qid, wip.v, hilo, wip.invert) }}}
      // else { println!("got a part for q{} but it's still not done", qid);
      //        for (qid, task) in self.wip.iter().by_ref().enumerate() {
      //          println!("    q{} : {:?} {:?}", qid, task, self.deps[qid]); }} }}

  /// initialization logic for running the swarm. spawns threads and copies latest cache.
  fn init_swarm(&mut self) {
    self.wip = vec![]; self.ites = vec![]; self.deps = vec![]; self.qid = BDDHashMap::new();
    // wipe out and replace the channels so un-necessary work from last iteration
    // (that was still going on when we returned a value) gets ignored..
    let (me, rx) = channel::<(QID, RMsg)>(); self.me = me; self.rx = rx;
    self.swarm = vec![];
    while self.swarm.len() < num_cpus::get() {
      let (tx, rx) = channel::<QMsg<S>>();
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
            assert_eq!(self.wip[qid], BddWIP::Fresh);
            self.wip[qid] = BddWIP::Parts(VHLParts{ v, hi:None, lo:None, invert });
            macro_rules! handle_part { ($xx:ident, $part:expr) => {
              match $xx {
                Norm::Nid(nid) => self.resolve_part(qid, $part, nid, false),
                Norm::Ite(ite) => self.add_task(Some(BddDep::new(qid, $part, false)), ite),
                Norm::Not(ite) => self.add_task(Some(BddDep::new(qid, $part, true)), ite)}}}
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
fn swarm_ite<S:BddState>(state: &Arc<S>, ite0:ITE)->RMsg {
  let ITE { i, t, e } = ite0;
  match ITE::norm(i,t,e) {
      Norm::Nid(n) => RMsg::Nid(n),
      Norm::Ite(ite) => swarm_ite_norm(state, ite),
      Norm::Not(ite) => rmsg_not(swarm_ite_norm(state, ite)) }}

fn swarm_vhl_norm<S:BddState>(state: &Arc<S>, ite:ITE)->RMsg {
  let ITE{i:vv,t:hi,e:lo} = ite; let v = vv.vid();
  if let Some(n) = state.get_simple_node(vv.vid(), HiLo{hi,lo}) { RMsg::Nid(n) }
  else { RMsg::Vhl{ v, hi, lo, invert:false } }}

fn swarm_ite_norm<S:BddState>(state: &Arc<S>, ite:ITE)->RMsg {
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
          Norm::Not(ite) => rmsg_not(swarm_vhl_norm(state, ite))}}
      // otherwise at least one side is not a simple nid yet, and we have to defer
      else { RMsg::Wip{ v, hi, lo, invert:false } }}}}



/// This is the loop run by each thread in the swarm.
fn swarm_loop<S:BddState>(tx:RTx, rx:QRx<S>, mut state:Arc<S>) {
  for qmsg in rx.iter() {
    match qmsg {
      QMsg::Cache(s) => { state = s }
      QMsg::Ite(qid, ite) => {
        trace!("--->   thread worker got qmsg {}: {:?}", qid, qmsg);
        let rmsg = swarm_ite(&state, ite);
        if tx.send((qid, rmsg)).is_err() { break } }}}}


/// Finally, we put everything together. This is the top-level type for this crate.
#[derive(Debug, Serialize, Deserialize)]
pub struct BddBase<S:BddState, W:BddWorker<S>> {
  /// allows us to give user-friendly names to specific nodes in the base.
  pub tags: HashMap<String, NID>,
  phantom: PhantomData<S>,
  worker: W}


impl<S:BddState, W:BddWorker<S>> BddBase<S,W> {

  /// constructor
  pub fn new(nvars:usize)->BddBase<S,W> {
    BddBase{phantom: PhantomData,
            worker: W::new(nvars),
            tags:HashMap::new()}}

  /// accessor for number of variables
  pub fn nvars(&self)->usize { self.worker.nvars() }

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] fn tup(&self, n:NID)->(NID,NID) { self.worker.tup(n) }

  /// walk node recursively, without revisiting shared nodes
  pub fn walk<F>(&self, n:NID, f:&mut F) where F: FnMut(NID,VID,NID,NID) {
    let mut seen = HashSet::new();
    self.step(n,f,&mut seen)}

  /// internal helper: one step in the walk.
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>)
  where F: FnMut(NID,VID,NID,NID) {
    if !seen.contains(&n) {
      seen.insert(n); let (hi,lo) = self.tup(n); f(n,n.vid(),hi,lo);
      if !is_const(hi) { self.step(hi, f, seen); }
      if !is_const(lo) { self.step(lo, f, seen); }}}

  pub fn save(&self, path:&str)->::std::io::Result<()> {
    let s = bincode::serialize(&self).unwrap();
    io::put(path, &s) }

  pub fn load(path:&str)->::std::io::Result<BDDBase> {
    let s = io::get(path)?;
    Ok(bincode::deserialize(&s).unwrap()) }

  // public node constructors

  pub fn and(&mut self, x:NID, y:NID)->NID { self.ite(x, y, O) }
  pub fn xor(&mut self, x:NID, y:NID)->NID { self.ite(x, !y, y) }
  pub fn  or(&mut self, x:NID, y:NID)->NID { self.ite(x, I, y) }
  pub fn  gt(&mut self, x:NID, y:NID)->NID { self.ite(x, !y, O) }
  pub fn  lt(&mut self, x:NID, y:NID)->NID { self.ite(x, O, y) }

  /// all-purpose node creation/lookup
  #[inline] pub fn ite(&mut self, f:NID, g:NID, h:NID)->NID { self.worker.ite(f,g,h) }

  /// nid of y when x is high
  pub fn when_hi(&mut self, x:VID, y:NID)->NID {
    let yv = y.vid();
    match x.cmp_depth(&yv) {
      VidOrdering::Level => self.tup(y).0,  // x ∧ if(x,th,_) → th
      VidOrdering::Above => y,              // y independent of x, so no change. includes yv = I
      VidOrdering::Below => {               // y may depend on x, so recurse.
        let (yt, ye) = self.tup(y);
        let (th,el) = (self.when_hi(x,yt), self.when_hi(x,ye));
        self.ite(NID::from_vid(yv), th, el) }}}

  /// nid of y when x is low
  pub fn when_lo(&mut self, x:VID, y:NID)->NID {
    let yv = y.vid();
    match x.cmp_depth(&yv) {
      VidOrdering::Level => self.tup(y).1,  // ¬x ∧ if(x,_,el) → el
      VidOrdering::Above => y,              // y independent of x, so no change. includes yv = I
      VidOrdering::Below => {               // y may depend on x, so recurse.
        let (yt, ye) = self.tup(y);
        let (th,el) = (self.when_lo(x,yt), self.when_lo(x,ye));
        self.ite(NID::from_vid(yv), th, el) }}}

  /// replace var x with y in z
  pub fn replace(&mut self, x:VID, y:NID, z:NID)->NID {
    if z.might_depend_on(x) {
      let (zt,ze) = self.tup(z); let zv = z.vid();
      if x==zv { self.ite(y, zt, ze) }
      else {
        let th = self.replace(x, y, zt);
        let el = self.replace(x, y, ze);
        self.ite(NID::from_vid(zv), th, el) }}
    else { z }}

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
    if o == self.nvars() { match self.when_lo(v, n) {
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
    if self.nvars() > 16 {
      panic!("refusing to generate a truth table of 2^{} bytes", self.nvars()) }
    let mut res = vec![0;(1 << self.nvars()) as usize];
    self.tt_aux(&mut res, VID::var(0), n0, 0);
    res }

} // end impl BddBase

// Base Trait

impl<S:BddState, W:BddWorker<S>> base::Base for BddBase<S,W> {
  fn new(n:usize)->Self { Self::new(n) }
  fn num_vars(&self)->usize { self.nvars() }

  fn when_hi(&mut self, v:VID, n:NID)->NID { self.when_hi(v,n) }
  fn when_lo(&mut self, v:VID, n:NID)->NID { self.when_lo(v,n) }

  // TODO: these should be moved into seperate struct
  fn def(&mut self, _s:String, _i:VID)->NID { todo!("BddBase::def()") }
  fn tag(&mut self, n:NID, s:String)->NID { self.tags.insert(s, n); n }
  fn get(&self, s:&str)->Option<NID> { Some(*self.tags.get(s)?) }

  fn and(&mut self, x:NID, y:NID)->NID { self.and(x, y) }
  fn xor(&mut self, x:NID, y:NID)->NID { self.xor(x, y) }
  fn or(&mut self, x:NID, y:NID)->NID  { self.or(x, y) }
  #[cfg(todo)] fn mj(&mut self, x:NID, y:NID, z:NID)->NID  {
    self.xor(x, self.xor(y, z)) }  // TODO: normalize order. make this the default impl.
  #[cfg(todo)] fn ch(&mut self, x:NID, y:NID, z:NID)->NID { self.ite(x, y, z) }

  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID { self.replace(v,n,ctx) }

  fn save(&self, path:&str)->::std::io::Result<()> { self.save(path) }

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

type S = SafeBddState;

/// The default type used by the rest of the system.
/// (Note the first three letters in uppercase).
pub type BDDBase = BddBase<S,BddSwarm<S>>;

// generic base::Base test suite
test_base_consts!(BDDBase);
test_base_vars!(BDDBase);
test_base_when!(BDDBase);


// basic test suite

#[test] fn test_base() {
  let mut base = BDDBase::new(3);
  let (v1, v2, v3) = (NID::var(1), NID::var(2), NID::var(3));
  assert_eq!(base.nvars(), 3);
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
pub type BddSwarmBase = BddBase<SafeBddState,BddSwarm<SafeBddState>>;

#[test] fn test_swarm_xor() {
  let mut base = BddSwarmBase::new(2);
  let (x0, x1) = (NID::var(0), NID::var(1));
  let x = base.xor(x0, x1);
  assert_eq!(x1,  base.when_lo(VID::var(0),x));
  assert_eq!(!x1, base.when_hi(VID::var(0),x));
  assert_eq!(x0,  base.when_lo(VID::var(1),x));
  assert_eq!(!x0, base.when_hi(VID::var(1),x));
  assert_eq!(x,   base.when_lo(VID::var(2),x));
  assert_eq!(x,   base.when_hi(VID::var(2),x))}

#[test] fn test_swarm_and() {
  let mut base = BddSwarmBase::new(2);
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
  let mut base = BddSwarmBase::new(3);
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
  let mut base = BddSwarmBase::new(4);
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
  let mut base = BDDBase::new(2);  let mut it = base.solutions(nid::O);
  assert_eq!(it.next(), None, "const O should yield no solutions.") }

#[test] fn test_bdd_solutions_i() {
  let mut base = BDDBase::new(2);
  let actual:HashSet<usize> = base.solutions(nid::I).map(|r| r.as_usize()).collect();
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
  // use base::Base; base.show(n);
  let actual:Vec<usize> = base.solutions(n).map(|x|x.as_usize()).collect();
  let expect = vec![0b001, 0b010, 0b101, 0b110 ]; // bits cba
  assert_eq!(actual, expect); }

impl BDDBase {
  pub fn solutions(&mut self, n:NID)->BDDSolIterator {
    self.solutions_trunc(n, self.nvars())}

  pub fn solutions_trunc(&self, n:NID, nvars:usize)->BDDSolIterator {
    assert!(nvars <= self.nvars(), "nvars arg to solutions_trunc must be <= self.nvars");
    BDDSolIterator::from_bdd(self, n, nvars)}}


/// helpers for solution cursor
impl HiLoBase for BDDBase {
  fn get_hilo(&self, n:NID)->Option<HiLo> {
    let (hi, lo) = self.worker.get_state().tup(n);
    Some(HiLo{ hi, lo }) }}

impl CursorPlan for BDDBase {}

impl BDDBase {
  pub fn first_solution(&self, n:NID, nvars:usize)->Option<Cursor> {
    if n==nid::O || nvars == 0 { None }
    else {
      let mut cur = Cursor::new(nvars, n);
      cur.descend(self);
      debug_assert!(nid::is_const(cur.node));
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
    assert!(nid::is_const(cur.node), "find_next_leaf should always start by looking at a leaf");
    if cur.nstack.is_empty() { assert!(cur.node == nid::I); return None }

    // now we are definitely at a leaf node with a branch above us.
    cur.step_up();

    let tv = cur.node.vid(); // branching var for current twig node
    let mut rippled = false;
    // if we've already walked the hi branch...
    if cur.scope.var_get(tv) {
      cur.to_next_lo_var();
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
    assert!(nid::is_const(cur.node), "advance should always start by looking at a leaf");
    if self.in_solution(&cur) {
      // if we're in the solution, we're going to increment the "counter".
      if let Some(zpos) = cur.increment() {
        self.log(&cur, format!("rebranch on {:?}",zpos).as_str());
        // The 'zpos' variable exists in the solution space, but there might or might
        // not be a branch node for that variable in the current bdd path.
        // Whether we follow the hi or lo branch depends on which variable we're looking at.
        if nid::is_const(cur.node) { return Some(cur) } // special case for topmost I (all solutions)
        cur.put_step(self, cur.var_get());
        cur.descend(self); }
      else { // overflow. we've counted all the way to 2^nvars-1, and we're done.
        self.log(&cur, "$ found all solutions!"); return None }}
    while !self.in_solution(&cur) {
      // If still here, we are looking at a leaf that isn't a solution (out=0 in truth table)
      let next = self.find_next_leaf(&mut cur);
      if next.is_none() { return None }}
    Some(cur)}
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
  let mut state = SafeBddState::new(8);
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
