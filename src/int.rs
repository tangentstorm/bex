/// This module allows you to work with vectors of bit objects
/// as if they were plain old integers.
extern crate std;
use std::cell::RefCell;
use std::rc::Rc;
use std::cmp::min;
use ast::ASTBase;
use base::{Base};
use {nid, nid::NID};
use {vid::VID};


// TBit : for use outside the Base, by types such as X32, below.
pub trait TBit
  : Sized + Clone
  + std::ops::Not<Output=Self>
  + std::ops::BitAnd<Self,Output=Self>
  + std::ops::BitXor<Self,Output=Self> { }

// TODO: how can i merge with mj() below?
fn bitmaj<T:TBit>(x:T, y:T, z:T) -> T {
  (x.clone()&y.clone()) ^ (x&z.clone()) ^ (y&z) }


// BaseBit implementation (u32 references into a Base)
pub type BaseRef = Rc<RefCell<ASTBase>>;

// -- basebit --
#[derive(Clone)]
pub struct BaseBit {pub base:BaseRef, pub n:NID}

impl BaseBit {
  /// perform an arbitrary operation using the base
  fn op<F:FnMut(&mut ASTBase)->NID>(&self, mut op:F)->BaseBit {
    let r = op(&mut self.base.borrow_mut());
    BaseBit{base:self.base.clone(), n:r} }}

impl std::cmp::PartialEq for BaseBit {
  fn eq(&self, other:&Self)->bool {
    self.base.as_ptr() == other.base.as_ptr() && self.n==other.n }}

impl TBit for BaseBit {}

impl std::ops::Not for BaseBit {
  type Output = Self;
  fn not(self) -> Self {
    self.op(|_| !self.n) }}

impl std::ops::BitAnd<BaseBit> for BaseBit {
  type Output = Self;
  fn bitand(self, other:Self) -> Self {
    self.op(|base| base.and(self.n, other.n)) }}

impl std::ops::BitXor<BaseBit> for BaseBit {
  type Output = Self;
  fn bitxor(self, other:Self) -> Self {
    self.op(|base| base.xor(self.n, other.n))}}

impl std::ops::BitOr<BaseBit> for BaseBit {
  type Output = Self;
  fn bitor(self, other:Self) -> Self {
    self.op(|base| base.or(self.n, other.n)) }}

impl std::fmt::Debug for BaseBit {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "[#{}]", self.n) }}

// -- thread - global base --

thread_local!{ pub static GBASE:BaseRef = Rc::new(RefCell::new(ASTBase::empty())); }
pub fn gbase_ref()->BaseRef {
  GBASE.with(|gb| gb.clone()) }

pub fn gbase_var(v:u32)->BaseBit {
  GBASE.with(|gb| { BaseBit{base:gb.clone(), n:NID::var(v) }}) }

pub fn gbase_tag(n:NID, s:String)->NID {
  GBASE.with(|gb| { gb.borrow_mut().tag(n,s) })}

pub fn gbase_def(s:String, i:VID)->BaseBit {
  GBASE.with(|gb| { let vn=gb.borrow_mut().def(s,i); BaseBit{base:gb.clone(), n:vn }}) }

pub fn gbase_o()->BaseBit { BaseBit{base:gbase_ref(), n:nid::O} }
pub fn gbase_i()->BaseBit { BaseBit{base:gbase_ref(), n:nid::I} }


// --- lifted u32 type -----------------------------------------

// TODO: implement iterators on the bits to simplify all these loops!!

pub trait BInt<T:TBit> : Sized {
  /// the number of bits
  fn n() -> u32;
  fn i(&self) -> T;
  fn o(&self) -> T;
  fn zero() -> Self;
  fn get(&self, i:u32) -> T;
  fn set(&mut self, i:u32, v:T);
  fn rotate_right(&self, y:u32) -> Self {
    let mut res = Self::zero();
    for i in 0..Self::n() { res.set(i, self.get((i+y) % Self::n())) }
    res}

  // TODO: this doesn't actually wrap! (should it??)
  fn wrapping_add(&self, y:Self) -> Self {
    let mut res = Self::zero(); let mut carry = self.o();
    for i in 0..Self::n() {
      let (a,b,c) = (self.get(i), y.get(i), carry);
      res.set(i, a.clone() ^ b.clone() ^ c.clone());
      carry = bitmaj(a, b, c);}
    res}

  fn from<B:BInt<T>>(other:&B) -> Self {
    let mut res = Self::zero();
    for i in 0..min(Self::n(),B::n()) { res.set(i, other.get(i).clone()) }
    res }

  fn times<B:BInt<T>>(&self, y0:&Self) -> B {
    let mut sum = B::zero();
    let x = B::from(self);
    let y = B::from(y0);
    for i in 0..B::n() {
      let mut xi = x.rotate_right(0); // poor man's copy
      for j in 0..B::n() {
        let xij = xi.get(j) & y.get(i);
        xi.set(j, xij) }
      sum = sum.wrapping_add(xi.rotate_right(B::n() -i)); }
    sum }

  fn u(self) -> usize; }


