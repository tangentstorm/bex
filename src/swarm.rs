use std::{collections::VecDeque, marker::PhantomData, sync::mpsc::{Sender, Receiver, channel}, thread};
use std::fmt::Debug;
use hashbrown::HashMap;

/// query id
#[derive(Debug, Clone)]
pub enum QID { INIT, STEP(usize), DONE }

pub struct QMsg<Q> { qid:QID, q: Q }
#[derive(Debug)]
pub struct RMsg<R> { wid: WID, qid:QID, r:Option<R> }

/// worker id
#[derive(Debug,Default,PartialEq,Eq,Hash,Clone,Copy)]
pub struct WID { n:usize }

pub trait Worker<Q,R>:Send+Sync where R:Debug {

  fn new(_wid:WID)->Self;
  fn get_wid(&self)->WID;

  fn send_msg(&self, tx:&Sender<RMsg<R>>, qid:QID, r:Option<R>) {
    // println!("\x1b[32mSENDING msg: qid:{:?} for wid: {:?} -> r:{:?}\x1b[0m", &qid, wid, &r);
    if tx.send(RMsg{ wid:self.get_wid(), qid:qid.clone(), r }).is_err() {
       self.on_work_send_err(qid) }}

  /// Generic worker lifecycle implementation.
  /// Hopefully, you won't need to override this.
  /// The worker receives a stream of Option(Q) structs (queries),
  /// and returns an R (result) for each one.
  fn work_loop(&mut self, wid:WID, rx:&Receiver<Option<QMsg<Q>>>, tx:&Sender<RMsg<R>>) {
    // and now the actual worker lifecycle:
    let msg = self.work_init(wid); self.send_msg(tx, QID::INIT, msg);
    let mut stream = rx.iter();
    while let Some(Some(QMsg{qid, q})) = stream.next() {
      if let QID::STEP(_) = qid {
        let msg = self.work_step(&qid, q); self.send_msg(tx, qid, msg); }
      else { panic!("Worker {:?} got unexpected qid instead of STEP: {:?}", wid, qid)}}
    let msg = self.work_done(); self.send_msg(tx, QID::DONE, msg); }

  /// What to do if a message send fails. By default, just print to stdout.
  fn on_work_send_err(&self, qid:QID) {
    println!("failed to send response for qid:{:?}", qid); }

  /// Override this to implement your worker's query-handling logic.
  fn work_step(&mut self, _qid:&QID, _q:Q)->Option<R> { None }

  /// Override this if you need to send a message to the swarm before the worker starts.
  fn work_init(&mut self, _wid:WID)->Option<R> { None }

  /// Override this if you need to send a message to the swarm after the work loop finishes.
  fn work_done(&mut self)->Option<R> { None }}

#[derive(Debug)]
pub enum SwarmCmd<Q:Debug,V:Debug> {
  Pass,
  Halt,
  Send(Q),
  Batch(Vec<(WID, Q)>),
  Panic(String),
  Return(V),
  // kill the worker
  Kill(WID)}

pub struct Swarm<Q,R,W> where W:Worker<Q,R>, Q:Debug, R:Debug {
  /// next QID
  nq: usize,
  //// sender that newly spawned workers can clone to talk to me.
  me: Sender<RMsg<R>>,
  /// receives result (and other intermediate) messages from the workers.
  rx: Receiver<RMsg<R>>,
  // /// worker queue. workers queue up to handle the queries.
  // wq: VecDeque<usize>,
  /// handles for sending messages to the workers
  whs: HashMap<WID, Sender<Option<QMsg<Q>>>>,
  /// next unique id for new worker
  nw: usize,
  /// phantom reference to the Worker class. In practice, the workers are owned
  /// by their threads, so we don't actually touch them directly.
  _w: PhantomData<W>,
  /// query queue. query will be given to next available worker
  qq: VecDeque<(QID, Q)>,
  /// handles to the actual threads
  threads: Vec<thread::JoinHandle<()>> }

impl<Q,R,W> Swarm<Q,R,W> where Q:'static+Send+Debug, R:'static+Send+Debug, W:Worker<Q, R> {

  pub fn new(num_workers:usize)->Self {
    let (me, rx) = channel();
    let n = if num_workers==0 { num_cpus::get() } else { num_workers };
    let mut this = Self { nq: 0, me, rx, whs:HashMap::new(), nw:0, qq:VecDeque::new(), _w:PhantomData, threads:vec![]};
    for _ in 0..n { this.spawn(); }
    this }

  fn spawn(&mut self)->WID {
    let wid = WID{ n: self.nw }; self.nw+=1;
    let me2 = self.me.clone();
    let (wtx, wrx) = channel();
    self.threads.push(thread::spawn(move || { W::new(wid).work_loop(wid, &wrx, &me2) }));
    self.whs.insert(wid, wtx);
    wid }

  /// add a query to the work to be done, with callbacks
  pub fn add(&mut self, q:Q)->&Self {
    let qid:QID = QID::STEP(self.nq);
    self.qq.push_back((qid, q));
    self.nq+=1;
    self}

  pub fn get_worker(&mut self, wid:WID)->&Sender<Option<QMsg<Q>>> {
    self.whs.get(&wid).unwrap_or_else(||
      panic!("requested non-existent worker {:?}", wid)) }

  pub fn kill(&mut self, w:WID) {
    if let Some(h) = self.whs.remove(&w) {
      if h.send(None).is_err() { panic!("couldn't kill worker") }}
    else { panic!("worker was already gone") }}


  pub fn send(&mut self, wid:WID, q:Q) {
    let qid = QID::STEP(self.nq); self.nq+=1;
    if self.get_worker(wid).send(Some(QMsg{ qid, q })).is_err() {
      panic!("couldn't send message to worker {:?}", wid) }}

  /// pass in the swarm dispatch loop
  pub fn run<F,V>(&mut self, mut on_msg:F)->Option<V> where V:Debug, F:FnMut(WID, &QID, Option<R>)->SwarmCmd<Q,V> {
    let mut res = None;
    loop {
      let RMsg { wid, qid, r } = self.rx.recv().expect("failed to read RMsg from queue!");
      // println!("Received RMSG:: wid:{:?}, qid:{:?}, r:{:?}", wid, qid, &r );
      let cmd = on_msg(wid, &qid, r);
      // println!("-> cmd: {:?}", cmd);
      match cmd {
        SwarmCmd::Pass => {},
        SwarmCmd::Halt => break,
        SwarmCmd::Kill(w) => { self.kill(w); if self.whs.is_empty() { break }},
        SwarmCmd::Send(q) => self.send(wid, q),
        SwarmCmd::Batch(wqs) => for (wid, q) in wqs { self.send(wid, q) },
        SwarmCmd::Panic(msg) => panic!("{}", msg),
        SwarmCmd::Return(v) => { res = Some(v); break } }}
      while let Some(&w) = self.whs.keys().take(1).next() { self.kill(w); }
      while !self.threads.is_empty() { self.threads.pop().unwrap().join().unwrap() }
      res}}
