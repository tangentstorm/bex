//! Generic Work-in-progress support, used by e.g. [`VhlSwarm`].
//!
//! In this module, the main structure is [`WorkState<K,V,P>`].
//!
//! - `K` is the hashable key for some query issued to the system.
//! - `V` is the type of values produced by the system.
//! - `P` is the type of parts that are being assembled to produce the final value.
//!
//! Inside the `WorkState`, each `K` is mapped to a `Work<V, WipRef<K,P>>`.
//! - [`Work`] is either `Todo(WipRef<K,P>)` or `Done(V)`.
//! - [`WipRef`] is really just `Wip<K,P>`.
//! - [`Wip<K,P>`] has `parts: P` and `deps: Vec<Dep<K>>`.
//! - [`Dep<K>`] tracks which other queries are dependent on this one. It has
//!     a `HiLoPart` and an `invert` flag. (TODO: explicit use of invert and
//!     HiloPart should probably be in a `VhlDep` struct.)
//!
//! With this framework, we can track the progress of a distributed computation.
//!
//! The main change I forsee making here is making `Dep` an enum, that also
//! includes an option to return a top-level result for a numbered query, as
//! currently only one query is allowed.
//!
//! It would also be quite nice if dependencies could be "released" when a query
//! "short circuits". Ex: if a constant 0 bubbles up to one side of an "AND" expression,
//! we ought to be able to cancel the other side recursively (without necessarily throwing
//! away the work that's been done so far).
//!
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::default::Default;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use crate::nid::NID;
use crate::vid::VID;
use crate::vhl::{HiLo, HiLoPart, VhlParts, HiLoCache};
use crate::bdd::{Norm, NormIteKey};
use dashmap::DashMap;

// cache lookup counters:
thread_local!{
  pub static COUNT_CACHE_TESTS: RefCell<u64> = const { RefCell::new(0) };
  pub static COUNT_CACHE_HITS: RefCell<u64> = const { RefCell::new(0) }; }



pub type WIPHashMap<K,V> = HashMap<K,V,fxhash::FxBuildHasher>;

#[derive(Debug,Copy,Clone)]
pub struct Dep<K> { pub dep: K, pub part: HiLoPart, pub invert: bool }
impl<K> Dep<K>{
  pub fn new(dep: K, part: HiLoPart, invert: bool)->Dep<K> { Dep{dep, part, invert} }}

#[derive(Debug, Default)]
pub struct Wip<K=NormIteKey, P=VhlParts> { pub parts : P, pub deps : Vec<Dep<K>> }

// TODO: wrap this with a smart pointer so Work::Done and Work::Todo are both usizes.
type WipRef<K=NormIteKey, P=VhlParts> = Wip<K, P>;

#[derive(Debug)]
pub enum Work<V, W=WipRef> { Todo(W), Done(V) }

impl<V,W> Default for Work<V, W> where W:Default {
    fn default() -> Self { Work::Todo(W::default()) }}

impl<V,W> Work<V,W> {

  pub fn is_todo(&self)->bool { matches!(self, Self::Todo(_))}

  pub fn is_done(&self)->bool { matches!(self, Self::Done(_))}

  pub fn unwrap(&self)->&V {
    if let Self::Done(v) = self { v } else {
      panic!("cannot unwrap() a Work::Todo") }}

  pub fn wip_mut(&mut self)->&mut W {
    if let Self::Todo(w) = self { w } else {
      panic!("cannot get wip() from a Work::Done") }}

  pub fn wip(&self)->&W {
    if let Self::Todo(w) = self { w } else {
      panic!("cannot get wip() from a Work::Done") }}}


/// Wrapper class to indicate a value is the final result
/// to the distributed problem we're solving.
pub struct Answer<T>(pub T); // TODO: nopub

/// Thread-safe map of queries->results, including results
/// that are currently under construction.
#[derive(Debug, Default)]
pub struct WorkState<K=NormIteKey, V=NID, P=VhlParts> where K:Eq+Hash+Debug {
  _kvp: PhantomData<(K,V,P)>,
  /// this is a kludge. it locks entire swarm from taking in new
  /// queries until an answer is found, because it's the only place
  /// we currently have to remember the query id. (since there's only
  /// one slot, we can only have one top level query at a time)
  pub qid:Mutex<Option<crate::swarm::QID>>, // pub so BddWorker can see it
  /// cache of hi,lo pairs.
  hilos: HiLoCache,
  // TODO: make .cache private
  pub cache: DashMap<K, Work<V, WipRef<K,P>>, fxhash::FxBuildHasher> }

impl<K:Eq+Hash+Debug,V:Clone> WorkState<K,V> {

  pub fn len(&self)->usize { self.hilos.len() }
  #[must_use] pub fn is_empty(&self) -> bool { self.len() == 0 }

  /// If the key exists in the cache AND the work is
  /// done, return the completed value, otherwise
  /// return None.
  pub fn get_done(&self, k:&K)->Option<V> {
    COUNT_CACHE_TESTS.with(|c| *c.borrow_mut() += 1);
    if let Some(w) = self.cache.get(k) {
      match w.value() {
        Work::Todo(_) => None,
        Work::Done(v) => {
          COUNT_CACHE_HITS.with(|c| *c.borrow_mut() += 1);
          Some(v.clone())}}}
    else { None }}

  pub fn get_cached_nid(&self, v:VID, hi:NID, lo:NID)->Option<NID> {
    self.hilos.get_node(v, HiLo{hi,lo})}

  pub fn vhl_to_nid(&self, v:VID, hi:NID, lo:NID)->NID {
    match self.hilos.get_node(v, HiLo{hi,lo}) {
      Some(n) => n,
      None => { self.hilos.insert(v, HiLo{hi, lo}) }}}

