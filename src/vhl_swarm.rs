//! # VHL Swarm
//!
//! Combines notions from the [`swarm`](crate::swarm), [`wip`], and [`vhl`](crate::vhl)
//! modules to create a distributed system for solving VHL queries. The main idea is
//! the swarm workers share a common [`WorkState`] and can delegate tasks to each
//! other by pushing new jobs onto a shared queue.
//!
//! For a complete example, see [`bdd_swarm`](crate::bdd::bdd_swarm).

use std::sync::mpsc::Sender;
use std::{fmt, hash::Hash};
use std::sync::Arc;
use concurrent_queue::{ConcurrentQueue,PopError};
use crate::vhl::HiLoPart;
use crate::vid::VID;
use crate::wip::Answer;
use crate::NID;
use crate::{wip, wip::{WorkState, COUNT_CACHE_HITS, COUNT_CACHE_TESTS}};
use crate::swarm::{RMsg, Swarm, SwarmCmd, Worker, QID, WID};

type R = wip::RMsg;

pub trait JobKey : 'static + Copy+Clone+Default+std::fmt::Debug+Eq+Hash+Send+Sync {}

/// wrapper struct for concurrent queue. This exists mostly so we
/// can implement Default for it. The J parameter indicates a "Job",
/// which is some kind of message indicating a request to (eventually)
/// construct a VHL.
#[derive(Debug)]
pub struct JobQueue<J> { q: ConcurrentQueue<J> }
impl<J> Default for JobQueue<J> {
  fn default()->Self { JobQueue{ q: ConcurrentQueue::unbounded() }}}
impl<J> JobQueue<J> where J:std::fmt::Debug {
  pub fn push(&self, job:J) { self.q.push(job).unwrap() }
  pub fn pop(&self)->Option<J> {
    match self.q.pop() {
      Ok(k) => Some(k),
      Err(PopError::Empty) => None,
      Err(PopError::Closed) => panic!("JobQueue was closed!") }}}

/// Query messages used by the swarm. There are several general
/// messages (Init, Stats) that we want for all implementations.
/// Each implementation has a different kind of "Job" message, though,
/// so we introduce type parameter J to represent that.
#[derive(Clone)]
pub enum VhlQ<J> where J:JobKey {
  /// The main recursive operation: convert ITE triple to a BDD.
  Job(J),
  /// Initialize worker with its "hive mind".
  Init(Arc<WorkState<J>>, Arc<JobQueue<J>>),
  /// ask for stats about cache
  Stats }

// Q::Cache() message could potentially be huge to print, so don't.
impl<J> fmt::Debug for VhlQ<J> where J:JobKey {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      VhlQ::Job(j) => { write!(f, "Q::Job({:?})", j) }
      VhlQ::Init(_cache, _queue) => { write!(f, "Q::Init(...)") }
      VhlQ::Stats => { write!(f, "Q::Stats")} } }}

pub trait VhlJobHandler<J> : Default where J: JobKey {
  type W : Worker<VhlQ<J>, R, J>;
  fn work_job(&mut self, w: &mut Self::W, job:J); }

#[derive(Debug, Default)]
pub struct VhlWorker<J, H> where J:JobKey, H:VhlJobHandler<J,W=Self> {
  /// worker id
  wid: WID,
  /// channel for sending back to the swarm
  tx:Option<Sender<RMsg<R>>>,
  /// quick access to the next job in the queue
  next: Option<J>,
  /// shared state for all workers
  state:Option<Arc<WorkState<J>>>,
  queue:Option<Arc<JobQueue<J>>>,
  handler: H }

/// These methods expose the WorkState methods on the worker itself.
impl<J,H> VhlWorker<J, H> where J:JobKey, H:VhlJobHandler<J,W=Self> {
  pub fn vhl_to_nid(&self, v:VID, hi:NID, lo:NID)->NID {
    self.state.as_ref().unwrap().vhl_to_nid(v, hi, lo) }
  pub fn resolve_nid(&mut self, q:&J, n:NID)->Option<Answer<NID>> {
    self.state.as_ref().unwrap().resolve_nid(q, n) }
  pub fn add_wip(&mut self, q:&J, vid:VID, invert:bool)->Option<Answer<NID>> {
    self.state.as_ref().unwrap().add_wip(q, vid, invert) }
  pub fn resolve_part(&mut self, q:&J, part:HiLoPart, nid:NID, invert:bool)->Option<Answer<NID>> {
    self.state.as_ref().unwrap().resolve_part(q, part, nid, invert) }
  pub fn add_dep(&mut self, q:&J, idep:wip::Dep<J>)->(bool, Option<Answer<NID>>) {
    self.state.as_ref().unwrap().add_dep(q, idep) }
  pub fn get_done(&self, q:&J)->Option<NID> {
    self.state.as_ref().unwrap().get_done(q) }
  pub fn tup(&self, n:NID)->(NID,NID) {
    self.state.as_ref().unwrap().tup(n) }}

