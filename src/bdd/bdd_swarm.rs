use std::borrow::BorrowMut;
use std::{fmt, sync::mpsc::Sender};
use std::sync::Arc;
use {wip, wip::{Dep, Work, ResStep, Answer}};
use vhl::{HiLoPart};
use {vid::VID, nid::{NID}, vhl::{HiLo}};
use bdd::{ITE, NormIteKey, Norm, BddState, COUNT_XMEMO_TEST, COUNT_XMEMO_FAIL};
use {swarm, swarm::{WID, QID, Swarm, RMsg}};
use concurrent_queue::{ConcurrentQueue,PopError};

// ----------------------------------------------------------------
// BddSwarm Protocol
// ----------------------------------------------------------------

#[derive(Debug)]
struct IteQueue{q: ConcurrentQueue<NormIteKey>}
impl Default for IteQueue {
    fn default() -> Self { IteQueue { q: ConcurrentQueue::unbounded() }}}
impl IteQueue {
  fn push(&self, ite:NormIteKey) { self.q.push(ite).unwrap(); }
  fn pop(&self)->Option<NormIteKey> {
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

impl swarm::Worker<Q,R,NormIteKey> for BddWorker {
  fn new(wid:WID)->Self { BddWorker{ wid, ..Default::default() }}
  fn get_wid(&self)->WID { self.wid }
  fn set_tx(&mut self, tx:&Sender<RMsg<R>>) { self.tx = Some(tx.clone()) }

  // TODO: would be nice to never have an un-initialized worker.
  //       this would require dropping this Q::Init concept.
  //  (Ideally we would remove the Option<> on .state and .queue)
  // !! Since the work_loop function is now non-blocking, it will
  //    try to pop from this queue even before a Q::Init message
  //    has been sent. So we have to do these dumb existence checks.
  fn queue_pop(&self)->Option<NormIteKey> {
    if let Some(ref q) = self.queue { q.pop() }
    else { None }}

  fn queue_push(&self, _ite:NormIteKey) {
    if let Some(ref q) = self.queue { q.push(_ite) }}

  fn work_item(&mut self, _ite:NormIteKey) {
    // TODO: self.ite_norm(ite)
  }

  fn work_step(&mut self, _qid:&QID, q:Q)->Option<R> {
    match q {
      Q::Init(s, q) => { self.state = Some(s); self.queue=Some(q); None }
      // Q::Ite(ite) => { self.queue_push(ite); None }
      Q::Ite(ite) => { Some(R::Res(ite, self.ite_norm(ite))) }
      Q::Stats => {
        let tests = COUNT_XMEMO_TEST.with(|c| c.replace(0));
        let fails = COUNT_XMEMO_FAIL.with(|c| c.replace(0));
        Some(R::MemoStats{ tests, fails }) } }}}

/// Code run by each thread in the swarm. Isolated as a function without channels for testing.
impl BddWorker {

  fn vhl_norm(&self, ite:NormIteKey)->ResStep {
    let ITE{i:vv,t:hi,e:lo} = ite.0; let v = vv.vid();
    ResStep::Nid(self.state.as_ref().unwrap().work.vhl_to_nid(v, hi, lo)) }

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
  swarm: Swarm<Q,R,BddWorker,NormIteKey>,
  /// reference to state shared by all threads.
  state: Arc<BddState>,
  queue: Arc<IteQueue>}


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
      let _v = self.state.work.cache.entry(ite).or_default();
      // TODO: push to queue
      self.swarm.add_query(Q::Ite(ite)); }

  fn add_wip(&self, q:NormIteKey, vid:VID, invert:bool) {
    if self.state.work.cache.contains_key(&q) {
      self.state.work.cache.alter(&q, |_k, v| match v {
        Work::Todo(wip::Wip{parts,deps}) => {
          let mut p = parts; p.v = vid; p.invert = invert;
          Work::Todo(wip::Wip{parts:p,deps})},
        Work::Done(_) => panic!("got wip for a Work::Done")})}
      else { panic!("got wip for unknown task");}}

  fn add_sub_task(&mut self, idep:Dep<NormIteKey>, ite:NormIteKey) {

    let mut done_nid = None; let mut was_empty = false;
    { // -- new way -- add_sub_task
      // this handles both the occupied and vacant cases:
      let mut v = self.state.work.cache.entry(ite).or_insert_with(|| {
        was_empty = true;
        Work::default()});
      match v.value_mut() {
        wip::Work::Todo(w) => w.borrow_mut().deps.push(idep),
        wip::Work::Done(n) => done_nid=Some(*n) }}
    if let Some(nid)=done_nid {
      self.resolve_part(&idep.dep, idep.part, nid, idep.invert); }
    if was_empty { self.add_query(ite); }}


  /// called whenever the wip resolves to a single nid
  fn resolve_nid(&mut self, q:&NormIteKey, nid:NID) {
    if let Some(Answer(a)) = self.state.work.resolve_nid(q, nid) {
      self.swarm.send_to_self(R::Ret(a))}}

  /// called whenever the wip resolves to a new simple (v/hi/lo) node.
  fn resolve_vhl(&mut self, q:&NormIteKey, v:VID, hilo:HiLo, invert:bool) {
    if let Some(Answer(a)) = self.state.work.resolve_vhl(q, v, hilo.hi, hilo.lo, invert) {
      self.swarm.send_to_self(R::Ret(a))}}

  fn resolve_part(&mut self, q:&NormIteKey, part:HiLoPart, nid:NID, invert:bool) {
    if let Some(Answer(a)) = self.state.work.resolve_part(q, part, nid, invert) {
      self.swarm.send_to_self(R::Ret(a))}}


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
