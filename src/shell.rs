//! Embeddable forth-like shell for bex.
//!
//! This is the core of `examples/shell/bex-shell.rs`, refactored so the same
//! code drives both the native binary and the wasm web demo. Everything the
//! native shell prints to stdout is captured into a string instead, and the
//! graph words (`sho`, `bdd`, `anf`) either open a desktop viewer (native) or
//! emit delimited graphviz dot source (wasm), according to
//! [`Shell::graphical`].
//!
//! Stack underflow no longer panics: it appends an `underflow` message to the
//! output and stops the current line. (The native shell used to crash here.)

use std::collections::HashMap;
use std::str::FromStr;

use crate::nid::NID;
use crate::ast::ASTBase;
use crate::solve;
use crate::anf::ANFBase;
use crate::bdd::BddBase;
use crate::ops;
use crate::base::Base;
use crate::GraphViz;

/// line printed before graphviz dot output inside captured shell output.
pub const DOT_BEGIN: &str = "@@bex-dot-begin@@";
/// line printed after graphviz dot output inside captured shell output.
pub const DOT_END: &str = "@@bex-dot-end@@";

fn swap(data: &mut [NID]) {
  let n = data.len();
  if n >= 2 { data.swap(n-2, n-1) }}

fn pop(data: &mut Vec<NID>)->Result<NID,String> {
  data.pop().ok_or_else(|| "underflow".to_string()) }

fn pop2(data: &mut Vec<NID>)->Result<(NID,NID),String> {
  let y=pop(data)?; let x=pop(data)?; Ok((x,y)) }

fn pop3(data: &mut Vec<NID>)->Result<(NID,NID,NID),String> {
  let (y,z)=pop2(data)?; let x=pop(data)?; Ok((x,y,z)) }

/// forth-like shell state. see the module docs.
pub struct Shell {
  base: ASTBase,
  scope: HashMap<String,NID>,
  data: Vec<NID>,
  bdds: BddBase,
  anfs: ANFBase,
  out: String,
  /// when true, `sho`/`bdd`/`anf` open graphs in a desktop viewer (native).
  /// when false, they append delimited dot source to the output instead (wasm).
  pub graphical: bool,
  /// set when the `q` word is evaluated.
  pub quit: bool }

impl Shell {
  pub fn new()->Shell {
    Shell{ base: ASTBase::empty(), scope: HashMap::new(), data: Vec::new(),
           bdds: BddBase::new(), anfs: ANFBase::new(),
           out: String::new(), graphical: false, quit: false }}

  /// clear the data stack (the `reset` word).
  pub fn reset(&mut self) { self.data = Vec::new(); }

  /// read-only view of the data stack.
  pub fn stack(&self)->&[NID] { &self.data }

  /// the data stack as a space-separated string (for the web ui).
  pub fn stack_string(&self)->String {
    self.data.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ") }

  fn delimit(out:&mut String, s:&str) {
    out.push_str(DOT_BEGIN); out.push('\n');
    out.push_str(s);
    if !s.ends_with('\n') { out.push('\n'); }
    out.push_str(DOT_END); out.push('\n'); }

  fn dot_of(&mut self, word:&str)->Result<(),String> {
    let data = &mut self.data;
    let mut s = String::new();
    match word {
      "dot" => { let x = pop(data)?; self.base.dot(x, &mut s); }
      "bdd-dot" => { let x = pop(data)?; self.bdds.write_dot(x, &mut s); }
      "anf-dot" => { let x = pop(data)?; self.anfs.write_dot(x, &mut s); }
      _ => unreachable!() }
    Self::delimit(&mut self.out, &s);
    Ok(()) }

  fn word(&mut self, word:&str)->Result<(),String> {
    let graphical = self.graphical;
    let data = &mut self.data;
    match word {
      "!" => { let x = pop(data)?; data.push(!x) }
      "and" => { let (x,y)=pop2(data)?; data.push(self.base.and(x,y)) }
      "xor" => { let (x,y)=pop2(data)?; data.push(self.base.xor(x,y)) }
      "or"  => { let (x,y)=pop2(data)?; data.push(self.base.or(x,y)) }
      "'and" => { data.push(ops::AND.to_nid()) }
      "'or" |
      "'vel" => { data.push(ops::VEL.to_nid()) }
      "'xor" => { data.push(ops::XOR.to_nid()) }
      "'imp" => { data.push(ops::IMP.to_nid()) }
      "'nor" => { data.push(ops::NOR.to_nid()) }
      "ite" => { let (x,y,z) = pop3(data)?; data.push(self.base.ite(x,y,z)); }
      "dot" | "bdd-dot" | "anf-dot" => { self.dot_of(word)?; }
      "sho" => { let x = pop(data)?;
                 if graphical { self.base.show(x); }
                 else { let mut s=String::new(); self.base.dot(x,&mut s);
                        Self::delimit(&mut self.out,&s); }}
      "bdd" => { let top=pop(data)?;
                 let n = solve::solve(&mut self.bdds, self.base.raw_ast(), top).n;
                 if graphical { self.bdds.show(n); }
                 else { let mut s=String::new(); self.bdds.write_dot(n,&mut s);
                        Self::delimit(&mut self.out,&s); }
                 data.push(n); }
      "anf" => { let top=pop(data)?;
                 let n = solve::solve(&mut self.anfs, self.base.raw_ast(), top).n;
                 if graphical { self.anfs.show(n); }
                 else { let mut s=String::new(); self.anfs.write_dot(n,&mut s);
                        Self::delimit(&mut self.out,&s); }
                 data.push(n); }
      // generic forth commands
      "q" => { self.quit = true; }
      "." => { let x = pop(data)?; self.out.push_str(&format!("{}\n", x)); }
      "drop" => { pop(data)?; }
      "dup" => { let x = pop(data)?; data.push(x); data.push(x); }
      "swap" => { if data.len() < 2 { return Err("underflow".to_string()); }
                  swap(data); }
      "reset" => { self.data = Vec::new(); }
      // bdd commands
      "I" => data.push(crate::nid::I),
      "O" => data.push(crate::nid::O),
      _ => {
        // define a new binding
        if word.starts_with(':') {
          let var = word.to_string().split_off(1);
          let val = pop(data)?;
          self.scope.insert(var,val); }
        // recall definition
        else if let Some(&val) = self.scope.get(word) { data.push(val); }
        // attempt to parse nid
        else { match NID::from_str(word) {
          Ok(nid) => data.push(nid),
          Err(err) => { self.out.push_str(&format!("{}\n", err)); }}}}}
    Ok(()) }

  /// evaluate one line of forth-like input, returning everything it printed.
  /// a stack underflow (or any other word error) ends the line with a message.
  pub fn eval_line(&mut self, line:&str)->String {
    self.out.clear();
    for word in line.split_whitespace() {
      if let Err(e) = self.word(word) {
        self.out.push_str(&e); self.out.push('\n'); break; }}
    std::mem::take(&mut self.out) }}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::nid::{I, O};
  use crate::vid::VID;

