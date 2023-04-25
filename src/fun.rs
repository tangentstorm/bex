//! The 'Fun' trait for dealing with boolean Functions
use crate::NID;

pub trait Fun<Ix=u8> : Sized {
  fn arity(&self)->Ix;
  /// partially apply the function to the given bit, resulting in a new function
  fn when(&self, bit:Ix, val:bool)->Self;
  /// rewrite the function when two inputs are the same
  fn when_same(&self, bit0:Ix, bit1:Ix)->Self;
  /// rewrite the function when two inputs are different
  fn when_diff(&self, bit0:Ix, bit1:Ix)->Self;
  /// rewrite the function when the given input bits are flipped. (bits is a bitset)
  fn when_flipped(&self, bits:Ix)->Self;
  /// rewrite the function when the given input bits are flipped (bits is vector of indices)
  fn when_flipped_vec(&self, _bits:&[Ix])->Self { todo!("Fun.when_flipped_vec") }
  /// rewrite the function with the given input bit swapped with its upstairs (left) neighbor.
  /// for example, `(f[abc]=a<(b^c)).when_lifted(a)` becomes: `f[bac]` or `g[abc]=b<(a^c)`
  fn when_lifted(&self, bit:Ix)->Self;
  /// apply the function to the given input bits
  fn apply(&self, _args:&[NID])->(Self, Vec<NID>) { todo!("Fun.apply") } }
