use std::fmt;
use std::sync::Arc;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use {wip, wip::{Dep,WIP,WorkState}};
use vhl::{HiLoPart, VHLParts};
use {vid::VID, nid::{NID}, vhl::{HiLo}};
use bdd::{ITE, Norm, BddState, COUNT_XMEMO_TEST, COUNT_XMEMO_FAIL};
use {swarm, swarm::{WID, QID, Swarm, RMsg}};

// ----------------------------------------------------------------
// BddSwarm Protocol
// ----------------------------------------------------------------

/// Query message for BddSwarm.
#[derive(Clone)]
pub enum Q {
  /// The main recursive operation: convert ITE triple to a BDD.
  Ite(ITE),
  /// Give the worker a new reference to the central cache.
  Cache(Arc<BddState>),
  /// ask for stats about cache
  Stats }

type R = wip::RMsg<Norm>;

// Q::Cache() message could potentially be huge to print, so don't.
impl std::fmt::Debug for Q {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Q::Ite(ite) => { write!(f, "Q::Ite({:?})", ite) }
      Q::Cache(_) => { write!(f, "Q::Cache(...)") }
      Q::Stats => { write!(f, "Q::Stats")} } }}

// ----------------------------------------------------------------

#[derive(Debug, Default)]
struct BddWorker { wid:WID, state:Option<Arc<BddState>> }
impl swarm::Worker<Q,R> for BddWorker {
  fn new(wid:WID)->Self { BddWorker{ wid, ..Default::default() }}
  fn get_wid(&self)->WID { self.wid }
  fn work_step(&mut self, _qid:&QID, q:Q)->Option<R> {
    //println!("Q--> {:?}, {:?}", _qid, q);
    match q {
      Q::Cache(s) => { self.state = Some(s); None }
      Q::Ite(ite) => { Some(swarm_ite(self.state.as_ref().unwrap(), ite)) }
      Q::Stats => {
        let tests = COUNT_XMEMO_TEST.with(|c| c.replace(0));
        let fails = COUNT_XMEMO_FAIL.with(|c| c.replace(0));
        Some(R::MemoStats{ tests, fails }) } }}}


// ----------------------------------------------------------------
/// BddSwarm: a multi-threaded swarm implementation
// ----------------------------------------------------------------
#[derive(Debug, Default)]
pub struct BddSwarm {
  swarm: Swarm<Q,R,BddWorker>,
  /// read-only version of the state shared by all threads.
  stable: Arc<BddState>,
  /// mutable version of the state kept by the main thread.
  recent: BddState,
  // work in progress
  work: WorkState<ITE>}

impl Serialize for BddSwarm {
  fn serialize<S:Serializer>(&self, ser: S)->Result<S::Ok, S::Error> {
    // all we really care about is the state:
    self.stable.serialize::<S>(ser) } }

impl<'de> Deserialize<'de> for BddSwarm {
  fn deserialize<D:Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
    let mut res = Self::new();
    res.stable = Arc::new(BddState::deserialize(d)?);
    Ok(res) }}


impl BddSwarm {

  pub fn new()->Self { Self::default() }

  pub fn tup(&self, n:NID)->(NID,NID) { self.recent.tup(n) }

  /// all-purpose if-then-else node constructor. For the swarm implementation,
  /// we push all the normalization and tree traversal work into the threads,
  /// while this function puts all the parts together.
  pub fn ite(&mut self, i:NID, t:NID, e:NID)->NID { self.run_swarm(i,t,e) } }


impl BddSwarm {

  fn add_query(&mut self, ite:ITE)->QID {
    let qid = self.swarm.add_query(Q::Ite(ite));
    self.work.qid.insert(ite, qid);
    self.work.qs.insert(qid, ite);
    self.work.wip.insert(qid, WIP::Fresh);
    qid }

  fn add_sub_task(&mut self, dep:Dep, ite:ITE) {
    // if this ite already has a worker assigned...
    // !! TODO: really this should be combined with the bdd cache check
    if let Some(&qid) = self.work.qid.get(&ite) {
      trace!("*** task {:?} is dup of q{:?} invert: {}", ite, qid, dep.invert);
      if let Some(&WIP::Done(nid)) = self.work.wip.get(&qid) {
        self.resolve_part(&dep.qid, dep.part, nid, dep.invert); }
      else { self.work.deps.get_mut(&qid).unwrap().push(dep) }}
    else {
      let qid = self.add_query(ite);
      trace!("*** added task #{:?}: {:?} with dep: {:?}", qid, ite, dep);
      self.work.deps.insert(qid, vec![dep]);}}


  /// called whenever the wip resolves to a single nid
  fn resolve_nid(&mut self, qid:&QID, nid:NID) {
    if let Some(&WIP::Done(old)) = self.work.wip.get(qid) {
      warn!("resolving already resolved nid for q{:?}", qid);
      assert_eq!(old, nid, "old and new resolutions didn't match!") }
    else {
      trace!("resolved_nid: {:?}=>{}. deps: {:?}", qid, nid, self.work.deps.get(qid));
      self.work.wip.insert(*qid,WIP::Done(nid));
      let &ite = self.work.qs.get(qid).unwrap();
      self.recent.xmemo.insert(ite, nid);
      let deps = self.work.deps.get(qid); // !! can i avoid clone here?
      if deps.is_none() { self.swarm.send_to_self(R::Ret(nid)); }
      else { for dep in deps.cloned().unwrap() {
        self.resolve_part(&dep.qid, dep.part, nid, dep.invert) }}}}

