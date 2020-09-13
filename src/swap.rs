/// Swap Solver
use hashbrown::{HashMap, hash_map::Entry};
use {base::{Base,GraphViz}, vid::VID, nid, nid::NID, bdd::BDDBase};
use vhl::{HiLo, VHL, Walkable};
use std::mem;

/// index + refcount (used by VHLRow)
struct IxRc { ix:nid::IDX, rc: u32 }

/// represents a single row in a VHL-graph
struct VHLRow {
  /** (external) branch vid label   */  v: VID,
  /** (internal) hilo pairs         */  hl: Vec<HiLo>,
  /** index and refcounts for hilos */  ix: HashMap<HiLo,IxRc>,
  /** total of ix[].1 (refcounts)   */  trc: u32,
  /** refcount for this row's vid   */  vrc: u32,
  /** free list (slots where rc=0)  */  fl: Vec<usize>}

impl VHLRow {
  fn new(v:VID)->Self { VHLRow{ v, hl:vec![], trc:0, vrc:0, ix:HashMap::new(), fl:vec![] }}

  pub fn print(&self) {
    print!("v:{} rc:{} [", self.v, self.vrc);
    for hl in &self.hl { print!(" ({}, {})", hl.hi, hl.lo)}
    println!(" ]"); }

  fn add_vid_ref(&mut self) { self.vrc += 1 }

  /// add a reference to the given (internal) hilo pair, inserting it into the row if necessary.
  /// returns the external nid, and a flag indicating whether the pair was freshly added.
  /// (if it was fresh, the scaffold needs to update the refcounts for each leg)
  fn add_ref(&mut self, hl0:HiLo, rc:u32)->(NID, bool) {
    assert!( !(hl0.hi.is_const() && hl0.lo.is_const()), "call add_vid_ref for pure vid references");
    let inv = hl0.lo.is_inv();
    let hl = if inv { !hl0 } else { hl0 };
    let (res, isnew) = match self.ix.entry(hl) {
      Entry::Occupied (mut e) => {
        let nid = NID::from_vid_idx(self.v, e.get().ix);
        e.get_mut().rc += rc;
        self.trc += rc;
        (nid, false) }
      Entry::Vacant(e) => {
        let idx = self.hl.len() as nid::IDX;
        let nid = NID::from_vid_idx(self.v, idx);
        e.insert(IxRc{ ix:idx, rc });
        self.hl.push(hl);
        (nid, true) }};
    (if inv { !res } else { res }, isnew)}}


/// A VHL graph broken into separate rows for easy variable reordering.
struct VHLScaffold {
  /** the rows of the structure     */  rows: Vec<VHLRow>,
  /** corresponding vid labels      */  vids: Vec<VID>}

impl VHLScaffold {

  /// Construct an empty scaffold.
  fn empty()->Self { VHLScaffold{ rows: vec![], vids: vec![] }}

  /// Construct a scaffold representing a single variable.
  fn new(top:VID)->Self { VHLScaffold{ vids: vec![top], rows: vec![VHLRow::new(top)] }}

  pub fn print(&self) {
    for (i, row) in self.rows.iter().enumerate().rev() {
      print!("row:{:3} ", i);
      row.print()}}

  /// return the index (height) of the given variable within the scaffold (if it exists)
  fn vix(&self, v:VID)->Option<usize> { self.vids.iter().position(|&x| x == v) }

  /// return the variable at the given height
  fn vat(&self, ix:usize)->VID { self.vids[ix] }

  /// add a new vid to the top of the stack. return its position.
  fn push(&mut self, v:VID)->usize {
    let ix = self.vids.len();
    self.vids.push(v);
    self.rows.push(VHLRow::new(v));
    ix }

