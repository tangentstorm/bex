/// A module for efficient implementation of binary decision diagrams.
use std::clone::Clone;
use std::cmp::min;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::marker::PhantomData;
use std::process::Command;      // for creating and viewing digarams
use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

extern crate num_cpus;

use serde::{Serialize, Serializer, Deserialize, Deserializer};
use bincode;
use base;
use io;
use nid;
pub use nid::*;


/// A BDDNode is a triple consisting of a VID, which references an input variable,
/// and high and low branches, each pointing at other nodes in the BDD. The
/// associated variable's value determines which branch to take.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BDDNode { pub v:VID, pub hi:NID, pub lo:NID } // if|then|else

/// An if/then/else triple. This is similar to an individual BDDNode, but the 'if' part
/// part represents a node, not a variable
#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone, Copy)]
pub struct ITE {i:NID, t:NID, e:NID}
impl ITE {
  /// shorthand constructor
  pub fn new (i:NID, t:NID, e:NID)-> ITE { ITE { i:i, t:t, e:e } }
  pub fn min_var(&self)->VID { return min(var(self.i), min(var(self.t), var(self.e))) }
  // NOTE: there is a separet impl for norm(), below
}

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
        return if g==I { return Norm::Nid(f) } else { Norm::Nid(not(f)) }}
      else {
        let nf = not(f);
        if      g==nf { g=O } // bounce!(f,O,h)
        else if h==f  { h=O } // bounce!(f,g,O)
        else if h==nf { h=I } // bounce!(f,g,I)
        else {
          let (fv, fi) = (var(f), idx(f));
          macro_rules! cmp { ($x0:expr,$x1:expr) => {
            { let x0=$x0; ((x0<fv) || ((x0==fv) && ($x1<fi))) }}}
          if is_const(g) && cmp!(var(h),idx(h)) {
            if g==I { g=f; f=h; h=g;  g=I; }     // bounce!(h,I,f)
            else    { f=not(h); g=O;  h=nf; }}   // bounce(not(h),O,nf)
          else if is_const(h) && cmp!(var(g),idx(g)) {
            if h==I { f=not(g); g=nf; h=I; }     // bounce!(not(g),nf,I)
            else    { h=f; f=g; g=h;  h=O; }}    // bounce!(g,f,O)
          else {
            let ng = not(g);
            if (h==ng) && cmp!(var(g), idx(g)) { h=f; f=g; g=h; h=nf; } // bounce!(g,f,nf)
            // choose form where first 2 slots are NOT inverted:
            // from { (f,g,h), (¬f,h,g), ¬(f,¬g,¬h), ¬(¬f,¬g,¬h) }
            else if is_inv(f) { f=g; g=h; h=f; f=nf; } // bounce!(nf,h,g)
            else if is_inv(g) { return match ITE::norm(f,ng,not(h)) {
              Norm::Nid(nid) => Norm::Nid(not(nid)),
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
    else { let mut hilo = self.get_hilo(n);
           if is_inv(n) { hilo = hilo.invert() };
           (hilo.hi, hilo.lo) }}

  /// fetch or create a "simple" node, where the hi and lo branches are both
  /// already fully computed pointers to existing nodes.
  #[inline] fn simple_node(&mut self, v:VID, hilo:HILO)->NID {
    match self.get_simple_node(v, hilo) {
      Some(&n) => n,
      None => { self.put_simple_node(v, hilo) }}}

  // --- implement these --------------------------------------------

  fn nvars(&self)->usize;

  #[inline] fn get_hilo(&self, n:NID)->HILO;
  /// load the memoized NID if it exists
  #[inline] fn get_memo<'a>(&'a self, v:VID, ite:&ITE) -> Option<&'a NID>;
  #[inline] fn put_xmemo(&mut self, ite:ITE, new_nid:NID);
  #[inline] fn get_simple_node<'a>(&'a self, v:VID, hilo:HILO)-> Option<&'a NID>;
  #[inline] fn put_simple_node(&mut self, v:VID, hilo:HILO)->NID; }


/// Groups everything by variable. I thought this would be useful, but it probably is not.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SafeVarKeyedBddState {
  /// variable-specific hi/lo pairs for individual bdd nodes.
  nodes: Vec<Vec<HILO>>,
  /// variable-specific memoization. These record (v,hilo) lookups.
  vmemo: Vec<BDDHashMap<HILO,NID>>,
  /// arbitrary memoization. These record normalized (f,g,h) lookups,
  /// and are indexed at three layers: v,f,(g h); where v is the
  /// branching variable.
  xmemo: Vec<BDDHashMap<ITE, NID>> }

/// Same as the safe version but disables bounds checking.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnsafeVarKeyedBddState {
  /// variable-specific hi/lo pairs for individual bdd nodes.
  nodes: Vec<Vec<HILO>>,
  /// variable-specific memoization. These record (v,hilo) lookups.
  vmemo: Vec<BDDHashMap<HILO,NID>>,
  /// arbitrary memoization. These record normalized (f,g,h) lookups,
  /// and are indexed at three layers: v,f,(g h); where v is the
  /// branching variable.
  xmemo: Vec<BDDHashMap<ITE, NID>> }


impl BddState for SafeVarKeyedBddState {

  /// constructor
  fn new(nvars:usize)->SafeVarKeyedBddState {
    SafeVarKeyedBddState{
      nodes: (0..nvars).map(|_| vec![]).collect(),
      vmemo:(0..nvars).map(|_| BDDHashMap::default()).collect(),
      xmemo:(0..nvars).map(|_| BDDHashMap::default()).collect() }}

  /// return the number of variables
  fn nvars(&self)->usize { self.nodes.len() }

  /// the "put" for this one is put_simple_node
  #[inline] fn get_hilo(&self, n:NID)->HILO {
    self.nodes[rv(var(n))][idx(n)] }

  /// load the memoized NID if it exists
  #[inline] fn get_memo<'a>(&'a self, v:VID, ite:&ITE) -> Option<&'a NID> {
    if is_var(ite.i) {
      self.vmemo[rvar(ite.i) as usize].get(&HILO::new(ite.t,ite.e)) }
    else { self.xmemo.as_slice().get(rv(v))?.get(&ite) }}

  #[inline] fn put_xmemo(&mut self, ite:ITE, new_nid:NID) {
    let v = ite.min_var();
    self.xmemo[rv(v)].insert(ite, new_nid); }

  #[inline] fn get_simple_node<'a>(&'a self, v:VID, hilo:HILO)-> Option<&'a NID> {
    self.vmemo[rv(v)].get(&hilo) }

  #[inline] fn put_simple_node(&mut self, v:VID, hilo:HILO)->NID {
    let ref mut vnodes = self.nodes[rv(v)];
    let res = nvi(v, vnodes.len() as IDX);
    vnodes.push(hilo);
    self.vmemo[rv(v) as usize].insert(hilo,res);
    res } }


