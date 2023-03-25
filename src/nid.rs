//! Node IDs (shared by various Base implementations)
use std::fmt;
use vid;

// -- core data types ---

/// A NID represents a node in a Base. Essentially, this acts like a tuple
/// containing a VID and index, but for performance reasons, it is packed into a u64.
/// See below for helper functions that manipulate and analyze the packed bits.
#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct NID { n: u64 }


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
#[inline(always)] fn nv(v:VidBits)->NID { NID { n:((v as u64) << 32)|VAR }}

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
      let ar:u8 = self.arity().unwrap();
      let ft:u32 = self.tbl().unwrap() & ((2<<ar as u32)-1);
      if ar == 2 { write!(f, "<{:04b}>", ft)} // TODO: dynamically format to a length
      else {  write!(f, "<{:b}>", ft) }}
    else { if self.is_inv() { write!(f, "¬")?; }
           if self.is_vid() { write!(f, "{}", self.vid()) }
           else if self.is_ixn() { write!(f, "#{}", self.idx()) }
           else { write!(f, "@[v{}:{}]", self.vid(), self.idx()) }}}}

/// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
impl fmt::Debug for NID { // for test suite output
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}


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


/// helper for 'fun' (function table) nids
/// u32 x contains the bits to permute.
/// pv is a permutation vector (the bytes 0..=31 in some order)
// b=pv[i] means to grab bit b from x and move to position i in the result.
fn permute_bits(x:u32, pv:&[u8])->u32 {
  let mut r:u32 = 0;
  for (i,b) in pv.iter().enumerate() { r |= ((x & (1<<b)) >> b) << i; }
  r }


impl NID {
  #[inline(always)] pub fn o()->Self { O }
  #[inline(always)] pub fn i()->Self { I }
  #[inline(always)] pub fn var(v:u32)->Self { Self::from_vid(vid::VID::var(v)) }
  #[inline(always)] pub fn vir(v:u32)->Self { Self::from_vid(vid::VID::vir(v)) }
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

  /// is this NID just an indexed node with no variable?
  #[inline(always)] pub fn is_ixn(self)->bool { vid_bits(self)==NOVAR }

  /// Map the NID to an index. (I.e., if n=idx(x), then x is the nth node branching on var(x))
  #[inline(always)] pub fn idx(self)->usize { (self.n & IDX_MASK) as usize }

  /// Return the NID with the 'INV' flag removed.
  // !! pos()? abs()? I don't love any of these names.
  #[inline(always)] pub fn raw(self)->NID { NID{ n: self.n & !INV }}

  /// construct a NID holding a truth table for up to 5 input bits.
  #[inline(always)] pub const fn fun(arity:u8, tbl:u32)->Self { NID { n:F+(tbl as u64)+((arity as u64)<< 32)} }

  /// is this NID a function (truth table)?
  #[inline(always)] pub fn is_fun(&self)->bool { self.n & F == F }

  #[inline(always)] pub fn tbl(&self)->Option<u32> { if self.is_fun(){ Some(self.idx() as u32)} else {None} }

