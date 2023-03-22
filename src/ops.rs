//! Tools for constructing boolean expressions using NIDs as logical operations.
use {nid::NID, vid::VID};
use std::slice::Iter;

///! A sequence of operations.
///! Currently, RPN is the only format, but I made this an enum
///! to provide a little future-proofing.
#[derive(PartialOrd, PartialEq, Eq, Hash, Debug, Clone)]
pub enum Ops { RPN(Vec<NID>) }
impl Ops {
  ///! Again, just for future proofing.
  pub fn to_rpn(&self)->Iter<'_, NID> {
    match self {
      Ops::RPN(vec) => vec.iter() }}

  /// ensure that last item is a function of n inputs,
  /// len is n+1, and first n inputs are not inverted.
  pub fn norm(&self)->Ops {
    let mut rpn:Vec<NID> = self.to_rpn().cloned().collect();
    let f0 = rpn.pop().expect("norm() expects at least one f-nid");
    assert!(f0.is_fun());
    let ar = f0.arity().unwrap();
    assert_eq!(ar, rpn.len() as u8);

    // if any of the input vars are negated, update the function to
    // negate the corresponding argument. this way we can just always
    // branch on the raw variable.
    let mut bits:u8 = 0;
    for (i,nid) in rpn.iter_mut().enumerate() { if nid.is_inv() { bits |= 1 << i;  *nid = !*nid; }}
    let f = f0.fun_flip_inputs(bits);
    rpn.push(f);
    Ops::RPN(rpn)}}

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

#[test] fn test_flip_and() {
  assert_eq!(AND.tbl().unwrap()                    & 0b1111, 0b0001 );
  assert_eq!(AND.fun_flip_inputs(1).tbl().unwrap() & 0b1111, 0b0010 );
  assert_eq!(AND.fun_flip_inputs(2).tbl().unwrap() & 0b1111, 0b0100 );
  assert_eq!(AND.fun_flip_inputs(3).tbl().unwrap() & 0b1111, 0b1000 );}

#[test] fn test_flip_vel() {
  assert_eq!(VEL.tbl().unwrap()                    & 0b1111, 0b0111 );
  assert_eq!(VEL.fun_flip_inputs(1).tbl().unwrap() & 0b1111, 0b1011 );
  assert_eq!(VEL.fun_flip_inputs(2).tbl().unwrap() & 0b1111, 0b1101 );
  assert_eq!(VEL.fun_flip_inputs(3).tbl().unwrap() & 0b1111, 0b1110 );}

#[test] fn test_flip_xor() {
  assert_eq!(XOR.tbl().unwrap()                    & 0b1111, 0b0110 );
  assert_eq!(XOR.fun_flip_inputs(1).tbl().unwrap() & 0b1111, 0b1001 );
  assert_eq!(XOR.fun_flip_inputs(2).tbl().unwrap() & 0b1111, 0b1001 );
  assert_eq!(XOR.fun_flip_inputs(3).tbl().unwrap() & 0b1111, 0b0110 );}

#[test] fn test_norm() {
  assert_eq!(AND.tbl().unwrap()                    & 0b1111, 0b0001 );
  let ops = Ops::RPN(vec![NID::var(0), !NID::var(1), AND]);
  let mut rpn:Vec<NID> = ops.norm().to_rpn().cloned().collect();
  let f = rpn.pop().unwrap();
  assert_eq!(2, f.arity().unwrap());
  assert_eq!(f.tbl().unwrap() & 0b1111, 0b0100);
  assert_eq!(rpn, vec![NID::var(0), NID::var(1)]);}
