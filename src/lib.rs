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

/// Variable IDs (used interally by Base implementations)
pub mod vid;
/// Node IDs (shared by various Base implementations)
pub mod nid;
/// Registers -- arbitrarily large arrays of bits.
pub mod reg;
/// (Var, Hi, Lo) triples
pub mod vhl;
/// Structures for storing work in progress.
pub mod wip;
/// RPN-like serialization structure for expressions.
pub mod ops;

// Cursors (register + stack and scope) for navigating vhl-graphs (Bdd, Anf, etc)
pub mod cur;

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

/// (Experimental) support for algebraic normal form (XOR of AND).
pub mod anf;

/// swap solver
pub mod swap;

/// multicore support
pub mod swarm;
