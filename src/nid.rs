//! Node IDs (shared by various Base implementations)
use std::fmt;
use std::str::FromStr;
use crate::vid;

// -- core data types ---

/// A NID represents a node in a Base. Essentially, this acts like a tuple
/// containing a VID and index, but for performance reasons, it is packed into a u64.
/// See below for helper functions that manipulate and analyze the packed bits.
#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct NID { n: u64 }

/// A truth table stored directly in a nid for functions of up to 5 inputs.
#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct NidFun { nid:NID }


// -- bits in the nid ---

/// Single-bit mask representing that a NID is inverted.
const INV:u64 = 1<<63;  // is inverted?

/// Single-bit mask indicating that a NID represents a variable. (The corresponding
/// "virtual" nodes have I as their hi branch and O as their lo branch. They're simple
/// and numerous enough that we don't bother actually storing them.)
const VAR:u64 = 1<<62;   // is variable?

/// Single-bit mask indicating that the NID represents a constant. The corresponding
/// virtual node branches on constant "true" value, hence the letter T. There is only
/// one such node -- O (I is its inverse) but having this bit in the NID lets us
/// easily detect and optimize the cases.
const T:u64 = 1<<61;    // T: max VID (hack so O/I nodes show up at bottom)

/// In addition, for solving, we want to distinguish between "virtual" variables which
/// represent some intermediate, unsimplified calculation, and "real" variables, which
/// represent actual input variables. That's what this bit does.
const RVAR:u64 = 1<<60;  // is *real* variable?

/// This bit indicates that the NID is meant to be used as a function.
/// (All nids represent functions, but this bit indicates that rather
/// than referring to an existing node, it is a function of <=5 inputs
/// and the entire truth table is stored in the index field.
// !TODO: Decide whether or not to merge F(unction) with T(able). If separate,
// then F+T might mean treat this as a function with a table, and F+!T would
// tell the interpreter to apply some previously defined expression as a function.
const F:u64 = 1<<59;

/// Constant used to extract the index part of a NID.
const IDX_MASK:u64 = (1<<32)-1;

/// NID of the virtual node represeting the constant function 0, or "always false."
pub const O:NID = NID{n: T};
/// NID of the virtual node represeting the constant function 1, or "always true."
pub const I:NID = NID{ n:T|INV };

// scaffolding for moving ASTBase over to use NIDS

/// bit buffer used for extracting/inserting a VID
type VidBits = usize;

/// On which variable does this node branch? (I and O branch on TV)
#[inline(always)] fn vid_bits(x:NID)->VidBits { ((x.n & !(INV|VAR)) >> 32) as VidBits}

/// Construct the NID for the (virtual) node corresponding to an input variable.
/// Private since moving to vid::VID, because this didn't set the "real" bit, and
/// I want the real bit to eventually go away in favor of an unset "virtual" bit.
#[inline(always)] const fn nv(v:VidBits)->NID { NID { n:((v as u64) << 32)|VAR }}

/// Construct a NID with the given variable and index.
#[inline(always)] fn nvi(v:VidBits,i:usize)->NID { NID{n: ((v as u64) << 32) + i as u64} }

const NOVAR:VidBits = (1<<26) as VidBits; // 134_217_728
const TOP:VidBits = (T>>32) as VidBits; // 536_870_912, // 1<<29, same as nid::T

fn vid_to_bits(v:vid::VID)->VidBits {
  if v.is_nov() { NOVAR }
  else if v.is_top() { TOP }
  else if v.is_var() { v.var_ix() | (RVAR>>32) as VidBits }
  else if v.is_vir() { v.vir_ix() as VidBits }
  else { panic!("unknown vid::VID {:?}?", v) }}

fn bits_to_vid(o:VidBits)->vid::VID {
  if o == TOP { vid::VID::top() }
  else if o == NOVAR { vid::VID::nov() }
  else if o & (RVAR>>32) as VidBits > 0 { vid::VID::var((o & !(RVAR>>32) as VidBits) as u32) }
  else { vid::VID::vir(o as u32) }}


