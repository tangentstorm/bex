
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
  cache:HashMap<ANF,NID>,
  tags:HashMap<String,NID>}



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


impl Base for ANFBase {

  type N = NID;
  type V = VID;

  fn new(n:usize)->Self {
    ANFBase { nvars: n, nodes:vec![], cache: HashMap::new(), tags:HashMap::new() }}
  fn num_vars(&self)->usize { self.nvars }

  #[inline] fn o(&self)->NID { O }
  #[inline] fn i(&self)->NID { I }
  #[inline] fn var(&mut self, v:VID)->NID { nid::nv(v) }

  fn def(&mut self, _s:String, _v:u32)->NID { todo!("anf::def"); }
  // TODO: tag and get are copied verbatim from bdd
  fn tag(&mut self, n:NID, s:String)->NID { self.tags.insert(s, n); n }
  fn get(&mut self, s:&String)->Option<NID> { Some(*self.tags.get(s)?) }

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
    else if x == nid::not(y) { O }

    // We want any 'xor 1' (not) to be kept at the top level. There are four cases:
    else {
      let (a,b) = (nid::raw(x), nid::raw(y));  // x!=I because it was handled above.
      if nid::is_inv(x) {
        // case 0:  x:~a & y:~b ==> 1 ^ a ^ ab ^ b
        if nid::is_inv(y) { expr![ self,  (I ^ (a ^ ((a & b) ^ b)))] }
        // case 1:  x:~a & y:b ==>  ab ^ b
        else { expr![ self, ((a & b) ^ b)] }}
      // case 2: x:a & y:~b ==> ab ^ a
      else if nid::is_inv(y) { expr![ self, ((a & b) ^ a)] }
      // case 3: x:a & y:b ==> ab
      else { self.calc_and(x, y) }}}

  fn xor(&mut self, x:NID, y:NID)->NID {
    if x == y { O }
    else if x == O { y }
    else if y == O { x }
    else if x == I { nid::not(y) }
    else if y == I { nid::not(x) }
    else if x == nid::not(y) { I }
    else {
      // xor the raw anf expressions (without any 'xor 1' bits), then xor the bits.
      let (a, b) = (nid::raw(x), nid::raw(y));
      let res = self.calc_xor(a, b);
      if nid::is_inv(x) == nid::is_inv(y) { res }
      else { nid::not(res) }}}

  fn or(&mut self, x:NID, y:NID)->NID { expr![self, ((x & y) ^ (x ^ y))] }

  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID {
    let cv = nid::var(ctx);
    if v < cv { ctx } // ctx can't contain v
    else {
      let x = self.fetch(ctx);
      let (hi, lo) = (x.hi, x.lo);
      if v == cv { expr![self, ((n & hi) ^ lo)] }
      else {
        let rhi = self.sub(v,n,hi);
        let rlo = self.sub(v,n,lo);
        let top = nid::nv(cv);
        expr![self, ((top & rhi) ^ rlo)] }}}

  fn solutions(&self)->&dyn Iterator<Item=Vec<bool>> { todo!("anf::solutions") }

  fn save(&self, _path:&str)->::std::io::Result<()> { todo!("anf::save") }
  fn save_dot(&self, _n:NID, _path:&str) { todo!("anf::save_dot") }
  fn show_named(&self, _n:NID, _path:&str) { todo!("anf::show_named") }

} // impl Base for ANFBase

// internal ANFBase implementation

impl ANFBase {

  fn fetch(&self, n:NID)->ANF {
    if nid::is_var(n) { // variables are (v*I)+O if normal, (v*I)+I if inverted.
      ANF{v:nid::var(n), hi:I, lo: if nid::is_inv(n) { I } else { O } }}
    else {
      let mut anf = self.nodes[nid::idx(n)].clone();
      if nid::is_inv(n) { anf.lo = nid::not(anf.lo) }
      anf }}

  fn vhl(&mut self, v:VID, hi0:NID, lo0:NID)->NID {
    // this is technically an xor operation, so if we want to call it directly,
    // we need to do the same logic as xor() to handle the 'not' bit.
    // note that the cache only ever contains 'raw' nodes, except hi=I
    let (hi,lo) = (if hi0 == I {I} else {nid::raw(hi0)}, nid::raw(lo0));
    let res =
      if let Some(&nid) = self.cache.get(&ANF{v, hi, lo}) { nid }
      else {
        let anf = ANF{ v, hi, lo };
        let nid = nid::nvi(v, self.nodes.len() as u32);
        self.cache.insert(anf, nid);
        self.nodes.push(anf);
        nid };
    let invert = if hi == I { nid::is_inv(lo) } else { nid::is_inv(hi) != nid::is_inv(lo) };
    if invert { nid::not( res )} else { res }}

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
    else {
      // x:(ab+c) * y:(aq+r) --> abq+abr+acq+cr --> a(b(q+r) + cq)+cr
      let ANF{ v:a, hi:b, lo:c } = self.fetch(x);
      let ANF{ v:p, hi:q, lo:r } = self.fetch(y);
      assert_eq!(a,p);
      // TODO: run in in parallel:
      let cr = self.and(c,r);
      let cq = self.and(c,q);
      let qxr = self.xor(q,r);
      let a = nid::nv(a);
      expr![self, ((a & ((b & qxr) ^ cq)) ^ cr)] }}

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
  assert_eq!(lo, I); // the final I never appears in the stored structure,
  // but if fetch is given an inverted nid, it inverts the lo branch.
}


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

#[test] fn test_anf_xor_inv() {
  let mut base = ANFBase::new(2);
  let a = base.var(0); let b = base.var(1);
  let axb = base.xor(a, b);
  let naxb = base.xor(nid::not(a), b);
  let axnb = base.xor(a, nid::not(b));
  let naxnb = base.xor(nid::not(a), nid::not(b));
  assert_eq!(naxnb, axb, "expect ~a ^ ~b == a^b");
  assert_eq!(axnb, naxb, "expect a ^ ~b ==  ~a ^ b");
  assert_eq!(axb, nid::not(naxb), "expect a ^ b ==  ~(~a ^ b)"); }


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
  assert_eq!(hi, b);
  assert_eq!(lo, O);}

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

// TODO: remove the argument to new()
#[test] fn test_anf_and_same_head() {
  // x:(ab+c) * y:(aq+r) --> abq+abr+acq+cr --> a(b(q+r) + cq)+cr
  let mut base = ANFBase::new(5);
  let a = base.var(0); let b = base.var(1); let c = base.var(2);
  let q = base.var(3); let r = base.var(4);
  let ab = base.and(a,b); let aq = base.and(a,q);
  let actual = expr![base, ((ab ^ c) & (aq ^ r))];
  let expected = expr![base, ((a & ((b & (q ^ r)) ^ (c&q)))^(c&r))];
  assert_eq!(expected, actual); }


#[test] fn test_anf_sub() {
  let mut base = ANFBase::new(6);
  let a = base.var(0); let b = base.var(1); let c = base.var(2);
  let x = base.var(3); let y = base.var(4); let z = base.var(5);
  let ctx = expr![base, ((a & b) ^ c) ];
  let xyz = expr![base, ((x & y) ^ z) ];
  assert_eq!(base.sub(nid::var(a), xyz, ctx), expr![base, ((xyz & b) ^ c)]);
  assert_eq!(base.sub(nid::var(b), xyz, ctx), expr![base, ((a & xyz) ^ c)]);}