  #[inline(always)] pub fn arity(&self)->Option<u8> {
    if self.is_fun(){ Some((self.n >> 32 & 0xff) as u8) }
    else if self.is_lit() { Some(0) }
    // !! TODO: decide what arity means for general nids.
    // !! if the node is already bound to variables. We could think of this as the number
    // !! of distinct variables it contains, *or* we could think of it as an expression that
    // !! takes no parameters. (Maybe the F bit, combined with the "T=Table" bit toggles this?)
    // !! Also, it's not obvious how to track the number of variables when combining two nodes
    // !! without a lot of external storage. The best we can do is look at the top var and
    // !! get an upper bound. With virs, we can't even do that. In any case, I don't actually
    // !! need this at the moment, so I will just leave it unimplemented.
    else { todo!("arity is only implemented for fun and lit nids at the moment") }}

  /// is it possible nid depends on var v?
  /// the goal here is to avoid exploring a subgraph if we don't have to.
  #[inline] pub fn might_depend_on(&self, v:vid::VID)->bool {
    if self.is_const() { false }
    else if self.is_vid() { self.vid() == v }
    else { let sv = self.vid(); sv == v || sv.is_above(&v) }}

  /// given a function, return the function you'd get if you inverted one or more of the input bits.
  /// bits is a bitmap where setting the (2^i)'s-place bit means to invert the `i`th input.
  /// For example: if `bits=0b00101` maps inputs `x0, x1, x2, x3, x4` to `!x0, x1, !x2, x3, x4`
  pub fn fun_flip_inputs(&self, bits:u8)->NID {
    let mut res:u32 = self.tbl().unwrap();
    let flip = |i:u8| (bits & (1<<i)) != 0;
    macro_rules! p { ($x:expr) => { res = permute_bits(res, $x) }}
    if flip(4) { p!(&[16,17,18,19,20,21,22,23,16,17,18,19,20,21,22,23,8 ,9 ,10,11,12,13,14,15,8 ,9 ,10,11,12,13,14,15]) }
    if flip(3) { p!(&[8 ,9 ,10,11,12,13,14,15,0 ,1 ,2 ,3 ,4 ,5 ,6 ,7 ,24,25,26,27,28,29,30,31,16,17,18,19,20,21,22,23]) }
    if flip(2) { p!(&[4 ,5 ,6 ,7 ,0 ,1 ,2 ,3 ,12,13,14,15,8 ,9 ,10,11,20,21,22,23,16,17,18,19,28,29,30,31,24,25,26,27]) }
    if flip(1) { p!(&[2 ,3 ,0 ,1 ,6 ,7 ,4 ,5 ,10,11,8 ,9 ,14,15,12,13,18,19,16,17,22,23,20,21,26,27,24,25,30,31,28,29]) }
    if flip(0) { p!(&[1 ,0 ,3 ,2 ,5 ,4 ,7 ,6 ,9 ,8 ,11,10,13,12,15,14,17,16,19,18,21,20,23,22,25,24,27,26,29,28,31,30]) }
    NID::fun(self.arity().unwrap(), res)}

  /// given a function, return the function you'd get if you "lift" one of the inputs
  /// by swapping it with its neighbors. (so bit=0 permutes inputs x0,x1,x2,x3,x4 to x1,x0,x2,x3,x4)
  pub fn fun_lift_input(&self, bit:u8)->NID {
    macro_rules! p { ($x:expr) => { NID::fun(self.arity().unwrap(), permute_bits(self.tbl().unwrap(), $x)) }}
    match bit {
      3 => p!(&[0 ,1 ,2 ,3 ,4 ,5 ,6 ,7 ,16,17,18,19,20,21,22,23,8 ,9 ,10,11,12,13,14,15,24,25,26,27,28,29,30,31]),
      2 => p!(&[0 ,1 ,2 ,3 ,8 ,9 ,10,11,4 ,5 ,6 ,7 ,12,13,14,15,16,17,18,19,24,25,26,27,20,21,22,23,28,29,30,31]),
      1 => p!(&[0 ,1 ,4 ,5 ,2 ,3 ,6 ,7 ,8 ,9 ,12,13,10,11,14,15,16,17,20,21,18,19,22,23,24,25,28,29,26,27,30,31]),
      0 => p!(&[0 ,2 ,1 ,3 ,4 ,6 ,5 ,7 ,8 ,10,9 ,11,12,14,13,15,16,18,17,19,20,22,21,23,24,26,25,27,28,30,29,31]),
      _ => panic!("{}", "lifted input bit must be in {0,1,2,3}")}}}

#[test] fn test_fun() {
  assert!(!NID::var(1).is_fun(), "var(1) should not be fun.");
  assert!(!NID::vir(1).is_fun(), "vir(1) should not be fun.");
  assert!(!NID::from_vid_idx(vid::NOV, 0).is_fun(), "idx var should not be fun");}
