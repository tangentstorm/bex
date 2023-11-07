/**
 * Nested algebraic form. Represents an ANF polynomial.
 * The main difference between this and anf.rs is that this
 * version allows deferred evaluation.
 */
use crate::{NID, vid::VID};
use crate::ast::RawASTBase;

#[derive(Debug)]
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

  pub fn and(&mut self, x:NID, y:NID)->NID {
    // !! assume the AST was already simplified, so no simple cases
    self.push(NAF::And{ x, y}) }

  pub fn xor(&mut self, x:NID, y:NID)->NID {
    // !! again, assume AST is pre-simplified
    self.push(NAF::Xor{ x, y}) }

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
