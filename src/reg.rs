/// Registers (bit vectors)
use std::mem::size_of;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reg { nvars: usize, data: Vec<usize> }

const USIZE:usize = size_of::<usize>() * 8;

impl Reg {

  pub fn new( nvars: usize )-> Self {
    Reg { nvars, data: vec![0; (nvars as f64 / USIZE as f64).ceil() as usize ]}}

  pub fn get(&self, ix: usize )->bool {
    let ix = (self.nvars-1)-ix;
    0 < (self.data[ix/USIZE] & 1 << (ix%USIZE)) }

  pub fn put(&mut self, ix:usize, v:bool) {
    let ix = (self.nvars-1)-ix;
    let i = ix/USIZE; let x = self.data[i];
    self.data[i] =
      if v { x |  (1 << (ix%USIZE)) }
      else { x & !(1 << (ix%USIZE)) }}

  pub fn as_usize(&self)->usize { self.data[0] }
  pub fn as_usize_rev(&self)->usize {
    assert!(self.nvars <= 64, "usize_rev only works for <= 64 vars!");
    let mut tmp = self.as_usize(); let mut res = 0;
    for _ in 0..self.nvars {
      res <<= 1;
      res += tmp & 1;
      tmp >>= 1;}
    res }

  pub fn len(&self)->usize { self.nvars }
  pub fn is_empty(&self)->bool { self.nvars == 0 }}


#[test]
fn test_reg_mut() {
  let mut reg = Reg::new(66);
  assert_eq!(reg.data.len(), 2);
  assert_eq!(reg.data[0], 0);
  assert_eq!(reg.get(0), false);
  reg.put(0, true);
  assert_eq!(reg.data[1], 2);
  assert_eq!(reg.get(0), true);
  assert_eq!(reg.get(1), false);
  reg.put(1, true);
  assert_eq!(reg.data[1], 3);
  assert_eq!(reg.get(1), true);}
