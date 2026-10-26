//! Swarm worker for parallel BDD `ite` reduction. Plugs `BddJobHandler`
//! into the generic `VhlSwarm` framework defined in `vhl_swarm`.
use crate::{vhl::{VhlParts, VhlSlots}, wip::{Dep, ResStep}};
use crate::nid::NID;
use crate::bdd::{ITE, NormIteKey, Norm};
use crate::vhl_swarm::{JobKey, VhlJobHandler, VhlSwarm, VhlWorker};

impl JobKey for NormIteKey {}

#[derive(Debug, Default)]
pub struct BddJobHandler {}

impl VhlJobHandler<NormIteKey> for BddJobHandler {
  type W = VhlWorker<NormIteKey, Self>;

  fn work_job(&mut self, w: &mut Self::W, q:NormIteKey) {
    // Causal-profiler progress point: one unit of ITE work dispatched
    // to a BDD swarm worker. See `src/coz_profile.rs`.
    crate::coz_progress!("bdd-ite");
    let res = match self.ite_norm(w, q) {
      ResStep::Nid(n) => w.resolve_job(&q, n),
      ResStep::Wip { v, hi, lo, invert } => {
        let mut res = w.add_wip(&q, VhlParts { v, hi:None, lo:None, invert });
        if res.answer.is_none() {
          for &(xx, slot) in &[(hi,VhlSlots::Hi), (lo,VhlSlots::Lo)] {
            if res.answer.is_some() { break }
            match xx {
            Norm::Nid(nid) => { res.merge(w.resolve_part(&q, slot, nid, false)) },
            Norm::Ite(ite) |
            Norm::Not(ite) => {
              let (was_new, answer) = w.add_dep(&ite, Dep::new(q, slot, xx.is_inv()));
              if was_new { w.delegate(ite) }
              res.merge(answer) }}}}
        res }};
    if let Some(nid) = w.handle_result(res) {
      w.send_answer(&q, nid) }}}


type BddWorker = VhlWorker<NormIteKey, BddJobHandler>;

impl BddJobHandler {

  fn vhl_norm(&self, w:&BddWorker, ite:NormIteKey)->ResStep {
    let ITE{i:vv,t:hi,e:lo} = ite.0; let v = vv.vid();
    ResStep::Nid(w.vhl_to_nid(v, hi, lo)) }

  fn ite_norm(&self, w: &BddWorker, ite:NormIteKey)->ResStep {
    let ITE { i, t, e } = ite.0;
    let (vi, vt, ve) = (i.vid(), t.vid(), e.vid());
    let v = ite.0.top_vid();
    match w.get_done(&ite) {
      Some(n) => ResStep::Nid(n),
      None => {
        let (hi_i, lo_i) = if v == vi {w.tup(i)} else {(i,i)};
        let (hi_t, lo_t) = if v == vt {w.tup(t)} else {(t,t)};
        let (hi_e, lo_e) = if v == ve {w.tup(e)} else {(e,e)};
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
            Norm::Ite(ite) => self.vhl_norm(w, ite),
            Norm::Not(ite) => !self.vhl_norm(w, ite)}}
        // otherwise at least one side is not a simple nid yet, and we have to defer
        else { ResStep::Wip{ v, hi, lo, invert:false } }}}} }


// ----------------------------------------------------------------
/// BddSwarm: a multi-threaded swarm implementation
// ----------------------------------------------------------------
pub type BddSwarm = VhlSwarm<NormIteKey, BddJobHandler>;

impl BddSwarm {
  /// all-purpose if-then-else node constructor. For the swarm implementation,
  /// we push all the normalization and tree traversal work into the threads,
  /// while this function puts all the parts together.
  pub fn ite(&mut self, i:NID, t:NID, e:NID)->NID {
    match ITE::norm(i,t,e) {
      Norm::Nid(n) => n,
      Norm::Ite(ite) => { self.run_swarm_job(ite) }
      Norm::Not(ite) => { !self.run_swarm_job(ite) }}}}


#[test] fn test_swarm_cache() {
  // run a query for ite(x1,x2,x3) twice and make sure it retrieves the cached value without crashing
  let mut swarm = BddSwarm::new_with_threads(2);
  let ite = NormIteKey(ITE{i:NID::var(1), t:NID::var(2), e:NID::var(3)});
  let n1 = swarm.ite(ite.0.i, ite.0.t, ite.0.e);
  let n2 = swarm.ite(ite.0.i, ite.0.t, ite.0.e);
  assert_eq!(n1, n2); }

