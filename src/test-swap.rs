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

// -- VhlScaffold ------------------------------------------------------------

#[cfg(test)]
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
  let mut xsd = XSDebug::new("abcd");
  let x = xsd.xid("a 1 b? 0 c?");
  let o = crate::nid::O; let i = crate::nid::I;
  assert_eq!(xsd.xs.tbl(x, None), vec![o,i,i,i, o,o,o,o]);
  let a = xsd.xid("a");
  assert_eq!(xsd.xs.tbl(x, Some(VID::var(0))), vec![a,i,o,o]);
  let y = xsd.xid("a 1 b?");
  assert_eq!(xsd.xs.tbl(x, Some(VID::var(1))), vec![y,o]);
  assert_eq!(xsd.xs.tbl(x, Some(VID::var(2))), vec![x]);
  assert_eq!(xsd.xs.tbl(x, Some(VID::var(3))), vec![x]);}

#[test] fn test_tbl_inv() {
  let mut xsd = XSDebug::new("abcd");
  let x = xsd.xid("a 1 b? 0 c?");
  let o = crate::nid::O; let i = crate::nid::I;
  assert_eq!(xsd.xs.tbl(!x, None), vec![i,o,o,o, i,i,i,i]);
  let a = xsd.xid("a");
  assert_eq!(xsd.xs.tbl(!x, Some(VID::var(0))), vec![!a,o,i,i]);
  let y = xsd.xid("a 1 b?");
  assert_eq!(xsd.xs.tbl(!x, Some(VID::var(1))), vec![!y,i]);
  assert_eq!(xsd.xs.tbl(!x, Some(VID::var(2))), vec![!x]);
  assert_eq!(xsd.xs.tbl(!x, Some(VID::var(3))), vec![!x]);}

#[test] fn test_tbl_skip() {
  // this bdd skips over the 'b' row
  let mut xsd = XSDebug::new("abc");
  let x = xsd.xid("a a! c?");
  let o = crate::nid::O; let i = crate::nid::I;
  assert_eq!(xsd.xs.tbl(x, None), vec![o,i,o,i, i,o,i,o]);}

#[test] fn test_untbl() {
  let mut xsd = XSDebug::new("abc");
  assert_eq!(xsd.run(" 01#"), "a");
  assert_eq!(xsd.run(".10#"), "a!");
  assert_eq!(xsd.run(".0101#"), "a");
  assert_eq!(xsd.run(".0111#"), "a1b? ");
  assert_eq!(xsd.run(".01b#"), "b"); }

#[test] fn test_untbl_base() {
  let mut xsd = XSDebug::new("abc");
  assert_eq!(xsd.run(" 01b#"), "b");
  assert_eq!(xsd.run(".10b#"), "b!");
  assert_eq!(xsd.run(".0101b#"), "b");
  assert_eq!(xsd.run(".0111b#"), "b1c? ");
  assert_eq!(xsd.run(".01c#"), "c"); }

// -- SwapSolver --------------------------------------------------------------

/// Mini-test framework.
/// replace v with src in dst, and check that it gives goal
/// vids format is "c|d|s", where:
///    c lists the character names for all variables
///    d lists the initial order of those variables in dst
///    s lists the initial order of those variables in src
#[cfg(test)]
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
    let mut ss = SwapSolver::new(); ss.init(rv);
    ss.dst = dst.xs; ss.dx = dx;
    ss.src = src.xs; ss.sx = sx;
    let xid = ss.sub();
    (ss, xid)};

  dst.xs = ss.dst; // move result back to the debugger for inspection.
  // all vars should now be in dst.xs, but we copy the names so fmt knows what to call them.
  for (&c, &i) in cv.iter() { if !dst.cv.contains_key(&c) { dst.name_var(VID::var(i as u32), c) }}
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
  // !! if the final order breaks on this test due to a regroup() change, it's okay: z isn't used.
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

// -- wtov ---------------------------------------------------------------------

#[test] fn check_wtov_simple() {
  let v = NID::ixn(1);
  let w = NID::ixn(2);
  let io = crate::vhl::HiLo::new(nid::I, nid::O);
  let mut ru = VhlRow::new(); ru.hm.insert(io, IxRc{ ix:v, irc: 1, erc:0 });
  let mut rd = VhlRow::new(); rd.hm.insert(io, IxRc{ ix:w, irc: 1, erc:0 });
  let mut worker = SwapWorker::new(WID::default());
  let res = worker.set_ru(VID::var(0), ru).set_rd(VID::var(1), rd).gather_umovs();
  assert_eq!(0, res.len());}

#[test] fn check_swap_merge() {
  // the point here is that utuu! becomes uutu! after the swap
  // so refcount of u should drop by 1.
  // TODO: assert that the refcount of u actually drops by 1.
  let mut xsd = XSDebug::new("tuvw");
  let top = xsd.xid("utv? uu!v? w?");
  let v = xsd.cv[&'v'];
  xsd.xs.swap(v);
  assert_eq!(xsd.fmt(top), "utu!w? v? ")}

