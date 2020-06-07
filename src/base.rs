#![macro_use]
///! bex: a boolean expression library for rust
///! outside the base, you deal only with opaque references.
///! inside, it could be stored any way we like.
use nid::VID;

pub trait Base {
  /// Node identifier type. Usually mapped to xxx::NID
  type N;

  fn new(n:usize)->Self where Self:Sized; // Sized so we can use trait objects.
  fn num_vars(&self)->usize;

  fn o(&self)->Self::N;
  fn i(&self)->Self::N;

  fn var(&mut self, i:VID)->Self::N;
  fn when_hi(&mut self, v:VID, n:Self::N)->Self::N;
  fn when_lo(&mut self, v:VID, n:Self::N)->Self::N;

  fn not(&mut self, x:Self::N)->Self::N;
  fn and(&mut self, x:Self::N, y:Self::N)->Self::N;
  fn xor(&mut self, x:Self::N, y:Self::N)->Self::N;
  fn or(&mut self, x:Self::N, y:Self::N)->Self::N;
  #[cfg(todo)] fn mj(&mut self, x:Self::N, y:Self::N, z:Self::N)->Self::N;
  #[cfg(todo)] fn ch(&mut self, x:Self::N, y:Self::N, z:Self::N)->Self::N;

  fn def(&mut self, s:String, i:VID)->Self::N;
  fn tag(&mut self, n:Self::N, s:String)->Self::N;
  fn get(&mut self, _s:&str)->Option<Self::N>;

  /// substitute node for variable in context.
  fn sub(&mut self, v:VID, n:Self::N, ctx:Self::N)->Self::N;
  fn solutions(&self)->&dyn Iterator<Item=Vec<bool>>;

  fn save(&self, path:&str)->::std::io::Result<()>;
  fn save_dot(&self, n:Self::N, path:&str);
  fn show_named(&self, n:Self::N, path:&str);
}

/*
/// TODO: Generic tagging support for any base type.
pub struct Tagged<B:Base> {
  base: B,
  tags: HashMap<String,B::N> }

impl<B:Base> Tagged<B> {
  pub fn def(&mut self, s:String, v:B::V)->B::N { self.base.var(v) }
  pub fn tag(&mut self, n:B::N, s:String)->B::N { n }}

 */

// Meta-macro that generates a macro for testing any base implementation.
macro_rules! base_test {
  ($name:ident, $basename:ident, $nvars:expr, $tt:tt) => {
    macro_rules! $name {
      ($BaseType:ident) => {
        #[test] fn $name() {
          use base::Base;
          let mut $basename = <$BaseType as Base>::new($nvars);
          $tt }}}}}


// Test operations on constants.
base_test!(test_base_consts, b, 0, {
  let (o, i) = (b.o(), b.i());

  // the const functions should give same answer each time
  assert!(o==b.o(), "o");  assert!(o==b.o(), "i");

  // not:
  assert!(i==b.not(o), "¬o");  assert!(o==b.not(i), "¬i");

  // and
  assert!(o==b.and(o,o), "o∧o");  assert!(o==b.and(i,o), "i∧o");
  assert!(o==b.and(o,i), "o∧i");  assert!(i==b.and(i,i), "i∧i");

  // xor
  assert!(o==b.xor(o,o), "o≠o");  assert!(i==b.xor(i,o), "i≠o");
  assert!(i==b.xor(o,i), "o≠i");  assert!(o==b.xor(i,i), "i≠i"); });


// Test simple variable operations.
base_test!(test_base_vars, b, 2, {
  assert!(b.num_vars() == 2);
  let x0 = b.var(0); let x02 = b.var(0); let x1 = b.var(1);
  assert!(x0 == x02, "var(0) should always return the same nid.");
  assert!(x1 != x0, "different variables should have different nids.");
  let nx0 = b.not(x0);
  assert!(x0 == b.not(nx0), "expected x0 = ¬¬x0"); });


// Test when_lo and when_hi for the simple cases.
base_test!(test_base_when, b, 2, {
  let (o, i, x0, x1) = (b.o(), b.i(), b.var(0), b.var(1));

  assert!(b.when_lo(0, o) == o, "x0=O should not affect O");
  assert!(b.when_hi(0, o) == o, "x0=I should not affect O");
  assert!(b.when_lo(0, i) == i, "x0=O should not affect I");
  assert!(b.when_hi(0, i) == i, "x0=I should not affect I");

  assert!(b.when_lo(0, x0) == o, "when_lo(0,x0) should be O");
  assert!(b.when_hi(0, x0) == i, "when_hi(0,x0) should not I");

  assert!(b.when_lo(0, x1) == x1, "x0=O should not affect x1");
  assert!(b.when_hi(0, x1) == x1, "x0=I should not affect x1"); });



// TODO: put these elsewhere.
#[cfg(todo)] pub fn order<T:PartialOrd>(x:T, y:T)->(T,T) { if x < y { (x,y) } else { (y,x) }}
#[cfg(todo)] pub fn order3<T:Ord+Clone>(x:T, y:T, z:T)->(T,T,T) {
  let mut res = [x,y,z];
  res.sort();
  (res[0].clone(), res[1].clone(), res[2].clone())}

