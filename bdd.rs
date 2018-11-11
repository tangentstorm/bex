// module for efficient implementation of binary decision diagrams
use std::cmp::min;
use std::collections::HashMap;
use std::collections::HashSet;
use std::process::Command;      // for creating and viewing digarams
use std::fs::File;
use std::cmp::max;
use std::io::Write;
use bincode;
use io;

#[derive(Debug, Serialize, Deserialize)]
pub struct BDDBase {
  nvars: usize,
  bits: Vec<BDD>,
  pub deep: Vec<NID>,              // the deepest nid touched by each node
  pub tags: HashMap<String, NID>,
  memo: HashMap<BDD,NID>}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BDD{ v:VID, hi:NID, lo:NID } // if|then|else

pub type NID = usize;
pub type VID = usize;

pub const O:usize = 0;
pub const I:usize = 1 << 63;

pub fn not(x:NID)->NID { x ^ I }    // 'not' bit is the significant bit 0=not(1)
pub fn pos(x:NID)->NID { x &!I }    // positive (strip 'not' bit)
pub fn inv(x:NID)->bool { I==x&I }  // is not bit set? (or 'is x inverted?')

/// Enum to represent a normalized form of a given f,g,h triple
#[derive(Debug)]
pub enum Norm {
  Nid(NID),
  Tup(NID, NID, NID),
  Not(NID, NID, NID)}


impl BDDBase {

  pub fn new(nvars:usize)->BDDBase {
    // the vars are 1-indexed, because node 0 is ⊥ (false)
    let mut bits = vec![BDD{v:I,hi:O,lo:I}]; // node 0 is ⊥
    let mut deep = vec![I];
    for i in 1..nvars+1 { bits.push(BDD{v:i, hi:I, lo: O}); deep.push(i); }
    BDDBase{nvars:nvars, bits:bits, deep:deep, memo:HashMap::new(),tags:HashMap::new()}}

  pub fn nvars(&self)->usize { self.nvars }

  pub fn tag(&mut self, s:String, n:NID) { self.tags.insert(s, n); }
  pub fn get(&self, s:&String)->Option<NID> { Some(*self.tags.get(s)?) }

  pub fn bdd(&self, n:NID)->BDD {
    if inv(n) { let b=self.bdd(not(n)); BDD{v:b.v, hi:not(b.hi), lo:not(b.lo)} }
    else { self.bits[n] }}

  pub fn tup(&self, n:NID)->(VID,NID,NID) {
    let bdd = self.bdd(n);
    (bdd.v, bdd.hi, bdd.lo) }

  pub fn deepest_var(&self, n:NID)->NID { self.deep[pos(n)] }

