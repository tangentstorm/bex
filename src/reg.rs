/// Registers (bit vectors)
use std::mem::size_of;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reg { nvars: usize, data: Vec<usize> }

const USIZE:usize = size_of::<usize>() * 8;

impl Reg {

  pub fn new( nvars: usize )-> Self {
    Reg { nvars, data: vec![0; (nvars as f64 / USIZE as f64).ceil() as usize ]}}

  pub fn get(&self, ix: usize )->bool {
    0 < (self.data[ix/USIZE] & 1 << (ix%USIZE)) }

  pub fn put(&mut self, ix:usize, v:bool) {
    let i = ix/USIZE; let x = self.data[i];
    self.data[i] =
      if v { x |  (1 << (ix%USIZE)) }
      else { x & !(1 << (ix%USIZE)) }}

  pub fn as_usize(&self)->usize { self.data[0] }
  pub fn len(&self)->usize { self.nvars }
  pub fn is_empty(&self)->bool { self.nvars == 0 }}


#[test]
fn test_reg_mut() {
  let mut reg = Reg::new(65);
  assert_eq!(reg.data.len(), 2);
  assert_eq!(reg.data[1], 0);
  assert_eq!(reg.get(65), false);
  reg.put(65, true);
  assert_eq!(reg.data[1], 2);
  assert_eq!(reg.get(65), true);
  assert_eq!(reg.get(64), false);
  reg.put(64, true);
  assert_eq!(reg.data[1], 3);
  assert_eq!(reg.get(64), true);}
