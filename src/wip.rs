//! Generic Work-in-progress support for VHL graphs.
use std::borrow::BorrowMut;
use std::default::Default;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::{collections::HashMap};
use std::hash::Hash;
use std::sync::Mutex;
use nid::NID;
use vid::VID;
use vhl::{HiLo, HiLoPart, VhlParts, HiLoCache};
use bdd::{Norm, NormIteKey};
use dashmap::DashMap;



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

  pub fn is_todo(&self)->bool {
    if let Self::Todo(_) = self { true } else { false }}

  pub fn is_done(&self)->bool {
    if let Self::Done(_) = self { true } else { false }}

  pub fn unwrap(&self)->&V {
    if let Self::Done(v) = self { &v } else {
      panic!("cannot unwrap() a Work::Todo") }}

  pub fn wip_mut(&mut self)->&mut W {
    if let Self::Todo(w) = self { w } else {
      panic!("cannot get wip() from a Work::Done") }}

  pub fn wip(&self)->&W {
    if let Self::Todo(w) = self { &w } else {
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
  pub cache: DashMap<K, Work<V, WipRef<K,P>>> }

impl<K:Eq+Hash+Debug,V:Clone> WorkState<K,V> {

  /// If the key exists in the cache AND the work is
  /// done, return the completed value, otherwise
  /// return None.
  pub fn get_done(&self, k:&K)->Option<V> {
    if let Some(w) = self.cache.get(k) {
      match w.value() {
        Work::Todo(_) => None,
        Work::Done(v) => Some(v.clone())}}
    else { None }}

  pub fn get_cached_nid(&self, v:VID, hi:NID, lo:NID)->Option<NID> {
    self.hilos.get_node(v, HiLo{hi,lo})}

  pub fn vhl_to_nid(&self, v:VID, hi:NID, lo:NID)->NID {
    match self.hilos.get_node(v, HiLo{hi,lo}) {
      Some(n) => n,
      None => { self.hilos.insert(v, HiLo{hi, lo}) }}}

  pub fn get_hilo(&self, n:NID)->HiLo { self.hilos.get_hilo(n) }}

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
    use crate::bdd::{ITE}; // TODO: normalization strategy might need to be generic
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
          parts = w.borrow_mut().parts.clone() }
        Work::Done(x) => { warn!("got part for K:{:?} ->Work::Done({:?})", q, x) } }}

    if let Some(HiLo{hi, lo}) = parts.hilo() {
      self.resolve_vhl(q, parts.v, hi, lo, parts.invert) }
    else { None}}

    /// set the branch variable and invert flag on the work in progress value
    pub fn add_wip(&self, q:&K, vid:VID, invert:bool) {
      if self.cache.contains_key(&q) {
        self.cache.alter(&q, |_k, v| match v {
          Work::Todo(Wip{parts,deps}) => {
            let mut p = parts; p.v = vid; p.invert = invert;
            Work::Todo(Wip{parts:p,deps})},
          Work::Done(_) => panic!("got wip for a Work::Done")})}
        else { panic!("got wip for unknown task");}}

    // returns true if the query is new to the system
    pub fn add_dep(&self, q:&K, idep:Dep<K>)->(bool, Option<Answer<NID>>) {
      let mut old_done = None; let mut was_empty = false; let mut answer = None;
      { // -- new way -- add_sub_task
        // this handles both the occupied and vacant cases:
        let mut v = self.cache.entry(*q).or_insert_with(|| {
          was_empty = true;
          Work::default()});
        match v.value_mut() {
          Work::Todo(w) => w.borrow_mut().deps.push(idep),
          Work::Done(n) => old_done=Some(*n) }}
      if let Some(nid)=old_done {
        answer = self.resolve_part(&idep.dep, idep.part, nid, idep.invert); }
      (was_empty, answer) }
  }



// one step in the resolution of a query.
// !! to be replaced by direct calls to
//    work.cache.resolve_nid, resolve_vhl, resolve_part
#[derive(PartialEq,Debug)]
pub enum ResStep {
  /// resolved to a nid
  Nid(NID),
  /// a simple node needs to be constructed:
  Vhl{v:VID, hi:NID, lo:NID, invert:bool},
  /// other work in progress
  Wip{v:VID, hi:Norm, lo:Norm, invert:bool}}

impl std::ops::Not for ResStep {
  type Output = ResStep;
  fn not(self)->ResStep {
    match self {
      ResStep::Nid(n) => ResStep::Nid(!n),
      ResStep::Vhl{v,hi,lo,invert} => ResStep::Vhl{v,hi,lo,invert:!invert},
      ResStep::Wip{v,hi,lo,invert} => ResStep::Wip{v,hi,lo,invert:!invert} }}}

/// Response message.
#[derive(PartialEq,Debug)]
pub enum RMsg {
  /// We've solved the whole problem, so exit the loop and return this nid.
  Ret(NID),
  /// return stats about the memo cache
  MemoStats { tests: u64, fails: u64 }}
