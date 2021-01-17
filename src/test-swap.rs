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
  let (v, x) = (xsd.vid('v'), xsd.xid(old));
  xsd.xs.swap(v);
  assert_eq!(xsd.fmt(x), new.to_string(), "unexpected results after swap.")}

#[test] fn test_swap() {
  check_swap("abv? cdv? w?", "acw? bdw? v? ");
  check_swap("abv? acv? w?", "abcw? v? ");
  check_swap("a abv? w?", "aabw? v? ");
  check_swap("abv? b w?", "abw? bv? "); }

#[test] fn test_tbl() {
  let mut xsd = XSDebug::new("abc");
  let x = xsd.xid("a 1 b? 0 c?");
  let o = XID_O; let i = XID_I;
  assert_eq!(xsd.xs.tbl(x, None), vec![o,o,o,o, i,i, o, i])}


// -- SwapSolver --------------------------------------------------------------

/// test for subbing in two new variables
#[test] fn test_two_new() {
  // # vars: "abxyz"
  // # syntax: x y v %   <---> replace v with y in x
  // xy* --> x0y?   # and
  // xy^ --> x!xy?  # xor
  // xy+ --> 1xy?   # or
  // expect: z    xy* z %  --> xy*
  // expect: abz? xy* z %  --> abx? b y?

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

/// test for subbing in two existing variables
#[test] fn test_two_old() {
  //   stack                     input
  //                             xy^
  //   x!xy?                     vw+    y%       # x^(v+w)
  //   x!   x!   xv?    w?       vw*    x%       # (v*w)^(v+w)
  // = vw*! vw*! vw* v? w?    # x-> (vw*)
  // = vw*! 1w*! 0w* v? w?    # fill in v
  // = vw*! w! 0 v? w?        # simplify const exprs
  // = v1*! 0! 0 v? w?        # fill in w
  // = v!   1 0 v?  w?        # simplify
  // = v!   v  w?             # simplify

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

/// test for subbing in one new variable
#[test] fn test_one_new() {
  //                                   wy^
  //   w!     w    y?                  xw*   y%
  // = w!     w    y?  x0w?  y%
  // = w!     w    x0w??            # replace y
  // = (w!w x0w??) (w!w x0w??) x?   # decompose on x
  // = (w!w 10w??) (w!w 00w??) x?   # subst x
  // = (w!w   w?)  (w!w   0?)  x?   # simplify 10w?->w  00w?->0
  // = (1!0   w?)  w  x?            # distribute w? on left,  apply 0? on right
  // = 00w?  w  x?                  # apply ! to 1
  // = 0 w x?                       # final answer
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
  assert_eq!(s.dst.exvex(wxw), VHL { v:x, hi:nid::O, lo:nw },
    "(w ^ (x & w)) should be (x ? O : w)"); }
