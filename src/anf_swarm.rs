//! Swarm-backed ANF construction.
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::Sender;

use concurrent_queue::{ConcurrentQueue, PopError};

use crate::base::Base;
use crate::bdd::BddBase;
use crate::cur::{Cursor, CursorPlan};
use crate::nid::{self, NID, I, O};
use crate::reg::Reg;
use crate::simp;
use crate::swarm::{QID, RMsg as SwarmRMsg, Swarm, Worker, WID};
use crate::vhl::{HiLo, HiLoBase, Vhl, Walkable};
use crate::vid::{VID, VidOrdering};
use crate::wip::{self, Answer, Dep, Parts, WorkState, WipBase, COUNT_CACHE_HITS, COUNT_CACHE_TESTS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnfJob {
  Xor(NID, NID),
  And(NID, NID),
}

impl Default for AnfJob {
  fn default() -> Self { AnfJob::Xor(O, O) }
}

impl AnfJob {
  pub fn xor(x:NID, y:NID)->Self {
    if x <= y { Self::Xor(x, y) } else { Self::Xor(y, x) }
  }

  pub fn and(x:NID, y:NID)->Self {
    if x <= y { Self::And(x, y) } else { Self::And(y, x) }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnfSlot { Hi, Lo, Bq, Br, Cq, Cr }

#[derive(Debug, Clone, Copy, Default)]
pub enum AnfParts {
  #[default]
  Empty,
  Xor { v:VID, hi:Option<NID>, lo:Option<NID> },
  AndAbove { v:VID, hi:Option<NID>, lo:Option<NID> },
  AndLevel { v:VID, bq:Option<NID>, br:Option<NID>, cq:Option<NID>, cr:Option<NID> },
}

impl Parts for AnfParts {
  type Slot = AnfSlot;

  fn set_slot(&mut self, slot: Self::Slot, nid: Option<NID>) {
    match self {
      AnfParts::Xor { hi, lo, .. } | AnfParts::AndAbove { hi, lo, .. } => {
        if slot == AnfSlot::Hi { *hi = nid } else if slot == AnfSlot::Lo { *lo = nid }
      }
      AnfParts::AndLevel { bq, br, cq, cr, .. } => match slot {
        AnfSlot::Bq => *bq = nid,
        AnfSlot::Br => *br = nid,
        AnfSlot::Cq => *cq = nid,
        AnfSlot::Cr => *cr = nid,
        _ => {}
      },
      AnfParts::Empty => {}
    }
  }

  fn is_ready(&self)->bool {
    match self {
      AnfParts::Empty => false,
      AnfParts::Xor { hi, lo, .. } | AnfParts::AndAbove { hi, lo, .. } => hi.is_some() && lo.is_some(),
      AnfParts::AndLevel { bq, br, cq, cr, .. } => bq.is_some() && br.is_some() && cq.is_some() && cr.is_some(),
    }
  }
}

#[derive(Debug, Default)]
struct AnfInner {
  nodes: Vec<Vhl>,
  cache: HashMap<Vhl,NID,fxhash::FxBuildHasher>,
}

#[derive(Debug, Default)]
pub struct AnfStore {
  inner: Mutex<AnfInner>,
}

impl AnfStore {
  pub fn fetch(&self, n:NID)->Vhl {
    if n.is_vid() {
      Vhl{ v:n.vid(), hi:I, lo: if n.is_inv() { I } else { O } }
    } else {
      let mut anf = self.inner.lock().unwrap().nodes[n.idx()];
      if n.is_inv() { anf.lo = !anf.lo }
      anf
    }
  }

  pub fn vhl(&self, v:VID, hi0:NID, lo0:NID)->NID {
    if hi0 == I && lo0 == O { return NID::from_vid(v) }
    let (hi, lo) = (hi0, lo0.raw());
    let mut inner = self.inner.lock().unwrap();
    let res =
      if let Some(&nid) = inner.cache.get(&Vhl{v, hi, lo}) { nid }
      else {
        let anf = Vhl{ v, hi, lo };
        let nid = NID::from_vid_idx(v, inner.nodes.len());
        inner.cache.insert(anf, nid);
        inner.nodes.push(anf);
        nid
      };
    if lo.is_inv() { !res } else { res }
  }

  fn xor_sync(&self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::xor(x,y) { nid }
    else {
      let (a, b) = (x.raw(), y.raw());
      let res = self.calc_xor_sync(a, b);
      if x.is_inv() == y.is_inv() { res } else { !res }
    }
  }

  fn calc_xor_sync(&self, x:NID, y:NID)->NID {
    let (xv, yv) = (x.vid(), y.vid());
    match xv.cmp_depth(&yv) {
      VidOrdering::Above => {
        let Vhl{v, hi, lo} = self.fetch(x);
        let lo = self.xor_sync(lo, y);
        self.vhl(v, hi, lo)
      }
      VidOrdering::Below => self.xor_sync(y, x),
      VidOrdering::Level => {
        let Vhl{v:a, hi:b, lo:c} = self.fetch(x);
        let Vhl{v:p, hi:q, lo:r} = self.fetch(y);
        assert_eq!(a,p);
        let hi = self.xor_sync(b, q);
        let lo = self.xor_sync(c, r);
        self.vhl(a, hi, lo)
      }
    }
  }

  pub fn when_lo(&self, v:VID, n:NID)->NID {
    let nv = n.vid();
    match v.cmp_depth(&nv) {
      VidOrdering::Above => n,
      VidOrdering::Level => self.fetch(n).lo,
      VidOrdering::Below => {
        let Vhl{ v:_, hi, lo } = self.fetch(n.raw());
        let hi1 = self.when_lo(v, hi);
        let lo1 = self.when_lo(v, lo);
        let res = self.vhl(nv, hi1, lo1);
        if n.is_inv() == res.is_inv() { res } else { !res }
      }
    }
  }

  pub fn when_hi(&self, v:VID, n:NID)->NID {
    let nv = n.vid();
    match v.cmp_depth(&nv) {
      VidOrdering::Above => n,
      VidOrdering::Level => self.fetch(n).hi,
      VidOrdering::Below => {
        let Vhl{ v:_, hi, lo } = self.fetch(n.raw());
        let hi1 = self.when_hi(v, hi);
        let lo1 = self.when_hi(v, lo);
        let res = self.vhl(nv, hi1, lo1);
        if n.is_inv() == res.is_inv() { res } else { !res }
      }
    }
  }

  pub fn sub(&self, v:VID, n:NID, ctx:NID)->NID {
    let cv = ctx.vid();
    if ctx.might_depend_on(v) {
      let x = self.fetch(ctx);
      let (hi, lo) = (x.hi, x.lo);
      if v == cv { self.xor_sync(self.and(n, hi), lo) }
      else {
        let rhi = self.sub(v,n,hi);
        let rlo = self.sub(v,n,lo);
        let top = NID::from_vid(cv);
        self.xor_sync(self.and(top, rhi), rlo)
      }
    } else { ctx }
  }

  pub fn and(&self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::and(x,y) { nid }
    else {
      let (a,b) = (x.raw(), y.raw());
      if x.is_inv() {
        if y.is_inv() { self.xor_sync(I, self.xor_sync(a, self.xor_sync(self.and(a,b), b))) }
        else { self.xor_sync(self.and(a,b), b) }
      } else if y.is_inv() { self.xor_sync(self.and(a,b), a) }
      else { self.calc_and_sync(x, y) }
    }
  }

  fn calc_and_sync(&self, x:NID, y:NID)->NID {
    let (xv, yv) = (x.vid(), y.vid());
    match xv.cmp_depth(&yv) {
      VidOrdering::Above =>
        if x.is_vid() { self.vhl(x.vid(), y, O) }
        else {
          let Vhl{v:a, hi:b, lo:c } = self.fetch(x);
          let hi = self.and(b, y);
          let lo = self.and(c, y);
          self.vhl(a, hi, lo)
        },
      VidOrdering::Below => self.and(y, x),
      VidOrdering::Level => {
        let Vhl{ v:a, hi:b, lo:c } = self.fetch(x);
        let Vhl{ v:p, hi:q, lo:r } = self.fetch(y);
        assert_eq!(a,p);
        let bq = self.and(b, q);
        let br = self.and(b, r);
        let cq = self.and(c, q);
        let cr = self.and(c, r);
        let hi = self.xor_sync(self.xor_sync(bq, br), cq);
        self.vhl(a, hi, cr)
      }
    }
  }
}

impl WipBase<AnfParts> for AnfStore {
  fn resolve_job(&self, parts:AnfParts)->NID {
    match parts {
      AnfParts::Empty => panic!("resolve_job on Empty"),
      AnfParts::Xor { v, hi:Some(hi), lo:Some(lo) } =>
        self.vhl(v, hi, lo),
      AnfParts::AndAbove { v, hi:Some(hi), lo:Some(lo) } =>
        self.vhl(v, hi, lo),
      AnfParts::AndLevel { v, bq:Some(bq), br:Some(br), cq:Some(cq), cr:Some(cr) } => {
        let hi = self.xor_sync(self.xor_sync(bq, br), cq);
        self.vhl(v, hi, cr)
      }
      _ => panic!("resolve_job called before AnfParts were complete"),
    }
  }
}

#[derive(Debug)]
pub struct JobQueue<J> { q: ConcurrentQueue<J> }
impl<J> Default for JobQueue<J> {
  fn default()->Self { JobQueue{ q: ConcurrentQueue::unbounded() }}
}
impl<J:fmt::Debug> JobQueue<J> {
  pub fn push(&self, job:J) { self.q.push(job).unwrap() }
  pub fn pop(&self)->Option<J> {
    match self.q.pop() {
      Ok(k) => Some(k),
      Err(PopError::Empty) => None,
      Err(PopError::Closed) => panic!("JobQueue was closed!"),
    }
  }
}

#[derive(Clone)]
pub enum AnfQ {
  Job(AnfJob),
  Init(Arc<WorkState<AnfJob, AnfParts, AnfStore>>, Arc<JobQueue<AnfJob>>),
  Stats,
}

impl fmt::Debug for AnfQ {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      AnfQ::Job(j) => write!(f, "Q::Job({:?})", j),
      AnfQ::Init(_, _) => write!(f, "Q::Init(...)"),
      AnfQ::Stats => write!(f, "Q::Stats"),
    }
  }
}

#[derive(Debug, Default)]
pub struct AnfWorker {
  wid: WID,
  tx:Option<Sender<SwarmRMsg<wip::RMsg>>>,
  next: Option<AnfJob>,
  state:Option<Arc<WorkState<AnfJob, AnfParts, AnfStore>>>,
  queue:Option<Arc<JobQueue<AnfJob>>>,
}

impl AnfWorker {
  fn send_answer(&self, nid:NID) {
    let qid = {
      let mut mx = self.state.as_ref().unwrap().qid.lock().unwrap();
      let q0 = (*mx).expect("no qid found in the mutex!");
      *mx = None;
      q0
    };
    self.send_msg(qid, Some(wip::RMsg::Ret(nid)))
  }

  fn send_msg(&self, qid:QID, r:Option<wip::RMsg>) {
    self.tx.as_ref().unwrap().send(SwarmRMsg{wid:self.wid, qid, r}).unwrap()
  }

  fn delegate(&mut self, job:AnfJob) { self.queue_push(job) }

  fn attach_child(&mut self, parent:&AnfJob, slot:AnfSlot, child:Core)->Option<Answer<NID>> {
    match child {
      Core::Nid(nid) => self.state.as_ref().unwrap().resolve_part(parent, slot, nid, false),
      Core::Job(job) => {
        let (was_new, answer) = self.state.as_ref().unwrap().add_dep(&job, Dep::new(*parent, slot, false));
        if was_new { self.delegate(job) }
        answer
      }
    }
  }

  fn work_job(&mut self, job:AnfJob) {
    let state = self.state.as_ref().unwrap();
    let res = match job {
      AnfJob::Xor(x, y) => match self.xor_step(x, y) {
        CoreStep::Nid(n) => state.resolve_job(&job, n),
        CoreStep::Parts(parts, children) => {
          let mut res = state.add_wip(&job, parts);
          if res.is_none() {
            for (slot, child) in children {
              res = self.attach_child(&job, slot, child);
            }
          }
          res
        }
      },
      AnfJob::And(x, y) => match self.and_step(x, y) {
        CoreStep::Nid(n) => state.resolve_job(&job, n),
        CoreStep::Parts(parts, children) => {
          let mut res = state.add_wip(&job, parts);
          if res.is_none() {
            for (slot, child) in children {
              res = self.attach_child(&job, slot, child);
            }
          }
          res
        }
      }
    };
    if let Some(Answer(nid)) = res { self.send_answer(nid) }
  }

  fn xor_core(&self, x:NID, y:NID)->Core {
    if let Some(n) = simp::xor(x, y) { Core::Nid(n) }
    else { Core::Job(AnfJob::xor(x, y)) }
  }

  fn and_core(&self, x:NID, y:NID)->Core {
    if let Some(n) = simp::and(x, y) { Core::Nid(n) }
    else { Core::Job(AnfJob::and(x, y)) }
  }

  fn xor_step(&self, x:NID, y:NID)->CoreStep {
    let store = &self.state.as_ref().unwrap().base;
    let (xv, yv) = (x.vid(), y.vid());
    match xv.cmp_depth(&yv) {
      VidOrdering::Above => {
        let Vhl{v, hi, lo} = store.fetch(x);
        CoreStep::Parts(
          AnfParts::Xor{ v, hi:Some(hi), lo:None },
          vec![(AnfSlot::Lo, self.xor_core(lo, y))]
        )
      }
      VidOrdering::Below => self.xor_step(y, x),
      VidOrdering::Level => {
        let Vhl{v:a, hi:b, lo:c} = store.fetch(x);
        let Vhl{v:p, hi:q, lo:r} = store.fetch(y);
        assert_eq!(a,p);
        CoreStep::Parts(
          AnfParts::Xor{ v:a, hi:None, lo:None },
          vec![
            (AnfSlot::Hi, self.xor_core(b, q)),
            (AnfSlot::Lo, self.xor_core(c, r)),
          ]
        )
      }
    }
  }

  fn and_step(&self, x:NID, y:NID)->CoreStep {
    let store = &self.state.as_ref().unwrap().base;
    let (xv, yv) = (x.vid(), y.vid());
    match xv.cmp_depth(&yv) {
      VidOrdering::Above =>
        if x.is_vid() { CoreStep::Nid(store.vhl(x.vid(), y, O)) }
        else {
          let Vhl{v:a, hi:b, lo:c } = store.fetch(x);
          CoreStep::Parts(
            AnfParts::AndAbove{ v:a, hi:None, lo:None },
            vec![
              (AnfSlot::Hi, self.and_core(b, y)),
              (AnfSlot::Lo, self.and_core(c, y)),
            ]
          )
        },
      VidOrdering::Below => self.and_step(y, x),
      VidOrdering::Level => {
        let Vhl{ v:a, hi:b, lo:c } = store.fetch(x);
        let Vhl{ v:p, hi:q, lo:r } = store.fetch(y);
        assert_eq!(a,p);
        CoreStep::Parts(
          AnfParts::AndLevel{ v:a, bq:None, br:None, cq:None, cr:None },
          vec![
            (AnfSlot::Bq, self.and_core(b, q)),
            (AnfSlot::Br, self.and_core(b, r)),
            (AnfSlot::Cq, self.and_core(c, q)),
            (AnfSlot::Cr, self.and_core(c, r)),
          ]
        )
      }
    }
  }
}

impl Worker<AnfQ, wip::RMsg, AnfJob> for AnfWorker {
  fn new(wid:WID)->Self { AnfWorker{ wid, ..Default::default() } }
  fn get_wid(&self)->WID { self.wid }
  fn set_tx(&mut self, tx:&Sender<SwarmRMsg<wip::RMsg>>) { self.tx = Some(tx.clone()) }
  fn queue_pop(&mut self)->Option<AnfJob> {
    if self.next.is_some() { self.next.take() }
    else if let Some(ref q) = self.queue { q.pop() }
    else { None }
  }
  fn queue_push(&mut self, job:AnfJob) {
    if self.next.is_none() { self.next = Some(job) }
    else { self.queue.as_ref().unwrap().push(job) }
  }
  fn work_item(&mut self, job:AnfJob) { self.work_job(job) }
  fn work_step(&mut self, qid:&QID, q:AnfQ)->Option<wip::RMsg> {
    match q {
      AnfQ::Init(s, q) => { self.state = Some(s); self.queue=Some(q); None }
      AnfQ::Job(job) => {
        let s = self.state.as_mut().unwrap();
        if let Some(cached) = s.get_done(&job) { return Some(wip::RMsg::Ret(cached)) }
        s.cache.entry(job).or_default();
        { let mut m = s.qid.lock().unwrap();
          assert!((*m).is_none(), "already working on a top-level query");
          *m = Some(*qid);
        }
        self.queue_push(job);
        None
      }
      AnfQ::Stats => {
        let tests = COUNT_CACHE_TESTS.with(|c| c.replace(0));
        let hits = COUNT_CACHE_HITS.with(|c| c.replace(0));
        Some(wip::RMsg::CacheStats{ tests, hits })
      }
    }
  }
}

enum Core { Nid(NID), Job(AnfJob) }
enum CoreStep { Nid(NID), Parts(AnfParts, Vec<(AnfSlot, Core)>) }

#[derive(Debug, Default)]
pub struct AnfSwarm {
  swarm: Swarm<AnfQ, wip::RMsg, AnfWorker, AnfJob>,
  state: Arc<WorkState<AnfJob, AnfParts, AnfStore>>,
  queue: Arc<JobQueue<AnfJob>>,
}

impl AnfSwarm {
  pub fn new()->Self { let mut me = Self::default(); me.reset(); me }
  pub fn new_with_threads(n:usize)->Self {
    let mut me = Self { swarm: Swarm::new_with_threads(n), ..Default::default() };
    me.reset();
    me
  }
  pub fn reset(&mut self) {
    self.state = Default::default();
    self.queue = Default::default();
    self.swarm.send_to_all(&AnfQ::Init(self.state.clone(), self.queue.clone()));
  }
  pub fn run_job(&mut self, job:AnfJob)->NID {
    let mut result = None;
    self.swarm.add_query(AnfQ::Job(job));
    while result.is_none() {
      let SwarmRMsg{wid:_, qid:_, r} = self.swarm.recv().expect("failed to recieve rmsg");
      if let Some(wip::RMsg::Ret(n)) = r { result = Some(n) }
    }
    result.unwrap()
  }
  pub fn xor(&mut self, x:NID, y:NID)->NID { self.run_job(AnfJob::xor(x,y)) }
  pub fn and(&mut self, x:NID, y:NID)->NID { self.run_job(AnfJob::and(x,y)) }
}

#[derive(Debug)]
pub struct AnfSwarmBase {
  pub tags: HashMap<String, NID>,
  pub swarm: AnfSwarm,
}

impl Default for AnfSwarmBase {
  fn default() -> Self {
    Self { tags: HashMap::new(), swarm: AnfSwarm::new() }
  }
}

impl AnfSwarmBase {
  pub fn new()->Self { Self::default() }
  pub fn new_with_threads(n:usize)->Self { Self { tags: HashMap::new(), swarm: AnfSwarm::new_with_threads(n) } }
  fn fetch(&self, n:NID)->Vhl { self.swarm.state.base.fetch(n) }

  pub fn first_term(&self, n:NID)->Option<Cursor> {
    if n == O { return None }
    let nvars = n.vid().var_ix();
    let mut cur = Cursor::new(nvars, n);
    cur.descend(self);
    Some(cur)
  }

  pub fn next_term(&self, mut cur:Cursor)->Option<Cursor> {
    if !cur.node.is_const() { cur.descend(self); }
    loop {
      cur.step_up();
      cur.ascend();
      if cur.at_top() && cur.var_get() { return None }
      cur.clear_trailing_bits();
      cur.put_step(self, true);
      if cur.node == I { return Some(cur) }
      cur.descend(self);
      if cur.node == I { return Some(cur) }
    }
  }

  pub fn terms(&self, n:NID)->AnfTermIterator<'_> {
    AnfTermIterator::from_anf_base(self, n)
  }

  pub fn to_base(&self, n:NID, dest: &mut dyn Base)->NID {
    let mut sum = nid::O;
    if n.is_inv() { sum = nid::I }
    for t in self.terms(n.raw()) {
      let mut term = I;
      for v in t.hi_bits() { term = dest.and(term, NID::var(v as u32)); }
      sum = dest.xor(sum, term);
    }
    sum
  }
}

