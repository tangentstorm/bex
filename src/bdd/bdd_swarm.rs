use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use {wip, wip::{QID,Dep,WIP,WorkState}};
use vhl::{HiLoPart, VHLParts};
use {vid::VID, nid::{NID}, vhl::{HiLo}};
use bdd::{ITE, Norm, BddState, BDDHashMap, COUNT_XMEMO_TEST, COUNT_XMEMO_FAIL};

// ----------------------------------------------------------------
// Helper types for BddSwarm
// ----------------------------------------------------------------

/// Query message for BddSwarm.
pub enum QMsg { Ite(QID, ITE), Cache(Arc<BddState>), Halt }
impl std::fmt::Debug for QMsg {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      QMsg::Ite(qid, ite) => { write!(f, "QMsg::Ite(q{}, {:?})", qid, ite) }
      QMsg::Cache(_) => { write!(f, "QMsg::Cache(...)") }
      QMsg::Halt => { write!(f, "QMsg::Halt")} } }}

type RMsg = wip::RMsg<Norm>;

/// Sender for QMsg
pub type QTx = Sender<QMsg>;
/// Receiver for QMsg
pub type QRx = Receiver<QMsg>;
/// Sender for RMsg
pub type RTx = Sender<(QID, RMsg)>;
/// Receiver for RMsg
pub type RRx = Receiver<(QID, RMsg)>;


// ----------------------------------------------------------------
/// BddSwarm: a multi-threaded swarm implementation
// ----------------------------------------------------------------
#[derive(Debug)]
pub struct BddSwarm {
  /// receives messages from the threads
  rx: RRx,
  /// send messages to myself (so we can put them back in the queue.
  me: RTx,
  /// QMsg senders for each thread, so we can send queries to work on.
  swarm: Vec<QTx>,
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

  pub fn new()->Self {
    let (me, rx) = channel::<(QID, RMsg)>();
    let swarm = vec![];
    let stable = Arc::new(BddState::new());
    let recent = BddState::new();
    Self{ me, rx, swarm, stable, recent, work:WorkState::new() }}

  pub fn get_state(&self)->&BddState { &self.recent }

  pub fn tup(&self, n:NID)->(NID,NID) { self.recent.tup(n) }

  /// all-purpose if-then-else node constructor. For the swarm implementation,
  /// we push all the normalization and tree traversal work into the threads,
  /// while this function puts all the parts together.
  pub fn ite(&mut self, i:NID, t:NID, e:NID)->NID { self.run_swarm(i,t,e) } }


impl BddSwarm {

  /// add a new task for the swarm to work on. (if it's a duplicate, we just
  /// add the dependencies to the original task (unless it's already finished,
  /// in which case we resolve immediately))
  fn add_task(&mut self, opt_dep:Option<Dep>, ite:ITE) {
    trace!("add_task({:?}, {:?})", opt_dep, ite);
    let (qid, is_dup) = {
      if let Some(&dup) = self.work.qid.get(&ite) { (dup, true) }
      else { (self.work.wip.len(), false) }};
    if is_dup {
      if let Some(dep) = opt_dep {
        trace!("*** task {:?} is dup of q{} invert: {}", ite, qid, dep.invert);
        if let WIP::Done(nid) = self.work.wip[qid] {
          self.resolve_part(dep.qid, dep.part, nid, dep.invert); }
        else { self.work.deps[qid].push(dep) }}
      else { panic!("Got duplicate request, but no dep. This should never happen!") }}
    else {
      self.work.qid.insert(ite, qid); self.work.qs.push(ite);
      let w:usize = qid % self.swarm.len();
      self.swarm[w].send(QMsg::Ite(qid, ite)).expect("send to swarm failed");
      self.work.wip.push(WIP::Fresh);
      if let Some(dep) = opt_dep {
        trace!("*** added task #{}: {:?} invert:{}", qid, ite, dep.invert);
        self.work.deps.push(vec![dep]) }
      else if qid == 0 {
        trace!("*** added task #{}: {:?} (no deps!)", qid, ite);
        self.work.deps.push(vec![]) }
      else { panic!("non 0 qid with no deps!?") }}}

