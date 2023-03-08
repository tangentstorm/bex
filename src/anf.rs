//! Structure for representing algebraic normal form (xor of ands).
//!
//! Algebraic normal form is essentially a boolean polynomial.
//! Each term is a product of non-negated inputs. To normalize,
//! We assume the inputs are sorted by number, and then we factor
//! out common prefixes.
//!
//! Thus:
//! ```text
//!     abcd + dc + abc + abd    (input)
//!     abc + abcd + abd + cd    (after sorting)
//!     ab(c(1+d) + d) + cd      (after factoring)
//! ```
//! In addition, identical suffixes after factoring always refer to the same node.
use std::collections::HashSet;
use base::{Base};
use simp;
use {nid, nid::{NID,I,O}};
use vid::{VID,VidOrdering};
use cur::{Cursor, CursorPlan};
use reg::Reg;
use vhl::{VHL, HiLo, HiLoBase, Walkable};
use hashbrown::HashMap;
use bdd::{BDDBase}; // for solutions
#[cfg(test)] use vid::{topmost, botmost};

/// (v AND hi) XOR lo
// TODO /// (ALL(v0..v1) AND hi) XOR lo
// TODO: /// The v0..v1 thing is used to collapse long chains of nodes where lo=O.
pub struct ANFBase {
  nodes:Vec<VHL>,
  cache:HashMap<VHL,NID>,
  tags:HashMap<String,NID>}


impl Walkable for ANFBase {
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>, topdown: bool)
  where F: FnMut(NID,VID,NID,NID) {
    if !seen.contains(&n) {
      seen.insert(n); let VHL{ v, hi, lo, } = self.fetch(n);
      if topdown { f(n,v,hi,lo) }
      if !hi.is_const() { self.step(hi, f, seen, topdown) }
      if !lo.is_const() { self.step(lo, f, seen, topdown) }
      if !topdown { f(n,v,hi,lo) }}}}


impl Base for ANFBase {

  fn new()->Self { ANFBase { nodes:vec![], cache: HashMap::new(), tags:HashMap::new() }}

  fn dot(&self, n:NID, wr: &mut dyn std::fmt::Write) {
    macro_rules! w {
      ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}
    w!("digraph anf {{");
    w!("subgraph head {{ h1[shape=plaintext; label=\"ANF\"] }}");
    w!("  I[label=⊤; shape=square];");
    w!("  O[label=⊥; shape=square];");
    w!("{{rank = same; I; O;}}");
    w!("node[shape=circle];");
    self.walk(n, &mut |n,_,_h,_l| w!("  \"{}\"[label=\"{:?}\"];", n, n.vid()));
    w!("edge[style=solid];");
    self.walk(n, &mut |n,_,hi,_l| w!("  \"{:?}\"->\"{:?}\";", n, hi));
    w!("edge[style=dashed];");
    self.walk(n, &mut |n,_,__,lo| w!("  \"{:?}\"->\"{:?}\";", n, lo));
    w!("}}"); }

  fn def(&mut self, _s:String, _v:VID)->NID { todo!("anf::def"); }
  // TODO: tag and get are copied verbatim from bdd
  fn tag(&mut self, n:NID, s:String)->NID { self.tags.insert(s, n); n }
  fn get(&self, s:&str)->Option<NID> { Some(*self.tags.get(s)?) }

  fn when_lo(&mut self, v:VID, n:NID)->NID {
    let nv = n.vid();
    match v.cmp_depth(&nv) {
      VidOrdering::Above => n, // n independent of v
      VidOrdering::Level => self.fetch(n).lo,
      VidOrdering::Below => {
        let VHL{ v:_, hi, lo } = self.fetch(n.raw());
        let hi1 = self.when_lo(v, hi);
        let lo1 = self.when_lo(v, lo);
        let res = self.vhl(nv, hi1, lo1);
        if n.is_inv() == res.is_inv() { res } else { !res }}}}

