//! Helper routines inspired by the APL family of programming languages.
use std;
use std::collections::HashMap;

/// Return the unique items of `xs` (in order of appearance),
/// and a mapping of those items to their indices.
pub fn group<T>(xs: &[T]) -> (Vec<&T>, HashMap<&T,Vec<usize>>)
where T: std::hash::Hash, T: std::cmp::Eq {
  let mut map:HashMap<&T,Vec<usize>> = HashMap::new();
  let mut nub = vec![]; // unique xs, in the order in which they appeared
  for (i,k) in xs.iter().enumerate() {
    let kxs = map.entry(k).or_default();
    nub.push(k); kxs.push(i) }
  (nub, map) }

/// Calculate a permutation vector that sorts array `xs`.
pub fn gradeup<T>(xs: &[T]) -> Vec<usize>
where T: std::cmp::Ord {
  let mut ixs:Vec<(usize,&T)> = xs.iter().enumerate().collect();
  ixs.sort_by_key(|ix|ix.1); ixs.iter().map(|ix|ix.0).collect()}

/// Map the indices in `ys` to the corresponding values from `xs`.
pub fn at<'a,T:Clone>(xs:&'a[T], ys:&'a[usize]) -> Vec<T> {
  ys.iter().map(|&i| xs[i].clone()).collect() }
