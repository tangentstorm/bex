//! Table NID operations and the `TableBase` decorator.
//!
//! Provides truth table alignment (expanding tables to a common variable set),
//! bitwise operations on table NIDs, and a `TableBase<T>` decorator that wraps
//! any `Base` to automatically collapse small subgraphs into table NIDs.
//!
//! **Bit ordering:** MSB-first, matching the NidFun / `select_bits` convention.
//! Entry i (where variable p has value `(i>>p)&1`) is stored at bit `(N-1-i)`
//! in the u32.  For 5 variables (N=32), entry 0 is bit 31 and entry 31 is bit 0.
//!
//! Variable signals (arity 5):
//!   x0 = 0x55555555  x1 = 0x33333333  x2 = 0x0F0F0F0F
//!   x3 = 0x00FF00FF  x4 = 0x0000FFFF

use crate::nid::{NID, NidFun, I, O};
use crate::vid::VID;
use crate::Fun;

/// Number of truth-table entries for the given arity.
#[inline] fn n_entries(arity: u8) -> u32 { 1u32 << arity }

/// Bitmask covering all truth-table bits for the given arity.
#[inline] fn tbl_mask(arity: u8) -> u32 {
  if arity >= 5 { 0xFFFFFFFF } else { (1u32 << n_entries(arity)) - 1 }}

// ---- stack-allocated small table ----

/// A truth table with its variable set, fully stack-allocated.
#[derive(Clone, Copy)]
struct SmallTbl {
  tbl: u32,
  vars: [u32; 5],
  arity: u8,
}

impl SmallTbl {
  #[inline] fn var_slice(&self) -> &[u32] { &self.vars[..self.arity as usize] }
}

/// Quick arity of a NID (0 for const, 1 for var, arity for fun, 6 for BDD nodes).
#[inline] fn quick_arity(n: NID) -> u8 {
  if n.is_const() { 0 }
  else if n.is_var() { 1 }   // real variables only, not virtual
  else if n.is_fun() { n.to_fun().unwrap().arity() }
  else { 6 }
}

/// Extract a SmallTbl from a NID.  Returns None for BDD nodes and virtual variables.
#[inline] fn nid_to_small(n: NID) -> Option<SmallTbl> {
  if n.is_const() {
    Some(SmallTbl { tbl: if n == I { 0xFFFFFFFF } else { 0 }, vars: [0;5], arity: 0 })
  } else if n.is_var() {   // real variables only
    let vi = n.vid().var_ix() as u32;
    let tbl = if n.is_inv() { 0b10u32 } else { 0b01u32 };
    Some(SmallTbl { tbl, vars: [vi, 0, 0, 0, 0], arity: 1 })
  } else if n.is_fun() {
    let f = n.to_fun().unwrap();
    let (ar, vars) = f.vars();
    let mut tbl = f.tbl();
    if n.is_inv() { tbl ^= tbl_mask(ar); }
    Some(SmallTbl { tbl, vars, arity: ar })
  } else { None }
}

// ---- truth table alignment (no heap allocation) ----

/// Insert a "don't care" variable at position `pos` in a truth table of `arity`
/// variables (MSB-first).  Returns the expanded table (arity+1 variables).
fn insert_var(tbl: u32, arity: u8, pos: u8) -> u32 {
  let n_old = n_entries(arity);
  let n_new = n_old << 1;
  let mut result: u32 = 0;
  for j in 0..n_new {
    let hi = (j >> (pos + 1)) << pos;
    let lo = j & ((1 << pos) - 1);
    let orig = hi | lo;
    let old_bit = (tbl >> (n_old - 1 - orig)) & 1;
    result |= old_bit << (n_new - 1 - j);
  }
  result
}