impl BddState for UnsafeVarKeyedBddState {

  /// constructor
  fn new(nvars:usize)->UnsafeVarKeyedBddState {
    UnsafeVarKeyedBddState{
      nodes: (0..nvars).map(|_| vec![]).collect(),
      vmemo:(0..nvars).map(|_| BDDHashMap::default()).collect(),
      xmemo:(0..nvars).map(|_| BDDHashMap::default()).collect() }}

  /// return the number of variables
  fn nvars(&self)->usize { self.nodes.len() }

  /// the "put" for this one is put_simple_node
  #[inline] fn get_hilo(&self, n:NID)->HILO {
    unsafe { let bits = self.nodes.as_slice().get_unchecked(rv(var(n))).as_slice();
             *bits.get_unchecked(idx(n)) }}

  /// load the memoized NID if it exists
  #[inline] fn get_memo<'a>(&'a self, v:VID, ite:&ITE) -> Option<&'a NID> {
    unsafe { if is_var(ite.i) {
      self.vmemo.as_slice().get_unchecked(rv(rvar(ite.i))).get(&HILO::new(ite.t,ite.e)) }
             else { self.xmemo.as_slice().get_unchecked(rv(v)).get(&ite) }}}

  #[inline] fn put_xmemo(&mut self, ite:ITE, new_nid:NID) { unsafe {
    let v = ite.min_var();
    self.xmemo.as_mut_slice().get_unchecked_mut(rv(v)).insert(ite, new_nid); }}

  #[inline] fn get_simple_node<'a>(&'a self, v:VID, hilo:HILO)-> Option<&'a NID> {
    unsafe { self.vmemo.as_slice().get_unchecked(rv(v)).get(&hilo) }}

  #[inline] fn put_simple_node(&mut self, v:VID, hilo:HILO)->NID {
    unsafe {
      let vnodes = self.nodes.as_mut_slice().get_unchecked_mut(rv(v));
      let res = nvi(v, vnodes.len() as IDX);
      vnodes.push(hilo);
      self.vmemo.as_mut_slice().get_unchecked_mut(rv(v) as usize).insert(hilo,res);
      res }} }

