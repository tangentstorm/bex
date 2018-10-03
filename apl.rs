// apl/j/k primitives for rust
use std;
use std::collections::HashMap;

pub fn group<'a,T>(xs: &'a Vec<T>) -> (Vec<&T>, HashMap<&T,Vec<usize>>)
where T: std::hash::Hash, T: std::cmp::Eq {
  let mut map:HashMap<&T,Vec<usize>> = HashMap::new();
  let mut nub = vec![]; // unique xs, in the order in which they appeared
  for (i,k) in xs.iter().enumerate() {
    let kxs = map.entry(k).or_insert_with(|| vec![]);
    nub.push(k); kxs.push(i) }
  (nub, map) }

// calculate a permutation vector that sorts array a
pub fn gradeup<'a,T>(xs: &'a Vec<T>) -> Vec<usize>
where T: std::cmp::Ord {
  let mut ixs:Vec<(usize,&T)> = xs.iter().enumerate().collect();
  ixs.sort_by_key(|ix|ix.1); ixs.iter().map(|ix|ix.0).collect()}

