//! A crate for working with boolean expressions.

#![allow(clippy::many_single_char_names)]

#[macro_use] extern crate log;
extern crate simplelog;
extern crate rand;
extern crate dashmap;
extern crate boxcar;
extern crate fxhash;
extern crate concurrent_queue;

pub mod base;   pub use crate::base::{Base, GraphViz};
pub mod vid;
pub mod nid;    pub use crate::nid::{NID,I,O};
pub mod fun;    pub use crate::fun::Fun;
pub mod reg;    pub use crate::reg::{Reg, RegOps, RegView};
pub mod vhl;
pub mod wip;
pub mod ops;
pub mod cur;
pub mod simp;
pub mod ast;
pub mod bdd;    pub use crate::bdd::BddBase;
pub mod solve;
pub mod apl;
pub mod int;
#[cfg(feature = "sql")]
pub mod sql;
pub mod anf;
pub mod swap;
pub mod swarm;
pub mod vhl_swarm;
pub mod naf;
