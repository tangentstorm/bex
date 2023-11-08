/**
 * Nested algebraic form. Represents an ANF polynomial.
 * The main difference between this and anf.rs is that this
 * version allows deferred evaluation.
 */
use crate::simp;
use crate::vhl::Vhl;
use crate::{NID, I, O, vid::VID};
use crate::ast::RawASTBase;
use crate::vid::{VidOrdering, topmost};
use dashmap::DashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NAF {
  Vhl ( Vhl ),
  And { inv:bool, x: NID, y: NID },
  Sum { inv:bool, xs: Vec<NID> }}

impl NAF {
  pub fn var(&self)->VID {
    match self {
      NAF::Vhl(vhl) => vhl.v,
      NAF::And { inv:_, x, y} => topmost(x.vid(), y.vid()),
      NAF::Sum { inv:_, xs } => {
        let mut v = xs[0].vid();
        for x in xs { v = topmost(v, x.vid())}
        v }}}

  pub fn inv_if(self, cond:bool)->Self {
    if cond { match self {
      NAF::Vhl(vhl) => NAF::Vhl(inv_vhl_if(vhl, true)),
      NAF::And { inv, x, y } => NAF::And { inv:!inv, x, y },
      NAF::Sum { inv, xs } => NAF::Sum { inv:!inv, xs }}}
    else { self }}}

type NafMap<K,V> = DashMap<K,V,fxhash::FxBuildHasher>;
type NafTerm = Vec<VID>;

struct VhlNid { nid:NID }
impl std::ops::Not for VhlNid {
  type Output = VhlNid;
  fn not(self) -> VhlNid { VhlNid{nid: !self.nid} }}

impl VhlNid {
  pub fn is_inv(&self)->bool { self.nid.is_inv() }
  pub fn raw(&self)->Self { VhlNid{nid:self.nid.raw()} }}



#[derive(Debug, Default)]
pub struct NafBase {
  pub nodes: Vec<NAF>,
  cache: NafMap<Vhl, NID> }

fn inv_vhl_if(vhl:Vhl, inv:bool)->Vhl {
  if inv { let Vhl{ v, hi, lo } = vhl;
    Vhl{v, hi, lo:!lo}}
  else { vhl }}

impl NafBase {
  fn new()->Self { NafBase{ nodes:vec![], cache: NafMap::default() } }

  /// insert a new node and and return a NID with its index.
  pub fn push(&mut self, naf:NAF)->NID {
    let nid = NID::from_vid_idx(naf.var(), self.nodes.len());
    // println!("naf[{nid:?}] = {naf:?}");
    self.nodes.push(naf);
    nid }