  pub fn get_hilo(&self, n:NID)->HiLo { self.hilos.get_hilo(n) }

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] pub fn tup(&self, n:NID)-> (NID, NID) {
    use crate::nid::{I,O};
    if n.is_const() { if n==I { (I, O) } else { (O, I) } }
    else if n.is_vid() { if n.is_inv() { (O, I) } else { (I, O) }}
    else { let hilo = self.get_hilo(n); (hilo.hi, hilo.lo) }} }

// TODO: nopub these methods
impl<K:Eq+Hash+Debug+Default+Copy> WorkState<K,NID> {
  pub fn resolve_nid(&self, q:&K, nid:NID)->Option<Answer<NID>> {
    let mut ideps = vec![];
    { // update work_cache and extract the ideps
      let mut v = self.cache.get_mut(q).unwrap();
      if let Work::Done(old) = v.value() {
        warn!("resolving an already resolved nid for {:?}", q);
        assert_eq!(*old, nid, "old and new resolutions didn't match!") }
      else {
        ideps = std::mem::take(&mut v.value_mut().wip_mut().deps);
        *v = Work::Done(nid) }}
    if ideps.is_empty() { Some(Answer(nid)) }
    else {
      let mut res = None;
      for d in ideps {
        if let Some(Answer(a)) = self.resolve_part(&d.dep, d.part, nid, d.invert) {
          res =Some(Answer(a)) }}
      res }}

  pub fn resolve_vhl(&self, q:&K, v:VID, h0:NID, l0:NID, invert:bool)->Option<Answer<NID>> {
    use crate::bdd::ITE; // TODO: normalization strategy might need to be generic
    // we apply invert first so it normalizes correctly.
    let (h1,l1) = if invert { (!h0, !l0) } else { (h0, l0) };
    let nid = match ITE::norm(NID::from_vid(v), h1, l1) {
      Norm::Nid(n) => n,
      Norm::Ite(NormIteKey(ITE{i:vv,t:hi,e:lo})) =>
        self.vhl_to_nid(vv.vid(), hi, lo),
      Norm::Not(NormIteKey(ITE{i:vv,t:hi,e:lo})) =>
       !self.vhl_to_nid(vv.vid(), hi, lo)};
    self.resolve_nid(q, nid) }

  pub fn resolve_part(&self, q:&K, part:HiLoPart, nid:NID, invert:bool)->Option<Answer<NID>> {
    let mut parts = VhlParts::default();
    { // -- new way --
      let mut v = self.cache.get_mut(q).unwrap();
      match v.value_mut() {
        Work::Todo(w) => {
          let n = if invert { !nid } else { nid };
          w.borrow_mut().parts.set_part(part, Some(n));
          parts = w.borrow_mut().parts }
        Work::Done(x) => { warn!("got part for K:{:?} ->Work::Done({:?})", q, x) } }}

    if let Some(HiLo{hi, lo}) = parts.hilo() {
      self.resolve_vhl(q, parts.v, hi, lo, parts.invert) }
    else { None}}

    /// set the branch variable and invert flag on the work in progress value
    pub fn add_wip(&self, q:&K, vid:VID, invert:bool)->Option<Answer<NID>> {
      let mut res = None;
      if self.cache.contains_key(q) {
        self.cache.alter(q, |_k, v| match v {
          Work::Todo(Wip{parts,deps}) => {
            let mut p = parts; p.v = vid; p.invert = invert;
            Work::Todo(Wip{parts:p,deps})},
          Work::Done(nid) => {
            res = Some(Answer(nid));
            Work::Done(nid) }});}
        else { panic!("got wip for unknown task");}
      res }

    // returns true if the query is new to the system
    pub fn add_dep(&self, q:&K, idep:Dep<K>)->(bool, Option<Answer<NID>>) {
      COUNT_CACHE_TESTS.with(|c| *c.borrow_mut() += 1);
      let mut old_done = None; let mut was_empty = false; let mut answer = None;
      { // -- new way -- add_sub_task
        // this handles both the occupied and vacant cases:
        let mut v = self.cache.entry(*q).or_insert_with(|| {
          was_empty = true;
          Work::default()});
        if !was_empty { COUNT_CACHE_HITS.with(|c| *c.borrow_mut() += 1) }
        match v.value_mut() {
          Work::Todo(w) => w.borrow_mut().deps.push(idep),
          Work::Done(n) => old_done=Some(*n) }}
      if let Some(nid)=old_done {
        answer = self.resolve_part(&idep.dep, idep.part, nid, idep.invert); }
      (was_empty, answer) }}



// one step in the resolution of a query.
// !! to be replaced by direct calls to
//    work.cache.resolve_nid, resolve_vhl, resolve_part
#[derive(PartialEq,Debug)]
pub enum ResStep {
  /// resolved to a nid
  Nid(NID),
  /// other work in progress
  Wip{v:VID, hi:Norm, lo:Norm, invert:bool}}

impl std::ops::Not for ResStep {
  type Output = ResStep;
  fn not(self)->ResStep {
    match self {
      ResStep::Nid(n) => ResStep::Nid(!n),
      ResStep::Wip{v,hi,lo,invert} => ResStep::Wip{v,hi,lo,invert:!invert} }}}

/// Response message.
#[derive(PartialEq,Debug)]
pub enum RMsg {
  /// We've solved the whole problem, so exit the loop and return this nid.
  Ret(NID),
  /// return stats about the memo cache
  CacheStats { tests: u64, hits: u64 }}