  /// swap two adjacent vids/rows. s should be 1 row above r. (s==r+1)
  fn swap(&mut self, r:usize, s:usize) {
    // println!("-- initial state --------------------");
    // self.print();

    assert_eq!(r+1, s, "can only swap a row with the row above. (at least for now).");
    assert!(s < self.vids.len(), "can't swap a row that's not in the scaffold.");
    // start: y=vids[s] is above z=vids[r]
    let (y, z) = (self.vids[s], self.vids[r]);
    // goal: z=vids[s] is above y=vids[r]
    self.rows[s].v = z;
    self.rows[r].v = y;
    self.vids.swap(r, s);
    // row[s] (top) will now be rewritten in place.
    // row[r] *or any row below it* may have refcount changes.
    // that means we need self.rows to remain mutable, even while we borrow row[s]
    // so we can loop through it. So: just swap out row[s] with a shim.
    let shim = VHLRow::new(VID::nov());
    let mut toprow = mem::replace(&mut self.rows[s], shim);

    /*
      row s:   y ____                        z ____
               :     \                       :     \
      row r:   z __    z __      =>          y __    y __
               :   \    :  \                 :   \    :   \
               oo   oi  io  ii               oo   io  oi   ii
     */
    let mut new_hl = vec![];
    for old in toprow.hl.iter_mut() {

      // helper: if a branch points at row s fetch its hilo. else dup it for the swap
      let old_hilo = |sn:NID|->(NID, NID) {
        if sn.is_const() || sn.vid().var_ix() != r { (sn, sn) }
        else { // !!we have to undo the invert bit because add_ref will re-apply it.
               // TODO: this is really confusing. clean this mess up!!!
               let vhl = if sn.is_inv() { !self.invin(sn) } else { self.invin(sn) };
               (vhl.hi, vhl.lo) }};

      let (oi,oo) = old_hilo(old.lo);
      let (ii,io) = old_hilo(old.hi);

      // TODO: put this nid/vid swapping logic in one place
      let fix_nid = |old:NID|->NID {
        if old.is_const() { old }
        else {
          // println!("old vid:, {}", old.vid());
          let v = if old.vid() == VID::var(s as u32) { VID::var(r as u32) } else { old.vid() };
          if old.is_var() { NID::from_vid(v) }
          else { NID::from_vid_idx(v, old.idx() as u32) }}};

      let mut new_ref = |hi0, lo0|-> NID {
        // println!("new_ref({},{})", hi0, lo0);
        let (hi, lo) = (fix_nid(hi0), fix_nid(lo0));
        if hi==lo { hi } else { self.add_iref(r, HiLo{ hi, lo }, 1).0 }};

      // this is the new pair for the top row. the only way new.hi == new.lo
      // would be if oo=oi and oi==ii, but if that were true and they both branch on row s,
      // then old.hi == old.lo, and the node should never have been there in the first place.
      let hi = new_ref(oi,oo);
      let lo = new_ref(ii,io);
      let new = HiLo { hi, lo };
      assert_ne!(new.hi, new.lo, "swap should result in a distinct HiLo pair");
      new_hl.push(new); }
    toprow.hl = new_hl;
    self.rows[s] = toprow;
    // println!("-- swapped rows {} and {} ----------", r, s);
    // self.print();
    // println!("-------------------------------------");
  }

  /// like vix() but creates a new row for the vid if it doesn't currently exist
  fn ensure_vix(&mut self, v:VID)->usize {
    match self.vix(v) {
      Some(ix) => { ix }
      None => { self.push(v) }}}

  /// rename an existing row
  fn relabel(&mut self, old:VID, new:VID) {
    if let Some(old_ix) = self.vix(old) { self.vids[old_ix] = new; self.rows[old_ix].v = new; }
    else { panic!("can't relabel old vid {} because it wasn't in the scaffold", old) }}

  /// add reference to nid (using external vid)
  fn add_nid_ref(&mut self, n:NID) {
    if n.is_const() { }
    else {
      let vix = self.ensure_vix(n.vid());
      if n.is_var() { self.rows[vix].add_vid_ref() }
      else {
        let hilo = self.rows[vix].hl[n.idx()];
        if let Some(mut ixrc) = self.rows[vix].ix.get_mut(&hilo) { ixrc.rc += 1 }
        else { panic!("can't add ref to nid that isn't in the scaffold")}}}}

