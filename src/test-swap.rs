// test suite for swap solver. (included at bottom of  swap.rs)

// -- XSDebug ------------------------------------------------------------------

#[test] fn test_xsdebug() {
  let mut xsd = XSDebug::new("abcvw");
  let (a, b, v) = (xsd.xid("a"), xsd.xid("b"), xsd.xid("v"));
  let x = xsd.xid("abv?");
  assert_eq!(xsd.ite(v,b,a), x);
  let (y,w) = (xsd.xid("acv?"), xsd.xid("w"));
  let z = xsd.ite(w,y,x);
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
  assert_eq!(xsd.xs.tbl(x, None), vec![o,i,i,i, o,o,o,o])}

#[test] fn test_untbl() {
  let mut xsd = XSDebug::new("abc");
  assert_eq!(xsd.run(" 01#"), "a");
  assert_eq!(xsd.run(".10#"), "a!");
  assert_eq!(xsd.run(".0101#"), "a");
  assert_eq!(xsd.run(".0111#"), "a1b? ");
  assert_eq!(xsd.run(".01b#"), "b"); }

// -- SwapSolver --------------------------------------------------------------

/// Mini-test framework.
/// replace v with src in dst, and check that it gives goal
/// vids format is "c|d|s", where:
///    c lists the character names for all variables
///    d lists the initial order of those variables in dst
///    s lists the initial order of those variables in src
fn check_sub(vids:&str, dst_s:&str, v:char, src_s:&str, goal:&str) {

  let mut dst = XSDebug::new("");
  let mut src = XSDebug::new("");
  let mut expected_order = "";

  // global map of all variables for this test
  let mut cv:HashMap<char,usize> = HashMap::new();
  let mut phase = 0;
  for (i,c) in  vids.char_indices() {
    if c == '|' { phase += 1 }
    else { match phase {
      0 => { cv.insert(c, i); },
      1 => dst.var(*cv.get(&c).expect("bad entry in dst vars"), c),
      2 => src.var(*cv.get(&c).expect("bad entry in src vars"), c),
      3 => {
        let mut parts = vids.split('|');
        expected_order = (if c=='=' { parts.next() } else { parts.last() }).unwrap();
        break },
      _ => panic!("too many '|' chars encountered!") }}}

  println!("building dst");
  let dx = dst.xid(dst_s);
  let rv = dst.vid(v);

  println!("building src");
  let sx = src.xid(src_s);

  // perform the substitution
  let (ss, xid) = {
    let mut ss = SwapSolver::new(rv);
    ss.dst = dst.xs; ss.dx = dx;
    ss.src = src.xs; ss.sx = sx;
    let xid = ss.sub();
    (ss, xid)};

  dst.xs = ss.dst; // move result back to the debugger for inspection.
  // all vars should now be in dst.xs, but we copy the names so fmt knows what to call them.
  for (&c, &i) in cv.iter() { if let None = dst.cx.get(&c) { dst.name_var(VID::var(i as u32), c) }}
  assert_eq!(dst.vids(), expected_order, "unexpected vid ordering at end");
  assert_eq!(dst.fmt(xid), dst.run(goal));}

#[test] fn test_sub_simple_0() {
  check_sub("xy|x|y|y", "x", 'x', "y", "y") }

#[test] fn test_sub_simple_1() {
  // goal: 'vxy?   v w %'
  // sets:   sv: w   dv: xy v:v     n: /  s:w d:xy
  // perm:   wvxy > wxvy > xwvy > xwyv > xywv > xyvw
  //   wxy?
  //   wxy? wxy? w?     // decompose on w
  //   0xy? 1xy? w?     // eval w
  //   0xy? 0x!y?! w?   // how fmt displays inverted xids.   !! have format not do this?
  check_sub("wvxy|vxy|w|xyw", "vxy?", 'v', "w", "0xy? 1xy? w?")}

/// test for subbing in two new variables
#[test] fn test_two_new() {
  // # vars: "abxyz"
  // # syntax: x y v %   <---> replace v with y in x
  // xy* --> x0y?   # and
  // xy^ --> x!xy?  # xor
  // xy+ --> 1xy?   # or
  // expect: z    xy* z %  --> xy*
  // expect: abz? xy* z %  --> abx? a y?
  // ab(x0y?) ab(x0y?) (x0y)?
  // ab(x0y?) ab(x0y?) (x0y)? ab(x0y?) ab(x0y?) (x0y)? y?
  // ab(x00?) ab(x00?) (x00)? ab(x01?) ab(x01?) (x01)? y?
  // abx?     abx?      x?    ab0?     ab0?      0? y?
  //                    abx?    ab0?   y?
  // abx? ay?
  check_sub("abzxy|abz|xy|abxy", "abz?", 'z', "x0y?", "abx? ay?")}

/*
/// test for subbing in two existing variables
#[test] fn test_two_old() { //TODO
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
  todo!()}
*/

/// test for subbing in one new variable
#[test] fn test_one_new() {
  //                                   wy^
  //   w!     w    y?                  xw*   y%
  // = w!     w    y?  w0x?  y%
  // = (w!wy? w!wy? w?)  (w0x? w0x?w?) y%   # reorder as yxw
  // = (0!0y? 1!1y? w?)  (00x? 10x?w?) y%
  // = y!yw?  (0x!w?) y%
  // = (0x!w?)! (0x!w?) w?
  // = (0x!0?)! (0x!1?) w?
  // = (0)! (x!) w?
  // = 1x!w?
  // = 0xw?!
  check_sub("wyx|wy|wx|xw", "w!wy?", 'y', "w0x?", "0xw?!")}


/// test for subbing in two new variables
#[test] fn test_nids_two_new() {
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

#[test] fn test_nids_two_old() {
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

/* !! this test fails with the nid version.
      There was a bug in the test that matched the wrong behavior of the nid version.
      I haven't decided whether to try and salvage the nid version
      for comparison or just delete it.
#[test] fn test_nids_one_new() {
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
*/