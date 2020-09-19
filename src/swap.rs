/// Swap Solver
use std::slice::Iter;
use hashbrown::{HashMap, hash_map::Entry};
use {base::{Base,GraphViz,SubSolver}, vid::VID, nid, nid::NID, bdd::BDDBase};
use vhl::{HiLo, VHL, Walkable};
use std::mem;
use std::cmp::Ordering;

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

  fn print(&self) {
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

  #[allow(dead_code)]
  fn print(&self) {
    println!("{:?}", self.vids);
    for (i, row) in self.rows.iter().enumerate().rev() {
      print!("row:{:3} ", i);
      row.print()}}

  /// return the index (height) of the given variable within the scaffold (if it exists)
  fn vix(&self, v:VID)->Option<usize> { self.vids.iter().position(|&x| x == v) }

  /// add a new vid to the top of the stack. return its position.
  fn push(&mut self, v:VID)->usize {
    let ix = self.vids.len();
    self.vids.push(v);
    self.rows.push(VHLRow::new(v));
    ix }

  /// drop top var v (double check that it's actually on top)
  fn drop(&mut self, v:VID) {
    if *self.vids.last().expect("can't drop from empty scaffold") == v {
      self.vids.pop();
      self.rows.pop(); }
    else { panic!("can't pop {} because it's not on top ({:?})", v, self.vids) }}

  /// swap two adjacent vids/rows. s should be 1 row above r. (s==r+1)
  fn swap(&mut self, r:usize, s:usize) {
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
    let mut old_rc = vec![];
    for old in toprow.hl.iter() {

      if let Some(ixrc) = toprow.ix.get(old) { old_rc.push(ixrc.rc) }
      else { println!("weird: no reference to {:?}", old); old_rc.push(0) }

      // helper: if a branch points at row s fetch its hilo. else dup it for the swap
      let old_hilo = |sn:NID|->(NID, NID) {
        if sn.is_const() || sn.vid().var_ix() != r { (sn, sn) }
        else { let vhl = self.invin(sn); (vhl.hi, vhl.lo) }};

      let (oi,oo) = old_hilo(old.lo);
      let (ii,io) = old_hilo(old.hi);

      // TODO: put this nid/vid swapping logic in one place
      let fix_nid = |old:NID|->NID {
        if old.is_const() { old }
        else {
          let inv = old.is_inv(); let old = if inv {!old} else {old};
          let v = if old.vid() == VID::var(s as u32) { VID::var(r as u32) } else { old.vid() };
          let new = if old.is_var() { NID::from_vid(v) } else { NID::from_vid_idx(v, old.idx() as u32) };
          if inv { !new } else { new }}};

      let mut new_ref = |hi0, lo0|-> NID {
        let (hi, lo) = (fix_nid(hi0), fix_nid(lo0));
        if hi==lo { hi } else { self.add_iref(r, HiLo{ hi, lo }, 1).0 }};

      // this is the new pair for the top row. the only way new.hi == new.lo
      // would be if oo=oi and oi==ii, but if that were true and they both branch on row s,
      // then old.hi == old.lo, and the node should never have been there in the first place.
      let lo = new_ref(io,oo); // oo stays same, oi->io
      let hi = new_ref(ii,oi); // ii stays same, io->oi
      let new = HiLo { hi, lo };
      // !! the following is normally true but *not* when lifting the var that's
      //    just been replaced with a function of two items already in the tree.
      // assert_ne!(new.hi, new.lo, "swap should result in a distinct HiLo pair");
      new_hl.push(new); }

    // rebuild index. refcounts remain the same.
    let mut new_ix = HashMap::new();
    for (ix, (&hl, &rc)) in new_hl.iter().zip(old_rc.iter()).enumerate() {
      new_ix.insert(hl, IxRc{ ix: ix as u32, rc }); }

    toprow.hl = new_hl;
    toprow.ix = new_ix;
    self.rows[s] = toprow;}

  /// lift var v to height ix, by repeatedly swapping upward
  fn lift(&mut self, v:VID, ix:usize) {
    if let Some(vix) = self.vix(v) {
      if vix > ix { panic!("this is not a lift operation. {} > {}", vix, ix) }
      if vix == ix { return }
      for i in vix .. ix { self.swap(i,i+1) }}
    else { panic!("can't lift {} because it isn't in the scaffold!", v) }}

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
        else { panic!("can't add ref to nid ({}) that isn't in the scaffold", n)}}}}

  /// add ref using internal index and hilo. returns internal nid and whether it was new
  fn add_iref(&mut self, ix:usize, hl:HiLo, rc:u32)->(NID, bool) {
    let (nid, isnew) = match (hl.hi, hl.lo) {
      // TODO: put all the nid-swapping and ref counting and const/var checking in their own places!!
      (nid::I, nid::O) => { self.rows[ix].add_vid_ref(); ( NID::var(ix as u32), false) }
      (nid::O, nid::I) => { self.rows[ix].add_vid_ref(); (!NID::var(ix as u32), false) }
      _ => {
        let (ex, isnew) = self.rows[ix].add_ref(hl, rc);
        (self.inen(ex), isnew) }};
    (nid,isnew)}


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
        let res = self.exin(res);
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
    let iv = if let Some(ix) = self.vix(ne.vid()) { VID::var(ix as u32) } else {
      panic!("inen({}) can't work because {} not in self.vids:{:?}", ne, ne.vid(), self.vids) };
    let res = if ne.is_var() { NID::from_vid(iv) } else { NID::from_vid_idx(iv, ne.idx() as u32) };
    if ne.is_inv() { !res } else { res }}

  /// return external vhl corresponding to external nid
  fn exvex(&self, ex:NID)->VHL {
    if ex.is_const() { return VHL { v: VID::nov(), hi:ex, lo:ex }}
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
    else { VID::nov() }}

  // this is a helper function for following a path in the source down to its leaves
  fn path(&self, top:NID, steps:&[(VID,u8)])->NID {
    let mut res=top; let mut i = 0; let mut vhl = self.exvex(res);
    println!("---------------------------------------");
    self.print();
    println!("steps: {:?}", steps);
    loop {
      // scaffold might skip over some vars since it's a reduced structure:
      println!("vhl: {:?}", vhl);
      while i < steps.len() && steps[i].0 != vhl.v {
        println!("skipping var : {}", vhl.v);
        i+= 1 }
      println!("i: {}", i);
      // path must specify all vars, in order:
      // but if we reach a node that doesn't branch on the next variable,
      // then assume it's below all our variables in the scaffold.
      if i == steps.len() || steps[i].0 != vhl.v { return res }
      res = if steps[i].1 == 1 { vhl.hi } else { vhl.lo };
      if i == steps.len() || res.is_const() { return res }
      else { i+=1; vhl = self.exvex(res) }}}

  fn const_path(&self, top:NID, steps:&[(VID, u8)])->bool {
    let res = self.path(top, steps);
    if !res.is_const() { panic!("path lead to {}, not {{I,O}}", res) }
    res == nid::I }

} // impl VHLScafffold

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
    for (&ev, row) in self.vids.iter().zip(self.rows.iter()) {
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
  /** the solution we're building   */  dst: VHLScaffold}

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
    #[allow(deprecated)] let mut heap = self.base.as_heap(self.key); // i just don't want anyone else using this.
    let mut last_vid = VID::nov();
    while let Some((VHL{ v:ev, hi, lo }, bnid)) = heap.pop() {
      if ev != last_vid { self.src.push(ev); last_vid = ev }
      if bnid.is_var() { /* todo: add ref to var row? */ }
      else {
        // both hi and lo should be known to us, since we're traversing bottom-up.
        let hi1 = if hi.is_lit() { hi } else { *nmap.get(&hi).expect("reference to unvisited hi node(!?)") };
        let lo1 = if lo.is_lit() { lo } else { *nmap.get(&lo).expect("reference to unvisited lo node(!?)") };
        nmap.insert(bnid, self.src.add_ref( VHL{ v:ev, hi:hi1, lo:lo1 })); }}
    nmap[&self.key] }

  pub fn solutions_trunc(&self, _n:NID, _regsize:usize)->Iter<'static, Reg> {
    println!("TODO: solutions_trunc");
    // TODO: garbage collection first!
    // for v in &self.dst.vids { println!("v: {}", v); }
    self.dst.print();
    EMPTYVEC.iter() }
}
use reg::Reg;
static EMPTYVEC: Vec<Reg> = vec![];