pub trait BddWorker<S:BddState> : Sized + Serialize {
  fn new(nvars:usize)->Self;
  fn new_with_state(state: S)->Self;
  fn nvars(&self)->usize;
  fn tup(&self, n:NID)->(NID,NID);
  fn ite(&mut self, f:NID, g:NID, h:NID)->NID;
}



// ----------------------------------------------------------------
/// SimpleBddWorker: a single-threaded worker implementation
// ----------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleBddWorker<S:BddState> { state:S }

impl<S:BddState> BddWorker<S> for SimpleBddWorker<S> {
  fn new(nvars:usize)->Self { Self{ state: S::new(nvars) }}
  fn new_with_state(state: S)->Self { Self{ state }}
  fn nvars(&self)->usize { self.state.nvars() }
  fn tup(&self, n:NID)->(NID,NID) { self.state.tup(n) }

  /// if-then-else routine. all-purpose node creation/lookup tool.
  fn ite(&mut self, f:NID, g:NID, h:NID)->NID {
    match ITE::norm(f,g,h) {
      Norm::Nid(x) => x,
      Norm::Ite(ite) => self.ite_norm(ite),
      Norm::Not(ite) => not(self.ite_norm(ite)) }} }

impl<S:BddState> SimpleBddWorker<S> {
  /// helper for ite to work on the normalized i,t,e triple
  #[inline] fn ite_norm(&mut self, ite:ITE)->NID {
    // !! this is one of the most time-consuming bottlenecks, so we inline a lot.
    // it should only be called from ite() on pre-normalized triples
    let ITE { i, t, e } = ite;
    let (vi, vt, ve) = (var(i), var(t), var(e));
    let v = min(vi, min(vt, ve));
    match self.state.get_memo(v, &ite) {
      Some(&n) => n,
      None => {
        // We know we're going to branch on v, and v is either the branch var
        // or not relevant to each of i,t,e. So we either retrieve the hilo pair
        // or just pass the nid directly to each side of the branch.
        let (hi_i, lo_i) = if v == vi {self.tup(i)} else {(i,i)};
        let (hi_t, lo_t) = if v == vt {self.tup(t)} else {(t,t)};
        let (hi_e, lo_e) = if v == ve {self.tup(e)} else {(e,e)};
        let new_nid = {
          // TODO: push one of these off into a queue for other threads
          let hi = self.ite(hi_i, hi_t, hi_e);
          let lo = self.ite(lo_i, lo_t, lo_e);
          if hi == lo {hi} else { self.state.simple_node(v, HILO::new(hi,lo)) }};
        // now add the triple to the generalized memo store
        if !is_var(i) { self.state.put_xmemo(ite, new_nid) }
        new_nid }}} }

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
    RMsg::Nid(n) => RMsg::Nid(not(n)),
    RMsg::Vhl{v,hi,lo,invert} => RMsg::Vhl{v,hi,lo,invert:!invert},
    RMsg::Wip{v,hi,lo,invert} => RMsg::Wip{v,hi,lo,invert:!invert},
    RMsg::Ret(n) => RMsg::Ret(not(n))}}


/// Sender for QMsg
type QTx<S> = Sender<QMsg<S>>;
/// Receiver for QMsg
type QRx<S> = Receiver<QMsg<S>>;
/// Sender for RMsg
type RTx = Sender<(QID, RMsg)>;
/// Receiver for RMsg
type RRx = Receiver<(QID, RMsg)>;

