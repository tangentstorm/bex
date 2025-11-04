/**
 * Nested algebraic form. Represents an ANF polynomial.
 * The main difference between this and anf.rs is that this
 * version allows deferred evaluation.
 * (Note: this module is experimental and far from stable.)
 */
use std::collections::HashSet;
use dashmap::DashMap;
use crate::ops::Ops;
use crate::{ops, simp, vhl::Vhl};
use crate::{NID, I, O, vid::VID};
use crate::{ast::RawASTBase, vid::{topmost, VidOrdering}};


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NAF {
  Vhl ( Vhl ),
  And { inv:bool, x: NID, y: NID },
  Xor { inv:bool, x: NID, y: NID }}


impl NAF {
  pub fn var(&self)->VID {
    match self {
      NAF::Vhl(vhl) => vhl.v,
      NAF::And { inv:_, x, y} => topmost(x.vid(), y.vid()),
      NAF::Xor { inv:_, x, y} => topmost(x.vid(), y.vid())}}

  pub fn inv_if(self, cond:bool)->Self {
    if cond { match self {
      NAF::Vhl(vhl) => NAF::Vhl(inv_vhl_if(vhl, true)),
      NAF::And { inv, x, y } => NAF::And { inv:!inv, x, y },
      NAF::Xor { inv, x, y } => NAF::Xor { inv:!inv, x, y }}}
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
  pub fn new()->Self { NafBase{ nodes:vec![], cache: NafMap::default() } }

  /// insert a new node and and return a NID with its index.
  pub fn push(&mut self, naf:NAF)->NID {
    let nid = NID::from_vid_idx(naf.var(), self.nodes.len());
    // println!("naf[{nid:?}] = {naf:?}");
    self.nodes.push(naf);
    nid }

  pub fn get(&self, n:NID)->Option<NAF> {
    if n.is_vid() {
      Some(NAF::Vhl(Vhl { v: n.vid(), hi:I, lo: NID::from_bit(n.is_inv()) }))}
    else if n.is_const() { None }
    else { self.nodes.get(n.idx()).cloned().map(|x|x.inv_if(n.is_inv())) }}

  /// get vhl if it's already a vhl (to convert, see .vhl())
  pub fn get_vhl(&self, xi:NID)->Option<Vhl> {
    if xi.is_vid() { Some(Vhl{ v:xi.vid(),  hi:I, lo:NID::from_bit(xi.is_inv()) }) }
    else if let Some(NAF::Vhl(vhl)) = self.get(xi.raw()) {
      Some(inv_vhl_if(vhl, xi.is_inv())) }
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
        self.cache.insert(vhl, nid);
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
          let bq_br = self.sub_xor(&bq, &br);
          let hi = self.sub_xor(&bq_br, &cq);
          // let hi = self.sub_sum(vec![bq, br, cq]);
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
    else { self.push(NAF::Xor{ inv:(xi.is_inv() ^ yi.is_inv()), x:xi.raw(), y:yi.raw() })}}


  pub fn walk<F>(&self, n:NID, f:&mut F) where F:FnMut(NID) {
    let mut seen = HashSet::new();
    self.step(n,f,&mut seen)}

  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>) where F:FnMut(NID) {
    if !seen.contains(&n.raw()) {
      seen.insert(n.raw());
      f(n);
      if !n.is_lit() {
        match self.get(n).unwrap() {
          NAF::Vhl(vhl) => {
            self.step(vhl.hi, f, seen);
            self.step(vhl.lo, f, seen)},
          NAF::And { inv:_, x, y } => {
            self.step(x, f, seen);
            self.step(y, f, seen)},
          NAF::Xor { inv:_, x, y } => {
            self.step(x, f, seen);
            self.step(y, f, seen)}}}}}

  /// this prints a tree of subnodes for the given nid, ending
  /// in a leaf whenever a VHL is found
  pub fn walk_vhls(&self, ixn:NID, depth:u32) {
    let naf = self.get(ixn.raw()).unwrap();
    for _ in 0..depth { print!(" ") }
    println!("{ixn:?} -> {naf:?}");
    match naf {
        NAF::Vhl(_) => (),
        NAF::And { inv:_, x, y } => {
          self.walk_vhls(x, depth+1);
          self.walk_vhls(y, depth+1);},
        NAF::Xor { inv:_, x, y } => {
          self.walk_vhls(x, depth+1);
          self.walk_vhls(y, depth+1);}}}

  pub fn find_vhls(&mut self, ixn:NID)->Vec<NAF> {
    let naf = self.get(ixn).unwrap();
    // println!("{ixn:?} -> {naf:?}");
    match naf {
        NAF::Vhl(_) => vec![naf],
        NAF::And { inv:_, x, y } => {
          let mut res = vec![];
          res.append(&mut self.find_vhls(x));
          res.append(&mut self.find_vhls(y));
          res},
        NAF::Xor { inv:_, x, y } => {
          let mut res = vec![];
          res.append(&mut self.find_vhls(x));
          res.append(&mut self.find_vhls(y));
          res}}}

