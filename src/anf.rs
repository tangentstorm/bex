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



  fn calc_and(&mut self, x:NID, y:NID)->NID {
    let (vx, vy) = (nid::var(x), nid::var(y));
    if vx < vy {
      // base case: x:a + y:(pq+r)  a<p<q, p<r  --> a(pq+r)
      if nid::is_var(x) { self.vhl(vx, y, O) }
      else {
        //     x:(ab+c) * y:(pq+r)
        //  =  ab(pq) + ab(r) + c(pq) + c(r)
        //  =  a(b(pq+r)) + c(pq+r)
        //  =  a(by) + cy
        // TODO: these can all happen in parallel.
        let ANF{v:a, hi:b, lo:c } = self.fetch(x);
        let hi = self.and(b, y);
        let lo = self.and(c, y);
        self.vhl(a, hi, lo)}}
    else if vx > vy { self.calc_and(y, x) }
    else { panic!("TODO: anf::calc_and when vx=vy")}}

  /// called only by xor, so simple cases are already handled.
  fn calc_xor(&mut self, x:NID, y:NID)->NID {
    let (vx, vy) = (nid::var(x), nid::var(y));
    if vx < vy  {
      // x:(ab+c) + y:(pq+r) --> ab+(c+(pq+r))
      let ANF{v, hi, lo} = self.fetch(x);
      let lo = self.xor(lo, y);
      self.vhl(v, hi, lo)}
    else if vx > vy { self.calc_xor(y, x) }
    else { // vx == vy
      // x:(ab+c) + y:(aq+r) -> ab+c+aq+r -> ab+aq+c+r -> a(b+q)+c+r
      let ANF{v:a, hi:b, lo:c} = self.fetch(x);
      let ANF{v:p, hi:q, lo:r} = self.fetch(y);
      assert_eq!(a,p);
      let hi = self.xor(b, q);
      let lo = self.xor(c, r);
      self.vhl(a, hi, lo)}}

} // impl ANFBase

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


// test suite
test_base_consts!(ANFBase);
test_base_vars!(ANFBase);
test_base_when!(ANFBase);

#[test] fn test_anf_hilo() {
  let mut base = ANFBase::new(1);
  let a = base.var(0);
  let ANF{ v, hi, lo } = base.fetch(a);
  assert_eq!(v, nid::var(a));
  assert_eq!(hi, I);
  assert_eq!(lo, O); }

#[test] fn test_anf_hilo_not() {
  let mut base = ANFBase::new(1);
  let a = base.var(0);
  let ANF{ v, hi, lo } = base.fetch(nid::not(a));
  assert_eq!(v, nid::var(a));
  assert_eq!(hi, I);
  assert_eq!(lo, I); }


#[test] fn test_anf_xor() {
  let mut base = ANFBase::new(2);
  let a = base.var(0); let b = base.var(1);
  let (axb, bxa) = (base.xor(a,b), base.xor(b,a));
  assert_eq!(O, base.xor(a,a), "a xor a should be 0");
  assert_eq!(base.not(a), base.xor(I,a), "a xor 1 should be ~a");
  assert_eq!(axb, bxa, "xor should be order-independent");

  let ANF{ v, hi, lo } = base.fetch(axb);
  assert_eq!(v, nid::var(a));
  assert_eq!(hi, I);
  assert_eq!(lo, b); }

#[test] fn test_anf_xor3() {
  let mut base = ANFBase::new(4);
  let a = base.var(0); let b = base.var(1); let c = base.var(2);
  assert_eq!(expr![base, ((a ^ b) ^ c)],
             expr![base, (a ^ (b ^ c))]); }


#[test] fn test_anf_and() {
  let mut base = ANFBase::new(2);
  let a = base.var(0); let b = base.var(1);
  let ab = base.and(a, b);
  let ANF{v, hi, lo} = base.fetch(ab);
  assert_eq!(v, nid::var(a));
  assert_eq!(hi, b);}


#[test] fn test_anf_and3() {
  let mut base = ANFBase::new(4);
  let a = base.var(0); let b = base.var(1); let c = base.var(2);
  assert_eq!(expr![base, ((a & b) & c)],
             expr![base, (a & (b & c))]); }


#[test] fn test_anf_and_big() {
  // x:(ab+c) * y:(pq+r) --> ab(pq+r) + c(pq+r)
  let mut base = ANFBase::new(4);
  let a = base.var(0); let b = base.var(1); let c = base.var(2);
  let p = base.var(3); let q = base.var(4); let r = base.var(5);
  let ab = base.and(a,b); let pq = base.and(p,q);
  let actual = expr![base, ((ab ^ c) & (pq ^ r))];
  let expected = expr![base, ((ab & (pq ^ r)) ^ (c & (pq ^ r)))];
  assert_eq!(expected, actual); }

