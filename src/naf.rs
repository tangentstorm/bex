/**
 * Nested algebraic form. Represents an ANF polynomial.
 * The main difference between this and anf.rs is that this
 * version allows deferred evaluation.
 */
use crate::{NID, vid::VID};
use crate::ast::RawASTBase;

#[derive(Debug, Clone)]
pub enum NAF {
  Vhl { v: VID, hi:NID, lo:NID },
  And { x: NID, y: NID },
  Xor { x: NID, y: NID }}



#[derive(Debug, Default)]
pub struct NAFBase {
  pub nodes: Vec<NAF>}

impl NAFBase {
  fn new()->Self { NAFBase{ nodes:vec![] } }

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
    // else if n.is_var() { Some(NAF::Vhl { v: n.vid(), hi:NID::i(), lo: NID::o()}) }
    else { None }}

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
      if let (NAF::Vhl { v:_xv, hi:_xh, lo:_xl },
              NAF::Vhl { v:_yv, hi:_yh, lo:_yl }) = (x, y) {
          todo!("implement XOR. Handle .is_inv bits")}}
    self.push(NAF::Xor{ x:xi, y:yi }) }

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
