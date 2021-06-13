///! Solution iterator for BDDBase

use std::collections::HashSet;
use {vhl::{HiLo, HiLoBase, Walkable}};
use crate::{vid::VID, nid::{NID,I,O}, bdd::BDDBase, reg::Reg};
use crate::cur::{Cursor, CursorPlan};


/// helpers for solution cursor
impl HiLoBase for BDDBase {
  fn get_hilo(&self, n:NID)->Option<HiLo> {
    let (hi, lo) = self.swarm.get_state().tup(n);
    Some(HiLo{ hi, lo }) }}

impl Walkable for BDDBase {
  /// internal helper: one step in the walk.
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>, topdown:bool)
  where F: FnMut(NID,VID,NID,NID) {
    if !seen.contains(&n) {
      seen.insert(n); let (hi,lo) = self.tup(n);
      if topdown { f(n, n.vid(), hi,lo ) }
      if !hi.is_const() { self.step(hi, f, seen, topdown) }
      if !lo.is_const() { self.step(lo, f, seen, topdown) }
      if !topdown { f(n, n.vid(), hi, lo) }}}}

pub struct BDDSolIterator<'a> {
  bdd: &'a BDDBase,
  next: Option<Cursor>}

impl<'a> BDDSolIterator<'a> {
  pub fn from_bdd(bdd: &'a BDDBase, n:NID, nvars:usize)->BDDSolIterator<'a> {
    // init scope with all variables assigned to 0
    let next = bdd.first_solution(n, nvars);
    BDDSolIterator{ bdd, next }}}


impl<'a> Iterator for BDDSolIterator<'a> {
  type Item = Reg;
  fn next(&mut self)->Option<Self::Item> {
    if let Some(cur) = self.next.take() {
      assert!(self.bdd.in_solution(&cur));
      let result = cur.scope.clone();
      self.next = self.bdd.next_solution(cur);
      Some(result)}
    else { None }}}
    impl CursorPlan for BDDBase {}


/// Solution iterators.
impl BDDBase {
  pub fn solutions(&mut self, n:NID)->BDDSolIterator {
    let nvars = if n.is_const() { 1 } else if n.vid().is_var() { n.vid().var_ix() }
    else if n.vid().is_vir() {
      panic!("It probably doesn't make sense to call solutions(n) when n.vid().is_vir(), but you can try solutions_pad() if you think it makes sense.") }
    else { panic!("Don't know how to find solutions({:?}). Maybe try solutions_pad()...?", n) };
    self.solutions_pad(n, nvars)}

  pub fn solutions_pad(&self, n:NID, nvars:usize)->BDDSolIterator {
    BDDSolIterator::from_bdd(self, n, nvars)}

  pub fn first_solution(&self, n:NID, nvars:usize)->Option<Cursor> {
    if n== O || nvars == 0 { None }
    else {
      let mut cur = Cursor::new(nvars, n);
      cur.descend(self);
      debug_assert!(cur.node.is_const());
      debug_assert!(self.in_solution(&cur), "{:?}", cur.scope);
      Some(cur) }}

  pub fn next_solution(&self, cur:Cursor)->Option<Cursor> {
    self.log(&cur, "advance>"); self.log_indent(1);
    let res = self.advance0(cur); self.log_indent(-1);
    res }

  /// is the cursor currently pointing at a span of 1 or more solutions?
  pub fn in_solution(&self, cur:&Cursor)->bool {
    self.includes_leaf(cur.node) }

  fn log_indent(&self, _d:i8) { /*self.indent += d;*/ }
  fn log(&self, _c:&Cursor, _msg: &str) {
    #[cfg(test)]{
      print!(" {}", if _c.invert { '¬' } else { ' ' });
      print!("{:>10}", format!("{}", _c.node));
      print!(" {:?}{}", _c.scope, if self.in_solution(&_c) { '.' } else { ' ' });
      let s = format!("{}", /*"{}", "  ".repeat(self.indent as usize),*/ _msg,);
      println!(" {:50} {:?}", s, _c.nstack);}}

  /// walk depth-first from lo to hi until we arrive at the next solution
  fn find_next_leaf(&self, cur:&mut Cursor)->Option<NID> {
    self.log(cur, "find_next_leaf"); self.log_indent(1);
    let res = self.find_next_leaf0(cur);
    self.log(cur, format!("^ next leaf: {:?}", res.clone()).as_str());
    self.log_indent(-1); res }

  fn find_next_leaf0(&self, cur:&mut Cursor)->Option<NID> {
    // we always start at a leaf and move up, with the one exception of root=I
    assert!(cur.node.is_const(), "find_next_leaf should always start by looking at a leaf");
    if cur.nstack.is_empty() { assert!(cur.node == I); return None }

    // now we are definitely at a leaf node with a branch above us.
    cur.step_up();

    let tv = cur.node.vid(); // branching var for current twig node
    let mut rippled = false;
    // if we've already walked the hi branch...
    if cur.scope.var_get(tv) {
      cur.go_next_lo_var();
      // if we've cleared the stack and already explored the hi branch...
      { let iv = cur.node.vid();
        if cur.nstack.is_empty() && cur.scope.var_get(iv) {
          // ... then first check if there are any variables above us on which
          // the node doesn't actually depend. ifso: ripple add. else: done.
          let top = cur.nvars-1;
          if let Some(x) = cur.scope.ripple(iv.var_ix(), top) {
            rippled = true;
            self.log(cur, format!("rippled top to {}. restarting.", x).as_str()); }
          else { self.log(cur, "no next leaf!"); return None }}} }

    if rippled { cur.clear_trailing_bits() }
    else if cur.var_get() { self.log(cur, "done with node."); return None }
    else { cur.put_step(self, true); }
    cur.descend(self);
    Some(cur.node) }

  /// walk depth-first from lo to hi until we arrive at the next solution
  fn advance0(&self, mut cur:Cursor)->Option<Cursor> {
    assert!(cur.node.is_const(), "advance should always start by looking at a leaf");
    if self.in_solution(&cur) {
      // if we're in the solution, we're going to increment the "counter".
      if let Some(zpos) = cur.increment() {
        self.log(&cur, format!("rebranch on {:?}",zpos).as_str());
        // The 'zpos' variable exists in the solution space, but there might or might
        // not be a branch node for that variable in the current bdd path.
        // Whether we follow the hi or lo branch depends on which variable we're looking at.
        if cur.node.is_const() { return Some(cur) } // special case for topmost I (all solutions)
        cur.put_step(self, cur.var_get());
        cur.descend(self); }
      else { // overflow. we've counted all the way to 2^nvars-1, and we're done.
        self.log(&cur, "$ found all solutions!"); return None }}
    // If still here, we are looking at a leaf that isn't a solution (out=0 in truth table)
    while !self.in_solution(&cur) { self.find_next_leaf(&mut cur)?; }
    Some(cur) }}
