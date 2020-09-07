/// Swap Solver
use hashbrown::HashMap;
use {base::Base, vid::VID, nid, nid::NID, vhl::{HiLo,VHL}, bdd::BDDBase};

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
  /** corresponding vid labels      */  vids: Vec<VID>}

impl VHLScaffold {

  /// Construct an empty scaffold.
  fn empty()->Self { VHLScaffold{ rows: vec![], vids: vec![] }}

  /// Construct a scaffold representing a single variable.
  fn new(top:VID)->Self { VHLScaffold{ vids: vec![top], rows: vec![VHLRow::new(VID::var(0))] }}

  /// return the index (height) of the given variable within the scaffold (if it exists)
  fn vix(&self, v:VID)->Option<usize> { self.vids.iter().position(|&x| x == v) }

  /// return the variable at the given height
  fn vat(&self, ix:usize)->VID { self.vids[ix] }

  // return the internal VHL corresponding to the external nid
  fn invex(&self, ex:NID)->VHL {
    if let Some(ix) = self.vix(ex.vid()) {
      let HiLo{ hi, lo } = self.rows[ix].hl[ex.idx()].clone();
      VHL{ v: VID::var(ix as u32), hi, lo }}
    else { panic!("nid {} is not in the scaffold.", ex)}}

  fn top(&self)->VID {
    if let Some(&v) = self.vids.last() { v }
    else { VID::nov() }}}

pub struct SwapSolver<T:Base> {
  /** normal base for delegation    */  base: T,
  /** base nid for last src def     */  key: NID,
  /** the new "top" at each step    */  src: VHLScaffold,
  /** the solution we're building   */  dst: VHLScaffold }

impl<T:Base> SwapSolver<T> {

    fn new(base: T, top:VID)->Self {
      SwapSolver{ base, key:nid::O, src: VHLScaffold::empty(), dst: VHLScaffold::new(top) }}

    fn vhl(&self, n:NID)->VHL { self.dst.invex(n) }}

impl<T:Base> Base for SwapSolver<T> {
  inherit![ new, num_vars, when_hi, when_lo, def, tag, get, save, dot ];

  fn and(&mut self, x:NID, y:NID)->NID { self.key = self.base.and(x,y); self.key }
  fn xor(&mut self, x:NID, y:NID)->NID { self.key = self.base.xor(x,y); self.key }
  fn or(&mut self, x:NID, y:NID)->NID  { self.key = self.base.or(x,y);  self.key }

  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID {
    assert_eq!(v, self.dst.top(), "can only sub(v,n,ctx) if v is top var in the scaffold.");
    assert_eq!(n, self.key, "can only sub(v,n,ctx) if n is result of last and/or/xor call.");
    NID::from_vid_idx(n.vid(), 0)}}

pub type BddSwapSolver = SwapSolver<BDDBase>;

#[test]
fn test_scaffold() {
  // a: ast node, v: vir
  let a5 = NID::vir(5); let v5 = a5.vid();
  let a4 = NID::vir(4); let v4 = a4.vid();
  let a2 = NID::vir(2);
  let mut s = BddSwapSolver::new(BDDBase::new(0), v5);
  assert_eq!(v5, s.dst.vids[0], "label v5 should map to x0 after new(v5)");
  let key = s.and(a4, a2);
  let res = s.sub(v5, key, a5);
  assert_eq!(res, NID::from_vid_idx(v4, 0), "(v4 AND v2) should have v4 at top");

}
