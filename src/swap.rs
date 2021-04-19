/// Swap Solver
/// This solver attempts to optimize the BDD concept for substitution solving.
/// It adjusts the input variable ordering by swapping adjacent inputs until the
/// one to be replaced next is at the top of the BDD. The actual replacement work
/// at each step then only involves the top three rows.
use base::GraphViz;
use hashbrown::{HashMap, hash_map::Entry, HashSet};
use {vid::VID, vid::NOV};
use {solve::SubSolver, reg::Reg, nid::{NID,O}, ops::Ops, std::path::Path, base::Base};
use std::fmt;

/// XID: An index-based unique identifier for nodes.
///
/// In a regular NID, the branch variable is embedded directly in the ID for easy
/// comparisons. The working assumption is always that the variable refers to
/// the level of the tree, and that the levels are numbered in ascending order.
///
/// In contrast, the swap solver works by shuffling the levels so that the next
/// substitution happens at the top, where there are only a small number of nodes.
///
/// When two adjacent levels are swapped, nodes on the old top level that refer to
/// the old bottom level are rewritten as nodes on the new top level. But nodes on
/// the old top level that do not refer to the bottom level remain on the old top
/// (new bottom) level. So some of the nodes with the old top branch variable change
/// their variable, and some do not.
///
/// NIDs are designed to optimize cases where comparing branch variables are important
/// and so the variable is encoded directly in the reference to avoid frequent lookups.
/// For the swap solver, however, this encoding would force us to rewrite the nids in
/// every layer above each swap, and references held outside the base would quickly
/// fall out of sync.
///
/// So instead, XIDs are simple indices into an array (XID=index ID). If we want to
/// know the branch variable for a XID, we simply look it up by index in a central
/// vector.
///
/// We could use pointers instead of array indices, but I want this to be a representation
/// that can persist on disk, so a simple flat index into an array of XVHLs is fine for me.

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
struct XID { x: i64 }
impl fmt::Debug for XID {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if *self == XID_O { write!(f, "XO")}
    else if *self == XID_I { write!(f, "XI")}
    else { write!(f, "{}#{}", if self.is_inv() { "!" } else {""}, self.raw().x)}}}
const XID_O:XID = XID { x: 0 };
const XID_I:XID = XID { x: !0 };
impl XID {
  fn ix(&self)->usize { self.x as usize }
  fn raw(&self)->XID { if self.x >= 0 { *self } else { !*self }}
  fn is_inv(&self)->bool { self.x<0 }
  fn is_const(&self)->bool { *self == XID_O || *self == XID_I }
  fn from_nid(x:NID)->Self {
    if x.is_lit() { panic!("don't know how to convert lit nid -> xid")}
    if x.vid()!=NOV { panic!("don't know how to convert nid.var(v!=NOV) -> xid")}
    if x.is_inv() { !XID{ x: x.idx() as i64 }} else { XID{ x: x.idx() as i64 } }}
  fn to_nid(&self)->NID {
    if self.is_inv() { !NID::from_vid_idx(NOV, !self.x as u32)}
    else { NID::from_vid_idx(NOV, self.x as u32) }}
  fn to_bool(&self)->bool {
    if self.is_const() { *self == XID_I }
    else { panic!("attempted to convert non-constant XID->bool") }}
  fn inv(&self) -> XID { XID { x: !self.x } }}
impl std::ops::Not for XID { type Output = XID; fn not(self)->XID { self.inv() }}

/// Like Hilo, but uses XIDs instead of NIDs
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
struct XHiLo { pub hi: XID, pub lo: XID }
impl std::ops::Not for XHiLo { type Output = XHiLo; fn not(self)->XHiLo { XHiLo { hi:!self.hi, lo:!self.lo }}}
impl XHiLo { fn as_tup(&self)->(XID,XID) { (self.hi, self.lo) }}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
struct XVHL { pub v: VID, pub hi: XID, pub lo: XID }
impl XVHL {
  fn hilo(&self)->XHiLo { XHiLo { hi:self.hi, lo:self.lo } }
  fn is_var(&self)->bool { self.v.is_var() && self.hi == XID_I && self.lo == XID_O }}
impl std::ops::Not for XVHL { type Output = XVHL; fn not(self)->XVHL { XVHL { v:self.v, hi:!self.hi, lo:!self.lo }}}

/// Dummy value to stick into vhls[0]
const XVHL_O:XVHL = XVHL{ v: NOV, hi:XID_O, lo:XID_O };

/// index + refcount
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct IxRc { ix:XID, rc: usize }

/**
We need to map:

  ix -> XVHL   (so we can look up info about the node)
  XVHL -> ix   (so we can avoid creating duplicates)
  v -> [ix]    (so we can quickly locate all nodes associated with a variable, and change them)

  these last two can and should be combined into v -> {XHiLo -> IxRc0}
  because we want to frequently swap out whole rows of variables.
  we'll call this XVHLRow
*/
struct XVHLRow { hm: HashMap<XHiLo, IxRc> }
impl XVHLRow { fn new()->Self {XVHLRow{ hm: HashMap::new() }}}

/// The scaffold itself contains the master list of records (vhls) and the per-row index
pub struct XVHLScaffold {
  vids: Vec<VID>,
  vhls: Vec<XVHL>,
  rows: HashMap<VID, XVHLRow> }

impl XVHLScaffold {
  fn new()->Self { XVHLScaffold{ vids:vec![], vhls:vec![XVHL_O], rows: HashMap::new() } }