/// Partial BDD node (for BddWIP).
#[derive(PartialEq,Debug,Copy,Clone)]
enum BddPart { HiPart, LoPart }

/// Work in progress for BddSwarm.
#[derive(PartialEq,Debug,Copy,Clone)]
struct BddParts{ v:VID, hi:Option<NID>, lo:Option<NID>, invert:bool}
impl BddParts {
  fn hilo(&self)->Option<HILO> {
    if let (Some(hi), Some(lo)) = (self.hi, self.lo) { Some(HILO{hi,lo}) } else { None }}}

/// Work in progress for BddSwarm.
#[derive(PartialEq,Debug,Copy,Clone)]
enum BddWIP { Fresh, Done(NID), Parts(BddParts) }

/// Helps track dependencies between WIP tasks
#[derive(Debug,Copy,Clone)]
struct BddDep { qid: QID, part: BddPart, invert: bool }
impl BddDep{
  fn new(qid: QID, part: BddPart, invert: bool)->BddDep { BddDep{qid, part, invert} }}



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
    res.recent = state.clone();
    res }

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
      else {
        if qid == 0 {
          trace!("*** added task #{}: {:?} (no deps!)", qid, ite);
          self.deps.push(vec![]) }
        else { panic!("non 0 qid with no deps!?") }}}}

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
  fn resolve_vhl(&mut self, qid:QID, v:VID, hilo:HILO, invert:bool) {
    trace!("resolve_vhl(q{}, v{}, {:?}, invert:{}", qid, v, hilo, invert);
    let HILO{hi:h0,lo:l0} = hilo;
    // we apply invert first so it normalizes correctly.
    let (h1,l1) = if invert { (not(h0), not(l0)) } else { (h0, l0) };
    let nid = match ITE::norm(nv(v), h1, l1) {
      Norm::Nid(n) => n,
      Norm::Ite(ITE{i:vv,t:hi,e:lo}) =>     self.recent.simple_node(var(vv), HILO{hi,lo}),
      Norm::Not(ITE{i:vv,t:hi,e:lo}) => not(self.recent.simple_node(var(vv), HILO{hi,lo})) };
    trace!("resolved vhl: q{}=>{}. #deps: {}", qid, nid, self.deps[qid].len());
    self.resolve_nid(qid, nid); }

  fn resolve_part(&mut self, qid:QID, part:BddPart, nid0:NID, invert:bool) {
    if let BddWIP::Parts(ref mut parts) = self.wip[qid] {
      let nid = if invert { not(nid0) } else { nid0 };
      trace!("   !! set {:?} for q{} to {}", part, qid, nid);
      if part == BddPart::HiPart { parts.hi = Some(nid) } else { parts.lo = Some(nid) }}
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
          RMsg::Vhl{v,hi,lo,invert} => { self.resolve_vhl(qid, v, HILO{hi, lo}, invert); }
          RMsg::Wip{v,hi,lo,invert} => {
            // by the time we get here, the task for this node was already created.
            // (add_task already filled in the v for us, so we don't need it.)
            assert_eq!(self.wip[qid], BddWIP::Fresh);
            self.wip[qid] = BddWIP::Parts(BddParts{ v, hi:None, lo:None, invert });
            macro_rules! handle_part { ($xx:ident, $part:expr) => {
              match $xx {
                Norm::Nid(nid) => self.resolve_part(qid, $part, nid, false),
                Norm::Ite(ite) => self.add_task(Some(BddDep::new(qid, $part, false)), ite),
                Norm::Not(ite) => self.add_task(Some(BddDep::new(qid, $part, true)), ite)}}}
            handle_part!(hi, BddPart::HiPart); handle_part!(lo, BddPart::LoPart); }
          RMsg::Ret(n) => { result = Some(n) }}}
      result.unwrap() }}}

    // TODO: at some point, we should kill all the child threads.

    match ITE::norm(i,t,e) {
      Norm::Nid(n) => n,
      Norm::Ite(ite) => { run_swarm_ite!(ite) }
      Norm::Not(ite) => { not(run_swarm_ite!(ite)) }}}

} // end bddswarm

