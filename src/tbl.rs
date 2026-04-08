//! Table NID operations and the `TableBase` decorator.
//!
//! Provides truth table alignment (expanding tables to a common variable set),
//! bitwise operations on table NIDs, and a `TableBase<T>` decorator that wraps
//! any `Base` to automatically collapse small subgraphs into table NIDs.

use crate::nid::{NID, NidFun, I, O};
use crate::vid::VID;
use crate::Fun;

// ---- truth table alignment ----

/// Insert a "don't care" variable at position `pos` in a truth table of `arity` variables.
/// The new variable does not affect the output: each block of 2^pos entries is duplicated.
/// Returns the expanded table (arity+1 variables).
fn insert_var(tbl: u32, arity: u8, pos: u8) -> u32 {
  let block = 1u32 << pos;        // size of each block
  let n = 1u32 << arity;          // number of entries in source table
  let mut result: u32 = 0;
  let mut src_bit = 0u32;
  let mut dst_bit = 0u32;
  while src_bit < n {
    // Copy a block of `block` bits, then duplicate it
    for _ in 0..block {
      let bit_val = (tbl >> src_bit) & 1;
      result |= bit_val << dst_bit;
      result |= bit_val << (dst_bit + block);
      src_bit += 1;
      dst_bit += 1;
    }
    dst_bit += block; // skip past the duplicated block
  }
  result
}

/// Expand a truth table's variable set to a target variable set.
/// `src_vars` must be a subset of `dst_vars` (both sorted ascending).
/// Returns the expanded truth table over `dst_vars`.
fn expand_table(tbl: u32, src_vars: &[u32], dst_vars: &[u32]) -> u32 {
  let mut result = tbl;
  let mut current_arity = src_vars.len() as u8;
  let mut src_idx = 0usize;
  for (dst_pos, &dv) in dst_vars.iter().enumerate() {
    if src_idx < src_vars.len() && src_vars[src_idx] == dv {
      src_idx += 1; // variable already present, skip
    } else {
      // insert don't-care at this position
      result = insert_var(result, current_arity, dst_pos as u8);
      current_arity += 1;
    }
  }
  result
}

/// Align two table NIDs to a common variable set.
/// Returns None if the combined variable count exceeds 5.
/// Otherwise returns (expanded_tbl_a, expanded_tbl_b, combined_vars).
pub fn align_tables(a: &NidFun, b: &NidFun) -> Option<(u32, u32, Vec<u32>)> {
  let (ar_a, vars_a) = a.vars();
  let (ar_b, vars_b) = b.vars();
  let va = &vars_a[..ar_a as usize];
  let vb = &vars_b[..ar_b as usize];
  // Merge sorted variable lists
  let mut combined = Vec::with_capacity(5);
  let (mut ia, mut ib) = (0, 0);
  while ia < va.len() && ib < vb.len() {
    if va[ia] < vb[ib] { combined.push(va[ia]); ia += 1; }
    else if va[ia] > vb[ib] { combined.push(vb[ib]); ib += 1; }
    else { combined.push(va[ia]); ia += 1; ib += 1; }
  }
  while ia < va.len() { combined.push(va[ia]); ia += 1; }
  while ib < vb.len() { combined.push(vb[ib]); ib += 1; }
  if combined.len() > 5 { return None; }
  let ta = expand_table(a.tbl(), va, &combined);
  let tb = expand_table(b.tbl(), vb, &combined);
  Some((ta, tb, combined))
}

// ---- constructing result table NIDs from raw truth tables ----

/// Build a NID from a truth table and variable set, normalizing edge cases:
/// - all-zero -> O, all-ones -> I
/// - single-variable -> var NID or !var NID
/// - otherwise -> table NID
pub fn make_table_nid(tbl: u32, vars: &[u32]) -> NID {
  let arity = vars.len();
  let mask = if arity >= 5 { 0xFFFFFFFFu32 } else { (1u32 << (1u32 << arity)) - 1 };
  let effective = tbl & mask;
  // constant?
  if effective == 0 { return O; }
  if effective == mask { return I; }
  // single variable?
  if arity == 1 {
    let v = VID::var(vars[0]);
    let var_pattern = 0b10u32; // truth table for identity: f(x)=x
    if effective == var_pattern { return NID::from_vid(v); }
    else { return !NID::from_vid(v); }
  }
  // general table NID
  NID::fun_with_vars(vars, effective).to_nid()
}

/// Convert a NidFun (from a Fun::when() result) + remaining variable set to the correct NID.
/// Handles arity 0 -> const, arity 1 -> variable, arity 2+ -> table NID.
pub fn nidfun_to_nid(f: &NidFun, vars: &[u32]) -> NID {
  make_table_nid(f.tbl(), vars)
}

// ---- promoting consts/vars to truth tables for alignment ----

