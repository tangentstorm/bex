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
//! - [`Wip<K,P>`] has `parts: P` and `deps: Vec<Dep<K,P::Slot>>`.
//! - [`Dep<K,S>`] tracks which other queries are dependent on this one. It has
//!     a slot selector and an `invert` flag.
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
use crate::vhl::{HiLo, VhlBase, VhlSlots, VhlParts};
use crate::bdd::{Norm, NormIteKey};
use dashmap::DashMap;

// cache lookup counters:
thread_local!{
  pub static COUNT_CACHE_TESTS: RefCell<u64> = const { RefCell::new(0) };
  pub static COUNT_CACHE_HITS: RefCell<u64> = const { RefCell::new(0) }; }



pub type WIPHashMap<K,V> = HashMap<K,V,fxhash::FxBuildHasher>;

pub trait Parts: Copy + Default {
  type Slot: Copy + Eq + Debug;

  fn set_slot(&mut self, slot: Self::Slot, nid: Option<NID>);
  fn is_ready(&self)->bool;
}

impl Parts for VhlParts {
  type Slot = VhlSlots;

  fn set_slot(&mut self, slot: Self::Slot, nid: Option<NID>) {
    self.set_slot(slot, nid)
  }

  fn is_ready(&self)->bool { self.hi.is_some() && self.lo.is_some() }
}

pub enum JobResult<K> {
  Done(NID),
  Follow(K),
}

pub trait WipBase<K, P:Parts> : Debug + Default + Send + Sync {
  fn resolve_job(&self, parts:P)->JobResult<K>;
}

#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub enum DepTarget<S> {
  Slot(S),
  Result,
}

#[derive(Debug,Copy,Clone)]
pub struct Dep<K, S> { pub dep: K, pub target: DepTarget<S>, pub invert: bool }
impl<K,S> Dep<K,S>{
  pub fn new(dep: K, slot: S, invert: bool)->Dep<K,S> {
    Dep{dep, target:DepTarget::Slot(slot), invert} }
  pub fn result(dep:K)->Dep<K,S> {
    Dep{dep, target:DepTarget::Result, invert:false} }}

#[derive(Debug)]
pub struct Wip<K=NormIteKey, P=VhlParts> where P:Parts {
  pub parts : P,
  pub deps : Vec<Dep<K, P::Slot>>
}

impl<K, P> Default for Wip<K, P> where P:Parts {
  fn default() -> Self {
    Wip { parts: P::default(), deps: vec![] }
  }
}

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
#[derive(Debug)]
pub struct Answer<T>(pub T); // TODO: nopub

#[derive(Debug)]
pub struct WorkResult<K> {
  pub answer: Option<Answer<NID>>,
  pub jobs: Vec<K>,
}

impl<K> WorkResult<K> {
  pub fn with_answer(nid:NID)->Self {
    Self { answer: Some(Answer(nid)), jobs: vec![] }
  }

  pub fn push_job(&mut self, job:K) { self.jobs.push(job) }

  pub fn merge(&mut self, mut other:WorkResult<K>) {
    if other.answer.is_some() { self.answer = other.answer.take() }
    self.jobs.append(&mut other.jobs);
  }
}

impl<K> Default for WorkResult<K> {
  fn default()->Self { Self { answer: None, jobs: vec![] } }
}

/// Thread-safe map of queries->results, including results
/// that are currently under construction.
#[derive(Debug)]
pub struct WorkState<K=NormIteKey, P=VhlParts, B=VhlBase>
where K:Eq+Hash+Debug, P:Parts, B:WipBase<K,P> {
  _kvp: PhantomData<(K,P)>,
  /// this is a kludge. it locks entire swarm from taking in new
  /// queries until an answer is found, because it's the only place
  /// we currently have to remember the query id. (since there's only
  /// one slot, we can only have one top level query at a time)
  pub qid:Mutex<Option<crate::swarm::QID>>, // pub so BddWorker can see it
  pub base: B,
  // TODO: make .cache private
  pub cache: DashMap<K, Work<NID, WipRef<K,P>>, fxhash::FxBuildHasher> }

impl<K,P,B> Default for WorkState<K,P,B>
where K:Eq+Hash+Debug, P:Parts, B:WipBase<K,P> {
  fn default() -> Self {
    Self {
      _kvp: PhantomData,
      qid: Mutex::new(None),
      base: B::default(),
      cache: DashMap::with_capacity_and_hasher_and_shard_amount(1 << 18, fxhash::FxBuildHasher::default(), 128),
    }
  }
}

impl<K:Eq+Hash+Debug+Copy,P:Parts,B:WipBase<K,P>> WorkState<K,P,B> {