impl std::ops::Not for NID {
  type Output = NID;
  fn not(self)-> NID {NID { n: self.n^INV }}}


/// Pretty-printer for NIDS that reveal some of their internal data.
impl fmt::Display for NID {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if self.is_const() { if self.is_inv() { write!(f, "I") } else { write!(f, "O") } }
    else if self.is_fun() {
      let fnid = self.to_fun().unwrap();
      let ar:u8 = fnid.arity(); // 2..5 inclusive
      // !! arity of 1 would just be ID or NOT, which are redundant because of the INV bit
      let ft:u32 = fnid.tbl();
      if ar == 2 { write!(f, "t{:04b}", ft) }
      else { write!(f, "f{}.{:X}", ar, ft) }}
    else {
      if self.is_inv() { write!(f, "!")?; }
      if self.is_vid() { write!(f, "{}", self.vid()) }
      else if self.is_ixn() { write!(f, "@{:X}", self.idx()) }
      else { write!(f, "{}.{:X}", self.vid(), self.idx()) }}}}

/// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
impl fmt::Debug for NID { // for test suite output
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}


impl FromStr for NID {
  type Err = String;

  fn from_str(word: &str) -> Result<Self, Self::Err> {
    match word {
    "O" => Ok(O),
    "I" => Ok(I),
    _ => {
      let (a, b) = if let Some(ix) = word.find('.') { word.split_at(ix) } else { (word, "") };
      let mut ch = a.chars().peekable();
      let mut inv: bool = false;
      if ch.peek().unwrap() == &'!' { ch.next(); inv = true }
      macro_rules! num_suffix {
        ($radix:expr, $ch:expr) => { usize::from_str_radix(&$ch.collect::<String>(), $radix) }}
      let is_upper_hex = |s: &str| s.chars().all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c));
      let c = ch.next().unwrap();
      // literals or VHL NIDS:
      if c == 'x' || c == 'v'  {
        let tail = ch.collect::<String>();
        if tail.is_empty() { return Err(format!("malformed variable: {}", word)); }
        if !is_upper_hex(&tail) { return Err(format!("hex must be uppercase: {}", word)); }
        if let Ok(n) = usize::from_str_radix(&tail, 16) {
          let v = if c == 'x' { vid::VID::var(n as u32) } else { vid::VID::vir(n as u32) };
          if b.is_empty() { Ok(NID::from_vid(v).inv_if(inv)) }
          else {
            if !is_upper_hex(&b[1..]) { return Err(format!("hex must be uppercase: {}", word)); }
            if let Ok(ix) = usize::from_str_radix(&b[1..], 16) {
            Ok(NID::from_vid_idx(v, ix).inv_if(inv) )}
            else { Err(format!("bad index after '.': {}", word)) }
          }
        }
        else { Err(format!("malformed variable: {}", word)) }}
      else { match c {
        '@' => {
          let tail = ch.collect::<String>();
          if tail.is_empty() { return Err(format!("bad ixn: {}", word)) }
          if !is_upper_hex(&tail) { return Err(format!("hex must be uppercase: {}", word)); }
          if let Ok(n) = usize::from_str_radix(&tail, 16) { Ok(NID::ixn(n).inv_if(inv)) }
          else { Err(format!("bad ixn: {}", word)) }
        }
        'f' =>
          if let Some(i) = word.find('.') {
            let (a, b) = word.split_at(i);
            if let Ok(ar) = num_suffix!(16, a.chars().skip(1)) {
              if !is_upper_hex(&b[1..]) { Err(format!("hex must be uppercase: {}", word)) }
              else if let Ok(tb) = usize::from_str_radix(&b[1..], 16) {
                Ok(NID::fun(ar as u8, tb as u32).to_nid().inv_if(inv))}
              else { Err(format!("bad fun arity: {}", word)) }}
            else { Err(format!("bad fun code: {}", word)) }}
          else {
            // shorthand: fX == f2.X (single uppercase hex digit only)
            let tail = ch.collect::<String>();
            if tail.len() == 1 && is_upper_hex(&tail) {
              let n = usize::from_str_radix(&tail, 16).map_err(|_| format!("bad fun: {}", word))?;
              Ok(NID::fun(2, n as u32).to_nid().inv_if(inv))
            } else { Err(format!("bad fun: {}", word)) }
          }
        't' =>
            {
              let bits = ch.collect::<String>();
              let len = bits.len();
              if !(len == 2 || len == 4 || len == 8 || len == 16 || len == 32) {
                return Err(format!("bad length for table (expect 2,4,8,16,32 bits): {}", word));
              }
              if !bits.chars().all(|c| c == '0' || c == '1') {
                return Err(format!("bad table (non-binary digit): {}", word));
              }
              let ar = match len { 2=>1, 4=>2, 8=>3, 16=>4, 32=>5, _=> unreachable!() };
              let tb = usize::from_str_radix(&bits, 2).map_err(|_| format!("bad table: {}", word))?;
              Ok(NID::fun(ar as u8, tb as u32).to_nid().inv_if(inv))
            }
        _ => Err(format!("{}?", word))}}}}}}


