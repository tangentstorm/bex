//! Defines a common NID scheme for bases whose nodes follow a
//! (var, lo, hi) structure. Used by BDD, ANF, and eventually ZDD.

use std::fmt;

// -- core data types ---

/// Variable ID: uniquely identifies an input variable in the BDD.
pub type VID = usize;

/// Index into a (usually VID-specific) vector.
pub type IDX = u32;

/// A NID represents a node in a Base. Essentially, this acts like a tuple
/// containing a VID and IDX, but for performance reasons, it is packed into a u64.
/// See below for helper functions that manipulate and analyze the packed bits.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct NID { n: u64 }

pub fn un(n:NID)->usize {
  if n == O { 0 }
  else if n == I { 1 }
  else if is_var(n) { var(n) as usize }
  else { idx(n) as usize }}
pub fn nu(u:usize, nvars:usize)->NID {
  if u == 0 { O }
  else if u == 1 { I }
  else if u < nvars { nv(u) }
  else { nvi(NOVAR, u as IDX) } }


// -- bits in the nid ---

/// Single-bit mask representing that a NID is inverted.
pub const INV:u64 = 1<<63;  // is inverted?

/// Single-bit mask indicating that a NID represents a variable. (The corresponding
/// "virtual" nodes have I as their hi branch and O as their lo branch. They're simple
/// and numerous enough that we don't bother actually storing them.)
pub const VAR:u64 = 1<<62;   // is variable?

/// In addition, for solving, we want to distinguish between "virtual" variables which
/// represent some intermediate, unsimplified calculation, and "real" variables, which
/// represent actual input variables. That's what this bit does.
pub const RVAR:u64 = 1<<60;  // is *real* variable?

/// Single-bit mask indicating that the NID represents a constant. The corresponding
/// virtual node branches on constant "true" value, hence the letter T. There is only
/// one such node -- O (I is its inverse) but having this bit in the NID lets us
/// easily detect and optimize the cases.
pub const T:u64 = 1<<61;    // T: max VID (hack so O/I nodes show up at bottom)

/// Constant used to extract the index part of a NID.
pub const IDX_MASK:u64 = (1<<32)-1;

/// temp const used while converting ASTBase (TODO: remove NOVAR)
pub const NOVAR:VID = 1<<31;


/// NID of the virtual node represeting the constant function 0, or "always false."
pub const O:NID = NID{ n:T };
/// NID of the virtual node represeting the constant function 1, or "always true."
pub const I:NID = NID{ n:(T|INV) };

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
#[inline(always)] pub fn raw(x:NID)->NID { NID{ n: x.n & !INV }}

/// Does the NID refer to one of the two constant nodes (O or I)?
#[inline(always)] pub fn is_const(x:NID)->bool { (x.n & T) != 0 }

/// Map the NID to an index. (I,e, if n=idx(x), then x is the nth node branching on var(x))
#[inline(always)] pub fn idx(x:NID)->usize { (x.n & IDX_MASK) as usize }

/// On which variable does this node branch? (I and O branch on TV)
#[inline(always)] pub fn var(x:NID)->VID { ((x.n & !(INV|VAR)) >> 32) as VID}
/// Same as var() but strips out the RVAR bit.
#[inline(always)] pub fn rvar(x:NID)->VID { ((x.n & !(INV|VAR|RVAR)) >> 32) as VID}

/// internal function to strip rvar bit and convert to usize
#[inline(always)] pub fn rv(v:VID)->usize { (v&!((RVAR>>32) as VID)) as usize}

/// Toggle the INV bit, applying a logical "NOT" operation to the corressponding node.
#[inline(always)] pub fn not(x:NID)->NID { NID { n:x.n^INV } }

/// Construct the NID for the (virtual) node corresponding to an input variable.
#[inline(always)] pub fn nv(v:VID)->NID { NID { n:((v as u64) << 32)|VAR }}

/// Construct the NID for the (virtual) node corresponding to an input variable.
#[inline(always)] pub fn nvr(v:VID)->NID { NID { n:((v as u64) << 32)|VAR|RVAR }}

/// Construct a NID with the given variable and index.
#[inline(always)] pub fn nvi(v:VID,i:IDX)->NID { NID{ n:((v as u64) << 32) + i as u64 }}


/// Pretty-printer for NIDS that reveal some of their internal data.
impl fmt::Display for NID {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if is_const(*self) { if is_inv(*self) { write!(f, "I") } else { write!(f, "O") } }
    else { if is_inv(*self) { write!(f, "¬")?; }
           if is_var(*self) {
             if is_rvar(*self) { write!(f, "x{}", rvar(*self)) }
             else { write!(f, "v{}", var(*self)) }}
           else if is_rvar(*self) { write!(f, "@[x{}:{}]", rvar(*self), idx(*self)) }
           else { write!(f, "@[v{}:{}]", var(*self), idx(*self)) }}}}

/// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
impl fmt::Debug for NID { // for test suite output
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}


/// Simple Hi/Lo pair stored internally when representing nodes.
/// All nodes with the same branching variable go in the same array, so there's
/// no point duplicating it.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HILO {pub hi:NID, pub lo:NID}

impl HILO {
  /// constructor
  pub fn new(hi:NID, lo:NID)->HILO { HILO { hi, lo } }

  /// apply the not() operator to both branches
  #[inline] pub fn invert(self)-> HILO { HILO{ hi: not(self.hi), lo: not(self.lo) }} }


#[test] fn test_nids() {
  assert_eq!(O, NID{n:0x2000000000000000u64});
  assert_eq!(I, NID{n:0xa000000000000000u64});
  assert_eq!(nv(0),  NID{n:0x4000000000000000u64});
  assert_eq!(nvr(0), NID{n:0x5000000000000000u64});
  assert_eq!(nv(1),  NID{n:0x4000000100000000u64});
  assert!(var(nv(0)) < var(nvr(0)));
  assert_eq!(nvi(0,0), NID{n:0x0000000000000000u64});
  assert_eq!(nvi(1,0), NID{n:0x0000000100000000u64}); }

