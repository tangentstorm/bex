//! Combinatorial number system encoding for variable subsets.
//!
//! Encodes a k-element subset of {0..MAX_VAR-1} as a single integer
//! that fits in 27 bits. Used by table NIDs to specify which variables
//! a truth table depends on.
//!
//! The 27-bit space is partitioned by arity using an offset scheme:
//!   arity 2: [0, C(N,2))
//!   arity 3: [C(N,2), C(N,2)+C(N,3))
//!   arity 4: [C(N,2)+C(N,3), C(N,2)+C(N,3)+C(N,4))
//!   arity 5: [C(N,2)+..+C(N,4), C(N,2)+..+C(N,5))
//!
//! Arity is recovered by checking which range the index falls into.

/// Maximum variable index (exclusive). Variables are 0..MAX_VAR-1.
pub const MAX_VAR: usize = 110;

/// Maximum number of variables in a table NID.
pub const MAX_ARITY: u8 = 5;

/// Precomputed Pascal's triangle: PASCAL[n][k] = C(n,k) for n <= MAX_VAR, k <= 5.
/// We only need k up to 5, so this is a small table.
pub const PASCAL: [[u32; 6]; MAX_VAR + 1] = {
  let mut t = [[0u32; 6]; MAX_VAR + 1];
  let mut n = 0;
  while n <= MAX_VAR {
    t[n][0] = 1;
    let mut k = 1;
    while k <= 5 && k <= n {
      t[n][k] = t[n-1][k-1] + t[n-1][k];
      k += 1;
    }
    n += 1;
  }
  t
};

/// C(n,k) via the precomputed table. Returns 0 if k > n or k > 5.
#[inline]
pub const fn choose(n: usize, k: usize) -> u32 {
  if k > 5 || k > n { 0 } else { PASCAL[n][k] }
}

/// Offset for each arity in the combined index space.
/// OFFSET[k] is the start of the range for arity k.
/// Arities 0 and 1 are included for internal use by the Fun trait,
/// even though they should eventually be normalized to consts/vars.
pub const OFFSET: [u32; 6] = {
  let o0: u32 = 0;                           // arity 0: 1 entry (the "no variables" case)
  let o1: u32 = o0 + 1;                      // arity 1: MAX_VAR entries
  let o2: u32 = o1 + MAX_VAR as u32;         // arity 2: C(MAX_VAR, 2) entries
  let o3: u32 = o2 + PASCAL[MAX_VAR][2];
  let o4: u32 = o3 + PASCAL[MAX_VAR][3];
  let o5: u32 = o4 + PASCAL[MAX_VAR][4];
  [o0, o1, o2, o3, o4, o5]
};

/// Total number of valid indices (must fit in 27 bits).
pub const TOTAL: u32 = OFFSET[5] + PASCAL[MAX_VAR][5];

/// Encode a sorted slice of variable indices into a combined 27-bit index.
/// `vars` must be sorted ascending, with length 2..=5, and all values < MAX_VAR.
///
/// Uses the combinatorial number system:
///   index = C(c_0, 1) + C(c_1, 2) + ... + C(c_{k-1}, k)
/// then adds the arity offset.
pub fn encode(vars: &[u32]) -> u32 {
  let k = vars.len();
  debug_assert!(k <= 5, "arity must be 0..5, got {}", k);
  debug_assert!(vars.windows(2).all(|w| w[0] < w[1]), "vars must be strictly sorted ascending");
  debug_assert!(vars.iter().all(|&v| (v as usize) < MAX_VAR), "variable index out of range");
  if k == 0 { return OFFSET[0]; }
  if k == 1 { return OFFSET[1] + vars[0]; }
  let mut idx: u32 = 0;
  for (i, &c) in vars.iter().enumerate() {
    idx += PASCAL[c as usize][i + 1];
  }
  idx + OFFSET[k]
}

