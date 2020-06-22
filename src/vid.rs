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
pub fn nov()->VID { VID { v:NoV }}
pub fn var(i:u32)->VID { VID { v: Var(i) }}
pub fn vir(i:u32)->VID { VID { v: Vir(i) }}
pub fn is_nov(v:VID)->bool { if let VID{ v:NoV } = v { true } else { false } }
pub fn is_var(v:VID)->bool { if let VID{ v:Var(_) } = v { true } else { false } }
pub fn is_vir(v:VID)->bool { if let VID{ v:Vir(_) } = v { true } else { false } }

impl VID {
  #[deprecated(note="VID scaffolding")]
  pub fn u(&self)->usize { match self.v {
    NoV => panic!("can't turn NoV into a number"),
    Vir(x) => x as usize,
    Var(x) => x as usize }}}
