<?xml version="1.0" encoding="utf-8"?>
<!-- Created by Leo: http://leoeditor.com/leo_toc.html -->
<leo_file xmlns:leo="http://leoeditor.com/namespaces/leo-python-editor/1.1" >
<leo_header file_format="2"/>
<globals/>
<preferences/>
<find_panel_settings/>
<vnodes>
<v t="tangentstorm.20220203082928.1"><vh>bex concepts</vh>
<v t="tangentstorm.20220203082928.2"><vh>nids and vids</vh>
<v t="tangentstorm.20220203082928.3"><vh>O and I</vh>
<v t="tangentstorm.20220203082928.4"><vh>boolean constants (nid.rs)</vh></v>
</v>
<v t="tangentstorm.20220203082928.5"><vh>virs and vars</vh></v>
<v t="tangentstorm.20220203082928.6"><vh>funs</vh></v>
</v>
<v t="tangentstorm.20220203082928.7"><vh>ops</vh></v>
<v t="tangentstorm.20220203082928.8"><vh>ast</vh></v>
</v>
<v t="tangentstorm.20220203082937.1"><vh>upcoming features</vh>
<v t="tangentstorm.20220203082937.2"><vh>bex world solver</vh></v>
</v>
<v t="tangentstorm.20220203082953.1"><vh>@path d:/ver/bex/src</vh>
<v t="tangentstorm.20220203082953.2"><vh>@clean vid.rs</vh>
<v t="tangentstorm.20220203082953.3"><vh>enum VidEnum (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.4"><vh>pub enum VidOrdering (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.5"><vh>pub struct VID (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.6"><vh>fn cmp_depth_idx (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.7"><vh>pub fn cmp_depth (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.8"><vh>pub fn topmost (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.9"><vh>pub fn botmost (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.10"><vh>pub fn topmost_of3 (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.11"><vh>impl VID (vid.rs)</vh>
<v t="tangentstorm.20220203082953.12"><vh>predicates</vh></v>
<v t="tangentstorm.20220203082953.13"><vh>comparisons (is_above/is_below)</vh></v>
<v t="tangentstorm.20220203082953.14"><vh>pub fn shift_up (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.15"><vh>pub fn var_ix (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.16"><vh>pub fn vir_ix (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.17"><vh>pub fn vid_ix (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.18"><vh>pub fn bitmask (vid.rs)</vh></v>
</v>
<v t="tangentstorm.20220203082953.19"><vh>impl fmt::Display for VID (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.20"><vh>impl fmt::Debug for VID { // for test suite output (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.21"><vh>impl VID (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.22"><vh>impl Ord for VID (vid.rs)</vh></v>
<v t="tangentstorm.20220203082953.23"><vh>impl PartialOrd for VID (vid.rs)</vh></v>
</v>
<v t="tangentstorm.20220203082953.24"><vh>@clean nid.rs</vh>
<v t="tangentstorm.20220203082953.25"><vh>pub struct NID (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.26"><vh>boolean constants (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.27"><vh>support routines</vh></v>
<v t="tangentstorm.20220203082953.28"><vh>impl std::ops::Not for NID (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.29"><vh>impl fmt::Display for NID (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.30"><vh>impl fmt::Debug for NID { // for test suite output (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.31"><vh>tests</vh></v>
<v t="tangentstorm.20220203082953.32"><vh>scaffolding</vh></v>
<v t="tangentstorm.20220203082953.33"><vh>pub fn no_var (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.34"><vh>pub fn ixn (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.35"><vh>fn vid_to_old (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.36"><vh>fn old_to_vid (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.37"><vh>impl NID (nid.rs)</vh>
<v t="tangentstorm.20220203082953.38"><vh>constructors</vh></v>
<v t="tangentstorm.20220203082953.39"><vh>pub fn vid (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.40"><vh>predicates</vh></v>
<v t="tangentstorm.20220203082953.41"><vh>pub fn idx (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.42"><vh>pub fn tbl (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.43"><vh>pub fn arity (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.44"><vh>pub fn fun_flip_inputs (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.45"><vh>fn permute_bits (nid.rs)</vh></v>
<v t="tangentstorm.20220203082953.46"><vh>pub fn fun_lift_input (nid.rs)</vh></v>
</v>
<v t="tangentstorm.20220203082953.47"><vh>more tests</vh></v>
</v>
<v t="tangentstorm.20220203082953.48"><vh>@clean ast.rs</vh>
<v t="tangentstorm.20220203082953.49"><vh>pub struct RawASTBase (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.50"><vh>impl RawASTBase (ast.rs)</vh>
<v t="tangentstorm.20220203082953.51"><vh>pub fn empty (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.52"><vh>pub fn len (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.53"><vh>pub fn is_empty (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.54"><vh>fn nid (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.55"><vh>pub fn load (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.56"><vh>fn when (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.57"><vh>fn walk where F: FnMut(NID (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.58"><vh>fn step where F:FnMut(NID (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.59"><vh>pub fn show (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.60"><vh>pub fn masks_and_costs (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.61"><vh>pub fn reftable (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.62"><vh>fn markdeps (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.63"><vh>pub fn permute (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.64"><vh>pub fn repack (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.65"><vh>pub fn get_ops (ast.rs)</vh></v>
</v>
<v t="tangentstorm.20220203082953.66"><vh>impl Base for RawASTBase (ast.rs)</vh>
<v t="tangentstorm.20220203082953.67"><vh>fn new (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.68"><vh>fn when_hi (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.69"><vh>fn when_lo (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.70"><vh>fn def (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.71"><vh>fn tag (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.72"><vh>fn and (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.73"><vh>fn xor (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.74"><vh>fn or (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.75"><vh>fn sub (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.76"><vh>fn get (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.77"><vh>fn save (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.78"><vh>fn dot (ast.rs)</vh></v>
</v>
<v t="tangentstorm.20220203082953.79"><vh>pub struct ASTBase (ast.rs)</vh>
<v t="tangentstorm.20220203082953.80"><vh>impl Base for ASTBase (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.81"><vh>impl ASTBase (ast.rs)</vh></v>
<v t="tangentstorm.20220203082953.82"><vh>test suite</vh></v>
</v>
</v>
</v>
</vnodes>
<tnodes>
<t tx="tangentstorm.20220203082928.1">@language rust
</t>
<t tx="tangentstorm.20220203082928.2">@
Bex uses graphs to represent boolean expressions.

Graphs consist of nodes (or "vertices" in some circles) and edges.

In bex, every node has an identifier, called a NID (for "node ID").
The word "NID" rhymes with "lid".

Except for parethesis, pretty much every token in the language of propositional logic can be represented by a NID.

</t>
<t tx="tangentstorm.20220203082928.3">@
For example, the constant NIDs `O` and `I` represent the values false (or 0,off,etc.) and true (or 1, "on", etc.), respectively.
  - 
</t>
<t tx="tangentstorm.20220203082928.4">/// NID of the virtual node represeting the constant function 0, or "always false."
pub const O:NID = new(T);
/// NID of the virtual node represeting the constant function 1, or "always true."
pub const I:NID = new(T|INV);


</t>
<t tx="tangentstorm.20220203082928.5"></t>
<t tx="tangentstorm.20220203082928.6">@
The 'fun' bit indicates that the nid represents a boolean function of up to 5 inputs.


For each arity (number of inputs) n, there are 2^2^n possible truth tables:

       i.6x       NB. http://jsoftware.com
    0 1 2 3 4 5

       2^2^i.6x
    2 4 16 256 65536 4294967296

For example, the two zero-input functions are O and I.

The four one-input functions are:

   x -&gt; O
   x -&gt; I
   x -&gt; x
   x -&gt; ~x

(The first three of these are redundant, and the last one is the "not" operator.)

The sixteen two-input functions include logical connectives like AND, OR, XOR, and
so on, as well as "not x", "not y", and of course the degenerate functions 
"(x,y)-&gt;O" and "(x,y)-&gt;I".

The 256 three-input functions include such useful as "majority" and "if z then x else y".

There aren't standard names for the boolean functions with extra inputs,
but any boolean expression of five or fewer inputs can be represented by a single nid
in this way.</t>
<t tx="tangentstorm.20220203082928.7"></t>
<t tx="tangentstorm.20220203082928.8"></t>
<t tx="tangentstorm.20220203082937.1"></t>
<t tx="tangentstorm.20220203082937.2">@
The world solver is an approach to solving boolean expressions by decomposition.

The general approach works *as if* we chose an input variable to remove from the expression, and made two copies of the expression, with the chosen variable replaced by each of the constants "0" and "1". We then simplify the expressions as much as possible, and then perform the same type of substitution for the next variable, but on each of the new copies.

The trick is to minimize wasted memory and time spent re-calculating things.

Rather than making complete copies of the AST at each point, we imagine something like a tree of "alternate worlds". The root of the tree is not the AST itself, but rather a mapping of the input variables to the "twigs" of the AST that take those variables, and an "overlay" that maps node identifiers to transformed twigs.

The solver does a breadth-first, bottom-up transformation of the graph.

Leaves are always literals (either constants or input variables).

Constants can always be eliminated by replacing the function in the downstream nodes (unless of course the entire expresion simplifies to a constant).

</t>
<t tx="tangentstorm.20220203082953.1">@language rust
@tabwidth -4
</t>
<t tx="tangentstorm.20220203082953.10">pub fn topmost_of3(x:VID, y:VID, z:VID)-&gt;VID { topmost(x, topmost(y, z)) }



</t>
<t tx="tangentstorm.20220203082953.11">impl VID {
  pub const fn top()-&gt;VID { VID { v:T }}
  pub const fn nov()-&gt;VID { VID { v:NoV }}
  pub const fn var(i:u32)-&gt;VID { VID { v: Var(i) }}
  pub const fn vir(i:u32)-&gt;VID { VID { v: Vir(i) }}
  @others
</t>
<t tx="tangentstorm.20220203082953.12">pub fn is_top(&amp;self)-&gt;bool { VID{ v:T } == *self }
pub fn is_nov(&amp;self)-&gt;bool { if let VID{ v:NoV } = self { true } else { false } }
pub fn is_var(&amp;self)-&gt;bool { if let VID{ v:Var(_) } = self { true } else { false } }
pub fn is_vir(&amp;self)-&gt;bool { if let VID{ v:Vir(_) } = self { true } else { false } }

</t>
<t tx="tangentstorm.20220203082953.13">pub fn is_above(&amp;self, other:&amp;VID)-&gt;bool { self.cmp_depth(&amp;other) == VidOrdering::Above }
pub fn is_below(&amp;self, other:&amp;VID)-&gt;bool { self.cmp_depth(&amp;other) == VidOrdering::Below }
</t>
<t tx="tangentstorm.20220203082953.14">pub fn shift_up(&amp;self)-&gt;VID {
  match self.v {
    NoV =&gt; panic!("VID::nov().shift_up() is undefined"),
    T =&gt; panic!("VID::top().shift_up() is undefined"), //VID::var(0),
    // these two might panic on over/underflow:
    Var(x) =&gt; VID::var(x+1),
    Vir(x) =&gt; VID::vir(x+1)}}

</t>
<t tx="tangentstorm.20220203082953.15">pub fn var_ix(&amp;self)-&gt;usize {
  if let Var(x) = self.v { x as usize } else { panic!("var_ix({:?})", self) }}

</t>
<t tx="tangentstorm.20220203082953.16">pub fn vir_ix(&amp;self)-&gt;usize {
  if let Vir(x) = self.v { x as usize } else { panic!("vir_ix({:?})", self) }}

</t>
<t tx="tangentstorm.20220203082953.17">pub fn vid_ix(&amp;self)-&gt;usize { match self.v {
  T =&gt; panic!("x.vid_ix() makes no sense when x==T. Test with nid::is_const first."),
  NoV =&gt; panic!("x.vid_ix() makes no sense when x==VID::NoV. Test with x.is_nov first."),
  Var(x) | Vir(x) =&gt; x as usize }}

</t>
<t tx="tangentstorm.20220203082953.18">pub fn bitmask(&amp;self)-&gt;u64 { match self.v {
  NoV|T =&gt; 0,
  Var(x) | Vir(x) =&gt; if x &lt; 64 { 1 &lt;&lt; x as u64 } else { 0 }}}}



</t>
<t tx="tangentstorm.20220203082953.19">/// Pretty-printer for NIDS that reveal some of their internal data.
impl fmt::Display for VID {
  fn fmt(&amp;self, f: &amp;mut fmt::Formatter) -&gt; fmt::Result {
    match self.v {
      T =&gt; write!(f, "T"),
      NoV =&gt; write!(f, "NoV"),
      Var(x) =&gt; write!(f, "x{}", x),
      Vir(x) =&gt; write!(f, "v{}", x) }}}

</t>
<t tx="tangentstorm.20220203082953.2">/// Variable Identifiers
use std::cmp::Ordering;
use std::fmt;

@others
</t>
<t tx="tangentstorm.20220203082953.20">/// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
impl fmt::Debug for VID { // for test suite output
  fn fmt(&amp;self, f: &amp;mut fmt::Formatter) -&gt; fmt::Result { write!(f, "{}", self) }}

</t>
<t tx="tangentstorm.20220203082953.21">

</t>
<t tx="tangentstorm.20220203082953.22">impl VID {
  #[deprecated(note="VID scaffolding")]
  pub fn u(&amp;self)-&gt;usize { match self.v {
    T  =&gt;  536870912, // 1&lt;&lt;29, same as nid::T,
    NoV =&gt; panic!("can't turn NoV into a number"),
    Var(x) =&gt; x as usize,
    Vir(x) =&gt; x as usize }}}

/// this is only so I can order ops. VID should otherwise always be
/// compared with is_above / iS_below or cmp_depth, for clarity.
impl Ord for VID {
  fn cmp(&amp;self, other: &amp;Self)-&gt; Ordering {
    match self.cmp_depth(other) {
      VidOrdering::Above =&gt; Ordering::Less,
      VidOrdering::Level =&gt; Ordering::Equal,
      VidOrdering::Below =&gt; Ordering::Greater}}}

</t>
<t tx="tangentstorm.20220203082953.23">impl PartialOrd for VID {
  fn partial_cmp(&amp;self, other: &amp;Self) -&gt; Option&lt;Ordering&gt; {
      Some(self.cmp(other))}}
</t>
<t tx="tangentstorm.20220203082953.24">/* Bitmask diagram:

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

@others
</t>
<t tx="tangentstorm.20220203082953.25">/// A NID represents a node in a Base. Essentially, this acts like a tuple
/// containing a VID and IDX, but for performance reasons, it is packed into a u64.
/// See below for helper functions that manipulate and analyze the packed bits.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct NID { n: u64 }

/// Just a constructor so I can add extra temp fields in development without breaking code.
const fn new (n:u64)-&gt;NID { NID{n} }



// -- bits in the nid ---

/// Single-bit mask representing that a NID is inverted.
const INV:u64 = 1&lt;&lt;63;  // is inverted?

/// Single-bit mask indicating that a NID represents a variable. (The corresponding
/// "virtual" nodes have I as their hi branch and O as their lo branch. They're simple
/// and numerous enough that we don't bother actually storing them.)
const VAR:u64 = 1&lt;&lt;62;   // is variable?

/// Single-bit mask indicating that the NID represents a constant. The corresponding
/// virtual node branches on constant "true" value, hence the letter T. There is only
/// one such node -- O (I is its inverse) but having this bit in the NID lets us
/// easily detect and optimize the cases.
const T:u64 = 1&lt;&lt;61;    // T: max VID (hack so O/I nodes show up at bottom)

/// In addition, for solving, we want to distinguish between "virtual" variables which
/// represent some intermediate, unsimplified calculation, and "real" variables, which
/// represent actual input variables. That's what this bit does.
const RVAR:u64 = 1&lt;&lt;60;  // is *real* variable?

/// This bit indicates that the NID is meant to be used as a function.
/// (All nids represent functions, but this bit indicates that rather
/// than referring to an existing node, it is a function of &lt;=5 inputs
/// and the entire truth table is stored in the index field.
// !TODO: Decide whether or not to merge F(unction) with T(able). If separate,
// then F+T might mean treat this as a function with a table, and F+!T would
// tell the interpreter to apply some previously defined expression as a function.
const F:u64 = 1&lt;&lt;59;

/// Constant used to extract the index part of a NID.
const IDX_MASK:u64 = (1&lt;&lt;32)-1;

</t>
<t tx="tangentstorm.20220203082953.26">/// NID of the virtual node represeting the constant function 0, or "always false."
pub const O:NID = new(T);
/// NID of the virtual node represeting the constant function 1, or "always true."
pub const I:NID = new(T|INV);


</t>
<t tx="tangentstorm.20220203082953.27">// NID support routines

/// Does the NID represent a variable?
#[inline(always)] pub fn is_var(x:NID)-&gt;bool { (x.n &amp; VAR) != 0 }
/// Does the NID represent a *real* variable?
#[inline(always)] pub fn is_rvar(x:NID)-&gt;bool { (x.n &amp; RVAR) != 0 }

/// Does the NID represent a VID?
#[inline(always)] pub fn is_vid(x:NID)-&gt;bool { (x.n &amp; VAR) != 0 }

/// Is n a literal (variable or constant)?
#[inline] pub fn is_lit(x:NID)-&gt;bool { is_vid(x) | is_const(x) }

/// Is the NID inverted? That is, does it represent `not(some other nid)`?
#[inline(always)] pub fn is_inv(x:NID)-&gt;bool { (x.n &amp; INV) != 0 }

/// Return the NID with the 'INV' flag removed.
// !! pos()? abs()? I don't love any of these names.
#[inline(always)] pub fn raw(x:NID)-&gt;NID { new(x.n &amp; !INV) }

/// Does the NID refer to one of the two constant nodes (O or I)?
#[inline(always)] pub fn is_const(x:NID)-&gt;bool { (x.n &amp; T) != 0 }

/// Map the NID to an index. (I,e, if n=idx(x), then x is the nth node branching on var(x))
#[inline(always)] pub fn idx(x:NID)-&gt;usize { (x.n &amp; IDX_MASK) as usize }

/// On which variable does this node branch? (I and O branch on TV)
#[inline(always)] pub fn vid(x:NID)-&gt;VID { ((x.n &amp; !(INV|VAR)) &gt;&gt; 32) as VID}

/// Toggle the INV bit, applying a logical "NOT" operation to the corressponding node.
#[deprecated(note="use !nid instead")]
#[inline(always)] pub fn not(x:NID)-&gt;NID { NID { n:x.n^INV } }

/// Construct the NID for the (virtual) node corresponding to an input variable.
/// Private since moving to vid::VID, because this didn't set the "real" bit, and
/// I want the real bit to eventually go away in favor of an unset "virtual" bit.
#[inline(always)] fn nv(v:VID)-&gt;NID { NID { n:((v as u64) &lt;&lt; 32)|VAR }}

/// Construct a NID with the given variable and index.
#[inline(always)] pub fn nvi(v:VID,i:IDX)-&gt;NID { new(((v as u64) &lt;&lt; 32) + i as u64) }

/// construct an F node
#[inline(always)] pub const fn fun(arity:u8,tbl:u32)-&gt;NID { NID { n:F+(tbl as u64)+((arity as u64)&lt;&lt; 32)}}
#[inline(always)] pub fn is_fun(x:&amp;NID)-&gt;bool { x.n &amp; F == F }
#[inline(always)] pub fn tbl(x:&amp;NID)-&gt;Option&lt;u32&gt; { if is_fun(x){ Some(idx(*x) as u32)} else {None}}
#[inline(always)] pub fn arity(x:&amp;NID)-&gt;u8 {
  if is_fun(x){ (x.n &gt;&gt; 32 &amp; 0xff) as u8 }
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



</t>
<t tx="tangentstorm.20220203082953.28">impl std::ops::Not for NID {
  type Output = NID;
  fn not(self)-&gt; NID {NID { n: self.n^INV }}}

</t>
<t tx="tangentstorm.20220203082953.29">

/// Pretty-printer for NIDS that reveal some of their internal data.
impl fmt::Display for NID {
  fn fmt(&amp;self, f: &amp;mut fmt::Formatter) -&gt; fmt::Result {
    if is_const(*self) { if is_inv(*self) { write!(f, "I") } else { write!(f, "O") } }
    else if self.is_fun() {
      let ar:u8 = self.arity().unwrap();
      let ft:u32 = self.tbl().unwrap() &amp; ((2&lt;&lt;ar as u32)-1);
      if ar == 2 { write!(f, "&lt;{:04b}&gt;", ft)} // TODO: dynamically format to a length
      else {  write!(f, "&lt;{:b}&gt;", ft) }}
    else { if is_inv(*self) { write!(f, "¬")?; }
           if is_var(*self) { write!(f, "{}", self.vid()) }
           else if is_rvar(*self) { write!(f, "@[{}:{}]", self.vid(), idx(*self)) }
           else if vid(*self) == NOVAR { write!(f, "#{}", idx(*self)) }
           else { write!(f, "@[v{}:{}]", vid(*self), idx(*self)) }}}}

</t>
<t tx="tangentstorm.20220203082953.3">/// this will probably go away in favor of a bitmask at some point
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
enum VidEnum {
  // How I (eventually) want the ordering, to be (once biggest vars go on top:)
  T,        // Special meta-constant on which I and O branch.
  NoV,      // Special case for AST nodes not tied to a variable
  Var(u32), // Real Vars go in the middle, with biggest u32 on top.
  Vir(u32), // Virtual are "biggest", so go to the top.
}

</t>
<t tx="tangentstorm.20220203082953.30">/// Same as fmt::Display. Mostly so it's easier to see the problem when an assertion fails.
impl fmt::Debug for NID { // for test suite output
  fn fmt(&amp;self, f: &amp;mut fmt::Formatter) -&gt; fmt::Result { write!(f, "{}", self) }}
</t>
<t tx="tangentstorm.20220203082953.31">


#[test] fn test_nids() {
  assert_eq!(O.n,   2305843009213693952); assert_eq!(O, new(0x2000000000000000));
  assert_eq!(I.n,  11529215046068469760); assert_eq!(I, new(0xa000000000000000));
  assert_eq!(NID::vir(0), new(0x4000000000000000u64));
  assert_eq!(NID::var(0), new(0x5000000000000000u64));
  assert_eq!(NID::vir(1),  new(0x4000000100000000u64));
  assert!(vid(NID::vir(0)) &lt; vid(NID::var(0)));
  assert_eq!(nvi(0,0), new(0x0000000000000000u64));
  assert_eq!(nvi(1,0), new(0x0000000100000000u64)); }

#[test] fn test_var() {
  assert_eq!(vid(O), 536_870_912, "var(O)");
  assert_eq!(vid(I), vid(O), "INV bit shouldn't be part of variable");
  assert_eq!(vid(NID::vir(0)), 0);
  assert_eq!(vid(NID::var(0)), 268_435_456);}

#[test] fn test_cmp() {
  let v = |x:usize|-&gt;NID { nv(x) };  let x=|x:u32|-&gt;NID { NID::var(x) };
  let o=|x:NID|vid(x);   let n=|x:NID|x.vid();
  assert!(o(O) == o(I),      "old:no=no");  assert!(n(O) == n(I),       "new:no=no");
  assert!(o(O)    &gt; o(v(0)), "old:no&gt;v0");  assert!(n(O).is_below(&amp;n(v(0))), "new:no bel v0");
  assert!(o(O)    &gt; o(x(0)), "old:no&gt;x0");  assert!(n(O).is_below(&amp;n(x(0))), "new:no bel x0");
  assert!(o(v(0)) &lt; o(x(0)), "old:v0&gt;x0");  assert!(n(v(0)).is_above(&amp;n(x(0))),  "new:v0 abv x0");
  assert!(o(v(1)) &lt; o(x(0)), "old:v1&lt;x0");  assert!(n(v(1)).is_above(&amp;n(x(0))),  "new:v1 abv x0");}



</t>
<t tx="tangentstorm.20220203082953.32">// scaffolding for moving ASTBase over to use NIDS
const NOVAR:VID = (1&lt;&lt;26) as VID; // 134_217_728
const TOP:VID = (T&gt;&gt;32) as VID; // 536_870_912, // 1&lt;&lt;29, same as nid::T
</t>
<t tx="tangentstorm.20220203082953.33">pub fn no_var(x:NID)-&gt;bool { vid(x)==NOVAR }
/// return a nid that is not tied to a variable
pub fn ixn(ix:IDX)-&gt;NID { nvi(NOVAR, ix) }
</t>
<t tx="tangentstorm.20220203082953.34">
use vid;

</t>
<t tx="tangentstorm.20220203082953.35">fn vid_to_old(v:vid::VID)-&gt;VID {
  if v.is_nov() { NOVAR }
  else if v.is_top() { TOP }
  else if v.is_var() { v.var_ix() | (RVAR&gt;&gt;32) as VID }
  else if v.is_vir() { v.vir_ix() as VID }
  else { panic!("unknown vid::VID {:?}?", v) }}

</t>
<t tx="tangentstorm.20220203082953.36">fn old_to_vid(o:VID)-&gt;vid::VID {
  if o == TOP { vid::VID::top() }
  else if o == NOVAR { vid::VID::nov() }
  else if o &amp; (RVAR&gt;&gt;32) as VID &gt; 0 { vid::VID::var((o &amp; !(RVAR&gt;&gt;32) as VID) as u32) }
  else { vid::VID::vir(o as u32) }}

/// helper for 'fun' (function table) nids
/// u32 x contains the bits to permute.
/// pv is a permutation vector (the bytes 0..=31 in some order)
// b=pv[i] means to grab bit b from x and move to position i in the result.
fn permute_bits(x:u32, pv:&amp;[u8])-&gt;u32 {
  let mut r:u32 = 0;
  for (i,b) in pv.iter().enumerate() { r |= ((x &amp; (1&lt;&lt;b)) &gt;&gt; b) &lt;&lt; i; }
  r }



</t>
<t tx="tangentstorm.20220203082953.37">// TODO: add n.is_vid() to replace current is_var()
// TODO: is_var() should only be true for vars, not both virs and vars.
// TODO: probably also need is_nov() for consistency.

impl NID {
  @others
</t>
<t tx="tangentstorm.20220203082953.38">pub fn var(v:u32)-&gt;Self { Self::from_vid(vid::VID::var(v)) }
pub fn vir(v:u32)-&gt;Self { Self::from_vid(vid::VID::vir(v)) }
pub fn from_var(v:vid::VID)-&gt;Self { NID::var(v.var_ix() as u32)}
pub fn from_vir(v:vid::VID)-&gt;Self { NID::vir(v.vir_ix() as u32)}
pub fn from_vid(v:vid::VID)-&gt;Self { nv(vid_to_old(v)) }
pub fn from_vid_idx(v:vid::VID, i:IDX)-&gt;Self { nvi(vid_to_old(v), i) }
</t>
<t tx="tangentstorm.20220203082953.39">pub fn vid(&amp;self)-&gt;vid::VID { old_to_vid(vid(*self)) }
</t>
<t tx="tangentstorm.20220203082953.4">#[derive(Eq, PartialEq)]
pub enum VidOrdering {
  Above,
  Level,
  Below }

</t>
<t tx="tangentstorm.20220203082953.40">pub fn is_const(&amp;self)-&gt;bool { is_const(*self) }
pub fn is_vid(&amp;self)-&gt;bool { is_vid(*self)}
pub fn is_var(&amp;self)-&gt;bool { self.is_vid() &amp;&amp; self.vid().is_var() }
pub fn is_vir(&amp;self)-&gt;bool { self.is_vid() &amp;&amp; self.vid().is_vir() }
pub fn is_lit(&amp;self)-&gt;bool { is_lit(*self) }
pub fn is_inv(&amp;self)-&gt;bool { is_inv(*self) }
</t>
<t tx="tangentstorm.20220203082953.41">pub fn idx(&amp;self)-&gt;usize { idx(*self) }
pub const fn fun(arity:u8, tbl:u32)-&gt;Self { fun(arity,tbl) }
pub fn is_fun(&amp;self)-&gt;bool { is_fun(self) }
</t>
<t tx="tangentstorm.20220203082953.42">pub fn tbl(&amp;self)-&gt;Option&lt;u32&gt; { tbl(self) }
</t>
<t tx="tangentstorm.20220203082953.43">pub fn arity(&amp;self)-&gt;Option&lt;u8&gt; { Some(arity(self)) }
/// is it possible nid depends on var v?
/// the goal here is to avoid exploring a subgraph if we don't have to.
#[inline] pub fn might_depend_on(&amp;self, v:vid::VID)-&gt;bool {
  if is_const(*self) { false }
  else if is_var(*self) { self.vid() == v }
  else { let sv = self.vid(); sv == v || sv.is_above(&amp;v) }}

</t>
<t tx="tangentstorm.20220203082953.44">/// given a function, return the function you'd get if you inverted one or more of the input bits.
/// bits is a bitmap where setting the (2^i)'s-place bit means to invert the `i`th input.
/// For example: if `bits=0b00101` maps inputs `x0, x1, x2, x3, x4` to `!x0, x1, !x2, x3, x4`
pub fn fun_flip_inputs(&amp;self, bits:u8)-&gt;NID {
  let mut res:u32 = self.tbl().unwrap();
  let flip = |i:u8| (bits &amp; (1&lt;&lt;i)) != 0;
  macro_rules! p { ($x:expr) =&gt; { res = permute_bits(res, $x); }}
  if flip(4) { p!(&amp;[16,17,18,19,20,21,22,23,16,17,18,19,20,21,22,23,8 ,9 ,10,11,12,13,14,15,8 ,9 ,10,11,12,13,14,15]) }
  if flip(3) { p!(&amp;[8 ,9 ,10,11,12,13,14,15,0 ,1 ,2 ,3 ,4 ,5 ,6 ,7 ,24,25,26,27,28,29,30,31,16,17,18,19,20,21,22,23]) }
  if flip(2) { p!(&amp;[4 ,5 ,6 ,7 ,0 ,1 ,2 ,3 ,12,13,14,15,8 ,9 ,10,11,20,21,22,23,16,17,18,19,28,29,30,31,24,25,26,27]) }
  if flip(1) { p!(&amp;[2 ,3 ,0 ,1 ,6 ,7 ,4 ,5 ,10,11,8 ,9 ,14,15,12,13,18,19,16,17,22,23,20,21,26,27,24,25,30,31,28,29]) }
  if flip(0) { p!(&amp;[1 ,0 ,3 ,2 ,5 ,4 ,7 ,6 ,9 ,8 ,11,10,13,12,15,14,17,16,19,18,21,20,23,22,25,24,27,26,29,28,31,30]) }
  NID::fun(self.arity().unwrap(), res)}

</t>
<t tx="tangentstorm.20220203082953.45"></t>
<t tx="tangentstorm.20220203082953.46">/// given a function, return the function you'd get if you "lift" one of the inputs
/// by swapping it with its neighbors. (so bit=0 permutes inputs x0,x1,x2,x3,x4 to x1,x0,x2,x3,x4)
pub fn fun_lift_input(&amp;self, bit:u8)-&gt;NID {
  macro_rules! p { ($x:expr) =&gt; { NID::fun(self.arity().unwrap(), permute_bits(self.tbl().unwrap(), $x)) }}
  match bit {
    3 =&gt; p!(&amp;[0 ,1 ,2 ,3 ,4 ,5 ,6 ,7 ,16,17,18,19,20,21,22,23,8 ,9 ,10,11,12,13,14,15,24,25,26,27,28,29,30,31]),
    2 =&gt; p!(&amp;[0 ,1 ,2 ,3 ,8 ,9 ,10,11,4 ,5 ,6 ,7 ,12,13,14,15,16,17,18,19,24,25,26,27,20,21,22,23,28,29,30,31]),
    1 =&gt; p!(&amp;[0 ,1 ,4 ,5 ,2 ,3 ,6 ,7 ,8 ,9 ,12,13,10,11,14,15,16,17,20,21,18,19,22,23,24,25,28,29,26,27,30,31]),
    0 =&gt; p!(&amp;[0 ,2 ,1 ,3 ,4 ,6 ,5 ,7 ,8 ,10,9 ,11,12,14,13,15,16,18,17,19,20,22,21,23,24,26,25,27,28,30,29,31]),
    _ =&gt; panic!("{}", "lifted input bit must be in {0,1,2,3}")}}}
</t>
<t tx="tangentstorm.20220203082953.47">
#[test] fn test_fun() {
  assert!(!NID::var(1).is_fun(), "var(1) should not be fun.");
  assert!(!NID::vir(1).is_fun(), "vir(1) should not be fun.");
  assert!(!NID::from_vid_idx(vid::NOV, 0).is_fun(), "idx var should not be fun");}
</t>
<t tx="tangentstorm.20220203082953.48">// a concrete implemetation:
use std::collections::{HashMap,HashSet};

use io;
use base::*;
use {nid, nid::NID};
use {vid, vid::VID};
use {ops, ops::Ops};
use simp;



#[derive(Serialize, Deserialize, Debug)]
@others
</t>
<t tx="tangentstorm.20220203082953.49">pub struct RawASTBase {
  bits: Vec&lt;Ops&gt;,                   // all known bits (simplified)
  tags: HashMap&lt;String, NID&gt;,       // support for naming/tagging bits.
  hash: HashMap&lt;Ops, NID&gt;,          // expression cache (simple+complex)
}


type VarMaskFn = fn(&amp;RawASTBase,vid::VID)-&gt;u64;

</t>
<t tx="tangentstorm.20220203082953.5">use self::VidEnum::*;



#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VID { v:VidEnum }
pub const NOV:VID = VID::nov();
pub const TOP:VID = VID::top();

</t>
<t tx="tangentstorm.20220203082953.50">/// An ASTBase that does not use extra simplification rules.
impl RawASTBase {

  @others
} // impl RawASTBase


</t>
<t tx="tangentstorm.20220203082953.51">pub fn empty()-&gt;RawASTBase { RawASTBase{ bits:vec![], tags:HashMap::new(), hash:HashMap::new() }}
</t>
<t tx="tangentstorm.20220203082953.52">pub fn len(&amp;self)-&gt;usize { self.bits.len() }
</t>
<t tx="tangentstorm.20220203082953.53">pub fn is_empty(&amp;self)-&gt;bool { self.bits.is_empty() }

</t>
<t tx="tangentstorm.20220203082953.54">fn nid(&amp;mut self, ops:Ops)-&gt;NID {
  match self.hash.get(&amp;ops) {
    Some(&amp;n) =&gt; n,
    None =&gt; {
      let nid = nid::ixn(self.bits.len() as u32);
      self.bits.push(ops.clone());
      self.hash.insert(ops, nid);
      nid }}}

</t>
<t tx="tangentstorm.20220203082953.55">pub fn load(path:&amp;str)-&gt;::std::io::Result&lt;RawASTBase&gt; {
  let s = io::get(path)?;
  Ok(bincode::deserialize(&amp;s).unwrap()) }



</t>
<t tx="tangentstorm.20220203082953.56">fn when(&amp;mut self, v:vid::VID, val:NID, nid:NID)-&gt;NID {
  if nid.is_vid() &amp;&amp; nid.vid() == v { val }
  else if nid.is_lit() { nid }
  else {
    let ops = self.get_ops(nid).clone();
    let rpn:Vec&lt;NID&gt; = ops.to_rpn().map(|&amp;nid|{
      if nid.is_fun() { nid }
      else { self.when(v, val, nid) }}).collect();
    self.nid(ops::rpn(&amp;rpn)) }}




</t>
<t tx="tangentstorm.20220203082953.57">fn walk&lt;F&gt;(&amp;self, n:NID, f:&amp;mut F) where F: FnMut(NID) {
  let mut seen = HashSet::new();
  self.step(n,f,&amp;mut seen)}

</t>
<t tx="tangentstorm.20220203082953.58">fn step&lt;F&gt;(&amp;self, n:NID, f:&amp;mut F, seen:&amp;mut HashSet&lt;NID&gt;) where F:FnMut(NID) {
  if !seen.contains(&amp;nid::raw(n)) {
    seen.insert(nid::raw(n));
    f(n);
    for op in self.get_ops(n).to_rpn() {
      if !op.is_fun() {
        self.step(*op, f, seen) }}}}

</t>
<t tx="tangentstorm.20220203082953.59">pub fn show(&amp;self, n:NID) { self.show_named(n, "+ast+") }



/// given a function that maps input bits to 64-bit masks, color each node
/// in the base according to its inputs (thus tracking the spread of influence
/// of up to 64 bits (or groups of bits).
///
/// while we're at it, calculate the cost of each bit, where constants have cost 0,
/// inputs have a cost of 1, and everything else is 1 + max(cost of input bits)
/// (TOOD: break masks_and_costs into two functions)
</t>
<t tx="tangentstorm.20220203082953.6">fn cmp_depth_idx(x:u32, y:&amp;u32)-&gt;VidOrdering {
  match x.cmp(y) {
    Ordering::Less =&gt; VidOrdering::Below,
    Ordering::Equal =&gt; VidOrdering::Level,
    Ordering::Greater =&gt; VidOrdering::Above }}

</t>
<t tx="tangentstorm.20220203082953.60">pub fn masks_and_costs(&amp;self, vm:VarMaskFn)-&gt;(Vec&lt;u64&gt;, Vec&lt;u32&gt;) {
  use std::cmp::max;
  let mut masks = vec![];
  let mut costs = vec![];
  for bit in self.bits.iter() {
    let (mask, cost) = {
      let cost = |x:NID| {
        if x.is_const() { 0 }
        else if x.is_vid() { 1 }
        else if nid::no_var(x) { costs[nid::idx(x)] }
        else { todo!("cost({:?})", x) }};
      let mask = |x:NID| {
        if nid::is_const(x) { 0 }
        else if x.is_vid() { vm(self, x.vid()) }
        else if nid::no_var(x) { masks[nid::idx(x)] }
        else { todo!("mask({:?})", x) }};
      let mut m = 0u64;
      let mut c = 0u32;
      for &amp;op in bit.to_rpn() {
        if ! op.is_fun() {
          m |= mask(op);
          c = max(c, cost(op)); }}
      (m, c+1) };
    masks.push(mask);
    costs.push(cost)}
  (masks, costs)}

</t>
<t tx="tangentstorm.20220203082953.61">/// this returns a ragged 2d vector of direct references for each bit in the base
pub fn reftable(&amp;self) -&gt; Vec&lt;Vec&lt;NID&gt;&gt; {
  todo!("test case for reftable!"); /*
  let bits = &amp;self.bits;
  let mut res:Vec&lt;Vec&lt;NID&gt;&gt; = vec![vec![]; bits.len()];
  for (n, &amp;bit) in bits.iter().enumerate() {
    let mut f = |x:NID| res[nid::idx(x)].push(n);
    match bit {
      Op::And(x,y)  =&gt; { f(x); f(y); }
      Op::Xor(x,y)  =&gt; { f(x); f(y); }
      Op::Or(x,y)   =&gt; { f(x); f(y); }
      Op::Ch(x,y,z) =&gt; { f(x); f(y); f(z); }
      Op::Mj(x,y,z) =&gt; { f(x); f(y); f(z); } } }
  res*/ }


</t>
<t tx="tangentstorm.20220203082953.62">/// this is part of the garbage collection system. keep is the top level nid to keep.
/// seen gets marked true for every nid that is a dependency of keep.
/// TODO:: use a HashSet for 'seen' in markdeps()
fn markdeps(&amp;self, keep:NID, seen:&amp;mut Vec&lt;bool&gt;) {
  if nid::is_lit(keep) { return }
  if !nid::no_var(keep) { todo!("markdeps({:?})", keep) }
  if !seen[nid::idx(keep)] {
    seen[nid::idx(keep)] = true;
    let mut f = |x:&amp;NID| { self.markdeps(*x, seen) };
    for op in self.bits[nid::idx(keep)].to_rpn() { if !op.is_fun() { f(op) }}}}



</t>
<t tx="tangentstorm.20220203082953.63">/// Construct a copy of the base, with the nodes reordered according to
/// permutation vector pv. That is, pv is a vector of unique node indices
/// that we want to keep, in the order we want them. (It might actually be
/// shorter than bits.len() and thus not technically a permutation vector,
/// but I don't have a better name for this concept.)
pub fn permute(&amp;self, pv:&amp;[usize])-&gt;RawASTBase {
  // map each kept node in self.bits to Some(new position)
  let new:Vec&lt;Option&lt;usize&gt;&gt; = {
    let mut res = vec![None; self.bits.len()];
    for (i,&amp;n) in pv.iter().enumerate() { res[n] = Some(i) }
    res };
  let nn = |x:NID|{
    if nid::is_lit(x) { x }
    else {
      let r = nid::ixn(new[nid::idx(x) as usize].expect("bad index in AST::permute") as u32);
      if nid::is_inv(x) { !r } else { r }}};
  let newbits = pv.iter().map(|&amp;old| {
    let new:Vec&lt;NID&gt; = self.bits[old].to_rpn().map(|&amp;x| { if x.is_fun() { x } else { nn(x) }}).collect();
    ops::rpn(&amp;new) }).collect();
  let mut newtags = HashMap::new();
  for (key, &amp;val) in &amp;self.tags { newtags.insert(key.clone(), nn(val)); }
  RawASTBase{ bits:newbits, tags:newtags, hash:HashMap::new() }}

</t>
<t tx="tangentstorm.20220203082953.64">/// Construct a new RawASTBase with only the nodes necessary to define the given nodes.
/// The relative order of the bits is preserved.
pub fn repack(&amp;self, keep:Vec&lt;NID&gt;) -&gt; (RawASTBase, Vec&lt;NID&gt;) {
  // garbage collection: mark dependencies of the bits we want to keep
  let mut deps = vec!(false;self.bits.len());
  for &amp;nid in keep.iter() { self.markdeps(nid, &amp;mut deps) }

  let mut new:Vec&lt;Option&lt;usize&gt;&gt; = vec![None; self.bits.len()];
  let mut old:Vec&lt;usize&gt; = vec![];
  for i in 0..self.bits.len() {
    if deps[i] { new[i]=Some(old.len()); old.push(i); }}

  (self.permute(&amp;old), keep.iter().map(|&amp;i|
    nid::ixn(new[nid::idx(i) as usize].expect("?!") as u32)).collect()) }

</t>
<t tx="tangentstorm.20220203082953.65">pub fn get_ops(&amp;self, n:NID)-&gt;&amp;Ops {
  if nid::no_var(n) { &amp;self.bits[nid::idx(n)] } else { panic!("don't know how to op({:?})", n) }}
</t>
<t tx="tangentstorm.20220203082953.66">impl Base for RawASTBase {

  @others
} // impl Base for RawASTBase


</t>
<t tx="tangentstorm.20220203082953.67">fn new()-&gt;Self { RawASTBase::empty() }

</t>
<t tx="tangentstorm.20220203082953.68">fn when_hi(&amp;mut self, v:vid::VID, n:NID)-&gt;NID { self.when(v, nid::I, n) }
</t>
<t tx="tangentstorm.20220203082953.69">fn when_lo(&amp;mut self, v:vid::VID, n:NID)-&gt;NID { self.when(v, nid::O, n) }

</t>
<t tx="tangentstorm.20220203082953.7">impl VID {
  pub fn cmp_depth(&amp;self, other: &amp;Self) -&gt; VidOrdering {
    use self::VidOrdering::*;
    match self.v {
      T =&gt; if other.v == T { Level } else { Below },
      NoV =&gt; match other.v {
        T   =&gt; Above,
        NoV =&gt; Level,
        _   =&gt; Below },
      Var(x) =&gt; match other.v {
        Vir(_) =&gt; Below,
        Var(y) =&gt; cmp_depth_idx(x,&amp;y),
        NoV|T =&gt; Above },
      Vir(x) =&gt; match other.v {
        Var(_) =&gt; Above,
        Vir(y) =&gt; cmp_depth_idx(x,&amp;y),
        NoV|T =&gt; Above }}}}

</t>
<t tx="tangentstorm.20220203082953.70">fn def(&amp;mut self, s:String, v:vid::VID)-&gt;NID {
  let nid = NID::from_vid(v);
  self.tag(nid, format!("{}{:?}", s, v)) }

</t>
<t tx="tangentstorm.20220203082953.71">fn tag(&amp;mut self, n:NID, s:String)-&gt;NID {
  let n = n;
  self.tags.insert(s, n); n }


</t>
<t tx="tangentstorm.20220203082953.72">fn and(&amp;mut self, x:NID, y:NID)-&gt;NID {
  if let Some(nid) = simp::and(x,y) { nid }
  else {
    let (lo, hi) = if x&lt;y {(x,y)} else {(y,x)};
    self.nid(ops::and(lo, hi)) }}

</t>
<t tx="tangentstorm.20220203082953.73">fn xor(&amp;mut self, x:NID, y:NID)-&gt;NID {
  if let Some(nid) = simp::xor(x,y) { nid }
  else {
    let (lo, hi) = if x&lt;y {(x,y)} else {(y,x)};
    self.nid(ops::xor(lo, hi)) }}

</t>
<t tx="tangentstorm.20220203082953.74">fn or(&amp;mut self, x:NID, y:NID)-&gt;NID {
  if let Some(nid) = simp::or(x,y) { nid }
  else if x.is_inv() &amp;&amp; y.is_inv() { !self.and(x, y) }
  else {
    let (lo, hi) = if x&lt;y {(x,y)} else {(y,x)};
    self.nid(ops::vel(lo, hi)) }}

</t>
<t tx="tangentstorm.20220203082953.75">fn sub(&amp;mut self, _v:vid::VID, _n:NID, _ctx:NID)-&gt;NID { todo!("ast::sub") }

</t>
<t tx="tangentstorm.20220203082953.76">fn get(&amp;self, s:&amp;str)-&gt;Option&lt;NID&gt; { Some(*self.tags.get(s)?) }
</t>
<t tx="tangentstorm.20220203082953.77">fn save(&amp;self, path:&amp;str)-&gt;::std::io::Result&lt;()&gt; {
  let s = bincode::serialize(&amp;self).unwrap();
  io::put(path, &amp;s) }

</t>
<t tx="tangentstorm.20220203082953.78">// generate dot file (graphviz)
fn dot(&amp;self, n:NID, wr: &amp;mut dyn std::fmt::Write) {
  macro_rules! w {
    ($x:expr $(,$xs:expr)*) =&gt; { writeln!(wr, $x $(,$xs)*).unwrap() }}
  macro_rules! dotop {
    ($s:expr, $n:expr $(,$xs:expr)*) =&gt; {{
      w!("  \"{}\"[label={}];", nid::raw($n), $s); // draw the node
      $({ if nid::is_inv($xs) { w!("edge[style=dashed];"); }
          else { w!("edge[style=solid];"); }
          w!(" \"{}\"-&gt;\"{}\";", nid::raw($xs), nid::raw($n)); })* }}}

  w!("digraph bdd {{");
  w!("rankdir=BT;"); // put root on top
  w!("node[shape=circle];");
  w!("edge[style=solid];");
  self.walk(n, &amp;mut |n| {
    match n {
      nid::O =&gt; w!(" \"{}\"[label=⊥];", n),
      nid::I =&gt; w!(" \"{}\"[label=⊤];", n),
      _ if n.is_vid() =&gt; w!("\"{}\"[label=\"{}\"];", nid::raw(n), n.vid()),
      _ =&gt; {
        let rpn: Vec&lt;NID&gt; = self.get_ops(n).to_rpn().cloned().collect();
        let fun = rpn.last().unwrap();
        if let Some(2) = fun.arity() {
          let (x, y) = (rpn[0], rpn[1]);
          match *fun {
            ops::AND =&gt; dotop!("∧",n,x,y),
            ops::XOR =&gt; dotop!("≠",n,x,y),
            ops::VEL =&gt; dotop!("∨",n,x,y),
            _ =&gt; panic!("unexpected op in dot(): {:?}", n) }}
        else { panic!("can't dot arbitrary ops yet: {:?}", rpn) }}}});
  w!("}}"); }
</t>
<t tx="tangentstorm.20220203082953.79">pub struct ASTBase { base: Simplify&lt;RawASTBase&gt; }
@others
</t>
<t tx="tangentstorm.20220203082953.8">pub fn topmost(x:VID, y:VID)-&gt;VID { if x.is_above(&amp;y) { x } else { y }}
</t>
<t tx="tangentstorm.20220203082953.80">impl Base for ASTBase {
  inherit![when_hi, when_lo, and, xor, or, def, tag, get, sub, save, dot ];
  fn new()-&gt;Self { ASTBase{ base: Simplify{ base: &lt;RawASTBase as Base&gt;::new() }}}}

</t>
<t tx="tangentstorm.20220203082953.81">impl ASTBase {
  pub fn empty()-&gt;Self { ASTBase { base: Simplify{ base: RawASTBase::empty() }}}
  pub fn raw_ast(&amp;self)-&gt;&amp;RawASTBase { &amp;self.base.base }}

</t>
<t tx="tangentstorm.20220203082953.82">test_base_consts!(ASTBase);
test_base_when!(ASTBase);

#[test]
fn ast_and(){
  let mut b = ASTBase::empty();
  let x0 = NID::var(0); let x1 = NID::var(1);
  let x01 = b.and(x0,x1);
  let x10 = b.and(x1,x0);
  assert_eq!(x01, x10, "expect $0 &amp; $1 == $1 &amp; $0"); }
</t>
<t tx="tangentstorm.20220203082953.9">pub fn botmost(x:VID, y:VID)-&gt;VID { if x.is_below(&amp;y) { x } else { y }}
</t>
</tnodes>
</leo_file>
