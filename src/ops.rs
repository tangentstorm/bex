//! Tools for constructing boolean expressions using NIDs as logical operations.
use crate::{NID, Fun, nid::NidFun, vid::VID};
use std::slice::Iter;

/// A sequence of operations.
/// Currently, RPN is the only format, but I made this an enum
/// to provide a little future-proofing.
#[derive(PartialOrd, PartialEq, Eq, Hash, Debug, Clone)]
pub enum Ops { RPN(Vec<NID>) }
impl Ops {
  ///! Again, just for future proofing.
  pub fn to_rpn(&self)->Iter<'_, NID> {
    match self {
      Ops::RPN(vec) => vec.iter() }}

  // return as a function application, where the first item is the function
  pub fn to_app(&self)->(NID, Vec<NID>) {
    match self {
      Ops::RPN(vec) => {
        let mut v = vec.clone();
        let f = v.pop().expect("to_app() expects at least one f-nid");
        assert!(f.is_fun());
        (f,v) }}}

  /// ensure that last item is a function of n inputs,
  /// len is n+1, and first n inputs are not inverted.
  pub fn norm(&self)->Ops {
    let mut rpn:Vec<NID> = self.to_rpn().cloned().collect();
    let f0 = rpn.pop().expect("norm() expects at least one f-nid").to_fun().unwrap();
    let ar = f0.arity();
    assert_eq!(ar, rpn.len() as u8);

    // if any of the input vars are negated, update the function to
    // negate the corresponding argument. this way we can just always
    // branch on the raw variable.
    let mut bits:u8 = 0;
    for (i,nid) in rpn.iter_mut().enumerate() { if nid.is_inv() { bits |= 1 << i;  *nid = !*nid; }}
    let f = f0.when_flipped(bits);
    rpn.push(f.to_nid());
    Ops::RPN(rpn)}}

/// constructor for rpn
pub fn rpn(xs:&[NID])->Ops { Ops::RPN(xs.to_vec()) }

pub mod sig {

  macro_rules! signals {
    ($($ids:ident : $exs:expr),+ $(,)?) => { signals![@ $($ids : $exs),+]; };
    (@) => {};
    (@ $id:ident : $ex:expr $(, $ids:ident : $exs:expr)*) => {
      pub const $id:u32 = $ex;
      signals![@ $($ids : $exs),*]; };}

  signals! {
    // constant signals
    K0:0x00000000, K1:0xffffffff,

    // input bit signals (raw and inverted)
    A:0x55555555, RX0:A, NX0:!A,
    B:0x33333333, RX1:B, NX1:!B,
    C:0x0f0f0f0f, RX2:C, NX2:!C,
    D:0x00ff00ff, RX3:D, NX3:!D,
    E:0x0000ffff, RX4:E, NX4:!E,

    // NB. all possible 5-bit anf terms (except the singleton terms above)
    // ] (,': ',([,'&',])/,','"_)S:0 terms=:(#~1<#S:0)/:~'ABCDE'{~L:0<@I.#:i.32
    AB: A&B,        ABC: A&B&C,     ABCD: A&B&C&D,  ABCDE: A&B&C&D&E,
    ABCE: A&B&C&E,  ABD: A&B&D,     ABDE: A&B&D&E,  ABE: A&B&E,
    AC: A&C,        ACD: A&C&D,     ACDE: A&C&D&E,  ACE: A&C&E,
    AD: A&D,        ADE: A&D&E,     AE: A&E,        BC: B&C,
    BCD: B&C&D,     BCDE: B&C&D&E,  BCE: B&C&E,     BD: B&D,
    BDE: B&D&E,     BE: B&E,        CD: C&D,        CDE: C&D&E,
    CE: C&E,        DE: D&E,

    // same ANF terms but now written in "big endian" style
    // ([,': ',],','"_)/@:>"1(\:~L:0 terms),.terms
    BA: AB,       CBA: ABC,    DCBA: ABCD,   EDCBA: ABCDE,
    ECBA: ABCE,   DBA: ABD,    EDBA: ABDE,   EBA: ABE,
    CA: AC,       DCA: ACD,    EDCA: ACDE,   ECA: ACE,
    DA: AD,       EDA: ADE,    EA: AE,       CB: BC,
    DCB: BCD,     EDCB: BCDE,  ECB: BCE,     DB: BD,
    EDB: BDE,     EB: BE,      DC: CD,       EDC: CDE,
    EC: CE,       ED: DE,

    // the (non-degenerate) dyadic boolean functions
    AND: A&B,    NAND: !(A&B),
    XOR: A^B,    IFF: !XOR,
    OR: A|B,     NOR: !OR,
    LT: !A&B,    GT: A&!B,
    LTE: !GT,    GTE: !LT,

    // and some aliases:
    VEL:OR,     IMP: LTE,
    NE: XOR,    EQ: IFF }}

