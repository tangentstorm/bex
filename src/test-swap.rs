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
    let mut ss = SwapSolver::new(0); ss.init(rv);
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

/// test for subbing in two existing variables.
/// This test is also interesting because in the process of running it,
/// one of the variables cancels out.
#[test] fn test_two_old() {
  //   xyz?    z!zx? y%             #  groups: d={}, v={y}, s={}, n={xz}
  // = xyz?   (z!zx? z!zx? z?) y%   # nothing changes for d, reorder src
  // = xyz?  (0!0x? 1!1x? z?) y%
  // = xyz?  (10x? 01x? z?) y%
  // = xyz?  (x!xz?) y%
  // = x(x!xz?)z?
  // = x(x!xz?)z? x(x!xz?)z? z?
  // = x(x!x0?)0? x(x!x1?)1? z?
  // = x (x!x1?) z?
  // = x x z?
  // = x
  check_sub("xyz|xyz|zx|xz", "xyz?", 'y', "z!zx?", "x")}

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
