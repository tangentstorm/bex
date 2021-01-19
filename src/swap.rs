/// Swap Solver
/// This solver attempts to optimize the BDD concept for substitution solving.
/// It adjusts the input variable ordering by swapping adjacent inputs until the
/// one to be replaced next is at the top of the BDD. The actual replacement work
/// at each step then only involves the top three rows.
use std::slice::Iter;
use hashbrown::{HashMap, hash_map::Entry, HashSet};
use {base::{Base,GraphViz,SubSolver}, vid::VID, vid::NOV, nid, nid::NID, bdd::BDDBase};
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
const XVHL_O:XVHL = XVHL{ v: NOV, hi:XID_O, lo:XID_I };

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
    let z = if let Some(lim) = limit {
      self.vix(lim).expect("limit var isn't in scaffold") as i64}
      else {0};
    let z = z - 1; // move one more row down.
    let mut v = self.get(top).expect("top wasn't in the scaffold").v;
    let mut i = self.vix(v).unwrap() as i64;
    assert!(i >= z, "invalid limit depth {} for node on row {}", z, i);
    while i > z {                       // copy-and-expand for each row down to limit
      v = self.vids[i as usize];     // redundant 1st time but can't put at end b/c -1
      let tmp = xs; xs = vec![];
      for x in tmp {
        let vhl = self.get(x).unwrap();
        if vhl.v == v { xs.push(vhl.lo); xs.push(vhl.hi); }
        else { xs.push(x); xs.push(x); }}
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
    let mut new_w = |hi, lo|->XWIP0 {
      if hi == lo { edec.push(hi); XWIP0::XID(lo) } // meld nodes (so dec ref)
      else { XWIP0::HL(hi, lo) }};

    let mut vdec = |xid| {
      println!("TODO: vdec");
      let (hi, lo)=vx.get(&xid).unwrap();
      rv.hm.get_mut(&XHiLo{hi:*hi, lo:*lo}).unwrap().rc -= 0 }; // TODO: -=1

    let mut wmov0: Vec<(XHiLo,XWIP0,XWIP0)> = vec![];
    let mut new_v = |whl,ii,io,oi,oo| { wmov0.push((whl, new_w(ii,oi), new_w(io,oo))) };

    for whl in rw.hm.keys() {
      let (hi, lo) = whl.as_tup();
      match (vx.get(&hi), vx.get(&lo)) {
        (None,          None         ) => {},  // no refs, so nothing to do.
        (None,          Some((oi,oo))) => { new_v(*whl, hi, hi,*oi,*oo); vdec(lo) },
        (Some((ii,io)), None         ) => { new_v(*whl,*ii,*io, lo, lo); vdec(hi) },
        (Some((ii,io)), Some((oi,oo))) => { new_v(*whl,*ii,*io,*oi,*oo); vdec(hi); vdec(lo) }}}

    // convert the XWIP0::HL entries to XWIP1::NEW
    enum XWIP1 { XID(XID), NEW(i64) }
    let mut wnix:i64 = 0; // next index for new node
    let mut wnew: HashMap<(XID,XID), IxRc> = HashMap::new();
    let mut eref: Vec<XID> = vec![]; // external nodes to incref
    let mut resolve = |xw0|->XWIP1 {
      match xw0 {
        XWIP0::XID(x) => { eref.push(x); XWIP1::XID(x) },
        XWIP0::HL(hi,lo) => {
          // these are the new children on the w level. it may turn out that these already
          // existed on w, in the set of nodes that did not refer to v. So either we incref
          // the existing node, or we create a new node:
          // TODO: this isn't really an IxRc since the xid is virtual
          match wnew.entry((hi, lo)) {
            Entry::Occupied(mut e) => { e.get_mut().rc += 1; XWIP1::NEW(e.get().ix.x) }
            Entry::Vacant(e) => {
              let x = wnix as i64; wnix += 1;
              e.insert(IxRc{ ix:XID{x}, rc:1 });
              XWIP1::NEW(x) }}}}};

    // make the removals from row w, and fill in wnew, wtov, eref
    let mut wtov: Vec<(IxRc,XWIP1,XWIP1)> = vec![];
    for (whl, wip_hi, wip_lo) in wmov0 {
      // construct new child nodes on the w level, or add new references to external nodes:
      let (hi, lo) = (resolve(wip_hi), resolve(wip_lo));
      // delete the old node from row w. the newly created nodes don't depend on v, and
      // the node to delete does depend on v, so there's never a conflict here.
      let ixrc = rw.hm.remove(&whl).expect("I saw a whl that wasn't there!");
      // we can't add directly to row v until we resolve the XWIP1::NEW entries,
      // but we can make a list of the work to be done:
      wtov.push((ixrc, hi, lo)); }

    // garbage collect on row v. these won't conflict with vnew because we will never
    // add a *completely* new node on row v - only move existing nodes from w, and
    // these will never match existing nodes on because at least one leg always
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
      let wipix = ixrc0.ix.x as usize;
      ixrc.ix = xids[wipix as usize];  // map the temp xid -> true xid
      wipxid[wipix as usize] = ixrc.ix; // remember for w2x, below.
      rw.hm.insert(XHiLo{hi:*hi, lo:*lo}, ixrc);
      // and now update the master store:
      self.vhls[ixrc.ix.x as usize] = XVHL{ v:w, hi:*hi, lo:*lo }; }

    // [commit wtov]
    // with those nodes created, we can finish moving the nodes from w to v.
    let w2x = |wip:&XWIP1| {
      match wip {
        XWIP1::XID(x) => *x,
        XWIP1::NEW(x) => { wipxid[*x as usize] } }};
    for (ixrc, wip_hi, wip_lo) in wtov.iter() {
      let (hi, lo) = (w2x(wip_hi), w2x(wip_lo));
      rv.hm.insert(XHiLo{hi, lo}, *ixrc);
      self.vhls[ixrc.ix.x as usize] = XVHL{ v, hi, lo }; }

    // TODO: [ commit vdel ]
    // we've already removed them from the local copy. just need to add the
    // original entries to a linked list.

    // TODO: [ commit eref changes ]
    // TODO: merge eref and edec into a hashmap of (XID->drc:usize)
    // it should be usize rather than i64 because nothing outside of these two rows
    // will ever have its refcount drop all the way to 0.
    // (each decref is something like (w?(v?a:b):(v?a:c))->(v?a:w?b:c) so we're just
    // merging two references into one, never completely deleting one).

    // finally, put the rows back where we found them:
    self.rows.insert(v, rv);
    self.rows.insert(w, rw);

  } // fn lift

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

