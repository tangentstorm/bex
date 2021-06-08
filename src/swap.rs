/// Swap Solver
/// This solver attempts to optimize the BDD concept for substitution solving.
/// It adjusts the input variable ordering by swapping adjacent inputs until the
/// one to be replaced next is at the top of the BDD. The actual replacement work
/// at each step then only involves the top three rows.
use base::GraphViz;
use hashbrown::{HashMap, HashSet};
use vid::{VID, NOV, TOP};
use {solve::SubSolver, reg::Reg, nid::{NID,O}, ops::Ops, std::path::Path, base::Base};
use std::{fmt, hash::Hash};
use std::cell::RefCell;
use swarm::{Swarm,Worker,QID,SwarmCmd,WID};

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
pub struct XID { x: i64 }
impl fmt::Debug for XID {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if *self == XID_O { write!(f, "XO")}
    else if *self == XID_I { write!(f, "XI")}
    else { write!(f, "{}#{}", if self.is_inv() { "!" } else {""}, self.raw().x)}}}
const XID_O:XID = XID { x: 0 };
const XID_I:XID = XID { x: !0 };
impl XID {
  fn ix(&self)->usize { self.raw().x as usize }
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
pub struct XVHL { pub v: VID, pub hi: XID, pub lo: XID }
impl XVHL {
  fn hilo(&self)->XHiLo { XHiLo { hi:self.hi, lo:self.lo } }
  fn is_var(&self)->bool { self.v.is_var() && self.hi == XID_I && self.lo == XID_O }}
impl std::ops::Not for XVHL { type Output = XVHL; fn not(self)->XVHL { XVHL { v:self.v, hi:!self.hi, lo:!self.lo }}}

/// Dummy value to stick into vhls[0]
const XVHL_O:XVHL = XVHL{ v: NOV, hi:XID_O, lo:XID_O };

/// Dummy value to use when allocating a new node
const XVHL_NEW:XVHL = XVHL{ v: VID::top(), hi:XID_O, lo:XID_O };

/// index + refcount
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct IxRc { ix:XID, irc: usize, erc: usize }
impl IxRc {
  fn rc(&self)->usize { self.irc + self.erc }
  fn add(&mut self, count:i64) { self.irc = (self.irc as i64 + count) as usize; }}

/**
We need to map:

  ix -> XVHL   (so we can look up info about the node)
  XVHL -> ix   (so we can avoid creating duplicates)
  v -> [ix]    (so we can quickly locate all nodes associated with a variable, and change them)

  these last two can and should be combined into v -> {XHiLo -> IxRc0}
  because we want to frequently swap out whole rows of variables.
  we'll call this XVHLRow
*/
#[derive(Clone, Debug)]
struct XVHLRow { hm: HashMap<XHiLo, IxRc> }
impl XVHLRow {
  fn new()->Self {XVHLRow{ hm: HashMap::new() }}
  /// build a reverse index, mapping of xids to hilo pairs
  fn xid_map(&self)->HashMap<XID,XHiLo> { self.hm.iter().map(|(hl,ixrc)|(ixrc.ix,*hl)).collect() }}

/// The scaffold itself contains the master list of records (vhls) and the per-row index
#[derive(Clone)]
pub struct XVHLScaffold {
  vids: Vec<VID>,
  vhls: Vec<XVHL>,
  rows: HashMap<VID, XVHLRow>,
  /// tracks whether all workers have completed their work
  complete: HashMap<VID,WID>,
  /// tracks rows that are locked during the distributed regroup() operation
  locked: HashSet<VID>,
  /// tracks refcount changes that are pending for locked rows ("deferred refcount delta")
  drcd: HashMap<VID,HashMap<XID, i64>> }

// snapshot used for debugging
thread_local! { static SNAPSHOT : RefCell<XVHLScaffold> = RefCell::new(XVHLScaffold::new()) }

impl XVHLScaffold {
  fn new()->Self { XVHLScaffold{
    vids:vec![], vhls:vec![XVHL_O], rows: HashMap::new(), locked:HashSet::new(), drcd:HashMap::new(), complete:HashMap::new() } }

  pub fn dump(&self, msg:&str) {
    println!("@dump: {}", msg);
    println!("${:?}", self.vids);
    println!("locks: {:?}", self.locked);
    let max = {
      let mut max0 = self.vhls.len();
      for (i, &x) in self.vhls.iter().enumerate().rev() {
        if x.v != NOV { max0 = i+1; break }}
      max0};
    for (i, &x) in self.vhls.iter().enumerate().rev() {
      if i >= max { continue } // hide empty rows at the end
      let rcs = if x.v == NOV || x.v == TOP { "-".to_string() }
      else if self.locked.contains(&x.v) { "[locked]".to_string() } // can't get rc for locked rows
      else {
        let ixrc = self.rows[&x.v].hm.get(&x.hilo()).unwrap();
        assert_eq!(ixrc.ix.x, i as i64);
        format!("(i:{} e:{})",ixrc.irc, ixrc.erc) };
      println!("^{:03}: {} {:?} {:?} {}", i, x.v, x.hi, x.lo, rcs)}
    println!("@/dump");}

  /// validate that this scaffold is well formed. (this is for debugging)
  pub fn validate(&self, msg:&str) {
    if let Err(e) = self.is_valid() {
      println!("==== ERROR: VALIDATION FAILED. ====");
      SNAPSHOT.with(|s| s.borrow().dump("{ last valid snapshot }"));
      println!("===================================");
      println!("error: {}",e);
      self.dump(msg);
      panic!("{}", e)}
    else { SNAPSHOT.with(|s| *s.borrow_mut() = self.clone())}}

