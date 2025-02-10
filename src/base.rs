#![macro_use]
//! Standard trait for databases of boolean expressions.
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::Write;
use std::process::Command;      // for creating and viewing digarams
use crate::{simp, nid::NID};
use crate::vid::VID;
use crate::reg::Reg;

/// Functions common to all expression databases.
pub trait Base {
  /// Create a new instance of the `Base`.
  fn new()->Self where Self:Sized; // Sized so we can use trait objects.

  /// Return the value of node `n` when `v=1`.
  fn when_hi(&mut self, v:VID, n:NID)->NID;
  /// Return the value of node `n` when `v=0`.
  fn when_lo(&mut self, v:VID, n:NID)->NID;

  /// Return a `NID` representing the logical AND of `x` and `y`.
  fn and(&mut self, x:NID, y:NID)->NID;

  /// Return a `NID` representing the logical XOR of `x` and `y`.
  fn xor(&mut self, x:NID, y:NID)->NID;

  /// Return a `NID` representing the logical OR of `x` and `y`.
  fn or(&mut self, x:NID, y:NID)->NID;

  /// Assign a name to variable `v`, and return its `NID`.
  fn def(&mut self, s:String, v:VID)->NID;

  /// Assign a name to node `n` and return `n`.
  fn tag(&mut self, n:NID, s:String)->NID;

  /// Fetch a node by name.
  fn get(&self, s:&str)->Option<NID>;

  /// substitute node for variable in context.
  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID;

  /// recursively evaluate a nid, substituting in the given values
  /// (internal helper function for eval, eval_all)
  fn _eval_aux(&mut self, _n:NID, _kv: &HashMap<VID,NID>, _cache:&mut HashMap<NID,NID>)->NID {
    todo!("_eval_aux not yet implemented for this type") }

  /// evaluate a list of nids, substituting in the given values.
  fn eval_all(&mut self, nids: &[NID], kv: &HashMap<VID,NID>)->Vec<NID> {
    let mut cache = HashMap::new();
    nids.iter().map(|&n| self._eval_aux(n, kv, &mut cache)).collect() }

  /// evaluate a single nid (substituting in the given values)
  fn eval(&mut self, nid:NID, kv:&HashMap<VID, NID>)->NID {
    self.eval_all(&[nid], kv)[0] }

  /// Render node `n` (and its descendents) in graphviz *.dot format.
  fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write);

  /// generate ALL solutions.
  // !! This is a terrible idea, but it's the best I can do right now.
  // TODO: figure out the right way to return an iterator in a trait.
  fn solution_set(&self, _n:NID, _nvars:usize)->HashSet<Reg> { unimplemented!() }

  // !! these are defined here but never overwritten in the trait (used by solver) [fix this]
  fn init_stats(&mut self) { }
  fn print_stats(&mut self) { }}


/// trait for visualization using GraphViz
pub trait GraphViz {
  fn write_dot(&self, n:NID, wr: &mut dyn std::fmt::Write);

  /// render to graphviz *.dot file
  fn save_dot(&self, n:NID, path:&str) {
    let mut s = String::new(); self.write_dot(n, &mut s);
    let mut txt = File::create(path).expect("couldn't create dot file");
    txt.write_all(s.as_bytes()).expect("failed to write text to dot file"); }

  /// call save_dot, use graphviz to convert to svg, and open result in firefox
  fn show_named(&self, n:NID, s:&str) {
    self.save_dot(n, format!("{}.dot", s).as_str());
    let out = Command::new("dot").args(["-Tsvg",format!("{}.dot",s).as_str()])
      .output().expect("failed to run 'dot' command");
    let mut svg = File::create(format!("{}.svg",s).as_str()).expect("couldn't create svg");
    svg.write_all(&out.stdout).expect("couldn't write svg");
    let _ = Command::new("firefox").args([format!("{}.svg",s).as_str()])
      .spawn().expect("failed to launch firefox").wait(); }

  fn show(&self, n:NID) { self.show_named(n, "+bdd") }
}

