/// Variable Identifiers
// !! EVID is private scaffolding. Will probably go away.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
enum EVID {
  NoV,
  Var(u32),
  Vir(u32)}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
pub struct VID { v:EVID }

use self::EVID::*;

impl VID {

  pub fn nov()->VID { VID { v:NoV }}
  pub fn var(i:u32)->VID { VID { v: Var(i) }}
  pub fn vir(i:u32)->VID { VID { v: Vir(i) }}
  pub fn is_nov(&self)->bool { if let VID{ v:NoV } = self { true } else { false } }
  pub fn is_var(&self)->bool { if let VID{ v:Var(_) } = self { true } else { false } }
  pub fn is_vir(&self)->bool { if let VID{ v:Vir(_) } = self { true } else { false } }

  pub fn var_ix(&self)->usize {
    if let Var(x) = self.v { x as usize } else { panic!("var_ix({:?})", self) }}

  pub fn vir_ix(&self)->usize {
    if let Vir(x) = self.v { x as usize } else { panic!("vir_ix({:?})", self) }}

  pub fn vid_ix(&self)->usize { match self.v {
    NoV => panic!("x.vid_ix() makes no sense when x==VID::NoV. Test with x.is_nov first."),
    Var(x) | Vir(x) => x as usize }}

  pub fn bitmask(&self)->u64 { match self.v {
    NoV => 0,
    Var(x) | Vir(x) => if x < 64 { 1 << x as u64 } else { 0 }}}

  #[deprecated(note="VID scaffolding")]
  pub fn u(&self)->usize { match self.v {
    NoV => panic!("can't turn NoV into a number"),
    Var(x) => x as usize,
    Vir(x) => x as usize }}}
