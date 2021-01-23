/// Swap Solver
/// This solver attempts to optimize the BDD concept for substitution solving.
/// It adjusts the input variable ordering by swapping adjacent inputs until the
/// one to be replaced next is at the top of the BDD. The actual replacement work
/// at each step then only involves the top three rows.
use std::slice::Iter;
use hashbrown::{HashMap, hash_map::Entry, HashSet};
use {base::{Base,GraphViz}, vid::VID, vid::NOV, nid, nid::NID, bdd::BDDBase};
use vhl::{HiLo, VHL, Walkable};
use std::mem;
use std::cmp::Ordering;

/// XID: An index-based unique identifier for nodes.
///
/// In a regular NID, the branch variable is embedded directly in the ID for easy
/// comparisions. The working assumption is always that the variable refers to
/// the level ofthe tree, and that the levels are numbered in ascending order.
///
/// In contrast, the swap solver works by shuffling the levels so that the next
/// substitution happens at the top, where there are only a small number of nodes.
///
/// When two adjacent levels are swapped, nodes on the old top level that refer to
/// the old bottom level are rewritten as nodes on the new top level. But nodes on
/// the old top level that do not refer to the bottom level remain on the old top
/// (new bottom) level. So some of the nodes with the old top brach variable change
/// their variable, and some do not.
///
/// NIDs are designed to optimize cases where comparing branch variables are important
/// and so the variable is encoded directly in the reference to avoid frequent lookups.
/// For the swap solver, however, this encoding would force us to rewrite the nids in
/// every layer above each swap, and references held outside the base would quickly
/// fall out of sync.
///
/// So instead, XIDs are simple indices into an array (XID=indeX ID). If we want to
/// know the branch variable for a XID, we simply look it up by index in a central
/// vector.
///
/// We could use pointers instead of array indices, but I want this to be a representation
/// that can persist on disk, so a simple flat index into an array of XVHLs is fine for me.

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
struct XID { x: i64 }
const XID_O:XID = XID { x: 0 };
const XID_I:XID = XID { x: !0 };
impl XID {
  fn O()->XID { XID_O }
  fn I()->XID { XID_I }
  fn ix(&self)->usize { self.x as usize }
  fn raw(&self)->XID { if self.x >= 0 { *self } else { !*self }}
  fn is_inv(&self)->bool { self.x<0 }
  fn is_const(&self)->bool { *self == XID_O || *self == XID_I }
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
impl XVHL { fn hilo(&self)->XHiLo { XHiLo { hi:self.hi, lo:self.lo } }}
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
struct XVHLRow { v: VID, hm: HashMap<XHiLo, IxRc> }
impl XVHLRow {
  fn new(v:VID)->Self {XVHLRow{ v, hm: HashMap::new() }}}

/// The scaffold itself contains the master list of records (vhls) and the per-row index
pub struct XVHLScaffold {
  vids: Vec<VID>,
  vhls: Vec<XVHL>,
  rows: HashMap<VID, XVHLRow> }

impl XVHLScaffold {
  fn new()->Self { XVHLScaffold{ vids:vec![], vhls:vec![XVHL_O], rows: HashMap::new() } }

  /// validate that this scaffold is well formed. (this is for debugging)
  pub fn validate(&self) {

    println!("@validate");
    println!("${:?}", self.vids);
    for &x in self.vhls.iter() {
      println!("^{},{},{}", x.v, x.hi.x, x.lo.x)}

    // vids must be unique:
    let mut vids:HashMap<VID, usize> = self.vids.iter().cloned().enumerate().map(|(i,v)|(v,i+1)).collect();
    assert_eq!(vids.len(), self.vids.len(), "duplicate vid(s) in list: {:?}", self.vids);
    vids.insert(NOV, 0);

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
        // the hi and lo branches should point "downward"
        assert!(vids[&self.branch_var(x.lo)] < vids[&x.v], "upward lo branch @vhl[{}]: {:?}", i, x);
        assert!(vids[&self.branch_var(x.hi)] < vids[&x.v], "upward hi branch @vhl[{}]: {:?}", i, x);

        // there should be no duplicate entries.
        if let Some(j) = seen.get(&x) { panic!("vhl[{}] is a duplicate of vhl[{}]: {:?}", i, j, x) }
        else { seen.insert(x, i); }

        // there should be a hashmap entry pointing back to the item:
        if let Some(ixrc) = self.rows[&x.v].hm.get(&XHiLo{ hi:x.hi, lo:x.lo }) {
          let ix = ixrc.ix.raw().x as usize;
          assert_eq!(ix, i, "hashmap stored wrong index ({:?}) for vhl[{}]: {:?} ", ixrc.ix, i, x)}
        else { panic!("no hashmap reference to vhl[{}]: {:?}", i, x) }}

