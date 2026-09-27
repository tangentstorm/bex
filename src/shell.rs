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
  let p = data.len()-1;
  if p > 0 { data.swap(p-1,p) }}

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
      "swap" => swap(data),
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
