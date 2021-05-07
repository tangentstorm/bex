///! test suite generated from doc/scaffold.ipynb

use std::iter::FromIterator;

/// test for diagram #0: independent nodes
#[allow(unused_variables)]
#[test] fn test_scaffold_diagram0() {

    let mut xs = XVHLScaffold::new();
    // arbitrary fake vars for the fake nodes to reference. (these go at the bottom)
    let vn0 = VID::var(0); xs.push(vn0); let vx0 = VID::var(20); xs.push(vx0);
    let vn1 = VID::var(1); xs.push(vn1); let vx1 = VID::var(21); xs.push(vx1);
    let vn2 = VID::var(2); xs.push(vn2); let vx2 = VID::var(22); xs.push(vx2);
    let vn3 = VID::var(3); xs.push(vn3); let vx3 = VID::var(23); xs.push(vx3);
    let vn4 = VID::var(4); xs.push(vn4);
    let vn5 = VID::var(5); xs.push(vn5);
    let vn6 = VID::var(6); xs.push(vn6);
    let vn7 = VID::var(7); xs.push(vn7);
    let vn8 = VID::var(8); xs.push(vn8);
    let vn9 = VID::var(9); xs.push(vn9);

    // variables used in the swap tests. These look "upside down" here
    // because we're pushing them onto a stack. Remember: v starts below w.
    let vt = VID::vir(0); xs.push(vt);
    let vv = VID::vir(1); xs.push(vv);
    let vw = VID::vir(2); xs.push(vw);
    let vz = VID::vir(4); xs.push(vz);
   
    // constructors for default nodes
    assert_eq!(1, xs.vhls.len(), "expecting only XVHL_O at this point");
    let mut node = |v,hi,lo|->XID { xs.add_ref(XVHL{v, hi, lo}, 0, 0) };
    const XO:XID = XID_O;
    const SLOTS:usize = 9;

    // leave some space for the numbered nodes in the diagrams by creating fake nodes:
    // (can't use XID_O because add_ref would overwrite the empty slot)
    let (n1,n2,n3) = (node(vn1,XO,!XO), node(vn2,XO,!XO), node(vn3,XO,!XO));
    let (n4,n5,n6) = (node(vn4,XO,!XO), node(vn5,XO,!XO), node(vn6,XO,!XO));
    let (n7,n8,n9) = (node(vn7,XO,!XO), node(vn8,XO,!XO), node(vn9,XO,!XO));

    // now some fake nodes for the t/z rows to point at when the edges are not defined:
    let (x0,x1,x2,x3) = (node(vx0,XO,!XO), node(vx1,XO,!XO), node(vx2,XO,!XO), node(vx3,XO,!XO));

    // and the default t and z rows themselves:
    let (z0,z1,z2,z3) = (node(vz,x0,!x0), node(vz,x1,!x1), node(vz,x2,!x2), node(vz,x3,!x3));
    let (t0,t1,t2,t3) = (node(vt,x0,!x0), node(vt,x1,!x1), node(vt,x2,!x2), node(vt,x3,!x3));

    // setup code generated from the diagram:

  let (old_xid, old_vhl) = (n2, xs.get(n2).unwrap()); xs.del_node(n2);
  let n2 = xs.add_ref(XVHL{ v:vv, hi:t2, lo:t3 }, 0, 0);
  assert_eq!(n2.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n2.");
  let (old_xid, old_vhl) = (n1, xs.get(n1).unwrap()); xs.del_node(n1);
  let n1 = xs.add_ref(XVHL{ v:vw, hi:t0, lo:t1 }, 0, 0);
  assert_eq!(n1.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n1.");
  let (old_xid, old_vhl) = (z0, xs.get(z0).unwrap()); xs.del_node(z0);
  let z0 = xs.add_ref(XVHL{ v:vz, hi:n1, lo:n2 }, 0, 0);
  assert_eq!(z0.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for z0.");

    // go back and clear out the fake nodes we created earlier:
    let mut i = 1;
    while i <= SLOTS && xs.vhls[i].v.is_vir() { i+=1 }
    while i <= SLOTS { xs.del_node(XID{x:i as i64}); i+=1; }

    // double check that the diagram itself follows the rules.
    xs.validate("setup from diagram");

    // now perform the swap:
    xs.swap(vv);

    // checks generated from the diagram:

  assert_eq!(xs.xids_on_row(vv), HashSet::from_iter(vec![n2]), "row v didn't match expected values!");
  assert_eq!(xs.xids_on_row(vw), HashSet::from_iter(vec![n1]), "row w didn't match expected values!");
 { let x=xs.get(n2).unwrap();
    assert_eq!( x.hi, t2, "wrong .hi for node n2");
    assert_eq!( x.lo, t3, "wrong .hi for node n2");
    assert_eq!(x.v, vv, "wrong variable for node n2: {:?}", n2); }
 { let x=xs.get(n1).unwrap();
    assert_eq!( x.hi, t0, "wrong .hi for node n1");
    assert_eq!( x.lo, t1, "wrong .hi for node n1");
    assert_eq!(x.v, vw, "wrong variable for node n1: {:?}", n1); }
 { let x=xs.get(z0).unwrap();
    assert_eq!( x.hi, n1, "wrong .hi for node z0");
    assert_eq!( x.lo, n2, "wrong .hi for node z0");
    assert_eq!(x.v, vz, "wrong variable for node z0: {:?}", z0); }
  assert_eq!(xs.get_refcount(t2).unwrap(), 1, "bad refcount for node t2 ({:?})!", t2);
  assert_eq!(xs.get_refcount(t3).unwrap(), 1, "bad refcount for node t3 ({:?})!", t3);
  assert_eq!(xs.get_refcount(t0).unwrap(), 1, "bad refcount for node t0 ({:?})!", t0);
  assert_eq!(xs.get_refcount(t1).unwrap(), 1, "bad refcount for node t1 ({:?})!", t1);
  assert_eq!(xs.get_refcount(n1).unwrap(), 1, "bad refcount for node n1 ({:?})!", n1);
  assert_eq!(xs.get_refcount(n2).unwrap(), 1, "bad refcount for node n2 ({:?})!", n2);

}



/// test for diagram #1: garbage collection
#[allow(unused_variables)]
#[test] fn test_scaffold_diagram1() {

    let mut xs = XVHLScaffold::new();
    // arbitrary fake vars for the fake nodes to reference. (these go at the bottom)
    let vn0 = VID::var(0); xs.push(vn0); let vx0 = VID::var(20); xs.push(vx0);
    let vn1 = VID::var(1); xs.push(vn1); let vx1 = VID::var(21); xs.push(vx1);
    let vn2 = VID::var(2); xs.push(vn2); let vx2 = VID::var(22); xs.push(vx2);
    let vn3 = VID::var(3); xs.push(vn3); let vx3 = VID::var(23); xs.push(vx3);
    let vn4 = VID::var(4); xs.push(vn4);
    let vn5 = VID::var(5); xs.push(vn5);
    let vn6 = VID::var(6); xs.push(vn6);
    let vn7 = VID::var(7); xs.push(vn7);
    let vn8 = VID::var(8); xs.push(vn8);
    let vn9 = VID::var(9); xs.push(vn9);

    // variables used in the swap tests. These look "upside down" here
    // because we're pushing them onto a stack. Remember: v starts below w.
    let vt = VID::vir(0); xs.push(vt);
    let vv = VID::vir(1); xs.push(vv);
    let vw = VID::vir(2); xs.push(vw);
    let vz = VID::vir(4); xs.push(vz);
   
    // constructors for default nodes
    assert_eq!(1, xs.vhls.len(), "expecting only XVHL_O at this point");
    let mut node = |v,hi,lo|->XID { xs.add_ref(XVHL{v, hi, lo}, 0, 0) };
    const XO:XID = XID_O;
    const SLOTS:usize = 9;

    // leave some space for the numbered nodes in the diagrams by creating fake nodes:
    // (can't use XID_O because add_ref would overwrite the empty slot)
    let (n1,n2,n3) = (node(vn1,XO,!XO), node(vn2,XO,!XO), node(vn3,XO,!XO));
    let (n4,n5,n6) = (node(vn4,XO,!XO), node(vn5,XO,!XO), node(vn6,XO,!XO));
    let (n7,n8,n9) = (node(vn7,XO,!XO), node(vn8,XO,!XO), node(vn9,XO,!XO));

    // now some fake nodes for the t/z rows to point at when the edges are not defined:
    let (x0,x1,x2,x3) = (node(vx0,XO,!XO), node(vx1,XO,!XO), node(vx2,XO,!XO), node(vx3,XO,!XO));

    // and the default t and z rows themselves:
    let (z0,z1,z2,z3) = (node(vz,x0,!x0), node(vz,x1,!x1), node(vz,x2,!x2), node(vz,x3,!x3));
    let (t0,t1,t2,t3) = (node(vt,x0,!x0), node(vt,x1,!x1), node(vt,x2,!x2), node(vt,x3,!x3));

    // setup code generated from the diagram:

  let (old_xid, old_vhl) = (n2, xs.get(n2).unwrap()); xs.del_node(n2);
  let n2 = xs.add_ref(XVHL{ v:vv, hi:t1, lo:t2 }, 0, 0);
  assert_eq!(n2.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n2.");
  let (old_xid, old_vhl) = (n1, xs.get(n1).unwrap()); xs.del_node(n1);
  let n1 = xs.add_ref(XVHL{ v:vw, hi:t0, lo:n2 }, 0, 0);
  assert_eq!(n1.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n1.");
  let (old_xid, old_vhl) = (z0, xs.get(z0).unwrap()); xs.del_node(z0);
  let z0 = xs.add_ref(XVHL{ v:vz, hi:old_vhl.hi, lo:old_vhl.lo }, 0, 0);
  assert_eq!(z0.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for z0.");

    // go back and clear out the fake nodes we created earlier:
    let mut i = 1;
    while i <= SLOTS && xs.vhls[i].v.is_vir() { i+=1 }
    while i <= SLOTS { xs.del_node(XID{x:i as i64}); i+=1; }

    // double check that the diagram itself follows the rules.
    xs.validate("setup from diagram");

    // now perform the swap:
    xs.swap(vv);

    // checks generated from the diagram:

  assert_eq!(xs.xids_on_row(vv), HashSet::from_iter(vec![]), "row v didn't match expected values!");
  assert_eq!(xs.xids_on_row(vw), HashSet::from_iter(vec![]), "row w didn't match expected values!");
 { let x=xs.get(t2).unwrap();
    assert_eq!(x.v, vt, "wrong variable for node t2: {:?}", t2); }
 { let x=xs.get(t1).unwrap();
    assert_eq!(x.v, vt, "wrong variable for node t1: {:?}", t1); }
 { let x=xs.get(t0).unwrap();
    assert_eq!(x.v, vt, "wrong variable for node t0: {:?}", t0); }
 { let x=xs.get(z0).unwrap();
    assert_eq!(x.v, vz, "wrong variable for node z0: {:?}", z0); }

}



/// test for diagram #2: dependent on one side
#[allow(unused_variables)]
#[test] fn test_scaffold_diagram2() {

    let mut xs = XVHLScaffold::new();
    // arbitrary fake vars for the fake nodes to reference. (these go at the bottom)
    let vn0 = VID::var(0); xs.push(vn0); let vx0 = VID::var(20); xs.push(vx0);
    let vn1 = VID::var(1); xs.push(vn1); let vx1 = VID::var(21); xs.push(vx1);
    let vn2 = VID::var(2); xs.push(vn2); let vx2 = VID::var(22); xs.push(vx2);
    let vn3 = VID::var(3); xs.push(vn3); let vx3 = VID::var(23); xs.push(vx3);
    let vn4 = VID::var(4); xs.push(vn4);
    let vn5 = VID::var(5); xs.push(vn5);
    let vn6 = VID::var(6); xs.push(vn6);
    let vn7 = VID::var(7); xs.push(vn7);
    let vn8 = VID::var(8); xs.push(vn8);
    let vn9 = VID::var(9); xs.push(vn9);

    // variables used in the swap tests. These look "upside down" here
    // because we're pushing them onto a stack. Remember: v starts below w.
    let vt = VID::vir(0); xs.push(vt);
    let vv = VID::vir(1); xs.push(vv);
    let vw = VID::vir(2); xs.push(vw);
    let vz = VID::vir(4); xs.push(vz);
   
    // constructors for default nodes
    assert_eq!(1, xs.vhls.len(), "expecting only XVHL_O at this point");
    let mut node = |v,hi,lo|->XID { xs.add_ref(XVHL{v, hi, lo}, 0, 0) };
    const XO:XID = XID_O;
    const SLOTS:usize = 9;

    // leave some space for the numbered nodes in the diagrams by creating fake nodes:
    // (can't use XID_O because add_ref would overwrite the empty slot)
    let (n1,n2,n3) = (node(vn1,XO,!XO), node(vn2,XO,!XO), node(vn3,XO,!XO));
    let (n4,n5,n6) = (node(vn4,XO,!XO), node(vn5,XO,!XO), node(vn6,XO,!XO));
    let (n7,n8,n9) = (node(vn7,XO,!XO), node(vn8,XO,!XO), node(vn9,XO,!XO));

    // now some fake nodes for the t/z rows to point at when the edges are not defined:
    let (x0,x1,x2,x3) = (node(vx0,XO,!XO), node(vx1,XO,!XO), node(vx2,XO,!XO), node(vx3,XO,!XO));

    // and the default t and z rows themselves:
    let (z0,z1,z2,z3) = (node(vz,x0,!x0), node(vz,x1,!x1), node(vz,x2,!x2), node(vz,x3,!x3));
    let (t0,t1,t2,t3) = (node(vt,x0,!x0), node(vt,x1,!x1), node(vt,x2,!x2), node(vt,x3,!x3));

    // setup code generated from the diagram:

  let (old_xid, old_vhl) = (n2, xs.get(n2).unwrap()); xs.del_node(n2);
  let n2 = xs.add_ref(XVHL{ v:vv, hi:t0, lo:t1 }, 0, 0);
  assert_eq!(n2.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n2.");
  let (old_xid, old_vhl) = (n1, xs.get(n1).unwrap()); xs.del_node(n1);
  let n1 = xs.add_ref(XVHL{ v:vw, hi:n2, lo:t2 }, 0, 0);
  assert_eq!(n1.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n1.");
  let (old_xid, old_vhl) = (z0, xs.get(z0).unwrap()); xs.del_node(z0);
  let z0 = xs.add_ref(XVHL{ v:vz, hi:n1, lo:old_vhl.lo }, 0, 0);
  assert_eq!(z0.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for z0.");

    // go back and clear out the fake nodes we created earlier:
    let mut i = 1;
    while i <= SLOTS && xs.vhls[i].v.is_vir() { i+=1 }
    while i <= SLOTS { xs.del_node(XID{x:i as i64}); i+=1; }

    // double check that the diagram itself follows the rules.
    xs.validate("setup from diagram");

    // now perform the swap:
    xs.swap(vv);

    // checks generated from the diagram:

  assert_eq!(xs.xids_on_row(vv), HashSet::from_iter(vec![n1]), "row v didn't match expected values!");
  assert_eq!(xs.xids_on_row(vw), HashSet::from_iter(vec![n2, n3]), "row w didn't match expected values!");
 { let x=xs.get(n3).unwrap();
    assert_eq!( x.hi, t1, "wrong .hi for node n3");
    assert_eq!( x.lo, t2, "wrong .hi for node n3");
    assert_eq!(x.v, vw, "wrong variable for node n3: {:?}", n3); }
 { let x=xs.get(n2).unwrap();
    assert_eq!( x.hi, t0, "wrong .hi for node n2");
    assert_eq!( x.lo, t2, "wrong .hi for node n2");
    assert_eq!(x.v, vw, "wrong variable for node n2: {:?}", n2); }
 { let x=xs.get(n1).unwrap();
    assert_eq!( x.hi, n2, "wrong .hi for node n1");
    assert_eq!( x.lo, n3, "wrong .hi for node n1");
    assert_eq!(x.v, vv, "wrong variable for node n1: {:?}", n1); }
 { let x=xs.get(z0).unwrap();
    assert_eq!( x.hi, n1, "wrong .hi for node z0");
    assert_eq!(x.v, vz, "wrong variable for node z0: {:?}", z0); }
  assert_eq!(xs.get_refcount(t1).unwrap(), 1, "bad refcount for node t1 ({:?})!", t1);
  assert_eq!(xs.get_refcount(t2).unwrap(), 2, "bad refcount for node t2 ({:?})!", t2);
  assert_eq!(xs.get_refcount(t0).unwrap(), 1, "bad refcount for node t0 ({:?})!", t0);
  assert_eq!(xs.get_refcount(n2).unwrap(), 1, "bad refcount for node n2 ({:?})!", n2);
  assert_eq!(xs.get_refcount(n3).unwrap(), 1, "bad refcount for node n3 ({:?})!", n3);
  assert_eq!(xs.get_refcount(n1).unwrap(), 1, "bad refcount for node n1 ({:?})!", n1);

}



/// test for diagram #3: extra link to #2
#[allow(unused_variables)]
#[test] fn test_scaffold_diagram3() {

    let mut xs = XVHLScaffold::new();
    // arbitrary fake vars for the fake nodes to reference. (these go at the bottom)
    let vn0 = VID::var(0); xs.push(vn0); let vx0 = VID::var(20); xs.push(vx0);
    let vn1 = VID::var(1); xs.push(vn1); let vx1 = VID::var(21); xs.push(vx1);
    let vn2 = VID::var(2); xs.push(vn2); let vx2 = VID::var(22); xs.push(vx2);
    let vn3 = VID::var(3); xs.push(vn3); let vx3 = VID::var(23); xs.push(vx3);
    let vn4 = VID::var(4); xs.push(vn4);
    let vn5 = VID::var(5); xs.push(vn5);
    let vn6 = VID::var(6); xs.push(vn6);
    let vn7 = VID::var(7); xs.push(vn7);
    let vn8 = VID::var(8); xs.push(vn8);
    let vn9 = VID::var(9); xs.push(vn9);

    // variables used in the swap tests. These look "upside down" here
    // because we're pushing them onto a stack. Remember: v starts below w.
    let vt = VID::vir(0); xs.push(vt);
    let vv = VID::vir(1); xs.push(vv);
    let vw = VID::vir(2); xs.push(vw);
    let vz = VID::vir(4); xs.push(vz);
   
    // constructors for default nodes
    assert_eq!(1, xs.vhls.len(), "expecting only XVHL_O at this point");
    let mut node = |v,hi,lo|->XID { xs.add_ref(XVHL{v, hi, lo}, 0, 0) };
    const XO:XID = XID_O;
    const SLOTS:usize = 9;

    // leave some space for the numbered nodes in the diagrams by creating fake nodes:
    // (can't use XID_O because add_ref would overwrite the empty slot)
    let (n1,n2,n3) = (node(vn1,XO,!XO), node(vn2,XO,!XO), node(vn3,XO,!XO));
    let (n4,n5,n6) = (node(vn4,XO,!XO), node(vn5,XO,!XO), node(vn6,XO,!XO));
    let (n7,n8,n9) = (node(vn7,XO,!XO), node(vn8,XO,!XO), node(vn9,XO,!XO));

    // now some fake nodes for the t/z rows to point at when the edges are not defined:
    let (x0,x1,x2,x3) = (node(vx0,XO,!XO), node(vx1,XO,!XO), node(vx2,XO,!XO), node(vx3,XO,!XO));

    // and the default t and z rows themselves:
    let (z0,z1,z2,z3) = (node(vz,x0,!x0), node(vz,x1,!x1), node(vz,x2,!x2), node(vz,x3,!x3));
    let (t0,t1,t2,t3) = (node(vt,x0,!x0), node(vt,x1,!x1), node(vt,x2,!x2), node(vt,x3,!x3));

    // setup code generated from the diagram:

  let (old_xid, old_vhl) = (n2, xs.get(n2).unwrap()); xs.del_node(n2);
  let n2 = xs.add_ref(XVHL{ v:vv, hi:t0, lo:t1 }, 0, 0);
  assert_eq!(n2.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n2.");
  let (old_xid, old_vhl) = (n1, xs.get(n1).unwrap()); xs.del_node(n1);
  let n1 = xs.add_ref(XVHL{ v:vw, hi:n2, lo:t2 }, 0, 0);
  assert_eq!(n1.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n1.");
  let (old_xid, old_vhl) = (z1, xs.get(z1).unwrap()); xs.del_node(z1);
  let z1 = xs.add_ref(XVHL{ v:vz, hi:n2, lo:old_vhl.lo }, 0, 0);
  assert_eq!(z1.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for z1.");
  let (old_xid, old_vhl) = (z0, xs.get(z0).unwrap()); xs.del_node(z0);
  let z0 = xs.add_ref(XVHL{ v:vz, hi:n1, lo:old_vhl.lo }, 0, 0);
  assert_eq!(z0.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for z0.");

    // go back and clear out the fake nodes we created earlier:
    let mut i = 1;
    while i <= SLOTS && xs.vhls[i].v.is_vir() { i+=1 }
    while i <= SLOTS { xs.del_node(XID{x:i as i64}); i+=1; }

    // double check that the diagram itself follows the rules.
    xs.validate("setup from diagram");

    // now perform the swap:
    xs.swap(vv);

    // checks generated from the diagram:

  assert_eq!(xs.xids_on_row(vv), HashSet::from_iter(vec![n2, n1]), "row v didn't match expected values!");
  assert_eq!(xs.xids_on_row(vw), HashSet::from_iter(vec![n3, n4]), "row w didn't match expected values!");
 { let x=xs.get(n4).unwrap();
    assert_eq!( x.hi, t1, "wrong .hi for node n4");
    assert_eq!( x.lo, t2, "wrong .hi for node n4");
    assert_eq!(x.v, vw, "wrong variable for node n4: {:?}", n4); }
 { let x=xs.get(n3).unwrap();
    assert_eq!( x.hi, t0, "wrong .hi for node n3");
    assert_eq!( x.lo, t2, "wrong .hi for node n3");
    assert_eq!(x.v, vw, "wrong variable for node n3: {:?}", n3); }
 { let x=xs.get(n1).unwrap();
    assert_eq!( x.hi, n3, "wrong .hi for node n1");
    assert_eq!( x.lo, n4, "wrong .hi for node n1");
    assert_eq!(x.v, vv, "wrong variable for node n1: {:?}", n1); }
 { let x=xs.get(n2).unwrap();
    assert_eq!( x.hi, t0, "wrong .hi for node n2");
    assert_eq!( x.lo, t1, "wrong .hi for node n2");
    assert_eq!(x.v, vv, "wrong variable for node n2: {:?}", n2); }
 { let x=xs.get(z0).unwrap();
    assert_eq!( x.hi, n1, "wrong .hi for node z0");
    assert_eq!(x.v, vz, "wrong variable for node z0: {:?}", z0); }
 { let x=xs.get(z1).unwrap();
    assert_eq!( x.hi, n2, "wrong .hi for node z1");
    assert_eq!(x.v, vz, "wrong variable for node z1: {:?}", z1); }
  assert_eq!(xs.get_refcount(t1).unwrap(), 2, "bad refcount for node t1 ({:?})!", t1);
  assert_eq!(xs.get_refcount(t2).unwrap(), 2, "bad refcount for node t2 ({:?})!", t2);
  assert_eq!(xs.get_refcount(t0).unwrap(), 2, "bad refcount for node t0 ({:?})!", t0);
  assert_eq!(xs.get_refcount(n3).unwrap(), 1, "bad refcount for node n3 ({:?})!", n3);
  assert_eq!(xs.get_refcount(n4).unwrap(), 1, "bad refcount for node n4 ({:?})!", n4);
  assert_eq!(xs.get_refcount(n1).unwrap(), 1, "bad refcount for node n1 ({:?})!", n1);
  assert_eq!(xs.get_refcount(n2).unwrap(), 1, "bad refcount for node n2 ({:?})!", n2);

}



/// test for diagram #4: both branches dependent on v
#[allow(unused_variables)]
#[test] fn test_scaffold_diagram4() {

    let mut xs = XVHLScaffold::new();
    // arbitrary fake vars for the fake nodes to reference. (these go at the bottom)
    let vn0 = VID::var(0); xs.push(vn0); let vx0 = VID::var(20); xs.push(vx0);
    let vn1 = VID::var(1); xs.push(vn1); let vx1 = VID::var(21); xs.push(vx1);
    let vn2 = VID::var(2); xs.push(vn2); let vx2 = VID::var(22); xs.push(vx2);
    let vn3 = VID::var(3); xs.push(vn3); let vx3 = VID::var(23); xs.push(vx3);
    let vn4 = VID::var(4); xs.push(vn4);
    let vn5 = VID::var(5); xs.push(vn5);
    let vn6 = VID::var(6); xs.push(vn6);
    let vn7 = VID::var(7); xs.push(vn7);
    let vn8 = VID::var(8); xs.push(vn8);
    let vn9 = VID::var(9); xs.push(vn9);

    // variables used in the swap tests. These look "upside down" here
    // because we're pushing them onto a stack. Remember: v starts below w.
    let vt = VID::vir(0); xs.push(vt);
    let vv = VID::vir(1); xs.push(vv);
    let vw = VID::vir(2); xs.push(vw);
    let vz = VID::vir(4); xs.push(vz);
   
    // constructors for default nodes
    assert_eq!(1, xs.vhls.len(), "expecting only XVHL_O at this point");
    let mut node = |v,hi,lo|->XID { xs.add_ref(XVHL{v, hi, lo}, 0, 0) };
    const XO:XID = XID_O;
    const SLOTS:usize = 9;

    // leave some space for the numbered nodes in the diagrams by creating fake nodes:
    // (can't use XID_O because add_ref would overwrite the empty slot)
    let (n1,n2,n3) = (node(vn1,XO,!XO), node(vn2,XO,!XO), node(vn3,XO,!XO));
    let (n4,n5,n6) = (node(vn4,XO,!XO), node(vn5,XO,!XO), node(vn6,XO,!XO));
    let (n7,n8,n9) = (node(vn7,XO,!XO), node(vn8,XO,!XO), node(vn9,XO,!XO));

    // now some fake nodes for the t/z rows to point at when the edges are not defined:
    let (x0,x1,x2,x3) = (node(vx0,XO,!XO), node(vx1,XO,!XO), node(vx2,XO,!XO), node(vx3,XO,!XO));

    // and the default t and z rows themselves:
    let (z0,z1,z2,z3) = (node(vz,x0,!x0), node(vz,x1,!x1), node(vz,x2,!x2), node(vz,x3,!x3));
    let (t0,t1,t2,t3) = (node(vt,x0,!x0), node(vt,x1,!x1), node(vt,x2,!x2), node(vt,x3,!x3));

    // setup code generated from the diagram:

  let (old_xid, old_vhl) = (n3, xs.get(n3).unwrap()); xs.del_node(n3);
  let n3 = xs.add_ref(XVHL{ v:vv, hi:t2, lo:t3 }, 0, 0);
  assert_eq!(n3.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n3.");
  let (old_xid, old_vhl) = (n2, xs.get(n2).unwrap()); xs.del_node(n2);
  let n2 = xs.add_ref(XVHL{ v:vv, hi:t0, lo:t1 }, 0, 0);
  assert_eq!(n2.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n2.");
  let (old_xid, old_vhl) = (n1, xs.get(n1).unwrap()); xs.del_node(n1);
  let n1 = xs.add_ref(XVHL{ v:vw, hi:n2, lo:n3 }, 0, 0);
  assert_eq!(n1.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for n1.");
  let (old_xid, old_vhl) = (z0, xs.get(z0).unwrap()); xs.del_node(z0);
  let z0 = xs.add_ref(XVHL{ v:vz, hi:n1, lo:old_vhl.lo }, 0, 0);
  assert_eq!(z0.raw(), old_xid.raw(), "scaffold failed to reuse empty slot for z0.");

    // go back and clear out the fake nodes we created earlier:
    let mut i = 1;
    while i <= SLOTS && xs.vhls[i].v.is_vir() { i+=1 }
    while i <= SLOTS { xs.del_node(XID{x:i as i64}); i+=1; }

    // double check that the diagram itself follows the rules.
    xs.validate("setup from diagram");

    // now perform the swap:
    xs.swap(vv);

    // checks generated from the diagram:

  assert_eq!(xs.xids_on_row(vv), HashSet::from_iter(vec![n1]), "row v didn't match expected values!");
  assert_eq!(xs.xids_on_row(vw), HashSet::from_iter(vec![n2, n3]), "row w didn't match expected values!");
 { let x=xs.get(n3).unwrap();
    assert_eq!( x.hi, t1, "wrong .hi for node n3");
    assert_eq!( x.lo, t3, "wrong .hi for node n3");
    assert_eq!(x.v, vw, "wrong variable for node n3: {:?}", n3); }
 { let x=xs.get(n2).unwrap();
    assert_eq!( x.hi, t0, "wrong .hi for node n2");
    assert_eq!( x.lo, t2, "wrong .hi for node n2");
    assert_eq!(x.v, vw, "wrong variable for node n2: {:?}", n2); }
 { let x=xs.get(n1).unwrap();
    assert_eq!( x.hi, n2, "wrong .hi for node n1");
    assert_eq!( x.lo, n3, "wrong .hi for node n1");
    assert_eq!(x.v, vv, "wrong variable for node n1: {:?}", n1); }
 { let x=xs.get(z0).unwrap();
    assert_eq!( x.hi, n1, "wrong .hi for node z0");
    assert_eq!(x.v, vz, "wrong variable for node z0: {:?}", z0); }
  assert_eq!(xs.get_refcount(t1).unwrap(), 1, "bad refcount for node t1 ({:?})!", t1);
  assert_eq!(xs.get_refcount(t3).unwrap(), 1, "bad refcount for node t3 ({:?})!", t3);
  assert_eq!(xs.get_refcount(t0).unwrap(), 1, "bad refcount for node t0 ({:?})!", t0);
  assert_eq!(xs.get_refcount(t2).unwrap(), 1, "bad refcount for node t2 ({:?})!", t2);
  assert_eq!(xs.get_refcount(n2).unwrap(), 1, "bad refcount for node n2 ({:?})!", n2);
  assert_eq!(xs.get_refcount(n3).unwrap(), 1, "bad refcount for node n3 ({:?})!", n3);
  assert_eq!(xs.get_refcount(n1).unwrap(), 1, "bad refcount for node n1 ({:?})!", n1);

}



