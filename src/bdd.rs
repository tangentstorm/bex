/// A module for efficient implementation of binary decision diagrams.
use std::cmp::min;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::marker::PhantomData;
use std::process::Command;      // for creating and viewing digarams
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
#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ITE {i:NID, t:NID, e:NID}

/// This represents the result of normalizing an ITE. There are three conditions:
#[derive(Debug)]
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
  /// shorthand constructor
  pub fn new (i:NID, t:NID, e:NID)-> ITE { ITE { i:i, t:t, e:e } }

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
pub trait BddState : Sized + Serialize {

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
  #[inline] fn put_xmemo(&mut self, v:VID, ite:ITE, new_nid:NID);
  #[inline] fn get_simple_node<'a>(&'a self, v:VID, hilo:HILO)-> Option<&'a NID>;
  #[inline] fn put_simple_node(&mut self, v:VID, hilo:HILO)->NID; }


/// Groups everything by variable. I thought this would be useful, but it probably is not.
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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

  #[inline] fn put_xmemo(&mut self, v:VID, ite:ITE, new_nid:NID) {
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

  #[inline] fn put_xmemo(&mut self, v:VID, ite:ITE, new_nid:NID) { unsafe {
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
        if !is_var(i) { self.state.put_xmemo(v, ite, new_nid) }
        new_nid }}} }

// -- message types for multi-threaded programming -----------------------------

#[derive(PartialEq,Debug)]
enum Intent { Add(u32, u32) }

#[derive(PartialEq,Debug)]
enum Event { Sum(u32) }

struct IntentMsg { id:usize, msg:Intent }
struct EventMsg { id:usize, msg:Event }


fn work(tx:Sender<EventMsg>, rx:Receiver<IntentMsg>) {
  loop {
    match rx.recv().expect("aaaaah!") {
      IntentMsg{ id, msg } => {
        println!("Worker got msg {}: {:?}", id, msg);
        match msg {
          Intent::Add(x,y) => {
            tx.send(EventMsg{id, msg:Event::Sum(x+y)})
              .expect("I TOLD you you'd regret not writing a better error message someday.");
          }} }} }}

struct Master {
  /// receives messages from the workers
  rx: Receiver<EventMsg>,
  /// send messages to myself (so we can put them back in the queue.
  me: Sender<EventMsg>,
  id: usize,
  workers: Vec<Sender<IntentMsg>> }

impl Master {
  fn new()->Master {
    let (me, rx) = channel::<EventMsg>();
    let mut workers = vec![];
    for _ in 0..2 {
      let (tx, rx) = channel::<IntentMsg>();
      let me_clone = me.clone();
      thread::spawn(|| { work(me_clone, rx) });
      workers.push(tx); }
    Master{ me, rx, id:0, workers}}

  fn run(&mut self, x:Intent)->Event {
    let w:usize = self.id % self.workers.len();
    self.workers[w].send(IntentMsg{ id:self.id, msg:x }).expect("ugh");
    let mut result:Option<Event> = None;
    while result.is_none() {
      let EventMsg{id, msg} = self.rx.recv().expect("oh no!");
      println!("Master got msg {}: {:?}", id, msg);
      if id==self.id { result = Some(msg) }
      else { self.me.send(EventMsg{id, msg}).expect(":/"); }}
    self.id += 1;
    result.expect("got invalid result?") } }




// ----------------------------------------------------------------
/// ThreadedBddWorker: a multi-threaded worker implementation
// ----------------------------------------------------------------
#[derive(Debug, Serialize, Deserialize)]
pub struct ThreadedBddWorker<S:BddState> { state:S }

impl<S:BddState> BddWorker<S> for ThreadedBddWorker<S> {
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

// !! everything above this line is exactly the same in SimpleBddWorker.
// TODO: clean up duplicate code between Simple/Threaded Bdd Workers

impl<S:BddState> ThreadedBddWorker<S> {
  #[inline] fn ite_norm(&mut self, ite:ITE)->NID { I }
}



/// This is the top-level type for this crate.
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