  fn is_valid(&self)->std::result::Result<(), String> {

    // vids must be unique:
    let mut vids:HashMap<VID, i64> = self.vids.iter().cloned().enumerate().map(|(i,v)|(v,i as i64)).collect();
    if !(vids.len()==self.vids.len()) { return Err(format!("duplicate vid(s) in list: {:?}", self.vids))}
    if !(vids.len()==self.rows.len()+self.locked.len()) { return Err("vids and rows should have the same len()".to_string()) }
    vids.insert(NOV, -1);

    let mut rc: HashMap<XID, usize> = HashMap::new();
    let mut seen : HashMap<XVHL,usize> = HashMap::new();
    // validate the rows:
    for (i, &x) in self.vhls.iter().enumerate() {
      // NOV indicates gc'd row, TOP is for allocated with alloc() or alloc_one()
      if x.v != NOV && x.v != TOP {

        // the vid should be in the scaffold, or cleared out to indicate a blank row.
        if !vids.contains_key(&x.v) { return Err(format!("invalid v for vhls[{}]: {}", i, x.v))}
        // the lo branch should not be inverted.
        if x.lo.is_inv() {return Err(format!("found inverted lo branch in vhls[{}]: {:?}", i, x))}

        // the lo branch should be different from the hi branch
        if x.lo==x.hi { return Err(format!("unmerged branches in vhl[{}]: {:?}", i, x)) }

        let hi = self.get(x.hi.raw()).expect("hi branch points nowhere");
        let lo = self.get(x.lo.raw()).expect("lo branch points nowhere");

        if !self.locked.contains(&x.v) {
          if hi.v == NOV && x.hi.raw() != XID_O { return Err(format!("hi branch to garbage-collected node {:?} @vhl[{}]",x.hi, i))}
          if lo.v == NOV && x.lo.raw() != XID_O { return Err(format!("lo branch to garbage-collected node {:?} @vhl[{}]",x.lo, i))}}

        // the hi and lo branches should point "downward"
        if !(vids[&lo.v] < vids[&x.v]) { return Err(format!("upward lo branch @vhl[{}]: {:?}", i, x))}
        if !(vids[&hi.v] < vids[&x.v]) { return Err(format!("upward hi branch @vhl[{}]: {:?}", i, x))};

        // there should be no duplicate entries.
        if let Some(j) = seen.get(&x) { return Err(format!("vhl[{}] is a duplicate of vhl[{}]: {:?}", i, j, x)) }
        else { seen.insert(x, i); }

        // there should be a hashmap entry pointing back to the item (but we can only check for unlocked rows):
        if !self.locked.contains(&x.v) {
          if let Some(ixrc) = self.rows[&x.v].hm.get(&XHiLo{ hi:x.hi, lo:x.lo }) {
            let ix = ixrc.ix.raw().x as usize;
            if ix!=i {return Err(format!("hashmap stored wrong index ({:?}) for vhl[{}]: {:?} ", ixrc.ix, i, x))}}
          else { return Err(format!("no hashmap reference to vhl[{}]: {:?}", i, x)) }}

        // update ref counts
        *rc.entry(x.hi.raw()).or_insert(0)+=1;
        *rc.entry(x.lo.raw()).or_insert(0)+=1; }}

      // if we are running this in the middle of a regroup(), we may have deferred refcounts.
      let mut drcd : HashMap::<XID,i64> = HashMap::new();
      for (_, hm) in &self.drcd {
        for (xid, drc) in hm {
          *drcd.entry(xid.raw()).or_insert(0) += drc; }}

      // check internal refcounts vs the ones we just calculated
      for (_v, row) in self.rows.iter() {
        for (_hl, ixrc) in row.hm.iter() {
          let xrc = *rc.get(&ixrc.ix.raw()).unwrap_or(&0) as i64;
          let drc = *drcd.get(&ixrc.ix.raw()).unwrap_or(&0);
          // *subtract* drc from expected count because those changes haven't happened yet.
          let expect = (xrc - drc) as usize;
          if ixrc.irc < expect {
            return Err(format!("refcount was too low for xid: {:?} (expected {}-{}={}, got {})",
               ixrc.ix, xrc, drc, expect, ixrc.irc)) }
          else if ixrc.irc > expect {
            return Err(format!("refcount was too high for xid: {:?} (expected {}-{}={}, got {})",
               ixrc.ix, xrc, drc, expect, ixrc.irc)) }
              }}
      Ok(())}

  pub fn get_ixrc(&self, x:XID)->Option<&IxRc> {
    let XVHL{ v, hi, lo } = self.vhls[x.ix()];
    self.rows[&v].hm.get(&XHiLo{ hi, lo }) }
  pub fn del_node(&mut self, x:XID) {
    let XVHL{ v, hi, lo } = self.vhls[x.ix()];
    self.add_ref_ix_or_defer(hi, -1);
    self.add_ref_ix_or_defer(lo, -1);
    self.vhls[x.ix()] = XVHL_O;
    self.rows.get_mut(&v).unwrap().hm.remove(&XHiLo{ hi, lo }); }
  pub fn get_refcount(&self, x:XID)->Option<usize> { self.get_ixrc(x).map(|ixrc| ixrc.irc) }
  pub fn ixrcs_on_row(&self, v:VID)->HashSet<&IxRc> { self.rows[&v].hm.values().collect() }
  pub fn xids_on_row(&self, v:VID)->HashSet<XID> { self.rows[&v].hm.values().map(|ixrc| ixrc.ix).collect() }

  /// return the index (height) of the given variable within the scaffold (if it exists)
  fn vix(&self, v:VID)->Option<usize> { self.vids.iter().position(|&x| x == v) }

  /// Some(top vid), or None if empty
  fn top_vid(&self)->Option<VID> { let len = self.vids.len(); if len>0 { Some(self.vids[len-1]) } else { None }}

  /// return the vid immediately above v in the scaffold, or None
  /// if v is top vid. Panics if v is not in the scaffold.
  fn vid_above(&self, v:VID)->Option<VID> {
    if let Some(x) = self.vix(v) { self.vids.get(x+1).cloned() }
    else { panic!("vid_above(v:{}): v not in the scaffold.", v) }}

  fn vid_below(&self, v:VID)->Option<VID> {
     if let Some(x) = self.vix(v) { if x>0 { self.vids.get(x-1).cloned()} else { None }}
     else { panic!("vid_below(v:{}): v not in the scaffold.", v) }}

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
  fn add_ref(&mut self, hl0:XVHL, irc:usize, erc:usize)->XID {
    let inv = hl0.lo.is_inv();
    let vhl = if inv { !hl0 } else { hl0 };
    if vhl == XVHL_O { return if inv { XID_I } else { XID_O }}
    debug_assert_ne!(vhl.hi, vhl.lo, "hi and lo should be different"); // to trigger traceback
    let hl = vhl.hilo();
    let row = self.rows.entry(vhl.v).or_insert_with(|| XVHLRow::new());
    let ixrc =
      if let Some(mut x) = row.hm.remove(&hl) { x.irc += irc; x.erc += erc; x }
      else { // entry was vacant:
        let alloc = self.alloc_one();
        self.vhls[alloc.x as usize] = vhl;
        let hi = self.get(vhl.hi).unwrap(); self.add_ref(hi,1,0);
        let lo = self.get(vhl.lo).unwrap(); self.add_ref(lo,1,0);
        IxRc{ ix:alloc, irc, erc }};
      // !! is there a way to just use row here, and still have &mut self for the new entry code?
      self.rows.get_mut(&vhl.v).unwrap().hm.insert(hl, ixrc);
      let res = ixrc.ix;
      if inv { !res } else { res }}

  fn add_iref_ix(&mut self, ix:XID, dirc:i64) { self.add_refs_ix(ix, dirc, 0) }
  fn add_eref_ix(&mut self, ix:XID, derc:i64) { self.add_refs_ix(ix, 0, derc) }

