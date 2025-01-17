use crate::{vhl::HiLoPart, wip::{Answer, Dep, ResStep}};
use crate::nid::NID;
use crate::bdd::{ITE, NormIteKey, Norm};
use crate::vhl_swarm::{JobKey, VhlJobHandler, VhlSwarm, VhlWorker};

impl JobKey for NormIteKey {}

#[derive(Debug, Default)]
pub struct BddJobHandler {}

impl VhlJobHandler<NormIteKey> for BddJobHandler {
  type W = VhlWorker<NormIteKey, Self>;

  fn work_job(&mut self, w: &mut Self::W, q:NormIteKey) {
    let res = match self.ite_norm(w, q) {
      ResStep::Nid(n) => w.resolve_nid(&q, n),
      ResStep::Wip { v, hi, lo, invert } => {
        let mut res = w.add_wip(&q, v, invert);
        if res.is_none() {
          for &(xx, part) in &[(hi,HiLoPart::HiPart), (lo,HiLoPart::LoPart)] {
            match xx {
            Norm::Nid(nid) => { res = w.resolve_part(&q, part, nid, false) },
            Norm::Ite(ite) |
            Norm::Not(ite) => {
              let (was_new, answer) = w.add_dep(&ite, Dep::new(q, part, xx.is_inv()));
              if was_new { w.delegate(ite) }
              res = answer }}}}
        res }};
    if let Some(Answer(nid)) = res {
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
