/// Swap Solver
use hashbrown::{HashMap, hash_map::Entry};
use {base::{Base,GraphViz}, vid::VID, nid, nid::NID, bdd::BDDBase};
use vhl::{HiLo, VHL, Walkable};

/// represents a single row in a VHL-graph
struct VHLRow {
  /** variable on which to branch   */  v: VID,
  /** hi/lo pairs                   */  hl: Vec<HiLo>,
  /** corresponding refcounts       */  rc: Vec<usize>,
  /** index for avoiding duplicates */  ix: HashMap<HiLo,usize>,
  /** free list (slots where rc=0)  */  fl: Vec<usize>}

impl VHLRow {
  fn new(v:VID)->Self { VHLRow{ v, hl:vec![], rc:vec![], ix:HashMap::new(), fl:vec![] }}

  fn add_ref(&mut self, hl0:HiLo)->NID {
    let inv = hl0.lo.is_inv();
    let hl = if inv { !hl0 } else { hl0 };
    let res = match self.ix.entry(hl) {
      Entry::Occupied (e) => {
        let nid = NID::from_vid_idx(self.v, *e.get() as nid::IDX);
        self.rc[nid.idx()] += 1;
        nid }
      Entry::Vacant(e) => {
        let idx = self.rc.len();
        let nid = NID::from_vid_idx(self.v, idx as nid::IDX);
        e.insert(idx);
        self.hl.push(hl);
        self.rc.push(1);
        nid }};
    if inv { !res } else { res }}}


/// A VHL graph broken into separate rows for easy variable reordering.
struct VHLScaffold {
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

  /// like vix() but creates a new row for the vid if it doesn't currently exist
  fn ensure_vix(&mut self, v:VID)->usize {
    match self.vix(v) {
      Some(ix) => { ix }
      None => {
        let ix = self.vids.len();
        self.vids.push(v);
        self.rows.push(VHLRow::new(VID::var(ix as u32)));
        ix }}}

  /// add a reference to the given VHL (inserting it into the appropriate row if necessary)
  fn add_ref(&mut self, vhl: VHL)->NID {
    let ix = self.ensure_vix(vhl.v);
    self.rows[ix].add_ref(vhl.hilo()) }

  // return the internal VHL corresponding to the external nid
  fn invex(&self, ex:NID)->VHL {
    if let Some(ix) = self.vix(ex.vid()) {
      let HiLo{ hi, lo } = self.rows[ix].hl[ex.idx()].clone();
      VHL{ v: VID::var(ix as u32), hi, lo }}
    else { panic!("nid {} is not in the scaffold.", ex)}}

  fn top_vid(&self)->VID {
    if let Some(&v) = self.vids.last() { v }
    else { VID::nov() }}}

impl GraphViz for VHLScaffold {
  fn write_dot(&self, n:NID, wr: &mut dyn std::fmt::Write) {
    macro_rules! w { ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap(); }}
    w!("digraph VHL {{");
    w!("subgraph head {{ h1[shape=plaintext; label=\"VHL\"] }}");
    w!("  O[label=⊥; shape=square];");
    w!("  I[label=⊤; shape=square];");
    w!("node[shape=circle];");
    for (i, (&ev, row)) in self.vids.iter().zip(self.rows.iter()).enumerate() {
      write!(wr, "{{rank=same ");
      for i in 0..row.hl.len() { write!(wr, " \"{}\"", NID::from_vid_idx(row.v, i as nid::IDX)); }
      w!("}}");
      for (j, hl) in row.hl.iter().enumerate() {
        let n = NID::from_vid_idx(row.v, j as nid::IDX);
        w!("  \"{}\"[label=\"{}\"];", n, ev);  // draw the nid itself
        let arrow = |n:NID| if n.is_const() || !n.is_inv() { "normal" } else { "odot" };
        let sink = |n:NID| if n.is_const() { n } else { nid::raw(n) };
        w!("edge[style=solid, arrowhead={}];", arrow(hl.hi));
        w!("  \"{}\"->\"{}\";", n, sink(hl.hi));
        w!("edge[style=dashed, arrowhead={}];", arrow(hl.lo));
        w!("  \"{}\"->\"{}\";", n, sink(hl.lo));
      }
    }
    w!("}}"); }}

