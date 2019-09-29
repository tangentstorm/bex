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


/// (v AND hi) XOR lo
// TODO /// (ALL(v0..v1) AND hi) XOR lo
// TODO: /// The v0..v1 thing is used to collapse long chains of nodes where lo=O.
#[derive(Clone, Copy)]
struct ANF {
  /// v is the variable in the head.
  _v: VID,
  /// the hi subgraph gets ANDed to the head.
  hi: NID,
  /// the lo subgraph gets XORed to the hi node.
  lo: NID }

enum ANFNode {
  /// Lit holds constants or simple variables
  Lit(NID),
  /// Ref holds a regular ANF node.
  Reg(ANF),
  /// Neg holds a negated ANF node. (Meaning an extra 1 term needs to be XORed).
  Neg(ANF)}

pub struct ANFBase {
  nvars:usize,
  nodes:Vec<ANF> }


impl Base for ANFBase {

  type N = NID;
  type V = VID;

  fn new(n:usize)->Self { ANFBase { nvars: n, nodes:vec![] } }
  fn num_vars(&self)->usize { self.nvars }

  #[inline] fn o(&self)->NID { O }
  #[inline] fn i(&self)->NID { I }
  #[inline] fn var(&mut self, v:VID)->NID { nid::nv(v) }

  fn def(&mut self, s:String, v:u32)->NID { println!("TODO: anf::def"); self.var(v as VID) }
  fn tag(&mut self, n:NID, s:String)->NID { println!("TODO: anf::tag"); n }

  fn when_lo(&mut self, v:VID, n:NID)->NID {
    let nv = nid::var(n);
    if nv > v { return n }  // n independent of v
    if nv == v {
      match self.fetch(n) {
        ANFNode::Lit(x) => O,  // should only happen when v==nv==x, and v:=O
        ANFNode::Reg(x) => x.lo,
        ANFNode::Neg(x) => nid::not(x.lo) }}
    else { panic!("TODO: anf::when_lo") }}

  fn when_hi(&mut self, v:VID, n:NID)->NID {
    let nv = nid::var(n);
    if nv > v { return n }  // n independent of v
    if nv == v {
      match self.fetch(n) {
        ANFNode::Lit(n) => I,  // should only happen when v==nv==x, and v:=I
        ANFNode::Reg(x) => self.xor(x.hi, x.lo),
        ANFNode::Neg(x) => nid::not(self.xor(x.hi, x.lo)) }}
    else { panic!("TODO: anf::when_hi") }}

  // logical ops

  #[inline] fn not(&mut self, n:NID)->NID { nid::not(n) }

  fn and(&mut self, x:NID, y:NID)->NID {
    if x == O || y == O { O }
    else if x == I || x == y { y }
    else if y == I { x }
    else if x == self.not(y) { O }
    else { self.calc_and(x, y) }}

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

  fn fetch(&mut self, n:NID)->ANFNode {
    if nid::is_lit(n) { ANFNode::Lit(n) }
    else {
      let a = self.nodes[nid::idx(n)];
      if nid::is_inv(n) { ANFNode::Neg(a) }
      else { ANFNode::Reg(a) }}}

  fn calc_and(&mut self, x:NID, y:NID)->NID {
    panic!("TODO: anf::calc_and")}

  fn calc_xor(&mut self, x:NID, y:NID)->NID {
    panic!("TODO: anf::calc_xor")}

} // impl ANFBase

// test suite

test_base_consts!(ANFBase);
test_base_vars!(ANFBase);
test_base_when!(ANFBase);
