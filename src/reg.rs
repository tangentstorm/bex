//! Registers -- arbitrarily large arrays of bits.
use std::fmt;
use crate::vid::VID;


#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reg { nbits: usize, data: Vec<usize> }

const USIZE:usize = usize::BITS as usize;


impl Reg {

  /// create a new register with the given number of bits
  pub fn new( nbits: usize )-> Self {
    Reg { nbits, data: vec![0; (nbits as f64 / USIZE as f64).ceil() as usize ]}}

  /// constructor that takes the indices of the high bits
  pub fn from_bits( nbits:usize, hi_bits: &[usize] )->Self {
    let mut res = Reg::new(nbits);
    for &bit in hi_bits { res.put(bit, true) }
    res}

  /// constructor that takes the indices of the high bits
  pub fn hi_bits(&self)->Vec<usize> {
    let mut res = vec![];
    for (j, &raw) in self.data.iter().enumerate() {
      let mut bits = raw;
      let offset = j * USIZE;
      for i in 0..USIZE {
        if (bits & 1) == 1 { res.push(offset + i) }
        bits >>= 1 }}
    res}


  /// fetch value of a bit by index
  pub fn get(&self, ix: usize )->bool {
    0 < (self.data[ix/USIZE] & 1 << (ix%USIZE)) }

  /// assign value of a bit by index
  pub fn put(&mut self, ix:usize, v:bool) {
    let i = ix/USIZE; let x = self.data[i];
    self.data[i] =
      if v { x |  (1 << (ix%USIZE)) }
      else { x & !(1 << (ix%USIZE)) }}

  /// fetch value of bit with the given variable's index
  pub fn var_get(&self, v:VID)->bool {
    let ix = v.var_ix();
    self.get(ix) }

  /// assign value of bit with the given variable's index
  pub fn var_put(&mut self, v:VID, val:bool) {
    let ix = v.var_ix();
    self.put(ix, val) }

  /// return the number of bits in the register.
  pub fn len(&self)->usize { self.nbits }

  /// true when the number of bits is 0.
  /// (mostly because clippy complains about len() without is_empty())
  pub fn is_empty(&self)->bool { self.nbits == 0 }



  /// build a usize from the least significant bits of the register.
  pub fn as_usize(&self)->usize { self.data[0] }

  /// build a usize from the least significant bits of the register, in reverse order.
  pub fn as_usize_rev(&self)->usize {
    assert!(self.nbits <= 64, "usize_rev only works for <= 64 bits!");
    let mut tmp = self.as_usize(); let mut res = 0;
    for _ in 0..self.nbits {
      res <<= 1;
      res += tmp & 1;
      tmp >>= 1;}
    res }

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



/// display the bits of the register and the usize
/// e.g. reg[11o=06]
impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      write!(f, "reg[")?;
      let mut write_bit = |i| { write!(f, "{}", if self.get(i) {'1'} else {'o'}) };
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
