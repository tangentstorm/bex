//! Simplification rules for simple boolean operations.
use crate::{nid::{NID, I, O}};

pub fn xor(x:NID, y:NID)->Option<NID> {
  if x == y { Some(O) }
  else if x == O { Some(y) }
  else if x == I { Some(!y) }
  else if y == O { Some(x) }
  else if y == I { Some(!x) }
  else if x == !y { Some(I) }
  else { None }}

pub fn and(x:NID, y:NID)->Option<NID> {
  if x == O || y == O { Some(O) }
  else if x == I || x == y { Some(y) }
  else if y == I { Some(x) }
  else if x == !y { Some(O) }
  else { None }}

pub fn or(x:NID, y:NID)->Option<NID> {
  if x == O { Some(y) }
  else if y == O { Some(x) }
  else if x == I || y == I { Some(I) }
  else if x == y { Some(x) }
  else if x == !y { Some(I) }
  else { None }}
