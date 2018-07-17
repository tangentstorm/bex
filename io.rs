/// binary io for typed vectors
use std::fs::File;
use std::io::prelude::*;

// https://stackoverflow.com/questions/28127165/how-to-convert-struct-to-u8
unsafe fn to_u8s<T: Sized>(p: &T) -> &[u8] {
  ::std::slice::from_raw_parts(
    (p as *const T) as *const u8,
    ::std::mem::size_of::<T>()) }

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
pub fn put<T:Sized>(path:&str, v:&Vec<T>) -> ::std::io::Result<()> {
  let mut f = File::create(path)?;
  f.write_all( unsafe{ slice_to_u8s(v.as_slice()) }) }

/// attempt to parse the file at the specified path as a binary Vec<T>.
pub fn get<T:Sized+Clone>(path:&str) -> ::std::io::Result<Vec<T>> {
  let mut f = File::open(path)?;
  let mut uv:Vec<u8> = Vec::new();
  f.read_to_end(&mut uv).expect("couldn't read file");
  let s:&[T] = unsafe { u8s_to_slice(&uv.as_slice())};
  Ok(s.to_vec()) }
