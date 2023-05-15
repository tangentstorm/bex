use crate::Fun;

/// helper for 'fun' (function table) nids
/// u32 x contains the bits to select (or permute).
/// pv is generally a permutation vector (the bytes 0..=31 in some order)
/// but could also be any vector of bits to select from x.
///
/// NOTE: tables are stored in reversed order, so that the number "0"
/// in the pv indicates the most significant bit of x, and "31" the least.
/// This is just so the table looks "correct" when printed or typed as
/// a rust constant.
///
/// If len(pv)<32, then the remaining bits of the result are set to 0.
///
/// In other words, if b=pv[i], we will grab bit (31-b) from x and move to
/// position (31-i) in the result.
fn select_bits(x:u32, pv:&[u8])->u32 {
  // println!(">> select_bits({:032b}, {:?})", x, pv);
  let mut r:u32 = 0;
  for (i,b) in pv.iter().enumerate() { r |= (1&(x>>(31-b))) << (31-i); }
  // println!("==============>{:032b}", r); //^(r<<16));
  r }

impl NidFun {
  pub fn tbl(&self)->u32 { self.nid.tbl().unwrap() }
  pub fn to_nid(&self)->NID { self.nid }}

use std::fmt::{Formatter,Debug,Error};
impl Debug for NidFun {
  fn fmt(&self, f:&mut Formatter<'_>)->Result<(), Error> {
    let mut s = String::new();
    s.push_str("NidFun{");
    s.push_str(&format!("arity:{}, ", self.arity()));
    s.push_str(&format!("tbl:{:032b}", self.tbl()));
    s.push_str("}");
    write!(f, "{}", s) }}



impl Fun for NidFun {