  fn add_refs_ix(&mut self, ix:XID, dirc:i64, derc:i64) {
    if ix.is_const() { return }
    let vhl = self.vhls[ix.raw().x as usize];
    if let Some(row) = self.rows.get_mut(&vhl.v) {
      if let Some(mut ixrc) = row.hm.get_mut(&vhl.hilo()) {
        if dirc < 0 && (dirc + ixrc.irc as i64 ) < 0 { panic!("dirc would result in negative refcount")}
        else { ixrc.irc = (ixrc.irc as i64 + dirc) as usize; }
        if derc < 0 && (derc + ixrc.erc as i64 ) < 0 { panic!("derc would result in negative refcount")}
        else { ixrc.erc = (ixrc.erc as i64 + derc) as usize; }}
      else { panic!("add_ref_ix warning: entry not found for {:?}", vhl) }}
    else if ix.raw() == XID_O { return } // ignore refs to XID_O/XID_I for now
    else { panic!("add_ref_ix warning: row not found for {:?}", vhl.v); }}

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
        if lo == hi { lo }
        else { self.add_ref(XVHL{ v, hi, lo }, 0, 0)} }).collect();
      if xs.len() == 1 { break }
      v = self.vid_above(v).expect("not enough vars in scaffold to untbl!"); }
    xs[0]}

  /// allocate a single xid
  // TODO: cache the empty slots so this doesn't take O(n) time.
  fn alloc_one(&mut self)->XID {
    for (j,vhl) in self.vhls.iter_mut().enumerate().skip(1) {
      if vhl.v == NOV { *vhl = XVHL_NEW; return XID{x:j as i64 }}}
    self.vhls.push(XVHL_NEW); XID{x:self.vhls.len() as i64-1}}

  /// allocate free xids
  fn alloc(&mut self, count:usize)->Vec<XID> {
    let mut i = count; let mut res = vec![];
    if count == 0 { return res }
    // reclaim garbage collected xids.
    for (j,vhl) in self.vhls.iter_mut().enumerate().skip(1) {
      if vhl.v == NOV {
        *vhl = XVHL_NEW;
        res.push(XID{x:j as i64});
        i-= 1;  if i == 0 { break; }}}
    // create new xids if there weren't enough reclaimed ones.
    // note that we give these nodes a fake variable distinct from NOV,
    // so that we don't allocate the same slot when running regroup()
    // across multiple threads
    while i > 0 {
      let x = self.vhls.len() as i64;
      self.vhls.push(XVHL_NEW);
      res.push(XID{x});
      i-=1 }
    res }

  /// swap vu's row up by one level in the scaffold.
  /// This is a much simpler form of the logic used by regroup().
  /// If you are doing more than one swap, you should call regroup() instead,
  /// because it will take advantage of multiple cores to perform all the swaps in parallel.
  fn swap(&mut self, vu:VID) {
    #[cfg(test)] { self.validate(&format!("swap({}) in {:?}.", vu, self.vids)); }
    let uix = self.vix(vu).expect("requested vid was not in the scaffold.");
    if uix+1 == self.vids.len() { println!("warning: attempt to lift top vid {}", vu); return }
    let vd = self.vids[uix+1]; // start: u is 1 level below d
    self.vids.swap(uix+1, uix);

    //  row d:   d ____                u        u ____
    //           :     \                        :     \
    //  row u:   u __    u __      =>  d   =>   d __    d __
    //           :   \    :  \                  :   \    :   \
    //           oo   oi  io  ii                oo   io  oi   ii
    let ru = self.rows.remove(&vu).unwrap();
    let rd = self.rows.remove(&vd).unwrap();
    let mut worker = SwapWorker::default();
    worker.set_ru(vu, ru).set_rd(vd, rd).find_movers();
    let needed = worker.recycle();
    let xids = self.alloc(needed);

    // self.locked.insert(vu); self.locked.insert(vd);
    // self.dump("before reclaim_nodes");
    // self.locked.remove(&vu); self.locked.remove(&vd);

    // remove nodes (do these first in case the refcount changes touch umov)
    let dels = worker.dels.len();
    self.reclaim_swapped_nodes(std::mem::replace(&mut worker.dels, vec![]));

    // commit changes to nodes:
    let (dnew, wipxid) = worker.dnew_mods(xids); let dnews=dnew.len();
    for (ix, hi, lo) in dnew { self.vhls[ix] = XVHL{ v: vd, hi, lo } }
    let unew = worker.umov_mods(wipxid); let unews=unew.len();
    for (ix, hi, lo) in unew { self.vhls[ix] = XVHL{ v: vu, hi, lo } }

    // [ commit refcount changes ]
    for (xid, dc) in worker.refs.iter() { self.add_iref_ix(*xid, *dc); }

    // finally, put the rows back where we found them:
    self.rows.insert(vu, worker.ru);
    self.rows.insert(vd, worker.rd);

    let counts:Vec<usize> = self.vids.iter().map(|v| self.rows[v].hm.len()).collect();
    println!("%swapped: vu:{:?} vd:{:?}", vu, vd);
    println!("%stats: dnews:{} unews:{} dels:{}", dnews, unews, dels);
    println!("%vids: {:?}", self.vids);
    println!("%counts: {:?}", counts);
    #[cfg(test)] { self.validate(format!("after swapping vu:{:?} and vd:{:?}.",vu,vd).as_str()); }}

  /// Reclaim the records for a list of garbage collected nodes.
  /// note: this should ONLY be called from swap() or regroup() because
  /// it doesn't change refcounts (since those functions handle the refcounting)
  // TODO: add to some kind of linked list so they're easier to find.
  fn reclaim_swapped_nodes(&mut self, xids:Vec<XID>) { for xid in xids { self.vhls[xid.raw().ix()] = XVHL_O }}

  /// Remove all nodes from the top rows of the scaffold, down to and including row v.
  /// (the rows themselves remain in place).
  fn clear_top_rows(&mut self, rv:VID) {
    assert!(self.locked.is_empty(), "refcounting would break if .locked is true here");
    let mut ix = self.vids.len()-1;
    loop {
      // we're working a row at a time from the top down.
      let v = self.vids[ix];
      for xid in self.xids_on_row(v) { self.del_node(xid); }
      if v == rv { break } else { ix -= 1 } }}

  /// v: the vid to remove
  fn remove_empty_row(&mut self, v:VID) {
    let ix = self.vix(v).expect("can't remove a row that doesn't exist.");
    assert!(self.rows[&v].hm.is_empty(), "can't remove a non-empty row!");
    self.vids.remove(ix);
    self.rows.remove(&v);}}