pub struct SwapSolver<T:Base + Walkable> {
  /** normal base for delegation    */  base: T,
  /** base nid for last src def     */  key: NID,
  /** the new "top" at each step    */  src: VHLScaffold,
  /** the solution we're building   */  dst: VHLScaffold }

impl<T:Base + Walkable> SwapSolver<T> {

    /// constructor
    fn new(base: T, top:VID)->Self {
      SwapSolver{ base, key:nid::O, src: VHLScaffold::empty(), dst: VHLScaffold::new(top) }}

    /// return internal vhl for external nid n
    fn invex(&self, n:NID)->VHL { self.dst.invex(n) }

    /// rebuilds the "src" scaffold from self.key (which refers to a node in self.base)
    fn rebuild_src(&mut self)->NID {
      let mut map:HashMap<NID,NID> = HashMap::new(); // bdd nids -> scaffold nids
      self.src = VHLScaffold::empty();
      if self.key.is_const() { panic!("cannot rebuild src from constant key.") }
      let mut heap = self.base.as_heap(self.key);
      let (mut i, mut last_vid) = (0, VID::nov());
      while let Some((VHL{ v, hi, lo }, bnid)) = heap.pop() {
        if v != last_vid { // starting a new row:
          self.src.vids.push(v);
          self.src.rows.push(VHLRow::new(VID::var(i)));
          last_vid = v;
          i += 1; }
        // both hi and lo should be known to us, since we're traversing bottom-up.
        let hi1 = if hi.is_const() { hi } else { *map.get(&hi).expect("reference to unvisited hi node(!?)") };
        let lo1 = if lo.is_const() { lo } else { *map.get(&lo).expect("reference to unvisited lo node(!?)") };
        map.insert(bnid, self.src.add_ref( VHL{ v, hi:hi1, lo:lo1 })); }
      map[&self.key] }}

impl<T:Base + Walkable> Base for SwapSolver<T> {
  inherit![ new, num_vars, when_hi, when_lo, def, tag, get, save, dot ];

  fn and(&mut self, x:NID, y:NID)->NID { self.key = self.base.and(x,y); self.key }
  fn xor(&mut self, x:NID, y:NID)->NID { self.key = self.base.xor(x,y); self.key }
  fn or(&mut self, x:NID, y:NID)->NID  { self.key = self.base.or(x,y);  self.key }

  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID {
    assert_eq!(v, self.dst.top_vid(), "can only sub(v,n,ctx) if v is top vid in the scaffold.");
    assert_eq!(n, self.key, "can only sub(v,n,ctx) if n is result of last and/or/xor call.");
    NID::from_vid_idx(n.vid(), 0)}}

pub type BddSwapSolver = SwapSolver<BDDBase>;

#[test]
fn test_scaffold() {
  // a: ast node, v: vir
  let a5 = NID::vir(5); let v5 = a5.vid();
  let a4 = NID::vir(4); let v4 = a4.vid();
  let a3 = NID::vir(3);
  let a2 = NID::vir(2);
  let mut s = BddSwapSolver::new(BDDBase::new(0), v5);
  assert_eq!(v5, s.dst.vids[0], "label v5 should map to x0 after new(v5)");
  let key = s.and(a4, a2);
  let key = s.xor(key, a3);
  let res = s.sub(v5, key, a5);
  assert_eq!(res, NID::from_vid_idx(v4, 0), "(v4 AND v2) should have v4 at top");
  s.base.show_named(s.key, "bdd");
  let n = s.rebuild_src();
  s.src.show_named(n, "scaffold");
}
