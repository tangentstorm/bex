//! Optional name registry for nodes in any `Base`.
use std::collections::HashMap;
use crate::{NID, vid::VID};

#[derive(Debug, Default)]
pub struct Names { by_name: HashMap<String, NID> }

impl Names {
  pub fn new() -> Self { Self::default() }
  pub fn tag(&mut self, n: NID, s: impl Into<String>) -> NID {
    self.by_name.insert(s.into(), n); n }
  pub fn get(&self, s: &str) -> Option<NID> {
    self.by_name.get(s).copied() }
  pub fn def(&mut self, s: String, v: VID) -> NID {
    let n = NID::from_vid(v);
    self.tag(n, format!("{}{:?}", s, v)) }
}
