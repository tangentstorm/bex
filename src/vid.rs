//! Variable IDs (used interally by Base implementations)
use std::cmp::Ordering;
use std::fmt;

/// this will probably go away in favor of a bitmask at some point
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
enum VidEnum {
  // How I (eventually) want the ordering, to be (once biggest vars go on top:)
  T,        // Special meta-constant on which I and O branch.
  NoV,      // Special case for AST nodes not tied to a variable
  Var(u32), // Real Vars go in the middle, with biggest u32 on top.
  Vir(u32), // Virtual are "biggest", so go to the top.
}

#[derive(Eq, PartialEq)]
pub enum VidOrdering {
  Above,
  Level,
  Below }

use self::VidEnum::*;


#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VID { v:VidEnum }
pub const NOV:VID = VID::nov();
pub const TOP:VID = VID::top();

impl Default for VID { fn default()->Self { NOV }}

fn cmp_depth_idx(x:u32, y:&u32)->VidOrdering {
  match x.cmp(y) {
    Ordering::Less => VidOrdering::Below,
    Ordering::Equal => VidOrdering::Level,
    Ordering::Greater => VidOrdering::Above }}

impl VID {
  pub fn cmp_depth(&self, other: &Self) -> VidOrdering {
    use self::VidOrdering::*;
    match self.v {
      T => if other.v == T { Level } else { Below },
      NoV => match other.v {
        T   => Above,
        NoV => Level,
        _   => Below },
      Var(x) => match other.v {
        Vir(_) => Below,
        Var(y) => cmp_depth_idx(x,&y),
        NoV|T => Above },
      Vir(x) => match other.v {
        Var(_) => Above,
        Vir(y) => cmp_depth_idx(x,&y),
        NoV|T => Above }}}}

pub fn topmost(x:VID, y:VID)->VID { if x.is_above(&y) { x } else { y }}
pub fn botmost(x:VID, y:VID)->VID { if x.is_below(&y) { x } else { y }}
pub fn topmost_of3(x:VID, y:VID, z:VID)->VID { topmost(x, topmost(y, z)) }


impl VID {
  pub const fn top()->VID { VID { v:T }}
  pub const fn nov()->VID { VID { v:NoV }}
  pub const fn var(i:u32)->VID { VID { v: Var(i) }}
  pub const fn vir(i:u32)->VID { VID { v: Vir(i) }}
  pub fn is_top(&self)->bool { VID{ v:T } == *self }
  pub fn is_nov(&self)->bool { matches!(self, VID{ v:NoV }) }
  pub fn is_var(&self)->bool { matches!(self, VID{v:Var(_)}) }
  pub fn is_vir(&self)->bool { matches!(self, VID{v:Vir(_)}) }

  pub fn is_above(&self, other:&VID)->bool { self.cmp_depth(other) == VidOrdering::Above }
  pub fn is_below(&self, other:&VID)->bool { self.cmp_depth(other) == VidOrdering::Below }
  pub fn shift_up(&self)->VID {
    match self.v {
      NoV => panic!("VID::nov().shift_up() is undefined"),
      T => panic!("VID::top().shift_up() is undefined"), //VID::var(0),
      // these two might panic on over/underflow:
      Var(x) => VID::var(x+1),
      Vir(x) => VID::vir(x+1)}}

  pub fn var_ix(&self)->usize {
    if let Var(x) = self.v { x as usize } else { panic!("var_ix({:?})", self) }}

  pub fn vir_ix(&self)->usize {
    if let Vir(x) = self.v { x as usize } else { panic!("vir_ix({:?})", self) }}

  pub fn vid_ix(&self)->usize { match self.v {
    T => panic!("x.vid_ix() makes no sense when x==T. Test with nid::is_const first."),
    NoV => panic!("x.vid_ix() makes no sense when x==VID::NoV. Test with x.is_nov first."),
    Var(x) | Vir(x) => x as usize }}

  pub fn bitmask(&self)->u64 { match self.v {
    NoV|T => 0,
    Var(x) | Vir(x) => if x < 64 { 1 << x as u64 } else { 0 }}}}


/// Pretty-printer for NIDS that reveal some of their internal data.
impl fmt::Display for VID {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self.v {
      T => write!(f, "T"),
      NoV => write!(f, "NoV"),
      Var(x) => write!(f, "x{:X}", x),
      Vir(x) => write!(f, "v{:X}", x) }}}

/// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
impl fmt::Debug for VID { // for test suite output
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}


impl VID {
  #[deprecated(note="VID scaffolding")]
  pub fn u(&self)->usize { match self.v {
    T  =>  536870912, // 1<<29, same as nid::T,
    NoV => panic!("can't turn NoV into a number"),
    Var(x) => x as usize,
    Vir(x) => x as usize }}}

/// this is only so I can order ops. VID should otherwise always be
/// compared with is_above / iS_below or cmp_depth, for clarity.
impl Ord for VID {
  fn cmp(&self, other: &Self)-> Ordering {
    match self.cmp_depth(other) {
      VidOrdering::Above => Ordering::Less,
      VidOrdering::Level => Ordering::Equal,
      VidOrdering::Below => Ordering::Greater}}}

impl PartialOrd for VID {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
      Some(self.cmp(other))}}

/// predefined consts for VIDS (mostly for tests)
#[allow(non_upper_case_globals)]
pub mod named {
  use super::VID;
  pub const x0:VID = VID::var(0);
  pub const x1:VID = VID::var(1);
  pub const x2:VID = VID::var(2);
  pub const x3:VID = VID::var(3);
  pub const x4:VID = VID::var(4);
  pub const v0:VID = VID::vir(0);
  pub const v1:VID = VID::vir(1);
  pub const v2:VID = VID::vir(2);
  pub const v3:VID = VID::vir(3);
  pub const v4:VID = VID::vir(4);
}