impl Walkable for AnfSwarmBase {
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>, topdown: bool)
  where F: FnMut(NID,VID,NID,NID) {
    if !seen.contains(&n) {
      seen.insert(n);
      let Vhl{ v, hi, lo, } = self.fetch(n);
      if topdown { f(n,v,hi,lo) }
      if !lo.is_const() { self.step(lo, f, seen, topdown) }
      if !hi.is_const() { self.step(hi, f, seen, topdown) }
      if !topdown { f(n,v,hi,lo) }
    }
  }
}

impl HiLoBase for AnfSwarmBase {
  fn get_hilo(&self, nid:NID)->Option<HiLo> {
    let Vhl { v:_, hi, lo } = self.fetch(nid);
    Some(HiLo { hi, lo })
  }
}

impl CursorPlan for AnfSwarmBase {}

impl Base for AnfSwarmBase {
  fn new()->Self where Self:Sized { Self::default() }

  fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write) {
    macro_rules! w {
      ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}
    w!("digraph anf {{");
    w!("  bgcolor=\"#3399cc\"; pad=0.225");
    w!("  node[shape=circle, style=filled, fillcolor=\"#cccccc\", fontname=calibri]");
    w!("  edge[arrowhead=none]");
    w!("subgraph head {{ h1[shape=plaintext, fillcolor=none, label=\"ANF\"] }}");
    w!("  I[label=⊤, shape=square, fillcolor=white]");
    w!("  O[label=⊥, shape=square, fontcolor=white, fillcolor=\"#333333\"]");
    w!("{{rank = same; I; O;}}");
    self.walk_dn(n, &mut |n,_,_h,_l| w!("  \"{}\"[label=\"{:?}\"];", n, n.vid()));
    w!("edge[style=solid];");
    self.walk_dn(n, &mut |n,_,hi,_l| w!("  \"{:?}\"->\"{:?}\";", n, hi));
    w!("edge[style=dashed];");
    self.walk_dn(n, &mut |n,_,__,lo| w!("  \"{:?}\"->\"{:?}\";", n, lo));
    w!("}}");
  }

  fn def(&mut self, _s:String, _v:VID)->NID { todo!("anf_swarm::def") }
  fn tag(&mut self, n:NID, s:String)->NID { self.tags.insert(s, n); n }
  fn get(&self, s:&str)->Option<NID> { Some(*self.tags.get(s)?) }

  fn when_lo(&mut self, v:VID, n:NID)->NID { self.swarm.state.base.when_lo(v, n) }
  fn when_hi(&mut self, v:VID, n:NID)->NID { self.swarm.state.base.when_hi(v, n) }

  fn and(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::and(x,y) { nid }
    else {
      let (a,b) = (x.raw(), y.raw());
      if x.is_inv() {
        if y.is_inv() {
          let ab = self.and(a,b);
          let ab_xor_b = self.xor(ab, b);
          let a_xor_rest = self.xor(a, ab_xor_b);
          self.xor(I, a_xor_rest)
        }
        else {
          let ab = self.and(a,b);
          self.xor(ab, b)
        }
      }
      else if y.is_inv() {
        let ab = self.and(a,b);
        self.xor(ab, a)
      }
      else { self.swarm.and(x, y) }
    }
  }

  fn xor(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::xor(x,y) { nid }
    else {
      let (a, b) = (x.raw(), y.raw());
      let res = self.swarm.xor(a, b);
      if x.is_inv() == y.is_inv() { res } else { !res }
    }
  }

  fn or(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::or(x,y) { nid }
    else {
      let xy = self.and(x, y);
      let xxy = self.xor(x, y);
      self.xor(xy, xxy)
    }
  }

  fn ite(&mut self, i:NID, t:NID, e:NID)->NID {
    if let Some(nid) = simp::ite(i,t,e) { nid }
    else {
      let not_i = !i;
      let it = self.and(i, t);
      let nie = self.and(not_i, e);
      self.xor(it, nie)
    }
  }

  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID { self.swarm.state.base.sub(v, n, ctx) }

  fn solution_set(&self, n: NID, nvars: usize)->HashSet<Reg> {
    let mut bdd = BddBase::new();
    let bnid = self.to_base(n, &mut bdd);
    bdd.solution_set(bnid, nvars)
  }
}