  /// validate that this scaffold is well formed. (this is for debugging)
  pub fn validate(&self, msg: &str) {

    println!("@validate: {}", msg);
    println!("${:?}", self.vids);
    println!("%{:?}", self.rows.keys().collect::<Vec<&VID>>());
    for (i, &x) in self.vhls.iter().enumerate() {
      let rc = if x.v == NOV { 0 }
      else {
        let ixrc = self.rows[&x.v].hm.get(&x.hilo()).unwrap();
        assert_eq!(ixrc.ix.x, i as i64);
        ixrc.rc};
      println!("^{:03}: {} {:?} {:?}   (rc:{})", i, x.v, x.hi, x.lo, rc)}

    // vids must be unique:
    let mut vids:HashMap<VID, i64> = self.vids.iter().cloned().enumerate().map(|(i,v)|(v,i as i64)).collect();
    assert_eq!(vids.len(), self.vids.len(), "duplicate vid(s) in list: {:?}", self.vids);
    assert_eq!(vids.len(), self.rows.len(), "vids and rows should have the same len()");
    vids.insert(NOV, -1);

    println!("vids:{:?}", vids);

    let mut rc: HashMap<XID, usize> = HashMap::new();
    let mut seen : HashMap<XVHL,usize> = HashMap::new();
    // validate the rows:
    for (i, &x) in self.vhls.iter().enumerate() {
      // the vid should be in the scaffold, or cleared out to indicate a blank row.
      assert!(vids.contains_key(&x.v), "invalid v for vhls[{}]: {}", i, x.v);
      // the lo branch should not be inverted.
      assert!(!x.lo.is_inv(), "found inverted lo branch in vhls[{}]: {:?}", i, x);

      // with the exception of garbage / O :
      if x.v != NOV {
        // the lo branch should be different from the hi branch
        assert_ne!(x.lo, x.hi, "unmerged branches in vhl[{}]: {:?}", i, x);

        let hi = self.get(x.hi.raw()).expect("hi branch points nowhere");
        let lo = self.get(x.lo.raw()).expect("lo branch points nowhere");

        if hi.v == NOV && x.hi.raw() != XID_O { panic!("hi branch to garbage-collected node")}
        if lo.v == NOV && x.lo.raw() != XID_O { panic!("lo branch to garbage-collected node")}

        // the hi and lo branches should point "downward"
        assert!(vids[&lo.v] < vids[&x.v], "upward lo branch @vhl[{}]: {:?}", i, x);
        assert!(vids[&hi.v] < vids[&x.v], "upward hi branch @vhl[{}]: {:?}", i, x);

        // there should be no duplicate entries.
        if let Some(j) = seen.get(&x) { panic!("vhl[{}] is a duplicate of vhl[{}]: {:?}", i, j, x) }
        else { seen.insert(x, i); }

        // there should be a hashmap entry pointing back to the item:
        if let Some(ixrc) = self.rows[&x.v].hm.get(&XHiLo{ hi:x.hi, lo:x.lo }) {
          let ix = ixrc.ix.raw().x as usize;
          assert_eq!(ix, i, "hashmap stored wrong index ({:?}) for vhl[{}]: {:?} ", ixrc.ix, i, x)}
        else { panic!("no hashmap reference to vhl[{}]: {:?}", i, x) }

        // update ref counts
        *rc.entry(x.hi.raw()).or_insert(0)+=1;
        *rc.entry(x.lo.raw()).or_insert(0)+=1;}}

      // check internal refcounts vs the ones we just calculated
      for (_v, row) in self.rows.iter() {
        for (_hl, ixrc) in row.hm.iter() {
          // println!("testing refcount {:?} for v:{:?} hl:{:?}", ixrc, v, hl);
          let expect = *rc.get(&ixrc.ix).unwrap_or(&0);
          assert!(ixrc.rc >= expect, "refcount was too low for xid: {:?} (expected {}, got {}",
            ixrc.ix, expect, ixrc.rc);}}

      println!("@/validate")}


  /// return the index (height) of the given variable within the scaffold (if it exists)
  fn vix(&self, v:VID)->Option<usize> { self.vids.iter().position(|&x| x == v) }

  /// return the vid immediately above v in the scaffold, or None
  /// if v is top vid. Panics if v is not in the scaffold.
  fn vid_above(&self, v:VID)->Option<VID> {
    if let Some(x) = self.vix(v) { self.vids.get(x+1).cloned() }
    else { panic!("vid_above(v:{}): v not in the scaffold.", v) }}

  // fn vid_below(&self, v:VID)->Option<VID> {
  //   if let Some(x) = self.vix(v) { if x>0 { self.vids.get(x-1).cloned()} else { None }}
  //   else { panic!("vid_below(v:{}): v not in the scaffold.", v) }}

  /// add a new vid to the top of the stack. return its position.
  fn push(&mut self, v:VID)->usize {
    if self.vix(v).is_some() { panic!("pushed variable that was already in the scaffold: {:?}", v) }
    let ix = self.vids.len();
    self.vids.push(v);
    self.rows.insert(v, XVHLRow::new());
    ix }

  // /// drop top var v (double check that it's actually on top)
  // fn drop(&mut self, v:VID) {
  //   if *self.vids.last().expect("can't drop from empty scaffold") == v {
  //     self.vids.pop();
  //     self.rows.remove(&v); }
  //   else { panic!("can't pop {} because it's not on top ({:?})", v, self.vids) }}

  /// add a reference to the given XVHL, inserting it into the row if necessary.
  /// returns the xid representing this xvhl triple.
  fn add_ref(&mut self, hl0:XVHL, rc:usize)->XID {
    let inv = hl0.lo.is_inv();
    let vhl = if inv { !hl0 } else { hl0 };
    if vhl == XVHL_O { return if inv { XID_I } else { XID_O }}
    debug_assert_ne!(vhl.hi, vhl.lo, "hi and lo should be different"); // to trigger traceback
    // allocate a xid just in case. if this isn't used, it'll just be used next time.
    let (alloc, alloc_new) = self.alloc_one();
    let row = self.rows.entry(vhl.v).or_insert_with(|| XVHLRow::new());
    let hl = vhl.hilo();
    let (res, is_new) = match row.hm.entry(hl) {
      Entry::Occupied (mut e) => {
        let xid = e.get().ix;
        e.get_mut().rc += rc;
        (xid, false) }
      Entry::Vacant(e) => {
        e.insert(IxRc{ ix:alloc, rc });
        if alloc_new { self.vhls.push(vhl) } else { self.vhls[alloc.x as usize] = vhl };
        (alloc, true) }};
    if is_new {
      let hi = self.get(vhl.hi).unwrap(); self.add_ref(hi,1);
      let lo = self.get(vhl.lo).unwrap(); self.add_ref(lo,1); }
    if inv { !res } else { res }}

  /// decrement refcount for ix. return new refcount.
  fn dec_ref_ix(&mut self, ix:XID)->usize { self.add_ref_ix(ix, -1) }

  fn add_ref_ix(&mut self, ix:XID, drc:i64)->usize {
    if ix.is_const() { return 1 }
    let vhl = self.vhls[ix.raw().x as usize];
    if let Some(row) = self.rows.get_mut(&vhl.v) {
      if let Some(mut ixrc) = row.hm.get_mut(&vhl.hilo()) {
        if drc < 0 && (drc + ixrc.rc as i64 ) < 0 { panic!("this would result in negative refcount")}
        else { ixrc.rc = (ixrc.rc as i64 + drc) as usize;  ixrc.rc }}
      else { println!("add_ref_ix warning: entry not found for {:?}", vhl); 0}}
    else { println!("add_ref_ix warning: row not found for {:?}", vhl.v); 0 }}