  /// add ref using internal index and hilo
  fn add_iref(&mut self, ix:usize, hl:HiLo, rc:u32)->(NID, bool) {
    match (hl.hi, hl.lo) {
      // TODO: put all the nid-swapping and ref counting and const/var checking in their own places!!
      (nid::I, nid::O) => { self.rows[ix].add_vid_ref(); ( NID::var(ix as u32), false) }
      (nid::O, nid::I) => { self.rows[ix].add_vid_ref(); (!NID::var(ix as u32), false) }
      _ => { self.rows[ix].add_ref(hl, rc) }}}


  /// add a reference to the given VHL (inserting it into the appropriate row if necessary)
  /// both the vhl and return NID use external variables
  fn add_ref(&mut self, vhl: VHL)->NID {
    let VHL { v, hi, lo } = vhl;
    let ix = self.ensure_vix(v);
    match (vhl.hi, vhl.lo) {
      (nid::I, nid::O) => { self.rows[ix].add_vid_ref();  NID::from_vid(v) }
      (nid::O, nid::I) => { self.rows[ix].add_vid_ref(); !NID::from_vid(v) }
      _ => {
        let (ihi, ilo) = (self.inen(hi), self.inen(lo));
        if !ihi.is_const(){assert!(ihi.vid().var_ix()<ix, "bad vhl: x{}.hi->{}", ix, ihi)}
        if !ilo.is_const(){assert!(ilo.vid().var_ix()<ix, "bad vhl: x{}.lo->{}", ix, ilo)}
        let (res, isnew) = self.add_iref(ix, HiLo{ hi:ihi, lo:ilo }, 1);
        if isnew { self.add_nid_ref(hi); self.add_nid_ref(lo); }
        res }}}

  /// return internal VHL from internal NID
  fn invin(&self, n:NID)->VHL {
    let v = n.vid();
    let res =
      if n.is_lit() { VHL { v, hi:nid::I, lo:nid::O } }
      else {
        let HiLo{ hi, lo } = self.rows[v.var_ix()].hl[n.idx()];
        VHL{ v, hi, lo }};
    if n.is_inv() { !res } else { res }}

  // return the internal VHL corresponding to the external nid
  fn invex(&self, ex:NID)->VHL {
    if let Some(ix) = self.vix(ex.vid()) {
      let HiLo{ hi, lo } = self.rows[ix].hl[ex.idx()];
      let res = VHL{ v: VID::var(ix as u32), hi, lo };
      if ex.is_inv() { !res } else { res }}
    else { panic!("nid {} is not in the scaffold.", ex)}}

  /// external nid from internal nid
  fn exin(&self, n:NID)->NID {
    if n.is_const() { return n }
    let ev = self.vids[n.vid().var_ix()];
    let res = if n.is_var() { NID::from_vid(ev) } else { NID::from_vid_idx(ev, n.idx() as u32) };
    if n.is_inv() { !res } else { res }}

  /// internal nid from external nid
  fn inen(&self, ne:NID)->NID {
    if ne.is_const() { return ne }
    let iv = if let Some(ix) = self.vix(ne.vid()) { VID::var(ix as u32) } else { panic!("inen({})", ne) };
    let res = if ne.is_var() { NID::from_vid(iv) } else { NID::from_vid_idx(iv, ne.idx() as u32) };
    if ne.is_inv() { !res } else { res }}

  /// return external vhl corresponding to external nid
  fn exvex(&self, ex:NID)->VHL {
    if let Some(ix) = self.vix(ex.vid()) {
      let res = if ex.is_var() { VHL{ v:ex.vid(), hi:nid::I, lo:nid::O } }
      else {
        let HiLo{ hi:h0, lo:l0 } = self.rows[ix].hl[ex.idx()];
        let hi = self.exin(h0);
        let lo = self.exin(l0);
        VHL{ v: ex.vid(), hi, lo }};
    if ex.is_inv() { !res } else { res }}
    else { panic!("nid {} is not in the scaffold.", ex)}}

