//! Experimental structure for representing algebraic normal form (xor of ands).
//!
//! Algebraic normal form is essentially a boolean polynomial.
//! Each term is a product of non-negated inputs. To normalize,
//! We assume the inputs are sorted by number, and then we factor
//! out common prefixes.
//!
//! Thus:
//! ```text
//!     abcd + dc + abc + abd    (input)
//!     abc + abcd + abd + cd    (after sorting)
//!     ab(c(1+d) + d) + cd      (after factoring)
//! ```
//! In addition, identical suffixes after factoring always refer to the same node.

use base::Base;
use nid;
use nid::{NID,VID,I,O};
use hashbrown::HashMap;


/// (v AND hi) XOR lo
// TODO /// (ALL(v0..v1) AND hi) XOR lo
// TODO: /// The v0..v1 thing is used to collapse long chains of nodes where lo=O.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ANF {
  /// v is the variable in the head.
  v: VID,
  /// the hi subgraph gets ANDed to the head.
  hi: NID,
  /// the lo subgraph gets XORed to the hi node.
  lo: NID }

pub struct ANFBase {
  nvars:usize,
  nodes:Vec<ANF>,
  cache:HashMap<ANF,NID>}


impl Base for ANFBase {

  type N = NID;
  type V = VID;

  fn new(n:usize)->Self { ANFBase { nvars: n, nodes:vec![], cache: HashMap::new() } }
  fn num_vars(&self)->usize { self.nvars }

  #[inline] fn o(&self)->NID { O }
  #[inline] fn i(&self)->NID { I }
  #[inline] fn var(&mut self, v:VID)->NID { nid::nv(v) }

  fn def(&mut self, s:String, v:u32)->NID { println!("TODO: anf::def"); self.var(v as VID) }
  fn tag(&mut self, n:NID, s:String)->NID { println!("TODO: anf::tag"); n }

  fn when_lo(&mut self, v:VID, n:NID)->NID {
    let nv = nid::var(n);
    if nv > v { n }  // n independent of v
    else if nv == v {
      if nid::is_lit(n) {
        // a leaf node should never be inverted... unless it's also the root.
        if nid::is_inv(n) { I } else { O }}
      else { self.fetch(n).lo }}
    else { panic!("TODO: anf::when_lo") }}

  fn when_hi(&mut self, v:VID, n:NID)->NID {
    let nv = nid::var(n);
    if nv > v { return n }  // n independent of v
    if nv == v {
      if nid::is_lit(n) {
        if nid::is_inv(n) { O } else { I }}
      else { self.fetch(n).hi }}
    else { panic!("TODO: anf::when_hi") }}

  // logical ops

  #[inline] fn not(&mut self, n:NID)->NID { nid::not(n) }

  fn and(&mut self, x:NID, y:NID)->NID {
    if x == O || y == O { O }
    else if x == I || x == y { y }
    else if y == I { x }
    else if x == self.not(y) { O }
    else { self.calc_and(self.fetch(x), self.fetch(y)) }}

  fn xor(&mut self, x:NID, y:NID)->NID {
    if x == y { O }
    else if x == O { y }
    else if y == O { x }
    else if x == I { self.not(y) }
    else if y == I { self.not(x) }
    else if x == self.not(y) { I }
    else { self.calc_xor(x, y) }}

  fn or(&mut self, x:NID, y:NID)->NID { panic!("TODO: anf::or") }

} // impl Base for ANFBase


// internal ANFBase implementation

impl ANFBase {


  fn fetch(&self, n:NID)->ANF {
    if nid::is_var(n) { // variables are (v*I)+O if normal, (v*I)+I if inverted.
      ANF{v:nid::var(n), hi:I, lo: if nid::is_inv(n) { I } else { O } }}
    else { self.nodes[nid::idx(n)] }}

  fn vhl(&mut self, v:VID, hi:NID, lo:NID)->NID {
    if let Some(&n) = self.cache.get(&ANF{v, hi, lo}) { n }
    else {
      let res = nid::nvi(v, self.nodes.len() as u32);
      let anf = ANF{ v, hi ,lo };
      self.cache.insert(anf, res);
      self.nodes.push(anf);
      res }}

  fn calc_and(&mut self, x:ANF, y:ANF)->NID {
    panic!("TODO: anf::calc_and")} // TODO

  /// called only by xor, so simple cases are already handled.
  fn calc_xor(&mut self, x:NID, y:NID)->NID {
    // x:(ab+c) + y:(pq+r) --> ab+(c+(pq+r))
    let (vx, vy) = (nid::var(x), nid::var(y));
    if vx < vy  {
      let ANF{v, hi, lo} = self.fetch(x);
      let lo = self.xor(lo, y);
      self.vhl(v, hi, lo)}
    else if vx > vy { self.calc_xor(y, x) }
    else { panic!("TODO: anf::calc_xor when vx==vy")}} // TODO

} // impl ANFBase

// test suite

test_base_consts!(ANFBase);
test_base_vars!(ANFBase);
test_base_when!(ANFBase);

#[test] fn test_anf_xor() {
  let mut base = ANFBase::new(4);
  let a = base.var(0); let b = base.var(1);
  let (axb, bxa) = (base.xor(a,b), base.xor(b,a));
  assert_eq!(axb, bxa, "xor should be order-independent") }
