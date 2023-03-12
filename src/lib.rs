//! A crate for working with boolean expressions.

#![allow(clippy::many_single_char_names)]

#[macro_use] extern crate log;
extern crate bincode;
#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate hashbrown;
extern crate simplelog;

pub mod base;
pub use base::{Base, GraphViz};
pub mod vid;
pub mod nid;
pub mod reg;
pub mod vhl;
pub mod wip;
pub mod ops;
pub mod cur;
pub mod simp;
pub mod ast;
pub mod bdd;
pub mod solve;
pub mod apl;
pub mod int;
pub mod io;
pub mod anf;
pub mod swap;
pub mod swarm;