  pub fn exvin(&self, n:NID)->VHL { self.exvex(self.exin(n)) }

  fn top_vid(&self)->VID {
    if let Some(&v) = self.vids.last() { v }
    else { VID::nov() }}}

impl GraphViz for VHLScaffold {
  fn write_dot(&self, o:NID, wr: &mut dyn std::fmt::Write) {
    assert_eq!(o, nid::O, "can't visualize individual nids yet. pass O for now");
    macro_rules! w { ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap(); }}
    w!("digraph VHL {{");
    w!("subgraph head {{ h1[shape=plaintext; label=\"VHL\"] }}");
    w!("  {{rank=same O I}}");
    w!("  O[label=⊥; shape=square];");
    w!("  I[label=⊤; shape=square];");
    w!("node[shape=circle];");
    for (i, (&ev, row)) in self.vids.iter().zip(self.rows.iter()).enumerate() {
      let iv = VID::var(i as u32);
      if !row.hl.is_empty() {
        write!(wr, "{{rank=same").unwrap();
        if row.vrc > 0 { write!(wr, " {}", ev).unwrap() }
        for i in 0..row.hl.len() { write!(wr, " \"{}\"", NID::from_vid_idx(ev, i as nid::IDX)).unwrap(); }
        w!("}}") }
      if row.vrc > 0 {
        w!(" {}[label=\"{}\"];", ev, ev);
        w!("edge[style=solid]; {}->I", ev);
        w!("edge[style=dashed]; {}->O", ev);}
      for (j, hl) in row.hl.iter().enumerate() {
        let n = NID::from_vid_idx(row.v, j as nid::IDX);
        w!("  \"{}\"[label=\"{}\"];", n, ev);  // draw the nid itself
        let arrow = |n:NID| if n.is_const() || !n.is_inv() { "normal" } else { "odot" };
        let sink = |n:NID| if n.is_const() { n } else { self.exin(nid::raw(n)) };
        w!("edge[style=solid, arrowhead={}];", arrow(hl.hi));
        w!("  \"{}\"->\"{}\";", n, sink(hl.hi));
        w!("edge[style=dashed, arrowhead={}];", arrow(hl.lo));
        w!("  \"{}\"->\"{}\";", n, sink(hl.lo)); }}
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

    /// rebuilds the "src" scaffold from self.key (which refers to a node in self.base)
    /// returns the internal nid
    fn rebuild_src(&mut self)->NID {
      let mut nmap:HashMap<NID,NID> = HashMap::new(); // bdd nids -> scaffold nids (external->internal)
      self.src = VHLScaffold::empty();
      if self.key.is_const() { panic!("cannot rebuild src from constant key.") }
      let mut heap = self.base.as_heap(self.key);
      let mut last_vid = VID::nov();
      while let Some((VHL{ v:ev, hi, lo }, bnid)) = heap.pop() {
        if ev != last_vid { self.src.push(ev); last_vid = ev }
        if bnid.is_var() { /* todo: add ref to var row? */ }
        else {
          // both hi and lo should be known to us, since we're traversing bottom-up.
          let hi1 = if hi.is_lit() { hi } else { *nmap.get(&hi).expect("reference to unvisited hi node(!?)") };
          let lo1 = if lo.is_lit() { lo } else { *nmap.get(&lo).expect("reference to unvisited lo node(!?)") };
          nmap.insert(bnid, self.src.add_ref( VHL{ v:ev, hi:hi1, lo:lo1 })); }}
      nmap[&self.key] }}

impl<T:Base + Walkable> Base for SwapSolver<T> {
  inherit![ new, num_vars, when_hi, when_lo, def, tag, get, save, dot ];

