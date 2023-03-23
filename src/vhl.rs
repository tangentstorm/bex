//! (Var, Hi, Lo) triples
use std::collections::BinaryHeap;
use std::collections::HashSet;
use dashmap::DashMap;
use nid::NID;
use vid::VID;

type VhlHashMap<K,V> = DashMap<K,V,fxhash::FxBuildHasher>;

#[derive(Debug,Default,Clone)]
struct VhlVec<T>{ pub vec: boxcar::Vec<T> }


/// Simple Hi/Lo pair stored internally when representing nodes.
/// All nodes with the same branching variable go in the same array, so there's
/// no point duplicating it.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Default)]
pub struct HiLo {pub hi:NID, pub lo:NID}

impl HiLo {
  /// constructor
  pub fn new(hi:NID, lo:NID)->HiLo { HiLo { hi, lo } }

  /// apply the not() operator to both branches
  #[inline] pub fn invert(self)-> HiLo { HiLo{ hi: !self.hi, lo: !self.lo }}

  pub fn get_part(&self, which:HiLoPart)->NID {
    if which == HiLoPart::HiPart { self.hi } else { self.lo }} }

impl std::ops::Not for HiLo {
  type Output = HiLo;
  fn not(self)-> HiLo {HiLo { hi:!self.hi, lo: !self.lo }}}


/// Vhl (for when we really do need the variable)
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
pub struct Vhl {pub v:VID, pub hi:NID, pub lo:NID}

impl Vhl {
  pub fn new(v: VID, hi:NID, lo:NID)->Vhl { Vhl{ v, hi, lo } }
  pub fn hilo(&self)->HiLo { HiLo{ hi:self.hi, lo: self.lo } }}

impl std::ops::Not for Vhl {
  type Output = Vhl;
  fn not(self)->Vhl { Vhl { v:self.v, hi:!self.hi, lo: !self.lo }}}


/// Enum for referring to the parts of a HiLo (for WIP).
#[derive(PartialEq,Debug,Copy,Clone)]
pub enum HiLoPart { HiPart, LoPart }

/// a deconstructed Vhl (for WIP)
#[derive(Default,PartialEq,Debug,Copy,Clone)]
pub struct VhlParts{
  pub v:VID,
  pub hi:Option<NID>,
  pub lo:Option<NID>,
  pub invert:bool}

  impl VhlParts {
    pub fn hilo(&self)->Option<HiLo> {
      if let (Some(hi), Some(lo)) = (self.hi, self.lo) { Some(HiLo{hi,lo}) }
      else { None }}
    pub fn set_part(&mut self, part:HiLoPart, v:Option<NID>) {
      if part == HiLoPart::HiPart { self.hi = v }
      else { self.lo = v }}}


pub trait Walkable {

  /// walk nodes in graph for nid n recursively, without revisiting shared nodes
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>, topdown:bool)
  where F: FnMut(NID,VID,NID,NID);

  fn walk<F>(&self, n:NID, f:&mut F) where F: FnMut(NID,VID,NID,NID) {
    let mut seen = HashSet::new();
    self.step(n, f, &mut seen, true)}

  /// same as walk, but visit children before firing the function.
  /// note that this walks from "left to right" ("lo' to "hi")
  /// and bottom to top, starting from the leftmost node.
  /// if you want the bottommost nodes to come first, use self.as_heap(n)
  fn walk_up<F>(&self, n:NID, f:&mut F) where F: FnMut(NID,VID,NID,NID) {
    let mut seen = HashSet::new();
    self.step(n, f, &mut seen, false)}

  /// this is meant for walking nodes ordered by variables from bottom to top.
  /// it's deprecated because the whole thing ought to be replaced by a nice iterator
  /// (also, it's not clear to me why the derived Ord for Vhl doesn't require Reverse() here)
  #[deprecated]
  fn as_heap(&self, n:NID)->BinaryHeap<(Vhl, NID)> {
    let mut result = BinaryHeap::new();
    self.walk_up(n, &mut |nid, v, hi, lo| result.push((Vhl{ v, hi, lo }, nid)));
    result }}


pub trait HiLoBase {
  fn get_hilo(&self, n:NID)->Option<HiLo>; }


#[derive(Debug, Default, Clone)]
pub struct HiLoCache {
  /// variable-agnostic hi/lo pairs for individual bdd nodes.
  hilos: VhlVec<HiLo>,
  /// reverse map for hilos.
  index: VhlHashMap<HiLo, usize>}


impl HiLoCache {

  pub fn new()->Self { Self::default() }

  // TODO: ->Option<HiLo>, and then impl HiLoBase
  #[inline] pub fn get_hilo(&self, n:NID)->HiLo {
    assert!(!n.is_lit());
    let res = self.hilos.vec[n.idx()];
    if n.is_inv() { res.invert() } else { res }}

  #[inline] pub fn get_node(&self, v:VID, hl0:HiLo)-> Option<NID> {
    let inv = hl0.lo.is_inv();
    let hl1 = if inv { hl0.invert() } else { hl0 };
    if let Some(x) = self.index.get(&hl1) {
      // !! maybe this should be an assertion, and callers
      //   should be adjusted to avoid asking for ill-formed Vhl triples?
      // (without this check, we potentially break the contract of always
      //  returning a NID that represents a valid Bdd)
      if hl1.hi.vid().is_below(&v) && hl1.lo.vid().is_below(&v) {
        let nid = NID::from_vid_idx(v, *x);
        return Some(if inv { !nid  } else { nid }) }}
    None }

  #[inline] pub fn insert(&self, v:VID, hl0:HiLo)->NID {
    let inv = hl0.lo.is_inv();
    let hilo = if inv { hl0.invert() } else { hl0 };
    let ix:usize =
      if let Some(ix) = self.index.get(&hilo) { *ix }
      else {
        let ix = self.hilos.vec.push(hilo);
        self.index.insert(hilo, ix);
        ix };
    let res = NID::from_vid_idx(v, ix);
    if inv { !res } else { res } }}