  /// fetch the XVHL for the given xid (if we know it)
  fn get(&self, x:XID)->Option<XVHL> {
    self.vhls.get(x.raw().ix()).map(|&y| if x.is_inv() { !y } else { y }) }

  /// follow the hi or lo branch of x
  fn follow(&self, x:XID, which:bool)->XID {
    let vhl = self.get(x).unwrap();
    if which { vhl.hi } else { vhl.lo }}

  fn branch_var(&self, x:XID)->VID { self.get(x).unwrap().v }

  /// produce the fully expanded "truth table" for a bdd
  /// down to the given row, by building rows of the corresponding
  /// binary tree. xids in the result will either be constants,
  /// branch on the limit var, or branch on some variable below it.
  fn tbl(&mut self, top:XID, limit:Option<VID>)->Vec<XID> {
    let mut xs = vec![top];
    println!("tbl/xs: {:?}", xs);
    for (i,&x) in xs.iter().enumerate() { println!("  [{}]: x:{} = {:?}", i, x.x, self.get(x).unwrap())}
    let z = if let Some(lim) = limit {
      self.vix(lim).expect("limit var isn't in scaffold") as i64}
      else {-1};
    let mut v = self.get(top).expect("top wasn't in the scaffold").v;
    let mut i = self.vix(v).unwrap() as i64;
    // i is index of top var (from XID), z of limit var, so i should be above z.
    // if i < z it just means top is lower than limit, so return vec![top]
    while i > z {                     // copy-and-expand for each row down to limit
      v = self.vids[i as usize];
      let tmp = xs; xs = vec![];
      for x in tmp {
        let vhl = self.get(x).unwrap();
        if vhl.v == v { xs.push(vhl.lo); xs.push(vhl.hi); }
        else { xs.push(x); xs.push(x); }}
      println!("tbl/xs v:{:?}", v);
      for (i,&x) in xs.iter().enumerate() { println!("  [{}]: x:{} = {:?}", i, x.x, self.get(x).unwrap())}
      i-=1}
    for &x in xs.iter() { self.add_ref_ix(x, 1); } // increment ref counts
    xs}

  /// Given a truth table, construct the corresponding bdd
  /// Starts at the lowest row variable unless base is given.
  fn untbl(&mut self, mut xs: Vec<XID>, base:Option<VID>)->XID {
    let mut v = base.unwrap_or(self.vids[0]);
    assert!(xs.len().is_power_of_two(), "untbl: xs len must be 2^x. len: {} {:?}", xs.len(), xs);
    loop {
      xs = xs.chunks(2).map(|lh:&[XID]| {
        let (lo, hi) = (lh[0], lh[1]);
        if lo == hi { self.dec_ref_ix(hi); lo } // 2 refs -> 1
        else {
          self.dec_ref_ix(hi); self.dec_ref_ix(lo);
          self.add_ref(XVHL{ v, hi, lo }, 1)} }).collect();
      println!("untbl/xs: {:?}", xs);
      if xs.len() == 1 { break }
      v = self.vid_above(v).expect("not enough vars in scaffold to untbl!"); }
    xs[0]}

  /// allocate a single xid. returs (xid, isnew)
  // TODO: cache the empty slots so this doesn't take O(n) time.
  fn alloc_one(&self)->(XID, bool) {
    for (j,vhl) in self.vhls.iter().enumerate().skip(1) {
      if vhl.v == NOV { return (XID{x:j as i64 }, false)}}
    (XID{x:self.vhls.len() as i64}, true)}

  /// allocate free xids
  fn alloc(&mut self, count:usize)->Vec<XID> {
    let mut i = count; let mut res = vec![];
    // reclaim garbage collected xids.
    for (j,vhl) in self.vhls.iter().enumerate().skip(1) {
      if vhl.v == NOV {
        res.push(XID{x:j as i64});
        i-= 1;  if i == 0 { break; }}}
    // create new xids if there weren't enough reclaimed
    while i > 0 {
      let x = self.vhls.len() as i64;
      self.vhls.push(XVHL_O);
      res.push(XID{x});
      i-=1 }
    res }

