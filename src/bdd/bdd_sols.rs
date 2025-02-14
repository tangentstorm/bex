//! Solution iterator for BddBase

use std::collections::HashSet;
use crate::vhl::{HiLo, HiLoBase, Walkable};
use crate::{vid::VID, nid::{NID,I,O}, bdd::BddBase, reg::Reg};
use crate::cur::{Cursor, CursorPlan};


/// helpers for solution cursor
impl HiLoBase for BddBase {
  fn get_hilo(&self, n:NID)->Option<HiLo> {
    let (hi, lo) = self.swarm.tup(n);
    Some(HiLo{ hi, lo }) }}

impl Walkable for BddBase {
  /// internal helper: one step in the walk.
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>, topdown:bool)
  where F: FnMut(NID,VID,NID,NID) {
    if !seen.contains(&n) {
      seen.insert(n); let (hi,lo) = self.tup(n);
      if topdown { f(n, n.vid(), hi,lo ) }
      if !lo.is_const() { self.step(lo, f, seen, topdown) }
      if !hi.is_const() { self.step(hi, f, seen, topdown) }
      if !topdown { f(n, n.vid(), hi, lo) }}}}

pub struct BDDSolIterator<'a> {
  bdd: &'a BddBase,
  next: Option<Cursor>}

impl<'a> BDDSolIterator<'a> {
  pub fn from_bdd(bdd: &'a BddBase, n:NID, nvars:usize)->BDDSolIterator<'a> {
    let next = bdd.first_solution(n, nvars);
    BDDSolIterator{ bdd, next }}}


impl Iterator for BDDSolIterator<'_> {
  type Item = Reg;
  fn next(&mut self)->Option<Self::Item> {
    if let Some(cur) = self.next.take() {
      assert!(self.bdd.in_solution(&cur));
      let result = cur.scope.clone();
      self.next = self.bdd.next_solution(cur);
      Some(result)}
    else { None }}}
    impl CursorPlan for BddBase {}


/// Solution iterators.
impl BddBase {
  pub fn solutions(&mut self, n:NID)->BDDSolIterator {
    let nvars = if n.is_const() { 1 } else if n.vid().is_var() { n.vid().var_ix() }
    else if n.vid().is_vir() {
      panic!("It probably doesn't make sense to call solutions(n) when n.vid().is_vir(), but you can try solutions_pad() if you think it makes sense.") }
    else { panic!("Don't know how to find solutions({:?}). Maybe try solutions_pad()...?", n) };
    self.solutions_pad(n, nvars)}

  pub fn solutions_pad(&self, n:NID, nvars:usize)->BDDSolIterator {
    BDDSolIterator::from_bdd(self, n, nvars)}


  /// base function to make a cursor. if nvars < n.vid().var_ix(), it will be ignored.
  /// if it is larger than the var_ix, all variables above the nid will be watched.
  pub fn make_cursor(&self, n: NID, watch_vars: &[usize], nvars: usize) -> Option<Cursor> {
    if n == O { return None; }
    let base_nvars = if n.is_const() { 0 } else { n.vid().var_ix() + 1 };
    let real_nvars = std::cmp::max(base_nvars, nvars);
    let mut cur = Cursor::new(real_nvars, n);
    for &idx in watch_vars { cur.watch.put(idx, true); }
    cur.descend(self);
    self.mark_skippable(&mut cur);
    debug_assert!(cur.node.is_const());
    debug_assert!(self.in_solution(&cur), "{:?}", cur.scope);
    Some(cur)}

  // Construct a "don't care" cursor: effective nvars with all indices watched.
  pub fn make_dontcare_cursor(&self, n: NID, nvars: usize) -> Option<Cursor> {
    self.make_cursor(n, &[], nvars)}

