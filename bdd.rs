// module for efficient implementation of binary decision diagrams
use std::cmp::min;
use std::collections::HashMap;
use std::collections::HashSet;
use std::process::Command;      // for creating and viewing digarams
use std::fs::File;
use std::cmp::Ordering;
use std::io::Write;
use std::fmt;
use fnv::FnvHashMap;
use bincode;
use io;

// core data types

#[derive(Debug, Serialize, Deserialize)]
pub struct BDDBase {
  nvars: usize,
  bits: Vec<BDD>,
  pub tags: HashMap<String, NID>,
  memo: FnvHashMap<BDD,NID> }

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BDD{ pub v:VID, pub hi:NID, pub lo:NID } // if|then|else

pub type VID = u32;
pub type IDX = u32;

#[repr(C)]
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NID { var: VID, idx: IDX }
pub const INV:VID = 1<<31;  // is inverted
pub const VAR:VID = 1<<30;  // is variable
pub const T:VID = 1<<29;  // T: max VID (hack so O/I nodes show up at bottom)
pub const O:NID = NID{ var:T,     idx:0 };
pub const I:NID = NID{ var:T|INV, idx:0 };
#[inline(always)] pub fn is_var(x:NID)->bool { (x.var & VAR) != 0 }
#[inline(always)] pub fn is_inv(x:NID)->bool { (x.var & INV) != 0 }
#[inline(always)] pub fn idx(x:NID)->usize { x.idx as usize }
#[inline(always)] pub fn var(x:NID)->VID { x.var & !(INV|VAR) }
#[inline(always)] pub fn not(x:NID)->NID { NID { var:x.var^INV, idx:x.idx } }
#[inline(always)] pub fn nv(v:VID)->NID { NID { var:v|VAR, idx:0 } }
#[inline(always)] pub fn nvi(v:VID,i:IDX)->NID { NID { var:v, idx:i } }

impl fmt::Display for NID {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if self.var == T { if is_inv(*self) { write!(f, "I") } else { write!(f, "O") } }
    else {
      if is_inv(*self) { write!(f, "¬")?; }
      if is_var(*self) { write!(f, "x{}", var(*self)) }
      else { write!(f, "@[x{}:{}]", var(*self), self.idx) } }}}


// old implementation where everything is in 1 64-bit number
// maybe more efficient, but i didn't get it to compile...
// i figure making the fields type-safe first will help me get the code right.
/*
pub const O:usize = 0;
pub const I:usize = 1 << 63;  // invert, also the 'true' nid.
pub const V:usize = 1 << 62;  // bit indicating virtual NID for variables
pub const MASK44:usize = 0x0fFFffFFffFF; // 44 1 bits. this is max index per variable.
pub fn not(x:NID)->NID { x ^ I }         // 'not' bit is the most significant bit 0=not(1)
pub fn pos(x:NID)->NID { x &!I }         // positive (strip 'not' bit)
pub fn inv(x:NID)->bool { I==x&I }       // is not bit set? (or 'is x inverted?')
pub fn idx(x:NID)->usize { x & MASK44 }  // limit ourselves to 44 bit indices
pub fn var(x:NID)->VID { pos(x) >> 40 }  // the remaining 18 bits represent variables
pub fn nvi(var:VID,idx:IDX) -> NID { (var << 40) + idx }
pub fn nv(v:VID) -> NID { v | V }
 */

/// Enum to represent a normalized form of a given f,g,h triple
#[derive(Debug)]
pub enum Norm {
  Nid(NID),
  Tup(NID, NID, NID),
  Not(NID, NID, NID)}


impl BDDBase {

  pub fn new(nvars:usize)->BDDBase {
    // the vars are 1-indexed, because node 0 is ⊥ (false)
    let bits = vec![BDD{v:T,hi:O,lo:I}]; // node 0 is ⊥
    BDDBase{nvars:nvars, bits:bits,
            memo:FnvHashMap::default(),
            tags:HashMap::new()}}

  pub fn nvars(&self)->usize { self.nvars }

  pub fn tag(&mut self, s:String, n:NID) { self.tags.insert(s, n); }
  pub fn get(&self, s:&String)->Option<NID> { Some(*self.tags.get(s)?) }

  #[inline]
  pub fn bdd(&self, n:NID)->BDD {
    // bdd for var x still has huge number for the v
    if is_var(n) {
      if is_inv(n) { BDD{v:var(n), lo:I, hi:O }}
      else { BDD{v:var(n), lo:O, hi:I } }}
    else if is_inv(n) {
      let mut b=self.bits[idx(n)]; b.hi=not(b.hi); b.lo=not(b.lo); b }
    else { self.bits[idx(n)] }}

  #[inline]
  pub fn tup(&self, n:NID)->(VID,NID,NID) {
    let bdd = self.bdd(n); (bdd.v, bdd.hi, bdd.lo) }

  /// walk node recursively, without revisiting shared nodes
  pub fn walk<F>(&self, n:NID, f:&mut F) where F: FnMut(NID,VID,NID,NID) {
    let mut seen = HashSet::new();
    self.step(n,f,&mut seen)}

