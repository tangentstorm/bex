use crate::simp;
use crate::vhl::Vhl;
/**
 * Nested algebraic form. Represents an ANF polynomial.
 * The main difference between this and anf.rs is that this
 * version allows deferred evaluation.
 */
use crate::{NID, I, O, vid::VID};
use crate::ast::RawASTBase;
use crate::vid::VidOrdering;
use dashmap::DashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NAF {
  Vhl { v: VID, hi:NID, lo:NID },
  And { x: NID, y: NID },
  Xor { x: NID, y: NID }}

type NafHashMap<K,V> = DashMap<K,V,fxhash::FxBuildHasher>;



#[derive(Debug, Default)]
pub struct NAFBase {
  pub nodes: Vec<NAF>,
  cache: NafHashMap<NAF, NID> }

impl NAFBase {
  fn new()->Self { NAFBase{ nodes:vec![], cache: NafHashMap::default() } }

  /// insert a new node and and return a NID with its index.
  pub fn push(&mut self, x:NAF)->NID {
    let res = NID::ixn(self.nodes.len());
    self.nodes.push(x);
    res }

  pub fn get(&self, n:NID)->Option<NAF> {
    assert!(!n.is_inv(), "can't fetch inverted nids");
    if n.is_ixn() { self.nodes.get(n.idx()).cloned() }
    else if n.is_var() {Some(NAF::Vhl { v: n.vid(), hi:I, lo: O}) }
    else { None }}

  fn vhl(&mut self, v:VID, hi0:NID, lo0:NID)->NID {
    // !! exactly the same logic as anf::vhl(), but different hashmap/vhl
    // this is technically an xor operation, so if we want to call it directly,
    // we need to do the same logic as xor() to handle the 'not' bit.
    // note that the cache only ever contains 'raw' nodes, except hi=I
    if hi0 == I && lo0 == O { return NID::from_vid(v) }
    let (hi,lo) = (hi0, lo0.raw());
    let res =
      if let Some(nid) = self.cache.get(&NAF::Vhl{v, hi, lo}) { *nid }
      else {
        let vhl = NAF::Vhl { v, hi, lo };
        let nid = NID::from_vid_idx(v, self.nodes.len());
        self.cache.insert(vhl.clone(), nid);
        self.nodes.push(vhl);
        nid };
    if lo.is_inv() { !res } else { res }}

  pub fn and_vhls(&mut self, xi:NID, yi:NID)->NID {
    if let Some(res) = simp::and(xi, yi) { return res }
    let (a,b) = (xi.raw(), yi.raw());
    if let (Some(x), Some(y)) = (self.get(xi.raw()), self.get(yi.raw())) {
      if let (NAF::Vhl { v:_xv, hi:_xh, lo:_xl },
              NAF::Vhl { v:_yv, hi:_yh, lo:_yl }) = (x, y) {
        return match (xi.is_inv(), yi.is_inv()) {
          (true, true)=> { // case 0:  x:~a & y:~b ==> 1 ^ a ^ ab ^ b
            expr![ self,  (I ^ (a ^ ((a & b) ^ b)))] },
          (true, false) => { // case 1:  x:~a & y:b ==>  ab ^ b
            expr![ self, ((a & b) ^ b)] },
          (false, true) => { // case 2: x:a & y:~b ==> ab ^ a
            expr![ self, ((a & b) ^ a)] },
          (false, false) => // case 3: x:a & y:b ==> ab
            self.calc_and(xi, yi)}}}
    self.push(NAF::And{ x:xi, y:yi }) }

  pub fn fetch(&mut self, n:NID)->Vhl {
    match self.get(n).unwrap() {
      NAF::And { x:_, y:_ } => panic!("expected VHL, got AND"),
      NAF::Xor { x:_, y:_ } => panic!("expected VHL, got AND"),
      NAF::Vhl { v, hi, lo } => Vhl { v, hi, lo }}}