impl<T:Base + Walkable> Base for SwapSolver<T> {
  inherit![ num_vars, when_hi, when_lo, def, tag, get, save, dot ];

  fn new(num_vars:usize)->Self { SwapSolver::new(T::new(num_vars), VID::vir((num_vars-1) as u32)) }

  fn and(&mut self, x:NID, y:NID)->NID { self.key = self.base.and(x,y); self.key }
  fn xor(&mut self, x:NID, y:NID)->NID { self.key = self.base.xor(x,y); self.key }
  fn or(&mut self, x:NID, y:NID)->NID  { self.key = self.base.or(x,y);  self.key }

  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID { // ( wv -> wyz )

    // the basic idea here is to substitute a variable in the scaffold
    // with a simple VHL that spans 2 rows. In theory this could be done
    // with any row, but by moving the variable we want to change to the top,
    // we can make sure the work stays very simple.
    // !! to be fair, it can be expensive to raise the variable, so we should test this
    //    and even come up with some heuristics for when to raise and when not.

    // raise the required vid (v) to the top if it's not there already.
    if self.dst.top_vid() != v {
      if self.dst.vix(v).is_some() { self.dst.lift(v, self.dst.vids.len()-1) }
      else if self.dst.vids.len() == 1 {
        // v isn't there but we just have one node.
        // TODO: give the solver a protocol to say explicitly what the top virtual variable is,
        // and then we shouldn't need this.
        let t = self.dst.top_vid(); self.dst.relabel(t, v); }
      else { // !! v isn't there to replace. maybe just return unchanged?
        panic!("can't sub({}) if {} isn't in the scaffold. rubild scaffold with {} on top.", v,v,v) }}
    assert_eq!(v, self.dst.top_vid(), "can only sub(v,n,ctx) if v is top vid in the scaffold.");

    // in the other solvers, the algorithm composes a simple VHL from 2 input variables
    // at each step, and then substitutes it into the solution VHL. We are doing the same, but
    // the simple VHL lives in the base, and the solution lives in self.dst. So the idea
    // is we will check that the node to substitute has the name nid as the last simple operation:
    assert_eq!(n, self.key, "can only sub(v,n,ctx) if n is result of last and/or/xor call.");
    // ... and then we can safely copy that VHL from the base to self.src:
    let top = self.rebuild_src();
    // because it was a simple substitution, the src now consists of a VHL on 2 variables.
    assert_eq!(self.src.vids.len(), 2, "src scaffold should use exactly 2 vids");

    // now we're going to rewrite the top 1 or 2 rows of self.dst in terms of
    // the two rows of self.src. the top variable in self.dst is the one we are
    // replacing, so it does not appear in self.src. But the two variables in self.src
    // can be any valid variables (though generally ones that appear lower than the old dst.top)
    // let's call the top one z and the bottom one y:
    let (y, z) = (self.src.vids[0], self.src.vids[1]);

    // used by multiple cases below
    macro_rules! s_bit { ($zc:expr,$yc:expr) => {
      self.src.const_path(top, &[(z,$zc), (y,$yc)]) }}

    // these two variables may or may not already have their own rows in self.dst,
    // which gives us four possible patterns. We can collapse these into three patterns:
    let dz:VHL = match (self.dst.vix(y), self.dst.vix(z)) {

      // --------------------------------------------------
      // case A: both z and x are new. this is easy. we just push a new variable
      // onto the dst and add the new VHL at the top. we map the I values in the src truth
      // table to dst.top.hi, and O to dst.top.lo
      (None, None) => {  // case_a(ctx, v, y, z);
        let sz:VHL = self.src.invex(top);
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

      // --------------------------------------------------
      // case B: one of {y,z} is new to self.dst, and one is already present.
      // if z is the one that exists, we need to swap the rows
      (None, Some(_zix)) => { todo!("z exists, y is new: swap y and z!") }
      (Some(yix), None) => {
        let yp = self.dst.vids.len()-2; // desired position for y
        if yix != yp { self.dst.lift(y, yp) }
        // these are all external nodes:
        let v00 = self.dst.path(ctx, &[(v, 0),(y, 0)]);
        let v01 = self.dst.path(ctx, &[(v, 0),(y, 1)]);
        let v10 = self.dst.path(ctx, &[(v, 1),(y, 0)]);
        let v11 = self.dst.path(ctx, &[(v, 1),(y, 1)]);
        let oo = if s_bit!(0,0) { v01 } else { v00 };
        let oi = if s_bit!(0,1) { v01 } else { v00 };
        let io = if s_bit!(1,0) { v11 } else { v10 };
        let ii = if s_bit!(1,1) { v11 } else { v10 };
        let zo = if oo == oi { oo } else { self.dst.add_ref(VHL{ v:y, hi:oi, lo:oo }) };
        let zi = if io == ii { ii } else { self.dst.add_ref(VHL{ v:y, hi:ii, lo:io }) };
        self.dst.relabel(v, z);
        VHL{ v:z, hi:zi, lo:zo } }

      // --------------------------------------------------
      // case C: both variables already exist in the dag
      (Some(mut yix), Some(zix)) => {   // ...yzv -> ...yz

        // ensure that z is directly under v:
        let zp = self.dst.vids.len()-2; // desired position for z
        if zix != zp {
          self.dst.lift(z, zp);
          // this might have changed y's position, so update:
          yix = self.dst.vix(y).expect("what happened to y?!") };

        // ensure that y is directly under z:
        let yp = self.dst.vids.len()-3; // desired position for y
        if yix != yp { self.dst.lift(y, yp) }

        macro_rules! d_nid { ($vc:expr,$zc:expr,$yc:expr) => {
          self.dst.path(top, &[(v,$vc),(z,$zc), (y,$yc)]) }}
        let oo = if s_bit!(0,0) { d_nid!(1,0,0) } else { d_nid!(0,0,0) };
        let oi = if s_bit!(0,1) { d_nid!(1,0,1) } else { d_nid!(0,0,1)};
        let io = if s_bit!(1,0) { d_nid!(1,1,0) } else { d_nid!(0,1,0) };
        let ii = if s_bit!(1,1) { d_nid!(1,1,1) } else { d_nid!(0,1,1)};

        self.dst.drop(v);
        // TODO: proper garbage collection with refcounts. but this works for now:
        self.dst.drop(z);  self.dst.drop(y);
        self.dst.push(y);  self.dst.push(z);

        // now re-add the 0..2 nodes on the y layer:
        let zo = if oo == oi { oo } else { self.dst.add_ref(VHL{ v:y, hi:oi, lo:oo }) };
        let zi = if io == ii { ii } else { self.dst.add_ref(VHL{ v:y, hi:ii, lo:io }) };

        // and the z layer:
        VHL{ v:z, hi:zi, lo:zo } }};

    // todo: dec-ref to old top node.
    self.dst.add_ref(dz) }}



fn max_vid<'r,'s>(a:&'r &VID, b:&'s &VID)-> Ordering {
  if a.is_above(b) {Ordering::Greater }
  else if a==b { Ordering::Equal }
  else { Ordering::Less }}

impl<T:Base+Walkable> SubSolver for SwapSolver<T> {
  fn init_sub(&mut self, top:NID) {
    if top.is_const() { }
    else {
      let v = if top.vid().is_nov() { VID::vir(top.idx() as u32) } else { top.vid() };
      if self.dst.top_vid() == v {}
      else if self.dst.vix(v).is_some() {
        let len = self.dst.vids.len();
        self.dst.lift(v, len-1); }
      else { self.dst.push(v); } }}

