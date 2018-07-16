/// = bex::x32 : treating arrays of bitrefs as u32/i32.

use std::cell::RefCell;
use std::rc::Rc;
use bex::*;


// TBit : for use outside the Base, by types such as X32, below.
trait TBit
  : Sized + Clone
  + std::ops::Not<Output=Self>
  + std::ops::BitAnd<Self,Output=Self>
  + std::ops::BitXor<Self,Output=Self> {
    fn when(self, var:u32, val:Self)->Self;
    fn sub(self, s:SID)->Self;
  }

// TODO: how can i merge with mj() below?
fn bitmaj<T:TBit>(x:T, y:T, z:T) -> T {
  (x.clone()&y.clone()) ^ (x.clone()&z.clone()) ^ (y&z) }


// BaseBit implementation (u38 references into TBase)
pub type BaseRef = Rc<RefCell<Base>>;

#[derive(Clone)]
pub struct BaseBit {pub base:BaseRef, pub n:NID}

impl std::cmp::PartialEq for BaseBit {
  fn eq(&self, other:&Self)->bool {
    self.base.as_ptr() == other.base.as_ptr() && self.n==other.n }}

impl TBit for BaseBit {
  fn when(self, var:u32, val:Self)->Self {
    let r = self.base.borrow_mut().when(var as usize, val.n, self.n);
    BaseBit{base:self.base, n:r} }

  fn sub(self, s:SID)->Self {
    BaseBit{base:self.base.clone(), n:self.base.borrow_mut().sub(self.n, s)} } }


impl std::ops::Not for BaseBit {
  type Output = BaseBit;
  fn not(self) -> Self {
    let r = self.base.borrow_mut().not(self.n);
    BaseBit{base:self.base, n:r} } }

impl std::ops::BitAnd<BaseBit> for BaseBit {
  type Output = Self;
  fn bitand(self, other:Self) -> Self {
    let r = self.base.borrow_mut().and(self.n, other.n);
    BaseBit{base:self.base, n:r} } }

impl std::ops::BitXor<BaseBit> for BaseBit {
  type Output = Self;
  fn bitxor(self, other:Self) -> Self {
    let r = self.base.borrow_mut().xor(self.n, other.n);
    BaseBit{base:self.base, n:r} } }

impl std::ops::BitOr<BaseBit> for BaseBit {
  type Output = Self;
  fn bitor(self, other:Self) -> Self {
    let r = self.base.borrow_mut().or(self.n, other.n);
    BaseBit{base:self.base, n:r} } }

impl std::fmt::Debug for BaseBit {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "[#{}]", self.n) }}


// --- lifted u32 type -----------------------------------------

pub trait TBit32<T:TBit> : Sized {
  fn i(&self) -> T;
  fn o(&self) -> T;
  fn new(&self, u:u32) -> Self;
  fn get(&self, i:u8) -> T;
  fn set(&mut self, i:u8, v:T);
  fn rotate_right(self, y:u8) -> Self {
    let mut res = self.new(0);
    for i in 0..32 { res.set(i, self.get((i+y) % 32)) }
    res}
  fn wrapping_add(self, y:Self) -> Self {
    let mut res = self.new(0); let mut carry = self.o();
    for i in 0..32 { match (self.get(i), y.get(i), carry) {
      (a,b,c) => { res.set(i, a.clone() ^ b.clone() ^ c.clone());
                   carry = bitmaj(a, b, c) }}}
    res}
  fn u(self) -> u32; }


#[derive(Clone,PartialEq)]
pub struct BB32{pub bits:Vec<BaseBit>}

thread_local!{ pub static GBASE:BaseRef = Rc::new(RefCell::new(Base::new())); }
pub fn gbase_ref()->BaseRef {
  GBASE.with(|gb| gb.clone()) }

pub fn gbase_var(v:VID)->BaseBit {
  GBASE.with(|gb| {
    let vn = gb.borrow_mut().var(v); BaseBit{base:gb.clone(), n:vn }}) }

pub fn gbase_tag(n:NID, s:String) {
  GBASE.with(|gb| gb.borrow_mut().tag(n,s) )}

pub fn gbase_def(s:String, i:u32)->BaseBit {
  GBASE.with(|gb| {
    let vn=gb.borrow_mut().def(s,i); BaseBit{base:gb.clone(), n:vn }}) }

pub fn gbase_o()->BaseBit { BaseBit{base:gbase_ref(), n:0} }
pub fn gbase_i()->BaseBit { BaseBit{base:gbase_ref(), n:1} }


fn bb32(u:u32) -> BB32 {
  let mut bits = vec![];
  for i in 0..32 { bits.push(if (u&1<<i)==0 { gbase_o() } else { gbase_i() }) }
  BB32{bits:bits} }

impl std::fmt::Debug for BB32 {
  fn fmt(&self, f: &mut std::fmt::Formatter)->std::fmt::Result {
    write!(f, "[").expect("!");
    for j in 0..32 {
      match self.bits[j].n {
        0 => { write!(f, "o").expect("!?"); },
        1 => { write!(f, "I").expect("!?"); },
        n => { write!(f, ":{}", n).expect("!?"); }}}
    write!(f, "]")}}


impl TBit32<BaseBit> for BB32 {
  fn o(&self)->BaseBit { gbase_o() }
  fn i(&self)->BaseBit { gbase_i() }
  fn new(&self, u:u32)->Self { bb32(u) }
  fn get(&self, i:u8)->BaseBit { self.bits[i as usize].clone() }
  fn set(&mut self, i:u8, v:BaseBit) { self.bits[i as usize]=v }

  fn u(self)->u32 {
    let mut u = 0; let mut i = 0;
    for ref bit in self.bits.iter() {
      if bit.clone() == &self.i() { u|=1<<i }
      // TODO : u() should return a Result
      i+=1 }
    u }}


// from this point on, we'll use X32 instead of a specific implementation.
// (there were multiple impls at one point, and may be again when I add
// bdd stuff in the future)
// TODO: figure out how to use traits/generic/other rust features to
// allow more than one type here.
pub type X32 = BB32;
pub fn x32(u:u32)->X32 { bb32(u) }


// operators and formatting for X32

impl std::fmt::LowerHex for X32 {
  fn fmt(&self, formatter:&mut std::fmt::Formatter) ->
    std::result::Result<(), std::fmt::Error> {
      self.clone().u().fmt(formatter) } }

impl std::ops::BitAnd<Self> for X32 {
  type Output = Self;
  fn bitand(self, rhs:Self) -> Self {
    let mut res = self.new(0);
    for i in 0..32 { res.bits[i] = self.bits[i].clone() & rhs.bits[i].clone() }
    res }}

impl std::ops::BitXor<Self> for X32 {
  type Output = Self;
  fn bitxor(self, rhs:Self) -> Self {
    let mut res = self.new(0);
    for i in 0..32 { res.bits[i] = self.bits[i].clone() ^ rhs.bits[i].clone() }
    res }}

impl std::ops::Shr<u8> for X32 {
  type Output = Self;
  fn shr(self, y:u8) -> Self {
    let mut res = self.new(0);
    for i in 0..(32-y) { res.bits[i as usize] = self.bits[(i+y) as usize].clone() }
    res }}

impl std::ops::Not for X32 {
  type Output = Self;
  fn not(self) -> Self {
    let mut res = self.clone();
    for i in 0..32 { res.bits[i] = !res.bits[i].clone() }
    res }}
