/**
 * Nested algebraic form. Represents an ANF polynomial.
 * The main difference between this and anf.rs is that this
 * version allows deferred evaluation.
 */
use crate::simp;
use crate::vhl::Vhl;
use crate::{NID, I, O, vid::VID};
use crate::ast::RawASTBase;
use crate::vid::VidOrdering;
use dashmap::DashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NAF {
  Vhl ( Vhl ),
  And { x: NID, y: NID },
  Xor { x: NID, y: NID }}

type NafHashMap<K,V> = DashMap<K,V,fxhash::FxBuildHasher>;

struct VhlNid { nid:NID }
impl std::ops::Not for VhlNid {
  type Output = VhlNid;
  fn not(self) -> VhlNid { VhlNid{nid: !self.nid} }}
impl VhlNid {
  pub fn is_inv(&self)->bool { self.nid.is_inv() }
  pub fn raw(&self)->Self { VhlNid{nid:self.nid.raw()} }}



#[derive(Debug, Default)]
pub struct NAFBase {
  pub nodes: Vec<NAF>,
  cache: NafHashMap<Vhl, NID> }

fn inv_vhl_if(vhl:Vhl, inv:bool)->Vhl {
  if inv { let Vhl{ v, hi, lo } = vhl;
    Vhl{v, hi, lo:!lo}}
  else { vhl }}

impl NAFBase {
  fn new()->Self { NAFBase{ nodes:vec![], cache: NafHashMap::default() } }

  /// insert a new node and and return a NID with its index.
  pub fn push(&mut self, x:NAF)->NID {
    let res = NID::ixn(self.nodes.len());
    // println!("naf[{:?}] = {:?}", res, x);
    self.nodes.push(x);
    res }

  fn get(&self, n:NID)->Option<NAF> {
    if n.is_ixn() {
      assert!(!n.is_inv(), "can't fetch inverted ixn nids");
      self.nodes.get(n.idx()).cloned() }
    else if n.is_var() {
      Some(NAF::Vhl(Vhl { v: n.vid(), hi:I, lo: NID::from_bit(n.is_inv()) }))}
    else { None }}

  fn get_vhl(&self, xi:NID)->Option<Vhl> {
    if xi.is_var() { Some(Vhl{ v:xi.vid(),  hi:I, lo:NID::from_bit(xi.is_inv()) }) }
    else if let Some(NAF::Vhl(vhl)) = self.get(xi.raw()) { Some(inv_vhl_if(vhl, xi.is_inv())) }
    else { None }}

  fn get_vhls(&self, xi:NID, yi:NID)->Option<(Vhl,Vhl)> {
    if let (Some(x), Some(y)) = (self.get_vhl(xi), self.get_vhl(yi)) { Some((x,y)) }
    else { None }}

  fn get_vhl_nids(&self, xi:NID, yi:NID)->Option<(VhlNid, VhlNid)> {
    if self.get_vhls(xi,yi).is_some() { Some((VhlNid{nid:xi}, VhlNid{nid:yi})) }
    else { None }}

  fn vhl(&mut self, v:VID, hi0:NID, lo0:NID)->VhlNid {
    // !! exactly the same logic as anf::vhl(), but different hashmap/vhl
    // this is technically an xor operation, so if we want to call it directly,
    // we need to do the same logic as xor() to handle the 'not' bit.
    // note that the cache only ever contains 'raw' nodes, except hi=I
    if hi0 == I && lo0 == O { return VhlNid{nid: NID::from_var(v)} }
    if hi0 == I && lo0 == I { return VhlNid{nid:!NID::from_var(v)} }
    let (hi,lo) = (hi0, lo0.raw());
    let vhl = Vhl{ v, hi, lo };
    let res:NID =
      if let Some(nid) = self.cache.get(&vhl) { *nid.value() }
      else {
        let vhl = Vhl { v, hi, lo };
        let nid = NID::ixn(self.nodes.len());
        self.cache.insert(vhl.clone(), nid);
        // !! i want to call self.push() here but borrow checker complains because cache is borrowed (!?)
        // println!("naf[{:?}] = {:?} (vhl)", nid, vhl);
        self.nodes.push(NAF::Vhl(vhl));
        nid };
    if lo.is_inv() { VhlNid{nid: !res} } else { VhlNid{nid: res} }}

  pub fn fetch(&mut self, n:NID)->Vhl {
    match self.get(n.raw()).unwrap() {
      NAF::And { x:_, y:_ } => panic!("expected VHL, got And"),
      NAF::Xor { x:_, y:_ } => panic!("expected VHL, got Xor"),
      NAF::Vhl(vhl) => inv_vhl_if(vhl, n.is_inv()) }}