  // cursor for .solutions: always watch all variables
  pub fn make_solution_cursor(&self, n: NID, nvars: usize) -> Option<Cursor> {
    let mut cur = self.make_cursor(n, &[], nvars)?;
    for i in 0..cur.nvars { cur.watch.put(i, true); }
    Some(cur)}

  pub fn first_solution(&self, n: NID, nvars: usize) -> Option<Cursor> {
    if n == O || nvars == 0 { None }
    else { self.make_solution_cursor(n, nvars)}}

  /// is the cursor currently pointing at a span of 1 or more solutions?
  pub fn in_solution(&self, cur:&Cursor)->bool {
    self.includes_leaf(cur.node) }


  /// helper function for next_solution
  /// walk depth-first from lo to hi until we arrive at the next solution
  fn find_next_leaf(&self, cur:&mut Cursor)->Option<NID> {
    // we always start at a leaf and move up, with the one exception of root=I
    assert!(cur.node.is_const(), "find_next_leaf should always start by looking at a leaf");
    if cur.nstack.is_empty() { assert!(cur.node == I); return None }

    // now we are definitely at a leaf node with a branch above us.
    cur.step_up();

    let tv = cur.node.vid(); // branching var for current twig node
    let mut rippled = false;
    // if we've already walked the hi branch...
    if cur.scope.var_get(tv) {
      cur.ascend();
      // if we've cleared the stack and already explored the hi branch...
      { let iv = cur.node.vid();
        if cur.nstack.is_empty() && cur.scope.var_get(iv) {
          // ... then first check if there are any variables above us on which
          // the node doesn't actually depend. ifso: ripple add. else: done.
          let top = cur.nvars-1;
          if cur.scope.ripple(iv.var_ix(), top).is_some() { rippled = true; }
          else { return None }}} }

    if rippled { cur.clear_trailing_bits() }
    else if cur.var_get() { return None }
    else { cur.put_step(self, true); }
    cur.descend(self);
    Some(cur.node) }

  /// walk depth-first from lo to hi until we arrive at the next solution
  pub fn next_solution(&self, mut cur:Cursor)->Option<Cursor> {
    assert!(cur.node.is_const(), "advance should always start by looking at a leaf");
    if self.in_solution(&cur) {
      // if we're in the solution, we're going to increment the "counter".
      if cur.increment().is_some() {
        // The 'zpos' variable exists in the solution space, but there might or might
        // not be a branch node for that variable in the current bdd path.
        // Whether we follow the hi or lo branch depends on which variable we're looking at.
        if cur.node.is_const() { return Some(cur) } // special case for topmost I (all solutions)
        cur.put_step(self, cur.var_get());
        cur.descend(self); }
      else { // overflow. we've counted all the way to 2^nvars-1, and we're done.
        return None }}
    // If still here, we are looking at a leaf that isn't a solution (out=0 in truth table)
    while !self.in_solution(&cur) { self.find_next_leaf(&mut cur)?; }
    self.mark_skippable(&mut cur);
    Some(cur) }

  fn mark_skippable(&self, cur: &mut Cursor) {
    let mut can_skip = Reg::new(cur.nvars);
    // iterate through the cursor's nid stack, checking each nid.vid_ix() to get its level.
    // any time there's a gap between the levels, mark that level as "don't care" by setting can_skip[i]=true.
    // We also need to include all the bits BELOW the current level any any bits above the top level.
    let mut prev = 0;
    // path from the top
    let path: Vec<usize> = cur.nstack.iter().map(|nid|nid.vid().var_ix()).collect();
    for (i,&level) in path.iter().rev().enumerate() {
      if i == 0 { for j in 0..level { can_skip.put(j, true);  }}
      else if level > prev + 1 {
        for j in (prev + 1)..level { can_skip.put(j, true); }}
      prev = level; }
    // skippable variables above the top level
    if !cur.nstack.is_empty() {
      for i in path[0]+1..cur.nvars { can_skip.put(i, true); }}
    cur.can_skip = can_skip; }

} // impl BddBase
