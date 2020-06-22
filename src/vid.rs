/// Variable Identifiers
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
pub enum VID {
  NOVAR,
  Var(u32),
  Vir(u32)}