  fn get(&self, n:NID)->Option<NAF> {
    if n.is_var() {
      Some(NAF::Vhl(Vhl { v: n.vid(), hi:I, lo: NID::from_bit(n.is_inv()) }))}
    else if n.is_const() { None }
    else { self.nodes.get(n.idx()).cloned().map(|x|x.inv_if(n.is_inv())) }}

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
        let nid = NID::from_vid_idx(v, self.nodes.len());
        self.cache.insert(vhl.clone(), nid);
        self.nodes.push(NAF::Vhl(vhl));
        nid };
    if lo.is_inv() { VhlNid{nid: !res} } else { VhlNid{nid: res} }}

  fn and_vhls(&mut self, xi:VhlNid, yi:VhlNid)->VhlNid {
      let x = self.get_vhl(xi.nid).unwrap();
      let y = self.get_vhl(yi.nid).unwrap();
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
          let hi = self.sub_sum(vec![bq, br, cq]);
          Vhl{ v:x.v, hi, lo:cr }}};
      let res = self.vhl(vhl.v, vhl.hi, vhl.lo);
      // case 0:  x: a & y: b ==> ab
      // case 1:  x:~a & y: b ==> ab ^ b
      // case 2:  x: a & y:~b ==> ab ^ a
      // case 3:  x:~a & y:~b ==> ab ^ a ^ b ^ 1
      if xi.is_inv() {
        if yi.is_inv() {
          let si = self.xor_vhls(xi.raw(), yi.raw());
          // the ! here handles the ^1
          !self.xor_vhls(res, si)}
        else { self.xor_vhls(res, yi.raw()) }}
      else if yi.is_inv() { self.xor_vhls(res, xi.raw()) }
      else { res }}

  fn xor_vhls(&mut self, xi:VhlNid, yi:VhlNid)->VhlNid {
    let x = self.get_vhl(xi.nid).unwrap();
    let y = self.get_vhl(yi.nid).unwrap();
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

  fn sub_and(&mut self, xi:&NID, yi:&NID)->NID {
    if let Some(res) = simp::and(*xi, *yi) { res }
    else { self.push(NAF::And{ inv:false, x:*xi, y:*yi })}}

  fn sub_xor(&mut self, xi:&NID, yi:&NID)->NID {
    if let Some(res) = simp::xor(*xi, *yi) { res }
    else { self.sub_sum(vec![*xi, *yi]) }}

  fn sub_sum(&mut self, nids: Vec<NID>)->NID {
    let mut xs = vec![]; let mut inv = false;
    // TODO: cancel pairs of duplicate nids
    for nid in nids {
      if nid == O { continue }
      if nid == I { inv = !inv; continue }
      if nid.is_inv() { inv = !inv }
      xs.push(nid.raw())}
    if xs.is_empty() { NID::from_bit(inv) }
    else { self.push(NAF::Sum{ inv, xs })}}

  /// this prints a tree of subnodes for the given nid, ending
  /// in a leaf whenever a VHL is found
  fn walk_vhls(&self, ixn:NID, depth:u32) {
    let naf = self.get(ixn.raw()).unwrap();
    for _ in 0..depth { print!(" ") }
    println!("{ixn:?} -> {naf:?}");
    match naf {
        NAF::Vhl(_) => return,
        NAF::And { inv:_, x, y } => {
          self.walk_vhls(x, depth+1);
          self.walk_vhls(y, depth+1);},
        NAF::Sum { inv:_, xs} => {
          for x in xs { self.walk_vhls(x, depth+1)  }}}}

  fn find_vhls(&mut self, ixn:NID)->Vec<NAF> {
    let naf = self.get(ixn).unwrap();
    println!("{ixn:?} -> {naf:?}");
    match naf {
        NAF::Vhl(_) => vec![naf],
        NAF::And { inv:_, x, y } => {
          let mut res = vec![];
          res.append(&mut self.find_vhls(x));
          res.append(&mut self.find_vhls(y));
          res},
        NAF::Sum { inv:_, xs } => {
          let mut res = vec![];
          for x in xs { res.append(&mut self.find_vhls(x)) }
          res}}}

  /// return the coefficient for the given term of the polynomial referred to by `nid`
  pub fn coeff(&mut self, term:NafTerm, nid:NID)->u8 {
    if nid == O { return 0 }
    if term.is_empty() {
      // !! not 100% sure what to do here.
      println!("[fyi] coeff([], {:?}). !!! does this make sense?", nid);
      return 1 }
    if nid.is_var() {
      return if term.len() == 1 { if nid.vid() == term[0] { 1 } else { 0 }}
      else { 0 }}
    if nid == I { return 1 }
    println!("coeff(term: {term:?}, nid: {nid:?})");
    let naf= self.get(nid).unwrap();
    match naf {
      NAF::Vhl(vhl) => {
        println!("vhl: {vhl:?}");
        let goal = term[0];
        return match vhl.v.cmp_depth(&goal) {
          VidOrdering::Below => { println!("terms are below goal {goal:?}. search failed."); 0 },
          VidOrdering::Level => {
            println!("vhl.v is goal {goal:?}. descending hi branch with new term");
            let next:NafTerm = term.iter().skip(1).cloned().collect();
            self.coeff(next, vhl.hi)},
          VidOrdering::Above => {
            println!("vhl.v > goal {goal:?}. descending lo branch with same term");
            self.coeff(term, vhl.lo) }}}
      //NAF::And { inv, x, y } => {  todo!("coeff for {naf:?}") }
      //NAF::Sum { inv, xs } => {
      _ => {
        println!("not a vhl: {:?}", self.get(nid));
        println!("-- walking tree to find vhls --");
        let vhls = self.find_vhls(nid);
        println!("-- end of walk --");
        self.walk_vhls(nid, 0);
        println!("sub-items to search: {vhls:?}");
       }}
    return 0 }

  /// return the final coefficient of the ANF polynomial
  /// (that is, the coefficient of the term that has every input variable in it)
  pub fn last_coeff(&mut self, ixn:NID)->u8 {
    let top: Vhl = self.get_vhl(ixn).unwrap();
    let term:NafTerm = (0..=top.v.var_ix()).rev().map(|x|VID::var(x as u32)).collect();
    self.coeff(term, ixn) }

  /// return a nid referring to the most recently defined node
  pub  fn top_nid(&self)->NID {
    let naf = self.nodes.last().unwrap();
    let v = naf.var();
    NID::from_vid_idx(v, self.nodes.len()-1) }

  /// return the definition of the topmost node in the translated AST
  pub fn top(&self)->Option<&NAF> { self.nodes.last().clone() }}


// a packed AST is arranged so that we can do a bottom-up computation
// by iterating through the bits.
pub fn from_packed_ast(ast: &RawASTBase)->NafBase {
  let mut res = NafBase::new();
  // the NafBase will have multiple nodes for each incoming AST node,
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