  pub fn calc_and(&mut self, x:NID, y:NID)->NID {
    let (xv, yv) = (x.vid(), y.vid());
    match xv.cmp_depth(&yv) {
      VidOrdering::Above =>
        // base case: x:a + y:(pq+r)  a<p<q, p<r  --> a(pq+r)
        if x.is_vid() { self.vhl(x.vid(), y, O) }
        else {
          //     x:(ab+c) * y:(pq+r)
          //  =  ab(pq) + ab(r) + c(pq) + c(r)
          //  =  a(b(pq+r)) + c(pq+r)
          //  =  a(by) + cy
          // TODO: these can all happen in parallel.
          let Vhl{v:a, hi:b, lo:c } = self.fetch(x);
          let hi = self.and(b, y);
          let lo = self.and(c, y);
          self.vhl(a, hi, lo)},
      VidOrdering::Below => self.and(y, x),
      VidOrdering::Level => {
        // x:(ab+c) * y:(aq+r) --> abq+abr+acq+cr --> a(b(q+r) + cq)+cr
        // xy = (ab+c)(aq+r)
        //       abaq + abr + caq +cr
        //       abq  + abr + acq + cr
        //       a(b(q+r)+cq)+cr
        let Vhl{ v:a, hi:b, lo:c } = self.fetch(x);
        let Vhl{ v:p, hi:q, lo:r } = self.fetch(y);
        assert_eq!(a,p);
        // TODO: run in in parallel:
        let cr = self.and(c,r);
        let cq = self.and(c,q);
        let qxr = self.xor(q,r);
        let n = NID::from_vid(a);
        expr![self, ((n & ((b & qxr) ^ cq)) ^ cr)] }}}

  pub fn xor_vhls(&mut self, xi:NID, yi:NID)->NID {
    if let Some(res) = simp::xor(xi, yi) { return res }
    if let (Some(x), Some(y)) = (self.get(xi.raw()), self.get(yi.raw())) {
      if let (NAF::Vhl { v:xv, hi:xhi, lo:xlo },
              NAF::Vhl { v:yv, hi:yhi, lo:ylo }) = (x, y) {
        let res = match xv.cmp_depth(&yv) {
          VidOrdering::Below => self.xor(yi, xi), // swap order
          VidOrdering::Above => {
            let lo = self.xor(xlo, yi);
            self.vhl(xv, xhi, lo)},
          VidOrdering::Level => {
            // x:(ab+c) + y:(aq+r) -> ab+c+aq+r -> ab+aq+c+r -> a(b+q)+c+r
            let v = xv; // since they're the same
            let hi = self.xor(xhi, yhi);
            let lo = self.xor(xlo, ylo);
            self.vhl(v, hi, lo)}};
        // handle the constant term:
        return if xi.is_inv() == yi.is_inv() { res } else { !res }}}
    self.push(NAF::Xor{ x:xi, y:yi }) }

  // these are for sub-expressions. they're named this way so expr![] works.
  pub fn xor(&mut self, xi: NID, yi:NID)->NID { self.xor_vhls(xi, yi)}
  pub fn and(&mut self, xi: NID, yi:NID)->NID { self.and_vhls(xi, yi)}

  // return the definition of the topmost node in the translated AST
  pub fn top(&self)->Option<&NAF> { self.nodes.last().clone() }}



// a packed AST is arranged so that we can do a bottom-up computation
// by iterating through the bits.
pub fn from_packed_ast(ast: &RawASTBase)->NAFBase {
  let mut res = NAFBase::new();
  // the NAFBase will have multiple nodes for each incoming AST node,
  // so keep a map of AST index -> NAF index
  let map = |n:NID, map:&Vec<NID>|->NID {
    if n.is_ixn() { let r = map[n.idx()]; if n.is_inv() { !r } else { r } }
    else { n }};
  let mut new_nids : Vec<NID> = vec![];
  for (i, bit) in ast.bits.iter().enumerate() {
    let (f, args) = bit.to_app();
    assert_eq!(2, args.len());
    let x = map(args[0], &new_nids);
    let y = map(args[1], &new_nids);
    let new = match f.to_fun().unwrap() {
      crate::ops::AND => res.and_vhls(x, y),
      crate::ops::XOR => res.xor_vhls(x, y),
      _ => panic!("no rule to translate bit #{:?} ({:?})", i, bit)};
    new_nids.push(new)}
  res }