  /// internal helper: one step in the walk.
  fn step<F>(&self, n:NID, f:&mut F, seen:&mut HashSet<NID>)
  where F: FnMut(NID,VID,NID,NID) {
    if !seen.contains(&n) {
      seen.insert(n); let (i,t,e) = self.tup(n); f(n,i,t,e);
      if idx(t) > 0 { self.step(t, f, seen); }
      if idx(e) > 0 { self.step(e, f, seen); }}}


  // generate dot file (graphviz)
  pub fn dot<T>(&self, n:NID, wr: &mut T) where T : ::std::fmt::Write {
    macro_rules! w {
      ($x:expr $(,$xs:expr)*) => { writeln!(wr, $x $(,$xs)*).unwrap() }}

    // TODO: integrate with print_nid
    let fmt = |x| match x {
      I=>"I".to_string(), O=>"O".to_string(),
      _ if is_inv(x) => format!("not{}", not(x)),
      _ => format!("{}", x)};

    w!("digraph bdd {{");
    w!("  I[label=⊤; shape=square];");
    w!("  O[label=⊥; shape=square];");
    w!("node[shape=circle];");
    self.walk(n, &mut |n,v,_,_| w!("  {}[label={}];", fmt(n), v));
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

  #[inline]
  pub fn ite(&mut self, f:NID, g:NID, h:NID)->NID {
    // println!("ite({},{},{})", f,g,h);
    match self.norm(f,g,h) {
      Norm::Nid(x) => x,
      Norm::Tup(x,y,z) => self.build(x,y,z),
      Norm::Not(x,y,z) => not(self.build(x,y,z)) }}

  /// nid of y when x is high
  #[inline]
  pub fn when_hi(&mut self, x:VID, y:NID)->NID {
    match var(y).cmp(&x) {
      Ordering::Greater => y,  // y independent of x, so no change. includes yv = I
      Ordering::Equal => { let(_,yt,_)=self.tup(y); yt } // x ∧ if(x,th,_) → th
      Ordering::Less => {      // y may depend on x, so recurse.
        let (yv, yt, ye) = self.tup(y);
        let (th,el) = (self.when_hi(x,yt), self.when_hi(x,ye));
        self.ite(nv(yv), th, el) }}}

  /// nid of y when x is lo
  #[inline]
  pub fn when_lo(&mut self, x:VID, y:NID)->NID {
    match var(y).cmp(&x) {
      Ordering::Greater => y,  // y independent of x, so no change. includes yv = I
      Ordering::Equal   => { let(_,_,ye)=self.tup(y); ye }, // ¬x ∧ if(x,_,el) → el
      Ordering::Less    => {   // y may depend on x, so recurse.
        let (yv, yt, ye) = self.tup(y);
        let (th,el) = (self.when_lo(x,yt), self.when_lo(x,ye));
        self.ite(nv(yv), th, el) }}}


  /// is n the nid of a variable?
  pub fn is_var(&self, n:NID)->bool { return is_var(n) }

  /// is it possible x depends on y?
  /// the goal here is to avoid exploring a subgraph if we don't have to.
  #[inline]
  fn might_depend(&mut self, x:NID, y:VID)->bool {
    if is_var(x) { var(x)==y } else { var(x) <= y }}

  /// replace var x with y in z
  pub fn replace(&mut self, x:VID, y:NID, z:NID)->NID {
    if self.might_depend(z, x) {
      let (zv,zt,ze) = self.tup(z);
      if x==zv { self.ite(y, zt, ze) }
      else {
        let th = self.replace(x, y, zt);
        let el = self.replace(x, y, ze);
        self.ite(nv(zv), th, el) }}
    else { z }}

  // private helpers for building nodes
  fn build(&mut self, f:NID, g:NID, h:NID)->NID {
    // !! this is one of the most time-consuming bottlenecks, so we inline a lot.
    let v = min(var(f), min(var(g), var(h)));
    let hi = { // when_xx and ite are both mutable borrows, so need temp storage
      let (i,t,e) = (self.when_hi(v,f), self.when_hi(v,g), self.when_hi(v,h));
      self.ite(i,t,e) };
    let lo = {
      let (i,t,e) = (self.when_lo(v,f), self.when_lo(v,g), self.when_lo(v,h));
      self.ite(i,t,e) };
    if hi == lo {hi} else { self.nid(v,hi,lo) }}

  /// this function takes the final form of the triple
  fn nid(&mut self, v:VID, hi:NID, lo:NID)->NID {
    let bdd = BDD{v:v,hi:hi,lo:lo};
    match self.memo.get(&bdd) {
      Some(&n) => n,
      None => {
        let res = NID { var:v, idx:self.bits.len() as IDX};
        self.memo.insert(bdd, res);
        self.bits.push(bdd);
        res }}}


  /// choose normal form for writing this node. Algorithm based on:
  /// "Efficient Implementation of a BDD Package"
  /// http://www.cs.cmu.edu/~emc/15817-f08/bryant-bdd-1991.pdf
  pub fn norm(&self, f0:NID, g0:NID, h0:NID)->Norm {
    // println!("norm(f:{}, g:{}, h:{}) h=O? {}", f0,g0,h0, h0==O);
    let mut f = f0; let mut g = g0; let mut h = h0;
    // rustc doesn't do tail call optimization, so we'll do it ourselves.
    macro_rules! bounce { ($x:expr,$y:expr,$z:expr) => {{
      // !! NB. can't set f,g,h directly because we might end up with e.g. `f=g;g=f;`
      let xx=$x; let yy=$y; let zz=$z;  f=xx; g=yy; h=zz; }}}
    loop {
      match (f,g,h) {
      (I, _, _)          => return Norm::Nid(g),
      (O, _, _)          => return Norm::Nid(h),
      (_, I, O)          => return Norm::Nid(f),
      (_, O, I)          => return Norm::Nid(not(f)),
      (_, _, O) if g==f  => return Norm::Nid(f),
      (_, _, I) if g==f  => return Norm::Nid(I),
      _otherwise => {
        let nf = not(f);
        if      g==h  { return Norm::Nid(g) }
        else if g==f  { g=I } // bounce!(f,I,h)
        else if g==nf { g=O } // bounce!(f,O,h)
        else if h==f  { h=O } // bounce!(f,g,O)
        else if h==nf { h=I } // bounce!(f,g,I)
        else {
          let (xf, xg, xh) = (idx(f), idx(g), idx(h));
          let (fv, gv, hv) = (var(f), var(g), var(h));
          macro_rules! cmp { ($x0:expr,$x1:expr) => { (($x0<fv) || (($x0==fv) && ($x1<xf))) }}
          match (g,h) {
            (I,_) if cmp!(hv,xh) => bounce!(h,I,f),
            (O,_) if cmp!(hv,xh) => bounce!(not(h),O,nf),
            (_,O) if cmp!(gv,xg) => bounce!(g,f,O),
            (_,I) if cmp!(gv,xg) => bounce!(not(g),nf,I),
            _otherwise => {
              let ng = not(g);
              if (h==ng) && cmp!(gv,xg) { bounce!(g,f,nf) }
              // choose form where first 2 slots are NOT inverted:
              // from { (f,g,h), (¬f,h,g), ¬(f,¬g,¬h), ¬(¬f,¬g,¬h) }
              else if is_inv(f) { bounce!(nf,h,g) }
              else if is_inv(g) { return match self.norm(f,ng,not(h)) {
                Norm::Nid(x) => Norm::Nid(not(x)),
                Norm::Not(x,y,z) => Norm::Tup(x,y,z),
                Norm::Tup(x,y,z) => Norm::Not(x,y,z)}}
              else { return Norm::Tup(f,g,h) }}}}}}}}


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
    let lo = self.ite(nv(y), xlo_ylo, xhi_ylo);
    let hi = self.ite(nv(y), xlo_yhi, xhi_yhi);
    self.ite(nv(x), lo, hi) }