  fn coeff_vhl(&mut self, term:&NafTerm, vhl:Vhl)->NID {
    println!("vhl: {vhl:?}");
    let goal = term[0];
    match vhl.v.cmp_depth(&goal) {
      VidOrdering::Below => { println!("terms are below goal {goal:?}. search failed."); O },
      VidOrdering::Level => {
        println!("vhl.v is goal {goal:?}. descending hi branch with new term");
        let next:NafTerm = term.iter().skip(1).cloned().collect();
        self.coeff(&next, vhl.hi)},
      VidOrdering::Above => {
        println!("vhl.v > goal {goal:?}. descending lo branch with same term");
        self.coeff(term, vhl.lo) }}}

  fn coeff_and(&mut self, _term:&NafTerm, _inv:bool, _x:NID, _y:NID)->NID { todo!("coeff_and"); } // TODO
  fn coeff_xor(&mut self, _term:&NafTerm, _inv:bool, _x:NID, _y:NID)->NID { todo!("coeff_xor"); } // TODO

  pub fn gather_terms(&mut self, xs:Vec<NID>)->(Vec<NAF>, Vec<NAF>, Vec<NAF>) {
    let mut vhls = vec![];
    let mut ands = vec![];
    let mut xors = vec![];
    for xi in xs {
      if let Some(x) = self.get(xi) {
        match x {
          NAF::Vhl(_) => vhls.push(x),
          NAF::And { inv:_, x:_, y:_ } => ands.push(x),
          NAF::Xor { inv:_, x:_, y:_ } => xors.push(x)}}
      else { todo!("consts in gather_terms") }}
    (vhls, ands, xors)}

  /// return the coefficient for the given term of the polynomial referred to by `nid`
  pub fn coeff(&mut self, term:&NafTerm, nid:NID)->NID {
    if nid.is_const() || term.is_empty() { return nid }
    if nid.is_vid() {
      return if term.len() == 1 { if nid.vid() == term[0] { I } else { O }}
      else { O }}
    println!("coeff(term: {term:?}, nid: {nid:?})");
    let naf= self.get(nid).unwrap();
    match naf {
      NAF::Vhl(vhl) => self.coeff_vhl(term, vhl),
      NAF::And { inv, x, y } => self.coeff_and(term, inv, x, y),
      NAF::Xor { inv, x, y } => self.coeff_xor(term, inv, x, y)}}

  /// return the final coefficient of the ANF polynomial
  /// (that is, the coefficient of the term that has every input variable in it)
  pub fn last_coeff(&mut self, ixn:NID)->NID {
    let top: Vhl = self.get_vhl(ixn).unwrap();
    let term:NafTerm = (0..=top.v.var_ix()).rev().map(|x|VID::var(x as u32)).collect();
    self.coeff(&term, ixn) }

  /// return a vector classifying how each node in the graph is connected to `nid`.
  /// 0:not connected. 1:lo branch. 1.hi branch. 3:both
  fn color_by_usage(&self, nid:NID)->Vec<u8> {
    let mut res = vec![0u8; self.nodes.len()];
    let vhl = self.get_vhl(nid).expect("can only color_terms on a vhl node");
    let mut paint = |n0:NID, bit:u8| {
      self.walk(n0, &mut |n:NID|{
        if !n.is_lit() { res[n.idx()] |= bit }})};
    paint(vhl.lo, 1);
    paint(vhl.hi, 2);
    res}

  pub fn print_usage(&self, ix:NID) {
    let (mut no, mut lo, mut hi, mut bo) = (0,0,0,0);
    for x in self.color_by_usage(ix) {
      match x {
        0 => no+=1,
        1 => lo+=1,
        2 => hi+=1,
        3 => bo+=1,
        _ => panic!("encountered unexpected usage color {x}!")}}
    let total = self.nodes.len();
    assert_eq!(no+lo+hi+bo, total);
    println!("Usage: ");
    println!("| {no:7} ({:5.2}%) can be discarded", (100 * no) as f64 / total as f64);
    println!("| {lo:7} ({:5.2}%) owned by lo branch", (100 * lo) as f64 / total as f64);
    println!("| {hi:7} ({:5.2}%) owned by hi branch", (100 * hi) as f64 / total as f64);
    println!("| {bo:7} ({:5.2}%) shared by both", (100 * bo) as f64 / total as f64);}