        // TODO: check reference counts
        }
      println!("@/validate")}


  /// return the index (height) of the given variable within the scaffold (if it exists)
  fn vix(&self, v:VID)->Option<usize> { self.vids.iter().position(|&x| x == v) }

  /// return the vid immediately above v in the scaffold, or None
  /// if v is top vid. Panics if v is not in the scaffold.
  fn vid_above(&self, v:VID)->Option<VID> {
    if let Some(x) = self.vix(v) { self.vids.get(x+1).cloned() }
    else { panic!("vid_above(v:{}): v not in the scaffold.", v) }}

  fn vid_below(&self, v:VID)->Option<VID> {
    if let Some(x) = self.vix(v) { if x>0 { self.vids.get(x-1).cloned()} else { None }}
    else { panic!("vid_above(v:{}): v not in the scaffold.", v) }}

  /// add a new vid to the top of the stack. return its position.
  fn push(&mut self, v:VID)->usize {
    // TODO: check for duplicates
    let ix = self.vids.len();
    self.vids.push(v);
    self.rows.insert(v, XVHLRow::new(v));
    ix }

  /// drop top var v (double check that it's actually on top)
  fn drop(&mut self, v:VID) {
    if *self.vids.last().expect("can't drop from empty scaffold") == v {
      self.vids.pop();
      self.rows.remove(&v); }
    else { panic!("can't pop {} because it's not on top ({:?})", v, self.vids) }}

  /// add a reference to the given XVHL, inserting it into the row if necessary.
  /// returns the external nid, and a flag indicating whether the pair was freshly added.
  /// (if it was fresh, the scaffold needs to update the refcounts for each leg)
  fn add_ref(&mut self, hl0:XVHL, rc:usize)->(XID, bool) {
    let inv = hl0.lo.is_inv();
    let vhl = if inv { !hl0 } else { hl0 };
    let row = self.rows.entry(vhl.v).or_insert_with(|| XVHLRow::new(vhl.v));
    let hl = vhl.hilo();
    let (res, isnew) = match row.hm.entry(hl) {
      Entry::Occupied (mut e) => {
        let xid = e.get().ix;
        e.get_mut().rc += rc;
        (xid, false) }
      Entry::Vacant(e) => {
        let ix = self.vhls.len();
        let xid = XID { x: ix as i64 };
        e.insert(IxRc{ ix:xid, rc });
        self.vhls.push(vhl);
        (xid, true) }};
    (if inv { !res } else { res }, isnew) }

  /// decrement refcount for ix. return new refcount.
  fn dec_ref_ix(&mut self, ix:XID)->usize {
    println!("todo: dec_ref_ix");
    1 }

  /// fetch the XVHL for the given xid (if we know it)
  fn get(&self, x:XID)->Option<XVHL> {
    self.vhls.get(x.raw().ix()).map(|&y| if x.is_inv() { !y } else { y }) }

  /// follow the hi or lo branch of x
  fn follow(&self, x:XID, which:bool)->XID {
    let vhl = self.get(x).unwrap();
    if which { vhl.hi } else { vhl.lo }}

  fn branch_var(&self, x:XID)->VID { self.get(x).unwrap().v }

  /// produce the fully expandend "truth table" for a bdd
  /// down to the given row, by building rows of the corresponding
  /// binary tree. xids in the result will either be constants,
  /// branch on the limit var, or branch on some variable below it.
  fn tbl(&self, top:XID, limit:Option<VID>)->Vec<XID> {
    let mut xs = vec![top];
    println!("tbl/xs: {:?}", xs);
    for (i,&x) in xs.iter().enumerate() { println!("  [{}]: x:{} = {:?}", i, x.x, self.get(x).unwrap())}
    let (z,lv) = if let Some(lim) = limit {
      (self.vix(lim).expect("limit var isn't in scaffold") as i64, lim)}
      else {(-1, VID::nov())};
    let mut v = self.get(top).expect("top wasn't in the scaffold").v;
    let mut i = self.vix(v).unwrap() as i64;
    assert!(i >= z, "invalid limit depth {} (var({})) for node on row {}", z, lv, i);
    while i > z {                       // copy-and-expand for each row down to limit
      v = self.vids[i as usize];     // redundant 1st time but can't put at end b/c -1
      let tmp = xs; xs = vec![];
      for x in tmp {
        let vhl = self.get(x).unwrap();
        if vhl.v == v { xs.push(vhl.lo); xs.push(vhl.hi); }
        else { xs.push(x); xs.push(x); }}
      println!("tbl/xs v:{:?}", v);
      for (i,&x) in xs.iter().enumerate() { println!("  [{}]: x:{} = {:?}", i, x.x, self.get(x).unwrap())}
      i-=1}
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
          let t = self.add_ref(XVHL{ v, hi, lo }, 1).0;
          self.dec_ref_ix(hi); self.dec_ref_ix(lo);
          t } }).collect();
      println!("untbl/xs: {:?}", xs);
      if xs.len() == 1 { break }
      v = self.vid_above(v).expect("not enough vars in scaffold to untbl!"); }
    xs[0]}

  fn alloc(&mut self, count:usize)->Vec<XID> {
    let mut i = count; let mut res = vec![];
    while i > 0 {
      // TODO: reclaim garbage collected xids.
      let x = self.vhls.len() as i64;
      self.vhls.push(XVHL_O);
      res.push(XID{x});
      i-=1 }
    res }

  /// swap v up by one level
  fn swap(&mut self, v:VID) {
    #[cfg(test)] {
      println!("swap({}) in {:?}. validating.", v, self.vids); self.validate(); println!("ok! begin swap.") }
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
    let mut rv = self.rows.remove(&v).unwrap();

    // row w may contain nodes that refer to v, which now need to be moved to row v.
    let mut rw = self.rows.remove(&w).unwrap();

    // build a map of xid->hilo for row v, so we know every xid that branches on v,
    // and can quickly retrieve its high and lo branches.
    let mut vx:HashMap<XID,(XID,XID)> = HashMap::new();
    for (vhl, ixrc) in rv.hm.iter() { vx.insert(ixrc.ix, vhl.as_tup()); }

    // moving a node from w->v modifies the old node in place, so no new xid is used.
    // (we know it's not already on row v because before the lift, row v could not refer to w)
    // at least one of the node's children will be replaced by a new node on row w. proof:
    //     general pattern of 3rd level rewrite is   abcd ->  acbd
    //     we can consolidate one side of the swap: abac -> aabc (so we get v?a:w?b:c)
    //     but we can't consodilate both:  abab -> abba, because abab can't appear in a bdd.
    //     no other pattern would result in consolidating both sides. (qed)
    // therefore: worst case for growth is every node in w moves to v and creates 2 new children.
    // a block of 2*w.len ids for this algorithm to assign.
    // so: in the future, if we want to create new nodes with unique ids in a distributed way,
    // we should allocate 2*w.len ids for this function to assign to the new nodes.
    // reference counts elsewhere in the graph can change, but never drop to 0.
    // if they did, then swaping the rows back would have to create new nodes elsewhere.

    // helpers to track which new nodes are to be created.
    // i am doing this because i don't want to refer to self -- partially to appease the
    // borrow checker immediately, but also because in the future i'd like this to be done
    // in a distributed process, which will modify the two rows in place and then send the
    // refcount and branch variable changes to a central repo.
    enum XWIP0 { XID(XID), HL(XID,XID) }

    let mut edec:Vec<XID> = vec![];          // external nodes to decref
    let mut child = |h:XID, l:XID|->XWIP0 {    // reference a node on/below row w, or or create a node on row w
      let (hi, lo, inv) = if l.is_inv() {(!h, !l, true)} else {(h, l, false)};
      if hi == lo { edec.push(hi); XWIP0::XID(if inv { !lo } else { lo }) } // meld nodes (so dec ref)
      else if let Some(ixrc) = rw.hm.get(&XHiLo{ hi, lo}) { XWIP0::XID(if inv {!ixrc.ix} else {ixrc.ix}) }
      else if inv { XWIP0::HL(!hi, !lo) } else { XWIP0::HL(hi, lo) }};

    let mut vdec = |xid:XID| {
      println!("TODO: vdec");
      let (hi, lo)=vx.get(&xid.raw()).unwrap();
      rv.hm.get_mut(&XHiLo{hi:*hi, lo:*lo}).unwrap().rc -= 0 }; // TODO: -=1

    // 1. Partition nodes on rw into two groups:
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

    let mut wmov0: Vec<(XHiLo,XWIP0,XWIP0)> = vec![];
    let mut new_v = |whl,ii,io,oi,oo| { wmov0.push((whl, child(ii,oi), child(io,oo))) };

    for whl in rw.hm.keys() {
      let (hi, lo) = whl.as_tup();
      let vget = |xid:XID|->Option<(XID,XID)> {
        if xid.is_inv() { vx.get(&xid.raw()).cloned().map(|(h,l)| (!h,!l)) } else { vx.get(&xid).cloned() }};
      match (vget(hi), vget(lo)) {
        (None,          None         ) => {},  // no refs, so nothing to do.
        (None,          Some((oi,oo))) => { new_v(*whl, hi, hi, oi, oo); vdec(lo) },
        (Some((ii,io)), None         ) => { new_v(*whl, ii, io, lo, lo); vdec(hi) },
        (Some((ii,io)), Some((oi,oo))) => { new_v(*whl, ii, io, oi, oo); vdec(hi); vdec(lo) }}}

    // convert the XWIP0::HL entries to XWIP1::NEW
    enum XWIP1 { XID(XID), NEW(i64) }
    let mut wnix:i64 = 0; // next index for new node
    let mut wnew: HashMap<(XID,XID), IxRc> = HashMap::new();
    let mut eref: Vec<XID> = vec![]; // external nodes to incref
    let mut resolve = |xw0|->XWIP1 {
      match xw0 {
        XWIP0::XID(x) => { eref.push(x); XWIP1::XID(x) },
        XWIP0::HL(hi0,lo0) => {
          // these are the new children on the w level. it may turn out that these already
          // existed on w, in the set of nodes that did not refer to v. So either we incref
          // the existing node, or we create a new node:
          // TODO: this isn't really an IxRc since the xid is virtual
          let (hi,lo,inv) = if lo0.is_inv() { (!hi0, !lo0, true) } else { (hi0,lo0,false) };
          match wnew.entry((hi, lo)) {
            Entry::Occupied(mut e) => { e.get_mut().rc += 1; XWIP1::NEW(e.get().ix.x) }
            Entry::Vacant(e) => {
              let x = wnix as i64; wnix += 1;
              e.insert(IxRc{ ix:XID{x}, rc:1 });
              XWIP1::NEW(if inv { !x } else { x }) }}}}};

    // make the removals from row w, and fill in wnew, wtov, eref
    let mut wtov: Vec<(IxRc,XWIP1,XWIP1)> = vec![];
    for (whl, wip_hi, wip_lo) in wmov0 {
      // construct new child nodes on the w level, or add new references to external nodes:
      let (hi, lo) = (resolve(wip_hi), resolve(wip_lo));
      // the lo branch should never be inverted:
      // the lo-lo path doesn't change in a swap, and lo branches are always raw
      // in the scaffold. (This means we only have to deal with inverted xids in)
      // the newly-created hi branches.
      if let XWIP1::NEW(x) = lo { assert!(x >= 0, "unexpected !lo branch");}
      // delete the old node from row w. the newly created nodes don't depend on v, and
      // the node to delete does depend on v, so there's never a conflict here.
      let ixrc = rw.hm.remove(&whl).expect("I saw a whl that wasn't there!");
      // we can't add directly to row v until we resolve the XWIP1::NEW entries,
      // but we can make a list of the work to be done:
      wtov.push((ixrc, hi, lo)); }

    // garbage collect on row v. these won't conflict with vnew because we will never
    // add a *completely* new node on row v - only move existing nodes from w, and
    // these will never match existing nodes on v because at least one leg always
    // points at w (and this wasn't possible before the lift). But we may need to delete
    // nodes because the rc dropped to 0 (when the node was only referenced by row w).
    let mut vdel:Vec<XID> = vec![];
    rv.hm.retain(|_, ixrc| {
      if ixrc.rc == 0 { vdel.push(ixrc.ix); false }
      else { true }});

    // If we are deleting from v and adding to w, we can re-use the xids.
    // otherwise, allocate some new xids.
    let xids: Vec<XID> = {
      let have = vdel.len();
      let need = wnew.len(); assert_eq!(need, wnix as usize);
      if need <= have {
        let tmp = vdel.split_off(need);
        let res = vdel; vdel = tmp;
        res }
      else {
        let mut res = vdel; vdel = vec![];
        res.extend(self.alloc(need-have));
        res }};

    // [commit wnew]
    // we now have a xid for each newly constructed (XWIP) child node on row w,
    // so go ahead and add them. we will also map the temp ix to the actual ix.
    let mut wipxid = vec![XID_O; wnix as usize];
    for ((hi,lo), ixrc0) in wnew.iter() {
      let mut ixrc = *ixrc0;
      let inv = ixrc0.ix.x < 0;
      let wipix = if inv { !ixrc0.ix.x } else { ixrc0.ix.x };
      ixrc.ix = xids[wipix as usize];  // map the temp xid -> true xid
      wipxid[wipix as usize] = ixrc.ix; // remember for w2x, below.
      assert!(!ixrc.ix.is_inv());
      assert!(rw.hm.get(&(XHiLo{hi:*hi, lo:*lo})).is_none());
      rw.hm.insert(XHiLo{hi:*hi, lo:*lo}, ixrc);
      // and now update the master store:
      self.vhls[ixrc.ix.x as usize] = XVHL{ v:w, hi:*hi, lo:*lo }; }

    // [commit wtov]
    // with those nodes created, we can finish moving the nodes from w to v.
    let w2x = |wip:&XWIP1| {
      match wip {
        XWIP1::XID(x) => *x,
        XWIP1::NEW(x) => { if *x<0 { !wipxid[!*x as usize]  } else { wipxid[*x as usize ]}}}};
    for (ixrc, wip_hi, wip_lo) in wtov.iter() {
      let (hi, lo) = (w2x(wip_hi), w2x(wip_lo));
      rv.hm.insert(XHiLo{hi, lo}, *ixrc);
      self.vhls[ixrc.ix.x as usize] = XVHL{ v, hi, lo }; }

    // TODO: [ commit vdel ]
    // we've already removed them from the local copy. just need to add the
    // original entries to a linked list.
    for xid in vdel { self.vhls[xid.raw().ix()].v = NOV }

    // TODO: [ commit eref changes ]
    // TODO: merge eref and edec into a hashmap of (XID->drc:usize)
    // it should be usize rather than i64 because nothing outside of these two rows
    // will ever have its refcount drop all the way to 0.
    // (each decref is something like (w?(v?a:b):(v?a:c))->(v?a:w?b:c) so we're just
    // merging two references into one, never completely deleting one).

    // finally, put the rows back where we found them:
    self.rows.insert(v, rv);
    self.rows.insert(w, rw);
    #[cfg(test)] { println!("swap complete. validating."); self.validate(); println!("valid!") }}

  /// arrange row order to match the given groups.
  /// the groups are given in bottom-up order, and should
  /// completely partition the scaffold vids.
  // TODO: executes these swaps in parallel
  fn regroup(&mut self, groups:Vec<HashSet<VID>>) {
    // TODO: check for complete partition
    let mut lc = 0; // left cursor
    let mut rc = 0; // right cursor
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
          while rc > lc { rc -= 1; self.swap(self.vids[rc]) }}}}}

} // impl XVHLScaffold