fn plan_regroup(vids:&Vec<VID>, groups:&Vec<HashSet<VID>>)->HashMap<VID,usize> {
  // vids are arranged from bottom to top
  let mut plan = HashMap::new();

  // if only one group, there's nothing to do:
  if groups.len() == 1 && groups[0].len() == vids.len() { return plan }

  // TODO: check for complete partition (set(vids)==set(U/groups)
  let mut sum = 0; for x in groups.iter() { sum+= x.len() }
  assert_eq!(vids.len(), sum, "vids and groups had different total size");

  // map each variable to its group number:
  let mut dest:HashMap<VID,usize> = HashMap::new();
  for (i, g) in groups.iter().enumerate() {
    for &v in g { dest.insert(v, i); }}

  // start position of each group:
  let mut start:Vec<usize> = groups.iter().scan(0, |a,x| {
    *a+=x.len(); Some(*a)}).collect();
  start.insert(0, 0);
  start.pop();

  // downward-moving cursor for each group (starts at last position)
  let mut curs:Vec<usize> = groups.iter().scan(0, |a,x|{
    *a+=x.len(); Some(*a)}).collect();

  let mut saw_misplaced = false;
  for (i,v) in vids.iter().enumerate().rev() {
    let g = dest[v]; // which group does it go to?
    // we never schedule a move for group 0. others just move past them.
    if g == 0 { if i>=start[1] { saw_misplaced = true }}
    // once we see a misplaced item, we have to track everything below it, so that
    // items that start in place *stay* in place as the swaps happen.
    else {
      curs[g]-=1;
      if saw_misplaced || i<start[g] {
        plan.insert(*v, curs[g]);
        saw_misplaced=true }}
    //println!("i: {} v: {} g:{}, saw_misplaced: {}, curs:{:?}, plan:{:?}" , i, v, g, saw_misplaced, curs, plan);
  }
  plan}

// functions for performing the distributed regroup()
impl XVHLScaffold {

  fn plan_regroup(&self, groups:&Vec<HashSet<VID>>)->HashMap<VID,usize> {
    //println!("self.vids: {:?}", self.vids);
    //println!("groups: {:?}", groups); // , groups.clone().iter().rev().collect::<Vec<_>>()
    let plan = plan_regroup(&self.vids, groups);
    //println!("plan: {:?}", plan);
    plan }

  fn take_row(&mut self, v:&VID)->Option<XVHLRow> {
    if self.locked.contains(v) { None }
    else { self.locked.insert(*v); self.rows.remove(v) }}

  fn next_regroup_task(&mut self, plan:&HashMap<VID,usize>)->(VID, Vec<Q>) {
    let mut res = vec![];
    // find a variable to move that isn't locked yet:
    for &vu in self.vids.iter().rev() {
      if self.locked.contains(&vu) { continue }
      if let Some(&dst) = plan.get(&vu) {
        if self.vix(vu).unwrap() == dst { continue }
        // we lock all the moving variables so they never cross each other
        if let Some(ru) = self.take_row(&vu) {
          res.push(Q::Init{vu, ru});
          let vd = self.vid_above(vu).unwrap();
          // schedule swap immediately if we can. (otherwise regroup() sets an alarm)
          if plan.contains_key(&vd) {
          /*println!("\x1b[33mWARNING: DEFERRING task for {} because row above ({}) is in the plan.\x1b[0m",vu, vd);*/ }
          else if let Some(rd) = self.take_row(&vd) { res.push(Q::Step{vd, rd}) }
          else { panic!("WHYY?") }
          return (vu, res) }
        else { // we couldn't take row u. it's probably being swapped
          let other = &self.vid_below(vu).unwrap();
          assert!(plan.contains_key(other), "couldn't take_row {} but vid_below is {}", vu, other);
          panic!("COULDN't TAKE ROW U ({}), BUT DON'T KNOW WHY", vu) }}}
      panic!("SPAWNED A THREAD WITH NOTHING TO DO")}

  /// arrange row order to match the given groups.
  /// the groups are given in bottom-up order (so groups[0] is on bottom), and should
  /// completely partition the scaffold vids.
  fn regroup(&mut self, groups:Vec<HashSet<VID>>) {
    assert!(self.locked.is_empty());
    self.complete = HashMap::new();
    self.drcd = HashMap::new();
    self.validate("before regroup()");
    // (var, ix) pairs, where plan is to lift var to row ix
    let plan = self.plan_regroup(&groups);
    if plan.len() == 0 { return }
    let mut swarm: Swarm<Q,R,SwapWorker> = Swarm::new(plan.len());
    let mut alarm: HashMap<VID,WID> = HashMap::new();
    let _:Option<()> = swarm.run(|wid,qid,r|->SwarmCmd<Q,()> {
      match qid {
        QID::INIT => { // assign next task to the worker
          let (vu, mut work) =  self.next_regroup_task(&plan);
          if vu == NOV { SwarmCmd::Pass }
          else { match work.len() {
            1 => { alarm.insert(self.vid_above(vu).unwrap(), wid); SwarmCmd::Send(work.pop().unwrap()) },
            2 => SwarmCmd::Batch(work.into_iter().map(move |q| (wid, q)).collect()),
            // TODO: assign extra workers to swaps with more nodes?
            // this also happens when we spawn a new thread to work on a formerly completed vid that got displaced
            _ => SwarmCmd::Pass }}}, // we have more threads than variables to swap.
        QID::STEP(_) => {
          if let None = r { return SwarmCmd::Pass } // TODO: this wasn't supposed to happen, but then Batch[Init]
          match r.unwrap() {

            R::DRcD{vu} => {
              SwarmCmd::Send(Q::DRcD(self.drcd.remove(&vu).unwrap_or_else(|| HashMap::new()))) },

            // recycle or allocate xids:
            R::Alloc{needed}  => {
              SwarmCmd::Send(Q::Xids(self.alloc(needed))) },

            // complete one swap in the move:
            R::PutRD{vu, vd, rd, dnew, umov, dels, refs} => {
              self.swarm_put_rd(&plan, &mut alarm, wid, vu, vd, rd, dnew, umov, dels, refs) },

            // finish the move for this vid
            R::PutRU{vu, ru} => {
              debug_assert!(plan.contains_key(&vu), "got back vu:{:?} that wasn't in the plan", vu);
              debug_assert!(self.locked.contains(&vu), "vu:{} wasn't locked!", vu);
              self.locked.remove(&vu);
              self.rows.insert(vu, ru);
              self.apply_drcd(&vu);
              self.complete.insert(vu, wid);

              if self.complete.len() == plan.len() {
                debug_assert!(alarm.is_empty(), "last worker died but we still have alarms: {:?}", alarm);
                SwarmCmd::Return(()) }
              else { SwarmCmd::Pass }}}},

        QID::DONE => { SwarmCmd::Pass }}});

        let plan2 = self.plan_regroup(&groups);
        debug_assert!(plan2.is_empty(), "regroup failed to make these moves: {:?}", plan2);
        debug_assert!(self.locked.is_empty());
        self.validate("after regroup()"); }


  // like add_ref_ix but defers if row is locked.
  fn add_ref_ix_or_defer(&mut self, xid:XID, drc:i64) {
    if drc != 0 {
      let v = self.vhls[xid.ix()].v;
      if self.locked.contains(&v) {
        //println!("row {} was locked so deferring xid:{:?} drc:{} ({:?})", v, xid.raw(), drc, self.vhls[xid.ix()]);
        *self.drcd.entry(v).or_default().entry(xid.raw()).or_default()+=drc; }
      else { self.add_iref_ix(xid, drc); }}}

