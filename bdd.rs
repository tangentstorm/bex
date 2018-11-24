/// A module for efficient implementation of binary decision diagrams.
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

/// A VID uniquely identifies an input variable in the BDD.
pub type VID = u32;
/// An IDX is an index into a vector.
pub type IDX = u32;

/// A BDDNode is a triple consisting of a VID, which references an input variable,
/// and high and low branches, each pointing at other nodes in the BDD. The
/// associated variable's value determines which branch to take.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BDDNode { pub v:VID, pub hi:NID, pub lo:NID } // if|then|else

/// A NID represents a node in the BDD. Here they contain information about
/// the branching variable so that it's easier to break up the BDD
/// into slices based on the branching variable of the nodes.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub struct NID { n: u64 }
pub const INV:u64 = 1<<63;  // is inverted?
pub const VAR:u64 = 1<<62;  // is variable?
pub const T:u64 = 1<<61;    // T: max VID (hack so O/I nodes show up at bottom)
pub const TV:VID = 1<<29;   // same thing but in 32 bits
pub const IDX_MASK:u64 = (1<<32)-1;
pub const O:NID = NID{ n:T };
pub const I:NID = NID{ n:(T|INV) };
#[inline(always)] pub fn is_var(x:NID)->bool { (x.n & VAR) != 0 }
#[inline(always)] pub fn is_inv(x:NID)->bool { (x.n & INV) != 0 }
#[inline(always)] pub fn is_const(x:NID)->bool { (x.n & T) != 0 }
//#[inline(always)] pub fn code(x:NID)->u32 { (x.n & (INV|T) >> 61) as u32 }
#[inline(always)] pub fn idx(x:NID)->usize { (x.n & IDX_MASK) as usize }
#[inline(always)] pub fn var(x:NID)->VID { ((x.n & !(INV|VAR)) >> 32) as VID}
#[inline(always)] pub fn not(x:NID)->NID { NID { n:x.n^INV } }
#[inline(always)] pub fn nv(v:VID)->NID { NID { n:((v as u64) << 32)|VAR }}
#[inline(always)] pub fn nvi(v:VID,i:IDX)->NID { NID{ n:((v as u64) << 32) + i as u64 }}

impl fmt::Display for NID {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if is_const(*self) { if is_inv(*self) { write!(f, "I") } else { write!(f, "O") } }
    else { if is_inv(*self) { write!(f, "¬")?; }
           if is_var(*self) { write!(f, "x{}", var(*self)) }
           else { write!(f, "@[x{}:{}]", var(*self), idx(*self)) } }}}

impl fmt::Debug for NID { // for test suite output
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self) }}

/// A Norm is an enum to represent a normalized form of a given f,g,h triple
#[derive(Debug)]
pub enum Norm {
  Nid(NID),
  Tup(NID, NID, NID),
  Not(NID, NID, NID)}


/// A BDD Base contains any number of BDD structures, and various caches
/// related to calculating nodes.
#[derive(Debug, Serialize, Deserialize)]
pub struct BDDBase {
  nvars: usize,
  // nbits:usize,
  bits: Vec<BDDNode>,
  pub tags: HashMap<String, NID>,
  /// variable-specific memoization. These record (v,lo,hi) lookups.
  vmemo: Vec<FnvHashMap<(NID, NID),NID>>,
  /// arbitrary memoization. These record normalized (f,g,h) lookups,
  /// and are indexed at three layers: v,f,(g h); where v is the
  /// branching variable.
  xmemo: Vec<FnvHashMap<NID, FnvHashMap<(NID,NID), NID>>> }




impl BDDBase {

  pub fn new(nvars:usize)->BDDBase {
    // the vars are 1-indexed, because node 0 is ⊥ (false)
    let bits = vec![BDDNode{v:TV,hi:O,lo:I}]; // node 0 is ⊥
    BDDBase{nvars:nvars, bits:bits,
            vmemo:(0..nvars).map(|_| FnvHashMap::default()).collect(),
            xmemo:(0..nvars).map(|_| FnvHashMap::default()).collect(),
            tags:HashMap::new()}}

  pub fn nvars(&self)->usize { self.nvars }

  pub fn tag(&mut self, s:String, n:NID) { self.tags.insert(s, n); }
  pub fn get(&self, s:&String)->Option<NID> { Some(*self.tags.get(s)?) }

  #[inline]
  pub fn bdd(&self, n:NID)->BDDNode {
    // bdd for var x still has huge number for the v
    if is_var(n) {
      if is_inv(n) { BDDNode{v:var(n), lo:I, hi:O }}
      else { BDDNode{v:var(n), lo:O, hi:I } }}
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


  // public node constructors

  pub fn and(&mut self, x:NID, y:NID)->NID { self.ite(x,  y, O) }
  pub fn xor(&mut self, x:NID, y:NID)->NID { self.ite(x, not(y), y) }
  pub fn  or(&mut self, x:NID, y:NID)->NID { self.ite(x, I, y) }
  pub fn  gt(&mut self, x:NID, y:NID)->NID { self.ite(x, not(y), O) }
  pub fn  lt(&mut self, x:NID, y:NID)->NID { self.ite(x, O, y) }

  /// nid of y when x is high
  #[inline] pub fn when_hi(&mut self, x:VID, y:NID)->NID {
    match var(y).cmp(&x) {
      Ordering::Greater => y,  // y independent of x, so no change. includes yv = I
      Ordering::Equal => self.tup(y).1, // x ∧ if(x,th,_) → th
      Ordering::Less => {      // y may depend on x, so recurse.
        let (yv, yt, ye) = self.tup(y);
        let (th,el) = (self.when_hi(x,yt), self.when_hi(x,ye));
        self.ite(nv(yv), th, el) }}}