#[test] fn test_swarm_save_load_json() {
  // populate the cache with a couple of jobs, save to json, and load it
  // into a fresh swarm; the fresh swarm should agree on already-computed
  // answers, and reproduce the same answers if asked to redo the work.
  let mut swarm = BddSwarm::new_with_threads(2);
  let ite1 = NormIteKey(ITE{i:NID::var(1), t:NID::var(2), e:NID::var(3)});
  let ite2 = NormIteKey(ITE{i:NID::var(2), t:NID::var(3), e:NID::var(1)});
  let n1 = swarm.run_swarm_job(ite1);
  let n2 = swarm.run_swarm_job(ite2);

  let json = swarm.save_json().expect("save_json failed");
  let mut loaded = BddSwarm::load_json(&json).expect("load_json failed");

  assert_eq!(loaded.get_done(&ite1), Some(n1));
  assert_eq!(loaded.get_done(&ite2), Some(n2));

  // re-running the same jobs on the loaded swarm should hit the restored
  // cache and reproduce the same nids.
  assert_eq!(loaded.run_swarm_job(ite1), n1);
  assert_eq!(loaded.run_swarm_job(ite2), n2); }


#[test] fn test_swarm_load_clears_qid() {
  // Claim 2: a checkpoint taken mid-job serializes qid=Some, but load must
  // reset it so a fresh top-level Job does not assert "already working".
  use crate::swarm::QID;
  use crate::wip::WorkState;
  use crate::vhl::{VhlBase, VhlParts};
  let ws = WorkState::<NormIteKey, VhlParts, VhlBase>::default();
  *ws.qid.lock().unwrap() = Some(QID::STEP(99));
  let n = ws.vhl_to_nid(crate::vid::VID::var(2), NID::var(1), NID::var(0));
  let ite = NormIteKey(ITE{i:NID::var(2), t:NID::var(1), e:NID::var(0)});
  ws.put_done(ite, n);
  let json = serde_json::to_string(&ws).expect("serialize");
  let mut loaded = BddSwarm::load_json(&json).expect("load_json");
  // Must be able to run a new top-level job without qid assert.
  let got = loaded.run_swarm_job(ite);
  assert_eq!(got, n); }

#[test] fn test_swarm_load_reseeds_unfinished_todos() {
  // Claim 2 (stall half): unfinished Todos survive load, but the job queue
  // does not. Re-submitting the root must re-seed Todos so work progresses.
  use crate::wip::{Work, WorkState, Wip};
  use crate::vhl::{VhlBase, VhlParts};
  let ws = WorkState::<NormIteKey, VhlParts, VhlBase>::default();
  *ws.qid.lock().unwrap() = Some(crate::swarm::QID::STEP(7));
  let ite = NormIteKey(ITE{i:NID::var(3), t:NID::var(2), e:NID::var(1)});
  // Pretend a mid-flight checkpoint left the root as Todo with empty deps.
  ws.cache.insert(ite, Work::Todo(Wip::default()));
  let json = serde_json::to_string(&ws).expect("serialize");
  let mut loaded = BddSwarm::load_json(&json).expect("load_json");
  // Should complete (recompute) rather than hang or assert on stale qid.
  let n = loaded.run_swarm_job(ite);
  assert_eq!(loaded.get_done(&ite), Some(n)); }

#[test] fn test_checkpoint_no_dangling_done_nids() {
  // Claim 1: concurrent base inserts + Done publishes must not produce a
  // checkpoint where a Done(nid) indexes past the restored HiLo base.
  use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
  use std::thread;
  use crate::wip::WorkState;
  use crate::vhl::{VhlBase, VhlParts};
  use crate::vid::VID;
  use crate::wip::Work;
  let ws = Arc::new(WorkState::<NormIteKey, VhlParts, VhlBase>::default());
  let stop = Arc::new(AtomicBool::new(false));
  let ws2 = ws.clone();
  let stop2 = stop.clone();
  let h = thread::spawn(move || {
    let mut i = 0u32;
    while !stop2.load(Ordering::Relaxed) {
      // hi/lo vids must be strictly below v for a well-formed VHL.
      let v = VID::var((i % 8) + 2);
      let hi = NID::var(i % 2);
      let lo = NID::var((i % 2) ^ 1);
      let nid = ws2.vhl_to_nid(v, hi, lo);
      let ite = NormIteKey(ITE{i:NID::from_vid(v), t:hi, e:lo});
      ws2.put_done(ite, nid);
      i = i.wrapping_add(1); }});
  for _ in 0..200 {
    let json = serde_json::to_string(ws.as_ref()).expect("serialize");
    let loaded: WorkState<NormIteKey, VhlParts, VhlBase> =
      serde_json::from_str(&json).expect("deserialize");
    let base_len = loaded.len();
    for r in loaded.cache.iter() {
      if let Work::Done(n) = r.value() {
        if !n.is_lit() {
          assert!(n.idx() < base_len,
            "torn checkpoint: Done({:?}) idx={} >= base_len={}", n, n.idx(), base_len); }}}}
  stop.store(true, Ordering::Relaxed);
  h.join().unwrap(); }
