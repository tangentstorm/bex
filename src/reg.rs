//! Registers -- arbitrarily large arrays of bits.
use std::fmt;
use crate::vid::VID;
use std::ops::{BitAnd, BitOr, BitXor, Not};

const USIZE:usize = usize::BITS as usize;

// === RegOps trait: common read-only operations ===

/// Trait for read-only bit-vector operations, implemented by both Reg and RegView.
pub trait RegOps {
  /// Access the underlying data as a slice
  fn data(&self) -> &[usize];

  /// Return the number of bits in the register
  fn len(&self) -> usize;

  /// True when the number of bits is 0
  fn is_empty(&self) -> bool { self.len() == 0 }

  /// Fetch value of a bit by index
  fn get(&self, ix: usize) -> bool {
    0 < (self.data()[ix/USIZE] & 1 << (ix%USIZE)) }

  /// Fetch value of bit with the given variable's index
  fn var_get(&self, v: VID) -> bool {
    self.get(v.var_ix()) }

  /// Build a usize from the least significant bits of the register
  fn as_usize(&self) -> usize { self.data()[0] }

  /// Build a usize from the least significant bits in reverse order
  fn as_usize_rev(&self) -> usize {
    assert!(self.len() <= 64, "usize_rev only works for <= 64 bits!");
    let mut tmp = self.as_usize(); let mut res = 0;
    for _ in 0..self.len() {
      res <<= 1;
      res += tmp & 1;
      tmp >>= 1; }
    res }

  /// Check if all bits are zero
  fn is_zero(&self) -> bool {
    self.data().iter().all(|&x| x == 0) }

  /// Find the index of the first (lowest) set bit, or None if all zeros
  fn first_set_bit(&self) -> Option<usize> {
    for (chunk_idx, &chunk) in self.data().iter().enumerate() {
      if chunk != 0 {
        return Some(chunk_idx * USIZE + chunk.trailing_zeros() as usize); }}
    None }

  /// Return the high bits of the register as a vector of indices
  fn hi_bits(&self) -> Vec<usize> {
    let mut res = vec![];
    for (j, &raw) in self.data().iter().enumerate() {
      let mut bits = raw;
      let offset = j * USIZE;
      for i in 0..USIZE {
        if (bits & 1) == 1 { res.push(offset + i) }
        bits >>= 1 }}
    res }
}

// === RegView: borrowed view into bit data ===

/// A borrowed view into register data. Zero-cost abstraction for read-only access.
#[derive(Clone, Copy)]
pub struct RegView<'a> {
  nbits: usize,
  data: &'a [usize],
}

impl<'a> RegView<'a> {
  /// Create a RegView from a slice
  pub fn new(nbits: usize, data: &'a [usize]) -> Self {
    RegView { nbits, data }
  }
}

impl RegOps for RegView<'_> {
  fn data(&self) -> &[usize] { self.data }
  fn len(&self) -> usize { self.nbits }
}

// === Reg: owned register ===

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reg { nbits: usize, pub(crate) data: Vec<usize> }

impl RegOps for Reg {
  fn data(&self) -> &[usize] { &self.data }
  fn len(&self) -> usize { self.nbits }
}

impl Reg {

  /// create a new register with the given number of bits
  pub fn new( nbits: usize )-> Self {
    Reg { nbits, data: vec![0; (nbits as f64 / USIZE as f64).ceil() as usize ]}}

  /// Mutable access to underlying data
  pub fn data_mut(&mut self) -> &mut [usize] { &mut self.data }

  /// Create a RegView borrowing this Reg's data
  pub fn as_view(&self) -> RegView<'_> {
    RegView { nbits: self.nbits, data: &self.data }
  }

  /// constructor that takes the indices of the high bits
  pub fn from_bits( nbits:usize, hi_bits: &[usize] )->Self {
    let mut res = Reg::new(nbits);
    for &bit in hi_bits {
      if bit >= nbits { panic!("called from_bits({nbits:?},...) with out-of-bounds bit {bit}") }
      else { res.put(bit, true) }}
    res}

  /// assign value of a bit by index
  pub fn put(&mut self, ix:usize, v:bool) {
    let i = ix/USIZE; let x = self.data[i];
    self.data[i] =
      if v { x |  (1 << (ix%USIZE)) }
      else { x & !(1 << (ix%USIZE)) }}

  /// assign value of bit with the given variable's index
  pub fn var_put(&mut self, v:VID, val:bool) {
    let ix = v.var_ix();
    self.put(ix, val) }

  // permute the bits according to the given permutation vector.
  // b=pv[i] means to grab bit b from x and move to position i in the result.
  pub fn permute_bits(&self, pv:&[usize])->Self {
    let mut res = self.clone();
    for (i,b) in pv.iter().enumerate() { res.put(i, self.get(*b)); }
    res}


  /// ripple add with carry within the region specified by start and end
  /// (inclusive), returning Some position where a 0 became a 1, or None on overflow.
  pub fn ripple(&mut self, start:usize, end:usize)->Option<usize> {
    let mut j = start as i64; let end = end as i64;
    if j == end { return None }
    let dj:i64 = if j > end { -1 } else { 1 };
    loop {
      let u = j as usize;
      let old = self.get(u);
      self.put(u, !old);
      if !old { break } // we flipped a 0 to a 1. return the position.
      else if j == end { return None }
      else { j+=dj }}
    Some(j as usize)}

  /// increment the register as if adding 1.
  /// return position where the ripple-carry stopped.
  pub fn increment(&mut self)->Option<usize> { self.ripple(0, self.nbits-1) }

} // impl Reg


// -- bitwise operations --------------------------------------------