  fn and(&mut self, x:NID, y:NID)->NID { self.key = self.base.and(x,y); self.key }
  fn xor(&mut self, x:NID, y:NID)->NID { self.key = self.base.xor(x,y); self.key }
  fn or(&mut self, x:NID, y:NID)->NID  { self.key = self.base.or(x,y);  self.key }

  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID { // ( wv -> wyz )
    // println!("-------------------------------------------");
    // self.dst.print();
    // println!("\nsub({} -> {} in {})", v, n, ctx);
    if self.dst.top_vid() != v {
      if let Some(ix) = self.dst.vix(v) {
        println!("{:?} // current level of v: vix({})->{}", self.dst.vids, v, ix);
        for i in ix .. self.dst.vids.len()-1 {
          self.dst.swap(i, i+1);
         // println!("{:?} // swap({} -> {})", self.dst.vids, i, i+1);
        }
       }
      else { panic!("can't sub(v:{},...) when v isn't even in the scaffold.", v) }}
    assert_eq!(v, self.dst.top_vid(), "can only sub(v,n,ctx) if v is top vid in the scaffold.");
    assert_eq!(n, self.key, "can only sub(v,n,ctx) if n is result of last and/or/xor call.");
    let top = self.rebuild_src();
    assert_eq!(self.src.vids.len(), 2, "src scaffold should use exactly 2 vids");
    let (y, z) = (self.src.vids[0], self.src.vids[1]);
    let (yix, zix) = (self.dst.vix(y), self.dst.vix(z));
    let sz:VHL = self.src.invex(top);
    let dz:VHL = match (yix, zix) {
      (None, None) => {
        let VHL {v:x, hi:dy1, lo:dy0 } = self.dst.exvex(ctx);
        assert_eq!(x, v, "replacing {} but ctx branched on {}", v, x);
        self.dst.relabel(x, y);
        self.dst.push(z);
        // dy is the node corresponding to src[$x0]... it's the old top (ctx, but with new label y instead of v)
        let dy = if ctx.is_var() { NID::from_vid(y) } else { NID::from_vid_idx(y, ctx.idx() as u32) };
        let map = |old| match old {
          nid::I => dy1,
          nid::O => dy0,
          fx0 if fx0 ==  NID::var(0) => dy,
          fx0 if fx0 == !NID::var(0) => !dy,
          _ => panic!("what? how is anything below $x1 not in {{ I, O, $x0, ~$x0 }}??") };
        VHL { v:z, hi:map(sz.hi), lo:map(sz.lo) }}
      (None, Some(_zi)) => { todo!("z exists, y is new: swap y and z!") }
      (Some(yi), None) => {
        if yi != self.dst.vids.len()-2 { todo!("<<lift y to just under the top level!>>") }
        let path = |zx,yx| {
          let mut n = if zx==1 { sz.hi } else { sz.lo };
          if n.is_var() {
            assert!(n.vid().var_ix() == 0, "should be branching on x0!!");
            let sy = self.src.invin(n);
            n = if yx==1 { sy.hi } else { sy.lo }}
            else {} // it doesn't branch on y, so nothing to do
            assert!( (n==nid::I) || (n==nid::O), "path should lead to a constant!");
            n == nid::I };
        // these are all external nodes:
        let vhl = self.dst.exvex(ctx);
        let VHL {v:x, hi:x1, lo:x0 } = self.dst.exvex(ctx);
        let VHL {v:_, hi:x11, lo:x10 } = self.dst.exvex(x1);
        let VHL {v:_, hi:x01, lo:x00 } = self.dst.exvex(x0);
        let oo = if path(0, 0) { x01 } else { x00 };
        let oi = if path(0, 1) { x01 } else { x00 };
        let io = if path(1, 0) { x11 } else { x10 };
        let ii = if path(1, 1) { x11 } else { x10 };
        let zo = if oo == oi { oo } else { self.dst.add_ref(VHL{ v:y, hi:oi, lo:oo }) };
        let zi = if io == ii { ii } else { self.dst.add_ref(VHL{ v:y, hi:ii, lo:io }) };
        self.dst.relabel(x, z);
        VHL{ v:z, hi:zi, lo:zo } }
      (Some(_yi), Some(_zi)) => { todo!("both y and z exist") }};
    // todo: dec-ref to old top node.
    self.dst.add_ref(dz) }}

pub type BddSwapSolver = SwapSolver<BDDBase>;

