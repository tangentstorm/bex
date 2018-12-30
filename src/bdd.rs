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

use serde::Serialize;
use bincode;
use io;


// core data types

/// Variable ID: uniquely identifies an input variable in the BDD.
pub type VID = u32;
/// Index into a (usually VID-specific) vector.
pub type IDX = u32;

/// A BDDNode is a triple consisting of a VID, which references an input variable,
/// and high and low branches, each pointing at other nodes in the BDD. The
/// associated variable's value determines which branch to take.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BDDNode { pub v:VID, pub hi:NID, pub lo:NID } // if|then|else

/// A NID represents a node in the BDD. Essentially, this acts like a tuple
/// containing a VID and IDX, but for performance reasons, it is packed into a u64.
/// See below for helper functions that manipulate and analyze the packed bits.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct NID { n: u64 }

/// Single-bit mask representing that a NID is inverted.
const INV:u64 = 1<<63;  // is inverted?

/// Single-bit mask indicating that a NID represents a variable. (The corresponding
/// "virtual" nodes have I as their hi branch and O as their lo branch. They're simple
/// and numerous enough that we don't bother actually storing them.)
const VAR:u64 = 1<<62;   // is variable?

/// In addition, for solving, we want to distinguish between "virtual" variables which
/// represent some intermediate, unsimplified calculation, and "real" variables, which
/// represent actual input variables. That's what this bit does.
const RVAR:u64 = 1<<60;  // is *real* variable?

/// Single-bit mask indicating that the NID represents a constant. The corresponding
/// virtual node branches on constant "true" value, hence the letter T. There is only
/// one such node -- O (I is its inverse) but having this bit in the NID lets us
/// easily detect and optimize the cases.
const T:u64 = 1<<61;    // T: max VID (hack so O/I nodes show up at bottom)

/// Constant used to extract the index part of a NID.
const IDX_MASK:u64 = (1<<32)-1;

/// NID of the virtual node represeting the constant function 0, or "always false."
pub const O:NID = NID{ n:T };
/// NID of the virtual node represeting the constant function 1, or "always true."
pub const I:NID = NID{ n:(T|INV) };

// NID support routines

/// Does the NID represent a variable?
#[inline(always)] pub fn is_var(x:NID)->bool { (x.n & VAR) != 0 }
/// Does the NID represent a *real* variable?
#[inline(always)] pub fn is_rvar(x:NID)->bool { (x.n & RVAR) != 0 }

/// Is the NID inverted? That is, does it represent `not(some other nid)`?
#[inline(always)] pub fn is_inv(x:NID)->bool { (x.n & INV) != 0 }

/// Does the NID refer to one of the two constant nodes (O or I)?
#[inline(always)] pub fn is_const(x:NID)->bool { (x.n & T) != 0 }

/// Map the NID to an index. (I,e, if n=idx(x), then x is the nth node branching on var(x))
#[inline(always)] pub fn idx(x:NID)->usize { (x.n & IDX_MASK) as usize }

/// On which variable does this node branch? (I and O branch on TV)
#[inline(always)] pub fn var(x:NID)->VID { ((x.n & !(INV|VAR)) >> 32) as VID}
/// Same as var() but strips out the RVAR bit.
#[inline(always)] pub fn rvar(x:NID)->VID { ((x.n & !(INV|VAR|RVAR)) >> 32) as VID}

/// internal function to strip rvar bit and convert to usize
#[inline(always)] fn rv(v:VID)->usize { (v&!((RVAR>>32) as VID)) as usize}

/// Toggle the INV bit, applying a logical "NOT" operation to the corressponding node.
#[inline(always)] pub fn not(x:NID)->NID { NID { n:x.n^INV } }

/// Construct the NID for the (virtual) node corresponding to an input variable.
#[inline(always)] pub fn nv(v:VID)->NID { NID { n:((v as u64) << 32)|VAR }}

/// Construct the NID for the (virtual) node corresponding to an input variable.
#[inline(always)] pub fn nvr(v:VID)->NID { NID { n:((v as u64) << 32)|VAR|RVAR }}