  /// called whenever the wip resolves to a single nid
  fn resolve_nid(&mut self, qid:QID, nid:NID) {
    trace!("resolve_nid(q{}, {})", qid, nid);
    if let WIP::Done(old) = self.work.wip[qid] {
      warn!("resolving already resolved nid for q{}", qid);
      assert_eq!(old, nid, "old and new resolutions didn't match!") }
    else {
      trace!("resolved_nid: q{}=>{}. deps: {:?}", qid, nid, self.work.deps[qid].clone());
      self.work.wip[qid] = WIP::Done(nid);
      let ite = self.work.qs[qid];
      self.recent.put_xmemo(ite, nid);
      for &dep in self.work.deps[qid].clone().iter() {
        self.resolve_part(dep.qid, dep.part, nid, dep.invert) }
      if qid == 0 { self.me.send((0, RMsg::Ret(nid))).expect("failed to send Ret"); }}}

  /// called whenever the wip resolves to a new simple (v/hi/lo) node.
  fn resolve_vhl(&mut self, qid:QID, v:VID, hilo:HiLo, invert:bool) {
    trace!("resolve_vhl(q{}, {:?}, {:?}, invert:{}", qid, v, hilo, invert);
    let HiLo{hi:h0,lo:l0} = hilo;
    // we apply invert first so it normalizes correctly.
    let (h1,l1) = if invert { (!h0, !l0) } else { (h0, l0) };
    let nid = match ITE::norm(NID::from_vid(v), h1, l1) {
      Norm::Nid(n) => n,
      Norm::Ite(ITE{i:vv,t:hi,e:lo}) =>  self.recent.simple_node(vv.vid(), HiLo{hi,lo}),
      Norm::Not(ITE{i:vv,t:hi,e:lo}) => !self.recent.simple_node(vv.vid(), HiLo{hi,lo})};
    trace!("resolved vhl: q{}=>{}. #deps: {}", qid, nid, self.work.deps[qid].len());
    self.resolve_nid(qid, nid); }

  fn resolve_part(&mut self, qid:QID, part:HiLoPart, nid:NID, invert:bool) {
    self.work.resolve_part(qid, part, nid, invert);
    if let WIP::Parts(wip) = self.work.wip[qid] {
      if let Some(hilo) = wip.hilo() { self.resolve_vhl(qid, wip.v, hilo, wip.invert) }}}

  /// initialization logic for running the swarm. spawns threads and copies latest cache.
  fn init_swarm(&mut self) {
    self.work.wip = vec![]; self.work.qs = vec![]; self.work.deps = vec![]; self.work.qid = BDDHashMap::new();
    // wipe out and replace the channels so un-necessary work from last iteration
    // (that was still going on when we returned a value) gets ignored..
    let (me, rx) = channel::<(QID, RMsg)>(); self.me = me; self.rx = rx;
    self.swarm = vec![];
    while self.swarm.len() < num_cpus::get() {
      let (tx, rx) = channel::<QMsg>();
      let me_clone = self.me.clone();
      let state = self.stable.clone();
      thread::spawn(move || swarm_loop(me_clone, rx, state));
      self.swarm.push(tx); }
    self.stable = Arc::new(self.recent.clone());
    for tx in self.swarm.iter() {
      tx.send(QMsg::Cache(self.stable.clone())).expect("failed to send QMsg::Cache"); }}

