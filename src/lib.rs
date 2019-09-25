//! A crate for working with boolean expressions.

#[macro_use] extern crate log;
extern crate bincode;
#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate fnv;
extern crate hashbrown;
extern crate simplelog;

/// The core library, and support for boolean expressions as abstract syntax trees.
pub mod base;
pub use base::*;

/// Misc helper routines inspired by the APL family of programming languages.
pub mod apl;
/// Base implementation for simple AST representation.
pub mod ast;
/// Binary decision diagrams.
pub mod bdd;
/// Working with arrays of bit structures as if they were integers.
pub mod int;
/// Input/output support for the other modules.
pub mod io;
/// solve AST-based expressions by converting them to BDDs.
pub mod solve;