  /// nid of y when x is lo
  #[inline] pub fn when_lo(&mut self, x:VID, y:NID)->NID {
    match var(y).cmp(&x) {
      Ordering::Greater => y,  // y independent of x, so no change. includes yv = I
      Ordering::Equal => self.tup(y).2, // ¬x ∧ if(x,_,el) → el
      Ordering::Less => {   // y may depend on x, so recurse.
        let (yv, yt, ye) = self.tup(y);
        let (th,el) = (self.when_lo(x,yt), self.when_lo(x,ye));
        self.ite(nv(yv), th, el) }}}


  /// is n the nid of a variable?
  pub fn is_var(&self, n:NID)->bool { return is_var(n) }

  /// is it possible x depends on y?
  /// the goal here is to avoid exploring a subgraph if we don't have to.
  #[inline]
  pub fn might_depend(&mut self, x:NID, y:VID)->bool {
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

// all-purpose node creation/lookup

  #[inline] pub fn ite(&mut self, f:NID, g:NID, h:NID)->NID {
    // println!("ite({},{},{})", f,g,h);
    let norm = self.norm(f,g,h);
    match norm {
      Norm::Nid(x) => x,
      Norm::Tup(x,y,z) => self.ite_norm(x,y,z),
      Norm::Not(x,y,z) => not(self.ite_norm(x,y,z)) }}

  /// load the memoized NID if it exists
  #[inline] fn get_norm_memo<'a>(&'a self, v:VID, f:NID, g:NID, h:NID) -> Option<&'a NID> {
    if is_var(f) { self.vmemo[var(f) as usize].get(&(g,h)) }
    else { self.xmemo[v as usize].get(&f).map_or(None, |fmemo| fmemo.get(&(g,h))) }}

  #[inline] fn ite_norm(&mut self, f:NID, g:NID, h:NID)->NID {
    // !! this is one of the most time-consuming bottlenecks, so we inline a lot.
    // this should only bec called from ite() on pre-normalized triples
    let v = min(var(f), min(var(g), var(h)));
    match self.get_norm_memo(v, f, g, h) {
      Some(&n) => n,
      None => {
        let new_nid = {
          macro_rules! branch { ($meth:ident) => {{
            let i = self.$meth(v,f);
            if is_const(i) { if is_inv(i) { self.$meth(v,g) } else { self.$meth(v,h) }}
            else { let (t,e) = (self.$meth(v,g), self.$meth(v,h)); self.ite(i,t,e) }}}}
          let (hi,lo) = (branch!(when_hi), branch!(when_lo));
          if hi == lo {hi} else {
            let hilo = (hi,lo);
            match self.vmemo[v as usize].get(&hilo) {
              Some(&n) => n,
              None => {
                let res = nvi(v, self.bits.len() as IDX);
                self.vmemo[v as usize].insert(hilo, res);
                self.bits.push(BDDNode{v:v, hi:hi, lo:lo});
                res }}}};
        if !is_var(f) { // now add the triple to the generalized memo store
          let mut hm = self.xmemo[v as usize].entry(f)
            .or_insert_with(|| FnvHashMap::default());
          hm.insert((g,h), new_nid); }
        new_nid }}}

  /// choose normal form for writing this node. Algorithm based on:
  /// "Efficient Implementation of a BDD Package"
  /// http://www.cs.cmu.edu/~emc/15817-f08/bryant-bdd-1991.pdf
  pub fn norm(&self, f0:NID, g0:NID, h0:NID)->Norm {
    let mut f = f0; let mut g = g0; let mut h = h0;
    // rustc doesn't do tail call optimization, so we'll do it ourselves.
    macro_rules! bounce { ($x:expr,$y:expr,$z:expr) => {{
      // !! NB. can't set f,g,h directly because we might end up with e.g. `f=g;g=f;`
      let xx=$x; let yy=$y; let zz=$z;  f=xx; g=yy; h=zz; }}}
    loop { match (f,g,h) {
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
    self.vmemo = other.vmemo;
    self.xmemo = other.xmemo;
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

} // end impl BDDBase


// basic test suite

#[test] fn test_nids() {
  assert_eq!(O, NID{n:0x2000000000000000u64});
  assert_eq!(I, NID{n:0xa000000000000000u64});
  assert_eq!(nv(0), NID{n:0x4000000000000000u64});
  assert_eq!(nv(1), NID{n:0x4000000100000000u64});
  assert_eq!(nvi(0,0), NID{n:0x0000000000000000u64});
  assert_eq!(nvi(1,0), NID{n:0x0000000100000000u64}); }

#[test] fn test_base() {
  let mut base = BDDBase::new(3);
  let (v1, v2, v3) = (nv(1), nv(2), nv(3));
  assert_eq!(base.nvars, 3);
  assert_eq!((TV,I,O), base.tup(I));
  assert_eq!((TV,O,I), base.tup(O));
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