  /// apply deferred refcount delta (call whenever a row gets unlocked)
  fn apply_drcd(&mut self, v:&VID) {
    if let Some(drcd) = self.drcd.remove(&v) {
      // if xvhl.v changed again (due to umov), we may need to defer again (since new row may be locked)
      for (&xid, &drc) in drcd.iter() { self.add_ref_ix_or_defer(xid, drc)} }}

  /// called whenever a worker returns a downward-moving row to the scaffold
  fn swarm_put_rd(&mut self, plan:&HashMap<VID,usize>, alarm:&mut HashMap<VID,WID>,
    wid:WID, vu:VID, vd:VID, rd:XVHLRow, dnew:Vec<Mod>, umov:Vec<Mod>, dels:Vec<XID>, refs:HashMap<XID,i64>
  )->SwarmCmd<Q,()> {
    // replace and unlock the downward-moving row:
    debug_assert_eq!(vd, self.vid_above(vu).unwrap(), "row d isn't the row that was above row u!?!");
    self.rows.insert(vd, rd);

    // apply modifications to the vhls table
    // TODO: probably we we should just have self.ixv : HashMap<ix,VID>  instead of vhls,
    // and then a self.nids : std::collections::BinaryHeap for reclaimed indices.
    // println!("vu:{} vd:{} dnew: {:?} umov:{:?} dels:{:?}", vu, vd, dnew, umov, dels);
    self.reclaim_swapped_nodes(dels);
    for (ix, hi, lo) in dnew {
      debug_assert!(hi.is_const() || self.vhls[hi.ix()] != XVHL_O, "garbage hi link in dnew: {:?}->{:?}", ix, hi);
      debug_assert!(lo.is_const() || self.vhls[lo.ix()] != XVHL_O, "garbage lo link in dnew: {:?}->{:?}", ix, lo);
      self.vhls[ix] = XVHL{ v: vd, hi, lo } }
    for (ix, hi, lo) in umov {
      debug_assert!(hi.is_const() || self.vhls[hi.ix()] != XVHL_O, "garbage hi link in umov: {:?}->{:?}", ix, hi);
      debug_assert!(lo.is_const() || self.vhls[lo.ix()] != XVHL_O, "garbage lo link in umov: {:?}->{:?}", ix, lo);
      self.vhls[ix] = XVHL{ v: vu, hi, lo } }
    // println!("ref changes: {:?}", refs);
    // for (xid, drc) in refs.iter() { println!("drc: {} for xid:{:?} ({:?})", *drc, *xid, self.vhls[xid.ix()] ); }
    for (xid, drc) in refs.iter() { self.add_ref_ix_or_defer(*xid, *drc) }

    debug_assert!(self.locked.contains(&vd), "vd:{} wasn't locked!??", vd);
    self.locked.remove(&vd);

    self.apply_drcd(&vd);

    // swap the two entries in .vids
    let old_uix = self.vix(vu).unwrap();
    let new_uix = old_uix + 1;
    self.vids.swap(old_uix, new_uix);

    println!("\x1b[36mswapped vu:{} -> vd:{} => {:?}\x1b[0m", vu, vd, self.vids);
    //self.validate(format!("after swapping vd:{:?} with vu:{:?}", vd, vu).as_str());

    let mut work:Vec<(WID, Q)> = vec![];

    // tell anyone waiting on rd that they can resume work
    debug_assert!(!alarm.contains_key(&vd), "alarm should never be placed on a downward-moving row.");
    if let Some(w2) = alarm.remove(&vu) {
      // println!("\x1b[35mTRIGGERED ALARM ON vu:{}, sending vd:{}\x1b[0m", vu, vd);
      // wake the sleeping worker right behind us:
      let rd = self.take_row(&vd).unwrap();
      work.push((w2, Q::Step{vd, rd})); }

    // vids within the same group will never swap with each other, but vids from different groups may.
    // if vu just moved into vd's planned spot, it means vd's move was already complete, and we just displaced it.
    // However, its worker is already dead (so we need a new one), and the row above is locked until we finish
    // the next move for vu (so we set an alarm rather than spawning a new thread)
    else if plan.contains_key(&vd) && self.complete.contains_key(&vd) {
      // println!("RE-SPAWNING WORKER FOR DISPLACED VID: {}", vd);
      let w = self.complete.remove(&vd).unwrap();
      work.push((w, Q::Init{ vu:vd, ru: self.take_row(&vd).unwrap() }));
      // the alarm goes on the upward-moving row
      alarm.insert(vu, w); }

    // are we there yet? :)
    if new_uix == plan[&vu] { work.push((wid, Q::Stop)); }
    else { // start or schedule the next swap
      let vd = self.vid_above(vu).unwrap();
      if let Some(rd) = self.take_row(&vd) { work.push((wid, Q::Step{vd, rd})); }
      else { alarm.insert(vd, wid); }}

    SwarmCmd::Batch(work) }}


// -- message types for swarm -------------------------------------------

type Mod = (usize,XID,XID);

#[derive(Debug)]
enum Q {
  Init{ vu:VID, ru: XVHLRow },
  Step{ vd:VID, rd: XVHLRow },
  Stop,
  DRcD( HashMap<XID,i64> ),
  Xids( Vec<XID> )}

#[derive(Debug)]
enum R {
  DRcD{ vu:VID },
  Alloc{ needed:usize },
  PutRD{ vu:VID, vd:VID, rd: XVHLRow, dnew:Vec<Mod>, umov:Vec<Mod>, dels:Vec<XID>, refs:HashMap<XID, i64> },
  PutRU{ vu:VID, ru: XVHLRow }}

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
        for ixrc in row.hm.values() { write!(wr, " \"{:?}\"", ixrc.ix).unwrap() }
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

// 0: swap the rows. (lift row u above row d)
//    u was independent before, so we leave it alone except for gc.
//    (but we might wind up using it later, so we do the gc step last.)
// 1: for each node n in row d:
//    - if n.rc=0, delete from hashmap and yield    | Delete(n.nid)
//    - if either leg points to row u:
//        decref the old node(s) on row u
//        add new node(s) on w with rc=1            | Create()   { or incref if duplicates? }
//          incref the hi/lo nodes.
//        move n to row u, copying n.rc, and yield  | Update(n.nid, v,h,l)
//    - else, leave n alone.
// 2: for n in row u:
//    if n.rc==0, Del(n.nid) and DecRef(n.hi, n.lo)


#[derive(PartialEq, Debug)]
enum ROW { U, D }

