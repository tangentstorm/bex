/// This module allows you to work with vectors of bit objects
/// as if they were plain old integers.
extern crate std;
use std::cell::RefCell;
use std::rc::Rc;
use std::cmp::min;
use base::{Base};
use ast::{ASTBase, NID, VID, SID};


// TBit : for use outside the Base, by types such as X32, below.
pub trait TBit
  : Sized + Clone
  + std::ops::Not<Output=Self>
  + std::ops::BitAnd<Self,Output=Self>
  + std::ops::BitXor<Self,Output=Self> {
    fn when(self, var:u32, val:Self)->Self;
    fn sub(self, s:SID)->Self;
  }

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

impl TBit for BaseBit {
  fn when(self, var:u32, val:Self)->Self {
    self.op(|base| base.when(var as usize, val.n, self.n)) }

  fn sub(self, s:SID)->Self {
    self.op(|base| base.sub(self.n, s)) }}

impl std::ops::Not for BaseBit {
  type Output = Self;
  fn not(self) -> Self {
    self.op(|base| base.not(self.n)) }}

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

pub fn gbase_var(v:VID)->BaseBit {
  GBASE.with(|gb| {
    let vn = gb.borrow_mut().var(v); BaseBit{base:gb.clone(), n:vn }}) }

pub fn gbase_tag(n:NID, s:String)->NID {
  GBASE.with(|gb| gb.borrow_mut().tag(n,s) )}

pub fn gbase_def(s:String, i:VID)->BaseBit {
  GBASE.with(|gb| {
    let vn=gb.borrow_mut().def(s,i); BaseBit{base:gb.clone(), n:vn }}) }

pub fn gbase_o()->BaseBit { BaseBit{base:gbase_ref(), n:0} }
pub fn gbase_i()->BaseBit { BaseBit{base:gbase_ref(), n:1} }


// --- lifted u32 type -----------------------------------------

// TODO: implement iterators on the bits to simplify all these loops!!

pub trait BInt<U, T:TBit> : Sized {
  /// the number of bits
  fn n() -> u8;
  fn i(&self) -> T;
  fn o(&self) -> T;
  fn zero() -> Self;
  fn new(&self, u:U) -> Self;
  fn get(&self, i:u8) -> T;
  fn set(&mut self, i:u8, v:T);
  fn rotate_right(&self, y:u8) -> Self {
    let mut res = Self::zero();
    for i in 0..Self::n() { res.set(i, self.get((i+y) % Self::n())) }
    res}

  // TODO: this doesn't actually wrap! (should it??)
  fn wrapping_add(&self, y:Self) -> Self {
    let mut res = Self::zero(); let mut carry = self.o();
    for i in 0..Self::n() { match (self.get(i), y.get(i), carry) {
      (a,b,c) => { res.set(i, a.clone() ^ b.clone() ^ c.clone());
                   carry = bitmaj(a, b, c) }}}
      res}

  fn from<B:BInt<U2,T>,U2>(other:&B) -> Self {
    let mut res = Self::zero();
    for i in 0..min(Self::n(),B::n()) { res.set(i, other.get(i).clone()) }
    res }

  fn times<U2,B:BInt<U2,T>>(&self, y0:&Self) -> B {
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

  fn u(self) -> U; }


macro_rules! xint_type {
  ($n:expr, $c:ident, $T:ident, $U:ty) => {

    #[derive(Clone,PartialEq)]
    pub struct $T{pub bits:Vec<BaseBit>}

    impl $T {
      pub fn new(u:usize)->$T {
        $T{bits:(0..$n)
           .map(|i| if (u&1<<i)==0 { gbase_o() } else { gbase_i() })
           .collect()}}

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

    /// shorthand constructor
    pub fn $c(u:$U) -> $T { $T::new(u as usize) }

    impl std::fmt::Debug for $T {
      fn fmt(&self, f: &mut std::fmt::Formatter)->std::fmt::Result {
        write!(f, "[").expect("!");
        for x in self.bits.iter() {
          match x.n {
            0 => { write!(f, "o").expect("!?"); },
            1 => { write!(f, "I").expect("!?"); },
            n => { write!(f, ":{}", n).expect("!?"); }}}
        write!(f, "]")}}

// TODO: just inline BInt here, so people don't have to import it.

    impl BInt<$U,BaseBit> for $T {
      fn n()->u8 { $n }
      fn zero()->Self { $T::new(0) }
      fn o(&self)->BaseBit { gbase_o() }
      fn i(&self)->BaseBit { gbase_i() }
      fn new(&self, u:$U)->Self { $T::new(u as usize) }
      fn get(&self, i:u8)->BaseBit { self.bits[i as usize].clone() }
      fn set(&mut self, i:u8, v:BaseBit) { self.bits[i as usize]=v }

      fn u(self)->$U {
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

    impl std::ops::Shr<u8> for $T {
      type Output = Self;
      fn shr(self, y:u8) -> Self {
        let mut res = Self::zero();
        for i in 0..($n-y) { res.bits[i as usize] = self.bits[(i+y) as usize].clone() }
        res }}

    impl std::ops::Not for $T {
      type Output = Self;
      fn not(self) -> Self {
        $T{bits: self.bits.iter().map(|x| !x.clone()).collect()} }}

}} // end xint_type macro

// actual type implementations:

xint_type!( 4,  x4,  X4,  u8);  // there's no u4
xint_type!( 8,  x8,  X8,  u8);
xint_type!(16, x16, X16, u16);
xint_type!(32, x32, X32, u32);
xint_type!(64, x64, X64, u64);



// -- test suite for x32

#[test] fn test_roundtrip() {
  let k = 1234567890u32;
  assert_eq!(x32(k).u(), k) }

#[test] fn test_add() {
  assert_eq!((x32(2).wrapping_add(x32(3))).u(), 5u32) }

#[test] fn test_mul32() {
  assert_eq!((x32(2).times::<u32,X32>(&x32(3))).u(),  6u32);
  assert_eq!((x32(3).times::<u32,X32>(&x32(5))).u(), 15u32) }

#[test] fn test_mul64() {
  assert_eq!((x64(2).times::<u64,X64>(&x64(3))).u(),  6u64);
  assert_eq!((x64(3).times::<u64,X64>(&x64(5))).u(), 15u64) }

#[test] fn test_ror() {
  assert_eq!((x32(10).rotate_right(1)).u(), 5u32) }

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
