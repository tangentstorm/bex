//! A crate for working with boolean expressions.

extern crate bincode;
#[macro_use] extern crate serde_derive;
extern crate fnv;
extern crate hashbrown;

/// The core library, and support for boolean expressions as abstract syntax trees.
pub mod base;
pub use base::*;

/// Misc helper routines inspired by the APL family of programming languages.
pub mod apl;
/// Binary decision diagrams.
pub mod bdd;
/// Working with arrays of bit structures as if they were integers.
pub mod x32;
/// Input/output support for the other modules.
pub mod io;