/// Construct a NID with the given variable and index.
#[inline(always)] pub fn nvi(v:VID,i:IDX)->NID { NID{ n:((v as u64) << 32) + i as u64 }}

/// Pretty-printer for NIDS that reveal some of their internal data.
impl fmt::Display for NID {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if is_const(*self) { if is_inv(*self) { write!(f, "I") } else { write!(f, "O") } }
    else { if is_inv(*self) { write!(f, "¬")?; }
           if is_var(*self) {
             if is_rvar(*self) { write!(f, "x{}", rvar(*self)) }
             else { write!(f, "v{}", var(*self)) }}
           else if is_rvar(*self) { write!(f, "@[x{}:{}]", rvar(*self), idx(*self)) }
           else { write!(f, "@[v{}:{}]", var(*self), idx(*self)) }}}}

/// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
impl fmt::Debug for NID { // for test suite output
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}

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
      else if T==(T & g.n & h.n) { // both const, and we know g!=h
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

/// Simple Hi/Lo pair stored internally when representing nodes.
/// All nodes with the same branching variable go in the same array, so there's
/// no point duplicating it.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HILO {hi:NID, lo:NID}

impl HILO {
  /// constructor
  fn new(hi:NID, lo:NID)->HILO { HILO { hi:hi, lo:lo } }

  /// apply the not() operator to both branches
  #[inline] fn invert(self)-> HILO { HILO{ hi: not(self.hi), lo: not(self.lo) }} }


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
  Smp{v:VID, hi:NID, lo:NID, invert:bool},
  /// other work in progress
  Wip{v:VID, hi:Norm, lo:Norm, invert:bool},
  /// We've solved the whole problem, so exit the loop and return this nid.
  Ret(NID)}

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
struct BddParts{ v:VID, hi:Option<NID>, lo:Option<NID>}
impl BddParts {
  // fn is_complete(&self)->bool { self.hi.is_some() && self.lo.is_some() }
  fn hilo(&self)->Option<HILO> {
    if let (Some(hi), Some(lo)) = (self.hi, self.lo) { Some(HILO{hi,lo}) } else { None }}}

/// Work in progress for BddSwarm.
#[derive(Debug,Copy,Clone)]
enum BddWIP { Done(NID), Parts(BddParts) }
impl BddWIP {
  /// construct a new WIP node that branches on the given variable
  fn new(v:VID)->BddWIP { BddWIP::Parts(BddParts{ v, hi:None, lo:None }) }}

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


use serde::ser::{Serializer};
impl<TState:BddState> Serialize for BddSwarm<TState> {
  fn serialize<S:Serializer>(&self, ser: S)->Result<S::Ok, S::Error> {
    // all we really care about is the state:
    self.stable.serialize::<S>(ser) } }


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
    let (qid, is_dup) = {
      if let Some(&dup) = self.qid.get(&ite) { (dup, true) }
      else { (self.wip.len(), false) }};
    if is_dup {
      if let Some(dep) = opt_dep {
        trace!("*** dup of task: {} invert: {}", qid, dep.invert);
        if let BddWIP::Done(nid) = self.wip[qid] { self.resolve_nid(dep.qid, nid) }
        else { self.deps[qid].push(dep) }}
      else { panic!("Got duplicate request, but no dep. This should never happen!") }}
    else {
      self.qid.insert(ite, qid); self.ites.push(ite);
      let w:usize = qid % self.swarm.len();
      self.swarm[w].send(QMsg::Ite(qid, ite)).expect("send to swarm failed");
      self.wip.push(BddWIP::new(ite.min_var()));
      if let Some(dep) = opt_dep {
        trace!("*** adding task #{}: {:?} invert:{}", qid, ite, dep.invert);
        self.deps.push(vec![dep]) }
      else {
        if qid == 0 {
          trace!("*** adding task #{}: {:?} (no deps!)", qid, ite);
          self.deps.push(vec![]) }
        else { panic!("non 0 qid with no deps!?") }}}}

  /// called whenever the wip resolves to a single nid
  fn resolve_nid(&mut self, qid:QID, nid:NID) {
    if let BddWIP::Parts(_) = self.wip[qid] {
      trace!("resolved_nid: q{}=>{}. deps: {:?}", qid, nid, self.deps[qid].clone());
      self.wip[qid] = BddWIP::Done(nid);
      let ite = self.ites[qid];
      self.recent.put_xmemo(ite, nid); }
    for &dep in self.deps[qid].clone().iter() {
      self.resolve_part(dep.qid, dep.part, nid, dep.invert) }
    if qid == 0 { self.me.send((0, RMsg::Ret(nid))).expect("failed to send Ret"); }}

  /// called whenever the wip resolves to a new simple (v/hi/lo) node.
  fn resolve_vhl(&mut self, qid:QID, v:VID, hilo:HILO, invert:bool) {
    let nid0 = self.recent.simple_node(v, hilo);
    let nid = if invert { not(nid0) } else { nid0 };
    trace!("resolved vhl: q{}=>{}. #deps: {}", qid, nid, self.deps[qid].len());
    self.resolve_nid(qid, nid); }

  fn resolve_part(&mut self, qid:usize, part:BddPart, nid0:NID, invert:bool) {
    if let BddWIP::Parts(ref mut parts) = self.wip[qid] {
      let nid = if invert { not(nid0) } else { nid0 };
      trace!("   !! set {:?} for q{} to {}", part, qid, nid);
      if part == BddPart::HiPart { parts.hi = Some(nid) } else { parts.lo = Some(nid) }}
    else { warn!("???? got a part for a qid #{} that was already done!", qid) }
    if let BddWIP::Parts(wip) = self.wip[qid] {
      if let Some(hilo) = wip.hilo() { self.resolve_vhl(qid, wip.v, hilo, false) }}}
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
    while self.swarm.len() < 2 { // TODO: configure threads here.
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
    macro_rules! swarm_ite { ($ite:expr) => {{
      self.init_swarm(); self.add_task(None, $ite);
      let mut result:Option<NID> = None;
      // each response can lead to up to two new ITE queries, and we'll relay those to
      // other workers too, until we get back enough info to solve the original query.
      while result.is_none() {
        let (qid, rmsg) = self.rx.recv().expect("failed to read RMsg from queue!");
        trace!("===> run_swarm got RMsg {}: {:?}", qid, rmsg);
        match rmsg {
          RMsg::Nid(nid) =>  { self.resolve_nid(qid, nid); }
          RMsg::Smp{v,hi,lo,invert} => { self.resolve_vhl(qid, v, HILO{hi, lo}, invert); }
          RMsg::Wip{v:_,hi,lo,invert} => {
            // by the time we get here, the task for this node was already created.
            // (add_task already filled in the v for us, so we don't need it.)
            macro_rules! handle_part { ($xx:ident, $part:expr) => {
              match $xx {
                Norm::Nid(nid) => self.resolve_part(qid, $part, nid, invert),
                Norm::Ite(ite) => self.add_task(Some(BddDep::new(qid, $part,  invert)), ite),
                Norm::Not(ite) => self.add_task(Some(BddDep::new(qid, $part, !invert)), ite) }}}
            handle_part!(hi, BddPart::HiPart); handle_part!(lo, BddPart::LoPart); }
          RMsg::Ret(n) => { result = Some(n) }}}
      result.unwrap() }}}

    // TODO: at some point, we should kill all the child threads.

    match ITE::norm(i,t,e) {
      Norm::Nid(n) => n,
      Norm::Ite(ite) => { swarm_ite!(ite) }
      Norm::Not(ite) => { not(swarm_ite!(ite)) }}}

} // end bddswarm

