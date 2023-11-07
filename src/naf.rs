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
    assert!(!n.is_const(), "const nids should be optimized out by now");
    if n.is_ixn() {
      assert!(!n.is_inv(), "can't fetch inverted nids");
      self.nodes.get(n.idx()).cloned() }
    // !! todo: uncomment to start building Vhl nodes up from variables.
    // else if n.is_var() { Some(NAF::Vhl { v: n.vid(), hi:I, lo: O}) }
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

  pub fn and(&mut self, xi:NID, yi:NID)->NID {
    // !! assume the AST was already simplified, so no simple cases
    if let (Some(x), Some(y)) = (self.get(xi.raw()), self.get(yi.raw())) {
      if let (NAF::Vhl { v:_xv, hi:_xh, lo:_xl },
              NAF::Vhl { v:_yv, hi:_yh, lo:_yl }) = (x, y) {
          todo!("implement AND. Handle .is_inv bits")}}
    self.push(NAF::And{ x:xi, y:yi }) }

  pub fn xor(&mut self, xi:NID, yi:NID)->NID {
    // !! again, assume AST is pre-simplified
    if let (Some(x), Some(y)) = (self.get(xi.raw()), self.get(yi.raw())) {
      if let (NAF::Vhl { v:xv, hi:xhi, lo:xlo },
              NAF::Vhl { v:yv, hi:yhi, lo:ylo }) = (x, y) {
        let res = match xv.cmp_depth(&yv) {
          VidOrdering::Below => self.xor(yi, xi), // swap order
          VidOrdering::Above => {
            let lo = self.sub_xor(xlo, yi);
            self.vhl(xv, xhi, lo)},
          VidOrdering::Level => {
            // x:(ab+c) + y:(aq+r) -> ab+c+aq+r -> ab+aq+c+r -> a(b+q)+c+r
            let v = xv; // since they're the same
            let hi = self.sub_xor(xhi, yhi);
            let lo = self.sub_xor(xlo, ylo);
            self.vhl(v, hi, lo)}};
        // handle the constant term:
        return if xi.is_inv() == yi.is_inv() { res } else { !res }}}
    self.push(NAF::Xor{ x:xi, y:yi }) }

  pub fn sub_xor(&mut self, xi: NID, yi:NID)->NID { self.xor(xi, yi)}
  pub fn sub_and(&mut self, xi: NID, yi:NID)->NID { self.and(xi, yi)}

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
      crate::ops::AND => res.and(x, y),
      crate::ops::XOR => res.xor(x, y),
      _ => panic!("no rule to translate bit #{:?} ({:?})", i, bit)};
    new_nids.push(new)}
  res }