  /// swap v up by one level
  fn swap(&mut self, v:VID) {
    #[cfg(test)] { self.validate(&format!("swap({}) in {:?}.", v, self.vids)); println!("ok! begin swap.") }
    let vi = self.vix(v).expect("requested vid was not in the scaffold.");
    if vi+1 == self.vids.len() { println!("warning: attempt to lift top vid {}", v); return }
    let w = self.vids[vi+1]; // start: v is 1 level below w
    self.vids.swap(vi+1, vi);

    //  row wi:  w ____                v        v ____
    //            :     \                        :     \
    //  row vi:  v __    v __      =>  w   =>   w __    w __
    //           :   \    :  \                  :   \    :   \
    //           oo   oi  io  ii                oo   io  oi   ii
    // we are lifting row v up 1. row v nodes cannot possibly refer to variable w,
    //  so we will not remove anything from this row.
    //  But we will add a new entry whenever nodes in row w refer to v
    println!("vids: {:?}", self.vids);
    println!("row keys: {:?}", self.rows.keys().collect::<Vec<&VID>>());
    let rv = self.rows.remove(&v).unwrap_or_else(|| panic!("row {:?} not found",v));

    // row w may contain nodes that refer to v, which now need to be moved to row v.
    let rw = self.rows.remove(&w).unwrap();
    let mut worker = SwapWorker::new(rv,rw);
    worker.find_movers0();
    worker.find_movers1();

    // If we are deleting from v and adding to w, we can re-use the xids.
    // otherwise, allocate some new xids.
    let xids = {
      let (vdel, mut xids, needed) = worker.recycle();
      if needed > 0 { xids.extend(self.alloc(needed)) };
      self.reclaim_nodes(vdel);
      xids };

    // [commit wnew]
    // we now have a xid for each newly constructed (XWIP) child node on row w,
    // so go ahead and add them. we will also map the temp ix to the actual ix.
    let mut wipxid = vec![XID_O; worker.wnix as usize];
    debug_assert_eq!(worker.wnix as usize, worker.wnew.len(), "wnew.len != wnix");
    for ((hi,lo), ixrc0) in worker.wnew.iter() {
      let mut ixrc = *ixrc0; // clone so we maintain the refcount
      debug_assert!(ixrc.rc > 0);
      let inv = ixrc0.ix.x < 0; assert!(!inv);
      let wipix = ixrc0.ix.x as usize;
      ixrc.ix = xids[wipix];  // map the temp xid -> true xid
      wipxid[wipix] = ixrc.ix; // remember for w2x, below.
      assert!(worker.rw.hm.get(&(XHiLo{hi:*hi, lo:*lo})).is_none());
      worker.rw.hm.insert(XHiLo{hi:*hi, lo:*lo}, ixrc);
      // and now update the master store:
      debug_assert_ne!(hi, lo, "hi=lo when committing wnew");
      self.vhls[ixrc.ix.x as usize] = XVHL{ v:w, hi:*hi, lo:*lo }; }

    // [commit wtov]
    // with those nodes created, we can finish moving the nodes from w to v.
    let w2x = |wip:&XWIP1| {
      match wip {
        XWIP1::XID(x) => *x,
        XWIP1::NEW(x) => { if *x<0 { !wipxid[!*x as usize]  } else { wipxid[*x as usize ]}}}};
    for (ixrc, wip_hi, wip_lo) in worker.wtov.iter() {
      let (hi, lo) = (w2x(wip_hi), w2x(wip_lo));
      debug_assert_ne!(hi, lo, "hi=lo when committing wtov");
      worker.rv.hm.insert(XHiLo{hi, lo}, *ixrc);
      self.vhls[ixrc.ix.x as usize] = XVHL{ v, hi, lo }; }

    // [ commit edec/eref changes ]
    if !(worker.edec.is_empty() && worker.eref.is_empty()) {
      let mut sum:HashMap<XID, i64> = HashMap::new();
      for &xid in worker.eref.iter() { *sum.entry(xid).or_insert(0) += 1; }
      for &xid in worker.edec.iter() { *sum.entry(xid).or_insert(0) -= 1; }
      for (xid, drc) in sum.iter() { self.add_ref_ix(*xid, *drc); }}
    // it should be usize rather than i64 because nothing outside of these two rows
    // will ever have its refcount drop all the way to 0.
    // (each decref is something like (w?(v?a:b):(v?a:c))->(v?a:w?b:c) so we're just
    // merging two references into one, never completely deleting one).

    // finally, put the rows back where we found them:
    self.rows.insert(v, worker.rv);
    self.rows.insert(w, worker.rw);
    #[cfg(test)] { self.validate("after swap."); println!("valid!") }}

  /// Reclaim the records for a list of garbage collected nodes.
  // TODO: add to some kind of linked list so they're easier to find.
  fn reclaim_nodes(&mut self, xids:Vec<XID>) { for xid in xids { self.vhls[xid.raw().ix()] = XVHL_O }}

  /// arrange row order to match the given groups.
  /// the groups are given in bottom-up order, and should
  /// completely partition the scaffold vids.
  // TODO: executes these swaps in parallel
  fn regroup(&mut self, groups:Vec<HashSet<VID>>) {
    // TODO: check for complete partition
    let mut lc = 0; // left cursor
    let mut rc;     // right cursor
    let mut ni = 0; // number of items in groups we've seen
    for g in groups {
      ni += g.len();
      while lc < ni {
        // if we're looking at something in right place, skip it
        while lc < ni && g.contains(&self.vids[lc as usize]) { lc+=1 }
        if lc < ni {
          // scan ahead for next group member
          rc = lc+1;
          while !g.contains(&self.vids[rc]) { rc+=1 }
          // now drag the misplaced row down
          while rc > lc { rc -= 1; self.swap(self.vids[rc]) }}}}}}


// -- graphviz ----------------------------------------------------------

impl GraphViz for XVHLScaffold {
  fn write_dot(&self, _:NID, wr: &mut dyn std::fmt::Write) {
    // TODO: show only the given nid, instead of the whole scaffold
    // assert_eq!(o, NID::o(), "can't visualize individual nids yet. pass O for now");
    macro_rules! w { ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap(); }}
    w!("digraph XVHL {{");
    w!("subgraph head {{ h1[shape=plaintext; label=\"XVHL\"] }}");
    w!("  {{rank=same XO XI}}");
    w!("  XO[label=⊥; shape=square];");
    w!("  XI[label=⊤; shape=square];");
    w!("node[shape=circle];");
    for ev in self.vids.iter().rev() {
      let row = &self.rows[ev];
      if !row.hm.is_empty() {
        write!(wr, "{{rank=same").unwrap();
        for ixrc in row.hm.values() { write!(wr, " \"{:?}\"", ixrc.ix);}
        w!("}}") }
      for (hl,ixrc) in row.hm.iter() {
        let x = ixrc.ix;
        w!("  \"{:?}\"[label=\"{}\"];", x, ev);  // draw the node itself
        let arrow = |n:XID| if n.is_const() || !n.is_inv() { "normal" } else { "odot" };
        let sink = |n:XID| if n.is_const() { n } else { n.raw() };
        w!("edge[style=solid, arrowhead={}];", arrow(hl.hi));
        w!("  \"{:?}\"->\"{:?}\";", x, sink(hl.hi));
        w!("edge[style=dashed, arrowhead={}];", arrow(hl.lo));
        w!("  \"{:?}\"->\"{:?}\";", x, sink(hl.lo)); }}
    w!("}}"); }}


// ---- swap worker -----------------------------------------------------

// helpers to track which new nodes are to be created.
// i am doing this because i don't want to refer to self -- partially to appease the
// borrow checker immediately, but also because in the future i'd like this to be done
// in a distributed process, which will modify the two rows in place and then send the
// refcount and branch variable changes to a central repo.

/// in the first WIP step, we either work with existing xids
/// and hilo pairs that may or may not already exist.
#[derive(Debug, Clone, Copy)]
enum XWIP0 { XID(XID), HL(XID,XID) }

/// in the second wip step, the hilo pairs are all resolved to existing
/// xids or mapped to a new one
#[derive(Debug, Clone, Copy)]
enum XWIP1 { XID(XID), NEW(i64) }


struct SwapWorker {
  // the rows to swap:
  rv:XVHLRow, rw:XVHLRow,

  /// external nodes whose refcounts need to be decremented after the swap.
  edec: Vec<XID>,
  /// external nodes to incref
  eref: Vec<XID>,