/// Expand a truth table from `src_vars` to `dst_vars` (MSB-first).
fn expand_table(tbl: u32, src_vars: &[u32], dst_vars: &[u32]) -> u32 {
  let mut result = tbl;
  let mut current_arity = src_vars.len() as u8;
  let mut src_idx = 0usize;
  for (dst_pos, &dv) in dst_vars.iter().enumerate() {
    if src_idx < src_vars.len() && src_vars[src_idx] == dv {
      src_idx += 1;
    } else {
      result = insert_var(result, current_arity, dst_pos as u8);
      current_arity += 1;
    }
  }
  result
}

/// Merge two sorted variable slices into a fixed-size array.
/// Returns (merged_vars, merged_len) or None if len > 5.
#[inline] fn merge_small(va: &[u32], vb: &[u32]) -> Option<([u32; 5], u8)> {
  let mut out = [0u32; 5];
  let mut len = 0u8;
  let (mut ia, mut ib) = (0, 0);
  while ia < va.len() && ib < vb.len() {
    if len >= 5 && va[ia] != vb[ib] { return None; }
    if va[ia] < vb[ib] { out[len as usize] = va[ia]; ia += 1; }
    else if va[ia] > vb[ib] { out[len as usize] = vb[ib]; ib += 1; }
    else { out[len as usize] = va[ia]; ia += 1; ib += 1; }
    len += 1;
  }
  while ia < va.len() { if len > 5 { return None; } out[len as usize] = va[ia]; ia += 1; len += 1; }
  while ib < vb.len() { if len > 5 { return None; } out[len as usize] = vb[ib]; ib += 1; len += 1; }
  if len > 5 { None } else { Some((out, len)) }
}

/// Align two SmallTbls to a common variable set.
fn align2(a: &SmallTbl, b: &SmallTbl) -> Option<(u32, u32, [u32; 5], u8)> {
  let (vars, len) = merge_small(a.var_slice(), b.var_slice())?;
  let dst = &vars[..len as usize];
  let ta = expand_table(a.tbl, a.var_slice(), dst);
  let tb = expand_table(b.tbl, b.var_slice(), dst);
  Some((ta, tb, vars, len))
}

/// Align three SmallTbls to a common variable set.
fn align3(a: &SmallTbl, b: &SmallTbl, c: &SmallTbl)
  -> Option<(u32, u32, u32, [u32; 5], u8)>
{
  // merge a+b first, then merge with c
  let (ab_vars, ab_len) = merge_small(a.var_slice(), b.var_slice())?;
  let (vars, len) = merge_small(&ab_vars[..ab_len as usize], c.var_slice())?;
  let dst = &vars[..len as usize];
  let ta = expand_table(a.tbl, a.var_slice(), dst);
  let tb = expand_table(b.tbl, b.var_slice(), dst);
  let tc = expand_table(c.tbl, c.var_slice(), dst);
  Some((ta, tb, tc, vars, len))
}

// ---- constructing result table NIDs from raw truth tables ----

/// Build a NID from a truth table (MSB-first) and variable set, normalising:
/// - all-zero → O,  all-ones → I
/// - single-variable → var NID (or !var)
/// - otherwise → table NID (with INV normalisation)
pub fn make_table_nid(tbl: u32, vars: &[u32]) -> NID {
  if vars.is_empty() {
    return if tbl & 1 == 0 { O } else { I }; }
  let (tbl, vars) = strip_unused_vars(tbl, vars);
  let arity = vars.len() as u8;
  let mask = tbl_mask(arity);
  let effective = tbl & mask;
  if effective == 0 { return O; }
  if effective == mask { return I; }
  if arity == 1 {
    let v = VID::var(vars[0]);
    return if effective == 0b01 { NID::from_vid(v) } else { !NID::from_vid(v) };
  }
  // INV normalisation: if f(0,0,…,0) = 1 (MSB is set), store ¬f and set INV.
  let n = n_entries(arity);
  if effective >> (n - 1) & 1 == 1 {
    let inv_tbl = !effective & mask;
    return !NID::fun_with_vars(&vars, inv_tbl).to_nid();
  }
  NID::fun_with_vars(&vars, effective).to_nid()
}