macro_rules! xint_type {
  ($n:expr, $T:ident) => {

    #[derive(Clone,PartialEq)]
    pub struct $T{pub bits:Vec<BaseBit>}

    impl $T {
      pub fn new(u:usize)->$T {
        $T{bits:(0..$n)
           .map(|i| if (u&1<<i)==0 { gbase_o() } else { gbase_i() })
           .collect()}}

      /// define an entire set of variables at once.
      pub fn def(s:&str, start:u32)->$T {
        $T::from_vec((0..$n).map(|i|{ gbase_def(s.to_string(), VID::var(start+i)) }).collect()) }

      pub fn from_vec(v:Vec<BaseBit>)->$T {
        $T{bits: if v.len() >= $n { v.iter().take($n).map(|x|x.clone()).collect() }
           else {
             let zs = (0..($n-v.len())).map(|_| gbase_o());
             v.iter().map(|x|x.clone()).chain(zs.into_iter()).collect() }}}

      pub fn eq(&self, other:&Self)-> BaseBit {
        let mut res = gbase_i();
        for (x, y) in self.bits.iter().zip(other.bits.iter()) {
          // TODO: implement EQL (XNOR) nodes in base
          let eq = !(x.clone()^y.clone());
          // println!("{} eq {} ?  {}", x.n, y.n, eq.n);
          res = res & eq}
        res}

      pub fn lt(&self, other:&Self)-> BaseBit {
        let mut res = gbase_o();
        for (x, y) in self.bits.iter().zip(other.bits.iter()) {
          // TODO: implement EQ, LT nodes in base
          let eq = !(x.clone() ^ y.clone());
          let lt = (!x.clone()) & y.clone();
          res = lt | (eq & res); }
        res}
    }

    impl std::fmt::Debug for $T {
      fn fmt(&self, f: &mut std::fmt::Formatter)->std::fmt::Result {
        write!(f, "[").expect("!");
        for x in self.bits.iter() { write!(f, "{:?}", x).expect("!?") }
        write!(f, "]")}}

// TODO: just inline BInt here, so people don't have to import it.

    impl BInt<BaseBit> for $T {
      fn n()->u32 { $n }
      fn zero()->Self { $T::new(0) }
      fn o(&self)->BaseBit { gbase_o() }
      fn i(&self)->BaseBit { gbase_i() }
      fn get(&self, i:u32)->BaseBit { self.bits[i as usize].clone() }
      fn set(&mut self, i:u32, v:BaseBit) { self.bits[i as usize]=v }

      fn u(self)->usize {
        let mut u = 0; let mut i = 0;
        #[allow(clippy::toplevel_ref_arg)]
        for ref bit in self.bits.iter() {
          if bit.clone() == &self.i() { u|=1<<i }
          // TODO : u() should return a Result
          i+=1 }
        u }}

    // formatting and bitwise operators

    impl std::fmt::LowerHex for $T {
      fn fmt(&self, formatter:&mut std::fmt::Formatter) ->
        std::result::Result<(), std::fmt::Error> {
          self.clone().u().fmt(formatter) } }

    impl std::ops::BitAnd<Self> for $T {
      type Output = Self;
      fn bitand(self, rhs:Self) -> Self {
        $T{bits: self.bits.iter().zip(rhs.bits.iter())
           .map(|(x,y)| x.clone() & y.clone())
           .collect() }}}

    impl std::ops::BitXor<Self> for $T {
      type Output = Self;
      fn bitxor(self, rhs:Self) -> Self {
        $T{bits: self.bits.iter().zip(rhs.bits.iter())
           .map(|(x,y)| x.clone() ^ y.clone())
           .collect() }}}

    impl std::ops::Shr<u32> for $T {
      type Output = Self;
      fn shr(self, y:u32) -> Self {
        let mut res = Self::zero();
        for i in 0..($n-y) { res.bits[i as usize] = self.bits[(i+y) as usize].clone() }
        res }}

    impl std::ops::Not for $T {
      type Output = Self;
      fn not(self) -> Self {
        $T{bits: self.bits.iter().map(|x| !x.clone()).collect()} }}

}} // end xint_type macro

// actual type implementations:

xint_type!( 2,  X2); pub fn x2(u:usize)->X2 { X2::new(u) }
xint_type!( 4,  X4); pub fn x4(u:usize)->X4 { X4::new(u) }
xint_type!( 8,  X8); pub fn x8(u:usize)->X8 { X8::new(u) }
xint_type!(16, X16); pub fn x16(u:usize)->X16 { X16::new(u) }
xint_type!(32, X32); pub fn x32(u:usize)->X32 { X32::new(u) }
xint_type!(64, X64); pub fn x64(u:usize)->X64 { X64::new(u) }



// -- test suite for x32

#[test] fn test_roundtrip() {
  let k = 1234567890;
  assert_eq!(x32(k).u(), k) }

#[test] fn test_add() {
  assert_eq!((x32(2).wrapping_add(x32(3))).u(), 5) }

#[test] fn test_mul32() {
  assert_eq!((x32(2).times::<X32>(&x32(3))).u(),  6);
  assert_eq!((x32(3).times::<X32>(&x32(5))).u(), 15) }

#[test] fn test_mul64() {
  assert_eq!((x64(2).times::<X64>(&x64(3))).u(),  6);
  assert_eq!((x64(3).times::<X64>(&x64(5))).u(), 15) }

#[test] fn test_ror() {
  assert_eq!((x32(10).rotate_right(1)).u(), 5) }

#[test] fn test_lt() {
  assert_eq!(x4(1).lt(&x4(2)), gbase_i());
  assert_eq!(x4(2).lt(&x4(1)), gbase_o());
  assert_eq!(x32(10).lt(&x32(11)), gbase_i());
  assert_eq!(x32(11).lt(&x32(10)), gbase_o());
  assert_eq!(x32(10).lt(&x32(10)), gbase_o()); }

#[test] fn test_eq() {
  assert_eq!(x32(10).eq(&x32(10)), gbase_i());
  assert_eq!(x32(11).eq(&x32(10)), gbase_o());
  assert_eq!(x32(10).eq(&x32(11)), gbase_o()); }
