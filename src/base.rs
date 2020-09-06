#![macro_use]
///! bex: a boolean expression library for rust
///! outside the base, you deal only with opaque node IDs (NIDs).
///! inside, it could be stored any way we like.
use std::fs::File;
use std::io::Write;
use std::process::Command;      // for creating and viewing digarams
use {nid, nid::{NID}};
use vid::VID;

pub trait Base {
  fn new(n:usize)->Self where Self:Sized; // Sized so we can use trait objects.
  fn num_vars(&self)->usize;

  fn when_hi(&mut self, v:VID, n:NID)->NID;
  fn when_lo(&mut self, v:VID, n:NID)->NID;

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

/// This macro makes it easy to define decorators for `Base` implementations.
/// Define your decorator as a struct with type parameter `T:Base` and member `base: T`,
/// then use this macro to implement the functions you *don't* want to manually decorate.
///
/// ```
/// #[macro_use] extern crate bex;
/// use bex::{base::Base, nid::NID, vid::VID};
///
/// // example do-nothing decorator
/// pub struct Decorated<T:Base> { base: T }
/// impl<T:Base> Base for Decorated<T> {
///   inherit![ new, num_vars, when_hi, when_lo, and, xor, or, def, tag, get, sub, save, dot ]; }
/// ```
#[macro_export] macro_rules! inherit {
  ( $($i:ident),* ) => { $( inherit_fn!($i); )* }
}

/// This helper macro provides actual implementations for the names passed to `inherit!`
#[macro_export] macro_rules! inherit_fn {
  (new) =>      { fn new(_n:usize)->Self where Self:Sized { unimplemented!("you need to explicitly construct the decorator") }};
  (num_vars) => { #[inline] fn num_vars(&self)->usize { self.base.num_vars() }};
  (when_hi) =>  { #[inline] fn when_hi(&mut self, v:VID, n:NID)->NID { self.base.when_hi(v, n) }};
  (when_lo) =>  { #[inline] fn when_lo(&mut self, v:VID, n:NID)->NID { self.base.when_lo(v, n) }};
  (and) =>      { #[inline] fn and(&mut self, x:NID, y:NID)->NID { self.base.and(x, y) }};
  (xor) =>      { #[inline] fn xor(&mut self, x:NID, y:NID)->NID { self.base.xor(x, y) }};
  (or) =>       { #[inline] fn or(&mut self, x:NID, y:NID)->NID  { self.base.or(x, y) }};
  (def) =>      { #[inline] fn def(&mut self, s:String, i:VID)->NID { self.base.def(s, i) }};
  (tag) =>      { #[inline] fn tag(&mut self, n:NID, s:String)->NID { self.base.tag(n, s) }};
  (get) =>      { #[inline] fn get(&self, s:&str)->Option<NID> { self.base.get(s) }};
  (sub) =>      { #[inline] fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID { self.base.sub(v, n, ctx) }};
  (save) =>     { #[inline] fn save(&self, path:&str)->::std::io::Result<()> { self.base.save(path) }};
  (dot) =>      { #[inline] fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write) { self.base.dot(n, wr) }};
}


// !! start on isolating simplification rules (for use in AST, ANF)
pub struct Simplify<T:Base> { pub base: T }
impl<T:Base> Base for Simplify<T> {
  inherit![ new, num_vars, when_hi, when_lo, xor, or, def, tag, get, sub, save, dot ];
  fn and(&mut self, x:NID, y:NID)->NID {
    if x == y { x }
    else {
      let (a, b) = if x < y { (x,y) } else { (y,x) };
      if a == nid::O { nid::O }
      else if a == nid::I { b }
      else if !a == b { nid::O }
      else { println!("dispatching to original AND"); self.base.and(x, y) }}
  }
}


// macros for building expressions

#[macro_export]
macro_rules! op {
  ($b:ident, $x:tt $op:ident $y:tt) => {{
    let x = expr![$b, $x];
    let y = expr![$b, $y];
    $b.$op(x,y) }}}

#[macro_export]
macro_rules! expr {
  ($_:ident, $id:ident) => { $id };
  ($b:ident, ($x:tt ^ $y:tt)) => { op![$b, $x xor $y] };
  ($b:ident, ($x:tt & $y:tt)) => { op![$b, $x and $y] };}


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
  use nid;
  let (o, i) = (nid::O, nid::I);

  assert!(o<i, "expect o<i");

  // and
  assert!(o==b.and(o,o), "o∧o");  assert!(o==b.and(i,o), "i∧o");
  assert!(o==b.and(o,i), "o∧i");  assert!(i==b.and(i,i), "i∧i");

  // xor
  assert!(o==b.xor(o,o), "o≠o");  assert!(i==b.xor(i,o), "i≠o");
  assert!(i==b.xor(o,i), "o≠i");  assert!(o==b.xor(i,i), "i≠i"); });


// Test when_lo and when_hi for the simple cases.
base_test!(test_base_when, b, 2, {
  use nid::{O,I,NID};
  let (o, i, n0, n1) = (O, I, NID::var(0), NID::var(1));
  let (x0, x1) = (n0.vid(), n1.vid());

  assert_eq!(b.when_lo(x0, o), o, "x0=O should not affect O");
  assert_eq!(b.when_hi(x0, o), o, "x0=I should not affect O");
  assert_eq!(b.when_lo(x0, i), i, "x0=O should not affect I");
  assert_eq!(b.when_hi(x0, i), i, "x0=I should not affect I");

  assert_eq!(b.when_lo(x0, n0), o, "when_lo(0,n0) should be O");
  assert_eq!(b.when_hi(x0, n0), i, "when_hi(0,n0) should be I");

  assert_eq!(b.when_lo(x0, n1), n1, "x0=O should not affect n1");
  assert_eq!(b.when_hi(x0, n1), n1, "x0=I should not affect n1");

  assert_eq!(b.when_lo(x1, n0), n0, "x1=O should not affect n0");
  assert_eq!(b.when_hi(x1, n0), n0, "x1=I should not affect n0");

  assert_eq!(b.when_lo(x1, n1), o, "when_lo(1,n1) should be O");
  assert_eq!(b.when_hi(x1, n1), i, "when_hi(1,n1) should be I");
});



// TODO: put these elsewhere.
#[cfg(todo)] pub fn order<T:PartialOrd>(x:T, y:T)->(T,T) { if x < y { (x,y) } else { (y,x) }}
#[cfg(todo)] pub fn order3<T:Ord+Clone>(x:T, y:T, z:T)->(T,T,T) {
  let mut res = [x,y,z];
  res.sort();
  (res[0].clone(), res[1].clone(), res[2].clone())}