  fn next_sub(&mut self, ctx:NID)->Option<(VID, NID)> {
    if ctx.is_const() { None }
    else {
      let mv = if let Some(&t) = self.dst.vids.iter().max_by(max_vid) { t } else { ctx.vid() };
      let res = if mv.is_vir() && ctx.is_var() { // should only be at very start
        self.init_sub(ctx);
        Some((ctx.vid(), ctx)) }
      else if mv.is_vir() {
        if mv != self.dst.top_vid() { self.dst.lift(mv, self.dst.vids.len()-1); }
        // lifting preserves the meaning of the node at that index,
        // but the variable label may have changed.
        let n = NID::from_vid_idx(mv, ctx.idx() as u32);
        Some((mv,n))}
      else { None };
      println!("{:?} next_sub({})->{:?}", self.dst.vids, ctx, res);
      res }}
}

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
  let nw = NID::vir(0); // let w = nw.vid();
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

#[test] fn test_row_refs() {
  let x1 = NID::var(1);
  let x0 = NID::var(0);
  let mut row = VHLRow::new(x1.vid());
  let (f,_) = row.add_ref(HiLo{hi: x0, lo:!x0}, 1);
  let (g,_) = row.add_ref(HiLo{hi: !x0, lo:x0}, 1);
  assert_ne!(f,g,"nids for different funtions should be different!"); }