impl Reg {
  fn mask_last_cell(&mut self) {
    let rem = self.nbits % USIZE;
    let mask = if rem == 0 { !0 } else { (1 << rem) - 1 };
    if let Some(last) = self.data.last_mut() { *last &= mask; }}

  /// Create a register with all bits set to 1
  pub fn ones(nbits: usize) -> Self {
    let mut reg = Reg::new(nbits);
    for cell in &mut reg.data { *cell = !0usize; }
    reg.mask_last_cell();
    reg }
}

impl<'b> BitAnd<&'b Reg> for &Reg {
  type Output = Reg;
  fn bitand(self, rhs: &'b Reg) -> Self::Output {
    let mut res = self.clone();
    for (i, &val) in rhs.data.iter().enumerate() {
      if i < res.data.len() { res.data[i] &= val; }
      else { res.data.push(val); }}
    res }}

impl<'b> BitOr<&'b Reg> for &Reg {
  type Output = Reg;
  fn bitor(self, rhs: &'b Reg) -> Self::Output {
    let mut res = self.clone();
    for (i, &val) in rhs.data.iter().enumerate() {
      if i < res.data.len() { res.data[i] |= val; }
      else { res.data.push(val); }}
    res}}

impl<'b> BitXor<&'b Reg> for &Reg {
  type Output = Reg;
  fn bitxor(self, rhs: &'b Reg) -> Self::Output {
    let mut res = self.clone();
    for (i, &val) in rhs.data.iter().enumerate() {
        if i < res.data.len() { res.data[i] ^= val; }
        else { res.data.push(val); }}
    res }}

impl Not for &Reg {
  type Output = Reg;
  fn not(self) -> Self::Output {
    let mut res = self.clone();
    for val in &mut res.data { *val = !*val; }
    res.mask_last_cell();
    res }}


/// display the bits of the register and the usize
/// e.g. reg[11o=06]
impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      write!(f, "reg[")?;
      let mut write_bit = |i| { write!(f, "{}", if self.get(i) {'i'} else {'o'}) };
      for i in (0..self.nbits).rev() { write_bit(i)? };
      write!(f, "={:02x}]", self.as_usize()) }}

/// Same as fmt::Display.
impl fmt::Debug for Reg { // for test suite output
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}

#[test] #[allow(clippy::bool_assert_comparison)]
fn test_reg_mut() {
  let mut reg = Reg::new(66);
  assert_eq!(reg.data.len(), 2);
  assert_eq!(reg.data[0], 0);
  assert_eq!(reg.get(0), false);
  reg.put(0, true);
  assert_eq!(reg.data[0], 1); // bit '0' is the least significant bit
  assert_eq!(reg.data[1], 0);
  assert_eq!(reg.get(0), true);
  assert_eq!(reg.get(1), false);
  // now
  assert_eq!(reg.as_usize(), 1, "{:?}=1", reg);
  reg.put(1, true);
  assert_eq!(reg.data[0], 3);
  assert_eq!(reg.get(1), true); }

#[test] fn test_reg_inc_hitop() {
  let mut reg = Reg::new(2);
  assert_eq!(0, reg.as_usize());
  assert_eq!(Some(0), reg.increment(), "00 -> 01");
  assert_eq!(1, reg.as_usize());
  assert_eq!(Some(1), reg.increment(), "01 -> 10");
  assert_eq!(2, reg.as_usize());
  assert_eq!(Some(0), reg.increment(), "10 -> 11");
  assert_eq!(3, reg.as_usize());
  assert_eq!(None, reg.increment(), "11 -> 00"); }


#[test] fn test_bits() {
  let ten = Reg::from_bits(4, &[3,1]);
  assert_eq!(ten.as_usize(), 0b1010, "reg with bits 3 and 1 set should equal 10");
  assert_eq!(ten.hi_bits(), [1,3], "bits for 'ten' should come back in order");
  let big = Reg::from_bits(65, &[64,63]);
  assert_eq!(big.hi_bits(), [63,64], "bits for 'big' should come back in order"); }

#[test]
fn test_reg_and() {
  let reg1 = Reg::from_bits(70, &[0, 1, 2, 3, 64, 65]);
  let reg2 = Reg::from_bits(70, &[1, 2, 66, 67]);
  let and_result = &reg1 & &reg2;
  assert_eq!(and_result.hi_bits(), vec![1, 2]);}

#[test]
fn test_reg_or() {
  let reg1 = Reg::from_bits(70, &[0, 1, 2, 3, 64, 65]);
  let reg2 = Reg::from_bits(70, &[1, 2, 66, 67]);
  let or_result = &reg1 | &reg2;
  assert_eq!(or_result.hi_bits(), vec![0, 1, 2, 3, 64, 65, 66, 67]);}

#[test]
fn test_reg_xor() {
  let reg1 = Reg::from_bits(70, &[0, 1, 2, 3, 64, 65, 68]);
  let reg2 = Reg::from_bits(70, &[1, 2, 66, 67, 68]);
  let xor_result = &reg1 ^ &reg2;
  assert_eq!(xor_result.hi_bits(), vec![0, 3, 64, 65, 66, 67]);}

#[test]
fn test_reg_not() {
  let reg1 = Reg::from_bits(70, &[0, 1, 2, 3, 64, 65]);
  let not_result = !&reg1;
  let expected_not_bits: Vec<usize> = (0..70).filter(|&i| ![0, 1, 2, 3, 64, 65].contains(&i)).collect();
  assert_eq!(not_result.hi_bits(), expected_not_bits);}

#[test]
fn test_reg_mask() {
  let mut reg = Reg::from_bits(5, &[0, 1, 2, 3]);
  assert_eq!(&reg.as_usize(), &0b1111);
  reg.mask_last_cell();
  assert_eq!(&reg.as_usize(), &0b1111); }