/// Convert a NidFun (from `Fun::when()`) + remaining variable set to NID.
pub fn nidfun_to_nid(f: &NidFun, vars: &[u32]) -> NID {
  make_table_nid(f.tbl(), vars)
}

// ---- degenerate variable detection (MSB-first) ----

fn var_is_unused(tbl: u32, arity: u8, pos: u8) -> bool {
  let n = n_entries(arity);
  for i in 0..n {
    if (i >> pos) & 1 == 0 {
      let j = i | (1 << pos);
      let vi = (tbl >> (n - 1 - i)) & 1;
      let vj = (tbl >> (n - 1 - j)) & 1;
      if vi != vj { return false; }
    }
  }
  true
}

fn remove_var(tbl: u32, arity: u8, pos: u8) -> u32 {
  let n_old = n_entries(arity);
  let n_new = n_old >> 1;
  let mut result: u32 = 0;
  let mut dst = 0u32;
  for i in 0..n_old {
    if (i >> pos) & 1 == 0 {
      let bit = (tbl >> (n_old - 1 - i)) & 1;
      result |= bit << (n_new - 1 - dst);
      dst += 1;
    }
  }
  result
}

fn strip_unused_vars(mut tbl: u32, vars: &[u32]) -> (u32, Vec<u32>) {
  let mut remaining: Vec<u32> = vars.to_vec();
  let mut pos = 0u8;
  while (pos as usize) < remaining.len() {
    if var_is_unused(tbl, remaining.len() as u8, pos) {
      tbl = remove_var(tbl, remaining.len() as u8, pos);
      remaining.remove(pos as usize);
    } else {
      pos += 1;
    }
  }
  (tbl, remaining)
}

// ---- table-level operations ----

/// Compute AND of two NIDs entirely via truth tables.
pub fn table_and(x: NID, y: NID) -> Option<NID> {
  if quick_arity(x) + quick_arity(y) > 5 { return None; }
  let a = nid_to_small(x)?;
  let b = nid_to_small(y)?;
  let (ta, tb, vars, len) = align2(&a, &b)?;
  Some(make_table_nid(ta & tb, &vars[..len as usize]))
}

/// Compute XOR of two NIDs entirely via truth tables.
pub fn table_xor(x: NID, y: NID) -> Option<NID> {
  if quick_arity(x) + quick_arity(y) > 5 { return None; }
  let a = nid_to_small(x)?;
  let b = nid_to_small(y)?;
  let (ta, tb, vars, len) = align2(&a, &b)?;
  Some(make_table_nid(ta ^ tb, &vars[..len as usize]))
}

/// Compute OR of two NIDs entirely via truth tables.
pub fn table_or(x: NID, y: NID) -> Option<NID> {
  if quick_arity(x) + quick_arity(y) > 5 { return None; }
  let a = nid_to_small(x)?;
  let b = nid_to_small(y)?;
  let (ta, tb, vars, len) = align2(&a, &b)?;
  Some(make_table_nid(ta | tb, &vars[..len as usize]))
}

/// Compute ITE(i, t, e) entirely via truth tables.
pub fn table_ite(i: NID, t: NID, e: NID) -> Option<NID> {
  if quick_arity(i) + quick_arity(t) + quick_arity(e) > 5 { return None; }
  let si = nid_to_small(i)?;
  let st = nid_to_small(t)?;
  let se = nid_to_small(e)?;
  let (ti, tt, te, vars, len) = align3(&si, &st, &se)?;
  Some(make_table_nid((ti & tt) | (!ti & te), &vars[..len as usize]))
}


// ---- public helpers for external callers (vhl.rs, bdd.rs) ----