/// A simple RPN debugger to make testing easier.
struct XSDebug {
  /** scaffold */   xs: XVHLScaffold,
  /** xid->char */  xc: HashMap<XID,char>,  // used in fmt for direct var refs
  /** vid->char */  vc: HashMap<VID,char>,  // used in fmt for branch vars from xs
  /** char->xid */  cx: HashMap<char,XID>,  // used in run to map iden->xid
  /** xid->vid */   xv: HashMap<XID,VID>,   // used in ite
  /** data stack */ ds: Vec<XID>}

impl XSDebug {
  pub fn new(vars:&str)->Self {
    let mut this = XSDebug {
      xs: XVHLScaffold::new(), ds: vec![],
      xc: HashMap::new(), vc:HashMap::new(), cx: HashMap::new(), xv: HashMap::new() };
    for (i, c) in vars.chars().enumerate() { this.var(i, c) }
    this }
  fn var(&mut self, i:usize, c:char) {
    let v = VID::var(i as u32); self.xs.push(v); self.name_var(v, c); }
  fn vids(&self)->String { self.xs.vids.iter().map(|v| *self.vc.get(v).unwrap()).collect() }
  fn name_var(&mut self, v:VID, c:char) {
    let x:XID = self.xs.add_ref(XVHL{ v, hi:XID_I, lo:XID_O}, 1).0;
    self.xc.insert(x, c); self.vc.insert(v, c);
    self.cx.insert(c, x); self.xv.insert(x, v);}
  fn pop(&mut self)->XID { self.ds.pop().expect("stack underflow") }
  fn xid(&mut self, s:&str)->XID { self.run(s); self.pop() }
  fn vid(&self, c:char)->VID { *self.cx.get(&c).map(|x| self.xv.get(x).unwrap()).unwrap() }
  fn run(&mut self, s:&str)->String {
    for c in s.chars() {
      match c {
        'a'..='z' => if let Some(&x) = self.cx.get(&c) { self.ds.push(x) }
          else { panic!("unknown variable: {}", c)},
        '0' => self.ds.push(XID_O),
        '1' => self.ds.push(XID_I),
        '.' => { self.ds.pop(); },
        '!' => { let x= self.pop(); self.ds.push(!x) },
        ' ' => {}, // no-op
        '#' => { // untbl
          let v = if self.ds.len()&1 == 0 { None } else {
            let x = self.pop();
            Some(*self.xv.get(&x).expect("last item in odd-len stack was not var for #"))};
          let x = self.xs.untbl(self.ds.clone(), v); // TODO: how can I just move ds here?
          self.ds = vec![x]; },
        '?' => { let vx=self.pop(); let hi = self.pop(); let lo = self.pop(); self.ite(vx,hi,lo); },
        _ => panic!("unrecognized character: {}", c)}}
    if let Some(&x) = self.ds.last() { self.fmt(x) } else { "".to_string() }}
  fn ite(&mut self, vx:XID, hi:XID, lo:XID)->XID {
    if let Some(&v) = self.xv.get(&vx) {
      let res = self.xs.add_ref(XVHL{ v, hi, lo }, 1).0;
      self.ds.push(res); res }
    else {  panic!("not a branch var: {}", self.fmt(vx)) }}
  fn fmt(&self, x:XID)->String {
    match x {
      XID_O => "0".to_string(),
      XID_I => "1".to_string(),
      _ => { let inv = x.x < 0; let x = x.raw(); let sign = if inv { "!" } else { "" };
        if let Some(&c) = self.xc.get(&x) { format!("{}{}", c, sign).to_string() }
        else {
          let XVHL{v,hi,lo} = self.xs.vhls[x.x as usize];
          let vc:char = *self.vc.get(&v).expect(&format!("couldn't map branch var back to char: {:?}", v));
          format!("{}{}{}?{} ", self.fmt(lo), self.fmt(hi), vc, sign) } } }}}

