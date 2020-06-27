/// Variable Identifiers
use std::cmp::Ordering;

/// this will probably go away in favor of a bitmask at some point
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
enum VidEnum {
  // How I (eventually) want the ordering, to be (once biggest vars go on top:)
  T,        // Special meta-constant on which I and O branch.
  NoV,      // Special case for AST nodes not tied to a variable
  Var(u32), // Real Vars go in the middle, with biggest u32 on top.
  Vir(u32), // Virtual are "biggest", so go to the top.
}

use self::VidEnum::*;


#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
pub struct VID { v:VidEnum }

impl Ord for VID {
  fn cmp(&self, other: &Self) -> Ordering {
    match self.v {
      T => if other.v == T { Ordering::Equal } else { Ordering::Greater },
      NoV => match other.v {
        T   => Ordering::Less,
        NoV => Ordering::Equal,
        _   => Ordering::Greater },
      Var(x) => match other.v {
        Vir(_) => Ordering::Greater,
        Var(y) => x.cmp(&y),
        NoV|T    => Ordering::Less },
      Vir(x) => match other.v {
        Var(_) => Ordering::Less,
        Vir(y) => x.cmp(&y),
        NoV|T => Ordering::Less
      }
    }
  }
}

impl PartialOrd for VID {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
      Some(self.cmp(other))
  }
}

impl VID {
  pub fn top()->VID { VID { v:T }}
  pub fn nov()->VID { VID { v:NoV }}
  pub fn var(i:u32)->VID { VID { v: Var(i) }}
  pub fn vir(i:u32)->VID { VID { v: Vir(i) }}
  pub fn is_top(&self)->bool { VID{ v:T } == *self }
  pub fn is_nov(&self)->bool { if let VID{ v:NoV } = self { true } else { false } }
  pub fn is_var(&self)->bool { if let VID{ v:Var(_) } = self { true } else { false } }
  pub fn is_vir(&self)->bool { if let VID{ v:Vir(_) } = self { true } else { false } }

  pub fn is_above(&self, other:&VID)->bool { self < other }
  pub fn is_below(&self, other:&VID)->bool { self > other }
  pub fn shift_up(&self)->VID {
    match self.v {
      NoV => panic!("VID::nov().shift_up() is undefined"),
      T => panic!("VID::top().shift_up() is undefined"), //VID::var(0),
      // these two might panic on underflow:
      Var(x) => VID::var(x-1),
      Vir(x) => VID::vir(x-1) }}

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
    Var(x) | Vir(x) => if x < 64 { 1 << x as u64 } else { 0 }}}

  #[deprecated(note="VID scaffolding")]
  pub fn u(&self)->usize { match self.v {
    T  =>  536870912, // 1<<29, same as nid::T,
    NoV => panic!("can't turn NoV into a number"),
    Var(x) => x as usize,
    Vir(x) => x as usize }}}