struct SwapWorker {
  /// the upward-moving variable (only used for tracing)
  vu:VID,
  /// the downward-moving variable (only used for tracing)
  vd:VID,
  /// row u is the row that moves upward.
  ru:XVHLRow,
  /// row d is the row that moves downward.
  rd:XVHLRow,
  /// external reference counts to change
  refs: HashMap<XID, i64>,
  /// track any nodes we've deleted (so scaffold can recycle them)
  dels: Vec<XID>,
  /// xids we've recycled ourselves
  mods: Vec<XID>,
  // reverse map for row u (so we can see if a branch from d points to row u)
  ru_map:HashMap<XID,XHiLo>,
  // reverse map for row d (to detect when we need a new ref from a umov to an existing node on row d)
  rd_map:HashMap<XID,XHiLo>,
  /// wip for nodes moving to row u.
  umov: Vec<(IxRc,XWIP1,XWIP1)>,
  // next (fake/tmp) xid to assign
  next:i64,
  /// new parent nodes to create on row d
  dnew: HashMap<(XID,XID), IxRc> }

impl Default for SwapWorker {
  fn default()->SwapWorker {
    SwapWorker{ next:0,
      vu:VID::nov(), ru:XVHLRow::new(), ru_map:HashMap::new(),
      vd:VID::nov(), rd:XVHLRow::new(), rd_map:HashMap::new(),
      refs:HashMap::new(), dels:vec![], mods:vec![], umov:vec![], dnew:HashMap::new() }}}

impl Worker<Q,R> for SwapWorker {
  fn work_step(&mut self, _qid:&QID, q:Q)->Option<R> {
    match q {
      Q::Init{vu, ru} => {
        self.set_ru(vu, ru);
        None },
      Q::Step{vd, rd} => {
        self.ru_map = self.ru.xid_map();
        self.reset_state().set_rd(vd, rd).find_movers();
        Some(R::DRcD{ vu:self.vu }) }
      Q::DRcD(rcds) => {
        for (xid, drc) in rcds {
          let hl = self.ru_map[&xid];
          self.ru.hm.get_mut(&hl).unwrap().add(drc); }
        Some(R::Alloc{needed: self.recycle()})},
      Q::Xids(xids) => {
        // println!("vu:{} vd:{} xids: {:?}", self.vu, self.vd, xids);
        let (dnew, wipxid) = self.dnew_mods(xids);
        let umov = self.umov_mods(wipxid);
        // now return the newly swapped row:
        let rd = std::mem::replace(&mut self.rd, XVHLRow::new());
        let refs = std::mem::replace(&mut self.refs, HashMap::new());
        let dels = std::mem::replace(&mut self.dels, vec![]);
        Some(R::PutRD{ vu:self.vu, vd:self.vd, rd, dnew, umov, dels, refs })},
      Q::Stop => {
        let ru = std::mem::replace(&mut self.ru, XVHLRow::new());
        Some(R::PutRU{ vu:self.vu, ru }) }}}}

impl SwapWorker {

  fn reset_state(&mut self)->&mut Self {
    // self.refs = HashMap::new();
    // self.dels = vec![]; - don't replace this because we call gc(row::U) in set_ru (before we call this).
    self.umov = vec![];
    self.dnew = HashMap::new();
    self.next = 0;
    self }

  /// set .rd and rebuild .rd_map. We garbage collect row d immediately so that we don't
  /// add things to umov that don't need to be there. (otherwise, we'd delete them in the
  /// recycle step but then add them back even though they're not referenced.)
  fn set_rd(&mut self, vd:VID, rd:XVHLRow)->&mut Self {
    self.vd = vd; self.rd_map = rd.xid_map(); self.rd = rd; self.gc(ROW::D); self }

  /// set .ru and rebuild .ru_map. We don't garbage collect row U because ... why?
  fn set_ru(&mut self, vu:VID, ru:XVHLRow)->&mut Self {
    self.vu = vu; self.ru_map = ru.xid_map(); self.ru = ru; self.gc(ROW::U); self }

  /// garbage collect nodes on one of the rows:
  fn gc(&mut self, which:ROW) {
    let mut dels = vec![];
    let mut refs: HashMap::<XID, i64> = HashMap::new();
    let row = match which { ROW::U => &mut self.ru, ROW::D => &mut self.rd };
    row.hm.retain(|hl, ixrc| {
      if ixrc.rc() == 0 {
        *refs.entry(hl.hi.raw()).or_insert(0)-=1;
        *refs.entry(hl.lo.raw()).or_insert(0)-=1;
        dels.push(ixrc.ix);
        false }
      else { true }});
    match which { ROW::U => self.ru_map = self.ru.xid_map(), ROW::D => self.rd_map = self.rd.xid_map() }
    self.dels.extend(dels);
    for (x, dc) in refs { self.xref(x, dc); }}

  /// record a refcount change to an external node
  fn xref(&mut self, x:XID, dc:i64)->XID {
    if x.is_const() { x }
    else { if let Some(key) = self.ru_map.get(&x.raw()) { self.ru.hm.get_mut(key).unwrap().add(dc); }
      else if let Some(key) = self.rd_map.get(&x.raw()) { self.rd.hm.get_mut(key).unwrap().add(dc); }
      else { *self.refs.entry(x.raw()).or_insert(0)+=dc }; x }}

  /// generate a new (wip) xid to use internally
  fn new_xid(&mut self)->XID { let xid = XID {x:self.next}; self.next+=1; xid }

  /// given the rows from swap(), find all the nodes from row d that need
  /// to move to row u. (that is, rows that have a reference to row u).
  /// rv is mutable here because we will decrease the refcount as we find
  /// each reference, and rw is mutable because we may *increase* the refcount.
  fn gather_umovs(&mut self)->Vec<(XHiLo, XWIP0, XWIP0)> {
    // moving a node from row d-> row u modifies the old node in place, so no new xid is used.
    // (we know it's not already on row u because before the lift, row u could not refer to row d)
    // at least one of the node's children will be replaced by a new node on row d. proof:
    //     general pattern of 3rd level rewrite is   abcd ->  acbd
    //     we can consolidate one side of the swap: abac -> aabc (so we get v?a:w?b:c)
    //     but we can't consolidate both:  abab -> abba, because abab can't appear in a bdd.
    //     no other pattern would result in consolidating both sides. (qed)
    // therefore: worst case for growth is every node in row d moves to row u and creates 2 new children.
    // a block of 2*(row d).len ids for this algorithm to assign.
    // so: in the future, if we want to create new nodes with unique ids in a distributed way,
    // we should allocate 2*(row d).len ids for this function to assign to the new nodes.
    // reference counts elsewhere in the graph can change (!!! really? they don't change in this step.),
    // but never drop to 0. if they did, then swapping the rows back would have to create new nodes elsewhere.
    let mut umovs: Vec<(XHiLo,XWIP0,XWIP0)> = vec![];
    for dhl in self.rd.hm.clone().keys() {
      // fetch the hi,lo branches, but only when they point to row u
      let uget = |xid:XID|->Option<XHiLo> {
        self.ru_map.get(&xid.raw()).cloned().map(|hl|
          if xid.is_inv() { !hl } else { hl })};
      let (hi, lo) = dhl.as_tup();
      let (uhi, ulo) = (uget(hi), uget(lo));
      // if neither points to row u, this node is independent, and there's nothing to do
      if let (None, None) = (uhi, ulo) {}
      else {  // otherwise we move the node to row u and build at least 1 new child on row d
        let (ii, io) = if let Some(x) = uhi {(x.hi, x.lo)} else {(hi, hi)};
        let (oi, oo) = if let Some(x) = ulo {(x.hi, x.lo)} else {(lo, lo)};
        // remove both refs for now, even though we may add one right back:
        self.xref(hi, -1); self.xref(lo, -1);
        umovs.push((*dhl, self.new_ref(ii,oi), self.new_ref(io,oo))); }}
    umovs }