  fn when_hi(&mut self, v:VID, n:NID)->NID {
    let nv = n.vid();
    match v.cmp_depth(&nv) {
      VidOrdering::Above => n,  // n independent of v
      VidOrdering::Level => self.fetch(n).hi,
      VidOrdering::Below => {
        let VHL{ v:_, hi, lo } = self.fetch(n.raw());
        let hi1 = self.when_hi(v, hi);
        let lo1 = self.when_hi(v, lo);
        let res = self.vhl(nv, hi1, lo1);
        if n.is_inv() == res.is_inv() { res } else { !res }}}}

  // logical ops

  fn and(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::and(x,y) { nid }
    // We want any 'xor 1' (not) to be kept at the top level. There are four cases:
    else {
      let (a,b) = (x.raw(), y.raw());  // x!=I because it was handled above.
      if x.is_inv() {
        // case 0:  x:~a & y:~b ==> 1 ^ a ^ ab ^ b
        if y.is_inv() { expr![ self,  (I ^ (a ^ ((a & b) ^ b)))] }
        // case 1:  x:~a & y:b ==>  ab ^ b
        else { expr![ self, ((a & b) ^ b)] }}
      // case 2: x:a & y:~b ==> ab ^ a
      else if y.is_inv() { expr![ self, ((a & b) ^ a)] }
      // case 3: x:a & y:b ==> ab
      else { self.calc_and(x, y) }}}

  fn xor(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::xor(x,y) { nid }
    else {
      // xor the raw anf expressions (without any 'xor 1' bits), then xor the bits.
      let (a, b) = (x.raw(), y.raw());
      let res = self.calc_xor(a, b);
      if x.is_inv() == y.is_inv() { res } else { !res }}}

  fn or(&mut self, x:NID, y:NID)->NID {
    if let Some(nid) = simp::or(x,y) { nid }
    else { expr![self, ((x & y) ^ (x ^ y))] }}

  fn sub(&mut self, v:VID, n:NID, ctx:NID)->NID {
    let cv = ctx.vid();
    if ctx.might_depend_on(v) {
      let x = self.fetch(ctx);
      let (hi, lo) = (x.hi, x.lo);
      if v == cv { expr![self, ((n & hi) ^ lo)] }
      else {
        let rhi = self.sub(v,n,hi);
        let rlo = self.sub(v,n,lo);
        let top = NID::from_vid(cv);
        expr![self, ((top & rhi) ^ rlo)] }}
    else { ctx }}

  fn save(&self, _path:&str)->::std::io::Result<()> { todo!("anf::save") }

  fn solution_set(&self, n: NID, nvars: usize)->hashbrown::HashSet<Reg> {
    self.solutions_pad(n, nvars).collect() }

} // impl Base for ANFBase

// internal ANFBase implementation

impl ANFBase {

  fn fetch(&self, n:NID)->VHL {
    if n.is_vid() { // variables are (v*I)+O if normal, (v*I)+I if inverted.
      VHL{v:n.vid(), hi:I, lo: if n.is_inv() { I } else { O } }}
    else {
      let mut anf = self.nodes[n.idx()];
      if n.is_inv() { anf.lo = !anf.lo }
      anf }}

  fn vhl(&mut self, v:VID, hi0:NID, lo0:NID)->NID {
    // this is technically an xor operation, so if we want to call it directly,
    // we need to do the same logic as xor() to handle the 'not' bit.
    // note that the cache only ever contains 'raw' nodes, except hi=I
    if hi0 == I && lo0 == O { return NID::from_vid(v) }
    let (hi,lo) = (hi0, lo0.raw());
    let res =
      if let Some(&nid) = self.cache.get(&VHL{v, hi, lo}) { nid }
      else {
        let anf = VHL{ v, hi, lo };
        let nid = NID::from_vid_idx(v, self.nodes.len());
        self.cache.insert(anf, nid);
        self.nodes.push(anf);
        nid };
    if lo.is_inv() { !res } else { res }}