#[test] fn test_nids() {
  let new = |n| { NID{n} };
  assert_eq!(O.n,   2305843009213693952); assert_eq!(O, new(0x2000000000000000));
  assert_eq!(I.n,  11529215046068469760); assert_eq!(I, new(0xa000000000000000));
  assert_eq!(NID::vir(0), new(0x4000000000000000u64));
  assert_eq!(NID::var(0), new(0x5000000000000000u64));
  assert_eq!(NID::vir(1),  new(0x4000000100000000u64));
  assert!(vid_bits(NID::vir(0)) < vid_bits(NID::var(0)));
  assert_eq!(nvi(0,0), new(0x0000000000000000u64));
  assert_eq!(nvi(1,0), new(0x0000000100000000u64)); }

#[test] fn test_var() {
  assert_eq!(vid_bits(O), 536_870_912, "var(O)");
  assert_eq!(vid_bits(I), vid_bits(O), "INV bit shouldn't be part of variable");
  assert_eq!(vid_bits(NID::vir(0)), 0);
  assert_eq!(vid_bits(NID::var(0)), 268_435_456);}

#[test] fn test_cmp() {
  let v = |x:usize|->NID { nv(x) };  let x=|x:u32|->NID { NID::var(x) };
  let o=vid_bits;   let n=|x:NID|x.vid();
  assert!(o(O) == o(I),      "old:no=no");  assert!(n(O) == n(I),       "new:no=no");
  assert!(o(O)    > o(v(0)), "old:no>v0");  assert!(n(O).is_below(&n(v(0))), "new:no bel v0");
  assert!(o(O)    > o(x(0)), "old:no>x0");  assert!(n(O).is_below(&n(x(0))), "new:no bel x0");
  assert!(o(v(0)) < o(x(0)), "old:v0>x0");  assert!(n(v(0)).is_above(&n(x(0))),  "new:v0 abv x0");
  assert!(o(v(1)) < o(x(0)), "old:v1<x0");  assert!(n(v(1)).is_above(&n(x(0))),  "new:v1 abv x0");}



impl NID {
  #[inline(always)] pub fn o()->Self { O }
  #[inline(always)] pub fn i()->Self { I }
  #[inline(always)] pub fn from_bit(b:bool)->Self { if b { I } else { O } }
  #[inline(always)] pub fn to_bit(&self)->bool { self != &O }

  #[inline(always)] pub const fn var(v:u32)->Self { nv((v | (RVAR>>32) as u32) as VidBits) }
  #[inline(always)] pub const fn vir(v:u32)->Self { nv(v as VidBits) }
  #[inline(always)] pub fn from_var(v:vid::VID)->Self { Self::var(v.var_ix() as u32)}
  #[inline(always)] pub fn from_vir(v:vid::VID)->Self { Self::vir(v.vir_ix() as u32)}

  #[inline(always)] pub fn from_vid(v:vid::VID)->Self { nv(vid_to_bits(v)) }
  #[inline(always)] pub fn from_vid_idx(v:vid::VID, i:usize)->Self { nvi(vid_to_bits(v), i) }
  #[inline(always)] pub fn vid(&self)->vid::VID { bits_to_vid(vid_bits(*self)) }
  // return a nid that is not tied to a variable
  #[inline(always)] pub fn ixn(ix:usize)->Self { nvi(NOVAR, ix) }

