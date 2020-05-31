/// binary io for hashmap<String,NID> and typed vectors
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use std::{collections::HashMap, hash::BuildHasher};


// these functions treat typed slices as raw bytes, making them easier to read/write
// https://stackoverflow.com/questions/28127165/how-to-convert-struct-to-u8

// adapted from the above, to deal with a slice:
unsafe fn slice_to_u8s<T: Sized>(p: &[T]) -> &[u8] {
  ::std::slice::from_raw_parts(
    (p.as_ptr()) as *const u8,
    ::std::mem::size_of::<T>() * p.len()) }

unsafe fn u8s_to_slice<T: Sized>(p: &[u8]) -> &[T] {
  ::std::slice::from_raw_parts(
    (p.as_ptr()) as *const T,
    p.len() / ::std::mem::size_of::<T>()) }


/// write the vector, as bytes, to a file at the specified path.
pub fn put<T:Sized>(path:&str, v:&[T]) -> ::std::io::Result<()> {
  let mut f = File::create(path)?;
  f.write_all( unsafe{ slice_to_u8s(v) }) }

/// attempt to parse the file at the specified path as a binary Vec<T>.
pub fn get<T:Sized+Clone>(path:&str) -> ::std::io::Result<Vec<T>> {
  let mut f = File::open(path)?;
  let mut uv:Vec<u8> = Vec::new();
  f.read_to_end(&mut uv).expect("couldn't read file");
  let s:&[T] = unsafe { u8s_to_slice(&uv.as_slice())};
  Ok(s.to_vec()) }


/// save a hashmap
pub fn put_map<S:BuildHasher>(path:&str, m:&HashMap<String,usize,S>) -> ::std::io::Result<()> {
  let mut f = File::create(path)?;
  for (k,v) in m.iter() { writeln!(&mut f, "{},{}", k, v)? }
  Ok(())}

/// load a hashmap
pub fn get_map(path:&str) -> ::std::io::Result<HashMap<String,usize>> {
  let mut m = HashMap::new();
  let f = File::open(path)?; let r = BufReader::new(&f);
  for line in r.lines() {
    let line = line.unwrap();
    let v:Vec<&str> = line.split(',').collect();
    m.insert(v[0].to_string(), v[1].parse::<usize>().unwrap()); }
  Ok(m)}
