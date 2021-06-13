//! Cursors: tools for navigating (v,hi,lo) structures.

use reg::Reg;
use {nid,nid::NID};
use vid::VID;
use vhl::{HiLoPart, HiLoBase};

pub trait CursorPlan : HiLoBase {
  /// is the given (leaf) node a solution, given the current inversion state?
  fn includes_leaf(&self, n:NID)->bool { n == nid::I }
  fn includes_lo(&self, n:NID)->bool { n != nid::O }
}


pub struct Cursor {
  pub nvars: usize,       // number of input variables in context
  pub node: NID,          // the current node.
  pub scope: Reg,         // the current variable assignments
  pub nstack: Vec<NID>,   // the path of nodes we have traversed
  pub istack: Vec<bool>,  // the stack of node inversion states
  pub invert: bool,       // whether to invert the results
}

impl Cursor {

  pub fn new(nvars:usize, node:NID)->Self {
    Cursor {
      node,
      nvars,
      invert: false,  // start:0, swap when we push, so parity of self.nstack == self.invert
      scope: Reg::new(nvars),
      nstack: vec![],
      istack: vec![]}}

  /// push a new node onto the stack
  fn push_node(&mut self, node:NID) {
    self.istack.push(self.invert);
    self.nstack.push(self.node);
    self.node = node;
    self.invert = nid::is_inv(node) && !nid::is_const(node); }

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

  /// walk down to next included term while setting the scope
  pub fn descend(&mut self, base: &dyn CursorPlan) {
    while !nid::is_const(self.node) {
      let hl = base.get_hilo(self.node).expect("couldn't get_hilo");
      let choice = !base.includes_lo(hl.lo);
      self.put_step(base, choice) }}

  pub fn var_get(&self)->bool {
    self.scope.var_get(self.node.vid()) }

  /// starting at a leaf, climb the stack until we reach
  /// a branch whose variable is still set to lo.
  pub fn go_next_lo_var(&mut self) {
    let mut bv = self.node.vid();
    while self.scope.var_get(bv) && !self.nstack.is_empty() {
      bv = self.step_up().vid(); }}

  pub fn clear_trailing_bits(&mut self) {
    let bi = self.node.vid().var_ix();
    for i in 0..bi { self.scope.put(i, false) }}

  /// decorate the increment() method on the scope register.
  /// returns Some index of first 0 or None on overflow.
  pub fn increment(&mut self)->Option<usize> {
    if let Some(zpos) = self.scope.increment() {
      let vz = VID::var(zpos as u32);
      // climb the bdd until we find the layer where the lmz would be.
      while !self.nstack.is_empty()
        && !vz.is_below(&self.nstack[self.nstack.len()-1].vid()) {
        self.pop_node(); }
      Some(zpos) }
    else { None }}

  }