  /// work in progress for nodes moving from row w to row v.
  wmov0: Vec<(XHiLo,XWIP0,XWIP0)>,
  /// wip for new children on row v.
  wtov: Vec<(IxRc,XWIP1,XWIP1)>,

  /// new parent nodes to create on row w
  wnew: HashMap<(XID,XID), IxRc>,

  /// next index for new node
  wnix: i64
}
impl SwapWorker {
  fn new(rv:XVHLRow, rw:XVHLRow )->Self {
    SwapWorker{ rv, rw, edec:vec![], eref:vec![],
      wmov0:vec![], wtov:vec![], wnew:HashMap::new(), wnix:0 } }

  /// collect the list of nodes on row w that reference row v, and thus have to be moved
  /// to row v. also decrement those refcounts as we find them.
  fn find_movers0(&mut self) {
      let mov_edec = wtov(&mut self.rw, &mut self.rv);
      self.wmov0 = mov_edec.0; self.edec = mov_edec.1; }

  /// Construct new child nodes on the w level, or add new references to external nodes.
  /// Converts the XWIP0::HL entries to XWIP1::NEW. clears out .wmov0,
  /// and populates .wtov, .wnew, and .eref
  fn find_movers1(&mut self) {
    for (whl, wip_hi, wip_lo) in std::mem::replace(&mut self.wmov0, vec![]) {
      let (hi, lo) = {
        let mut resolve = |xw0:XWIP0|->XWIP1 {
          match xw0 {
            // the child() function would have marked it as a XID if it were already in row w.
            XWIP0::XID(x) => { self.eref.push(x); XWIP1::XID(x) },
            XWIP0::HL(hi0,lo0) => {
              // these are the new children on the w level, so we are creating a new node.
              // but: it's possible that multiple new nodes point to the same place.
              // this pass ensures that all duplicates resolve to the same place.
              // TODO: this isn't really an IxRc since the xid is virtual
              let (hi,lo,inv) = if lo0.is_inv() { (!hi0, !lo0, true) } else { (hi0,lo0,false) };
              let x = match self.wnew.entry((hi, lo)) {
                Entry::Occupied(mut e) => {
                  e.get_mut().rc += 1;
                  e.get().ix.x }
                Entry::Vacant(e) => {
                  let x = self.wnix as i64; self.wnix += 1;
                  self.eref.push(hi); self.eref.push(lo);
                  e.insert(IxRc{ ix:XID{x}, rc:1 });
                  x }};
              XWIP1::NEW(if inv { !x } else { x }) }}};
        (resolve(wip_hi), resolve(wip_lo))};

      // the lo branch should never be inverted, since the lo-lo path doesn't change in a swap,
      // and lo branches are always raw in the scaffold.
      // This means we only have to deal with inverted xids the newly-created hi branches.
      if let XWIP1::NEW(x) = lo { assert!(x >= 0, "unexpected !lo branch");}
      // delete the old node from row w. the newly created nodes don't depend on v, and
      // the node to delete does depend on v, so there's never a conflict here.
      let ixrc = self.rw.hm.remove(&whl).expect("I saw a whl that wasn't there!");
      // we can't add directly to row v until we resolve the XWIP1::NEW entries,
      // but we can make a list of the work to be done:
      self.wtov.push((ixrc, hi, lo)); }}

  /// remove garbage from row v. these won't conflict with .wtov because we will never
  /// add a *completely* new node on row v - only move existing nodes from w, and
  /// these will never match existing nodes on v because at least one leg always
  /// points at w (and this wasn't possible before the lift). But we may need to delete
  /// nodes because the rc dropped to 0 (when the node was only referenced by row w).
  fn recycle(&mut self)->(Vec<XID>, Vec<XID>, usize) {

    // vdel contains xids that the scaffold should delete.
    let mut vdel: Vec<XID> = vec![];
    self.rv.hm.retain(|_, ixrc| {
      if ixrc.rc == 0 { vdel.push(ixrc.ix); false }
      else { true }});

    let mut needed = 0; // in case there are more new nodes than old trash

    // vmod reclaims xids from vdel that can be recycled
    let vmod: Vec<XID> = {
      let have = vdel.len();
      let need = self.wnew.len(); assert_eq!(need, self.wnix as usize);
      if need <= have {
        let tmp = vdel.split_off(need);
        let res = vdel; vdel = tmp;
        res }
      else {
        let mut res = vdel; vdel = vec![];
        needed = need-have;
        res }};
    (vdel,vmod,needed)}

}