  fn calc_and(&mut self, x:NID, y:NID)->NID {
    let (xv, yv) = (x.vid(), y.vid());
    match xv.cmp_depth(&yv) {
      VidOrdering::Above =>
        // base case: x:a + y:(pq+r)  a<p<q, p<r  --> a(pq+r)
        if x.is_vid() { self.vhl(x.vid(), y, O) }
        else {
          //     x:(ab+c) * y:(pq+r)
          //  =  ab(pq) + ab(r) + c(pq) + c(r)
          //  =  a(b(pq+r)) + c(pq+r)
          //  =  a(by) + cy
          // TODO: these can all happen in parallel.
          let VHL{v:a, hi:b, lo:c } = self.fetch(x);
          let hi = self.and(b, y);
          let lo = self.and(c, y);
          self.vhl(a, hi, lo)},
      VidOrdering::Below => self.and(y, x),
      VidOrdering::Level => {
        // x:(ab+c) * y:(aq+r) --> abq+abr+acq+cr --> a(b(q+r) + cq)+cr
        // xy = (ab+c)(aq+r)
        //       abaq + abr + caq +cr
        //       abq  + abr + acq + cr
        //       a(b(q+r)+cq)+cr
        let VHL{ v:a, hi:b, lo:c } = self.fetch(x);
        let VHL{ v:p, hi:q, lo:r } = self.fetch(y);
        assert_eq!(a,p);
        // TODO: run in in parallel:
        let cr = self.and(c,r);
        let cq = self.and(c,q);
        let qxr = self.xor(q,r);
        let n = NID::from_vid(a);
        expr![self, ((n & ((b & qxr) ^ cq)) ^ cr)] }}}

  /// called only by xor, so simple cases are already handled.
  fn calc_xor(&mut self, x:NID, y:NID)->NID {
    let (xv, yv) = (x.vid(), y.vid());
    match xv.cmp_depth(&yv) {
      VidOrdering::Above =>  {
        // x:(ab+c) + y:(pq+r) --> ab+(c+(pq+r))
        let VHL{v, hi, lo} = self.fetch(x);
        let lo = self.xor(lo, y);
        self.vhl(v, hi, lo)},
      VidOrdering::Below => self.xor(y, x),
      VidOrdering::Level => {
        // a + aq
        // a(1+0) + a(q+0)
        // a((1+0) + (q+0))
        // x:(ab+c) + y:(aq+r) -> ab+c+aq+r -> ab+aq+c+r -> a(b+q)+c+r
        let VHL{v:a, hi:b, lo:c} = self.fetch(x);
        let VHL{v:p, hi:q, lo:r} = self.fetch(y);
        assert_eq!(a,p);
        let hi = self.xor(b, q);
        let lo = self.xor(c, r);
        self.vhl(a, hi, lo)}}}

  pub fn solutions_pad(&self, n:NID, nvars:usize)->ANFSolIterator {
    ANFSolIterator::from_anf_base(self, n, nvars)}
} // impl ANFBase


impl HiLoBase for ANFBase {
  fn get_hilo(&self, nid:NID)->Option<HiLo> {
    let VHL { v:_, hi, lo } = self.fetch(nid);
    Some(HiLo { hi, lo }) }}

impl CursorPlan for ANFBase {}

// cursor logic
impl ANFBase {

  fn log(&self, _cur:&Cursor, _msg: &str) {
    #[cfg(test)] {
      print!("{:>10}", _cur.node);
      print!(" {:?}", _cur.scope);
      println!(" {:50} {:?}", _msg, _cur.nstack); }}

  pub fn first_term(&self, n:NID)->Option<Cursor> {
    if n == O { return None } // O has no other terms, and we can't represent O with a cursor
    let nvars = n.vid().var_ix();
    let mut cur = Cursor::new(nvars, n); // vid().var_ix()+1
    cur.descend(self); // walk down the lo branches to lowest term (O or I)
    Some(cur) }

  pub fn next_term(&self, mut cur:Cursor)->Option<Cursor> {
    self.log(&cur,"== next_term()");
    if !cur.node.is_const() {
      println!("warning: ANFBase::next_term should be called on cursor pointing at a leaf.");
      cur.descend(self); }
    loop {
      cur.step_up();                             self.log(&cur,"step up");
      cur.go_next_lo_var();                      self.log(&cur,"next lo");
      if cur.at_top() && cur.var_get() { self.log(&cur, "@end"); return None }
      cur.clear_trailing_bits();                 self.log(&cur, "cleared trailing");
      cur.put_step(self, true);
      if cur.node == I { self.log(&cur, "<-- answer (lo)"); return Some(cur) }
      cur.descend(self);                         self.log(&cur, "descend");
      if cur.node == I { self.log(&cur, "<-- answer (lo)"); return Some(cur) }}}

  pub fn terms(&self, n:NID)->ANFTermIterator {
    ANFTermIterator::from_anf_base(self, n) }}

