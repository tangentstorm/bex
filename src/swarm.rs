use std::{collections::VecDeque, marker::PhantomData, sync::mpsc::{Sender, Receiver, channel}, thread};
use std::fmt::Debug;
use hashbrown::HashMap;

// enum Q {
//   Init{ ru:XVHLRow, rd: XVHLRow },
//   Step{ rd: XVHLRow },
//   Xids( Vec<XID> )}

// enum R {
//   Alloc{ n: usize },
//   PutRD{ rd: XVHLRow },
//   PutRU{ ru: XVHLRow }}



/// query id
#[derive(Debug, Clone)]
enum QID { INIT, STEP(usize), DONE }

#[derive(Debug)]
struct QMsg<Q:Clone> { qid:QID, q: Q }
#[derive(Debug)]
struct RMsg<R:Debug+Clone> { wid: WID, qid:QID, r:R }

/// worker id
type WID = usize; // worker id

trait Worker<Q,R>:Send+Sync where Q:Clone, R: Debug+Clone {

  fn new(wid:WID)->Self;

  /// Generic worker lifecycle implementation.
  /// Hopefully, you won't need to override this.
  /// The worker receives a stream of Option(Q) structs (queries),
  /// and returns an R (result) for each one.
  fn work_loop(&mut self, wid:WID, rx:&Receiver<Option<QMsg<Q>>>, tx:&Sender<RMsg<R>>) {
    // any phase can send a message if it wants:
    macro_rules! work_phase {
        [$qid:expr, $x:expr] => {
          if let Some(r) = $x {
            if tx.send(RMsg{ wid, qid:$qid, r:r.clone() }).is_err() { self.on_work_send_err($qid, r ) }}}}
    // and now the actual worker lifecycle:
    work_phase![QID::INIT, self.work_init(wid)];
    let mut stream = rx.iter();
    while let Some(Some(QMsg{qid, q})) = stream.next() {
      work_phase![qid.clone(), self.work_step(&qid, &q)]; }
    work_phase![QID::DONE, self.work_done()]; }

  /// What to do if a message send fails. By default, just print to stdout.
  fn on_work_send_err(&mut self, qid:QID, r:R) {
    println!("failed to send ({:?}, {:?})", qid, r); }

  /// Override this to implement your worker's query-handling logic.
  fn work_step(&mut self, _qid:&QID, _q:&Q)->Option<R> { None }

  /// Override this if you need to send a message to the swarm before the worker starts.
  fn work_init(&mut self, _wid:WID)->Option<R> { None }

  /// Override this if you need to send a message to the swarm after the work loop finishes.
  fn work_done(&mut self)->Option<R> { None }}

struct Swarm<'a,Q,R,W> where Q:Clone, R:Debug+Clone, W:Default+Worker<Q,R> {
  /// next QID
  nq: usize,
  /// receives result (and other intermediate) messages from the workers.
  rx: Receiver<RMsg<R>>,
  /// worker queue. workers queue up to handle the queries.
  wq: VecDeque<usize>,
  /// handles for sending messages to the workers
  whs: Vec<Sender<Option<QMsg<Q>>>>,
  /// phantom reference to the Worker class. In practice, the workers are owned
  /// by their threads, so we don't actually touch them directly.
  _w: PhantomData<W>,
  /// end of loop indicator
  done: bool,
  /// query queue. query will be given to next available worker
  qq: VecDeque<(QID, Q)>,
  /// callbacks
  cbs: HashMap<usize,Vec<&'a dyn FnMut(R)>> }

impl<'a,Q,R,W> Swarm<'a,Q,R,W> where Q:'static+Send+Clone, R:'static+Send+Debug+Clone, W:'a+Default+Worker<Q, R> {

  fn new(num_workers:usize)->Self {
    let (me, rx) = channel();
    let n = if num_workers==0 { num_cpus::get() } else { num_workers };
    let mut wq = VecDeque::new();
    let whs = (0..n).map(|wid| {
      let me2 = me.clone();
      let (wtx, wrx) = channel();
      thread::spawn(move || { W::new(wid).work_loop(wid, &wrx, &me2) });
      wq.push_back(wid);
      wtx }).collect();
    Self { nq: 0, rx, whs, wq, qq:VecDeque::new(), done:false, cbs:HashMap::new(), _w:PhantomData }}

  /// add a query to the work to be done, with callbacks
  fn add(&mut self, q:Q, cb:&'a dyn FnMut(R))->&Self {
    let qid:QID = QID::STEP(self.nq);
    self.cbs.entry(self.nq).or_insert_with(|| vec![]).push(cb);
    self.qq.push_back((qid, q));
    self.nq+=1;
    self}

  /// pass in the swarm dispatch loop
  fn run(&mut self, on_msg:&'a mut dyn FnMut(WID, QID, R)->bool) {
    self.done = false;
    while !self.done {
      let RMsg { wid, qid, r } = self.rx.recv().expect("failed to read RMsg from queue!");
      self.done = on_msg(wid, qid, r)}}}