/// given the rows from swap(), find all the nodes from row w that need
/// to move to row v. (that is, rows that have a reference to row v).
/// rv is mutable here because we will decrease the refcount as we find
/// each reference, and rw is mutable because we may *increase* the refcount.
fn wtov(rw:&mut XVHLRow, rv:&mut XVHLRow)->(Vec<(XHiLo, XWIP0, XWIP0)>, Vec<XID>) {
  // build a map of xid->hilo for row v, so we know every xid that branches on v,
  // and can quickly retrieve its high and lo branches.
  let mut vx:HashMap<XID,(XID,XID)> = HashMap::new();
  for (vhl, ixrc) in rv.hm.iter() { vx.insert(ixrc.ix, vhl.as_tup()); }
  // moving a node from w->v modifies the old node in place, so no new xid is used.
  // (we know it's not already on row v because before the lift, row v could not refer to w)
  // at least one of the node's children will be replaced by a new node on row w. proof:
  //     general pattern of 3rd level rewrite is   abcd ->  acbd
  //     we can consolidate one side of the swap: abac -> aabc (so we get v?a:w?b:c)
  //     but we can't consolidate both:  abab -> abba, because abab can't appear in a bdd.
  //     no other pattern would result in consolidating both sides. (qed)
  // therefore: worst case for growth is every node in w moves to v and creates 2 new children.
  // a block of 2*w.len ids for this algorithm to assign.
  // so: in the future, if we want to create new nodes with unique ids in a distributed way,
  // we should allocate 2*w.len ids for this function to assign to the new nodes.
  // reference counts elsewhere in the graph can change (!!! really? they don't change in this step.),
  // but never drop to 0. if they did, then swapping the rows back would have to create new nodes elsewhere.

  let mut edec:Vec<XID> = vec![];
  let mut wmov0: Vec<(XHiLo,XWIP0,XWIP0)> = vec![];
  let mut wref:Vec<XHiLo> = vec![];

  // vv here indicates that both sides referenced v originally, so there is a chance for refcount changes.
  let mut child = |h:XID, l:XID,vv:bool|->XWIP0 { // reference a node on/below row w, or create a node on row w
    let (hi, lo, inv) = if l.is_inv() {(!h, !l, true)} else {(h, l, false)};
    // hi == lo only when the match passes hi,hi or lo,lo. So: this is always a single external reference
    // that we've passed twice, and we're not really dropping a reference here.
    // No refcount changes happen outside rows v and w. (!!! at least in this step?)
    if hi == lo { if vv { edec.push(lo); } XWIP0::XID(if inv { !lo } else { lo }) }
    else if let Some(ixrc) = rw.hm.get(&XHiLo{ hi, lo}) {
      wref.push(XHiLo{hi,lo}); // rw can't be mutable here so remember to modify it later
      XWIP0::XID(if inv {!ixrc.ix} else {ixrc.ix}) }
    else if inv { XWIP0::HL(!hi, !lo) } else { XWIP0::HL(hi, lo) }};

  let mut vdec = |xid:XID| {
    let (hi, lo)=vx.get(&xid.raw()).unwrap();
    let ixrc = rv.hm.get_mut(&XHiLo{hi:*hi, lo:*lo}).unwrap();
    if ixrc.rc == 0 { println!("warning: rc was already 0"); }
    else { ixrc.rc -= 1; }};

  // Partition nodes on rw into two groups:
  // group I (independent):
  //   These are nodes that do not reference row v.
  //   These remain on rw, unchanged.
  // group D (dependent):
  //   These are nodes with at least one child on row v.
  //   These must be rewritten in place to branch on v (and moved to rv).
  //   "In place" means that their XIDs must be preserved.
  //   The moved nodes will have children on row w:
  //      These may be new nodes, or may already exist in group I.
  //   The old children (on row v) may see their refcounts drop to 0.

  let mut new_v = |whl,ii,io,oi,oo,vv| { wmov0.push((whl, child(ii,oi,vv), child(io,oo,vv))) };

  for whl in rw.hm.keys() {
    let (hi, lo) = whl.as_tup();
    let vget = |xid:XID|->Option<(XID,XID)> {
      if xid.is_inv() { vx.get(&xid.raw()).cloned().map(|(h,l)| (!h,!l)) } else { vx.get(&xid).cloned() }};
    match (vget(hi), vget(lo)) {
      (None,          None         ) => {},  // independent of row v, so nothing to do.
      (None,          Some((oi,oo))) => { new_v(*whl, hi, hi, oi, oo, false); vdec(lo) },
      (Some((ii,io)), None         ) => { new_v(*whl, ii, io, lo, lo, false); vdec(hi) },
      (Some((ii,io)), Some((oi,oo))) => { new_v(*whl, ii, io, oi, oo, true); vdec(hi); vdec(lo) }}}

  for hl in wref.iter() { rw.hm.get_mut(hl).unwrap().rc += 1 }
  (wmov0, edec) }

// -- debugger ------------------------------------------------------------

/// A simple RPN debugger to make testing easier.
struct XSDebug {
  /** scaffold */   xs: XVHLScaffold,
  /** vid->char */  vc: HashMap<VID,char>,  // used in fmt for branch vars
  /** char->vid */  cv: HashMap<char,VID>,  // used in run to map iden->vid
  /** data stack */ ds: Vec<XID>}

impl XSDebug {
  pub fn new(vars:&str)->Self {
    let mut this = XSDebug {
      xs: XVHLScaffold::new(), ds: vec![],
      vc:HashMap::new(), cv: HashMap::new() };
    for (i, c) in vars.chars().enumerate() { this.var(i, c) }
    this }
  fn var(&mut self, i:usize, c:char) {
    let v = VID::var(i as u32); self.xs.push(v); self.xs.add_ref(XVHL{v, hi:XID_I, lo:XID_O}, 1);
    self.name_var(v, c); }
  fn vids(&self)->String { self.xs.vids.iter().map(|v| *self.vc.get(v).unwrap()).collect() }
  fn name_var(&mut self, v:VID, c:char) { self.vc.insert(v, c); self.cv.insert(c, v); }
  fn pop(&mut self)->XID { self.ds.pop().expect("stack underflow") }
  fn xid(&mut self, s:&str)->XID { self.run(s); self.pop() }
  fn vid(&self, c:char)->VID { *self.cv.get(&c).unwrap() }
  fn run(&mut self, s:&str)->String {
    for c in s.chars() {
      match c {
        'a'..='z' =>
          if let Some(&v) = self.cv.get(&c) { self.ds.push(self.xs.add_ref(XVHL{v,hi:XID_I,lo:XID_O},1)) }
          else { panic!("unknown variable: {}", c)},
        '0' => self.ds.push(XID_O),
        '1' => self.ds.push(XID_I),
        '.' => { self.ds.pop(); },
        '!' => { let x= self.pop(); self.ds.push(!x) },
        ' ' => {}, // no-op
        '#' => { // untbl
          let v = if self.ds.len()&1 == 0 { None } else {
            let x = self.pop();
            let vhl = self.xs.get(x).unwrap();
            if !vhl.is_var() { panic!("last item in odd-len stack was not var for #") }
            Some(vhl.v)};
          let x = self.xs.untbl(self.ds.clone(), v); // TODO: how can I just move ds here?
          self.ds = vec![x]; },
        '?' => { let vx=self.pop(); let hi = self.pop(); let lo = self.pop(); self.ite(vx,hi,lo); },
        _ => panic!("unrecognized character: {}", c)}}
    if let Some(&x) = self.ds.last() { self.fmt(x) } else { "".to_string() }}
  fn ite(&mut self, vx:XID, hi:XID, lo:XID)->XID {
    if let Some(xvhl) = self.xs.get(vx) {
      if !xvhl.is_var() { panic!("not a branch var: {} ({:?})", self.fmt(vx), xvhl) }
      assert_ne!(hi, lo, "hi and lo branches must be different");
      let res = self.xs.add_ref(XVHL{v:xvhl.v, hi, lo}, 1); self.ds.push(res); res }
    else { panic!("limit not found for '#': {:?}", vx) }}
  fn fmt(&self, x:XID)->String {
    match x {
      XID_O => "0".to_string(),
      XID_I => "1".to_string(),
      _ => { let inv = x.is_inv(); let x = x.raw(); let sign = if inv { "!" } else { "" };
        let xv = self.xs.get(x).unwrap();
        let vc:char = *self.vc.get(&xv.v).unwrap();
        if xv.is_var() { format!("{}{}", vc, sign).to_string() }
        else { format!("{}{}{}?{} ", self.fmt(xv.lo), self.fmt(xv.hi), vc, sign) } } }}}

