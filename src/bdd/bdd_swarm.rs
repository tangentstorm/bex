use std::{fmt, sync::mpsc::Sender};
use std::sync::Arc;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use {wip, wip::{Dep, WorkCache, WIP, WorkState}};
use vhl::{HiLoPart, VhlParts};
use {vid::VID, nid::{NID}, vhl::{HiLo}};
use bdd::{ITE, Norm, BddState, COUNT_XMEMO_TEST, COUNT_XMEMO_FAIL};
use {swarm, swarm::{WID, QID, Swarm, RMsg}};
use concurrent_queue::{ConcurrentQueue,PopError};

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
  Ite(ITE),
  /// Initialize worker with its "hive mind".
  Init(Arc<BddState>, Arc<IteQueue>),
  /// ask for stats about cache
  Stats }

type R = wip::RMsg<Norm>;

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
      Q::Ite(ite) => { Some(self.ite(ite)) }
      Q::Stats => {
        let tests = COUNT_XMEMO_TEST.with(|c| c.replace(0));
        let fails = COUNT_XMEMO_FAIL.with(|c| c.replace(0));
        Some(R::MemoStats{ tests, fails }) } }}}

/// Code run by each thread in the swarm. Isolated as a function without channels for testing.
impl BddWorker {
  fn ite(&self, ite0:ITE)->R {
    let ITE { i, t, e } = ite0;
    match ITE::norm(i,t,e) {
        Norm::Nid(n) => R::Nid(n),
        Norm::Ite(ite) => self.ite_norm(ite),
        Norm::Not(ite) => !self.ite_norm(ite) }}

  fn vhl_norm(&self, ite:ITE)->R {
    let ITE{i:vv,t:hi,e:lo} = ite; let v = vv.vid();
    if let Some(n) = self.state.as_ref().unwrap().get_simple_node(v, HiLo{hi,lo}) {
      R::Nid(n) }
    else { R::Vhl{ v, hi, lo, invert:false } }}

  fn ite_norm(&self, ite:ITE)->R {
    let ITE { i, t, e } = ite;
    let (vi, vt, ve) = (i.vid(), t.vid(), e.vid());
    let v = ite.top_vid(); let state = self.state.as_ref().unwrap();
    match state.get_memo(&ite) {
      Some(n) => R::Nid(n),
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
            Norm::Nid(n) => { R::Nid(n) }
            // otherwise, the normalized triple might already be in cache:
            Norm::Ite(ite) => self.vhl_norm(ite),
            Norm::Not(ite) => !self.vhl_norm(ite)}}
        // otherwise at least one side is not a simple nid yet, and we have to defer
        else { R::Wip{ v, hi, lo, invert:false } }}}} }


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
  work_state: WorkState<ITE>,
  _work: Arc<WorkCache>}

// temp methods
impl BddSwarm {

  fn qid_to_ite(&self, qid:&QID)->ITE {
    if let Some(&ite) = self.work_state.qs.get(qid) { ite }
    else { panic!("no ite found for qid: {:?}", qid)}}

  fn ite_to_qid(&self, ite:&ITE)->QID {
    if let Some(&qid) = self.work_state.qid.get(ite) { qid }
    else { panic!("no qid found for ite: {:?}", ite)}}

  fn add_wip(&mut self, top:&ITE, p:VhlParts) {
    let qid = self.ite_to_qid(top);
    self.work_state.wip.insert(qid, WIP::Parts(p)); }}


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
  pub fn ite(&mut self, i:NID, t:NID, e:NID)->NID { self.run_swarm(i,t,e) } }


impl BddSwarm {

  fn add_query(&mut self, ite:ITE)->QID {
    let qid = self.swarm.add_query(Q::Ite(ite));
    self.work_state.qid.insert(ite, qid);
    self.work_state.qs.insert(qid, ite);
    self.work_state.wip.insert(qid, WIP::Fresh);
    qid }

  fn add_sub_task(&mut self, idep:Dep<ITE>, ite:ITE) {
    let dep_qid = self.ite_to_qid(&idep.dep);
    let qdep : Dep<QID> = Dep { dep: dep_qid, part:idep.part, invert:idep.invert };

    // if this ite already has a worker assigned...
    // !! TODO: really this should be combined with the bdd cache check
    if let Some(&qid) = self.work_state.qid.get(&ite) {
      trace!("*** task {:?} is dup of q{:?} invert: {}", ite, qid, idep.invert);
      if let Some(&WIP::Done(nid)) = self.work_state.wip.get(&qid) {
        self.resolve_part(&idep.dep, idep.part, nid, idep.invert); }
      else { self.work_state.deps.get_mut(&qid).unwrap().push(qdep) }}
    else {
      let qid = self.add_query(ite);
      trace!("*** added task #{:?}: {:?} with dep: {:?}", qid, ite, idep);
      self.work_state.deps.insert(qid, vec![qdep]);}}


