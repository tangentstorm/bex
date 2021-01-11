// test suite for swap solver. (included at bottom of  swap.rs)

// -- XSDebug ------------------------------------------------------------------

#[test] fn test_xsdebug() {
  let mut xsd = XSDebug::new("abcvw");
  let a = xsd.xid("a");
  let b = xsd.xid("b");
  let v = xsd.xid("v");
  let c = xsd.ite(v,a,b);
  let x = xsd.xid("abv?");
  assert_eq!(c,x);
  let y = xsd.xid("acv?");
  let w = xsd.xid("w");
  let z = xsd.ite(w,x,y);
  assert_eq!(xsd.fmt(z), "abv? acv? w? "); }

// -- XVHLScaffold ------------------------------------------------------------


fn check_swap(old:&str, new:&str) {
  let mut xsd = XSDebug::new("abcdvw");
  let v = xsd.vid('v');
  let x = xsd.xid(old);
  xsd.xs.swap(v);
  assert_eq!(xsd.fmt(x), new.to_string(), "unexpected results after swap.")}

#[test] fn test_scaffold() {
  check_swap("abv? cdv? w?", "acw? bdw? v? ");
  check_swap("abv? acv? w?", "abcw? v? ");
  check_swap("a abv? w?", "aabw? v? "); // TODO: fails with stack overflow?!
 }


// -- SwapSolver --------------------------------------------------------------

#[test] fn test_swap() {
  let nz = NID::vir(3); let z = nz.vid();
  let ny = NID::vir(2); let y = ny.vid();
  let mut s = BddSwapSolver::new(BDDBase::new(0), z);
  let key = s.and(nz, !ny);
  s.rebuild_src();
  // s.src.print();
  assert_eq!(s.src.top_vid(), z, "z=v3 should start out on top");
  assert_eq!(s.src.exvex(key), VHL { v:z, hi:!ny, lo:nid::O },
    "(z ^ !y) should be (z ? !y : O)");
  // println!("key: {}", key);
  let internal = s.src.inen(key);
  s.src.swap(0,1);
  assert_eq!(s.src.top_vid(), y, "y=v2 should be on top now");
  // s.src.print();
  // println!("src.vids: {:?}", s.src.vids);
  // TODO: double check this.
  assert_eq!(s.src.exin(internal), NID::from_vid_idx(s.src.top_vid(),0));
  assert_eq!(s.src.exvin(internal), VHL { v:y, hi:nid::O, lo:nz },
    "after swap (z ^ !y) should be (y ? O : z)"); }

#[test] fn test_scaffold_refs() {
  let x1 = NID::var(1);
  let x0 = NID::var(0);
  let mut s = VHLScaffold::new(x0.vid());
  s.push(x1.vid());
  let (f,_) = s.add_iref(1, HiLo{hi: x0, lo:!x0}, 1);
  let (g,_) = s.add_iref(1, HiLo{hi: !x0, lo:x0}, 1);
  assert_ne!(f,g,"nids for different funtions should be different!");}

/// test for subbing in two existing variables
#[test] fn test_two_old() {
  let nz = NID::vir(4); let z = nz.vid();
  let ny = NID::vir(3); let y = ny.vid();
  let nx = NID::vir(2); let x = nx.vid();
  let nw = NID::vir(1); let w = nw.vid();
  let nv = NID::vir(0); let v = nv.vid();
  let mut s = BddSwapSolver::new(BDDBase::new(0), z);

  // we start with just z on top:     (z)          0 1
  // substitute z -> y ^ x          = (y ^ x)      0 1 ; 1 0     <->   0110 ; 0110
  let key = s.xor(ny, nx);
  let res = s.sub(z, key, nz);
  assert_eq!(vec![x,y], s.dst.vids);
  assert_eq!(s.dst.exvex(res), VHL { v:y, hi:!nx, lo:nx },
    "(y ^ x) should be (y ? !x : x)");

  // substitute y -> w | v          = ((w|v)^x)
  let key = s.or(nw, nv);
  let res = s.sub(y, key, res);
  assert_eq!(vec![x,v,w], s.dst.vids);
  // todo: make this a standard helper method (VHLScaffold::tt3)
  let VHL{ v:_, hi:i, lo:o } = s.dst.exvex(res);
  let VHL{ v:wo, hi:oi, lo:oo } = s.dst.exvex(o);

  // expr should be: w ? (!x) : (v ? !x : x)
  // so: the lo half of the truth table branches on v
  assert_eq!(wo, v, "w.lo should point to branch on v");
  let VHL{ v:_, hi:ooi, lo:ooo } = s.dst.exvex(oo);
  let VHL{ v:_, hi:oii, lo:oio } = s.dst.exvex(oi);

  // and the right hand side has two copies of !x
  let VHL{ v:wi, hi:_, lo:_ } = s.dst.exvex(i);
  assert_eq!(wi, x, "w.hi should point directly at -.x");
  use nid::{I,O};
  let (ioo, ioi, iio, iii) = (I,O,I,O);
  // s.dst.print();
  assert_eq!((ooo, ooi, oio, oii, ioo, ioi, iio, iii ), (O,I,I,O, I,O,I,O));
  assert_eq!(s.dst.exvex(res), VHL { v:w, hi:!nx, lo:NID::from_vid_idx(v,0) },
    "((w|v) ^ x) should be (w ? !x : (v?!x:x)) ");
  // substitute x -> v & w          = ((w|v)^(w&v))
  let key = s.and(nv, nw);
  let res = s.sub(x, key, res);
  // simplification:                = w ^ v
  assert_eq!(s.dst.exvex(res), VHL { v:w, hi:!nv, lo:nv },
    "((w|v) ^ (w&v)) should be (w ? !v : v)");
  assert!(s.dst.vix(x).is_none(), "x({}) should be gone from dst after substitution", x); }

// -- Substitution Logic ------------------------------------------------------

/// test for subbing in two new variables
#[test] fn test_two_new() {
  // a: ast node, v: vir
  let a5 = NID::vir(5); let v5 = a5.vid();
  let a4 = NID::vir(4); let v4 = a4.vid();
  let a2 = NID::vir(2);
  let mut s = BddSwapSolver::new(BDDBase::new(0), v5);
  assert_eq!(v5, s.dst.vids[0], "label v5 should map to x0 after new(v5)");
  let key = s.and(a4, a2);
  let res = s.sub(v5, key, a5);
  // s.dst.print(); //  s.dst.show_named(nid::O, "dst");
  assert_eq!(s.dst.exvex(res), VHL { v:v4, hi:a2, lo:nid::O },
    "(v4 AND v2) should be (v4 ? v2 : O)"); }

/// test for subbing in one new variable
#[test] fn test_one_new() {
  let nz = NID::vir(3); let z = nz.vid();
  let ny = NID::vir(2); let y = ny.vid();
  let nx = NID::vir(1); let x = nx.vid();
  let nw = NID::vir(0); // let w = nw.vid();
  // we start with just z on top:
  let mut s = BddSwapSolver::new(BDDBase::new(0), z);
  // substitute z -> w ^ y:
  let key = s.xor(nw, ny);
  let wy = s.sub(z, key, nz);
  // substitute y -> x & w  (one new var, one old var)
  // so (w ^ y) -> (w ^ (x & w))
  let key = s.and(nx, nw);
  let wxw = s.sub(y, key, wy);
  assert_eq!(s.dst.exvex(wxw), VHL { v:x, hi:!nw, lo:nid::O },
    "(w ^ (x & w)) should be (x ? !w : O)"); }
