///! (Var, Hi, Lo) triples
use nid::NID;
use vid::VID;

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


pub trait HiLoBase {
  fn get_hilo(&self, n:NID)->Option<HiLo>;
}