/// index + refcount (used by VHLRow)
#[derive(Debug, PartialEq, Eq)]
struct IxRc0 { ix:nid::IDX, rc: u32 }

/// VHLRow represents a single row in a VHL-graph
struct VHLRow {
  /** (external) branch vid label   */  v: VID,
  /** (internal) hilo pairs         */  hl: Vec<HiLo>,
  /** index and refcounts for hilos */  ix: HashMap<HiLo,IxRc0>,
  /** internal refcount (sum ix[].1)*/  irc_: u32,
  /** refcount for this row's vid   */  vrc_: u32}
  // /** free list (slots where rc=0)  */  fl: Vec<usize>}

impl VHLRow {
  fn new(v:VID)->Self { VHLRow{ v, hl:vec![], irc_:0, vrc_:0, ix:HashMap::new()}} //, fl:vec![] }}

  fn print(&self) {
    print!("v:{} vrc:{} [", self.v, self.vrc());
    for hl in &self.hl { print!(" ({}, {})", hl.hi, hl.lo)}
    println!(" ]"); }


  fn vrc(&self)->u32 { self.vrc_ }
  fn irc(&self)->u32 { self.irc_ }

  /// (new interface)
  /// add a reference to the given (internal) hilo pair, inserting it into the row if necessary.
  /// returns the external nid, and a flag indicating whether the pair was freshly added.
  /// (if it was fresh, the scaffold needs to update the refcounts for each leg)
  fn add_ref(&mut self, hl0:HiLo, rc:u32)->(NID, bool) {
    let inv = hl0.lo.is_inv();
    let hl = if inv { !hl0 } else { hl0 };
    let (res, isnew) = match self.ix.entry(hl) {
      Entry::Occupied (mut e) => {
        let nid = NID::from_vid_idx(self.v, e.get().ix);
        e.get_mut().rc += rc;
        (nid, false) }
      Entry::Vacant(e) => {
        let idx = self.hl.len() as nid::IDX;
        let nid = NID::from_vid_idx(self.v, idx);
        e.insert(IxRc0{ ix:idx, rc });
        self.hl.push(hl);
        (nid, true) }};
    self.irc_ += rc;
    (if inv { !res } else { res }, isnew) }

  /// (old interface) add variable-specific reference
  fn add_vref(&mut self) {
    self.add_ref(HiLo{ hi: nid::I, lo: nid::O }, 1);
    self.irc_ -= 1;
    self.vrc_ += 1; }