/// Code run by each thread in the swarm. Isolated as a function without channels for testing.
fn swarm_ite<S:BddState>(state: &Arc<S>, ite0:ITE)->RMsg {
  let ITE { i, t, e } = ite0;
  match ITE::norm(i,t,e) {
      Norm::Nid(n) => RMsg::Nid(n),
      Norm::Ite(ite) => swarm_ite_norm(state, ite),
      Norm::Not(ite) => rmsg_not(swarm_ite_norm(state, ite)) }}

fn swarm_vhl_norm<S:BddState>(state: &Arc<S>, ite:ITE)->RMsg {
  let ITE{i:vv,t:hi,e:lo} = ite; let v = var(vv);
  debug_assert!(is_var(vv)); debug_assert_eq!(v, ite.min_var());
  if let Some(&n) = state.get_simple_node(v, HILO{hi,lo}) { RMsg::Nid(n) }
  else { RMsg::Vhl{ v, hi, lo, invert:false } }}

fn swarm_ite_norm<S:BddState>(state: &Arc<S>, ite:ITE)->RMsg {
  let ITE { i, t, e } = ite;
  let (vi, vt, ve) = (var(i), var(t), var(e));
  let v = min(vi, min(vt, ve));
  match state.get_memo(v, &ite) {
    Some(&n) => RMsg::Nid(n),
    None => {
      let (hi_i, lo_i) = if v == vi {state.tup(i)} else {(i,i)};
      let (hi_t, lo_t) = if v == vt {state.tup(t)} else {(t,t)};
      let (hi_e, lo_e) = if v == ve {state.tup(e)} else {(e,e)};
      // now construct and normalize the queries for the hi/lo branches:
      let hi = ITE::norm(hi_i, hi_t, hi_e);
      let lo = ITE::norm(lo_i, lo_t, lo_e);
      // if they're both simple nids, we're guaranteed to have a vhl, so check cache
      if let (Norm::Nid(hn), Norm::Nid(ln)) = (hi,lo) {
        match ITE::norm(nv(v), hn, ln) {
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

  /// add a new tag to the tag map
  pub fn tag(&mut self, s:String, n:NID) { self.tags.insert(s, n); }

  /// retrieve a NID by tag
  pub fn get(&self, s:&String)->Option<NID> { Some(*self.tags.get(s)?) }

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] fn tup(&self, n:NID)->(NID,NID) { self.worker.tup(n) }

  /// retrieve a node by its id.
  pub fn bdd(&self, n:NID)->BDDNode {
    let t=self.tup(n); BDDNode{v:var(n), hi:t.0, lo:t.1 }}

  /// walk node recursively, without revisiting shared nodes
  pub fn walk<F>(&self, n:NID, f:&mut F) where F: FnMut(NID,VID,NID,NID) {
    let mut seen = HashSet::new();
    self.step(n,f,&mut seen)}

  /// internal helper: one step in the walk.
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>)
  where F: FnMut(NID,VID,NID,NID) {
    if !seen.contains(&n) {
      seen.insert(n); let (hi,lo) = self.tup(n); f(n,var(n),hi,lo);
      if !is_const(hi) { self.step(hi, f, seen); }
      if !is_const(lo) { self.step(lo, f, seen); }}}

  pub fn save(&self, path:&str)->::std::io::Result<()> {
    let s = bincode::serialize(&self).unwrap();
    return io::put(path, &s) }

  pub fn load(path:&str)->::std::io::Result<(BDDBase)> {
    let s = io::get(path)?;
    return Ok(bincode::deserialize(&s).unwrap()); }

  // generate dot file (graphviz)
  pub fn dot<T>(&self, n:NID, wr: &mut T) where T : ::std::fmt::Write {
    macro_rules! w {
      ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}

    let fmt = |n:NID| {
      if is_rvar(n) { format!("x{}", rvar(n)) }
      else { format!("{}", n) }};

    w!("digraph bdd {{");
    w!("  I[label=⊤; shape=square];");
    w!("  O[label=⊥; shape=square];");
    w!("node[shape=circle];");
    self.walk(n, &mut |n,_,_,_| w!("  \"{}\"[label=\"{}\"];", n, fmt(n)));
    w!("edge[style=solid];");
    self.walk(n, &mut |n,_,t,_| w!("  \"{}\"->\"{}\";", n, t));
    w!("edge[style=dashed];");
    self.walk(n, &mut |n,_,_,e| w!("  \"{}\"->\"{}\";", n, e));
    w!("}}"); }

  pub fn save_dot(&self, n:NID, path:&str) {
    let mut s = String::new(); self.dot(n, &mut s);
    let mut txt = File::create(path).expect("couldn't create dot file");
    txt.write_all(s.as_bytes()).expect("failed to write text to dot file"); }

  pub fn show_named(&self, n:NID, s:&str) {   // !! almost exactly the same as in bdd.rs
    self.save_dot(n, format!("{}.dot", s).as_str());
    let out = Command::new("dot").args(&["-Tpng",format!("{}.dot",s).as_str()])
      .output().expect("failed to run 'dot' command");
    let mut png = File::create(format!("{}.png",s).as_str()).expect("couldn't create png");
    png.write_all(&out.stdout).expect("couldn't write png");
    Command::new("firefox").args(&[format!("{}.png",s).as_str()])
      .spawn().expect("failed to launch firefox"); }

  pub fn show(&self, n:NID) { self.show_named(n, "+bdd") }


  // public node constructors

  pub fn and(&mut self, x:NID, y:NID)->NID { self.ite(x,  y, O) }
  pub fn xor(&mut self, x:NID, y:NID)->NID { self.ite(x, not(y), y) }
  pub fn  or(&mut self, x:NID, y:NID)->NID { self.ite(x, I, y) }
  pub fn  gt(&mut self, x:NID, y:NID)->NID { self.ite(x, not(y), O) }
  pub fn  lt(&mut self, x:NID, y:NID)->NID { self.ite(x, O, y) }

  /// all-purpose node creation/lookup
  #[inline] pub fn ite(&mut self, f:NID, g:NID, h:NID)->NID { self.worker.ite(f,g,h) }

  /// nid of y when x is high
  pub fn when_hi(&mut self, x:VID, y:NID)->NID {
    let yv = var(y);
    if yv == x { self.tup(y).0 }  // x ∧ if(x,th,_) → th
    else if yv > x { y }          // y independent of x, so no change. includes yv = I
    else {                        // y may depend on x, so recurse.
      let (yt, ye) = self.tup(y);
      let (th,el) = (self.when_hi(x,yt), self.when_hi(x,ye));
      self.ite(nv(yv), th, el) }}

  /// nid of y when x is low
  pub fn when_lo(&mut self, x:VID, y:NID)->NID {
    let yv = var(y);
    if yv == x { self.tup(y).1 }  // ¬x ∧ if(x,_,el) → el
    else if yv > x { y }          // y independent of x, so no change. includes yv = I
    else {                        // y may depend on x, so recurse.
      let (yt, ye) = self.tup(y);
      let (th,el) = (self.when_lo(x,yt), self.when_lo(x,ye));
      self.ite(nv(yv), th, el) }}

  /// is it possible x depends on y?
  /// the goal here is to avoid exploring a subgraph if we don't have to.
  #[inline] pub fn might_depend(&mut self, x:NID, y:VID)->bool {
    if is_var(x) { var(x)==y } else { var(x) <= y }}

  /// replace var x with y in z
  pub fn replace(&mut self, x:VID, y:NID, z:NID)->NID {
    if self.might_depend(z, x) {
      let (zt,ze) = self.tup(z); let zv = var(z);
      if x==zv { self.ite(y, zt, ze) }
      else {
        let th = self.replace(x, y, zt);
        let el = self.replace(x, y, ze);
        self.ite(nv(zv), th, el) }}
    else { z }}

  /// swap input variables x and y within bdd n
  pub fn swap(&mut self, n:NID, x:VID, y:VID)-> NID {
    if y>x { return self.swap(n,y,x) }
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
    let lo = self.ite(nv(y), xlo_ylo, xhi_ylo);
    let hi = self.ite(nv(y), xlo_yhi, xhi_yhi);
    self.ite(nv(x), lo, hi) }

  pub fn node_count(&self, n:NID)->usize {
    let mut c = 0; self.walk(n, &mut |_,_,_,_| c+=1); c }

  /// helper for truth table builder
  fn tt_aux(&mut self, res:&mut Vec<u8>, v:VID, n:NID, i:usize) {
    if v as usize == self.nvars() { match self.when_lo(v, n) {
      O => {} // res[i] = 0; but this is already the case.
      I => { res[i] = 1; }
      x => panic!("expected a leaf nid, got {}", x) }}
    else {
      let lo = self.when_lo(v,n); self.tt_aux(res, v+1, lo, i*2);
      let hi = self.when_hi(v,n); self.tt_aux(res, v+1, hi, i*2+1); }}

  /// Truth table. Could have been Vec<bool> but this is mostly for testing
  /// and the literals are much smaller when you type '1' and '0' instead of
  /// 'true' and 'false'.
  pub fn tt(&mut self, n0:NID)->Vec<u8> {
    if self.nvars() > 16 {
      panic!("refusing to generate a truth table of 2^{} bytes", self.nvars()) }
    let mut res = vec![0;(1 << self.nvars()) as usize];
    self.tt_aux(&mut res, 0, n0, 0);
    res }

} // end impl BddBase

// Base Trait

impl<S:BddState, W:BddWorker<S>> base::Base for BddBase<S,W> {
  type N = NID;
  type V = VID;

  fn new(n:usize)->Self { Self::new(n) }
  fn num_vars(&self)->usize { self.nvars() }

  fn o(&self)->NID { O }
  fn i(&self)->NID { I }
  fn var(&mut self, v:VID)->NID { nid::nv(v) }

  fn when_hi(&mut self, v:VID, n:NID)->NID { self.when_hi(v,n) }
  fn when_lo(&mut self, v:VID, n:NID)->NID { self.when_lo(v,n) }

  // TODO: these should be moved into seperate struct
  fn def(&mut self, s:String, i:u32)->NID { nv(19760820) }  // TODO: make default impl
  fn tag(&mut self, n:NID, s:String)->NID { self.tag(s, n); n }

  fn not(&mut self, x:NID)->NID { not(x) }
  fn and(&mut self, x:NID, y:NID)->NID { self.and(x, y) }
  fn xor(&mut self, x:NID, y:NID)->NID { self.xor(x, y) }
  fn or(&mut self, x:NID, y:NID)->NID  { self.or(x, y) }
  #[cfg(todo)] fn mj(&mut self, x:NID, y:NID, z:NID)->NID  {
    self.xor(x, self.xor(y, z))  // TODO: normalize order. make this the default impl.
  }
  #[cfg(todo)] fn ch(&mut self, x:NID, y:NID, z:NID)->NID { self.ite(x, y, z) }
}


