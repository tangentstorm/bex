
use crate::{NID, I, O, vid::VID};
use crate::ast::RawASTBase;

#[derive(Debug)]
pub enum NAF {
  Vhl { v: VID, hi:NID, lo:NID }}

pub fn from_packed_ast(ast: &RawASTBase)->NAF {
  NAF::Vhl { v: VID::vir(ast.len() as u32), hi:I, lo:O}}
