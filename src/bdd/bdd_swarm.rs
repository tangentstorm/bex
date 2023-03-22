use std::{fmt, sync::mpsc::Sender};
use std::sync::Arc;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use {wip, wip::{Dep, WorkCache, Work}};
use vhl::{HiLoPart, VhlParts};
use {vid::VID, nid::{NID}, vhl::{HiLo}};
use bdd::{ITE, NormIteKey, Norm, BddState, COUNT_XMEMO_TEST, COUNT_XMEMO_FAIL};
use {swarm, swarm::{WID, QID, Swarm, RMsg}};
use concurrent_queue::{ConcurrentQueue,PopError};

use crate::wip::ResStep;

// ----------------------------------------------------------------
// BddSwarm Protocol
// ----------------------------------------------------------------

#[derive(Debug)]
struct IteQueue{q: ConcurrentQueue<ITE>}
impl Default for IteQueue {
    fn default() -> Self { IteQueue { q: ConcurrentQueue::unbounded() }}}
impl IteQueue {
  fn push(&self, ite:ITE) { self.q.push(ite).unwrap(); }
  fn pop(&self)->Option<ITE> {
    match self.q.pop() {
      Ok(ite) => Some(ite),
      Err(PopError::Empty) => None,
      Err(PopError::Closed) => panic!("IteQueue was closed!") }}}

/// Query message for BddSwarm.
#[derive(Clone)]
 enum Q {
  /// The main recursive operation: convert ITE triple to a BDD.
  Ite(NormIteKey),
  /// Initialize worker with its "hive mind".
  Init(Arc<BddState>, Arc<IteQueue>),
  /// ask for stats about cache
  Stats }

type R = wip::RMsg;

// Q::Cache() message could potentially be huge to print, so don't.
impl std::fmt::Debug for Q {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Q::Ite(ite) => { write!(f, "Q::Ite({:?})", ite) }
      Q::Init(_cache, _queue) => { write!(f, "Q::Init(...)") }
      Q::Stats => { write!(f, "Q::Stats")} } }}

// ----------------------------------------------------------------

#[derive(Debug, Default)]
struct BddWorker {
  wid:WID,
  // channel for sending back to the swarm
  tx:Option<Sender<RMsg<R>>>,
  state:Option<Arc<BddState>>,
  queue:Option<Arc<IteQueue>> }

impl swarm::Worker<Q,R,ITE> for BddWorker {
  fn new(wid:WID)->Self { BddWorker{ wid, ..Default::default() }}
  fn get_wid(&self)->WID { self.wid }
  fn set_tx(&mut self, tx:&Sender<RMsg<R>>) { self.tx = Some(tx.clone()) }

  // TODO: would be nice to never have an un-initialized worker.
  //       this would require dropping this Q::Init concept.
  //  (Ideally we would remove the Option<> on .state and .queue)
  // !! Since the work_loop function is now non-blocking, it will
  //    try to pop from this queue even before a Q::Init message
  //    has been sent. So we have to do these dumb existence checks.
  fn queue_pop(&self)->Option<ITE> {
    if let Some(ref q) = self.queue { q.pop() }
    else { None }}

  fn queue_push(&self, item:ITE) {
    if let Some(ref q) = self.queue { q.push(item) }}

  fn work_item(&mut self, _item:ITE) {  }

  fn work_step(&mut self, _qid:&QID, q:Q)->Option<R> {
    match q {
      Q::Init(s, q) => { self.state = Some(s); self.queue=Some(q); None }
      Q::Ite(ite) => { Some(R::Res(ite, self.ite_norm(ite))) }
      Q::Stats => {
        let tests = COUNT_XMEMO_TEST.with(|c| c.replace(0));
        let fails = COUNT_XMEMO_FAIL.with(|c| c.replace(0));
        Some(R::MemoStats{ tests, fails }) } }}}

/// Code run by each thread in the swarm. Isolated as a function without channels for testing.
impl BddWorker {

  fn vhl_norm(&self, ite:NormIteKey)->ResStep {
    let ITE{i:vv,t:hi,e:lo} = ite.0; let v = vv.vid();
    if let Some(n) = self.state.as_ref().unwrap().get_simple_node(v, HiLo{hi,lo}) {
      ResStep::Nid(n) }
    else { ResStep::Vhl{ v, hi, lo, invert:false } }}

