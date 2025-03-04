//! Cursors (register + stack and scope) for navigating vhl-graphs (Bdd, Anf, etc)

use crate::reg::Reg;
use crate::{nid,nid::NID};
use crate::vid::VID;
use crate::vhl::{HiLoPart, HiLoBase};

pub trait CursorPlan : HiLoBase {
  /// is the given (leaf) node a solution, given the current inversion state?
  fn includes_leaf(&self, n:NID)->bool { n == nid::I }
  fn includes_lo(&self, n:NID)->bool { n != nid::O }
}

#[derive(Debug)]
pub struct Cursor {
  /// number of input variables in context
  pub nvars: usize,
  /// the current node.
  pub node: NID,
  /// whether to invert the results
  pub invert: bool,
  /// the path of nodes we have traversed
  pub nstack: Vec<NID>,
  /// the stack of node inversion states
  pub istack: Vec<bool>,
  /// the current variable assignments
  pub scope: Reg,
  /// can_skip[i]=1 means the variable is a "don't care" and can be skipped over.
  /// this is set on each step by the next_solution method of whatever data structure
  /// we're iterating through.
  pub can_skip: Reg,
  /// watch[i]=1 is an indication from the caller that they wants us to force
  /// iteration over this variable regardless of can_skip[i]
  pub watch: Reg }

impl Cursor {

  pub fn new(nvars:usize, node:NID)->Self {
    Cursor {
      nvars,
      node,
      invert: false,  // start:0, swap when we push, so parity of self.nstack == self.invert
      scope: Reg::new(nvars),
      can_skip: Reg::new(nvars), // by default we don't skip anything
      watch: Reg::new(nvars), // nor do we force anything
      nstack: vec![],
      istack: vec![]}}

  pub fn new_with_watch(nvars:usize, node:NID, watch:Reg)->Self {
    Self { watch, ..Self::new(nvars, node) }}

  /// push a new node onto the stack
  fn push_node(&mut self, node:NID) {
    self.istack.push(self.invert);
    self.nstack.push(self.node);
    self.node = node;
    self.invert = node.is_inv() && !node.is_const() }

  /// pop a node from the stack and return the old node id.
  fn pop_node(&mut self) {
    assert!(!self.nstack.is_empty());
    self.invert = self.istack.pop().expect("istack.pop() should have worked, as len>0");
    self.node = self.nstack.pop().expect("nstack.pop() should have worked, as len>0"); }

  /// take one step upward and return new node id.
  pub fn step_up(&mut self)->NID {
    self.pop_node();
    self.node }

  pub fn at_top(&self)->bool { self.nstack.is_empty() }

  fn step_down(&mut self, base: &dyn CursorPlan, which:HiLoPart) {
    let hl = base.get_hilo(self.node).expect("node not found for step_down");
    self.push_node(hl.get_part(which)); }

  pub fn put_step(&mut self, base:&dyn CursorPlan, val:bool) {
   self.scope.var_put(self.node.vid(), val);
   if val { self.step_down(base, HiLoPart::HiPart) }
   else { self.step_down(base, HiLoPart::LoPart) }}

  pub fn dontcares(&self)->Vec<usize> {
    println!("self.can_skip = {:?}", self.can_skip);
    println!("self.watch.= {:?}", self.watch);
    let mut res = vec![];
    for i in self.can_skip.hi_bits() {
      if !self.watch.get(i) { res.push(i) }}
    res }

  pub fn cube(&self)->Vec<(VID,bool)> {
    let mut res = vec![];
    for i in 0..self.nvars {
      if self.watch.get(i) || !self.can_skip.get(i) {
        res.push((VID::var(i as u32), self.scope.get(i))) }}
    res }

  /// walk down to next included term while setting the scope.
  /// this finds the leftmost leaf beneath the current node that contains a solution.
  /// it does NOT backtrack up higher in the graph, so once we reach the bottom we have
  /// to call ascend() to get back to the next branch point.
  pub fn descend(&mut self, base: &dyn CursorPlan) {
    while !self.node.is_const() {
      let hl = base.get_hilo(self.node).expect("couldn't get_hilo");
      let choice = !base.includes_lo(hl.lo);
      self.put_step(base, choice) }}

  pub fn var_get(&self)->bool {
    self.scope.var_get(self.node.vid()) }

  /// starting at a leaf, climb the stack until we reach
  /// a branch whose variable is still set to lo.
  pub fn ascend(&mut self) {
    let mut bv = self.node.vid();
    while self.scope.var_get(bv) && !self.nstack.is_empty() {
      bv = self.step_up().vid(); }}

  pub fn clear_trailing_bits(&mut self) {
    let bi = self.node.vid().var_ix();
    for i in 0..bi { self.scope.put(i, false) }}

  /// decorate the increment() method on the scope register.
  /// returns Some index of first 0 or None on overflow.
  pub fn increment(&mut self) -> Option<usize> {
    // directly compose bits from the three registers to handle the "don't care" situation
    let len = self.scope.data.len();
    for i in 0..len { self.scope.data[i] |= self.can_skip.data[i] & (!self.watch.data[i]); }
    // then increment as usual, and capture the bottom 0 index
    if let Some(zpos) = self.scope.increment() {
      let vz = VID::var(zpos as u32);
      // climb the bdd until we find the layer where the lmz would be.
      while !self.nstack.is_empty()
        && !vz.is_below(&self.nstack[self.nstack.len()-1].vid()) {
        self.pop_node(); }
      Some(zpos) }
    else { None }}}
