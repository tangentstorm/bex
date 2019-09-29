#![macro_use]

///! bex: a boolean expression library for rust
///! outside the base, you deal only with opaque references.
///! inside, it could be stored any way we like.
pub trait Base {

  /// Node identifier type. Usually mapped to xxx::NID
  type N;

  /// Variable identifier type. Usually mapped to xxx::VID
  type V;

  fn new(n:usize)->Self;
  fn num_vars(&self)->usize;

  fn o(&self)->Self::N;
  fn i(&self)->Self::N;
  fn var(&mut self, v:Self::V)->Self::N;
  fn def(&mut self, s:String, i:u32)->Self::N;
  fn tag(&mut self, n:Self::N, s:String)->Self::N;
  fn not(&mut self, x:Self::N)->Self::N;
  fn and(&mut self, x:Self::N, y:Self::N)->Self::N;
  fn xor(&mut self, x:Self::N, y:Self::N)->Self::N;
  fn or(&mut self, x:Self::N, y:Self::N)->Self::N;
  #[cfg(todo)] fn mj(&mut self, x:Self::N, y:Self::N, z:Self::N)->Self::N;
  #[cfg(todo)] fn ch(&mut self, x:Self::N, y:Self::N, z:Self::N)->Self::N;
}

macro_rules! base_test {
  ($name:ident, $basename:ident, $nvars:expr, $tt:tt) => {
    macro_rules! $name {
      ($BaseType:ident) => {
        #[test] fn $name() {
          use base::Base;
          let mut $basename = <$BaseType as Base>::new($nvars);
          $tt }}}}}

base_test!{test_base_consts, base, 0, {
  let (o,i) = (base.o(), base.i());
  assert_eq!(base.and(o, i), o); }}


// TODO: put these elsewhere.
#[cfg(todo)] pub fn order<T:PartialOrd>(x:T, y:T)->(T,T) { if x < y { (x,y) } else { (y,x) }}
#[cfg(todo)] pub fn order3<T:Ord+Clone>(x:T, y:T, z:T)->(T,T,T) {
  let mut res = [x,y,z];
  res.sort();
  (res[0].clone(), res[1].clone(), res[2].clone())}