  fn ite_norm(&self, ite:NormIteKey)->ResStep {
    let ITE { i, t, e } = ite.0;
    let (vi, vt, ve) = (i.vid(), t.vid(), e.vid());
    let v = ite.0.top_vid(); let state = self.state.as_ref().unwrap();
    match state.get_memo(&ite) {
      Some(n) => ResStep::Nid(n),
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
            // !! but wait. how is this possible? i.is_const() and v == fake variable "T"?
            Norm::Nid(n) => { ResStep::Nid(n) }
            // otherwise, the normalized triple might already be in cache:
            Norm::Ite(ite) => self.vhl_norm(ite),
            Norm::Not(ite) => !self.vhl_norm(ite)}}
        // otherwise at least one side is not a simple nid yet, and we have to defer
        else { ResStep::Wip{ v, hi, lo, invert:false } }}}} }


// ----------------------------------------------------------------
/// BddSwarm: a multi-threaded swarm implementation
// ----------------------------------------------------------------
#[derive(Debug, Default)]
pub struct BddSwarm {
  swarm: Swarm<Q,R,BddWorker,ITE>,
  /// reference to state shared by all threads.
  state: Arc<BddState>,
  queue: Arc<IteQueue>,
  // work in progress
  work: Arc<WorkCache>}

impl Serialize for BddSwarm {
  fn serialize<S:Serializer>(&self, ser: S)->Result<S::Ok, S::Error> {
    // all we really care about is the state:
    self.state.serialize::<S>(ser) } }

impl<'de> Deserialize<'de> for BddSwarm {
  fn deserialize<D:Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
    let mut res = Self::new();
    res.state = Arc::new(BddState::deserialize(d)?);
    Ok(res) }}


impl BddSwarm {

  pub fn new()->Self {
    let mut me = Self::default();
    me.swarm.send_to_all(&Q::Init(me.state.clone(), me.queue.clone()));
    me }

  pub fn tup(&self, n:NID)->(NID,NID) { self.state.tup(n) }

  /// all-purpose if-then-else node constructor. For the swarm implementation,
  /// we push all the normalization and tree traversal work into the threads,
  /// while this function puts all the parts together.
  pub fn ite(&mut self, i:NID, t:NID, e:NID)->NID {
    match ITE::norm(i,t,e) {
      Norm::Nid(n) => n,
      Norm::Ite(ite) => { self.run_swarm_ite(ite) }
      Norm::Not(ite) => { !self.run_swarm_ite(ite) }}} }


impl BddSwarm {

  fn add_query(&mut self, ite:NormIteKey) {
      let _v = self.work.cache.entry(ite).or_default();
      // TODO: push to queue
      self.swarm.add_query(Q::Ite(ite)); }

  fn add_wip(&self, q:NormIteKey, v:VID, invert:bool) {
    if let wip::Work::Todo(w) = self.work.cache.get(&q).unwrap().value() {
      let mut val = w.borrow_mut();
      val.parts.v = v;
      val.parts.invert = invert; }
    else { panic!("got wip for non-Todo task"); }}

  fn add_sub_task(&mut self, idep:Dep<NormIteKey>, ite:NormIteKey) {

    let mut done_nid = None; let mut was_empty = false;
    { // -- new way -- add_sub_task
      // this handles both the occupied and vacant cases:
      let v = self.work.cache.entry(ite).or_insert_with(|| {
        was_empty = true;
        Work::default()});
      match v.value() {
        wip::Work::Todo(w) => w.borrow_mut().deps.push(idep),
        wip::Work::Done(n) => done_nid=Some(*n) }}
    if let Some(nid)=done_nid {
      self.resolve_part(&idep.dep, idep.part, nid, idep.invert); }
    if was_empty { self.add_query(ite); }}


  /// called whenever the wip resolves to a single nid
  fn resolve_nid(&mut self, ite:&NormIteKey, nid:NID) {
    let mut ideps = vec![];
    { // update work_cache and extract the ideps
      let mut v = self.work.cache.get_mut(ite).unwrap();
      if let Work::Done(old) = v.value() {
        warn!("resolving an already resolved nid for {:?}", ite);
        assert_eq!(*old, nid, "old and new resolutions didn't match!") }
      else {
        ideps = std::mem::take(&mut v.value().wip().borrow_mut().deps);
        *v = Work::Done(nid) }}
    self.state.xmemo.insert(*ite, nid);  // (only while xmemo still exists)
    if ideps.is_empty() { self.swarm.send_to_self(R::Ret(nid)) }
    else { for d in ideps { self.resolve_part(&d.dep, d.part, nid, d.invert); }}}