/// this lets a JobHandler send answers and sub-tasks to the swarm.
impl<J,H> VhlWorker<J,H> where J:JobKey, H:VhlJobHandler<J,W=Self> {
  pub fn send_answer(&self, _q:&J, nid:NID) {
    // println!("!! final answer: {:?} !!", nid);
    let qid = {
      let mut mx = self.state.as_ref().unwrap().qid.lock().unwrap();
      let q0 = (*mx).expect("no qid found in the mutex!");
      *mx = None; // unblock the next query!
      q0};
    self.send_msg(qid, Some(R::Ret(nid))) }
  pub fn delegate(&mut self, job:J) {
    self.queue_push(job)}
  pub fn send_msg(&self, qid:QID, r:Option<R>) {
    self.tx.as_ref().unwrap().send(RMsg{wid:self.wid, qid, r}).unwrap() }}

impl<J,H> Worker<VhlQ<J>, R, J> for VhlWorker<J,H> where J:JobKey, H:VhlJobHandler<J,W=Self> {
  fn new(wid:WID)->Self { VhlWorker{ wid, ..Default::default() }}
  fn get_wid(&self)->WID { self.wid }
  fn set_tx(&mut self, tx:&Sender<RMsg<R>>) { self.tx = Some(tx.clone()) }
  fn queue_pop(&mut self)->Option<J> {
    if self.next.is_some() { self.next.take() }
    else if let Some(ref q) = self.queue { q.pop() }
    else { None }}
  fn queue_push(&mut self, job:J) {
    if self.next.is_none() { self.next = Some(job) }
    else { self.queue.as_ref().unwrap().push(job) }}
  fn work_item(&mut self, job:J) {
    // swap the handler out of self so it can borrow us mutably
    let mut h = std::mem::take(&mut self.handler);
    h.work_job(self, job);
    self.handler = h; }
  fn work_step(&mut self, qid:&QID, q:VhlQ<J>)->Option<R> {
    match q {
      VhlQ::Init(s, q) => { self.state = Some(s); self.queue=Some(q); None }
      VhlQ::Job(job) => {
        let s = self.state.as_mut().unwrap();
        if let Some(cached) = s.get_done(&job) { return Some(R::Ret(cached)) }
        s.cache.entry(job).or_default();
        { let mut m = s.qid.lock().unwrap();
          assert!((*m).is_none(), "already working on a top-level query");
          *m = Some(*qid); }
        self.queue_push(job); None }
      VhlQ::Stats => {
        let tests = COUNT_CACHE_TESTS.with(|c| c.replace(0));
        let hits = COUNT_CACHE_HITS.with(|c| c.replace(0));
        Some(R::CacheStats{ tests, hits }) } }}}


#[derive(Debug, Default)]
pub struct VhlSwarm<J, H> where J:JobKey, H:VhlJobHandler<J,W=VhlWorker<J,H>>{
  swarm: Swarm<VhlQ<J>, R, VhlWorker<J, H>, J>,
  state: Arc<WorkState<J>>,
  queue: Arc<JobQueue<J>>}

impl<J,H> VhlSwarm<J,H> where J:JobKey, H:VhlJobHandler<J,W=VhlWorker<J,H>> {

  pub fn new()->Self { let mut me = Self::default(); me.reset(); me }

  pub fn new_with_threads(n:usize)->Self {
    let mut me = Self {
      swarm: Swarm::new_with_threads(n),
      ..Default::default()};
    me.reset(); me }

  pub fn run<F,V>(&mut self, on_msg:F)->Option<V>
  where V:fmt::Debug, F:FnMut(WID, &QID, Option<R>)->SwarmCmd<VhlQ<J>, V> {
    self.swarm.run(on_msg)}

  pub fn q_sender(&self)->Sender<VhlQ<J>> { self.swarm.q_sender() }

  // reset internal state without the cost of destroying and recreating
  // all the worker threads.
  pub fn reset(&mut self) {
    self.state = Default::default();
    self.queue = Default::default();
    self.swarm.send_to_all(&VhlQ::Init(self.state.clone(), self.queue.clone())); }

  pub fn tup(&self, n:NID)->(NID,NID) { self.state.tup(n) }

  pub fn len(&self)->usize { self.state.len() }
  #[must_use] pub fn is_empty(&self) -> bool { self.len() == 0 }

  pub fn run_swarm_job(&mut self, job:J)->NID {
    let mut result: Option<NID> = None;
    self.swarm.add_query(VhlQ::Job(job));
    // each response can lead to up to two new ITE queries, and we'll relay those to
    // other workers too, until we get back enough info to solve the original query.
    while result.is_none() {
      let RMsg{wid:_,qid:_,r} = self.swarm.recv().expect("failed to recieve rmsg");
      if let Some(rmsg) = r { match rmsg {
        R::Ret(n) => { result = Some(n) }
        R::CacheStats{ tests:_, hits:_ }
          => { panic!("got R::CacheStats before sending Q::Stats"); } }}}
    result.unwrap() }

  pub fn get_stats(&mut self) {
    self.swarm.send_to_all(&VhlQ::Stats);
    let (mut tests, mut hits, mut reports) = (0, 0, 0);
    while reports < self.swarm.num_workers() {
        let RMsg{wid:_, qid:_, r} = self.swarm.recv().expect("still expecting an Rmsg::CacheStats");
        if let Some(wip::RMsg::CacheStats{ tests:t, hits: h }) = r { reports += 1; tests+=t; hits += h }
        else { println!("extraneous rmsg from swarm after Q::Stats: {:?}", r) }}
    COUNT_CACHE_TESTS.with(|c| *c.borrow_mut() += tests);
    COUNT_CACHE_HITS.with(|c| *c.borrow_mut() += hits); }}