#[test] fn test_fun_tbl() {
  use crate::ops; let o = crate::nid::O; let i = crate::nid::I;
  let result = fun_tbl(ops::AND.to_nid());
  assert_eq!(result, vec![o,o,o,i])}


// -- SwapSolver regroup tests ------------------------------------------------

#[cfg(test)] macro_rules! s {
  { } => { HashSet::new() };
  {$( $x:expr ),+ } => {{ let mut tmp = HashSet::new(); $( tmp.insert($x); )* tmp }};}
#[cfg(test)] macro_rules! d {
  { } => { HashMap::new() };
  {$( $k:ident : $v:expr ),+ }=> {{ let mut tmp = HashMap::new(); $( tmp.insert($k,$v); )* tmp }};}
// Test for plan_regroup disabled - uses original algorithm with complex cursor logic
// #[test] fn test_plan_regroup() { ... }

/// Regression test for bug #12: reordering operation does not complete
/// This test creates a simpler permutation that exercises the algorithm.
#[test] fn test_regroup_bug_12_regression() {
  // Create a scaffold with 4 variables
  let mut xs = VhlScaffold::new();
  let vars: Vec<VID> = (0..4).map(|i| VID::var(i)).collect();

  // Push variables in order [0,1,2,3]
  for &v in &vars {
    xs.push(v);
    // Add a simple node for each variable so the row isn't empty
    xs.add(v, nid::I, nid::O, true);
  }

  // Target groups: [{2,0}, {3,1}]
  // After sorting by vid_ix: [0,2], [1,3]
  // Expected final order: [0,2,1,3]
  let groups = vec![
    s![vars[2], vars[0]],
    s![vars[3], vars[1]]
  ];

  // This tests swapping where variables need to pass through each other
  xs.regroup(groups);

  // Verify the final ordering is correct
  let expected_order = vec![vars[0], vars[2], vars[1], vars[3]];
  assert_eq!(xs.vids, expected_order,
    "Bug #12: regroup() did not achieve the expected variable ordering");
}

/// Another regression test for bug #12 with an interleaved pattern
/// This pattern requires many variables to "pass through" each other
#[test] fn test_regroup_bug_12_interleaved() {
  let mut xs = VhlScaffold::new();
  let vars: Vec<VID> = (0..8).map(|i| VID::var(i)).collect();

  // Push variables in order [0,1,2,3,4,5,6,7]
  for &v in &vars {
    xs.push(v);
    xs.add(v, nid::I, nid::O, true);
  }

  // Target groups: [{7,5,3,1}, {6,4,2,0}]
  // After sorting by vid_ix: [1,3,5,7], [0,2,4,6]
  // Expected final order: [1,3,5,7,0,2,4,6]
  // This is a particularly challenging pattern because odd and even indices swap places
  let groups = vec![
    s![vars[7], vars[5], vars[3], vars[1]],
    s![vars[6], vars[4], vars[2], vars[0]]
  ];

  xs.regroup(groups);

  let expected_order = vec![vars[1], vars[3], vars[5], vars[7], vars[0], vars[2], vars[4], vars[6]];
  assert_eq!(xs.vids, expected_order,
    "Bug #12: regroup() failed on interleaved pattern");
}

/// Unit tests for the should_swap function
/// Tests all the edge cases of the swap decision logic
#[test] fn test_should_swap_both_moving_up() {
  // When both variables are moving up, should NOT swap (prevents infinite loop)
  let mut xs = VhlScaffold::new();
  let v0 = VID::var(0);
  let v1 = VID::var(1);
  let v2 = VID::var(2);
  let v3 = VID::var(3);

  // Push in order [v0, v1, v2, v3] (bottom to top)
  for &v in &[v0, v1, v2, v3] {
    xs.push(v);
    xs.add(v, nid::I, nid::O, true);
  }

  // Plan: v0 wants to go from 0→2, v1 wants to go from 1→3
  // Both moving up
  let mut plan = HashMap::new();
  plan.insert(v0, 2);
  plan.insert(v1, 3);

  // v0 (at 0, wants 2) trying to swap with v1 (at 1, wants 3) - both up
  assert!(!xs.should_swap(v0, v1, &plan),
    "Both moving up: should NOT swap");
}

#[test] fn test_should_swap_both_moving_down() {
  // When both variables are moving down, should NOT swap
  let mut xs = VhlScaffold::new();
  let v0 = VID::var(0);
  let v1 = VID::var(1);
  let v2 = VID::var(2);
  let v3 = VID::var(3);

  for &v in &[v0, v1, v2, v3] {
    xs.push(v);
    xs.add(v, nid::I, nid::O, true);
  }

  // Plan: v2 wants to go from 2→0, v3 wants to go from 3→1
  // Both moving down
  let mut plan = HashMap::new();
  plan.insert(v2, 0);
  plan.insert(v3, 1);

  // v2 (at 2, wants 0) trying to swap with v3 (at 3, wants 1) - both down
  assert!(!xs.should_swap(v2, v3, &plan),
    "Both moving down: should NOT swap");
}