  /// (old interface) add hilo-specific reference(s)
  fn add_iref(&mut self, hl0:HiLo, rc:u32)->(NID, bool) {
    assert!( !(hl0.hi.is_const() && hl0.lo.is_const()), "call add_vref for pure vid references");
    self.add_ref(hl0, rc)}}

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

      if let Some(IxRc0) = toprow.ix.get(old) { old_rc.push(IxRc0.rc) }
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
      new_ix.insert(hl, IxRc0{ ix: ix as u32, rc }); }

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
      if n.is_var() { self.rows[vix].add_vref() }
      else {
        let hilo = self.rows[vix].hl[n.idx()];
        if let Some(mut IxRc0) = self.rows[vix].ix.get_mut(&hilo) { IxRc0.rc += 1 }
        else { panic!("can't add ref to nid ({}) that isn't in the scaffold", n)}}}}

  /// add ref using internal index and hilo. returns internal nid and whether it was new
  fn add_iref(&mut self, ix:usize, hl:HiLo, rc:u32)->(NID, bool) {
    let (nid, isnew) = match (hl.hi, hl.lo) {
      // TODO: put all the nid-swapping and ref counting and const/var checking in their own places!!
      (nid::I, nid::O) => { self.rows[ix].add_vref(); ( NID::var(ix as u32), false) }
      (nid::O, nid::I) => { self.rows[ix].add_vref(); (!NID::var(ix as u32), false) }
      _ => {
        let (ex, isnew) = self.rows[ix].add_iref(hl, rc);
        (self.inen(ex), isnew) }};
    (nid,isnew)}


  /// add a reference to the given VHL (inserting it into the appropriate row if necessary)
  /// both the vhl and return NID use external variables
  fn add_ref(&mut self, vhl: VHL)->NID {
    let VHL { v, hi, lo } = vhl;
    let ix = self.ensure_vix(v);
    match (vhl.hi, vhl.lo) {
      (nid::I, nid::O) => { self.rows[ix].add_vref();  NID::from_vid(v) }
      (nid::O, nid::I) => { self.rows[ix].add_vref(); !NID::from_vid(v) }
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
        if row.vrc() > 0 { write!(wr, " {}", ev).unwrap() }
        for i in 0..row.hl.len() { write!(wr, " \"{}\"", NID::from_vid_idx(ev, i as nid::IDX)).unwrap(); }
        w!("}}") }
      if row.vrc() > 0 {
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
  ///  3. new vids from src (set s) are between rv and set d.
  /// so from bottom to top: ( d, s, v, n )
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
    let mut sg = vec![s];
    for ni in (vix+1)..self.dst.vids.len() { sg.push(set(vec![self.dst.vids[ni]])) }
    self.src.regroup(sg); // final order: [s,n]

    // now whatever order the s group wound up in, we can insert
    // them in the dst directly above v. final order: [ d,v,s,n ]
    for &si in self.src.vids.iter().rev() { self.dst.vids.insert(vix, si) }

    // return the row index at the bottom of set x
    vix}

  /// Replace rv with src(sx) in dst(dx)
  fn sub(&mut self)->XID {

    // 1. permute vars.
    let vix = self.arrange_vids();

    // 2. let q = truth table for src
    let q: Vec<bool> = self.src.tbl(self.sx, None).iter().map(|x|{ x.to_bool() }).collect();

    // 3. let p = (partial) truth table for dst at the row branching on rv.
    //    (each item is either a const or branches on a var equal to or below rv)
    let p: Vec<XID> = self.dst.tbl(self.dx, Some(self.rv));

    // 4. let r = the partial truth table for result at row rv.
    //    We're removing rv from p here.
    let mut r:Vec<XID> = p.iter().zip(q.iter()).map(|(&di,&qi)|
      if self.dst.branch_var(di) == self.rv { self.dst.follow(di, qi) } else { di }).collect();
    println!("p: {:?}\nq: {:?}\nr: {:?}", p, q, r);

    // 5. rebuild the rows above set d, and return new top node
    println!("vids: {:?}, rv: {:?}, above: {:?}", self.dst.vids, self.rv, self.dst.vid_above(self.rv));
    self.dx = self.dst.untbl(r, Some(self.dst.vids[vix]));

    println!("final result: {:?}", self.dst.get(self.dx));

    // 6. garbage collect (TODO?) and return result
    self.dx }} // sub, SwapSolver

pub struct OldSwapSolver<T:Base + Walkable> {
  /** normal base for delegation    */  base: T,
  /** base nid for last src def     */  key: NID,
  /** the new "top" at each step    */  src: VHLScaffold,
  /** the solution we're building   */  dst: VHLScaffold}

impl<T:Base + Walkable> OldSwapSolver<T> {

  /// constructor
  fn new(base: T, top:VID)->Self {
    OldSwapSolver { base, key:nid::O, src: VHLScaffold::empty(), dst: VHLScaffold::new(top) }}

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


impl<T:Base + Walkable> Base for OldSwapSolver<T> {
  inherit![ num_vars, when_hi, when_lo, def, tag, get, save, dot ];

  fn new(num_vars:usize)->Self { OldSwapSolver::new(T::new(num_vars), VID::vir((num_vars-1) as u32)) }

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

impl<T:Base+Walkable> SubSolver for OldSwapSolver<T> {
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

pub type BddSwapSolver = OldSwapSolver<BDDBase>;

include!("test-swap.rs");