  fn and_vhls(&mut self, xi:VhlNid, yi:VhlNid)->VhlNid {
      let x = self.fetch(xi.nid);
      let y = self.fetch(yi.nid);
      let vhl = match x.v.cmp_depth(&y.v) {
        VidOrdering::Below => { return self.and_vhls(yi, xi) },
        VidOrdering::Above => {
          //     x:(ab+c) * y:(pq+r)  -> a(by) + cy
          let hi = self.sub_and(&x.hi, &yi.nid);
          let lo = self.sub_and(&x.lo, &yi.nid);
          Vhl { v:x.v, hi, lo }}
        VidOrdering::Level => {
          // xy = (vb+c)(vq+r)
          //       vbq + vbr + vcq + cr
          //       v(bq+br+cq) + cr
          let Vhl{ v:_, hi:b, lo:c } = x;
          let Vhl{ v:_, hi:q, lo:r } = y;
          let bq = self.sub_and(&b, &q);
          let br = self.sub_and(&b, &r);
          let cq = self.sub_and(&c, &q);
          let cr = self.sub_and(&c, &r);
          let h0 = self.sub_xor(&bq, &br);
          let hi = self.sub_xor(&h0, &cq);
          Vhl{ v:x.v, hi, lo:cr }}};
      let mut res = self.vhl(vhl.v, vhl.hi, vhl.lo);
      // case 0:  x: a & y: b ==> ab
      // case 1:  x:~a & y: b ==> ab ^ b
      // case 2:  x: a & y:~b ==> ab ^ a
      // case 3:  x:~a & y:~b ==> ab ^ a ^ b ^ 1
      if xi.is_inv() { res = self.xor_vhls(res, yi.raw()) }
      if yi.is_inv() { res = self.xor_vhls(res, xi.raw()) }
      if xi.is_inv() && yi.is_inv() { res = !res }
      res }

  fn xor_vhls(&mut self, xi:VhlNid, yi:VhlNid)->VhlNid {
    let x = self.fetch(xi.nid);
    let y = self.fetch(yi.nid);
    let res = match x.v.cmp_depth(&y.v) {
      VidOrdering::Below => { return self.xor_vhls(yi, xi) },
      VidOrdering::Above => {
        let lo = self.sub_xor(&x.lo, &yi.nid);
        self.vhl(x.v, x.hi, lo)},
      VidOrdering::Level => {
        // x:(ab+c) + y:(aq+r) -> ab+c+aq+r -> ab+aq+c+r -> a(b+q)+c+r
        let hi = self.sub_xor(&x.hi, &y.hi);
        let lo = self.sub_xor(&x.lo, &y.lo);
        self.vhl(x.v, hi, lo)}};
    // handle the constant term:
    if xi.is_inv() == yi.is_inv() { res } else { !res }}

  // these are for sub-expressions. they're named this way so expr![] works.
  pub fn xor(&mut self, xi: NID, yi:NID)->NID {
    if let Some(res) = simp::xor(xi, yi) { res }
    else if let Some((x,y)) = self.get_vhl_nids(xi, yi) { self.xor_vhls(x, y).nid }
    else {
      println!("self.nodes:");
      for (i, n) in self.nodes.iter().enumerate() {
        println!("{:4} | {:?}", i, n)}
      println!("xi: {:?} ix: {:?}-> {:?}", xi, xi.idx(), self.get(xi));
      println!("yi: {:?} -> {:?}", yi, self.get(yi));
      panic!("bad args to top-level xor: ({:?}, {:?})", xi, yi)}}

  pub fn and(&mut self, xi: NID, yi:NID)->NID {
    if let Some(res) = simp::and(xi, yi) { res }
    else if let Some((x,y)) = self.get_vhl_nids(xi, yi) { self.and_vhls(x, y).nid }
    else { panic!("bad args to top-level and: ({:?}, {:?})", xi, yi) }}

  pub fn sub_and(&mut self, xi:&NID, yi:&NID)->NID {
    if let Some(res) = simp::and(*xi, *yi) { res }
    else { self.push(NAF::And{ x:*xi, y:*yi })}}

  pub fn sub_xor(&mut self, xi:&NID, yi:&NID)->NID {
    if let Some(res) = simp::xor(*xi, *yi) { res }
    else { self.push(NAF::Xor{ x:*xi, y:*yi })}}

  // return the definition of the topmost node in the translated AST
  pub fn top(&self)->Option<&NAF> { self.nodes.last().clone() }}



// a packed AST is arranged so that we can do a bottom-up computation
// by iterating through the bits.
pub fn from_packed_ast(ast: &RawASTBase)->NAFBase {
  let mut res = NAFBase::new();
  // the NAFBase will have multiple nodes for each incoming AST node,
  // so keep a map of AST index -> NAF index
  let map = |n:NID, map:&Vec<NID>|->NID {
    if n.is_ixn() { let r = map[n.idx()]; if n.is_inv() { !r } else { r } }
    else { n }};
  let mut new_nids : Vec<NID> = vec![];
  for (i, bit) in ast.bits.iter().enumerate() {
    let (f, args) = bit.to_app();
    assert_eq!(2, args.len());
    let x = map(args[0], &new_nids);
    let y = map(args[1], &new_nids);
    let new = match f.to_fun().unwrap() {
      crate::ops::AND => res.and(x, y),
      crate::ops::XOR => res.xor(x, y),
      _ => panic!("no rule to translate bit #{:?} ({:?})", i, bit)};
    // println!("map[{:3}] {:?} -> {:?}", i, bit, new);
    new_nids.push(new)}
  res }