pub struct ANFTermIterator<'a> {
  base: &'a ANFBase,
  next: Option<Cursor> }

impl<'a> ANFTermIterator<'a> {
  pub fn from_anf_base(base: &'a ANFBase, nid:NID)->Self {
    if let Some(next) = base.first_term(nid) {
      ANFTermIterator{ base, next:Some(next) }}
    else {
      ANFTermIterator{ base, next:None }}}}

impl<'a> Iterator for ANFTermIterator<'a> {
  type Item = Reg;
  fn next(&mut self)->Option<Self::Item> {
    if let Some(cur) = self.next.take() {
      let reg = cur.scope.clone();
      self.next = self.base.next_term(cur);
      Some(reg) }
    else { None }}}


/// iterator for actual solutions.
/// this works by converting to a bdd.

pub struct ANFSolIterator<'a> {
  _anf: &'a ANFBase,
  bdd: BDDBase,
  //acur: Option<Cursor>,
  bcur: Option<Cursor>}

impl<'a>  ANFSolIterator<'a> {
  pub fn from_anf_base(anf: &'a ANFBase, nid:NID, nvars:usize)->Self {
    let mut bdd = BDDBase::new();
    // TODO: convert ANF->BDD incrementally, to speed up time to first solution.
    // This will involve copying bcur.scope but changing the actual nids on the stack.
    //let acur = anf.first_term(nvars, nid);
    let bnid = anf.to_base(nid, &mut bdd);
    let bcur = bdd.first_solution(bnid, nvars);
    ANFSolIterator{ _anf:anf, bdd, bcur } }}

impl<'a> Iterator for ANFSolIterator<'a> {
  type Item = Reg;
  fn next(&mut self)->Option<Self::Item> {
    if let Some(cur) = self.bcur.take() {
      let res = Some(cur.scope.clone());
      self.bcur = self.bdd.next_solution(cur);
      res }
    else { None } }}


impl ANFBase {

  /// transfer node to another base (e.g. bdd), and return the NID from that base.
  pub fn to_base(&self, n:NID, dest: &mut dyn Base)->NID {
    let mut sum = nid::O;
    if n.is_inv() { sum = nid::I }
    for t in self.terms(n.raw()) {
      let mut term = I;
      for v in t.hi_bits() {
        term = dest.and(term, NID::var(v as u32));
        println!("term: {}", term) }
      sum = dest.xor(sum, term);
      println!("sum: {}", sum) }
    sum }}


// test suite
test_base_consts!(ANFBase);
test_base_when!(ANFBase);

#[test] fn test_anf_hilo() {
  let base = ANFBase::new();
  let a = NID::var(0);
  let VHL{ v, hi, lo } = base.fetch(a);
  assert_eq!(v, a.vid());
  assert_eq!(hi, I);
  assert_eq!(lo, O); }

#[test] fn test_anf_hilo_not() {
  let base = ANFBase::new();
  let a = NID::var(0);
  let VHL{ v, hi, lo } = base.fetch(!a);
  assert_eq!(v, a.vid());
  assert_eq!(hi, I);
  assert_eq!(lo, I); // the final I never appears in the stored structure,
  // but if fetch is given an inverted nid, it inverts the lo branch.
}


#[test] fn test_anf_xor() {
  let mut base = ANFBase::new();
  let a = NID::var(0); let b = NID::var(1);
  let (axb, bxa) = (base.xor(a,b), base.xor(b,a));
  assert_eq!(O, base.xor(a,a), "a xor a should be 0");
  assert_eq!(!a, base.xor(I,a), "a xor 1 should be ~a");
  assert_eq!(axb, bxa, "xor should be order-independent");

  let VHL{ v, hi, lo } = base.fetch(axb);
  // I want this to work regardless of which direction the graph goes:
  let topv = topmost(a.vid(), b.vid());
  let botv = botmost(a.vid(), b.vid());
  assert_eq!(v, topv);
  assert_eq!(hi, I);
  assert_eq!(lo, NID::from_vid(botv)); }