/// The default type used by the rest of the system.
/// (Note the first three letters in uppercase).
#[cfg(safe)]
type S = SafeVarKeyedBddState;
#[cfg(not(safe))]
type S = UnsafeVarKeyedBddState;

#[cfg(feature="noswarm")]
pub type BDDBase = BddBase<S,SimpleBddWorker<S>>;

#[cfg(not(feature="noswarm"))]
pub type BDDBase = BddBase<S,BddSwarm<S>>;


// generic base::Base test suite
test_base_consts!(BDDBase);
test_base_vars!(BDDBase);
test_base_when!(BDDBase);


// basic test suite

#[test] fn test_base() {
  let mut base = BDDBase::new(3);
  let (v1, v2, v3) = (nv(1), nv(2), nv(3));
  assert_eq!(base.nvars(), 3);
  assert_eq!((I,O), base.tup(I));
  assert_eq!((O,I), base.tup(O));
  assert_eq!((I,O), base.tup(v1));
  assert_eq!((I,O), base.tup(v2));
  assert_eq!((I,O), base.tup(v3));
  assert_eq!(I, base.when_hi(3,v3));
  assert_eq!(O, base.when_lo(3,v3))}

#[test] fn test_and() {
  let mut base = BDDBase::new(3);
  let (v1, v2) = (nv(1), nv(2));
  let a = base.and(v1, v2);
  assert_eq!(O,  base.when_lo(1,a));
  assert_eq!(v2, base.when_hi(1,a));
  assert_eq!(O,  base.when_lo(2,a));
  assert_eq!(v1, base.when_hi(2,a));
  assert_eq!(a,  base.when_hi(3,a));
  assert_eq!(a,  base.when_lo(3,a))}

