/// Registers (bit vectors)
use std::fmt;
use std::mem::size_of;
use vid::{VID, SMALL_ON_TOP};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reg { nvars: usize, data: Vec<usize> }

const USIZE:usize = size_of::<usize>() * 8;

impl Reg {

  pub fn new( nvars: usize )-> Self {
    Reg { nvars, data: vec![0; (nvars as f64 / USIZE as f64).ceil() as usize ]}}

  pub fn get(&self, ix: usize )->bool {
    let ix = if SMALL_ON_TOP { (self.nvars-1)-ix } else { ix };
    // let ix = (self.nvars-1)-ix;
    0 < (self.data[ix/USIZE] & 1 << (ix%USIZE)) }

  pub fn put(&mut self, ix:usize, v:bool) {
    let ix = if SMALL_ON_TOP { (self.nvars-1)-ix } else { ix };
    // let ix = (self.nvars-1)-ix;
    let i = ix/USIZE; let x = self.data[i];
    self.data[i] =
      if v { x |  (1 << (ix%USIZE)) }
      else { x & !(1 << (ix%USIZE)) }}

  pub fn var_get(&self, v:VID)->bool {
    let ix = v.var_ix();
    self.get(ix) }
  pub fn var_put(&mut self, v:VID, val:bool) {
    let ix = v.var_ix();
    //let ix = if SMALL_ON_TOP { ix } else { (self.nvars-1)-ix };
    self.put(ix, val) }

  pub fn as_usize_fwd(&self)->usize { self.data[0] }
  pub fn as_usize_rev(&self)->usize {
    assert!(self.nvars <= 64, "usize_rev only works for <= 64 vars!");
    let mut tmp = self.as_usize_fwd(); let mut res = 0;
    for _ in 0..self.nvars {
      res <<= 1;
      res += tmp & 1;
      tmp >>= 1;}
    res }

  pub fn as_usize(&self)->usize {
    if SMALL_ON_TOP { self.as_usize_fwd() }
    else { self.as_usize_fwd() }}

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
  pub fn increment(&mut self)->Option<usize> {
    if SMALL_ON_TOP { self.ripple(self.nvars-1, 0) }
    else { self.ripple(0, self.nvars-1) }}

  pub fn len(&self)->usize { self.nvars }
  pub fn is_empty(&self)->bool { self.nvars == 0 }}

  impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      write!(f, "reg[")?;
      let mut write_bit = |i| { write!(f, "{}", if self.get(i) {'1'} else {'o'}) };
      if SMALL_ON_TOP { for i in 0..self.nvars { write_bit(i)? }}
      else { for i in (0..self.nvars).rev() { write_bit(i)? } };
      write!(f, "={:02x}]", self.as_usize()) }}

  /// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
  impl fmt::Debug for Reg { // for test suite output
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}

#[test]
fn test_reg_mut() {
  let mut reg = Reg::new(66);
  assert_eq!(reg.data.len(), 2);
  assert_eq!(reg.data[0], 0);
  assert_eq!(reg.get(0), false);
  reg.put(0, true);
  if SMALL_ON_TOP {
    assert_eq!(reg.data[0], 0);
    assert_eq!(reg.data[1], 2); // bit '0' is the most signficant bit
    assert_eq!(reg.get(0), true);
    assert_eq!(reg.get(1), false);
    // it's 0 as_usize because we get the 64 rightmost bits, and it looks like this:
    // reg[1ooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo=0]=1
    assert_eq!(reg.as_usize(), 0, "expected reg[...=0], got: {:?}", reg);
    reg.put(1, true);
    assert_eq!(reg.data[1], 3); }
  else {
    assert_eq!(reg.data[0], 1); // bit '0' is the least significant bit
    assert_eq!(reg.data[1], 0);
    assert_eq!(reg.get(0), true);
    assert_eq!(reg.get(1), false);
    // now
    assert_eq!(reg.as_usize(), 1, "{:?}=1", reg);
    reg.put(1, true);
    assert_eq!(reg.data[0], 3); }
  assert_eq!(reg.get(1), true); }

#[cfg(feature="small_on_top")]
#[test] fn test_reg_inc_lotop() {
  let mut reg = Reg::new(2);
  assert_eq!(0, reg.as_usize());
  assert_eq!(Some(1), reg.increment(), "00 -> 01");
  assert_eq!(1, reg.as_usize());
  assert_eq!(Some(0), reg.increment(), "01 -> 10");
  assert_eq!(2, reg.as_usize());
  assert_eq!(Some(1), reg.increment(), "10 -> 11");
  assert_eq!(3, reg.as_usize());
  assert_eq!(None, reg.increment(), "11 -> 00"); }

#[cfg(not(feature="small_on_top"))]
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