  #[inline(always)] fn arity(&self)->u8 {
    (self.nid.n >> 32 & 0xff) as u8 }

  /// given a function, return the function you'd get if you inverted one or more of the input bits.
  /// bits is a bitmap where setting the (2^i)'s-place bit means to invert the `i`th input.
  /// For example: if `bits=0b00101` maps inputs `x0, x1, x2, x3, x4` to `!x0, x1, !x2, x3, x4`
  fn when_flipped(&self, bits:u8)->Self {
    let mut res:u32 = self.nid.tbl().unwrap();
    let flip = |i:u8| (bits & (1<<i)) != 0;
    macro_rules! p { ($x:expr) => { res = select_bits(res, $x) }}
    if flip(4) { p!(&[16,17,18,19,20,21,22,23,16,17,18,19,20,21,22,23,8 ,9 ,10,11,12,13,14,15,8 ,9 ,10,11,12,13,14,15]) }
    if flip(3) { p!(&[8 ,9 ,10,11,12,13,14,15,0 ,1 ,2 ,3 ,4 ,5 ,6 ,7 ,24,25,26,27,28,29,30,31,16,17,18,19,20,21,22,23]) }
    if flip(2) { p!(&[4 ,5 ,6 ,7 ,0 ,1 ,2 ,3 ,12,13,14,15,8 ,9 ,10,11,20,21,22,23,16,17,18,19,28,29,30,31,24,25,26,27]) }
    if flip(1) { p!(&[2 ,3 ,0 ,1 ,6 ,7 ,4 ,5 ,10,11,8 ,9 ,14,15,12,13,18,19,16,17,22,23,20,21,26,27,24,25,30,31,28,29]) }
    if flip(0) { p!(&[1 ,0 ,3 ,2 ,5 ,4 ,7 ,6 ,9 ,8 ,11,10,13,12,15,14,17,16,19,18,21,20,23,22,25,24,27,26,29,28,31,30]) }
    NID::fun(self.arity(), res)}

  /// given a function, return the function you'd get if you "lift" one of the inputs
  /// by swapping it with its neighbors. (so bit=0 permutes inputs x0,x1,x2,x3,x4 to x1,x0,x2,x3,x4)
  fn when_lifted(&self, bit:u8)->Self {
    macro_rules! p { ($x:expr) => { NID::fun(self.arity(), select_bits(self.nid.tbl().unwrap(), $x)) }}
    match bit {
      3 => p!(&[0 ,1 ,2 ,3 ,4 ,5 ,6 ,7 ,16,17,18,19,20,21,22,23,8 ,9 ,10,11,12,13,14,15,24,25,26,27,28,29,30,31]),
      2 => p!(&[0 ,1 ,2 ,3 ,8 ,9 ,10,11,4 ,5 ,6 ,7 ,12,13,14,15,16,17,18,19,24,25,26,27,20,21,22,23,28,29,30,31]),
      1 => p!(&[0 ,1 ,4 ,5 ,2 ,3 ,6 ,7 ,8 ,9 ,12,13,10,11,14,15,16,17,20,21,18,19,22,23,24,25,28,29,26,27,30,31]),
      0 => p!(&[0 ,2 ,1 ,3 ,4 ,6 ,5 ,7 ,8 ,10,9 ,11,12,14,13,15,16,18,17,19,20,22,21,23,24,26,25,27,28,30,29,31]),
      _ => panic!("{}", "lifted input bit must be in {0,1,2,3}")}}

  fn when(&self, bit:u8, val:bool)->NidFun {
    let a = self.arity();
    if bit >= a { panic!("fun_when: arity is {a} but bit index is {bit}") };
    let tt0 = self.tbl();
    // select the parts of the table where the input bit matches the given value
    macro_rules! s { ($x:expr)=> { { let t=select_bits(tt0, $x); t^(t>>16) }}}
    // tables generated in j with:   ;"1(',',~":)&.>"0 I.|.|:#:i.2^5
    let tt = if val { match bit {
      0 => s!(&[ 1, 3, 5, 7, 9,11,13,15,17,19,21,23,25,27,29,31]),
      1 => s!(&[ 2, 3, 6, 7,10,11,14,15,18,19,22,23,26,27,30,31]),
      2 => s!(&[ 4, 5, 6, 7,12,13,14,15,20,21,22,23,28,29,30,31]),
      3 => s!(&[ 8, 9,10,11,12,13,14,15,24,25,26,27,28,29,30,31]),
      4 => s!(&[16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31]),
      _ => panic!("fun_when: bit must be <5")}}
    else { match bit {  // ;"1(',',~":)&.>"0 I.-.|.|:#:i.2^5
      0 => s!(&[0,2,4,6,8,10,12,14,16,18,20,22,24,26,28,30]),
      1 => s!(&[0,1,4,5,8, 9,12,13,16,17,20,21,24,25,28,29]),
      2 => s!(&[0,1,2,3,8, 9,10,11,16,17,18,19,24,25,26,27]),
      3 => s!(&[0,1,2,3,4, 5, 6, 7,16,17,18,19,20,21,22,23]),
      4 => s!(&[0,1,2,3,4, 5, 6, 7, 8, 9,10,11,12,13,14,15]),
      _ => panic!("fun_when: bit must be <5")}};

    // since bit<a, we know a>0, so a-1 is safe.
    NID::fun(a-1, tt) }

  fn when_same(&self, bit0:u8, bit1:u8)->NidFun {
    assert_ne!(bit0, bit1, "fun_when_same: bits must have different indices.");
    let a = self.arity();
    assert!(bit0 < a && bit1 < a, "fun_when_same: bits must be < arity");
    macro_rules! s { ($x:expr)=> {
      { let t=select_bits(self.tbl(), $x); NID::fun(a-1, t^(t>>16)) }}}
    if bit0>bit1 { self.when_same(bit1, bit0)}
    else { match (bit1, bit0) {
      // J: xy ,. _ ,. I. =/"2 (xy=.4-5 5#: I.,(>/~) y=.i._5) { |.|:#:i.2^5
      (4, 3) => s!(&[0,1,2,3, 4, 5, 6, 7,24,25,26,27,28,29,30,31]),
      (4, 2) => s!(&[0,1,2,3, 8, 9,10,11,20,21,22,23,28,29,30,31]),
      (4, 1) => s!(&[0,1,4,5, 8, 9,12,13,18,19,22,23,26,27,30,31]),
      (4, 0) => s!(&[0,2,4,6, 8,10,12,14,17,19,21,23,25,27,29,31]),
      (3, 2) => s!(&[0,1,2,3,12,13,14,15,16,17,18,19,28,29,30,31]),
      (3, 1) => s!(&[0,1,4,5,10,11,14,15,16,17,20,21,26,27,30,31]),
      (3, 0) => s!(&[0,2,4,6, 9,11,13,15,16,18,20,22,25,27,29,31]),
      (2, 1) => s!(&[0,1,6,7, 8, 9,14,15,16,17,22,23,24,25,30,31]),
      (2, 0) => s!(&[0,2,5,7, 8,10,13,15,16,18,21,23,24,26,29,31]),
      (1, 0) => s!(&[0,3,4,7, 8,11,12,15,16,19,20,23,24,27,28,31]),
      _ => { panic!("fun_when_same: bad bit pair ({bit0},{bit1})")}}}}

  fn when_diff(&self, bit0:u8, bit1:u8)->NidFun {
    assert_ne!(bit0, bit1, "fun_when_diff: bits must have different indices.");
    let a = self.arity();
    assert!(bit0 < a && bit1 < a, "fun_when_diff: bits must be < arity");
    macro_rules! s { ($x:expr)=> {
      { let t=select_bits(self.tbl(), $x); NID::fun(a-1, t^(t>>16)) }}}
    if bit0>bit1 { self.when_diff(bit1, bit0)}
    else { match (bit1, bit0) {
      // J: xy ,. _ ,. I. ~:/"2 (xy=.4-5 5#: I.,(>/~) y=.i._5) { |.|:#:i.2^5
      (4, 3) => s!(&[8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23]),
      (4, 2) => s!(&[4,5, 6, 7,12,13,14,15,16,17,18,19,24,25,26,27]),
      (4, 1) => s!(&[2,3, 6, 7,10,11,14,15,16,17,20,21,24,25,28,29]),
      (4, 0) => s!(&[1,3, 5, 7, 9,11,13,15,16,18,20,22,24,26,28,30]),
      (3, 2) => s!(&[4,5, 6, 7, 8, 9,10,11,20,21,22,23,24,25,26,27]),
      (3, 1) => s!(&[2,3, 6, 7, 8, 9,12,13,18,19,22,23,24,25,28,29]),
      (3, 0) => s!(&[1,3, 5, 7, 8,10,12,14,17,19,21,23,24,26,28,30]),
      (2, 1) => s!(&[2,3, 4, 5,10,11,12,13,18,19,20,21,26,27,28,29]),
      (2, 0) => s!(&[1,3, 4, 6, 9,11,12,14,17,19,20,22,25,27,28,30]),
      (1, 0) => s!(&[1,2, 5, 6, 9,10,13,14,17,18,21,22,25,26,29,30]),
      _ => { panic!("fun_when_diff: bad bit pair ({bit0},{bit1})")}}}}
  }