/// Get the truth table and variable set for any "small" NID
/// (constant, variable, or table NID). Returns None for BDD nodes.
fn nid_to_table(n: NID) -> Option<(u32, Vec<u32>)> {
  if n.is_const() {
    Some((if n == I { 0xFFFFFFFF } else { 0 }, vec![]))
  } else if n.is_var() {
    let vi = n.vid().var_ix() as u32;
    let tbl = if n.is_inv() { 0b01u32 } else { 0b10u32 };
    Some((tbl, vec![vi]))
  } else if n.is_fun() {
    let f = n.to_fun().unwrap();
    let (ar, vars) = f.vars();
    let mut tbl = f.tbl();
    if n.is_inv() {
      let mask = if ar >= 5 { 0xFFFFFFFFu32 } else { (1u32 << (1u32 << ar)) - 1 };
      tbl ^= mask;
    }
    Some((tbl, vars[..ar as usize].to_vec()))
  } else {
    None
  }
}

/// Try to align two NIDs (each may be const, var, or table) to a common variable set.
/// Returns None if either is a BDD node or combined vars > 5.
fn align_nids(a: NID, b: NID) -> Option<(u32, u32, Vec<u32>)> {
  let (ta, va) = nid_to_table(a)?;
  let (tb, vb) = nid_to_table(b)?;
  // Merge
  let mut combined = Vec::with_capacity(5);
  let (mut ia, mut ib) = (0, 0);
  while ia < va.len() && ib < vb.len() {
    if va[ia] < vb[ib] { combined.push(va[ia]); ia += 1; }
    else if va[ia] > vb[ib] { combined.push(vb[ib]); ib += 1; }
    else { combined.push(va[ia]); ia += 1; ib += 1; }
  }
  while ia < va.len() { combined.push(va[ia]); ia += 1; }
  while ib < vb.len() { combined.push(vb[ib]); ib += 1; }
  if combined.len() > 5 { return None; }
  let ea = expand_table(ta, &va, &combined);
  let eb = expand_table(tb, &vb, &combined);
  Some((ea, eb, combined))
}

/// Try to align three NIDs to a common variable set.
fn align_three(i: NID, t: NID, e: NID) -> Option<(u32, u32, u32, Vec<u32>)> {
  let (ti, vi) = nid_to_table(i)?;
  let (tt, vt) = nid_to_table(t)?;
  let (te, ve) = nid_to_table(e)?;
  // Merge three sorted lists
  let mut combined = Vec::with_capacity(5);
  let (mut a, mut b, mut c) = (0, 0, 0);
  loop {
    let va = if a < vi.len() { Some(vi[a]) } else { None };
    let vb = if b < vt.len() { Some(vt[b]) } else { None };
    let vc = if c < ve.len() { Some(ve[c]) } else { None };
    match (va, vb, vc) {
      (None, None, None) => break,
      _ => {
        let min = [va, vb, vc].iter().copied().flatten().min().unwrap();
        combined.push(min);
        if va == Some(min) { a += 1; }
        if vb == Some(min) { b += 1; }
        if vc == Some(min) { c += 1; }
      }
    }
  }
  if combined.len() > 5 { return None; }
  let ei = expand_table(ti, &vi, &combined);
  let et = expand_table(tt, &vt, &combined);
  let ee = expand_table(te, &ve, &combined);
  Some((ei, et, ee, combined))
}

// ---- table-level operations ----

/// Compute AND of two NIDs entirely via truth tables.
/// Returns None if either operand is a BDD node or combined vars > 5.
pub fn table_and(x: NID, y: NID) -> Option<NID> {
  let (tx, ty, vars) = align_nids(x, y)?;
  Some(make_table_nid(tx & ty, &vars))
}

/// Compute XOR of two NIDs entirely via truth tables.
pub fn table_xor(x: NID, y: NID) -> Option<NID> {
  let (tx, ty, vars) = align_nids(x, y)?;
  Some(make_table_nid(tx ^ ty, &vars))
}

/// Compute OR of two NIDs entirely via truth tables.
pub fn table_or(x: NID, y: NID) -> Option<NID> {
  let (tx, ty, vars) = align_nids(x, y)?;
  Some(make_table_nid(tx | ty, &vars))
}