/// Decode a combined 27-bit index into (arity, variable indices).
/// Returns (arity, vars) where vars is a fixed-size array with
/// the first `arity` elements populated (rest are 0).
pub fn decode(index: u32) -> (u8, [u32; 5]) {
  let (arity, combinadic) = split(index);
  let k = arity as usize;
  let mut vars = [0u32; 5];
  if k == 0 { return (0, vars); }
  if k == 1 { vars[0] = combinadic; return (1, vars); }
  let mut remaining = combinadic;
  // Decode from highest position down
  for i in (0..k).rev() {
    // Find largest c such that C(c, i+1) <= remaining
    let rank = i + 1;
    let mut c: usize = 0;
    while c + 1 < MAX_VAR && PASCAL[c + 1][rank] <= remaining {
      c += 1;
    }
    vars[i] = c as u32;
    remaining -= PASCAL[c][rank];
  }
  debug_assert_eq!(remaining, 0, "decode error: leftover {}", remaining);
  (arity, vars)
}

/// Split a combined index into (arity, combinadic within that arity's range).
#[inline]
fn split(index: u32) -> (u8, u32) {
  if index < OFFSET[1] { (0, index - OFFSET[0]) }
  else if index < OFFSET[2] { (1, index - OFFSET[1]) }
  else if index < OFFSET[3] { (2, index - OFFSET[2]) }
  else if index < OFFSET[4] { (3, index - OFFSET[3]) }
  else if index < OFFSET[5] { (4, index - OFFSET[4]) }
  else { (5, index - OFFSET[5]) }
}

/// Fast arity extraction from a combined index.
#[inline]
pub fn arity_of(index: u32) -> u8 {
  split(index).0
}

/// Extract the top (highest-index) variable without a full decode.
/// This is performance-critical as it is called from NID::vid().
pub fn top_var_of(index: u32) -> u32 {
  let (arity, combinadic) = split(index);
  let k = arity as usize;
  if k == 0 { return 0; } // no variables; return 0 as placeholder
  if k == 1 { return combinadic; } // arity 1: combinadic IS the variable index
  // The top variable c_{k-1} is the largest c such that C(c, k) <= combinadic
  let mut c: usize = 0;
  while c + 1 < MAX_VAR && PASCAL[c + 1][k] <= combinadic {
    c += 1;
  }
  c as u32
}