  /// If the key exists in the cache AND the work is
  /// done, return the completed value, otherwise
  /// return None.
  pub fn get_done(&self, k:&K)->Option<NID> {
    COUNT_CACHE_TESTS.with(|c| *c.borrow_mut() += 1);
    if let Some(w) = self.cache.get(k) {
      match w.value() {
        Work::Todo(_) => None,
        Work::Done(v) => {
          COUNT_CACHE_HITS.with(|c| *c.borrow_mut() += 1);
          Some(*v)}}}
    else { None }}

  pub fn resolve_job(&self, q:&K, nid:NID)->WorkResult<K> {
    let mut ideps = vec![];
    {
      let mut v = self.cache.get_mut(q).unwrap();
      if let Work::Done(old) = v.value() {
        warn!("resolving an already resolved nid for {:?}", q);
        assert_eq!(*old, nid, "old and new resolutions didn't match!");
      } else {
        ideps = std::mem::take(&mut v.value_mut().wip_mut().deps);
        *v = Work::Done(nid);
      }
    }
    let mut res = if ideps.is_empty() { WorkResult::with_answer(nid) } else { WorkResult::default() };
    for d in ideps {
      res.merge(self.resolve_dep(d, nid));
    }
    res
  }

  fn resolve_dep(&self, d:Dep<K, P::Slot>, nid:NID)->WorkResult<K> {
    match d.target {
      DepTarget::Slot(slot) => self.resolve_part(&d.dep, slot, nid, d.invert),
      DepTarget::Result => {
        let n = if d.invert { !nid } else { nid };
        self.resolve_job(&d.dep, n)
      }
    }
  }

  pub fn resolve_part(&self, q:&K, slot:P::Slot, nid:NID, invert:bool)->WorkResult<K> {
    let mut parts = None;
    {
      let mut v = self.cache.get_mut(q).unwrap();
      match v.value_mut() {
        Work::Todo(w) => {
          let n = if invert { !nid } else { nid };
          w.borrow_mut().parts.set_slot(slot, Some(n));
          let new_parts = w.borrow_mut().parts;
          if new_parts.is_ready() { parts = Some(new_parts) }
        }
        Work::Done(x) => {
          warn!("got part for K:{:?} ->Work::Done({:?})", q, x);
        }
      }
    }

    if let Some(parts) = parts {
      match self.base.resolve_job(parts) {
        JobResult::Done(nid) => self.resolve_job(q, nid),
        JobResult::Follow(job) => {
          let (was_new, mut res) = self.add_dep(&job, Dep::result(*q));
          if was_new { res.push_job(job) }
          res
        }
      }
    } else {
      WorkResult::default()
    }
  }

  // returns true if the query is new to the system
  pub fn add_dep(&self, q:&K, idep:Dep<K, P::Slot>)->(bool, WorkResult<K>) {
    COUNT_CACHE_TESTS.with(|c| *c.borrow_mut() += 1);
    let mut old_done = None;
    let mut was_empty = false;
    let mut res = WorkResult::default();
    {
      let mut v = self.cache.entry(*q).or_insert_with(|| {
        was_empty = true;
        Work::default()
      });
      if !was_empty { COUNT_CACHE_HITS.with(|c| *c.borrow_mut() += 1) }
      match v.value_mut() {
        Work::Todo(w) => w.borrow_mut().deps.push(idep),
        Work::Done(n) => old_done = Some(*n),
      }
    }
    if let Some(nid)=old_done {
      res = self.resolve_dep(idep, nid);
    }
    (was_empty, res)
  }

  pub fn add_wip(&self, q:&K, parts:P)->WorkResult<K> {
    let mut res = None;
    if self.cache.contains_key(q) {
      self.cache.alter(q, |_k, v| match v {
        Work::Todo(Wip{parts:_,deps}) => Work::Todo(Wip{parts, deps}),
        Work::Done(nid) => {
          res = Some(Answer(nid));
          Work::Done(nid)
        }
      });
    } else { panic!("got wip for unknown task"); }
    WorkResult { answer: res, jobs: vec![] }
  }
}

// TODO: nopub these methods
impl<K:Eq+Hash+Debug+Default+Copy> WorkState<K,VhlParts,VhlBase> {
  pub fn len(&self)->usize { self.base.len() }
  #[must_use] pub fn is_empty(&self) -> bool { self.len() == 0 }

  pub fn get_cached_nid(&self, v:VID, hi:NID, lo:NID)->Option<NID> {
    self.base.get_cached_nid(v, hi, lo)}

  pub fn vhl_to_nid(&self, v:VID, hi:NID, lo:NID)->NID {
    self.base.vhl_to_nid(v, hi, lo)}

  pub fn get_hilo(&self, n:NID)->HiLo { self.base.get_hilo(n) }

  /// return (hi, lo) pair for the given nid. used internally
  #[inline] pub fn tup(&self, n:NID)-> (NID, NID) {
    self.base.tup(n) }

}



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