  /// Construct new child nodes on row d, or add new references to external nodes.
  /// Converts the XWIP0::HL entries to XWIP1::NEW.
  fn find_movers(&mut self) {
    // collect the list of nodes on row d that reference row u, and thus have to be moved to row u.
    for (whl, wip_hi, wip_lo) in self.gather_umovs() {
      let hi = self.resolve(wip_hi);
      let lo = self.resolve(wip_lo);
      // the lo branch should never be inverted, since the lo-lo path doesn't change in a swap,
      // and lo branches are always raw in the scaffold.
      // This means we only have to deal with inverted xids the newly-created hi branches.
      if let XWIP1::NEW(x) = lo { assert!(x >= 0, "unexpected !lo branch");}
      // delete the old node from row d. the newly created nodes don't depend on vid u, and
      // the node to delete does depend on vid u, so there's never a conflict here.
      let ixrc = self.rd.hm.remove(&whl).unwrap();
      // we can't add directly to row u until we resolve the XWIP1::NEW entries,
      // but we can make a list of the work to be done:
      self.umov.push((ixrc, hi, lo)); }}

  fn resolve(&mut self, xw0:XWIP0)->XWIP1 {
    match xw0 {
      // the new_ref() function would have marked it as a XID if it were already in row d.
      XWIP0::XID(x) => { XWIP1::XID(self.xref(x,1)) },
      XWIP0::HL(hi0,lo0) => {
        // these are the new children on the w level, so we are creating a new node.
        // but: it's possible that multiple new nodes point to the same place.
        // this pass ensures that all duplicates resolve to the same place.
        // TODO: this isn't really an IxRc since the xid is virtual
        let (hi,lo,inv) = if lo0.is_inv() { (!hi0, !lo0, true) } else { (hi0,lo0,false) };
        let ir = {
          if let Some(mut ixrc) = self.dnew.remove(&(hi,lo)) {
            // Entry::Occupied - we were already going to create this node.
            ixrc.irc += 1; ixrc }
          else { // Entry::Vacant, so build a new node with one reference to it.
            self.xref(hi, 1); self.xref(lo,1);
            IxRc { ix: self.new_xid(), irc: 1, erc: 0 }}};
        self.dnew.insert((hi,lo), ir);
        XWIP1::NEW(if inv { !ir.ix.x } else { ir.ix.x }) }}}

  /// remove garbage from row u. these won't conflict with .unew because we will never
  /// add a *completely* new node on row u - only move existing nodes from row d, and
  /// these will never match existing nodes on row u because at least one leg always
  /// points at var d (and this wasn't possible before the lift). But we may need to delete
  /// nodes because the rc dropped to 0 (when the node was only referenced by row d).
  fn recycle(&mut self)->usize {
    // garbage collect row d FIRST in case it contains the only references to a node on row u
    self.gc(ROW::D);
    self.gc(ROW::U);

    // remove any ref changes to nodes we've deleted
    for xid in &self.dels { self.refs.remove(&xid.raw()); }
    let mut dels = self.dels.clone();
    let mut needed = 0; // in case there are more new nodes than old trash

    // mods reclaims xids from dels that can be recycled
    self.mods = {
      let have = dels.len();
      let need = self.dnew.len();
      if need <= have {
        let tmp = dels.split_off(need);
        let res = dels; dels = tmp;
        res }
      else {
        let res = dels; dels = vec![];
        needed = need-have;
        res }};
    self.dels = dels;
    needed }

  /// add newly created child nodes on row d, and
  /// return the list of changes to make to the master scaffold,
  /// and a vector mapping the wip ix to the final xid
  fn dnew_mods(&mut self, alloc:Vec<XID>)->(Vec<(usize, XID, XID)>, Vec<XID>) {
    self.mods.extend(alloc);
    let xids = std::mem::replace(&mut self.mods, vec![]);
    assert_eq!(xids.len(), self.dnew.len());
    let mut res = vec![];
    let mut wipxid = vec![XID_O; self.dnew.len()];
    for ((hi,lo), ixrc0) in self.dnew.iter() {
      let mut ixrc = *ixrc0; // clone so we maintain the refcount
      debug_assert!(ixrc.irc > 0);
      let inv = ixrc0.ix.x < 0; assert!(!inv);
      let wipix = ixrc0.ix.x as usize;
      ixrc.ix = xids[wipix];  // map the temp xid -> true xid
      wipxid[wipix] = ixrc.ix; // remember for w2x, below.
      assert!(self.rd.hm.get(&(XHiLo{hi:*hi, lo:*lo})).is_none());
      let key = XHiLo{hi:*hi, lo:*lo};
      self.rd.hm.insert(key, ixrc);  // refcount chages are done so no need for rd_map
      // and now update the master store:
      debug_assert_ne!(hi, lo, "hi=lo when committing wnew");
      res.push((ixrc.ix.x as usize, *hi, *lo)); }
    (res, wipxid)}

  /// move the dependent nodes from row d to row u, and
  /// return the list of changes to make to the master scaffold.
  /// wipxid argument is the mapping returned by dnew_mods
  fn umov_mods(&mut self, wipxid:Vec<XID>)->Vec<(usize, XID, XID)> {
    let mut res = vec![];
    let w2x = |wip:&XWIP1| {
      match wip {
        XWIP1::XID(x) => *x,
        XWIP1::NEW(x) => { if *x<0 { !wipxid[!*x as usize]  } else { wipxid[*x as usize ]}}}};
    for (ixrc, wip_hi, wip_lo) in self.umov.iter() {
      let (hi, lo) = (w2x(wip_hi), w2x(wip_lo));
      let key = XHiLo{hi, lo};
      self.ru.hm.insert(key, *ixrc);
      self.ru_map.insert(ixrc.ix, key); // probably redundant.
      res.push((ixrc.ix.ix(), hi, lo)); }
    res}