/// Align two table NIDs to a common variable set.
/// Returns None if the combined variable count exceeds 5.
pub fn align_tables(a: &NidFun, b: &NidFun) -> Option<(u32, u32, Vec<u32>)> {
  let (ar_a, vars_a) = a.vars();
  let (ar_b, vars_b) = b.vars();
  let sa = SmallTbl { tbl: a.tbl(), vars: vars_a, arity: ar_a };
  let sb = SmallTbl { tbl: b.tbl(), vars: vars_b, arity: ar_b };
  let (ta, tb, vars, len) = align2(&sa, &sb)?;
  Some((ta, tb, vars[..len as usize].to_vec()))
}


// ---- TableBase decorator ----

/// A decorator around any `Base` that automatically computes operations
/// on table NIDs (and constants/variables) without entering the BDD pipeline.
pub struct TableBase<T: crate::base::Base> { pub base: T }

impl<T: crate::base::Base> crate::base::Base for TableBase<T> {
  crate::inherit![new, def, tag, get, sub, dot];

  fn when_hi(&mut self, v:VID, n:NID)->NID {
    if n.is_fun() {
      if let Some(f) = n.to_fun() {
        if let Some(pos) = f.var_position(v) {
          let reduced = f.when(pos, true);
          let (ar, vs) = f.vars();
          let mut new_vars: Vec<u32> = vs[..ar as usize].to_vec();
          new_vars.remove(pos as usize);
          let result = nidfun_to_nid(&reduced, &new_vars);
          return if n.is_inv() { !result } else { result };
        } else { return n; }}}
    self.base.when_hi(v, n)
  }

  fn when_lo(&mut self, v:VID, n:NID)->NID {
    if n.is_fun() {
      if let Some(f) = n.to_fun() {
        if let Some(pos) = f.var_position(v) {
          let reduced = f.when(pos, false);
          let (ar, vs) = f.vars();
          let mut new_vars: Vec<u32> = vs[..ar as usize].to_vec();
          new_vars.remove(pos as usize);
          let result = nidfun_to_nid(&reduced, &new_vars);
          return if n.is_inv() { !result } else { result };
        } else { return n; }}}
    self.base.when_lo(v, n)
  }

  fn and(&mut self, x:NID, y:NID)->NID {
    if let Some(r) = table_and(x, y) { return r; }
    self.base.and(x, y)
  }

  fn xor(&mut self, x:NID, y:NID)->NID {
    if let Some(r) = table_xor(x, y) { return r; }
    self.base.xor(x, y)
  }

  fn or(&mut self, x:NID, y:NID)->NID {
    if let Some(r) = table_or(x, y) { return r; }
    self.base.or(x, y)
  }

  fn ite(&mut self, i:NID, t:NID, e:NID)->NID {
    if let Some(r) = table_ite(i, t, e) { return r; }
    self.base.ite(i, t, e)
  }
}


#[cfg(test)]
mod tests {
  use super::*;
  use crate::nid::named::*;

  /// Evaluate a NID on a specific input assignment (MSB-first convention).
  fn eval_nid(n: NID, vars_vals: &[(u32, bool)]) -> bool {
    let s = nid_to_small(n).expect("eval_nid: not a table/const/var NID");
    let nn = n_entries(s.arity);
    let mut row = 0u32;
    for (pos, &v) in s.var_slice().iter().enumerate() {
      for &(vi, val) in vars_vals {
        if vi == v && val { row |= 1 << pos; }
      }
    }
    (s.tbl >> (nn - 1 - row)) & 1 == 1
  }

  #[test]
  fn test_insert_var() {
    // f(a) = identity.  MSB-first arity 1: 0b01
    // Insert don't-care at position 0 → arity 2, a moves to position 1.
    // Signal for position 1 is 0b0011.
    assert_eq!(insert_var(0b01, 1, 0), 0b0011);
    // Insert don't-care at position 1 → a stays at position 0.
    // Signal for position 0 is 0b0101.
    assert_eq!(insert_var(0b01, 1, 1), 0b0101);
  }

