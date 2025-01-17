//! mini-framework for multicore programming.
use std::{marker::PhantomData, thread};
use std::sync::mpsc::{Sender, Receiver, channel, SendError, RecvError};
use std::fmt::Debug;
use std::collections::HashMap;
use rand::seq::SliceRandom;

/// query id
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum QID { #[default] INIT, STEP(usize), DONE }

pub struct QMsg<Q> { qid:QID, q: Q }
#[derive(Debug)]
pub struct RMsg<R> { pub wid: WID, pub qid:QID, pub r:Option<R> }

/// worker id
#[derive(Debug,Default,PartialEq,Eq,Hash,Clone,Copy)]
pub struct WID { pub n:usize }

pub trait Worker<Q,R,I=()> where R:Debug, Q:Clone {

  fn new(_wid:WID)->Self;
  fn get_wid(&self)->WID;

  // swarm will call this method so a worker implementation
  // can clone the sender and send messages back to the swarm
  // outside of the work_xxx methods.
  fn set_tx(&mut self, _tx:&Sender<RMsg<R>>) {}

  /// send a message from the worker back to the swarm's main thread.
  /// to call this, you need your own copy of the Sender (which you)
  /// can obtain by implementing `set_tx` and keeping a reference to
  /// a clone of the parameter. (You probably also need the qid, which
  /// you can capture in `work_step`)
  fn send_msg(&self, tx:&Sender<RMsg<R>>, qid:QID, r:Option<R>) {
    // println!("\x1b[32mSENDING msg: qid:{:?} for wid: {:?} -> r:{:?}\x1b[0m", &qid, wid, &r);
    let res = tx.send(RMsg{ wid:self.get_wid(), qid, r });
    if res.is_err() { self.on_work_send_err(res.err().unwrap()) }}

  /// allow workers to push items into a shared (or private) queue.
  fn queue_push(&mut self, _item:I) { panic!("no queue defined"); }
  /// allow workers to pop items from a shared (or private) queue.
  fn queue_pop(&mut self)->Option<I> { None }

  /// Generic worker lifecycle implementation.
  /// Hopefully, you won't need to override this.
  /// The worker receives a stream of Option(Q) structs (queries),
  /// and returns an R (result) for each one.
  fn work_loop(&mut self, wid:WID, rx:&Receiver<Option<QMsg<Q>>>, tx:&Sender<RMsg<R>>) {
    self.set_tx(tx);
    // and now the actual worker lifecycle:
    let msg = self.work_init(wid); self.send_msg(tx, QID::INIT, msg);
    loop {
      if let Some(item) = self.queue_pop() { self.work_item(item) }
      match rx.try_recv() {
        Ok(None) => break,
        Ok(Some(QMsg{qid, q})) => {
          if let QID::STEP(_) = qid {
            let msg = self.work_step(&qid, q); self.send_msg(tx, qid, msg); }
          else { panic!("Worker {:?} got unexpected qid instead of STEP: {:?}", wid, qid)}}
        Err(e) => match e {
          std::sync::mpsc::TryRecvError::Empty => {} // no problem!
          std::sync::mpsc::TryRecvError::Disconnected => break }}}
    let msg = self.work_done(); self.send_msg(tx, QID::DONE, msg); }

  /// What to do if a message send fails. By default, just print to stdout.
  fn on_work_send_err(&self, err:SendError<RMsg<R>>) {
    println!("failed to send response: {:?}", err.to_string()); }

  /// Override this to implement logic for working on queue items
  fn work_item(&mut self, _item:I) {  }

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
  // send a new query to a worker
  Send(Q),
  Batch(Vec<(WID, Q)>),
  Panic(String),
  Return(V),
  // kill the worker
  Kill(WID)}

#[derive(Debug)]
pub struct Swarm<Q,R,W,I=()> where W:Worker<Q,R,I>, Q:Debug+Clone, R:Debug {
  /// next QID
  nq: usize,
  //// sender that newly spawned workers can clone to talk to me.
  me: Sender<RMsg<R>>,
  /// receives result (and other intermediate) messages from the workers.
  rx: Receiver<RMsg<R>>,
  /// sender for queries. clone with self.q_sender()
  qtx: Sender<Q>,
  qrx: Receiver<Q>,
  // /// worker queue. workers queue up to handle the queries.
  // wq: VecDeque<usize>,
  /// handles for sending messages to the workers
  whs: HashMap<WID, Sender<Option<QMsg<Q>>>>,
  /// next unique id for new worker
  nw: usize,
  /// phantom reference to the Worker class. In practice, the workers are owned
  /// by their threads, so we don't actually touch them directly.
  _w: PhantomData<W>,
  _i: PhantomData<I>,
  /// handles to the actual threads
  threads: Vec<thread::JoinHandle<()>> }

impl<Q,R,W,I> Default for Swarm<Q,R,W,I> where Q:'static+Send+Debug+Clone, R:'static+Send+Debug, W:Worker<Q, R,I> {
  fn default()->Self { Self::new_with_threads(4) }}

impl<Q,R,W,I> Drop for Swarm<Q,R,W,I> where Q:Debug+Clone, R:Debug, W:Worker<Q, R,I> {
  fn drop(&mut self) { self.kill_swarm() }}

impl<Q,R,W,I> Swarm<Q,R,W,I> where Q:Debug+Clone, R:Debug, W:Worker<Q, R,I> {

  pub fn kill_swarm(&mut self) {
    while let Some(&w) = self.whs.keys().take(1).next() { self.kill(w); }
      while !self.threads.is_empty() { self.threads.pop().unwrap().join().unwrap() }}

  pub fn num_workers(&self)->usize { self.whs.len() }

  pub fn kill(&mut self, w:WID) {
    if let Some(h) = self.whs.remove(&w) {
      if h.send(None).is_err() { panic!("couldn't kill worker") }}
    else { panic!("worker was already gone") }}}

impl<Q,R,W,I> Swarm<Q,R,W,I> where Q:'static+Send+Debug+Clone, R:'static+Send+Debug, W:Worker<Q, R, I> {

  pub fn new()->Self { Self::default() }

  pub fn new_with_threads(n:usize)->Self {
    let (tx, rx) = channel();
    let (qtx, qrx) = channel();
    let mut me = Self { nq: 0, me:tx, rx, qtx, qrx, whs:HashMap::new(), nw:0,
       _w:PhantomData, _i:PhantomData, threads:vec![]};
    me.start(n); me }

  pub fn start(&mut self, num_workers:usize) {
    let n = if num_workers==0 { num_cpus::get() } else { num_workers };
    for _ in 0..n { self.spawn(); }}

  fn spawn(&mut self)->WID {
    let wid = WID{ n: self.nw }; self.nw+=1;
    let me2 = self.me.clone();
    let (wtx, wrx) = channel();
    self.threads.push(thread::spawn(move || { W::new(wid).work_loop(wid, &wrx, &me2) }));
    self.whs.insert(wid, wtx);
    wid }

  /// send query to an arbitrary worker.
  pub fn add_query(&mut self, q:Q)->QID {
    // let wid = self.whs.keys().collect::<Vec<_>>()[0];
    let &wid = self.whs.keys().collect::<Vec<_>>()
       .choose(&mut rand::thread_rng()).unwrap();
    self.send(*wid, q)}

  pub fn send(&mut self, wid:WID, q:Q)->QID {
    let qid = QID::STEP(self.nq); self.nq+=1;
    let w = self.whs.get(&wid).unwrap_or_else(||
      panic!("requested non-existent worker {:?}", wid));
    if w.send(Some(QMsg{ qid, q })).is_err() {
      panic!("couldn't send message to worker {:?}", wid) }
    qid}

  pub fn recv(&self)->Result<RMsg<R>, RecvError> { self.rx.recv() }

  pub fn send_to_all(&mut self, q:&Q) {
    let wids: Vec<WID> = self.whs.keys().cloned().collect();
    for wid in wids { self.send(wid, q.clone()); }}

  /// returns a channel to which you can send a Q, rather than calling
  /// add_query. (useful when the swarm is running in a separate thread)
  pub fn q_sender(&self)->Sender<Q> { self.qtx.clone() }

  pub fn send_to_self(&self, r:R) {
    self.me.send(RMsg{ wid:WID::default(), qid:QID::default(), r:Some(r)})
      .expect("failed to sent_self"); }

  /// pass in the swarm dispatch loop
  pub fn run<F,V>(&mut self, mut on_msg:F)->Option<V>
    where V:Debug, F:FnMut(WID, &QID, Option<R>)->SwarmCmd<Q,V> {
    let mut res = None;
    loop {
      if let Ok(q) = self.qrx.try_recv() { self.add_query(q); }
      if let Ok(rmsg) = self.rx.try_recv() {
        let RMsg { wid, qid, r } = rmsg;
        let cmd = on_msg(wid, &qid, r);
        match cmd {
          SwarmCmd::Pass => {},
          SwarmCmd::Halt => break,
          SwarmCmd::Kill(w) => { self.kill(w); if self.whs.is_empty() { break }},
          SwarmCmd::Send(q) => { self.send(wid, q); },
          SwarmCmd::Batch(wqs) => for (wid, q) in wqs { self.send(wid, q); },
          SwarmCmd::Panic(msg) => panic!("{}", msg),
          SwarmCmd::Return(v) => { res = Some(v); break }}}}
      res}}
