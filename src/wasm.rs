//! wasm-bindgen bindings for the bex web demo (feature `wasm`).
//!
//! Exposes the embeddable [`crate::shell::Shell`] to javascript. Graphviz
//! dot output inside [`WasmShell::eval`] results is delimited by
//! `@@bex-dot-begin@@` / `@@bex-dot-end@@` lines (see
//! [`crate::shell::DOT_BEGIN`], [`crate::shell::DOT_END`]).

use wasm_bindgen::prelude::*;
use crate::shell::Shell;

/// the bex forth-like shell, exposed to javascript.
#[wasm_bindgen]
pub struct WasmShell { sh: Shell }

#[wasm_bindgen]
impl WasmShell {
  /// create a new shell: fresh bases, empty stack, no desktop viewer.
  #[wasm_bindgen(constructor)]
  pub fn new()->WasmShell {
    let mut sh = Shell::new(); sh.graphical = false;
    WasmShell{ sh }}

  /// evaluate one line of forth-like input, returning everything it printed.
  pub fn eval(&mut self, line:&str)->String { self.sh.eval_line(line) }

  /// the data stack as a space-separated string.
  pub fn stack(&self)->String { self.sh.stack_string() }

  /// reset to a fresh shell.
  pub fn reset(&mut self) { self.sh = Shell::new(); }

  /// true after the `q` word has been evaluated.
  pub fn quit_requested(&self)->bool { self.sh.quit }

  /// crate version string.
  pub fn version()->String { format!("bex {}", env!("CARGO_PKG_VERSION")) }
}