#[test] fn test_anf_xor_inv() {
  let mut base = ANFBase::new();
  let a = NID::var(0); let b = NID::var(1);
  let axb = base.xor(a, b);
  let naxb = base.xor(!a, b);
  let axnb = base.xor(a, !b);
  let naxnb = base.xor(!a, !b);
  assert_eq!(naxnb, axb, "expect ~a ^ ~b == a^b");
  assert_eq!(axnb, naxb, "expect a ^ ~b ==  ~a ^ b");
  assert_eq!(axb, !naxb, "expect a ^ b ==  ~(~a ^ b)"); }


#[test] fn test_anf_xor3() {
  let mut base = ANFBase::new();
  let a = NID::var(0); let b = NID::var(1); let c = NID::var(2);
  assert_eq!(expr![base, ((a ^ b) ^ c)],
             expr![base, (a ^ (b ^ c))]); }


#[test] fn test_anf_and() {
  let mut base = ANFBase::new();
  let a = NID::var(0); let b = NID::var(1);
  let ab = base.and(a, b);
  let VHL{v, hi, lo} = base.fetch(ab);
  let topv = topmost(a.vid(), b.vid());
  let botv = botmost(a.vid(), b.vid());
  assert_eq!(v, topv);
  assert_eq!(hi, NID::from_vid(botv));
  assert_eq!(lo, O);}

  #[test] fn test_anf_xtb() {
    let mut base = ANFBase::new();
    let (x0, x1) = (VID::var(0), VID::var(1));
    let t = NID::from_vid(topmost(x0,x1));
    let b = NID::from_vid(botmost(x0,x1));
    let tb = base.and(b,t);
    let bxtb = base.xor(b, tb); // b ^ tb = t(b ^ a)
    let txtb = base.xor(t, tb); // b ^ ba = b((a+1)+0)

    let (bv, tv) = (b.vid(), t.vid());
    assert_eq!(base.fetch(b), VHL{ v:bv, hi:I, lo:nid::O}, "b = b(1)+0");
    assert_eq!(base.fetch(t), VHL{ v:tv, hi:I, lo:nid::O}, "t = t(1)+0");
    assert_eq!(base.fetch(tb), VHL{ v:tv, hi:b, lo:nid::O}, "tb = t(b)+0");
    assert_eq!(base.fetch(bxtb), VHL{ v:tv, hi:b, lo:b}, "b + tb = t(b)+b");
    assert_eq!(base.fetch(txtb), VHL{ v:tv, hi:!b, lo:nid::O}, "t+tb = t(b+1)+0");
  }

#[test] fn test_anf_and3() {
  let mut base = ANFBase::new();
  let a = NID::var(0); let b = NID::var(1); let c = NID::var(2);
  assert_eq!(expr![base, ((a & b) & c)],
             expr![base, (a & (b & c))]); }


#[test] fn test_anf_and_big() {
  // x:(ab+c) * y:(pq+r) --> ab(pq+r) + c(pq+r)
  let mut base = ANFBase::new();
  let a = NID::var(0); let b = NID::var(1); let c = NID::var(2);
  let p = NID::var(3); let q = NID::var(4); let r = NID::var(5);
  let ab = base.and(a,b); let pq = base.and(p,q);
  let actual = expr![base, ((ab ^ c) & (pq ^ r))];
  let expected = expr![base, ((ab & (pq ^ r)) ^ (c & (pq ^ r)))];
  assert_eq!(expected, actual); }

#[test] fn test_anf_and_same_head() {
  // x:(ab+c) * y:(aq+r) --> abq+abr+acq+cr --> a(b(q+r) + cq)+cr
  let mut base = ANFBase::new();
  let a = NID::var(0); let b = NID::var(1); let c = NID::var(2);
  let q = NID::var(3); let r = NID::var(4);
  let ab = base.and(a,b); let aq = base.and(a,q);
  let actual = expr![base, ((ab ^ c) & (aq ^ r))];
  let expected = expr![base, ((a & ((b & (q ^ r)) ^ (c&q)))^(c&r))];
  assert_eq!(expected, actual); }