  #[test]
  fn test_expand_table() {
    // f(x1) = x1, identity = 0b01 (MSB-first arity 1)
    // Expand to {x0, x1}: insert x0 at position 0.
    // x1 signal over {x0,x1} is 0b0011.
    assert_eq!(expand_table(0b01, &[1], &[0, 1]), 0b0011);

    // f(x0) = x0, identity = 0b01
    // Expand to {x0, x1}: insert x1 at position 1.
    // x0 signal over {x0,x1} is 0b0101.
    assert_eq!(expand_table(0b01, &[0], &[0, 1]), 0b0101);
  }

  #[test]
  fn test_table_and_two_vars() {
    let result = table_and(x0, x1).unwrap();
    assert!(!eval_nid(result, &[(0,false),(1,false)]));
    assert!(!eval_nid(result, &[(0,true),(1,false)]));
    assert!(!eval_nid(result, &[(0,false),(1,true)]));
    assert!( eval_nid(result, &[(0,true),(1,true)]));
    // also check the stored table directly (MSB-first AND = 0b0001)
    let f = result.raw().to_fun().unwrap();
    assert_eq!(f.tbl(), 0b0001);
  }

  #[test]
  fn test_table_or_two_vars() {
    let result = table_or(x0, x1).unwrap();
    assert!(!eval_nid(result, &[(0,false),(1,false)]));
    assert!( eval_nid(result, &[(0,true),(1,false)]));
    assert!( eval_nid(result, &[(0,false),(1,true)]));
    assert!( eval_nid(result, &[(0,true),(1,true)]));
  }

  #[test]
  fn test_table_xor_two_vars() {
    let result = table_xor(x0, x1).unwrap();
    assert!(!eval_nid(result, &[(0,false),(1,false)]));
    assert!( eval_nid(result, &[(0,true),(1,false)]));
    assert!( eval_nid(result, &[(0,false),(1,true)]));
    assert!(!eval_nid(result, &[(0,true),(1,true)]));
    // MSB-first XOR = 0b0110
    let f = result.raw().to_fun().unwrap();
    assert_eq!(f.tbl(), 0b0110);
  }

  #[test]
  fn test_table_and_with_const() {
    assert_eq!(table_and(x0, I).unwrap(), x0);
    assert_eq!(table_and(x0, O).unwrap(), O);
    assert_eq!(table_and(I, x1).unwrap(), x1);
  }

  #[test]
  fn test_table_xor_same_var() {
    assert_eq!(table_xor(x0, x0).unwrap(), O);
  }

  #[test]
  fn test_table_and_disjoint_vars() {
    let a01 = table_and(x0, x1).unwrap();
    let a23 = table_and(x2, x3).unwrap();
    let result = table_and(a01, a23).unwrap();
    assert!( eval_nid(result, &[(0,true),(1,true),(2,true),(3,true)]));
    assert!(!eval_nid(result, &[(0,true),(1,true),(2,true),(3,false)]));
    assert!(!eval_nid(result, &[(0,false),(1,true),(2,true),(3,true)]));
  }

  #[test]
  fn test_table_exceeds_5_vars() {
    let a = table_and(x0, table_and(x1, x2).unwrap()).unwrap();
    let b = table_and(x3, table_and(x4, NID::var(5)).unwrap()).unwrap();
    assert!(table_and(a, b).is_none(), "should fail: 6 vars > 5");
  }

  #[test]
  fn test_table_ite() {
    let result = table_ite(x0, x1, x2).unwrap();
    assert!( eval_nid(result, &[(0,true),(1,true),(2,false)]));  // take x1=T
    assert!(!eval_nid(result, &[(0,true),(1,false),(2,true)]));  // take x1=F
    assert!( eval_nid(result, &[(0,false),(1,false),(2,true)])); // take x2=T
    assert!(!eval_nid(result, &[(0,false),(1,true),(2,false)])); // take x2=F
  }