/// Compute ITE(i, t, e) entirely via truth tables.
pub fn table_ite(i: NID, t: NID, e: NID) -> Option<NID> {
  let (ti, tt, te, vars) = align_three(i, t, e)?;
  Some(make_table_nid((ti & tt) | (!ti & te), &vars))
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
          let result = make_table_nid(reduced.tbl(), &new_vars);
          return if n.is_inv() { !result } else { result };
        } else {
          return n; // variable not in set, no change
        }
      }
    }
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
          let result = make_table_nid(reduced.tbl(), &new_vars);
          return if n.is_inv() { !result } else { result };
        } else {
          return n;
        }
      }
    }
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

  #[test]
  fn test_insert_var() {
    // f(a) = a (truth table 0b10) -> insert don't-care at position 0
    // Position 0 is fastest-alternating. Original var shifts to position 1.
    // New table: bit0(new=0,a=0)=0, bit1(new=1,a=0)=0, bit2(new=0,a=1)=1, bit3(new=1,a=1)=1
    assert_eq!(insert_var(0b10, 1, 0), 0b1100);
    // insert don't-care at position 1 (slower). Original var stays at position 0.
    // bit0(a=0,new=0)=0, bit1(a=1,new=0)=1, bit2(a=0,new=1)=0, bit3(a=1,new=1)=1
    assert_eq!(insert_var(0b10, 1, 1), 0b1010);
  }

  #[test]
  fn test_expand_table() {
    // f(x1) = x1 (table 0b10) with vars [1]
    // expand to [0, 1]: insert x0 at position 0
    let expanded = expand_table(0b10, &[1], &[0, 1]);
    // Result should be x1 over {x0, x1}: 0b1100
    assert_eq!(expanded, 0b1100);

    // f(x0) = x0 (table 0b10) with vars [0]
    // expand to [0, 1]: insert x1 at position 1
    let expanded2 = expand_table(0b10, &[0], &[0, 1]);
    // Result should be x0 over {x0, x1}: 0b1010
    assert_eq!(expanded2, 0b1010);
  }

  #[test]
  fn test_table_and_two_vars() {
    // x0 AND x1: both are simple variables
    let result = table_and(x0, x1).unwrap();
    assert!(result.is_fun());
    let f = result.to_fun().unwrap();
    assert_eq!(f.arity(), 2);
    // AND truth table: 0b1000 (row order: 00->0, 01->0, 10->0, 11->1)
    assert_eq!(f.tbl(), 0b1000);
  }

  #[test]
  fn test_table_or_two_vars() {
    let result = table_or(x0, x1).unwrap();
    let f = result.to_fun().unwrap();
    assert_eq!(f.arity(), 2);
    assert_eq!(f.tbl(), 0b1110);
  }

  #[test]
  fn test_table_xor_two_vars() {
    let result = table_xor(x0, x1).unwrap();
    let f = result.to_fun().unwrap();
    assert_eq!(f.arity(), 2);
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
    // (x0 AND x1) AND (x2 AND x3) -> should produce a 4-variable table
    let a01 = table_and(x0, x1).unwrap();
    let a23 = table_and(x2, x3).unwrap();
    let result = table_and(a01, a23).unwrap();
    let f = result.to_fun().unwrap();
    assert_eq!(f.arity(), 4);
    let (_, vs) = f.vars();
    assert_eq!(&vs[..4], &[0, 1, 2, 3]);
  }

  #[test]
  fn test_table_exceeds_5_vars() {
    // Create functions on 3 disjoint vars each -> combined = 6 > 5
    let a = table_and(x0, table_and(x1, x2).unwrap()).unwrap();
    let b = table_and(x3, table_and(x4, NID::var(5)).unwrap()).unwrap();
    assert!(table_and(a, b).is_none(), "should fail: 6 vars > 5");
  }

  #[test]
  fn test_table_ite() {
    // ite(x0, x1, x2) = if x0 then x1 else x2
    // Signals: x0=0xAA, x1=0xCC, x2=0xF0 (for 3-var 8-bit table)
    // (x0 & x1) | (~x0 & x2) = 0x88 | 0x50 = 0xD8 = 0b11011000
    let result = table_ite(x0, x1, x2).unwrap();
    let f = result.to_fun().unwrap();
    assert_eq!(f.arity(), 3);
    assert_eq!(f.tbl(), 0b11011000);
  }

  #[test]
  fn test_table_and_overlapping_vars() {
    // f(x1, x3) AND g(x2, x3) -> combined {x1, x2, x3}, 3 vars
    let f = NID::fun_with_vars(&[1, 3], 0b1110).to_nid(); // x1 OR x3
    let g = NID::fun_with_vars(&[2, 3], 0b1000).to_nid(); // x2 AND x3
    let result = table_and(f, g).unwrap();
    let rf = result.to_fun().unwrap();
    assert_eq!(rf.arity(), 3);
    let (_, vs) = rf.vars();
    assert_eq!(&vs[..3], &[1, 2, 3]);
  }

  #[test]
  fn test_make_table_nid_degenerate() {
    // all-zero -> O
    assert_eq!(make_table_nid(0, &[0, 1]), O);
    // all-ones -> I
    assert_eq!(make_table_nid(0b1111, &[0, 1]), I);
    // single var identity
    assert_eq!(make_table_nid(0b10, &[3]), NID::var(3));
    // single var inverted
    assert_eq!(make_table_nid(0b01, &[3]), !NID::var(3));
  }
}