impl<T:Base> GraphViz for T {
  fn write_dot(&self, n:NID, wr: &mut dyn std::fmt::Write) {
    T::dot(self,n, wr)}}


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
///   inherit![ new, when_hi, when_lo, and, xor, or, def, tag, get, sub, dot ]; }
/// ```
#[macro_export] macro_rules! inherit {
  ( $($i:ident),* ) => { $( inherit!(@fn $i); )* };
  (@fn new) =>      { #[inline] fn new()->Self where Self:Sized { Self { base: T::new() }} };
  (@fn when_hi) =>  { #[inline] fn when_hi(&mut self, v:VID, n:NID)->NID { self.base.when_hi(v, n) }};
  (@fn when_lo) =>  { #[inline] fn when_lo(&mut self, v:VID, n:NID)->NID { self.base.when_lo(v, n) }};
  (@fn and) =>      { #[inline] fn and(&mut self, x:NID, y:NID)->NID { self.base.and(x, y) }};
  (@fn xor) =>      { #[inline] fn xor(&mut self, x:NID, y:NID)->NID { self.base.xor(x, y) }};
  (@fn or) =>       { #[inline] fn or(&mut self, x:NID, y:NID)->NID  { self.base.or(x, y) }};
  (@fn def) =>      { #[inline] fn def(&mut self, s:String, i:VID)->NID { self.base.def(s, i) }};
  (@fn tag) =>      { #[inline] fn tag(&mut self, n:NID, s:String)->NID { self.base.tag(n, s) }};
  (@fn get) =>      { #[inline] fn get(&self, s:&str)->Option<NID> { self.base.get(s) }};
  (@fn sub) =>      { #[inline] fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID { self.base.sub(v, n, ctx) }};
  (@fn dot) =>      { #[inline] fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write) { self.base.dot(n, wr) }}; }



// !! start on isolating simplification rules (for use in AST, ANF)
pub struct Simplify<T:Base> { pub base: T }

impl<T:Base> Base for Simplify<T> {
  inherit![ new, when_hi, when_lo, xor, or, def, tag, get, sub, dot ];
  fn and(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::and(x,y) { nid }
    else {
      let (a, b) = if x < y { (x,y) } else { (y,x) };
      self.base.and(a, b) }}}


// macros for building and testing expressions

/// Macro for building complex expressions in a `Base`.
/// example: `expr![base, (x & y) | (y ^ z)]`
#[macro_export] macro_rules! expr {
  (@op $b:ident, $x:tt $op:ident $y:tt) => {{
    let x = expr![$b, $x];
    let y = expr![$b, $y];
    $b.$op(x,y) }};
  ($_:ident, $id:ident) => { $id };
  ($b:ident, ($x:tt ^ $y:tt)) => { expr![@op $b, $x xor $y] };
  ($b:ident, ($x:tt & $y:tt)) => { expr![@op $b, $x and $y] };}

/// Macro to make a substitution map for eval.
/// example: `use vid::named::{x0, x1}; vid_map![x0:I, x1:O]`
#[macro_export] macro_rules! vid_map {
  ($($x:ident : $y:expr),*) => {
     vec![$(($x, $y)),*].iter().copied().collect::<HashMap<VID,NID>>() }}


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
  ($name:ident, $basename:ident, $tt:tt) => {
    macro_rules! $name {
      ($BaseType:ident) => {
        #[test] fn $name() {
          use crate::base::Base;
          let mut $basename = <$BaseType as Base>::new();
          $tt }}}}}


// Test operations on constants.
base_test!(test_base_consts, b, {
  use crate::{O,I};

  assert!(O<I, "expect O<I");

  // and
  assert!(O==b.and(O,O), "O∧O");  assert!(O==b.and(I,O), "I∧O");
  assert!(O==b.and(O,I), "O∧I");  assert!(I==b.and(I,I), "I∧I");

  // xor
  assert!(O==b.xor(O,O), "O≠O");  assert!(I==b.xor(I,O), "I≠O");
  assert!(I==b.xor(O,I), "O≠I");  assert!(O==b.xor(I,I), "I≠I"); });


// Test when_lo and when_hi for the simple cases.
base_test!(test_base_when, b, {
  use crate::nid::{O,I, named::{x0, x1}};
  let (vx0, vx1) = (x0.vid(), x1.vid());

  assert_eq!(b.when_lo(vx0, O), O, "vx0=O should not affect O");
  assert_eq!(b.when_hi(vx0, O), O, "vx0=I should not affect O");
  assert_eq!(b.when_lo(vx0, I), I, "vx0=O should not affect I");
  assert_eq!(b.when_hi(vx0, I), I, "vx0=I should not affect I");

  assert_eq!(b.when_lo(vx0, x0), O, "when_lo(vx0, x0) should be O");
  assert_eq!(b.when_hi(vx0, x0), I, "when_hi(vx0, x0) should be I");

  assert_eq!(b.when_lo(vx0, x1), x1, "vx0=O should not affect x1");
  assert_eq!(b.when_hi(vx0, x1), x1, "vx0=I should not affect x1");

  assert_eq!(b.when_lo(vx1, x0), x0, "vx1=O should not affect x0");
  assert_eq!(b.when_hi(vx1, x0), x0, "vx1=I should not affect x0");

  assert_eq!(b.when_lo(vx1, x1), O, "when_lo(vx1, x1) should be O");
  assert_eq!(b.when_hi(vx1, x1), I, "when_hi(vx1, x1) should be I");
});



// TODO: put these elsewhere.
// pub fn order<T:PartialOrd>(x:T, y:T)->(T,T) { if x < y { (x,y) } else { (y,x) }}
// pub fn order3<T:Ord+Clone>(x:T, y:T, z:T)->(T,T,T) {
//  let mut res = [x,y,z];
//  res.sort();
//  (res[0].clone(), res[1].clone(), res[2].clone())}