  fn fresh()->Shell { Shell::new() }

  #[test] fn swap_empty_is_underflow_not_panic() {
    let mut sh = fresh();
    let out = sh.eval_line("swap");
    assert!(out.contains("underflow"), "got: {out:?}");
    assert!(sh.stack().is_empty()); }

  #[test] fn swap_single_is_underflow() {
    let mut sh = fresh();
    let out = sh.eval_line("I swap");
    assert!(out.contains("underflow"), "got: {out:?}"); }

  #[test] fn swap_two_swaps() {
    let mut sh = fresh();
    sh.eval_line("I O swap");
    assert_eq!(sh.stack(), &[O, I]); }

  #[test] fn anf_dot_of_consts() {
    // regression test: ANFBase::dot used to panic on constant roots
    // (ANFBase::fetch indexed ANFBase.nodes with I/O, which have no entry).
    for (word, expect) in [("I anf", I), ("O anf", O)] {
      let mut sh = fresh();
      let out = sh.eval_line(word);
      assert!(out.contains(DOT_BEGIN), "missing dot begin in {out:?}");
      assert!(out.contains(DOT_END), "missing dot end in {out:?}");
      assert_eq!(sh.stack(), &[expect]); }}

  #[test] fn bdd_dot_of_const() {
    let mut sh = fresh();
    let out = sh.eval_line("I O and bdd");
    assert!(out.contains(DOT_BEGIN), "missing dot begin in {out:?}");
    assert_eq!(sh.stack(), &[O]); }

  #[test] fn solve_direct_ite_xor() {
    // the wasm32 threadless BDD path: direct single-threaded ITE, no swarm.
    let mut base = ASTBase::new();
    let x0 = NID::from_vid(VID::var(0));
    let x1 = NID::from_vid(VID::var(1));
    let top = base.xor(x0, x1);
    let mut bdd = BddBase::new();
    bdd.set_direct_ite(true);
    let d = solve::solve(&mut bdd, base.raw_ast(), top);
    assert_eq!(format!("{:?}", d.n), "t0110"); }

  #[test] fn bind_and_recall() {
    let mut sh = fresh();
    sh.eval_line("I O xor :x");
    assert!(sh.stack().is_empty(), "bind should pop, got {:?}", sh.stack());
    sh.eval_line("x");
    assert_eq!(sh.stack().len(), 1); }

  #[test] fn reset_clears_stack() {
    let mut sh = fresh();
    sh.eval_line("I O xor");
    assert!(!sh.stack().is_empty());
    sh.reset();
    assert!(sh.stack().is_empty()); }

  #[test] fn quit_word_sets_flag() {
    let mut sh = fresh();
    assert!(!sh.quit);
    sh.eval_line("q");
    assert!(sh.quit); }
}