pub struct AnfTermIterator<'a> {
  base: &'a AnfSwarmBase,
  next: Option<Cursor>,
}

impl<'a> AnfTermIterator<'a> {
  pub fn from_anf_base(base: &'a AnfSwarmBase, nid:NID)->Self {
    if let Some(next) = base.first_term(nid) {
      AnfTermIterator{ base, next:Some(next) }
    } else {
      AnfTermIterator{ base, next:None }
    }
  }
}

impl Iterator for AnfTermIterator<'_> {
  type Item = Reg;
  fn next(&mut self)->Option<Self::Item> {
    if let Some(cur) = self.next.take() {
      let reg = cur.scope.clone();
      self.next = self.base.next_term(cur);
      Some(reg)
    } else { None }
  }
}

#[test] fn test_swarm_anf_xor() {
  let mut a0 = crate::anf::ANFBase::new();
  let mut a1 = AnfSwarmBase::new();
  let (x0, x1, x2) = (NID::var(0), NID::var(1), NID::var(2));
  let t0 = a0.xor(x0, x1);
  let n0 = a0.xor(t0, x2);
  let t1 = a1.xor(x0, x1);
  let n1 = a1.xor(t1, x2);
  assert_eq!(a0.solution_set(n0, 3), a1.solution_set(n1, 3));
}

#[test] fn test_swarm_anf_and() {
  let mut a0 = crate::anf::ANFBase::new();
  let mut a1 = AnfSwarmBase::new();
  let (x0, x1, x2) = (NID::var(0), NID::var(1), NID::var(2));
  let t0 = a0.xor(x0, x1);
  let n0 = a0.and(t0, x2);
  let t1 = a1.xor(x0, x1);
  let n1 = a1.and(t1, x2);
  assert_eq!(a0.solution_set(n0, 3), a1.solution_set(n1, 3));
}