/// Code run by each thread in the swarm:

fn swarm_loop<S:BddState>(tx:RTx, rx:QRx<S>, mut state:Arc<S>) {
  for qmsg in rx.iter() {
    match qmsg {
      QMsg::Cache(s) => { state = s }
      QMsg::Ite(qid, ITE{i:i0,t:t0,e:e0}) => {
        trace!("got qmsg with qid = {}", qid);

        macro_rules! yld { ($rmsg:expr) => { if tx.send((qid, $rmsg)).is_err() { break } }}

        macro_rules! smp_norm { ($v:expr, $hh:expr, $ll:expr, $inv:expr) => {{
          if let Some(&n) = state.get_simple_node($v,HILO{hi:$hh,lo:$ll}) {
            yld!(RMsg::Nid(if $inv { not(n) } else { n })) }
          else { yld!(RMsg::Smp{ v:$v, hi:$hh, lo:$ll, invert:$inv }) }}}}

        macro_rules! swarm_ite_norm { ($ite:expr, $invert:expr)=> {{
          let ITE{i,t,e} = $ite; let invert = $invert;
          let (vi, vt, ve) = (var(i), var(t), var(e));
          let v = min(vi, min(vt, ve));
          match state.get_memo(v,&$ite) {
            Some(&n) => yld!(RMsg::Nid(if invert { not(n) } else { n })),
            None => {
              let (hi_i, lo_i) = if v == vi {state.tup(i)} else {(i,i)};
              let (hi_t, lo_t) = if v == vt {state.tup(t)} else {(t,t)};
              let (hi_e, lo_e) = if v == ve {state.tup(e)} else {(e,e)};

              // now construct and normalize the queries for the hi/lo branches:
              let hi = ITE::norm(hi_i, hi_t, hi_e);
              let lo = ITE::norm(lo_i, lo_t, lo_e);

              // if they're both simple nids, see if the memo already exists.
              if let (Norm::Nid(hn), Norm::Nid(ln)) = (hi,lo) {
                match ITE::norm(nv(v), hn, ln) {
                  Norm::Nid(n) => {yld!(RMsg::Nid(if invert { not(n) } else { n }))}
                  Norm::Ite(ITE{i:vv,t:hh,e:ll}) => smp_norm!(var(vv),hh,ll, invert),
                  Norm::Not(ITE{i:vv,t:hh,e:ll}) => smp_norm!(var(vv),hh,ll,!invert)}}
              else { yld!(RMsg::Wip{ v, hi, lo, invert }) }}} }}}

        trace!("--->   thread worker got qmsg {}: {:?}", qid, qmsg);
        match ITE::norm(i0, t0, e0) {
          Norm::Nid(x) => yld!(RMsg::Nid(x)),
          Norm::Ite(ite) => swarm_ite_norm!(ite, false),
          Norm::Not(ite) => swarm_ite_norm!(ite, true) }}}}}


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
    txt.write_all(s.as_bytes()).expect("failet to write text to dot file"); }

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