  /// walk node recursively, without revisiting shared nodes
  pub fn walk<F>(&self, n:NID, f:&mut F) where F: FnMut(NID,NID,NID,NID) {
    let mut seen = HashSet::new();
    self.step(n,f,&mut seen)}

  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>)
  where F: FnMut(NID,NID,NID,NID) {
    if !seen.contains(&n) {
      seen.insert(n); let (i,t,e) = self.tup(n); f(n,i,t,e);
      if pos(t) > 0 { self.step(t, f, seen); }
      if pos(e) > 0 { self.step(e, f, seen); }}}


  // generate dot file (graphviz)
  pub fn dot<T>(&self, n:NID, wr: &mut T) where T : ::std::fmt::Write {
    macro_rules! w {
      ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}

    // TODO: integrate with print_nid
    let fmt = |x| match x {
      I=>"I".to_string(), O=>"O".to_string(),
      _ if inv(x) => format!("not{}", not(x)),
      _ => format!("{}", x)};

    w!("digraph bdd {{");
    w!("  I[label=⊤; shape=square];");
    w!("  O[label=⊥; shape=square];");
    w!("node[shape=circle];");
    self.walk(n, &mut |n,i,_,_|
              w!("  {}[label={}];", fmt(n), fmt(if i==I {n} else {i})));
    w!("edge[style=solid];");
    self.walk(n, &mut |n,_,t,_| w!("  {}->{};", fmt(n), fmt(t)));
    w!("edge[style=dashed];");
    self.walk(n, &mut |n,_,_,e| w!("  {}->{};", fmt(n), fmt(e)));
    w!("}}"); }

  pub fn save_dot(&self, n:NID, path:&str) {
    let mut s = String::new(); self.dot(n, &mut s);
    let mut txt = File::create(path).expect("couldn't create dot file");
    txt.write_all(s.as_bytes()).expect("failet to write text to dot file"); }

  pub fn show(&self, n:NID) {
    self.save_dot(n, "+bdd.dot");
    let out = Command::new("dot").args(&["-Tpng","+bdd.dot"])
      .output().expect("failed to run 'dot' command");
    let mut png = File::create("+bdd.png").expect("couldn't create png");
    png.write_all(&out.stdout).expect("couldn't write png");
    Command::new("firefox").args(&["+bdd.png"])
      .spawn().expect("failed to launch firefox"); }


  // public node builders

  pub fn and(&mut self, x:NID, y:NID)->NID { self.ite(x,  y, O) }
  pub fn xor(&mut self, x:NID, y:NID)->NID { self.ite(x, not(y), y) }
  pub fn  or(&mut self, x:NID, y:NID)->NID { self.ite(x, I, y) }
  pub fn  gt(&mut self, x:NID, y:NID)->NID { self.ite(x, not(y), O) }
  pub fn  lt(&mut self, x:NID, y:NID)->NID { self.ite(x, O, y) }

  pub fn ite(&mut self, f:NID, g:NID, h:NID)->NID {
    match self.norm(f,g,h) {
      Norm::Nid(x) => x,
      Norm::Tup(x,y,z) => self.build(x,y,z),
      Norm::Not(x,y,z) => not(self.build(x,y,z)) }}

  /// nid of y when x is high
  pub fn when_hi(&mut self, x:NID, y:NID)->NID {
    let (yv, yt, ye) = self.tup(y);
    if        yv == I { y }       // y is constant, so no change
    else if   yv == x { yt }      // x ∧ if(x,th,_) → th
    else if   yv > x  { y }       // y independent of x, so no change.
    else { // yv < x              // y depends on x, so recurse.
      let (th,el) = (self.when_hi(x,yt), self.when_hi(x,ye));
      print!("yv<x -> "); print_tup((yv,th, el)); println!("");
      self.ite(yv, th, el) }}

  /// nid of y when x is lo
  pub fn when_lo(&mut self, x:NID, y:NID)->NID {
    let (yv, yt, ye) = self.tup(y);
    if        yv == I { y }       // y constant, so no change
    else if   yv == x { ye }      // ¬x ∧ if(x,_,el) → el
    else if   yv > x  { y }       // y independent of x, so no change.
    else { // yv < x              // y depends on x, so recurse.
      let (th,el) = (self.when_lo(x,yt), self.when_lo(x,ye));
      self.ite(yv, th, el) }}

  /// is n the nid of a variable?
  pub fn is_var(&self, n:NID)->bool {
    let (nv, _, _) = self.tup(n); return nv == n}

  /// is it possible x depends on y?
  /// the goal here is to avoid exploring a subgraph if we don't have to.
  pub fn might_depend(&mut self, x:NID, y:NID)->bool {  // -- TODO --
    let (v,_,_) = self.tup(x);
    if self.is_var(x) { x==y }
    else if y > self.deep[pos(x)] { false }
    else { v <= y && !self.is_var(x)}}

  /// replace x with y in z
  pub fn replace(&mut self, x:NID, y:NID, z:NID)->NID {
    assert!(self.is_var(x), "x should represent a variable");
    if self.might_depend(z,x) {
      let (zv,zt,ze) = self.tup(z);
      if x==zv { self.ite(y, zt, ze) }
      else {
        let th = self.replace(x, y, zt);
        let el = self.replace(x, y, ze);
        self.ite(zv, th, el) }}
    else { z }}

  // private helpers for building nodes

  fn build(&mut self, f:NID, g:NID, h:NID)->NID {
    let ((fv,_,_), (gv,_,_), (hv,_,_))
      = (self.tup(f), self.tup(g), self.tup(h));
    let v = min(fv, min(gv,hv));
    let th = {
      let (i,t,e) = (self.when_hi(v,f), self.when_hi(v,g), self.when_hi(v,h));
      self.ite(i,t,e) };
    let el = {
      let (i,t,e) = (self.when_lo(v,f), self.when_lo(v,g), self.when_lo(v,h));
      self.ite(i,t,e) };
    if th == el { th } else { self.nid(v,th,el) }}

  fn nid(&mut self, f:NID, g:NID, h:NID)->NID {
    let bdd = BDD{v:f,hi:g,lo:h};
    match self.memo.get(&bdd) {
      Some(&n) => n,
      None => {
        let res = self.bits.len() as NID;
        self.memo.insert(bdd, res);
        self.bits.push(bdd);
        let (a,b,c) = (self.deep[pos(f)], self.deep[pos(g)], self.deep[pos(h)]);
        self.deep.push(max(pos(a), max(pos(b), pos(c))));
        res }}}


  /// choose normal form for writing this node. Algorithm based on:
  /// "Efficient Implementation of a BDD Package"
  /// http://www.cs.cmu.edu/~emc/15817-f08/bryant-bdd-1991.pdf
  pub fn norm(&self, f:NID, g:NID, h:NID)->Norm {
    let (nf,ng,nh,pf,pg,ph) = (not(f), not(g), not(h), pos(f), pos(g), pos(h));
    match (f,g,h) {
      (I, g, _)          => Norm::Nid(g),
      (O, _, h)          => Norm::Nid(h),
      (_, g, h) if g==h  => Norm::Nid(g),
      (f, I, O)          => Norm::Nid(f),
      (_, O, I)          => Norm::Nid(nf),
      (f, g, O) if g==f  => Norm::Nid(f),
      (f, g, I) if g==f  => Norm::Nid(I),
      (f, g, h) if g==f  => self.norm(f,I,h),
      (f, g, h) if g==nf => self.norm(f,O,h),
      (f, g, h) if h==f  => self.norm(f,g,O),
      (f, g, h) if h==nf => self.norm(f,g,I),
      _otherwise         => {
        let (fv,_ft,_fe) = self.tup(f);
        let (gv,_gt,_ge) = self.tup(g);
        let (hv,_ht,_he) = self.tup(h);
        let cmp = |x0,x1, y0,y1| (x0<y0) || ((x0==y0) && (x1<y1));
        match (g,h) {
          (I,h) if cmp(hv,ph, fv,pf) => self.norm(h,I,f),
          (g,O) if cmp(gv,pg, fv,pf) => self.norm(g,f,O),
          (_,I) if cmp(gv,pg, fv,pf) => self.norm(ng,nf,I),
          (O,_) if cmp(hv,ph, fv,pf) => self.norm(nh,O,nf),
          (g,x) if cmp(gv,pg, fv,pf) && x==ng => self.norm(g,f,nf),
          _otherwise => {
            // choose form where first 2 slots are NOT inverted:
            // from { (f,g,h), (¬f,h,g), ¬(f,¬g,¬h), ¬(¬f,¬g,¬h) }
            if inv(f) { self.norm(nf,h,g) }
            else if inv(g) { match self.norm(f,ng,nh) {
              Norm::Nid(x) => Norm::Nid(not(x)),
              Norm::Not(x,y,z) => Norm::Tup(x,y,z),
              Norm::Tup(x,y,z) => Norm::Not(x,y,z)}}
            else { Norm::Tup(f,g,h) }}}}}}


  pub fn save(&self, path:&str)->::std::io::Result<()> {
    let s = bincode::serialize(&self).unwrap();
    return io::put(path, &s) }

  pub fn from_path(path:&str)->::std::io::Result<(BDDBase)> {
    let s = io::get(path)?;
    return Ok(bincode::deserialize(&s).unwrap()); }

  pub fn load(&mut self, path:&str)->::std::io::Result<()> {
    let other = BDDBase::from_path(path)?;
    self.nvars = other.nvars;
    self.bits = other.bits;
    self.deep = other.deep;
    self.memo = other.memo;
    self.tags = other.tags;
    Ok(()) }


  pub fn swap(&mut self, n:NID, x:VID, y:VID)-> NID {
    if y>x { return self.swap(n,y,x) }
    /*
        x ____                        x'____
        :     \                       :     \
        y __    y __      =>          y'__    y'__
        :   \    :  \                 :   \    :   \
        ll   lh  hl  hh               ll   hl  lh   hh
    */
    let (xlo, xhi) = (self.when_lo(x,n), self.when_hi(x,n));
    let (xlo_ylo, xlo_yhi) = (self.when_lo(y,xlo), self.when_hi(y,xlo));
    let (xhi_ylo, xhi_yhi) = (self.when_lo(y,xhi), self.when_hi(y,xhi));
    let lo = self.ite(y, xlo_ylo, xhi_ylo);
    let hi = self.ite(y, xlo_yhi, xhi_yhi);
    self.ite(x, lo, hi) }

  pub fn node_count(&self, n:NID)->usize {
    let mut c = 0; self.walk(n, &mut |_,_,_,_| c+=1); c }

} // BDDBase


