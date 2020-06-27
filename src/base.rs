#![macro_use]
///! bex: a boolean expression library for rust
///! outside the base, you deal only with opaque references.
///! inside, it could be stored any way we like.
use std::fs::File;
use std::io::Write;
use std::process::Command;      // for creating and viewing digarams
use {nid, nid::{NID}};
use vid::VID;

pub trait Base {
  fn new(n:usize)->Self where Self:Sized; // Sized so we can use trait objects.
  fn num_vars(&self)->usize;

  fn o(&self)->NID { nid::O }
  fn i(&self)->NID { nid::I }

  fn var(&mut self, v:u32)->NID { NID::var(v) }
  fn vir(&mut self, v:u32)->NID { NID::vir(v) }

  fn when_hi(&mut self, v:VID, n:NID)->NID;
  fn when_lo(&mut self, v:VID, n:NID)->NID;

  fn not(&mut self, x:NID)->NID;
  fn and(&mut self, x:NID, y:NID)->NID;
  fn xor(&mut self, x:NID, y:NID)->NID;
  fn or(&mut self, x:NID, y:NID)->NID;
  #[cfg(todo)] fn mj(&mut self, x:NID, y:NID, z:NID)->NID;
  #[cfg(todo)] fn ch(&mut self, x:NID, y:NID, z:NID)->NID;

  fn def(&mut self, s:String, i:VID)->NID;
  fn tag(&mut self, n:NID, s:String)->NID;
  fn get(&self, _s:&str)->Option<NID>;

  /// substitute node for variable in context.
  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID;

  fn save(&self, path:&str)->::std::io::Result<()>;

  /// implement this to render a node and its descendents in graphviz *.dot format.
  fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write);

  /// render to graphviz *.dot file
  fn save_dot(&self, n:NID, path:&str) {
    let mut s = String::new(); self.dot(n, &mut s);
    let mut txt = File::create(path).expect("couldn't create dot file");
    txt.write_all(s.as_bytes()).expect("failed to write text to dot file"); }

  /// call save_dot, use graphviz to convert to svg, and open result in firefox
  fn show_named(&self, n:NID, s:&str) {
    self.save_dot(n, format!("{}.dot", s).as_str());
    let out = Command::new("dot").args(&["-Tsvg",format!("{}.dot",s).as_str()])
      .output().expect("failed to run 'dot' command");
    let mut svg = File::create(format!("{}.svg",s).as_str()).expect("couldn't create svg");
    svg.write_all(&out.stdout).expect("couldn't write svg");
    Command::new("firefox").args(&[format!("{}.svg",s).as_str()])
      .spawn().expect("failed to launch firefox"); }

  fn show(&self, n:NID) { self.show_named(n, "+bdd") }
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

  assert!(o<i, "expect o<i");

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
  // assert!(b.o() < x0, "expect O < $0");
  assert!(x0 < b.i(), "expect $0 < I");
  let nx0 = b.not(x0);
  assert!(x0 == b.not(nx0), "expected x0 = ¬¬x0"); });


// Test when_lo and when_hi for the simple cases.
base_test!(test_base_when, b, 2, {
  let (o, i, x0, x1) = (b.o(), b.i(), b.var(0), b.var(1));
  let v = x0.vid();

  assert_eq!(b.when_lo(v, o), o, "x0=O should not affect O");
  assert_eq!(b.when_hi(v, o), o, "x0=I should not affect O");
  assert_eq!(b.when_lo(v, i), i, "x0=O should not affect I");
  assert_eq!(b.when_hi(v, i), i, "x0=I should not affect I");

  assert_eq!(b.when_lo(v, x0), o, "when_lo(0,x0) should be O");
  assert_eq!(b.when_hi(v, x0), i, "when_hi(0,x0) should not I");

  assert_eq!(b.when_lo(v, x1), x1, "x0=O should not affect x1");
  assert_eq!(b.when_hi(v, x1), x1, "x0=I should not affect x1"); });



// TODO: put these elsewhere.
#[cfg(todo)] pub fn order<T:PartialOrd>(x:T, y:T)->(T,T) { if x < y { (x,y) } else { (y,x) }}
#[cfg(todo)] pub fn order3<T:Ord+Clone>(x:T, y:T, z:T)->(T,T,T) {
  let mut res = [x,y,z];
  res.sort();
  (res[0].clone(), res[1].clone(), res[2].clone())}