// ------------------------------------------------------

pub struct SwapSolver {
  /** the result (destination) bdd  */  dst: XVHLScaffold,
  /** top node in the destination   */  dx: XID,
  /** the variable we're replacing  */  rv: VID,
  /** the replacement (source) bdd  */  src: XVHLScaffold,
  /** top node in the source bdd    */  sx: XID }

impl SwapSolver {
  /// constructor
  // TODO: remove the nvars parameter to new()?
  pub fn new(_nvars: usize) -> Self {
    let dst = XVHLScaffold::new();
    let src = XVHLScaffold::new();
    SwapSolver{ dst, dx:XID_O, rv:NOV, src, sx: XID_O }}

  /// Arrange the two scaffolds so that their variable orders match.
  ///  1. vids shared between src and dst (set n) are above rv
  ///  2. vids that are only in the dst (set d) are below rv
  ///  3. new vids from src (set s) are directly above rv.
  /// so from bottom to top: ( d, v, s, n )
  /// (the d vars are not actually copied to the src, but otherwise the
  /// orders should match exactly when we're done.)
  fn arrange_vids(&mut self)->usize {

    type VS = HashSet<VID>;
    let set = |vec:Vec<VID>|->VS { vec.iter().cloned().collect() };
    self.dst.vix(self.rv).expect("rv not found in dst!");
    let v:VS = set(vec![self.rv]);
    let sv:VS = set(self.src.vids.clone());  assert!(!sv.contains(&self.rv));
    let dv:VS = set(self.dst.vids.clone()).difference(&v).cloned().collect();
    let n:VS = dv.intersection(&sv).cloned().collect(); // n = intersection (shared set)
    let s:VS = sv.difference(&n).cloned().collect();    // s = only src
    let d:VS = dv.difference(&n).cloned().collect();    // d = only dst
    println!("dst order before regroup: {:?}", self.dst.vids);
    println!("d: {:?}", d);
    println!("v: {:?}", v);
    println!("n: {:?}", n);
    self.dst.regroup(vec![d, v, n]);
    println!("dst order after regroup: {:?}", self.dst.vids);
    println!("s: {:?}", s);
    // the order of n has to match in both. we'll use the
    // existing order of n from dst because it's probably bigger.
    let vix = self.dst.vix(self.rv).unwrap();
    let mut sg = vec![s.clone()];
    for ni in (vix+1)..self.dst.vids.len() { sg.push(set(vec![self.dst.vids[ni]])) }
    self.src.regroup(sg); // final order: [s,n]

    // now whatever order the s group wound up in, we can insert
    // them in the dst directly *above* v. final order: [ d,v,s,n ]
    for &si in self.src.vids.iter().rev() {
      if s.contains(&si) {
        self.dst.rows.insert(si, XVHLRow::new());
        self.dst.vids.insert(vix+1, si) }}

    println!("dst.vids: {:?}", self.dst.vids);
    println!("src.vids: {:?}", self.src.vids);

    // return the row index at the bottom of set s
    vix}

  /// Replace rv with src(sx) in dst(dx)
  fn sub(&mut self)->XID {

    println!(">>>>>>>>>> self.dx: {:?} -> {:?}", self.dx, self.dst.get(self.dx));

    let rvix = self.dst.vix(self.rv);
    if rvix.is_none() { return self.dx } // rv isn't in the scaffold, so do nothing.
    let vhl = self.dst.get(self.dx).unwrap();
    let vvix = self.dst.vix(vhl.v);
    if vvix.is_none() { panic!("bad vhl:{:?} for self.dx:{:?} ", vhl, self.dx); }

    // !! this is a kludge. the ref count should already be at least 1.
    self.dst.add_ref(vhl, 1);

    // 1. permute vars.
    self.dst.validate("before permute");
    let vix = self.arrange_vids();
    self.dst.validate("after permute");

    // same test again, but after the permute:
    let vhl = self.dst.get(self.dx).unwrap();
    let vvix = self.dst.vix(vhl.v);
    if vvix.is_none() {
      panic!("bad vhl:{:?} for self.dx:{:?} after arrange-vids. how can this happen??", vhl, self.dx); }
    // if the expression doesn't depend on the replacement var, do nothing.
    if rvix.unwrap() > vvix.unwrap() { return self.dx }

    // 2. let q = truth table for src
    let q: Vec<bool> = self.src.tbl(self.sx, None).iter().map(|x|{ x.to_bool() }).collect();
    self.src.validate("replacement bdd");

    // 3. let p = (partial) truth table for dst at the row branching on rv.
    //    (each item is either a const or branches on a var equal to or below rv)
    let mut p: Vec<XID> = self.dst.tbl(self.dx, Some(self.rv));
    println!("rv: {:?}", self.rv);
    println!("p0: {:?}", p);
    for (i, &x) in p.iter().enumerate() { println!("p0[{}]: {:?}", i, self.dst.get(x).unwrap()) }
    println!("---------------");
    // !! tbl() branches from the top var in dx, not the top var in the scaffold.
    //    src may contain vars above branch(dx), so p=tbl(dx) may be smaller than q=tbl(sx).
    //    So: Scale p to the size of q by repeatedly doubling the entries.
    // !! yes, this is a wasteful algorithm but the expectation is that p
    //    and q are quite small: < 2^n items where n = number of vars in
    //    the replacement. I expect n<16, since if n is too much higher than
    //    that, I expect this whole algorithm to break down anyway.

    if p.len() < q.len() {
      let old_p_len = p.len();
      p = p.iter().cycle().take(q.len()).cloned().collect();
      for i in old_p_len..p.len() { self.dst.add_ref_ix(p[i], 1); }}
    println!("p: {:?}", p);
    for (i, &x) in p.iter().enumerate() { println!("p[{}]: {:?}", i, self.dst.get(x).unwrap()) }
    println!("---------------");

    // 4. let r = the partial truth table for result at row rv.
    //    We're removing rv from p (and dst itself) here.
    let r:Vec<XID> = p.iter().zip(q.iter()).map(|(&pi,&qi)|
      if self.dst.branch_var(pi) == self.rv {
        let xid = self.dst.follow(pi, qi);
        self.dst.dec_ref_ix(pi);
        self.dst.add_ref_ix(xid, 1); xid }
      else { pi }).collect();

    // clear all rows above v in the scaffold, and then delete v
    println!("clearing vids={:?} down to rv={:?}", self.dst.vids, self.rv);
    let mut ix = self.dst.vids.len()-1;
    loop {
      let v = self.dst.vids[ix];
      println!("clearing row: {:?}", v);
      // Mark VHLS as garbage (to pass the self-check)
      for ixrc in self.dst.rows[&v].hm.values() {
        assert_eq!(v, self.dst.vhls[ixrc.ix.raw().x as usize].v,
                   "about to collect garbage that isn't mine to collect");
        self.dst.vhls[ixrc.ix.raw().x as usize] = XVHL_O }
      if v == self.rv {
        self.dst.vids.remove(ix);
        self.dst.rows.remove(&v);
        break }
      else {
        self.dst.rows.insert(v, XVHLRow::new());
        ix -= 1 }}
    assert_eq!(ix,vix);

    // -- should be valid again now.
    println!("self.dst.vids: after removing {:?} {:?}", self.rv, self.dst.vids);
    self.dst.validate("after removing top rows");

    // 5. rebuild the rows above set d, and return new top node
    let bv = self.dst.vids[vix]; // whatever the new branch var in that slot is
    println!("vids: {:?}, bv: {:?}, above: {:?}", self.dst.vids, bv, self.dst.vid_above(bv));
    self.dx = self.dst.untbl(r, Some(bv));

    println!("final result: {:?}", self.dst.get(self.dx));
    self.dst.validate("after substitution");

    // 6. garbage collect (TODO?) and return result
    self.dx }} // sub, SwapSolver