/// x0 & x1
pub const AND:NidFun = NID::fun(2,sig::AND);

/// x0 ^ x1
pub const XOR:NidFun = NID::fun(2,sig::XOR);

/// x0 | x1   (vel is the latin word for 'inclusive or', and the origin of the "∨" symbol in logic)
pub const VEL:NidFun = NID::fun(2,sig::VEL);

/// !(x0 | x1)
pub const NOR:NidFun = NID::fun(2,sig::NOR);

/// x0 implies x1  (x0 <= x1)
pub const IMP:NidFun = NID::fun(2,sig::IMP);

/// convenience trait that allows us to mix vids and nids
/// freely when constructing expressions.
pub trait ToNID { fn to_nid(&self)->NID; }
impl ToNID for NID { fn to_nid(&self)->NID { *self }}
impl ToNID for VID { fn to_nid(&self)->NID { NID::from_vid(*self) }}

/// construct the expression `x AND y`
pub fn and<X:ToNID,Y:ToNID>(x:X,y:Y)->Ops { rpn(&[x.to_nid(), y.to_nid(), AND.to_nid()]) }

/// construct the expression `x XOR y`
pub fn xor<X:ToNID,Y:ToNID>(x:X,y:Y)->Ops { rpn(&[x.to_nid(), y.to_nid(), XOR.to_nid()]) }

/// construct the expression `x VEL y` ("x or y")
pub fn vel<X:ToNID,Y:ToNID>(x:X,y:Y)->Ops { rpn(&[x.to_nid(), y.to_nid(), VEL.to_nid()]) }

/// construct the expression `x IMP y` ("x implies y")
pub fn imp<X:ToNID,Y:ToNID>(x:X,y:Y)->Ops { rpn(&[x.to_nid(), y.to_nid(), IMP.to_nid()]) }

#[test] fn test_flip_and() {
  assert_eq!(AND.tbl()                 & 0b1111, 0b0001 );
  assert_eq!(AND.when_flipped(1).tbl() & 0b1111, 0b0010 );
  assert_eq!(AND.when_flipped(2).tbl() & 0b1111, 0b0100 );
  assert_eq!(AND.when_flipped(3).tbl() & 0b1111, 0b1000 );}

#[test] fn test_flip_vel() {
  assert_eq!(VEL.tbl()                 & 0b1111, 0b0111 );
  assert_eq!(VEL.when_flipped(1).tbl() & 0b1111, 0b1011 );
  assert_eq!(VEL.when_flipped(2).tbl() & 0b1111, 0b1101 );
  assert_eq!(VEL.when_flipped(3).tbl() & 0b1111, 0b1110 );}

#[test] fn test_flip_xor() {
  assert_eq!(XOR.tbl()                 & 0b1111, 0b0110 );
  assert_eq!(XOR.when_flipped(1).tbl() & 0b1111, 0b1001 );
  assert_eq!(XOR.when_flipped(2).tbl() & 0b1111, 0b1001 );
  assert_eq!(XOR.when_flipped(3).tbl() & 0b1111, 0b0110 );}

#[test] fn test_norm() {
  assert_eq!(AND.tbl()                 & 0b1111, 0b0001 );
  let ops = Ops::RPN(vec![NID::var(0), !NID::var(1), AND.to_nid()]);
  let mut rpn:Vec<NID> = ops.norm().to_rpn().cloned().collect();
  let f = rpn.pop().unwrap().to_fun().unwrap();
  assert_eq!(2, f.arity());
  assert_eq!(f.tbl() & 0b1111, 0b0100);
  assert_eq!(rpn, vec![NID::var(0), NID::var(1)]);}