// ------------------------------------------------------

pub struct SwapSolver {
  /** the result (destination) bdd  */  dst: XVHLScaffold,
  /** top node in the destination   */  dx: XID,
  /** the variable we're replacing  */  rv: VID,
  /** the replacement (source) bdd  */  src: XVHLScaffold,
  /** top node in the source bdd    */  sx: XID }

impl SwapSolver {
  /// constructor
  fn new(v: VID) -> Self {
    let mut dst = XVHLScaffold::new(); dst.push(v);
    let dx = dst.add_ref(XVHL{ v, hi:XID_I, lo:XID_O }, 1).0;
    let src = XVHLScaffold::new();
    SwapSolver{ dst, dx, rv:v, src, sx: XID_O }}

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
    self.dst.regroup(vec![d, v, n]);

    // the order of n has to match in both. we'll use the
    // existing order of n from dst because it's probably bigger.
    let vix = self.dst.vix(self.rv).unwrap();
    let mut sg = vec![s.clone()];
    for ni in (vix+1)..self.dst.vids.len() { sg.push(set(vec![self.dst.vids[ni]])) }
    self.src.regroup(sg); // final order: [s,n]

    // now whatever order the s group wound up in, we can insert
    // them in the dst directly *above* v. final order: [ d,v,s,n ]
    for &si in self.src.vids.iter().rev() {
      if s.contains(&si) { self.dst.vids.insert(vix+1, si) }}

