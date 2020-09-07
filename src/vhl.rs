///! (Var, Hi, Lo) triples
use std::collections::HashSet;
use nid::{NID, IDX};
use vid::VID;

pub type VHLHashMap<K,V> = hashbrown::hash_map::HashMap<K,V>;


/// Simple Hi/Lo pair stored internally when representing nodes.
/// All nodes with the same branching variable go in the same array, so there's
/// no point duplicating it.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HiLo {pub hi:NID, pub lo:NID}

impl HiLo {
  /// constructor
  pub fn new(hi:NID, lo:NID)->HiLo { HiLo { hi, lo } }

  /// apply the not() operator to both branches
  #[inline] pub fn invert(self)-> HiLo { HiLo{ hi: !self.hi, lo: !self.lo }}

  pub fn get_part(&self, which:HiLoPart)->NID {
    if which == HiLoPart::HiPart { self.hi } else { self.lo }} }


/// Enum for referring to the parts of a HiLo (for WIP).
#[derive(PartialEq,Debug,Copy,Clone)]
pub enum HiLoPart { HiPart, LoPart }


/// a deconstructed VHL (for WIP)
#[derive(PartialEq,Debug,Copy,Clone)]
pub struct VHLParts{
  pub v:VID,
  pub hi:Option<NID>,
  pub lo:Option<NID>,
  pub invert:bool}

impl VHLParts {
  pub fn hilo(&self)->Option<HiLo> {
    if let (Some(hi), Some(lo)) = (self.hi, self.lo) { Some(HiLo{hi,lo}) }
    else { None }}}


pub trait Walkable {

  /// walk nodes in graph for nid n recursively, without revisiting shared nodes
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>, topdown:bool)
  where F: FnMut(NID,VID,NID,NID);

  fn walk<F>(&self, n:NID, f:&mut F) where F: FnMut(NID,VID,NID,NID) {
    let mut seen = HashSet::new();
    self.step(n, f, &mut seen, true)}

  /// same as walk, but visit children before firing the function.
  fn walk_up<F>(&self, n:NID, f:&mut F) where F: FnMut(NID,VID,NID,NID) {
    let mut seen = HashSet::new();
    self.step(n, f, &mut seen, false)}}


pub trait HiLoBase {
  fn get_hilo(&self, n:NID)->Option<HiLo>;
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HiLoCache {
  /// variable-agnostic hi/lo pairs for individual bdd nodes.
  hilos: Vec<HiLo>,
  /// reverse map for hilos.
  index: VHLHashMap<HiLo, IDX>,
  /// variable-specific memoization. These record (v,hilo) lookups.
  /// There shouldn't be any need for this, but an undiagnosed
  /// bug prevents me from removing it.
  vindex: VHLHashMap<(VID,HiLo), IDX>}

// TODO: remove vindex. There's no reason to store (x1,y,z) separately from (y,z).
// !! Previously, in test_nano_bdd, I wind up with a node branching on x2
//      to another node also branching on x2.
//    As of 2020-07-10, the new problem is just that test_multi_bdd
//      and test_nano_bdd start taking minutes to run.
//    I can't currently think of a reason vindex[(vX,hilo)] shouldn't behave
//      exactly the same as vindex[(vY,hilo)] and thus == index[hilo], but I'm
//      obviously missing something. :/
//    It could be a bug in replace(), but that's a simple function.
//    More likely, it's something to do with the recent/stable dichotomy in BddSwarm,
//      or simply the fact that each worker has its own recent state and they're getting
//      out of sync.


impl HiLoCache {

  pub fn new()->Self {
    HiLoCache {
      hilos: vec![],
      index: VHLHashMap::default(),
      vindex: VHLHashMap::default()}}

  // TODO: ->Option<HiLo>, and then impl HiLoBase
  #[inline] pub fn get_hilo(&self, n:NID)->HiLo {
    assert!(!n.is_lit());
    let res = self.hilos[n.idx()];
    if n.is_inv() { res.invert() } else { res }}

  #[inline] pub fn get_node(&self, v:VID, hl0:HiLo)-> Option<NID> {
    let inv = hl0.lo.is_inv();
    let hl1 = if inv { hl0.invert() } else { hl0 };
    let to_nid = |&ix| NID::from_vid_idx(v, ix);
    let res = self.vindex.get(&(v, hl1)).map(to_nid);
    // let res = if res.is_none() { self.index.get(&hl1).map(to_nid) } else { res };
    if inv { res.map(|nid| !nid ) } else { res }}

  #[inline] pub fn insert(&mut self, v:VID, hl0:HiLo)->NID {
    let inv = hl0.lo.is_inv();
    let hilo = if inv { hl0.invert() } else { hl0 };
    let ix:IDX =
      if let Some(&ix) = self.index.get(&hilo) { ix }
      else {
        let ix = self.hilos.len() as IDX;
        self.hilos.push(hilo);
        self.index.insert(hilo, ix);
        self.vindex.insert((v,hilo), ix);
        ix };
    let res = NID::from_vid_idx(v, ix);
    if inv { !res } else { res } }}

impl Default for HiLoCache {
  fn default() -> Self { Self::new() }}
