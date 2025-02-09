// generic Base test suite
test_base_consts!(BddBase);
test_base_when!(BddBase);

#[cfg(test)]
use std::{iter::FromIterator, hash::Hash};

#[cfg(test)]
fn hs<T: Eq+Hash>(xs: Vec<T>)->HashSet<T> { <HashSet<T>>::from_iter(xs) }

// basic test suite

#[test] fn test_base() {
  use crate::nid::named::{x0, x1, x2};
  let mut base = BddBase::new();
  assert_eq!((I,O), base.tup(I));
  assert_eq!((O,I), base.tup(O));
  assert_eq!((I,O), base.tup(x0));
  assert_eq!((I,O), base.tup(x1));
  assert_eq!((I,O), base.tup(x2));
  assert_eq!(I, base.when_hi(VID::var(2),x2));
  assert_eq!(O, base.when_lo(VID::var(2),x2))}

#[test] fn test_and() {
  use crate::nid::named::{x0, x1, x2};
  let mut base = BddBase::new();
  let a = base.and(x1, x2);
  assert_eq!(O,  base.when_lo(x1.vid(),a));
  assert_eq!(x2, base.when_hi(x1.vid(),a));
  assert_eq!(O,  base.when_lo(x2.vid(),a));
  assert_eq!(x1, base.when_hi(x2.vid(),a));
  assert_eq!(a,  base.when_hi(x0.vid(),a));
  assert_eq!(a,  base.when_lo(x0.vid(),a))}

#[test] fn test_xor() {
  use crate::nid::named::{x0, x1};
  let mut base = BddBase::new();
  let x = base.xor(x0, x1);
  assert_eq!(x1,  base.when_lo(x0.vid(),x));
  assert_eq!(!x1, base.when_hi(x0.vid(),x));
  assert_eq!(x0,  base.when_lo(x1.vid(),x));
  assert_eq!(!x0, base.when_hi(x1.vid(),x));
  assert_eq!(x,   base.when_lo(VID::var(2),x));
  assert_eq!(x,   base.when_hi(VID::var(2),x))}

// swarm test suite
#[test] fn test_swarm_xor() {
  use crate::nid::named::{x0, x1};
  let mut base = BddBase::new();
  let x = base.xor(x0, x1);
  assert_eq!(x1,  base.when_lo(x0.vid(),x));
  assert_eq!(!x1, base.when_hi(x0.vid(),x));
  assert_eq!(x0,  base.when_lo(x1.vid(),x));
  assert_eq!(!x0, base.when_hi(x1.vid(),x));
  assert_eq!(x,   base.when_lo(VID::var(2),x));
  assert_eq!(x,   base.when_hi(VID::var(2),x))}

#[test] fn test_swarm_and() {
  use crate::nid::named::{x0, x1};
  let mut base = BddBase::new();
  let a = base.and(x0, x1);
  assert_eq!(O,  base.when_lo(x0.vid(),a));
  assert_eq!(x1, base.when_hi(x0.vid(),a));
  assert_eq!(O,  base.when_lo(x1.vid(),a));
  assert_eq!(x0, base.when_hi(x1.vid(),a));
  assert_eq!(a,  base.when_hi(VID::var(2),a));
  assert_eq!(a,  base.when_lo(VID::var(2),a))}

/// slightly harder test case that requires ite() to recurse
#[test] fn test_swarm_ite() {
  use crate::nid::named::{x0, x1, x2};
  let mut base = BddBase::new();
  assert_eq!(vec![0,0,0,0,1,1,1,1], base.tt(x2, 3));
  assert_eq!(vec![0,0,1,1,0,0,1,1], base.tt(x1, 3));
  assert_eq!(vec![0,1,0,1,0,1,0,1], base.tt(x0, 3));
  let x = base.xor(x2, x1);
  assert_eq!(vec![0,0,1,1,1,1,0,0], base.tt(x, 3));
  let a = base.and(x1, x0);
  assert_eq!(vec![0,0,0,1,0,0,0,1], base.tt(a, 3));
  let i = base.ite(x, a, !a);
  assert_eq!(vec![1,1,0,1,0,0,1,0], base.tt(i, 3))}