#[test] fn test_scaffold_refs() {
  let x1 = NID::var(1);
  let x0 = NID::var(0);
  let mut s = VHLScaffold::new(x0.vid());
  s.push(x1.vid());
  let (f,_) = s.add_iref(1, HiLo{hi: x0, lo:!x0}, 1);
  let (g,_) = s.add_iref(1, HiLo{hi: !x0, lo:x0}, 1);
  assert_ne!(f,g,"nids for different funtions should be different!");}

/// test for subbing in two existing variables
#[test] fn test_two_old() {
  let nz = NID::vir(4); let z = nz.vid();
  let ny = NID::vir(3); let y = ny.vid();
  let nx = NID::vir(2); let x = nx.vid();
  let nw = NID::vir(1); let w = nw.vid();
  let nv = NID::vir(0); let v = nv.vid();
  let mut s = BddSwapSolver::new(BDDBase::new(0), z);

  // we start with just z on top:     (z)          0 1
  // substitute z -> y ^ x          = (y ^ x)      0 1 ; 1 0     <->   0110 ; 0110
  let key = s.xor(ny, nx);
  let res = s.sub(z, key, nz);
  assert_eq!(vec![x,y], s.dst.vids);
  assert_eq!(s.dst.exvex(res), VHL { v:y, hi:!nx, lo:nx },
    "(y ^ x) should be (y ? !x : x)");

  // substitute y -> w | v          = ((w|v)^x)
  let key = s.or(nw, nv);
  let res = s.sub(y, key, res);
  assert_eq!(vec![x,v,w], s.dst.vids);
  // todo: make this a standard helper method (VHLScaffold::tt3)
  let VHL{ v:_, hi:i, lo:o } = s.dst.exvex(res);
  let VHL{ v:wo, hi:oi, lo:oo } = s.dst.exvex(o);

  // expr should be: w ? (!x) : (v ? !x : x)
  // so: the lo half of the truth table branches on v
  assert_eq!(wo, v, "w.lo should point to branch on v");
  let VHL{ v:_, hi:ooi, lo:ooo } = s.dst.exvex(oo);
  let VHL{ v:_, hi:oii, lo:oio } = s.dst.exvex(oi);

  // and the right hand side has two copies of !x
  let VHL{ v:wi, hi:_, lo:_ } = s.dst.exvex(i);
  assert_eq!(wi, x, "w.hi should point directly at -.x");
  use nid::{I,O};
  let (ioo, ioi, iio, iii) = (I,O,I,O);
  // s.dst.print();
  assert_eq!((ooo, ooi, oio, oii, ioo, ioi, iio, iii ), (O,I,I,O, I,O,I,O));
  assert_eq!(s.dst.exvex(res), VHL { v:w, hi:!nx, lo:NID::from_vid_idx(v,0) },
    "((w|v) ^ x) should be (w ? !x : (v?!x:x)) ");
  // substitute x -> v & w          = ((w|v)^(w&v))
  let key = s.and(nv, nw);
  let res = s.sub(x, key, res);
  // simplification:                = w ^ v
  assert_eq!(s.dst.exvex(res), VHL { v:w, hi:!nv, lo:nv },
    "((w|v) ^ (w&v)) should be (w ? !v : v)");
  assert!(s.dst.vix(x).is_none(), "x({}) should be gone from dst after substitution", x); }