  /// called whenever the wip resolves to a new simple (v/hi/lo) node.
  fn resolve_vhl(&mut self, ite:&NormIteKey, v:VID, hilo:HiLo, invert:bool) {
    let HiLo{hi:h0,lo:l0} = hilo;
    // we apply invert first so it normalizes correctly.
    let (h1,l1) = if invert { (!h0, !l0) } else { (h0, l0) };
    let nid = match ITE::norm(NID::from_vid(v), h1, l1) {
      Norm::Nid(n) => n,
      Norm::Ite(NormIteKey(ITE{i:vv,t:hi,e:lo})) =>
        self.state.simple_node(vv.vid(), HiLo{hi,lo}),
      Norm::Not(NormIteKey(ITE{i:vv,t:hi,e:lo})) =>
       !self.state.simple_node(vv.vid(), HiLo{hi,lo})};
    self.resolve_nid(ite, nid) }

  fn resolve_part(&mut self, ite:&NormIteKey, part:HiLoPart, nid:NID, invert:bool) {
    let mut parts = VhlParts::default();
    { // -- new way --
      let v = self.work.cache.get_mut(ite).unwrap();
      match v.value() {
        wip::Work::Todo(w) => {
          let n = if invert { !nid } else { nid };
          w.borrow_mut().parts.set_part(part, Some(n));
          parts = w.borrow().parts.clone() }
        wip::Work::Done(_) => {} }}

    if let Some(hilo) = parts.hilo() {
        self.resolve_vhl(ite, parts.v, hilo, parts.invert); }}


  fn run_swarm_ite(&mut self, ite0:NormIteKey)->NID {
    let mut result: Option<NID> = None;
    self.add_query(ite0);
    // each response can lead to up to two new ITE queries, and we'll relay those to
    // other workers too, until we get back enough info to solve the original query.
    while result.is_none() {
      let RMsg{wid:_,qid:_,r} = self.swarm.recv().expect("failed to recieve rmsg");
      if let Some(rmsg) = r { match rmsg {
        R::Res(q_ite, step) => match step {
          ResStep::Nid(nid) =>  {
            self.resolve_nid(&q_ite, nid); }
          ResStep::Vhl{v,hi,lo,invert} => {
            self.resolve_vhl(&q_ite, v, HiLo{hi, lo}, invert); }
          ResStep::Wip{v,hi,lo,invert} => {
            self.add_wip(q_ite, v, invert);
            for &(xx, part) in &[(hi,HiLoPart::HiPart), (lo,HiLoPart::LoPart)] {
              match xx {
                Norm::Nid(nid) => self.resolve_part(&q_ite, part, nid, false),
                Norm::Ite(ite) => { self.add_sub_task(Dep::new(q_ite, part, false), ite);},
                Norm::Not(ite) => { self.add_sub_task(Dep::new(q_ite, part, true), ite);}}}}}
        R::Ret(n) => { result = Some(n) }
        R::MemoStats{ tests:_, fails:_ }
          => { panic!("got R::MemoStats before sending Q::Stats"); } }}}
    result.unwrap() }

  pub fn get_stats(&mut self) {
    self.swarm.send_to_all(&Q::Stats);
    let (mut tests, mut fails, mut reports, mut shorts) = (0, 0, 0, 0);
    // // println!("waiting for MemoStats");
    while reports < self.swarm.num_workers() {
       let RMsg{wid:_, qid:_, r} = self.swarm.recv().expect("still expecting an Rmsg::MemoCount");
       if let Some(wip::RMsg::MemoStats{ tests:t, fails: f }) = r { reports += 1; tests+=t; fails += f }
       else { shorts += 1; println!("extraneous rmsg from swarm: {:?}", r) }}
    // if tests > 0 { println!("{:?} result: {:?}  tests: {}  fails: {}  hits: {}", ite, result, tests, fails, tests-fails); }
    if shorts > 0 { println!("----------- shorts: {}", shorts)} // i don't think this actually happens.
    COUNT_XMEMO_TEST.with(|c| *c.borrow_mut() += tests );
    COUNT_XMEMO_FAIL.with(|c| *c.borrow_mut() += fails ); }

} // end bddswarm
