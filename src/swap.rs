/// Swap Solver
use hashbrown::HashMap;
use {base::Base, vid::VID, nid, nid::NID, vhl::HiLo, bdd::BDDBase};

/// represents a single row in a VHL-graph
pub struct VHLRow {
  /** variable on which to branch   */  v: VID,
  /** hi/lo pairs                   */  hl: Vec<HiLo>,
  /** corresponding refcounts       */  rc: Vec<usize>,
  /** index for avoiding duplicates */  ix: HashMap<HiLo,usize>,
  /** free list (slots where rc=0)  */  fl: Vec<usize>}

impl VHLRow {
  fn new(v:VID)->Self { VHLRow{ v, hl:vec![], rc:vec![], ix:HashMap::new(), fl:vec![] } }}


/// A VHL graph broken into separate rows for easy variable reordering.
pub struct VHLScaffold {
  /** the rows of the structure     */  rows: Vec<VHLRow>,
  /** corresponding vids            */  vids: Vec<VID>}

impl VHLScaffold {
  fn new()->Self { VHLScaffold{ rows: vec![], vids: vec![] }}}

pub struct SwapSolver<T:Base> {
  /** normal base for delegation    */  base: T,
  /** the new "top" at each step    */  src: VHLScaffold,
  /** the solution we're building   */  dst: VHLScaffold }

impl<T:Base> SwapSolver<T> {
    fn new(base: T)->Self {
      SwapSolver{ base, src: VHLScaffold::new(), dst: VHLScaffold::new() }}}

impl<T:Base> Base for SwapSolver<T> {
  inherit![ new, when_hi, when_lo, and, xor, num_vars, or, def, tag, get, save, dot ];
  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID {
    ctx }}


#[test]
fn test_swap() {
  let s = SwapSolver::new(BDDBase::new(0));
}
