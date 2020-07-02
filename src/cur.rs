/// Cursors: tools for navigating (v,hi,lo) structures.
use reg::Reg;
use {nid,nid::NID};
use vid::{VID,SMALL_ON_TOP};


pub struct Cursor {
  pub nvars: usize,       // number of input variables in context
  pub node: NID,          // the current node.
  pub scope: Reg,         // the current variable assignments
  pub nstack: Vec<NID>,   // the path of nodes we have traversed
  pub istack: Vec<bool>,  // the stack of node inversion states
  pub invert: bool,       // whether to invert the results
}

impl Cursor {

  /// push a new node onto the stack
  pub fn push_node(&mut self, node:NID) {
    self.istack.push(self.invert);
    self.nstack.push(self.node);
    self.node = node;
    self.invert = nid::is_inv(node) && !nid::is_const(node); }

  /// pop a node from the stack.
  pub fn pop_node(&mut self)->NID {
    assert!(!self.nstack.is_empty());
    let res = self.node;
    self.invert = self.istack.pop().expect("istack.pop() should have worked, as len>0");
    self.node = self.nstack.pop().expect("nstack.pop() should have worked, as len>0");
    res }

  /// set entry in scope to hi for current branch.
  /// returns false if the entry was alreday hi
  pub fn set_var_hi(&mut self)->bool {
    let bv = self.node.vid();
    if self.scope.var_get(bv) { false }
    else { self.scope.var_put(bv, true); true }}

  /// starting at a leaf, climb the stack until we reach
  /// a branch whose variable is still set to lo.
  pub fn to_next_lo_var(&mut self) {
    let mut bv = self.node.vid();
    while !self.nstack.is_empty() && self.scope.var_get(bv) {
      bv = self.pop_node().vid(); }}

  /// set all variables below current branch to lo
  pub fn clear_trailing_bits(&mut self) {
    let bi = self.node.vid().var_ix();
    if SMALL_ON_TOP {
      for i in (bi+1)..self.nvars { self.scope.put(i, false); }}
    else if bi > 0 { // no trailing bits if branch on x0
      for i in 0..(bi-1) { self.scope.put(i, false) }}}

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