  /// called whenever the wip resolves to a single nid
  fn resolve_nid(&mut self, ite:&ITE, nid:NID) {
    let qid = &self.ite_to_qid(ite);
    if let Some(&WIP::Done(old)) = self.work_state.wip.get(qid) {
      warn!("resolving already resolved nid for q{:?}", qid);
      assert_eq!(old, nid, "old and new resolutions didn't match!") }
    else {
      trace!("resolved_nid: {:?}=>{}. deps: {:?}", qid, nid, self.work_state.deps.get(qid));
      self.work_state.wip.insert(*qid,WIP::Done(nid));
      assert!(ite == self.work_state.qs.get(qid).unwrap());
      self.state.xmemo.insert(*ite, nid);
      let deps = self.work_state.deps.get(qid); // !! can i avoid clone here?
      if deps.is_none() { self.swarm.send_to_self(R::Ret(nid)); }
      else { for dep in deps.cloned().unwrap() {
        let dep_ite = self.qid_to_ite(&dep.dep);
        self.resolve_part(&dep_ite, dep.part, nid, dep.invert) }}}}

  /// called whenever the wip resolves to a new simple (v/hi/lo) node.
  fn resolve_vhl(&mut self, ite:&ITE, v:VID, hilo:HiLo, invert:bool) {
    let qid = &self.ite_to_qid(ite);
    trace!("resolve_vhl({:?}, {:?}, {:?}, invert:{}", qid, v, hilo, invert);
    let HiLo{hi:h0,lo:l0} = hilo;
    // we apply invert first so it normalizes correctly.
    let (h1,l1) = if invert { (!h0, !l0) } else { (h0, l0) };
    let nid = match ITE::norm(NID::from_vid(v), h1, l1) {
      Norm::Nid(n) => n,
      Norm::Ite(ITE{i:vv,t:hi,e:lo}) =>  self.state.simple_node(vv.vid(), HiLo{hi,lo}),
      Norm::Not(ITE{i:vv,t:hi,e:lo}) => !self.state.simple_node(vv.vid(), HiLo{hi,lo})};
    trace!("resolved vhl: {:?}=>{}. #deps: {}", qid, nid, self.work_state.deps[qid].len());
    self.resolve_nid(&ite, nid) }

  fn resolve_part(&mut self, ite:&ITE, part:HiLoPart, nid:NID, invert:bool) {
    let qid = &self.ite_to_qid(ite);
    self.work_state.resolve_part(qid, part, nid, invert);
    if let WIP::Parts(wip) = self.work_state.wip[qid] {
      if let Some(hilo) = wip.hilo() {
        self.resolve_vhl(ite, wip.v, hilo, wip.invert); }}}


  /// distrubutes the standard ite() operatation across a swarm of threads
  fn run_swarm(&mut self, i:NID, t:NID, e:NID)->NID {
    match ITE::norm(i,t,e) {
      Norm::Nid(n) => n,
      Norm::Ite(ite) => { self.run_swarm_ite(ite) }
      Norm::Not(ite) => { !self.run_swarm_ite(ite) }}}

  fn run_swarm_ite(&mut self, ite0:ITE)->NID {
    let mut result: Option<NID> = None;
    self.add_query(ite0);
    // each response can lead to up to two new ITE queries, and we'll relay those to
    // other workers too, until we get back enough info to solve the original query.
    while result.is_none() {
      let RMsg{wid:_,qid,r} = self.swarm.recv().expect("failed to recieve rmsg");
      //println!("{:?} -> {:?}", qid, r);
      if let Some(rmsg) = r { match rmsg {
        R::Nid(nid) =>  {
          let ite = self.qid_to_ite(&qid);
          self.resolve_nid(&ite, nid); }
        R::Vhl{v,hi,lo,invert} => {
          let ite = self.qid_to_ite(&qid);
          self.resolve_vhl(&ite, v, HiLo{hi, lo}, invert); }
        R::Wip{v,hi,lo,invert} => {
          // by the time we get here, the task for this node was already created.
          let q_ite = self.qid_to_ite(&qid);
          self.add_wip(&q_ite, VhlParts{ v, hi:None, lo:None, invert });
          for &(xx, part) in &[(hi,HiLoPart::HiPart), (lo,HiLoPart::LoPart)] {
            match xx {
              Norm::Nid(nid) => self.resolve_part(&q_ite, part, nid, false),
              Norm::Ite(ite) => { self.add_sub_task(Dep::new(q_ite, part, false), ite);},
              Norm::Not(ite) => { self.add_sub_task(Dep::new(q_ite, part, true), ite);}}}}
        R::Ret(n) => { result = Some(n) }
        R::MemoStats{ tests:_, fails:_ }
          => { panic!("got R::MemoStats before sending Q::Halt"); } }}}
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