#[test] fn test_xor() {
  let mut base = BDDBase::new(3);
  let (v1, v2) = (nv(1), nv(2));
  let x = base.xor(v1, v2);
  assert_eq!(v2,      base.when_lo(1,x));
  assert_eq!(not(v2), base.when_hi(1,x));
  assert_eq!(v1,      base.when_lo(2,x));
  assert_eq!(not(v1), base.when_hi(2,x));
  assert_eq!(x,       base.when_lo(3,x));
  assert_eq!(x,       base.when_hi(3,x))}

// swarm test suite
pub type BddSwarmBase = BddBase<SafeVarKeyedBddState,BddSwarm<SafeVarKeyedBddState>>;

#[test] fn test_swarm_xor() {
  let mut base = BddSwarmBase::new(2);
  let (x0, x1) = (nv(0), nv(1));
  let x = base.xor(x0, x1);
  assert_eq!(x1,      base.when_lo(0,x));
  assert_eq!(not(x1), base.when_hi(0,x));
  assert_eq!(x0,      base.when_lo(1,x));
  assert_eq!(not(x0), base.when_hi(1,x));
  assert_eq!(x,       base.when_lo(2,x));
  assert_eq!(x,       base.when_hi(2,x))}

#[test] fn test_swarm_and() {
  let mut base = BddSwarmBase::new(2);
  let (x0, x1) = (nv(0), nv(1));
  let a = base.and(x0, x1);
  assert_eq!(O,  base.when_lo(0,a));
  assert_eq!(x1, base.when_hi(0,a));
  assert_eq!(O,  base.when_lo(1,a));
  assert_eq!(x0, base.when_hi(1,a));
  assert_eq!(a,  base.when_hi(2,a));
  assert_eq!(a,  base.when_lo(2,a))}