  /// Does the NID refer to one of the two constant nodes (O or I)?
  #[inline(always)] pub fn is_const(&self)->bool { (self.n & T) != 0 }

  /// Does the NID represent a VID (either Var or Vir)?
  #[inline(always)] pub fn is_vid(&self)->bool { (self.n & VAR) != 0 }

  /// Does the NID represent an input variable?
  #[inline(always)] pub fn is_var(&self)->bool { self.is_vid() && self.vid().is_var() }

  /// Does the NID represent a virtual variable?
  #[inline(always)] pub fn is_vir(&self)->bool { self.is_vid() && self.vid().is_vir() }

  /// Is n a literal (variable or constant)?
  #[inline(always)] pub fn is_lit(&self)->bool { self.is_vid() | self.is_const()}

  /// Is the NID inverted? That is, does it represent `!(some other nid)`?
  #[inline(always)] pub fn is_inv(&self)->bool { (self.n & INV) != 0 }

  /// Invert if the condition is true
  #[inline(always)] pub fn inv_if(&self, cond:bool)->NID {
    if cond { NID { n: self.n ^ INV }} else { *self }}

  /// is this NID just an indexed node with no variable?
  #[inline(always)] pub fn is_ixn(self)->bool { (self.n & (F|T|VAR) == 0) && vid_bits(self)==NOVAR }

  /// Map the NID to an index. (I.e., if n=idx(x), then x is the nth node branching on var(x))
  #[inline(always)] pub fn idx(self)->usize { (self.n & IDX_MASK) as usize }

  /// Return the NID with the 'INV' flag removed.
  // !! pos()? abs()? I don't love any of these names.
  #[inline(always)] pub fn raw(self)->NID { NID{ n: self.n & !INV }}

  /// construct a NID holding a truth table for up to 5 input bits.
  #[inline(always)] pub const fn fun(arity:u8, tbl:u32)->NidFun {
    NidFun { nid: NID { n:F+(((1<<(1<<arity)) -1) & tbl as u64)+((arity as u64)<< 32)}} }

  /// is this NID a function (truth table)?
  #[inline(always)] pub fn is_fun(&self)->bool { self.n & F == F }
  #[inline(always)] pub fn to_fun(&self)->Option<NidFun> {
    if self.is_fun() { Some(NidFun { nid:*self }) } else { None }}

  #[inline(always)] pub fn tbl(&self)->Option<u32> { if self.is_fun(){ Some(self.idx() as u32)} else {None} }

  /// is it possible nid depends on var v?
  /// the goal here is to avoid exploring a subgraph if we don't have to.
  #[inline] pub fn might_depend_on(&self, v:vid::VID)->bool {
    if self.is_const() { false }
    else if self.is_vid() { self.vid() == v }
    else { let sv = self.vid(); sv == v || sv.is_above(&v) }}

  // -- int conversions used by the dd python package
  #[inline] pub fn _from_u64(x:u64)->Self { NID{n:x} }
  #[inline] pub fn _to_u64(&self)->u64 { self.n }
}

#[test] fn test_tbl_fmt() {
  assert_eq!("t1110", format!("{}", NID::fun(2, 0b1110).to_nid()));
  assert_eq!("f3.FC", format!("{}", NID::fun(3, 0xFC).to_nid()));}

include!("nid-fun.rs");

/// predefined consts for VIDS (mostly for tests)
#[allow(non_upper_case_globals)]
pub mod named {
  use super::NID;
  pub const x0:NID = NID::var(0);
  pub const x1:NID = NID::var(1);
  pub const x2:NID = NID::var(2);
  pub const x3:NID = NID::var(3);
  pub const x4:NID = NID::var(4);
  pub const v0:NID = NID::vir(0);
  pub const v1:NID = NID::vir(1);
  pub const v2:NID = NID::vir(2);
  pub const v3:NID = NID::vir(3);
  pub const v4:NID = NID::vir(4);
}
