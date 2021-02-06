/// Swap Solver
/// This solver attempts to optimize the BDD concept for substitution solving.
/// It adjusts the input variable ordering by swapping adjacent inputs until the
/// one to be replaced next is at the top of the BDD. The actual replacement work
/// at each step then only involves the top three rows.
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
    for &x in self.vhls.iter() {
      println!("^{},{:?},{:?}", x.v, x.hi, x.lo)}

    // vids must be unique:
    let mut vids:HashMap<VID, i64> = self.vids.iter().cloned().enumerate().map(|(i,v)|(v,i as i64)).collect();
    assert_eq!(vids.len(), self.vids.len(), "duplicate vid(s) in list: {:?}", self.vids);
    assert_eq!(vids.len(), self.rows.len(), "vids and rows should have the same len()");
    vids.insert(NOV, -1);

    println!("vids:{:?}", vids);

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
  /// returns the external nid, and a flag indicating whether the pair was freshly added.
  /// (if it was fresh, the scaffold needs to update the refcounts for each leg)
  fn add_ref(&mut self, hl0:XVHL, rc:usize)->XID {
    let inv = hl0.lo.is_inv();
    let vhl = if inv { !hl0 } else { hl0 };
    if vhl == XVHL_O { return if inv { XID_I } else { XID_O }}
    // allocate a xid just in case
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
  fn dec_ref_ix(&mut self, ix:XID)->usize {
    if ix.is_const() { return 1 }
    let vhl = self.vhls[ix.raw().x as usize];
    if let Some(row) = self.rows.get_mut(&vhl.v) {
      if let Some(mut ixrc) = row.hm.get_mut(&vhl.hilo()) {
        if ixrc.rc > 0 { ixrc.rc -= 1; ixrc.rc }
        else { println!("dec_ref warning: ixrc was already 0 for {:?}", vhl); 0 }}
      else { println!("dec_ref warning: entry not found for {:?}", vhl); 0}}
    else { println!("dec_ref warning: row not found for {:?}", vhl.v); 0 }}

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
  fn tbl(&self, top:XID, limit:Option<VID>)->Vec<XID> {
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
          let t = self.add_ref(XVHL{ v, hi, lo }, 1);
          t } }).collect();
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
    #[cfg(test)] {
      self.validate(&format!("swap({}) in {:?}.", v, self.vids)); println!("ok! begin swap.") }
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
    let mut rv = self.rows.remove(&v).unwrap_or_else(|| panic!("row {:?} not found",v));

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
    //     but we can't consolidate both:  abab -> abba, because abab can't appear in a bdd.
    //     no other pattern would result in consolidating both sides. (qed)
    // therefore: worst case for growth is every node in w moves to v and creates 2 new children.
    // a block of 2*w.len ids for this algorithm to assign.
    // so: in the future, if we want to create new nodes with unique ids in a distributed way,
    // we should allocate 2*w.len ids for this function to assign to the new nodes.
    // reference counts elsewhere in the graph can change, but never drop to 0.
    // if they did, then swapping the rows back would have to create new nodes elsewhere.

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
      let (hi, lo)=vx.get(&xid.raw()).unwrap();
      let ixrc = rv.hm.get_mut(&XHiLo{hi:*hi, lo:*lo}).unwrap();
      if ixrc.rc == 0 { println!("warning: rc was already 0"); }
      else { ixrc.rc -= 1; }};

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
    for xid in vdel { self.vhls[xid.raw().ix()] = XVHL_O }

    // TODO: [ commit eref changes ]
    // TODO: merge eref and edec into a hashmap of (XID->drc:usize)
    // it should be usize rather than i64 because nothing outside of these two rows
    // will ever have its refcount drop all the way to 0.
    // (each decref is something like (w?(v?a:b):(v?a:c))->(v?a:w?b:c) so we're just
    // merging two references into one, never completely deleting one).

    // finally, put the rows back where we found them:
    self.rows.insert(v, rv);
    self.rows.insert(w, rw);
    #[cfg(test)] { self.validate("after swap."); println!("valid!") }}

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
          while rc > lc { rc -= 1; self.swap(self.vids[rc]) }}}}}

} // impl XVHLScaffold


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
    let r:Vec<XID> = p.iter().zip(q.iter()).map(|(&pi,&qi)|
      if self.dst.branch_var(pi) == self.rv { self.dst.follow(pi, qi) } else { pi }).collect();

    // clear all rows above v in the scaffold, and then delete v
    println!("clearing vids={:?} down to rv={:?}", self.dst.vids, self.rv);
    let mut ix = self.dst.vids.len()-1;
    loop {
      let v = self.dst.vids[ix];
      println!("clearing row: {:?}", v);
      // Mark VHLS as garbage (to pass the self-check)
      for (vhl, ixrc) in self.dst.rows[&v].hm.iter() {
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

impl SubSolver for SwapSolver {

  fn init(&mut self, v: VID)->NID {
    self.dst = XVHLScaffold::new(); self.dst.push(v);
    self.rv = v;
    self.dx = self.dst.add_ref(XVHL{ v, hi:XID_I, lo:XID_O}, 1);
    self.dx.to_nid() }

  fn subst(&mut self, ctx: NID, v: VID, ops: &Ops)->NID {
    self.src = XVHLScaffold::new();
    let mut rpn:Vec<NID> = ops.to_rpn().cloned().collect();
    let f0 = rpn.pop().expect("empty ops passed to subst");
    assert!(f0.is_fun());
    let ar = f0.arity().unwrap();
    assert_eq!(ar, rpn.len() as u8);

    // if any of the input vars are negated, update the function to
    // negate the corresponding argument. this way we can just always
    // branch on the raw variable.
    let mut bits:u8 = 0;
    for (i,nid) in rpn.iter().enumerate() { if nid.is_inv() { bits |= 1 << i; } }
    let f = f0.fun_flip_inputs(bits);

    // so now, src.vids is just the raw input variables (probably virtual ones).
    for nid in rpn { assert!(nid.is_var()); self.src.push(nid.vid()); }

    // untbl the function to give us the full BDD of our substitution.
    let mut tbl = vec![XID_O;(1<<ar) as usize];
    let ft = f.tbl().expect("final op wasn't a function");
    for i in 0..(1<<ar) { if ft & (1<<i) != 0 { tbl[i as usize] = XID_I; }}
    self.sx = self.src.untbl(tbl, None);

    // everything's ready now, so just do it!
    self.dx = XID::from_nid(ctx);
    self.rv = v;
    self.sub().to_nid()}

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
    // TODO: SubSolver::dump()
  }
}

include!("test-swap.rs");