/// The default type used by the rest of the system.
/// (Note the first three letters in uppercase).
#[cfg(safe)]
type S = SafeVarKeyedBddState;
#[cfg(not(safe))]
type S = UnsafeVarKeyedBddState;

pub type BDDBase = BddBase<S,SimpleBddWorker<S>>;



// basic test suite

#[test] fn test_nids() {
  assert_eq!(O, NID{n:0x2000000000000000u64});
  assert_eq!(I, NID{n:0xa000000000000000u64});
  assert_eq!(nv(0),  NID{n:0x4000000000000000u64});
  assert_eq!(nvr(0), NID{n:0x5000000000000000u64});
  assert_eq!(nv(1),  NID{n:0x4000000100000000u64});
  assert!(var(nv(0)) < var(nvr(0)));
  assert_eq!(nvi(0,0), NID{n:0x0000000000000000u64});
  assert_eq!(nvi(1,0), NID{n:0x0000000100000000u64}); }

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
  debug!("\n----| x = x0 xor x1 |----");
  let x = base.xor(x0, x1);
  assert_eq!(vec![0,0,1,1,1,1,0,0], base.tt(x));
  debug!("\n----| a = x1 and x2 |----");
  let a = base.and(x1, x2);
  assert_eq!(vec![0,0,0,1,0,0,0,1], base.tt(a));
  debug!("\n----| x ? a : !a |----");
  let i = base.ite(x, a, not(a));
  assert_eq!(vec![1,1,0,1,0,0,1,0], base.tt(i)); }
