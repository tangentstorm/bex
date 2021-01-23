//! Defines a common NID scheme for bases whose nodes follow a
//! (var, lo, hi) structure. Used by BDD, ANF, and eventually ZDD.

/* Bitmask diagram:

   NID | VAR
   ----+----------------------
   63  | 31  : INV
   62  | 30  : VAR
   61  | 29  : T (const / max vid)
   60  | 28  : RVAR

*/

use std::fmt;

// -- core data types ---

/// (OLD) Variable ID: uniquely identifies an input variable in the BDD.
/// This name is private to the nid module since vid::VID supercedes it.
type VID = usize;

/// Index into a (usually VID-specific) vector.
pub type IDX = u32;

/// A NID represents a node in a Base. Essentially, this acts like a tuple
/// containing a VID and IDX, but for performance reasons, it is packed into a u64.
/// See below for helper functions that manipulate and analyze the packed bits.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Serialize, Deserialize)]
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
#[inline(always)] pub fn is_var(x:NID)->bool { (x.n & VAR) != 0 }
/// Does the NID represent a *real* variable?
#[inline(always)] pub fn is_rvar(x:NID)->bool { (x.n & RVAR) != 0 }

/// Is n a literal (variable or constant)?
#[inline] pub fn is_lit(x:NID)->bool { is_var(x) | is_const(x) }

/// Is the NID inverted? That is, does it represent `not(some other nid)`?
#[inline(always)] pub fn is_inv(x:NID)->bool { (x.n & INV) != 0 }

/// Return the NID with the 'INV' flag removed.
// !! pos()? abs()? I don't love any of these names.
#[inline(always)] pub fn raw(x:NID)->NID { new(x.n & !INV) }

/// Does the NID refer to one of the two constant nodes (O or I)?
#[inline(always)] pub fn is_const(x:NID)->bool { (x.n & T) != 0 }

/// Map the NID to an index. (I,e, if n=idx(x), then x is the nth node branching on var(x))
#[inline(always)] pub fn idx(x:NID)->usize { (x.n & IDX_MASK) as usize }

/// On which variable does this node branch? (I and O branch on TV)
/// TODO: there should probably be a self.get_vid() instead
#[inline(always)] pub fn var(x:NID)->VID { ((x.n & !(INV|VAR)) >> 32) as VID}
/// Same as var() but strips out the RVAR bit.
#[inline(always)] pub fn rvar(x:NID)->VID { ((x.n & !(INV|VAR|RVAR)) >> 32) as VID}

/// Toggle the INV bit, applying a logical "NOT" operation to the corressponding node.
#[deprecated(note="use !nid instead")]
#[inline(always)] pub fn not(x:NID)->NID { NID { n:x.n^INV } }

/// Construct the NID for the (virtual) node corresponding to an input variable.
/// Private since moving to vid::VID, because this didn't set the "real" bit, and
/// I want the real bit to eventually go away in favor of an unset "virtual" bit.
#[inline(always)] fn nv(v:VID)->NID { NID { n:((v as u64) << 32)|VAR }}

/// Construct a NID with the given variable and index.
#[inline(always)] pub fn nvi(v:VID,i:IDX)->NID { new(((v as u64) << 32) + i as u64) }

/// construct an F node
#[inline(always)] pub const fn fun(arity:u8,tbl:u32)->NID { NID { n:F+(tbl as u64)+((arity as u64)<< 32)}}
#[inline(always)] pub fn is_fun(x:&NID)->bool { x.n & F == F }
#[inline(always)] pub fn tbl(x:&NID)->Option<u32> { if is_fun(x){ Some(idx(*x) as u32)} else {None}}
#[inline(always)] pub fn arity(x:&NID)->usize {
  if is_fun(x){ (x.n >> 32 & 0xff) as usize }
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
    else { if is_inv(*self) { write!(f, "¬")?; }
           if is_var(*self) { write!(f, "{}", self.vid()) }
           else if is_rvar(*self) { write!(f, "@[{}:{}]", self.vid(), idx(*self)) }
           else if var(*self) == NOVAR { write!(f, "#{}", idx(*self)) }
           else { write!(f, "@[v{}:{}]", var(*self), idx(*self)) }}}}

/// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
impl fmt::Debug for NID { // for test suite output
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}