/// test for subbing in two new variables
#[test] fn test_two_new() {
  // a: ast node, v: vir
  let a5 = NID::vir(5); let v5 = a5.vid();
  let a4 = NID::vir(4); let v4 = a4.vid();
  let a2 = NID::vir(2);
  let mut s = BddSwapSolver::new(BDDBase::new(0), v5);
  assert_eq!(v5, s.dst.vids[0], "label v5 should map to x0 after new(v5)");
  let key = s.and(a4, a2);
  let res = s.sub(v5, key, a5);
  // s.dst.print(); //  s.dst.show_named(nid::O, "dst");
  assert_eq!(s.dst.exvex(res), VHL { v:v4, hi:a2, lo:nid::O },
    "(v4 AND v2) should be (v4 ? v2 : O)"); }


/// test for subbing in one new variable
#[test] fn test_one_new() {
  let nz = NID::vir(3); let z = nz.vid();
  let ny = NID::vir(2); let y = ny.vid();
  let nx = NID::vir(1); let x = nx.vid();
  let nw = NID::vir(0); let w = nw.vid();
  // we start with just z on top:
  let mut s = BddSwapSolver::new(BDDBase::new(0), z);
  // substitute z -> w ^ y:
  let key = s.xor(nw, ny);
  let wy = s.sub(z, key, nz);
  // substitute y -> x & w  (one new var, one old var)
  // so (w ^ y) -> (w ^ (x & w))
  let key = s.and(nx, nw);
  let wxw = s.sub(y, key, wy);
  assert_eq!(s.dst.exvex(wxw), VHL { v:x, hi:!nw, lo:nid::O },
    "(w ^ (x & w)) should be (x ? !w : O)"); }


#[test] fn test_swap() {
  let nz = NID::vir(3); let z = nz.vid();
  let ny = NID::vir(2); let y = ny.vid();
  let mut s = BddSwapSolver::new(BDDBase::new(0), z);
  let key = s.and(nz, !ny);
  s.rebuild_src();
  // s.src.print();
  assert_eq!(s.src.top_vid(), z, "z=v3 should start out on top");
  assert_eq!(s.src.exvex(key), VHL { v:z, hi:!ny, lo:nid::O },
    "(z ^ !y) should be (z ? !y : O)");
  // println!("key: {}", key);
  let internal = s.src.inen(key);
  s.src.swap(0,1);
  assert_eq!(s.src.top_vid(), y, "y=v2 should be on top now");
  // s.src.print();
  // println!("src.vids: {:?}", s.src.vids);
  // TODO: double check this.
  assert_eq!(s.src.exin(internal), NID::from_vid_idx(s.src.top_vid(),0));
  assert_eq!(s.src.exvin(internal), VHL { v:y, hi:nid::O, lo:nz },
    "after swap (z ^ !y) should be (y ? O : z)"); }

#[cfg(feature="skipped_tests")]
/// test for subbing in two existing variables
#[test] fn test_two_old() {
  let nz = NID::vir(4); let z = nz.vid();
  let ny = NID::vir(3); let y = ny.vid();
  let nx = NID::vir(2); let x = nx.vid();
  let nw = NID::vir(1); let w = nw.vid();
  let nv = NID::vir(0); let v = nv.vid();
  let mut s = BddSwapSolver::new(BDDBase::new(0), z);
  // we start with just z on top:     (z)
  // substitute z -> y ^ x          = (y ^ x)
  let key = s.xor(ny, nx);
  let res = s.sub(z, key, nz);
  // substitute y -> w | v          = ((w|v)^x)
  let key = s.or(nw, nv);
  let res = s.sub(y, key, res);
  // substitute x -> v & w          = ((w|v)^(w&v))
  let key = s.and(nv, nw);
  let res = s.sub(x, key, res);
  // simplification:                = w ^ v
  assert_eq!(s.dst.exvex(res), VHL { v:w, hi:!nv, lo:nv },
    "((w|v) ^ (w&v)) should be (w ? !v : v)"); }
