///! This module provides tools for constructing boolean expressions using
///! NIDs as logical operations.
use {nid::NID, vid::VID};
use std::slice::Iter;

///! A sequence of operations.
///! Currently, RPN is the only format, but I made this an enum
///! to provide a little future-proofing.
#[derive(PartialOrd, PartialEq, Debug)]
pub enum Ops { RPN(Vec<NID>) }
impl Ops {
  ///! Again, just for future proofing.
  pub fn to_rpn(&self)->Iter<'_, NID> {
    match self {
      Ops::RPN(vec) => vec.iter(),
      _ => todo!("to_rpn") }}}

/// constructor for rpn
pub fn rpn(xs:&[NID])->Ops { Ops::RPN(xs.to_vec()) }

/// x0 & x1
pub const AND:NID = NID::fun(2,0b0001);

/// x0 ^ x1
pub const XOR:NID = NID::fun(2,0b0110);

/// x0 | x1   (vel is the latin word for 'inclusive or', and the origin of the "∨" symbol in logic)
pub const VEL:NID = NID::fun(2,0b0111);

/// !(x0 | x1)
pub const NOR:NID = NID::fun(2,0b1000);

/// x0 implies x1  (x0 <= x1)
pub const IMP:NID = NID::fun(2,0b1011);

/// convenience trait that allows us to mix vids and nids
/// freely when constructing expressions.
pub trait ToNID { fn to_nid(&self)->NID; }
impl ToNID for NID { fn to_nid(&self)->NID { *self }}
impl ToNID for VID { fn to_nid(&self)->NID { NID::from_vid(*self) }}

/// construct the expression `x AND y`
pub fn and<X:ToNID,Y:ToNID>(x:X,y:Y)->Ops { rpn(&[x.to_nid(), y.to_nid(), AND]) }

/// construct the expression `x XOR y`
pub fn xor<X:ToNID,Y:ToNID>(x:X,y:Y)->Ops { rpn(&[x.to_nid(), y.to_nid(), XOR]) }

/// construct the expression `x VEL y` ("x or y")
pub fn vel<X:ToNID,Y:ToNID>(x:X,y:Y)->Ops { rpn(&[x.to_nid(), y.to_nid(), VEL]) }

/// construct the expression `x IMP y` ("x implies y")
pub fn imp<X:ToNID,Y:ToNID>(x:X,y:Y)->Ops { rpn(&[x.to_nid(), y.to_nid(), IMP]) }
