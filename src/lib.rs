//! A crate for working with boolean expressions.

#![allow(clippy::many_single_char_names)]

#[macro_use] extern crate log;
extern crate bincode;
#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate simplelog;
extern crate rand;
extern crate dashmap;
extern crate boxcar;
extern crate fxhash;

pub mod base;
pub use base::{Base, GraphViz};
pub mod vid;
pub mod nid;
pub use nid::{NID,I,O};
pub mod reg;
pub mod vhl;
pub mod wip;
pub mod ops;
pub mod cur;
pub mod simp;
pub mod ast;
pub mod bdd;
pub use bdd::BDDBase;
pub mod solve;
pub mod apl;
pub mod int;
pub mod io;
pub mod anf;
pub mod swap;
pub mod swarm;