#[test] fn test_base() {
  let mut base = BDDBase::new(3);
  assert_eq!(base.bits.len(), 4);
  assert_eq!((I,I,O), base.tup(I));
  assert_eq!((I,O,I), base.tup(O));
  assert_eq!((I,O,I), base.tup(0));
  assert_eq!((1,I,O), base.tup(1));
  assert_eq!((2,I,O), base.tup(2));
  assert_eq!((3,I,O), base.tup(3));
  assert_eq!(I, base.when_hi(3,3));
  assert_eq!(O, base.when_lo(3,3))}

#[test] fn test_and() {
  let mut base = BDDBase::new(3);
  let a = base.and(1,2);
  assert_eq!(O, base.when_lo(1,a));
  assert_eq!(2, base.when_hi(1,a));
  assert_eq!(O, base.when_lo(2,a));
  assert_eq!(1, base.when_hi(2,a));
  assert_eq!(a, base.when_hi(3,a));
  assert_eq!(a, base.when_lo(3,a))}

#[test] fn test_xor() {
  let mut base = BDDBase::new(3);
  let x = base.xor(1,2);
  assert_eq!(2,      base.when_lo(1,x));
  assert_eq!(not(2), base.when_hi(1,x));
  assert_eq!(1,      base.when_lo(2,x));
  assert_eq!(not(1), base.when_hi(2,x));
  assert_eq!(x,      base.when_lo(3,x));
  assert_eq!(x,      base.when_hi(3,x))}



pub fn print_nid(x:NID){ match x {
  I=>print!("I"), O=>print!("O"),
  _ if inv(x) => print!("-{}", not(x)), _=> print!("{}", x)}}

pub fn print_tup(n:(NID,NID,NID)){
  print!("("); print_nid(n.0); print!(", "); print_nid(n.1);
  print!(", "); print_nid(n.2); println!(")")}