  pub fn node_count(&self, n:NID)->usize {
    let mut c = 0; self.walk(n, &mut |_,_,_,_| c+=1); c }

} // BDDBase


#[test] fn test_base() {
  let mut base = BDDBase::new(3);
  let (v1, v2, v3) = (nv(1), nv(2), nv(3));
  assert_eq!(base.nvars, 3);
  assert_eq!((T,I,O), base.tup(I));
  assert_eq!((T,O,I), base.tup(O));
  assert_eq!((1,I,O), base.tup(v1));
  assert_eq!((2,I,O), base.tup(v2));
  assert_eq!((3,I,O), base.tup(v3));
  assert_eq!(I, base.when_hi(3,v3));
  assert_eq!(O, base.when_lo(3,v3))}

#[test] fn test_and() {
  let mut base = BDDBase::new(3);
  let (v1, v2) = (nv(1), nv(2));
  let a = base.and(v1, v2);
  assert_eq!(O,  base.when_lo(1,a));
  assert_eq!(v2, base.when_hi(1,a));
  assert_eq!(O,  base.when_lo(2,a));
  assert_eq!(v1, base.when_hi(2,a));
  assert_eq!(a,  base.when_hi(3,a));
  assert_eq!(a,  base.when_lo(3,a))}

#[test] fn test_xor() {
  let mut base = BDDBase::new(3);
  let (v1, v2) = (nv(1), nv(2));
  let x = base.xor(v1, v2);
  assert_eq!(v2,      base.when_lo(1,x));
  assert_eq!(not(v2), base.when_hi(1,x));
  assert_eq!(v1,      base.when_lo(2,x));
  assert_eq!(not(v1), base.when_hi(2,x));
  assert_eq!(x,       base.when_lo(3,x));
  assert_eq!(x,       base.when_hi(3,x))}



pub fn print_nid(x:NID){ match x {
  I=>print!("I"), O=>print!("O"),
  _ if is_inv(x) => print!("-{}", not(x)), _=> print!("{}", x)}}

pub fn print_tup(n:(NID,NID,NID)){
  print!("("); print_nid(n.0); print!(", "); print_nid(n.1);
  print!(", "); print_nid(n.2); println!(")")}