#[test] fn test_nids() {
  assert_eq!(O.n,   2305843009213693952); assert_eq!(O, new(0x2000000000000000));
  assert_eq!(I.n,  11529215046068469760); assert_eq!(I, new(0xa000000000000000));
  assert_eq!(NID::vir(0), new(0x4000000000000000u64));
  assert_eq!(NID::var(0), new(0x5000000000000000u64));
  assert_eq!(NID::vir(1),  new(0x4000000100000000u64));
  assert!(var(NID::vir(0)) < var(NID::var(0)));
  assert_eq!(nvi(0,0), new(0x0000000000000000u64));
  assert_eq!(nvi(1,0), new(0x0000000100000000u64)); }

  #[test] fn test_var() {
    assert_eq!(var(O), 536_870_912, "var(O)");
    assert_eq!(var(I), var(O), "INV bit shouldn't be part of variable");
    assert_eq!(var(NID::vir(0)), 0);
    assert_eq!(var(NID::var(0)), 268_435_456);
  }

  #[test] fn test_cmp() {
    let v = |x:usize|->NID { nv(x) };  let x=|x:u32|->NID { NID::var(x) };
    let o=|x:NID|var(x);   let n=|x:NID|x.vid();
    assert!(o(O) == o(I),      "old:no=no");  assert!(n(O) == n(I),       "new:no=no");
    assert!(o(O)    > o(v(0)), "old:no>v0");  assert!(n(O).is_below(&n(v(0))), "new:no bel v0");
    assert!(o(O)    > o(x(0)), "old:no>x0");  assert!(n(O).is_below(&n(x(0))), "new:no bel x0");
    assert!(o(v(0)) < o(x(0)), "old:v0>x0");  assert!(n(v(0)).is_above(&n(x(0))),  "new:v0 abv x0");
    assert!(o(v(1)) < o(x(0)), "old:v1<x0");  assert!(n(v(1)).is_above(&n(x(0))),  "new:v1 abv x0");
  }


// scaffolding for moving ASTBase over to use NIDS
const NOVAR:VID = (1<<27) as VID; // 134_217_728
const TOP:VID = (T>>32) as VID; // 536_870_912, // 1<<29, same as nid::T
pub fn no_var(x:NID)->bool { var(x)==NOVAR }
/// return a nid that is not tied to a variable
pub fn ixn(ix:IDX)->NID { nvi(NOVAR, ix) }

use vid;

fn vid_to_old(v:vid::VID)->VID {
  if v.is_nov() { NOVAR }
  else if v.is_top() { TOP }
  else if v.is_var() { v.var_ix() | (RVAR>>32) as VID }
  else if v.is_vir() { v.vir_ix() as VID }
  else { panic!("unknown vid::VID {:?}?", v) }}

fn old_to_vid(o:VID)->vid::VID {
  if o == TOP { vid::VID::top() }
  else if o == NOVAR { vid::VID::nov() }
  else if o & (RVAR>>32) as VID > 0 {
     vid::VID::var((o & !(RVAR>>32) as VID) as u32) }
  else { vid::VID::vir(o as u32) }}

impl NID {
  pub fn var(v:u32)->Self { Self::from_vid(vid::VID::var(v)) }
  pub fn vir(v:u32)->Self { Self::from_vid(vid::VID::vir(v)) }
  pub fn from_var(v:vid::VID)->Self { NID::var(v.var_ix() as u32)}
  pub fn from_vir(v:vid::VID)->Self { NID::vir(v.vir_ix() as u32)}
  pub fn from_vid(v:vid::VID)->Self { nv(vid_to_old(v)) }
  pub fn from_vid_idx(v:vid::VID, i:IDX)->Self { nvi(vid_to_old(v), i) }
  pub fn vid(&self)->vid::VID { old_to_vid(var(*self)) }
  pub fn is_const(&self)->bool { is_const(*self) }
  pub fn is_var(&self)->bool { is_var(*self) }
  pub fn is_lit(&self)->bool { is_lit(*self) }
  pub fn is_inv(&self)->bool { is_inv(*self) }
  pub fn idx(&self)->usize { idx(*self) }
  pub const fn fun(arity:u8, tbl:u32)->Self { fun(arity,tbl) }
  pub fn is_fun(&self)->bool { is_fun(self) }
  pub fn tbl(&self)->Option<u32> { tbl(self) }
  pub fn arity(&self)->Option<usize> { Some(arity(self)) }
  /// is it possible nid depends on var v?
  /// the goal here is to avoid exploring a subgraph if we don't have to.
  #[inline] pub fn might_depend_on(&self, v:vid::VID)->bool {
    if is_const(*self) { false }
    else if is_var(*self) { self.vid() == v }
    else { let sv = self.vid(); sv == v || sv.is_above(&v) }}
}
