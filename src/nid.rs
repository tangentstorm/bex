/* Bitmask diagram:

   NID | VAR
   ----+----------------------
   63  | 31  : INV
   62  | 30  : VAR
   61  | 29  : T (const / max vid)
   60  | 28  : RVAR

*/
use std::fmt;
use vid;

// -- core data types ---

/// (OLD) Variable ID: uniquely identifies an input variable in the BDD.
/// This name is private to the nid module since vid::VID supercedes it.
type OLDVID = usize;

/// A NID represents a node in a Base. Essentially, this acts like a tuple
/// containing a VID and IDX, but for performance reasons, it is packed into a u64.
/// See below for helper functions that manipulate and analyze the packed bits.
#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct NID { n: u64 }

/// Just a constructor so I can add extra temp fields in development without breaking code.
const fn new (n:u64)->NID { NID{n} }


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
pub const O:NID = new(T);
/// NID of the virtual node represeting the constant function 1, or "always true."
pub const I:NID = new(T|INV);

// NID support routines

/// Does the NID represent a variable?
#[inline(always)] fn is_var(x:NID)->bool { (x.n & VAR) != 0 }
/// Does the NID represent a *real* variable?
#[inline(always)] fn is_rvar(x:NID)->bool { (x.n & RVAR) != 0 }

/// Does the NID represent a VID?
#[inline(always)] fn is_vid(x:NID)->bool { (x.n & VAR) != 0 }

/// Is n a literal (variable or constant)?
#[inline] fn is_lit(x:NID)->bool { is_vid(x) | is_const(x) }

/// Is the NID inverted? That is, does it represent `not(some other nid)`?
#[inline(always)] fn is_inv(x:NID)->bool { (x.n & INV) != 0 }

/// Return the NID with the 'INV' flag removed.
// !! pos()? abs()? I don't love any of these names.
#[inline(always)] fn raw(x:NID)->NID { new(x.n & !INV) }

/// Does the NID refer to one of the two constant nodes (O or I)?
#[inline(always)] fn is_const(x:NID)->bool { (x.n & T) != 0 }

/// Map the NID to an index. (I,e, if n=idx(x), then x is the nth node branching on var(x))
#[inline(always)] fn idx(x:NID)->usize { (x.n & IDX_MASK) as usize }

/// On which variable does this node branch? (I and O branch on TV)
#[inline(always)] fn vid(x:NID)->OLDVID { ((x.n & !(INV|VAR)) >> 32) as OLDVID}

/// Construct the NID for the (virtual) node corresponding to an input variable.
/// Private since moving to vid::VID, because this didn't set the "real" bit, and
/// I want the real bit to eventually go away in favor of an unset "virtual" bit.
#[inline(always)] fn nv(v:OLDVID)->NID { NID { n:((v as u64) << 32)|VAR }}

/// Construct a NID with the given variable and index.
#[inline(always)] fn nvi(v:OLDVID,i:usize)->NID { new(((v as u64) << 32) + i as u64) }

/// construct an F node
#[inline(always)] const fn fun(arity:u8,tbl:u32)->NID { NID { n:F+(tbl as u64)+((arity as u64)<< 32)}}
#[inline(always)] fn is_fun(x:&NID)->bool { x.n & F == F }
#[inline(always)] fn tbl(x:&NID)->Option<u32> { if is_fun(x){ Some(idx(*x) as u32)} else {None}}
#[inline(always)] fn arity(x:&NID)->u8 {
  if is_fun(x){ (x.n >> 32 & 0xff) as u8 }
  else if is_lit(*x) { 0 }
  // !! TODO: decide what arity means for general nids.
  // !! if the node is already bound to variables. We could think of this as the number
  // !! of distinct variables it contains, *or* we could think of it as an expression that
  // !! takes no parameters. (Maybe the F bit, combined with the "T=Table" bit toggles this?)
  // !! Also, it's not obvious how to track the number of variables when combining two nodes
  // !! without a lot of external storage. The best we can do is look at the top var and
  // !! get an upper bound. With virs, we can't even do that. In any case, I don't actually
  // !! need this at the moment, so I will just leave it unimplemented.
  else { todo!("arity is only implemented for fun and lit nids at the moment") }}


impl std::ops::Not for NID {
  type Output = NID;
  fn not(self)-> NID {NID { n: self.n^INV }}}


/// Pretty-printer for NIDS that reveal some of their internal data.
impl fmt::Display for NID {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if is_const(*self) { if is_inv(*self) { write!(f, "I") } else { write!(f, "O") } }
    else if self.is_fun() {
      let ar:u8 = self.arity().unwrap();
      let ft:u32 = self.tbl().unwrap() & ((2<<ar as u32)-1);
      if ar == 2 { write!(f, "<{:04b}>", ft)} // TODO: dynamically format to a length
      else {  write!(f, "<{:b}>", ft) }}
    else { if is_inv(*self) { write!(f, "¬")?; }
           if is_var(*self) { write!(f, "{}", self.vid()) }
           else if is_rvar(*self) { write!(f, "@[{}:{}]", self.vid(), idx(*self)) }
           else if vid(*self) == NOVAR { write!(f, "#{}", idx(*self)) }
           else { write!(f, "@[v{}:{}]", vid(*self), idx(*self)) }}}}

/// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
impl fmt::Debug for NID { // for test suite output
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}


