/// bex: a boolean expression library for rust
use std::collections::HashMap;

// abstract bits and bit base types (trait Base)
pub type VID = usize;
pub type NID = usize;
pub type SID = usize; // canned substition
pub type SUB = HashMap<VID,NID>;

pub const GONE:usize = 1<<63;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Op {
  O, I, Var(VID), Not(NID), And(NID,NID), Or(NID,NID), Xor(NID,NID),
  // Eql(NID,NID), LT(Nid,Nid),
  Ch(NID, NID, NID), Mj(NID, NID, NID) }

/// outside the base, you deal only with opaque references.
/// inside, it could be stored any way we like.
pub trait Base {
  fn o(&self)->NID;
  fn i(&self)->NID;
  fn var(&mut self, v:VID)->NID;
  fn def(&mut self, s:String, i:u32)->NID;
  fn tag(&mut self, n:NID, s:String)->NID;
  fn not(&mut self, x:NID)->NID;
  fn and(&mut self, x:NID, y:NID)->NID;
  fn xor(&mut self, x:NID, y:NID)->NID;
  fn or(&mut self, x:NID, y:NID)->NID;
  #[cfg(todo)] fn mj(&mut self, x:NID, y:NID, z:NID)->NID;
  #[cfg(todo)] fn ch(&mut self, x:NID, y:NID, z:NID)->NID;
  fn when(&mut self, v:VID, val:NID, nid:NID)->NID;
  fn sid(&mut self, kv:SUB) -> SID;
  fn sub(&mut self, x:NID, s:SID)->NID; // set many inputs at once
  fn nid(&mut self, op:Op)->NID;   // given an op, return a nid
}


// TODO: put these elsewhere.
#[cfg(todo)] pub fn order<T:PartialOrd>(x:T, y:T)->(T,T) { if x < y { (x,y) } else { (y,x) }}
#[cfg(todo)] pub fn order3<T:Ord+Clone>(x:T, y:T, z:T)->(T,T,T) {
  let mut res = [x,y,z];
  res.sort();
  (res[0].clone(), res[1].clone(), res[2].clone())}