  /// reference a node on/below row d, or create a node on row d
  fn new_ref(&mut self, h:XID, l:XID)->XWIP0 {
    let (hi, lo, inv) = if l.is_inv() {(!h, !l, true)} else {(h, l, false)};
    // hi == lo only when the match statement passes hi,hi or lo,lo.
    // previously, this triggered a decref, but that was incorrect:
    // we are only ever adding references at this step, and when hi==lo,
    // we simply wind up adding 1 reference instead of 2. (The old nodes on row u
    // might have other parents, so the external nodes can only *gain* references.)
    // (Of course, it can't be the case that swapping twice creates references since
    // this should be a no-op. The "extra" references created by the first swap are
    // balanced we garbage collect extraneous row u nodes on the second swap and
    // decref their children.)
    if hi == lo { return XWIP0::XID(if inv { !lo } else { lo }); }
    if let Some(ixrc) = self.rd.hm.get(&XHiLo{ hi, lo}) {
      XWIP0::XID(if inv {!ixrc.ix} else {ixrc.ix}) }
    else if inv { XWIP0::HL(!hi, !lo) } else { XWIP0::HL(hi, lo) }}}

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
    let v = VID::var(i as u32); self.xs.push(v); self.xs.add_ref(XVHL{v, hi:XID_I, lo:XID_O}, 0, 1);
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
          if let Some(&v) = self.cv.get(&c) { self.ds.push(self.xs.add_ref(XVHL{v,hi:XID_I,lo:XID_O}, 0, 1)) }
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
      let res = self.xs.add_ref(XVHL{v:xvhl.v, hi, lo}, 0, 1); self.ds.push(res); res }
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
  pub fn new() -> Self {
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
    self.dst.regroup(vec![d, v, n]);

    // the order of n has to match in both. we'll use the
    // existing order of n from dst because it's probably bigger.
    let vix = self.dst.vix(self.rv).unwrap();
    let mut sg = vec![s.clone()];
    for ni in (vix+1)..self.dst.vids.len() { sg.push(set(vec![self.dst.vids[ni]])) }
    println!("regrouping src. vids: {:?} groups: {:?}", self.src.vids, sg);
    self.src.regroup(sg); // final order: [s,n]

    // now whatever order the s group wound up in, we can insert
    // them in the dst directly *above* v. final order: [ d,v,s,n ]
    for &si in self.src.vids.iter().rev() {
      if s.contains(&si) {
        self.dst.rows.insert(si, XVHLRow::new());
        self.dst.vids.insert(vix+1, si) }}

    // return the row index at the bottom of set s
    vix}

  /// Replace rv with src(sx) in dst(dx)
  fn sub(&mut self)->XID {

    let rvix = self.dst.vix(self.rv);
    if rvix.is_none() { return self.dx } // rv isn't in the scaffold, so do nothing.
    if self.dx == XID_O { panic!("dx is XID_O. this should never happen.")}
    let vhl = self.dst.get(self.dx).unwrap();
    if vhl.v == VID::nov() { panic!("node dx:{:?} appears to have been garbage collected!?!", self.dx)}
    let vvix = self.dst.vix(vhl.v);
    if vvix.is_none() { panic!("got vhl:{:?} for self.dx:{:?} but {:?} is not in dst!?", vhl, self.dx, vhl.v); }

    // add external refs so our root nodes don't get collected
    self.dst.add_eref_ix(self.dx, 1);
    self.src.add_eref_ix(self.sx, 1);

    // 1. permute vars.
    let vix = self.arrange_vids();
    self.dst.validate("before removing top rows");

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
    //    It's a table in the sense that it's a fully expanded row in a binary tree,
    //    rather than a compressed bdd.
    let mut p: Vec<XID> = self.dst.tbl(self.dx, Some(self.rv));
    self.dst.validate("after calling tbl");

    // !! tbl() branches from the top var in dx, not the top var in the scaffold.
    //    src may contain vars above branch(dx), so p=tbl(dx) may be smaller than q=tbl(sx).
    //    So: Scale p to the size of q by repeatedly doubling the entries.
    // !! yes, this is a wasteful algorithm but the expectation is that p
    //    and q are quite small: < 2^n items where n = number of vars in
    //    the replacement. I expect n<16, since if n is too much higher than
    //    that, I expect this whole algorithm to break down anyway.

    if p.len() < q.len() { p = p.iter().cycle().take(q.len()).cloned().collect(); }

    // 4. let r = the partial truth table for result at row rv.
    let r:Vec<XID> = p.iter().zip(q.iter()).map(|(&pi,&qi)|
      if self.dst.branch_var(pi) == self.rv { self.dst.follow(pi, qi) }
      else { pi }).collect();

    // 5. clear all rows above v in the scaffold, and then delete v
    self.dst.clear_top_rows(self.rv);
    self.dst.remove_empty_row(self.rv);
    self.dst.validate("after removing top rows");

    // 6. rebuild the rows above set d, and return new top node
    let bv = self.dst.vids[vix]; // whatever the new branch var in that slot is
    self.dx = self.dst.untbl(r, Some(bv));
    self.dst.validate("after substitution");

    // 7. return result
    // self.dst.add_eref_ix(self.dx, -1); (except it's already 0 because of the beheading)
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
    self.dx = self.dst.add_ref(XVHL{ v, hi:XID_I, lo:XID_O}, 0, 1);
    self.dx.to_nid() }

  fn subst(&mut self, ctx: NID, v: VID, ops: &Ops)->NID {
    let Ops::RPN(mut rpn) = ops.norm();
    println!("@:sub {:>4} -> {:>24} -> {:>20}",
      format!("{:?}",v), format!("{:?}", ops), format!("{:?}", rpn));

    let f = rpn.pop().unwrap(); // guaranteed by norm() to be a fun-nid

    // so now, src.vids is just the raw input variables (probably virtual ones).
    self.src = XVHLScaffold::new();
    for nid in rpn.iter() { assert!(nid.is_vid()); self.src.push(nid.vid()); }

    // untbl the function to give us the full BDD of our substitution.
    let tbl = fun_tbl(f);
    self.sx = self.src.untbl(tbl, None);

    // everything's ready now, so just do it!
    self.dx = XID::from_nid(ctx);
    self.rv = v;
    let r  = self.sub().to_nid();

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
    let mut bdd = crate::bdd::BDDBase::new();
    for (i,rv) in self.dst.vids.iter().enumerate() {
      let bv = NID::from_vid(VID::var(i as u32));
      for (x, ixrc) in self.dst.rows[rv].hm.iter() {
        if ixrc.rc() > 0 || *rv == self.dst.top_vid().unwrap() {
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
    for reg in bdd.solutions_pad(nctx, nvars) { res.insert(reg.permute_bits(&pv)); }
    res}

  fn status(&self) -> String { "".to_string() } // TODO
  fn dump(&self, _path: &Path, _note: &str, _step: usize, _old: NID, _vid: VID, _ops: &Ops, _new: NID) {
    self.dst.save_dot(_new, format!("xvhl-{:04}.dot", _step).as_str());
  }
}

include!("test-swap.rs");
include!("test-swap-scaffold.rs");