  /// called whenever the wip resolves to a new simple (v/hi/lo) node.
  fn resolve_vhl(&mut self, qid:&QID, v:VID, hilo:HiLo, invert:bool) {
    trace!("resolve_vhl({:?}, {:?}, {:?}, invert:{}", qid, v, hilo, invert);
    let HiLo{hi:h0,lo:l0} = hilo;
    // we apply invert first so it normalizes correctly.
    let (h1,l1) = if invert { (!h0, !l0) } else { (h0, l0) };
    let nid = match ITE::norm(NID::from_vid(v), h1, l1) {
      Norm::Nid(n) => n,
      Norm::Ite(ITE{i:vv,t:hi,e:lo}) =>  self.recent.simple_node(vv.vid(), HiLo{hi,lo}),
      Norm::Not(ITE{i:vv,t:hi,e:lo}) => !self.recent.simple_node(vv.vid(), HiLo{hi,lo})};
    trace!("resolved vhl: {:?}=>{}. #deps: {}", qid, nid, self.work.deps[qid].len());
    self.resolve_nid(qid, nid) }

  fn resolve_part(&mut self, qid:&QID, part:HiLoPart, nid:NID, invert:bool) {
    self.work.resolve_part(qid, part, nid, invert);
    if let WIP::Parts(wip) = self.work.wip[qid] {
      if let Some(hilo) = wip.hilo() { self.resolve_vhl(qid, wip.v, hilo, wip.invert); }}}

  /// initialization logic for running the swarm. spawns threads and copies latest cache.
  fn init_swarm(&mut self) {
    // self.swarm = Swarm::new();
    //self.swarm.start(0);
    self.stable = Arc::new(self.recent.clone());
    self.swarm.send_to_all(&Q::Cache(self.stable.clone())); }


  /// distrubutes the standard ite() operatation across a swarm of threads
  fn run_swarm(&mut self, i:NID, t:NID, e:NID)->NID {
    match ITE::norm(i,t,e) {
      Norm::Nid(n) => n,
      Norm::Ite(ite) => { self.run_swarm_ite(ite) }
      Norm::Not(ite) => { !self.run_swarm_ite(ite) }}}

  fn run_swarm_ite(&mut self, ite:ITE)->NID {
    self.init_swarm();
    self.add_query(ite);
    let mut result: Option<NID> = None;
    // each response can lead to up to two new ITE queries, and we'll relay those to
    // other workers too, until we get back enough info to solve the original query.
    while result.is_none() {
      let RMsg{wid:_,qid,r} = self.swarm.recv().expect("failed to recieve rmsg");
      //println!("{:?} -> {:?}", qid, r);
      if let Some(rmsg) = r { match rmsg {
        R::Nid(nid) =>  { self.resolve_nid(&qid, nid); }
        R::Vhl{v,hi,lo,invert} => { self.resolve_vhl(&qid, v, HiLo{hi, lo}, invert); }
        R::Wip{v,hi,lo,invert} => {
          // by the time we get here, the task for this node was already created.
          // (add_task already filled in the v for us, so we don't need it.)
          assert_eq!(self.work.wip[&qid], WIP::Fresh);
          self.work.wip.insert(qid, WIP::Parts(VHLParts{ v, hi:None, lo:None, invert }));
          for &(xx, part) in &[(hi,HiLoPart::HiPart), (lo,HiLoPart::LoPart)] {
            match xx {
              Norm::Nid(nid) => self.resolve_part(&qid, part, nid, false),
              Norm::Ite(ite) => { self.add_sub_task(Dep::new(qid, part, false), ite);},
              Norm::Not(ite) => { self.add_sub_task(Dep::new(qid, part, true), ite);}}}}
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

/// Code run by each thread in the swarm. Isolated as a function without channels for testing.
fn swarm_ite(state: &Arc<BddState>, ite0:ITE)->R {
  let ITE { i, t, e } = ite0;
  match ITE::norm(i,t,e) {
      Norm::Nid(n) => R::Nid(n),
      Norm::Ite(ite) => swarm_ite_norm(state, ite),
      Norm::Not(ite) => !swarm_ite_norm(state, ite) }}

fn swarm_vhl_norm(state: &Arc<BddState>, ite:ITE)->R {
  let ITE{i:vv,t:hi,e:lo} = ite; let v = vv.vid();
  if let Some(n) = state.get_simple_node(v, HiLo{hi,lo}) { R::Nid(n) }
  else { R::Vhl{ v, hi, lo, invert:false } }}

fn swarm_ite_norm(state: &Arc<BddState>, ite:ITE)->R {
  let ITE { i, t, e } = ite;
  let (vi, vt, ve) = (i.vid(), t.vid(), e.vid());
  let v = ite.top_vid();
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
          Norm::Ite(ite) => swarm_vhl_norm(state, ite),
          Norm::Not(ite) => !swarm_vhl_norm(state, ite)}}
      // otherwise at least one side is not a simple nid yet, and we have to defer
      else { R::Wip{ v, hi, lo, invert:false } }}}}