#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_pascal() {
    assert_eq!(choose(0, 0), 1);
    assert_eq!(choose(5, 0), 1);
    assert_eq!(choose(5, 1), 5);
    assert_eq!(choose(5, 2), 10);
    assert_eq!(choose(5, 3), 10);
    assert_eq!(choose(5, 5), 1);
    assert_eq!(choose(10, 3), 120);
    assert_eq!(choose(MAX_VAR, 2), 5995);
    assert_eq!(choose(MAX_VAR, 5), 122_391_522);
  }

  #[test]
  fn test_total_fits_27_bits() {
    assert!(TOTAL < (1 << 27), "total {} must fit in 27 bits (max {})", TOTAL, 1u32 << 27);
  }

  #[test]
  fn test_offsets() {
    assert_eq!(OFFSET[0], 0);
    assert_eq!(OFFSET[1], 1);
    assert_eq!(OFFSET[2], 1 + MAX_VAR as u32);
    assert_eq!(OFFSET[3], OFFSET[2] + 5995);
    assert_eq!(OFFSET[4], OFFSET[3] + 215_820);
    assert_eq!(OFFSET[5], OFFSET[4] + 5_773_185);
  }

  #[test]
  fn test_encode_decode_roundtrip_arity2() {
    let vars = [3, 7];
    let idx = encode(&vars);
    let (arity, decoded) = decode(idx);
    assert_eq!(arity, 2);
    assert_eq!(&decoded[..2], &vars);
  }

  #[test]
  fn test_encode_decode_roundtrip_arity3() {
    let vars = [1, 5, 9];
    let idx = encode(&vars);
    let (arity, decoded) = decode(idx);
    assert_eq!(arity, 3);
    assert_eq!(&decoded[..3], &vars);
  }

  #[test]
  fn test_encode_decode_roundtrip_arity4() {
    let vars = [0, 2, 4, 6];
    let idx = encode(&vars);
    let (arity, decoded) = decode(idx);
    assert_eq!(arity, 4);
    assert_eq!(&decoded[..4], &vars);
  }

  #[test]
  fn test_encode_decode_roundtrip_arity5() {
    let vars = [0, 1, 2, 3, 4];
    let idx = encode(&vars);
    let (arity, decoded) = decode(idx);
    assert_eq!(arity, 5);
    assert_eq!(&decoded[..5], &vars);
  }

  #[test]
  fn test_encode_decode_max_vars() {
    // largest possible variable indices for arity 5
    let vars = [105, 106, 107, 108, 109];
    let idx = encode(&vars);
    assert!(idx < (1 << 27));
    let (arity, decoded) = decode(idx);
    assert_eq!(arity, 5);
    assert_eq!(&decoded[..5], &vars);
  }

  #[test]
  fn test_top_var_of() {
    let vars = [3, 7, 42];
    let idx = encode(&vars);
    assert_eq!(top_var_of(idx), 42);

    let vars2 = [0, 109];
    let idx2 = encode(&vars2);
    assert_eq!(top_var_of(idx2), 109);

    let vars3 = [0, 1, 2, 3, 4];
    let idx3 = encode(&vars3);
    assert_eq!(top_var_of(idx3), 4);
  }

  #[test]
  fn test_arity_of() {
    assert_eq!(arity_of(encode(&[0, 1])), 2);
    assert_eq!(arity_of(encode(&[0, 1, 2])), 3);
    assert_eq!(arity_of(encode(&[0, 1, 2, 3])), 4);
    assert_eq!(arity_of(encode(&[0, 1, 2, 3, 4])), 5);
  }

  #[test]
  fn test_exhaustive_arity2_small() {
    // encode and decode all 2-element subsets of {0..9}
    for a in 0u32..10 {
      for b in (a+1)..10 {
        let vars = [a, b];
        let idx = encode(&vars);
        let (arity, decoded) = decode(idx);
        assert_eq!(arity, 2, "failed for {:?}", vars);
        assert_eq!(&decoded[..2], &vars, "roundtrip failed for {:?}", vars);
      }
    }
  }

  #[test]
  fn test_exhaustive_arity3_small() {
    for a in 0u32..8 {
      for b in (a+1)..9 {
        for c in (b+1)..10 {
          let vars = [a, b, c];
          let idx = encode(&vars);
          let (arity, decoded) = decode(idx);
          assert_eq!(arity, 3, "failed for {:?}", vars);
          assert_eq!(&decoded[..3], &vars, "roundtrip failed for {:?}", vars);
        }
      }
    }
  }

  #[test]
  fn test_exhaustive_arity5_tiny() {
    // all 5-element subsets of {0..8}: C(9,5) = 126
    for a in 0u32..5 {
      for b in (a+1)..6 {
        for c in (b+1)..7 {
          for d in (c+1)..8 {
            for e in (d+1)..9 {
              let vars = [a, b, c, d, e];
              let idx = encode(&vars);
              let (arity, decoded) = decode(idx);
              assert_eq!(arity, 5, "failed for {:?}", vars);
              assert_eq!(&decoded[..5], &vars, "roundtrip failed for {:?}", vars);
            }
          }
        }
      }
    }
  }

  #[test]
  fn test_no_range_overlap() {
    // The maximum index for each arity should be below the offset for the next
    let max2 = encode(&[108, 109]);
    let min3 = encode(&[0, 1, 2]);
    assert!(max2 < min3, "arity 2 max {} overlaps arity 3 min {}", max2, min3);

    let max3 = encode(&[107, 108, 109]);
    let min4 = encode(&[0, 1, 2, 3]);
    assert!(max3 < min4, "arity 3 max {} overlaps arity 4 min {}", max3, min4);

    let max4 = encode(&[106, 107, 108, 109]);
    let min5 = encode(&[0, 1, 2, 3, 4]);
    assert!(max4 < min5, "arity 4 max {} overlaps arity 5 min {}", max4, min5);
  }

  #[test]
  fn test_encoding_is_unique() {
    // All arity-2 subsets of {0..19} should produce unique indices
    let mut indices = std::collections::HashSet::new();
    for a in 0u32..20 {
      for b in (a+1)..20 {
        let idx = encode(&[a, b]);
        assert!(indices.insert(idx), "duplicate index for [{}, {}]", a, b);
      }
    }
  }
}