/// slightly harder test case that requires ite() to recurse
#[test] fn test_swarm_another() {
  let (b, a) = (NID::var(2), NID::var(3));
  let mut base = BddBase::new();
  let anb = base.and(a,!b);
  assert_eq!(vec![0,0,0,0,0,0,0,0,1,1,1,1,0,0,0,0], base.tt(anb, 4));

  let anb_nb = base.xor(anb,!b);
  assert_eq!(vec![1,1,1,1,0,0,0,0,0,0,0,0,0,0,0,0], base.tt(anb_nb, 4));
  let anb2 = base.xor(!b, anb_nb);
  assert_eq!(vec![0,0,0,0,0,0,0,0,1,1,1,1,0,0,0,0], base.tt(anb2, 4));
  assert_eq!(anb, anb2)}

/// Test cases for SolutionIterator
#[test] fn test_bdd_solutions_o() {
  let mut base = BddBase::new();  let mut it = base.solutions(O);
  assert_eq!(it.next(), None, "const O should yield no solutions.") }

#[test] fn test_bdd_solutions_i() {
  let base = BddBase::new();
  let actual:HashSet<usize> = base.solutions_pad(I, 2).map(|r| r.as_usize()).collect();
  assert_eq!(actual, hs(vec![0b00, 0b01, 0b10, 0b11]),
     "const true should yield all solutions"); }

#[test] fn test_bdd_solutions_simple() {
  let base = BddBase::new(); let a = NID::var(0);
  let mut it = base.solutions_pad(a, 1);
  // it should be sitting on first solution, which is a=1
  assert_eq!(it.next().expect("expected solution!").as_usize(), 0b1);
  assert_eq!(it.next(), None);}


#[test] fn test_bdd_solutions_extra() {
  let mut base = BddBase::new();
  let (b, d) = (NID::var(1), NID::var(3));
  // the idea here is that we have "don't care" above, below, and between the used vars:
  let n = base.and(b,d);
  // by default, we ignore the "don't cares" above:
  let actual:Vec<_> = base.solutions(n).map(|r| r.as_usize()).collect();
  assert_eq!(actual, vec![0b1010, 0b1011, 0b1110, 0b1111]);

  // but we can pad this if we prefer:
  let actual:Vec<_> = base.solutions_pad(n, 5).map(|r| r.as_usize()).collect();
  assert_eq!(actual, vec![0b01010, 0b01011, 0b01110, 0b01111,
                          0b11010, 0b11011, 0b11110, 0b11111])}

#[test] fn test_bdd_solutions_xor() {
  let mut base = BddBase::new();
  let (a, b) = (NID::var(0), NID::var(1));
  let n = base.xor(a, b);
  // base.show(n);
  let actual:Vec<usize> = base.solutions_pad(n, 3).map(|x|x.as_usize()).collect();
  let expect = vec![0b001, 0b010, 0b101, 0b110 ]; // bits cba
  assert_eq!(actual, expect); }

#[test] fn test_hilocache_simple() {
  use crate::vhl; // TODO: probably move this to test-vhl.rs?
  let cache = vhl::HiLoCache::default();
  let hl = vhl::HiLo::new(NID::var(5), NID::var(6));
  let x0 = VID::var(0);
  let v0 = VID::vir(0);
  let v1 = VID::vir(1);
  assert!(cache.get_node(v0, hl).is_none());
  let nv0 = cache.insert(v0, hl);
  assert_eq!(nv0, NID::from_vid_idx(v0, 0));

  // I want the following to just work, but it doesn't:
  // let nv1 = state.get_simple_node(v1, hl).expect("nv1");

  let nv1 = cache.insert(v1, hl);
  assert_eq!(nv1, NID::from_vid_idx(v1, 0));

  // this node is "malformed" because the lower number is on top,
  // but the concept should still work:
  let nx0 = cache.insert(x0, hl);
  assert_eq!(nx0, NID::from_vid_idx(x0, 0));}