#[test] fn test_anf_sub() {
  let mut base = ANFBase::new();
  let a = NID::var(0); let b = NID::var(1); let c = NID::var(2);
  let x = NID::var(3); let y = NID::var(4); let z = NID::var(5);
  let ctx = expr![base, ((a & b) ^ c) ];
  let xyz = expr![base, ((x & y) ^ z) ];
  assert_eq!(base.sub(a.vid(), xyz, ctx), expr![base, ((xyz & b) ^ c)]);
  assert_eq!(base.sub(b.vid(), xyz, ctx), expr![base, ((a & xyz) ^ c)]);}

#[test] fn test_anf_sub_inv() {
    let mut base = ANFBase::new(); let nv = NID::var;
    let (v1,v2,v4,v6) = (nv(1), nv(2), nv(4), nv(6));
    let ctx = expr![base, (v1 & v6) ];
    let top = expr![base, ((I^v4) & v2)];
    assert_eq!(top, base.and(!v4, v2), "sanity check");
    // v1 * v6 ; v1->(~v4 * v2)
    // -> (v2 * (v4 + 1)) *v6
    // -> (v2v4 +v2) & v6
    // -> v2v4v6 + v2v6
    let expect = expr![base, ((v2 & (v4 & v6)) ^ (v2 & v6))];
    let actual = base.sub(v1.vid(), top, ctx);
    // base.show_named(top, "newtop");
    // base.show_named(expect, "expect");
    // base.show_named(actual, "actual");
    assert_eq!(expect, actual);}


#[test] fn test_anf_terms() {
  let mut base = ANFBase::new(); let nv = NID::var;
  let (x,y,z) = (nv(0), nv(1), nv(2));
  let n = expr![base, ((z^(z&y))^((y&x)^x))];
  // anf: x(0+1) + y(x(0+1)) + z(y(0+1))
  // terms: x yx zy z
  let terms:Vec<_> = base.terms(n).map(|t| t.as_usize()).collect();
  assert_eq!(terms, [0b001, 0b011, 0b100, 0b110]);}


#[test] fn test_anf_terms_not() {
  let mut anf = ANFBase::new();
  let (a,_,c) = (NID::var(0), NID::var(1), NID::var(2));
  let anc = expr![anf, (a & (c^I))];
  let res:Vec<_> = anf.terms(anc).map(|reg|reg.as_usize()).collect();
  assert_eq!(res, vec![0b001,0b101]); }

#[test] fn test_anf_terms_bug() {
  let mut anf = ANFBase::new();
  let (a,b,c) = (NID::var(0), NID::var(1), NID::var(2));
  let x = expr![anf, ((a & (b^c)) ^ (b & (c^I)))]; // b^ba^ca^cb
  let t:Vec<_> = anf.terms(x).map(|r|r.as_usize()).collect();
  assert_eq!(t, vec![0b010,0b011,0b101,0b110]); }

#[test] fn test_anf_to_base() {
  use bdd::BDDBase;
  let mut anf = ANFBase::new();
  let mut bdd = BDDBase::new();
  let (a,b,c) = (NID::var(0), NID::var(1), NID::var(2));
  let initial = expr![anf, (a & (c^I))];
  let expect  = expr![bdd, (a & (c^I))];
  let actual  = anf.to_base(initial, &mut bdd);
  assert_eq!(expect, actual, "anf-> bdd should get same answer as pure bdd (1).");
  let initial = expr![anf, (a & (b^c))];
  let expect  = expr![bdd, (a & (b^c))];
  let actual  = anf.to_base(initial, &mut bdd);
  assert_eq!(expect, actual, "anf-> bdd should get same answer as pure bdd (2).");

  let initial = expr![anf, ((a & (b^c)) ^ (b & (c^I)))];
  let expect  = expr![bdd, ((a & (b^c)) ^ (b & (c^I)))];
  let actual  = anf.to_base(initial, &mut bdd);
  assert_eq!(expect, actual, "anf-> bdd should get same answer as pure bdd (3).");}