fn fun_tbl(f:NID)->Vec<XID> {
  assert!(f.is_fun(), "can't convert non-fun nid to table");
  let ar = f.arity().unwrap();
  let ft = f.tbl().unwrap();
  let mut tbl = vec![XID_O;(1<<ar) as usize];
  let end = (1<<ar)-1;
  for i in 0..=end { if ft & (1<<i) != 0 { tbl[end-i as usize] = XID_I; }}
  tbl }

impl SubSolver for SwapSolver {

  fn init(&mut self, v: VID)->NID {
    self.dst = XVHLScaffold::new(); self.dst.push(v);
    self.rv = v;
    self.dx = self.dst.add_ref(XVHL{ v, hi:XID_I, lo:XID_O}, 1);
    self.dx.to_nid() }

  fn subst(&mut self, ctx: NID, v: VID, ops: &Ops)->NID {
    let Ops::RPN(mut rpn) = ops.norm();
    println!("@:sub {:>4} -> {:>24} -> {:>20}",
      format!("{:?}",v), format!("{:?}", ops), format!("{:?}", rpn));

    let f = rpn.pop().unwrap(); // guaranteed by norm() to be a fun-nid

    // so now, src.vids is just the raw input variables (probably virtual ones).
    self.src = XVHLScaffold::new();
    for nid in rpn.iter() { assert!(nid.is_var()); self.src.push(nid.vid()); }

    // untbl the function to give us the full BDD of our substitution.
    let tbl = fun_tbl(f);
    self.sx = self.src.untbl(tbl, None);

    // everything's ready now, so just do it!
    self.dx = XID::from_nid(ctx);
    self.rv = v;
    let r  = self.sub().to_nid();

    // println!("-----> regrouping in top-down order");
    // let mut ord = self.dst.vids.clone(); ord.sort();
    // self.dst.regroup(ord.iter().rev().cloned().map(|v| { let mut h = HashSet::new(); h.insert(v); h }).collect());
    // println!("-----> sorted vids: {:?}", self.dst.vids);
    r }

  fn get_all(&self, ctx: NID, nvars: usize)->HashSet<Reg> {

    // TODO: prove that we're only copying the nodes directly reachable from xctx.
    // Proper garbage collection should be sufficient for this.

    self.dst.validate("before get_all");

    // Copy from the scaffold to the BDD Base.
    let mut x2n:HashMap<XID,NID> = HashMap::new();
    x2n.insert(XID_O, O);

    // copy each row over, from bottom to top...
    // vids[i] in the scaffold becomes var(i) in the bdd.
    let mut bdd = crate::bdd::BDDBase::new(nvars);
    for (i,rv) in self.dst.vids.iter().enumerate() {
      println!("i,rv: {} {:?}",i,rv);
      let bv = NID::from_vid(VID::var(i as u32));
      for (x, ixrc) in self.dst.rows[rv].hm.iter() {
        if ixrc.rc > 0 {
          println!("x: {:?}", x);
          let nx = |x:XID|->NID { if x.is_inv() { !x2n[&!x] } else { x2n[&x] }};
          let (hi, lo) = (nx(x.hi), nx(x.lo));
          // !! row pairs are never inverted, so we shouldn't have to mess with inv() (... right??)
          x2n.insert(ixrc.ix, bdd.ite(bv, hi, lo)); }}}

    // Now the base solutions back to the original input ordering.
    // Each solution `Reg` contains one bit per input var.
    // To map it back to problem-land:  problem_var[i] = solution_var[self.vix(var(i))]
    // "pv" actually stands for permutation vector, but problem var works too. :)
    let mut pv:Vec<usize> = vec![0;self.dst.vids.len()];
    for (i,v) in self.dst.vids.iter().enumerate() { pv[v.var_ix()] = i; }

    // TODO: fill in extra problem vars that got removed from the final scaffold.
    // !! It may be the case that the problem collapsed from n vars to n-k vars, but
    //    we still need the solution to be in terms of all n vars... Alternately, the
    //    SubSolver protocol could have an output field for discarded inputs.

    let mut res:HashSet<Reg> = HashSet::new();
    let nctx = x2n[&XID::from_nid(ctx)];
    for reg in bdd.solutions_trunc(nctx, nvars) { res.insert(reg.permute_bits(&pv)); }
    res}

  fn status(&self) -> String { "".to_string() } // TODO
  fn dump(&self, _path: &Path, _note: &str, _step: usize, _old: NID, _vid: VID, _ops: &Ops, _new: NID) {
    self.dst.save_dot(_new, format!("xvhl-{:04}.dot", _step).as_str());
  }
}

include!("test-swap.rs");