  #[test]
  fn test_table_and_overlapping_vars() {
    // OR(x1,x3) = 0b0111, AND(x2,x3) = 0b0001  (MSB-first)
    // result simplifies to AND(x2,x3), since x1 drops out
    let f = NID::fun_with_vars(&[1, 3], 0b0111).to_nid();
    let g = NID::fun_with_vars(&[2, 3], 0b0001).to_nid();
    let result = table_and(f, g).unwrap();
    assert!( eval_nid(result, &[(2,true),(3,true)]));
    assert!(!eval_nid(result, &[(2,true),(3,false)]));
    assert!(!eval_nid(result, &[(2,false),(3,true)]));
  }

  #[test]
  fn test_make_table_nid_degenerate() {
    assert_eq!(make_table_nid(0, &[0, 1]), O);
    assert_eq!(make_table_nid(0b1111, &[0, 1]), I);
    // MSB-first arity-1 identity = 0b01
    assert_eq!(make_table_nid(0b01, &[3]), NID::var(3));
    assert_eq!(make_table_nid(0b10, &[3]), !NID::var(3));
  }

  #[test]
  fn test_strip_unused_vars() {
    // AND = 0b0001 over {x0,x1}: depends on both → no change
    let (tbl, vars) = strip_unused_vars(0b0001, &[0, 1]);
    assert_eq!(vars, vec![0, 1]);
    assert_eq!(tbl, 0b0001);

    // x0 signal = 0b0101 over {x0,x1}: doesn't depend on x1
    let (tbl, vars) = strip_unused_vars(0b0101, &[0, 1]);
    assert_eq!(vars, vec![0]);
    assert_eq!(tbl, 0b01);

    // x1 signal = 0b0011 over {x0,x1}: doesn't depend on x0
    let (tbl, vars) = strip_unused_vars(0b0011, &[0, 1]);
    assert_eq!(vars, vec![1]);
    assert_eq!(tbl, 0b01);
  }

  #[test]
  fn test_make_table_nid_removes_unused() {
    // x0 signal = 0b0101 over {x0,x1}: only depends on x0
    let n = make_table_nid(0b0101, &[0, 1]);
    assert_eq!(n, x0);
  }

  #[test]
  fn test_inv_normalization() {
    // OR(x0,x1) = 0b0111. f(0,0)=0 (MSB=0), so no INV needed.
    let n_or = make_table_nid(0b0111, &[0, 1]);
    assert!(!n_or.is_inv());
    // NAND(x0,x1) = 0b1110.  f(0,0)=1 (MSB=1), so store ¬f = AND = 0b0001 with INV.
    let n_nand = make_table_nid(0b1110, &[0, 1]);
    assert!(n_nand.is_inv(), "NAND should be stored inverted");
    let f = n_nand.raw().to_fun().unwrap();
    assert_eq!(f.tbl(), 0b0001, "underlying function should be AND");
  }

  #[test]
  fn test_table_xor_degenerate_result() {
    assert_eq!(table_xor(x0, x0).unwrap(), O);
    let a = table_and(x0, x1).unwrap();
    assert_eq!(table_xor(a, a).unwrap(), O);
  }

  #[test]
  fn test_early_bailout() {
    // BDD nodes (not const/var/fun) should return None instantly
    let bdd_nid = NID::from_vid_idx(VID::var(0), 42);
    assert!(table_and(bdd_nid, x0).is_none());
    assert!(table_ite(bdd_nid, x0, x1).is_none());

    // Arity sum > 5 should bail before any real work
    let big = table_and(x0, table_and(x1, x2).unwrap()).unwrap(); // arity 3
    let big2 = table_and(x3, table_and(x4, NID::var(5)).unwrap()).unwrap(); // arity 3
    // 3 + 3 = 6 > 5, bails at the arity check
    assert!(table_and(big, big2).is_none());
  }
}