  pub fn print_stats(&self) {
    let (mut num_vhls, mut num_ands, mut num_xors) = (0, 0, 0);
    let size = self.nodes.iter().map(|naf| naf.var().vid_ix()).max().unwrap_or(0) + 1;
    let mut by_var = vec![0; size];
    let mut ands_by_var = vec![0; size];
    let mut xors_by_var = vec![0; size];
    let mut vhls_by_var = vec![0; size];

    for naf in &self.nodes {
      let vix = naf.var().vid_ix();
      by_var[vix] += 1;
      match naf {
        NAF::Vhl(_) => { num_vhls += 1; vhls_by_var[vix] += 1; },
        NAF::And { inv: _, x: _, y: _ } => { num_ands += 1; ands_by_var[vix] += 1; },
        NAF::Xor { inv: _, x: _, y: _ } => { num_xors += 1; xors_by_var[vix] += 1; }}}

    let total = num_vhls + num_ands + num_xors;
    print!("     {total:8} nodes.    ");
    print!("| vhls: {num_vhls:7} ({:5.2}%) ", num_vhls as f64 / total as f64 * 100.0);
    print!("| ands: {num_ands:7} ({:5.2}%) ", num_ands as f64 / total as f64 * 100.0);
    print!("| xors: {num_xors:7} ({:5.2}%) ", num_xors as f64 / total as f64 * 100.0);
    println!();
    println!("{:-<97}","");
    for (i,n) in by_var.iter().enumerate().rev().take(8) {
      print!("{:>4}: {n:7}  ({:5.2})%", VID::var(i as u32).to_string(), *n as f64 / total as f64 * 100.0);
      let n = vhls_by_var[i]; print!(" | vhls: {n:7} ({:5.2}%)", n as f64 / total as f64 * 100.0);
      let n = ands_by_var[i]; print!(" | ands: {n:7} ({:5.2}%)", n as f64 / total as f64 * 100.0);
      let n = xors_by_var[i]; print!(" | xors: {n:7} ({:5.2}%)", n as f64 / total as f64 * 100.0);
      println!(); }}

  /// return a nid referring to the most recently defined node
  pub  fn top_nid(&self)->NID {
    let naf = self.nodes.last().unwrap();
    let v = naf.var();
    NID::from_vid_idx(v, self.nodes.len()-1) }

  /// return the definition of the topmost node in the translated AST
  pub fn top(&self)->Option<&NAF> { self.nodes.last() }}


// a packed AST is arranged so that we can do a bottom-up computation
// by iterating through the bits.
pub fn from_packed_ast(ast: &RawASTBase)->NafBase {
  let mut res = NafBase::new();
  // the NafBase will have multiple references to each incoming AST node.
  // keep a map so we always point to the same translation.
  let new_nid = |n:NID, map:&Vec<NID>|->NID {
    if n.is_ixn() { let r = map[n.idx()]; if n.is_inv() { !r } else { r } }
    else { n }};
  let mut new_nids : Vec<NID> = vec![];
  for (i, bit) in ast.bits.iter().enumerate() {
    let (f, args) = bit.to_app();
    let x = new_nid(args[0], &new_nids);
    let y = new_nid(args[1], &new_nids);
    let z = if args.len() == 3 { new_nid(args[2], &new_nids) } else { O };
    let new = match f.to_fun().unwrap() {
      ops::ANF => res.vhl(x.vid(), y, z).nid, // !! do I need a NANF version?
      ops::AND => res.and(x, y),
      ops::XOR => res.xor(x, y),
      ops::NXOR => !res.xor(x, y),
      ops::NAND => !res.and(x, y),
      _ => panic!("no rule to translate bit #{:?} ({:?})", i, bit)};
    new_nids.push(new)}
  res }

impl NafBase {
  pub fn to_packed_ast(&self, top0:NID)->RawASTBase {
    let mut res = RawASTBase::empty();
    let ix = |n:NID|->NID { if n.is_const() || n.is_lit() { n } else { NID::ixn(n.idx()) }};
    for naf in &self.nodes {
      res.bits.push(Ops::RPN(match naf {
        NAF::Vhl(Vhl{ v, hi, lo }) => {
          vec![NID::from_vid(*v), ix(*hi), ix(*lo), ops::ANF.to_nid()]},
        NAF::And { inv, x, y } => {
          vec![ix(*x), ix(*y), (if *inv { ops::NAND } else { ops::AND }).to_nid()]},
        NAF::Xor { inv, x, y } => {
          vec![ix(*x), ix(*y), (if *inv { ops::NXOR } else { ops::XOR }).to_nid()]} })); }
    res.rebuild_metadata();
    for i in (self.nodes.len()-16)..self.nodes.len()  {
      let row = &res.bits[i];
      let (f, args) = row.to_app();
      println!("#{i:04X} {f:?}({args:?})")}
    let top:NID = if top0 == O { NID::ixn(res.bits.len()-1) } else { top0 };
    let (ast, _new_top) = res.repack(vec![top]);
    println!("ast has {} bits. old top: {top:?} new top: {_new_top:?}", ast.bits.len());
    ast }}