  /// distrubutes the standard ite() operatation across a swarm of threads
  fn run_swarm(&mut self, i:NID, t:NID, e:NID)->NID {
    macro_rules! run_swarm_ite { ($n:expr, $ite:expr) => {{
      self.init_swarm(); self.add_task(None, $ite);
      let mut result:Option<NID> = None;
      // each response can lead to up to two new ITE queries, and we'll relay those to
      // other workers too, until we get back enough info to solve the original query.
      while result.is_none() {
        let (qid, rmsg) = self.rx.recv().expect("failed to read RMsg from queue!");
        trace!("===> run_swarm got RMsg {}: {:?}", qid, rmsg);
        match rmsg {
          RMsg::MemoStats{ tests:_, fails:_ } => { panic!("got RMsg::MemoStats before sending QMsg::Halt"); }
          RMsg::Nid(nid) =>  { self.resolve_nid(qid, nid); }
          RMsg::Vhl{v,hi,lo,invert} => { self.resolve_vhl(qid, v, HiLo{hi, lo}, invert); }
          RMsg::Wip{v,hi,lo,invert} => {
            // by the time we get here, the task for this node was already created.
            // (add_task already filled in the v for us, so we don't need it.)
            assert_eq!(self.work.wip[qid], WIP::Fresh);
            self.work.wip[qid] = WIP::Parts(VHLParts{ v, hi:None, lo:None, invert });
            macro_rules! handle_part { ($xx:ident, $part:expr) => {
              match $xx {
                Norm::Nid(nid) => self.resolve_part(qid, $part, nid, false),
                Norm::Ite(ite) => self.add_task(Some(Dep::new(qid, $part, false)), ite),
                Norm::Not(ite) => self.add_task(Some(Dep::new(qid, $part, true)), ite)}}}
            handle_part!(hi, HiLoPart::HiPart); handle_part!(lo, HiLoPart::LoPart); }
          RMsg::Ret(n) => {
            result = Some(n);
            for tx in self.swarm.iter() {  tx.send(QMsg::Halt).expect("failed to send QMsg::Halt") }}}}
      let (mut tests, mut fails, mut reports, mut shorts) = (0, 0, 0, 0);
      // println!("waiting for MemoStats");
      while reports < self.swarm.len() {
        let (_qid, rmsg) = self.rx.recv().expect("still expecting an Rmsg::MemoCount");
        if let wip::RMsg::MemoStats{ tests:t, fails: f } = rmsg { reports += 1; tests+=t; fails += f }
        else { shorts += 1; println!("extraneous rmsg from swarm: {:?}", rmsg) }}
      // if tests > 0 { println!("{:?} result: {:?}  tests: {}  fails: {}  hits: {}", $ite, result, tests, fails, tests-fails); }
      if shorts > 0 { println!("----------- shorts: {}", shorts)} // i don't think this actually happens.
      COUNT_XMEMO_TEST.with(|c| *c.borrow_mut() += tests );
      COUNT_XMEMO_FAIL.with(|c| *c.borrow_mut() += fails );
      result.unwrap() }}}
    match ITE::norm(i,t,e) {
      Norm::Nid(n) => n,
      Norm::Ite(ite) => { run_swarm_ite!(0,ite) }
      Norm::Not(ite) => { !run_swarm_ite!(0,ite) }}}
} // end bddswarm

/// Code run by each thread in the swarm. Isolated as a function without channels for testing.
fn swarm_ite(state: &Arc<BddState>, ite0:ITE)->RMsg {
  let ITE { i, t, e } = ite0;
  match ITE::norm(i,t,e) {
      Norm::Nid(n) => RMsg::Nid(n),
      Norm::Ite(ite) => swarm_ite_norm(state, ite),
      Norm::Not(ite) => !swarm_ite_norm(state, ite) }}

fn swarm_vhl_norm(state: &Arc<BddState>, ite:ITE)->RMsg {
  let ITE{i:vv,t:hi,e:lo} = ite; let v = vv.vid();
  if let Some(n) = state.get_simple_node(v, HiLo{hi,lo}) { RMsg::Nid(n) }
  else { RMsg::Vhl{ v, hi, lo, invert:false } }}

fn swarm_ite_norm(state: &Arc<BddState>, ite:ITE)->RMsg {
  let ITE { i, t, e } = ite;
  let (vi, vt, ve) = (i.vid(), t.vid(), e.vid());
  let v = ite.top_vid();
  match state.get_memo(&ite) {
    Some(n) => RMsg::Nid(n),
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
          Norm::Nid(n) => { RMsg::Nid(n) }
          // otherwise, the normalized triple might already be in cache:
          Norm::Ite(ite) => swarm_vhl_norm(state, ite),
          Norm::Not(ite) => !swarm_vhl_norm(state, ite)}}
      // otherwise at least one side is not a simple nid yet, and we have to defer
      else { RMsg::Wip{ v, hi, lo, invert:false } }}}}


/// This is the loop run by each thread in the swarm.
fn swarm_loop(tx:RTx, rx:QRx, mut state:Arc<BddState>) {
  for qmsg in rx.iter() {
    match qmsg {
      QMsg::Cache(s) => { state = s }
      QMsg::Ite(qid, ite) => {
        trace!("--->   thread worker got qmsg {}: {:?}", qid, qmsg);
        let rmsg = swarm_ite(&state, ite);
        if tx.send((qid, rmsg)).is_err() { break } }
      QMsg::Halt => {
        let tests = COUNT_XMEMO_TEST.with(|c| c.replace(0));
        let fails = COUNT_XMEMO_FAIL.with(|c| c.replace(0));
        if tx.send((QID::MAX, RMsg::MemoStats{ tests, fails })).is_err() {
          println!("failed to send memostats (tests:{} fails: {})", tests, fails)}
        break; } }}}