#[test] fn test_should_swap_opposite_directions_improves() {
  // When moving opposite directions and swap improves distance, should swap
  let mut xs = VhlScaffold::new();
  let v0 = VID::var(0);
  let v1 = VID::var(1);
  let v2 = VID::var(2);
  let v3 = VID::var(3);

  for &v in &[v0, v1, v2, v3] {
    xs.push(v);
    xs.add(v, nid::I, nid::O, true);
  }

  // Plan: v1 wants to go from 1→3 (up), v2 wants to go from 2→0 (down)
  // Opposite directions, swap helps both
  let mut plan = HashMap::new();
  plan.insert(v1, 3);
  plan.insert(v2, 0);

  // v1 at 1 wants 3 (distance 2), v2 at 2 wants 0 (distance 2) - total 4
  // After swap: v1 at 2 wants 3 (distance 1), v2 at 1 wants 0 (distance 1) - total 2
  assert!(xs.should_swap(v1, v2, &plan),
    "Opposite directions with improved distance: should swap");
}

#[test] fn test_should_swap_opposite_directions_no_improvement() {
  // When moving opposite directions but swap doesn't improve distance, should NOT swap
  let mut xs = VhlScaffold::new();
  let v0 = VID::var(0);
  let v1 = VID::var(1);
  let v2 = VID::var(2);
  let v3 = VID::var(3);

  for &v in &[v0, v1, v2, v3] {
    xs.push(v);
    xs.add(v, nid::I, nid::O, true);
  }

  // Plan: v0 wants to go from 0→1 (up by 1), v3 wants to go from 3→2 (down by 1)
  // But they're not adjacent, so let's test v1 and v2 with targets that don't help
  // v1 at 1 wants 2 (up), v2 at 2 wants 1 (down)
  // Before: v1 distance 1, v2 distance 1, total 2
  // After swap: v1 at 2 wants 2 (distance 0), v2 at 1 wants 1 (distance 0), total 0
  // This actually improves! Let me construct a case that doesn't improve.

  // v1 at 1 wants 3 (distance 2), v2 at 2 wants 3 (distance 1)
  // Wait, that's both moving up.

  // Let's do: v1 at 1 wants 0 (down, distance 1), v2 at 2 wants 3 (up, distance 1), total 2
  // After swap: v1 at 2 wants 0 (distance 2), v2 at 1 wants 3 (distance 2), total 4
  // Swap makes it worse!
  let mut plan = HashMap::new();
  plan.insert(v1, 0);  // v1 wants to go down
  plan.insert(v2, 3);  // v2 wants to go up

  assert!(!xs.should_swap(v1, v2, &plan),
    "Opposite directions but swap increases distance: should NOT swap");
}

#[test] fn test_should_swap_vd_staying() {
  // When vd is staying (not in plan or at target), should allow vu to pass
  let mut xs = VhlScaffold::new();
  let v0 = VID::var(0);
  let v1 = VID::var(1);
  let v2 = VID::var(2);

  for &v in &[v0, v1, v2] {
    xs.push(v);
    xs.add(v, nid::I, nid::O, true);
  }

  // Plan: only v0 wants to move up to 2
  // v1 is not in the plan (staying)
  let mut plan = HashMap::new();
  plan.insert(v0, 2);

  // v0 at 0 wants 2 (moving up), v1 at 1 not in plan (staying)
  assert!(xs.should_swap(v0, v1, &plan),
    "vd staying: should allow vu to pass through");
}

#[test] fn test_should_swap_vu_staying() {
  // When vu is staying but vd is moving, should allow swap
  let mut xs = VhlScaffold::new();
  let v0 = VID::var(0);
  let v1 = VID::var(1);
  let v2 = VID::var(2);

  for &v in &[v0, v1, v2] {
    xs.push(v);
    xs.add(v, nid::I, nid::O, true);
  }

  // Plan: only v1 wants to move down to 0
  // v0 is not in the plan (staying)
  let mut plan = HashMap::new();
  plan.insert(v1, 0);

  // v0 at 0 not in plan (staying), v1 at 1 wants 0 (moving down)
  assert!(xs.should_swap(v0, v1, &plan),
    "vu staying, vd moving: should allow swap");
}

#[test] fn test_should_swap_both_staying() {
  // When both are staying (neither in plan), directions are both 0
  // This hits the "same direction" check but with dir=0
  let mut xs = VhlScaffold::new();
  let v0 = VID::var(0);
  let v1 = VID::var(1);

  for &v in &[v0, v1] {
    xs.push(v);
    xs.add(v, nid::I, nid::O, true);
  }

  // Empty plan - both staying
  let plan = HashMap::new();

  // Both at their "target" (current position), dir_vu = 0, dir_vd = 0
  // The code checks dir_vd == 0 first and returns true
  assert!(xs.should_swap(v0, v1, &plan),
    "Both staying: should allow (vd==0 case triggers first)");
}