    println!("dst.vids: {:?}", self.dst.vids);
    println!("src.vids: {:?}", self.src.vids);

    // return the row index at the bottom of set s
    vix}

  /// Replace rv with src(sx) in dst(dx)
  fn sub(&mut self)->XID {

    // 1. permute vars.
    let vix = self.arrange_vids();

    // 2. let q = truth table for src
    let q: Vec<bool> = self.src.tbl(self.sx, None).iter().map(|x|{ x.to_bool() }).collect();

    // 3. let p = (partial) truth table for dst at the row branching on rv.
    //    (each item is either a const or branches on a var equal to or below rv)
    let mut p: Vec<XID> = self.dst.tbl(self.dx, Some(self.rv));
    println!("rv: {:?}", self.rv);
    println!("p0: {:?}", p);
    for (i, &x) in p.iter().enumerate() { println!("p0[{}]: {:?}", i, self.dst.get(x).unwrap()) }
    println!("---------------");
    //    Scale p to the size of q by repeatedly doubling the entries.
    //    !! yes, this is a wasteful algorithm but the expectation is that p
    //       and q are quite small: < 2^n items where n = number of vars in
    //       the replacement. I expect n<16, since if n is too much higher than
    //       that, I expect this whole algorithm to break down anyway.
    if p.len() < q.len() { p = p.iter().cycle().take(q.len()).cloned().collect() }
    println!("p: {:?}", p);
    for (i, &x) in p.iter().enumerate() { println!("p[{}]: {:?}", i, self.dst.get(x).unwrap()) }
    println!("---------------");

    // 4. let r = the partial truth table for result at row rv.
    //    We're removing rv from p (and dst itself) here.
    let mut r:Vec<XID> = p.iter().zip(q.iter()).map(|(&pi,&qi)|
      if self.dst.branch_var(pi) == self.rv { self.dst.follow(pi, qi) } else { pi }).collect();
    println!("p: {:?}\nq: {:?}\nr: {:?}", p, q, r);
    for (i, &x) in p.iter().enumerate() { println!("r[{}]: {:?}", i, self.dst.get(x).unwrap()) }
    println!("---------------");
    for (i, &x) in r.iter().enumerate() { println!("r[{}]: {:?}", i, self.dst.get(x).unwrap()) }

    self.dst.vids.remove(self.dst.vix(self.rv).unwrap());
    self.dst.rows.remove(&self.rv);

    // 5. rebuild the rows above set d, and return new top node
    let bv = self.dst.vids[vix]; // whatever the new branch var in that slot is
    println!("vids: {:?}, bv: {:?}, above: {:?}", self.dst.vids, bv, self.dst.vid_above(bv));
    self.dx = self.dst.untbl(r, Some(bv));

    println!("final result: {:?}", self.dst.get(self.dx));

    // 6. garbage collect (TODO?) and return result
    self.dx }} // sub, SwapSolver



include!("test-swap.rs");