#[test] fn test_nids() {
  assert_eq!(O.n,   2305843009213693952); assert_eq!(O, new(0x2000000000000000));
  assert_eq!(I.n,  11529215046068469760); assert_eq!(I, new(0xa000000000000000));
  assert_eq!(NID::vir(0), new(0x4000000000000000u64));
  assert_eq!(NID::var(0), new(0x5000000000000000u64));
  assert_eq!(NID::vir(1),  new(0x4000000100000000u64));
  assert!(vid(NID::vir(0)) < vid(NID::var(0)));
  assert_eq!(nvi(0,0), new(0x0000000000000000u64));
  assert_eq!(nvi(1,0), new(0x0000000100000000u64)); }

#[test] fn test_var() {
  assert_eq!(vid(O), 536_870_912, "var(O)");
  assert_eq!(vid(I), vid(O), "INV bit shouldn't be part of variable");
  assert_eq!(vid(NID::vir(0)), 0);
  assert_eq!(vid(NID::var(0)), 268_435_456);}

#[test] fn test_cmp() {
  let v = |x:usize|->NID { nv(x) };  let x=|x:u32|->NID { NID::var(x) };
  let o=vid;   let n=|x:NID|x.vid();
  assert!(o(O) == o(I),      "old:no=no");  assert!(n(O) == n(I),       "new:no=no");
  assert!(o(O)    > o(v(0)), "old:no>v0");  assert!(n(O).is_below(&n(v(0))), "new:no bel v0");
  assert!(o(O)    > o(x(0)), "old:no>x0");  assert!(n(O).is_below(&n(x(0))), "new:no bel x0");
  assert!(o(v(0)) < o(x(0)), "old:v0>x0");  assert!(n(v(0)).is_above(&n(x(0))),  "new:v0 abv x0");
  assert!(o(v(1)) < o(x(0)), "old:v1<x0");  assert!(n(v(1)).is_above(&n(x(0))),  "new:v1 abv x0");}


// scaffolding for moving ASTBase over to use NIDS
const NOVAR:OLDVID = (1<<26) as OLDVID; // 134_217_728
const TOP:OLDVID = (T>>32) as OLDVID; // 536_870_912, // 1<<29, same as nid::T

fn vid_to_old(v:vid::VID)->OLDVID {
  if v.is_nov() { NOVAR }
  else if v.is_top() { TOP }
  else if v.is_var() { v.var_ix() | (RVAR>>32) as OLDVID }
  else if v.is_vir() { v.vir_ix() as OLDVID }
  else { panic!("unknown vid::VID {:?}?", v) }}

fn old_to_vid(o:OLDVID)->vid::VID {
  if o == TOP { vid::VID::top() }
  else if o == NOVAR { vid::VID::nov() }
  else if o & (RVAR>>32) as OLDVID > 0 { vid::VID::var((o & !(RVAR>>32) as OLDVID) as u32) }
  else { vid::VID::vir(o as u32) }}

/// helper for 'fun' (function table) nids
/// u32 x contains the bits to permute.
/// pv is a permutation vector (the bytes 0..=31 in some order)
// b=pv[i] means to grab bit b from x and move to position i in the result.
fn permute_bits(x:u32, pv:&[u8])->u32 {
  let mut r:u32 = 0;
  for (i,b) in pv.iter().enumerate() { r |= ((x & (1<<b)) >> b) << i; }
  r }


// TODO: add n.is_vid() to replace current is_var()
// TODO: is_var() should only be true for vars, not both virs and vars.
// TODO: probably also need is_nov() for consistency.

impl NID {
  pub fn var(v:u32)->Self { Self::from_vid(vid::VID::var(v)) }
  pub fn vir(v:u32)->Self { Self::from_vid(vid::VID::vir(v)) }
  // return a nid that is not tied to a variable
  pub fn ixn(ix:usize)->Self { nvi(NOVAR, ix) }
  pub fn from_var(v:vid::VID)->Self { NID::var(v.var_ix() as u32)}
  pub fn from_vir(v:vid::VID)->Self { NID::vir(v.vir_ix() as u32)}
  pub fn from_vid(v:vid::VID)->Self { nv(vid_to_old(v)) }
  pub fn from_vid_idx(v:vid::VID, i:usize)->Self { nvi(vid_to_old(v), i) }
  pub fn vid(&self)->vid::VID { old_to_vid(vid(*self)) }
  pub fn is_const(&self)->bool { is_const(*self) }
  pub fn is_vid(&self)->bool { is_vid(*self)}
  pub fn is_var(&self)->bool { self.is_vid() && self.vid().is_var() }
  pub fn is_vir(&self)->bool { self.is_vid() && self.vid().is_vir() }
  pub fn is_lit(&self)->bool { is_lit(*self) }
  pub fn is_inv(&self)->bool { is_inv(*self) }
  /// is this NID just an indexed node with no variable?
  pub fn is_ixn(self)->bool { vid(self)==NOVAR }
  pub fn idx(self)->usize { idx(self) }
  pub fn raw(self)->NID { raw(self) }
  pub const fn fun(arity:u8, tbl:u32)->Self { fun(arity,tbl) }
  pub fn is_fun(&self)->bool { is_fun(self) }
  pub fn tbl(&self)->Option<u32> { tbl(self) }
  pub fn arity(&self)->Option<u8> { Some(arity(self)) }
  /// is it possible nid depends on var v?
  /// the goal here is to avoid exploring a subgraph if we don't have to.
  #[inline] pub fn might_depend_on(&self, v:vid::VID)->bool {
    if is_const(*self) { false }
    else if is_var(*self) { self.vid() == v }
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