#[test] fn test_fun() {
  assert!(!NID::var(1).is_fun(), "var(1) should not be fun.");
  assert!(!NID::vir(1).is_fun(), "vir(1) should not be fun.");
  assert!(!NID::from_vid_idx(vid::NOV, 0).is_fun(), "idx var should not be fun");}

#[test] fn test_fun_consts() {
  let x0  = NID::fun(1, 0x55555555);
  let nx0 = NID::fun(1, 0xaaaaaaaa);
  let dk0  = NID::fun(1, 0x00000000);  // degenerate constant 0 (takes an arg but ignores it)
  let a_xor_b = NID::fun(2, 0x66666666);  // x0 xor x1
  let a_and_b = NID::fun(2, 0x11111111);  // x0 and x1
  // TODO: separate out the concepts of t-nid vs f-nid.
  // f-nids should always densely pack the arguments,
  // whereas t-nids are always functions of 5 inputs, some of which may be ignored.
  // in this case, we're dealing with an f-nid, so we start with a=x0,b=x1
  // and then when we set a=true, we're left with b, but b is now x0.
  // (a^b).when(a,1)->!b, so result should be !x0, or 0xaaaaaaaa
  assert_eq!(a_xor_b.when(1, true), nx0);  // obvious
  assert_eq!(a_xor_b.when(1, false), x0);

  assert_eq!(a_xor_b.when(0, true), nx0); // because of renumbering
  assert_eq!(a_xor_b.when(0, false), x0);

  assert_eq!(a_and_b.when(0, true), x0); // obvious
  // TODO: the following is the current behavior, but it probably should not be this way.
  // somehow, we need to express the idea that the function is degenerate.
  assert_eq!(a_and_b.when(0, false), dk0);
  assert_eq!(a_and_b.when(0, true), x0); // because of renumbering
  assert_eq!(a_and_b.when(0, false), dk0);

  // TODO: O and I should allow .to_fun() and have arity 0
  // assert_eq!(NID::o().to_fun().unwrap(), dk0);
}