/// slightly harder test case that requires ite() to recurse
#[test] fn test_swarm_ite() {
  //use simplelog::*;  TermLogger::init(LevelFilter::Trace, Config::default()).unwrap();
  let mut base = BddSwarmBase::new(3);
  let (x0,x1,x2) = (nv(0), nv(1), nv(2));
  assert_eq!(vec![0,0,0,0,1,1,1,1], base.tt(x0));
  assert_eq!(vec![0,0,1,1,0,0,1,1], base.tt(x1));
  assert_eq!(vec![0,1,0,1,0,1,0,1], base.tt(x2));
  let x = base.xor(x0, x1);
  assert_eq!(vec![0,0,1,1,1,1,0,0], base.tt(x));
  let a = base.and(x1, x2);
  assert_eq!(vec![0,0,0,1,0,0,0,1], base.tt(a));
  let i = base.ite(x, a, not(a));
  assert_eq!(vec![1,1,0,1,0,0,1,0], base.tt(i)); }


/// slightly harder test case that requires ite() to recurse
#[test] fn test_swarm_another() {
  use simplelog::*;  TermLogger::init(LevelFilter::Trace, Config::default()).unwrap();
  let mut base = BddSwarmBase::new(4);
  let (a,b,c,d) = (nv(0), nv(1), nv(2), nv(3));
  let anb = base.and(a,not(b));
  assert_eq!(vec![0,0,0,0,0,0,0,0,1,1,1,1,0,0,0,0], base.tt(anb));

  let anb_nb = base.xor(anb,not(b));
  assert_eq!(vec![1,1,1,1,0,0,0,0,0,0,0,0,0,0,0,0], base.tt(anb_nb));
  let anb2 = base.xor(not(b), anb_nb);
  assert_eq!(vec![0,0,0,0,0,0,0,0,1,1,1,1,0,0,0,0], base.tt(anb2));
  assert_eq!(anb, anb2);
}

