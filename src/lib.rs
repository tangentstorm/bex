//! A crate for working with boolean expressions.

#![allow(clippy::many_single_char_names)]

#[macro_use] extern crate log;
extern crate bincode;
#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate fnv;
extern crate hashbrown;
extern crate simplelog;

/// Standard interface for working with databases of expressions.
pub mod base;
pub use base::*;

/// Node IDs (shared by various Base implementations)
pub mod nid;

/// Abstract syntax trees (simple logic combinators).
pub mod ast;
/// Binary decision diagrams (if-then-else).
pub mod bdd;

/// Solve AST-based expressions by converting them to other forms.
pub mod solve;

/// Helper routines inspired by the APL family of programming languages.
pub mod apl;
/// Helpers for working with arrays of bit structures as if they were integers.
pub mod int;
/// Input/output helpers.
pub mod io;

/// (Experimental/Unfinished) support for algebraic normal form (XOR of AND).
pub mod anf;
