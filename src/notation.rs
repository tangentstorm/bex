//! Standard bex notations (issue #10): a common vocabulary for spelling NIDs,
//! an infix algebraic grammar built on top of that vocabulary, bracket-notation
//! function application, and a JSON encoding of the same expressions.
//!
//! See `doc/NOTATION.md` for the full write-up (including the "Decisions"
//! section documenting choices made where the issue text was ambiguous).
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use json::JsonValue;

use crate::base::Base;
use crate::nid::{NID, NidFun};
use crate::Fun;


// ---- ParsedNid: NID::from_str/Display + optional numeric namespace prefix ----

/// A NID, plus the optional numeric namespace prefix (`N:`) mentioned in the
/// issue for "dealing with multiple bases". `NID::from_str`/`Display` already
/// implement every other spelling in the vocabulary (`O`, `I`, `xN`, `vN`,
/// `!xN`, `tBBBB`, `fN.MMM`, `fH`, `xNN.MMMM`, `@MMMM`, `T{...}`); this type
/// wraps that parser/printer and adds the namespace, guaranteeing
/// `parse(display(x)) == x` for every form (see `doc/NOTATION.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedNid { pub ns: Option<u32>, pub nid: NID }

impl ParsedNid {
  pub fn new(nid:NID)->Self { ParsedNid{ ns:None, nid } }
  pub fn with_ns(ns:u32, nid:NID)->Self { ParsedNid{ ns:Some(ns), nid } }
}

impl From<NID> for ParsedNid { fn from(nid:NID)->Self { ParsedNid::new(nid) } }

impl FromStr for ParsedNid {
  type Err = String;
  fn from_str(s:&str)->Result<Self, String> {
    // Namespace prefix is only recognized when everything before the first
    // ':' is a run of ascii digits (this is what lets it coexist with the
    // named-table form `T{x3,x7:1110}`, which also contains a ':').
    if let Some(ix) = s.find(':') {
      let (a, b) = s.split_at(ix);
      if !a.is_empty() && a.chars().all(|c| c.is_ascii_digit()) {
        let ns:u32 = a.parse().map_err(|_| format!("bad namespace prefix: {}", s))?;
        let nid_txt = &b[1..];
        // Reject empty / bang-only before NID::from_str (which historically panicked).
        if nid_txt.is_empty() || nid_txt == "!" {
          return Err(format!("empty nid after namespace: {}", s)); }
        let nid:NID = nid_txt.parse()?;
        return Ok(ParsedNid::with_ns(ns, nid));
      }}
    if s.is_empty() || s == "!" {
      return Err(format!("empty nid: {:?}", s)); }
    let nid:NID = s.parse()?;
    Ok(ParsedNid::new(nid)) }}

impl fmt::Display for ParsedNid {
  fn fmt(&self, f:&mut fmt::Formatter)->fmt::Result {
    if let Some(ns) = self.ns { write!(f, "{}:{}", ns, self.nid) }
    else { write!(f, "{}", self.nid) }}}


// ---- infix algebraic grammar --------------------------------------------

/// AST for the infix algebraic grammar described in the issue: the vocabulary
/// of operators (`|`,`&`,`%`,`=`,`!`,`<`,`/`,`?:`), `:` assignment, parens,
/// and bracket-notation function application.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
  Nid(NID),
  Var(String),
  Not(Box<Expr>),
  And(Box<Expr>, Box<Expr>),
  Xor(Box<Expr>, Box<Expr>),
  Or(Box<Expr>, Box<Expr>),
  /// `x < y` : "y and not x"
  Lt(Box<Expr>, Box<Expr>),
  /// `x / y` : "x implies y"
  Le(Box<Expr>, Box<Expr>),
  /// `x = y` : "equal" (non-associative at parse time)
  Eq(Box<Expr>, Box<Expr>),
  /// `f ? g : h` : "if f then g else h"
  Ite(Box<Expr>, Box<Expr>, Box<Expr>),
  /// `name : expr`
  Assign(String, Box<Expr>),
  /// `f[a b c]` : bracket-notation application
  Apply(Box<Expr>, Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok { LParen, RParen, LBracket, RBracket, Bang, Amp, Pct, Pipe, Lt, Slash, Eq, Question, Colon, Word(String) }

/// Split an infix-grammar source string into tokens.
/// A "word" is a maximal run of nid/identifier characters; digits immediately
/// followed by `:` and more word characters are glued into the word so that
/// namespace-prefixed nids (e.g. `2:x0`) tokenize as a single word, while a
/// bare `name : expr` assignment (which always has non-numeric `name`) still
/// splits into separate `Word`/`Colon` tokens. See doc/NOTATION.md.
fn tokenize(s:&str)->Result<Vec<Tok>, String> {
  fn is_word_char(c:char)->bool { c.is_alphanumeric() || "._@{},".contains(c) }
  let chars:Vec<char> = s.chars().collect();
  let mut i = 0usize;
  let mut toks = vec![];
  while i < chars.len() {
    let c = chars[i];
    if c.is_whitespace() { i += 1; continue; }
    match c {
      '(' => { toks.push(Tok::LParen); i += 1; }
      ')' => { toks.push(Tok::RParen); i += 1; }
      '[' => { toks.push(Tok::LBracket); i += 1; }
      ']' => { toks.push(Tok::RBracket); i += 1; }
      '!' => { toks.push(Tok::Bang); i += 1; }
      '&' => { toks.push(Tok::Amp); i += 1; }
      '%' => { toks.push(Tok::Pct); i += 1; }
      '|' => { toks.push(Tok::Pipe); i += 1; }
      '<' => { toks.push(Tok::Lt); i += 1; }
      '/' => { toks.push(Tok::Slash); i += 1; }
      '=' => { toks.push(Tok::Eq); i += 1; }
      '?' => { toks.push(Tok::Question); i += 1; }
      ':' => { toks.push(Tok::Colon); i += 1; }
      _ if is_word_char(c) => {
        let start = i;
        let mut word = String::new();
        loop {
          if i >= chars.len() { break; }
          let c = chars[i];
          if is_word_char(c) { word.push(c); i += 1; }
          else if c == ':' && !word.is_empty() && word.chars().all(|c| c.is_ascii_digit())
                  && i+1 < chars.len() && is_word_char(chars[i+1]) {
            word.push(':'); i += 1;
          }
          else { break; }}
        if word.is_empty() { return Err(format!("bad character at {}: {:?}", start, c)); }
        toks.push(Tok::Word(word)); }
      _ => return Err(format!("unexpected character at {}: {:?}", i, c)) }}
  Ok(toks) }


struct Parser { toks:Vec<Tok>, pos:usize }

impl Parser {
  fn peek(&self)->Option<&Tok> { self.toks.get(self.pos) }
  fn next(&mut self)->Option<Tok> { let t = self.toks.get(self.pos).cloned(); if t.is_some() { self.pos += 1; } t }
  fn expect(&mut self, t:&Tok)->Result<(), String> {
    if self.peek() == Some(t) { self.pos += 1; Ok(()) }
    else { Err(format!("expected {:?}, found {:?}", t, self.peek())) }}

  fn parse_expr(&mut self)->Result<Expr, String> { self.parse_assign() }

  fn parse_assign(&mut self)->Result<Expr, String> {
    if let Some(Tok::Word(w)) = self.peek().cloned() {
      if self.toks.get(self.pos+1) == Some(&Tok::Colon) && w.parse::<ParsedNid>().is_err() {
        self.pos += 2; // consume Word and Colon
        let rhs = self.parse_assign()?;
        return Ok(Expr::Assign(w, Box::new(rhs))); }}
    self.parse_ternary() }

  fn parse_ternary(&mut self)->Result<Expr, String> {
    let cond = self.parse_eq()?;
    if self.peek() == Some(&Tok::Question) {
      self.next();
      let then_e = self.parse_ternary()?;
      self.expect(&Tok::Colon)?;
      let else_e = self.parse_ternary()?;
      Ok(Expr::Ite(Box::new(cond), Box::new(then_e), Box::new(else_e)))
    } else { Ok(cond) }}

  fn parse_eq(&mut self)->Result<Expr, String> {
    let left = self.parse_rel()?;
    if self.peek() == Some(&Tok::Eq) {
      self.next();
      let right = self.parse_rel()?;
      if self.peek() == Some(&Tok::Eq) {
        return Err("'=' is non-associative; use parentheses to chain comparisons".to_string()); }
      Ok(Expr::Eq(Box::new(left), Box::new(right)))
    } else { Ok(left) }}

  fn parse_rel(&mut self)->Result<Expr, String> {
    let mut left = self.parse_or()?;
    loop { match self.peek() {
      Some(&Tok::Lt) => { self.next(); let r = self.parse_or()?; left = Expr::Lt(Box::new(left), Box::new(r)); }
      Some(&Tok::Slash) => { self.next(); let r = self.parse_or()?; left = Expr::Le(Box::new(left), Box::new(r)); }
      _ => break }}
    Ok(left) }

  fn parse_or(&mut self)->Result<Expr, String> {
    let mut left = self.parse_xor()?;
    while self.peek() == Some(&Tok::Pipe) { self.next(); let r = self.parse_xor()?; left = Expr::Or(Box::new(left), Box::new(r)); }
    Ok(left) }

  fn parse_xor(&mut self)->Result<Expr, String> {
    let mut left = self.parse_and()?;
    while self.peek() == Some(&Tok::Pct) { self.next(); let r = self.parse_and()?; left = Expr::Xor(Box::new(left), Box::new(r)); }
    Ok(left) }

  fn parse_and(&mut self)->Result<Expr, String> {
    let mut left = self.parse_unary()?;
    while self.peek() == Some(&Tok::Amp) { self.next(); let r = self.parse_unary()?; left = Expr::And(Box::new(left), Box::new(r)); }
    Ok(left) }

  fn parse_unary(&mut self)->Result<Expr, String> {
    if self.peek() == Some(&Tok::Bang) { self.next(); let e = self.parse_unary()?; Ok(Expr::Not(Box::new(e))) }
    else { self.parse_postfix() }}

  fn parse_postfix(&mut self)->Result<Expr, String> {
    let mut e = self.parse_primary()?;
    while self.peek() == Some(&Tok::LBracket) {
      self.next();
      let mut args = vec![];
      while self.peek() != Some(&Tok::RBracket) {
        if self.peek().is_none() { return Err("unterminated bracket application".to_string()); }
        args.push(self.parse_unary()?); }
      self.expect(&Tok::RBracket)?;
      e = Expr::Apply(Box::new(e), args); }
    Ok(e) }

  fn parse_primary(&mut self)->Result<Expr, String> {
    match self.next() {
      Some(Tok::LParen) => { let e = self.parse_expr()?; self.expect(&Tok::RParen)?; Ok(e) }
      Some(Tok::Word(w)) => {
        match w.parse::<ParsedNid>() {
          Ok(pn) => Ok(Expr::Nid(pn.nid)),
          Err(_) => Ok(Expr::Var(w)) }}
      other => Err(format!("expected a value, found {:?}", other)) }}
}

/// Parse an expression in the infix algebraic grammar described in the
/// issue. Operator precedence, highest to lowest: unary `!`; `&`; `%`; `|`;
/// `<` and `/`; `=` (non-associative); `?:` (right-associative). `:`
/// assigns a name to the expression on its right, and parentheses and
/// bracket-notation application (`f[a b]`) both nest arbitrarily.
pub fn parse_expr(s:&str)->Result<Expr, String> {
  let toks = tokenize(s)?;
  let mut p = Parser{ toks, pos:0 };
  let e = p.parse_expr()?;
  if p.pos != p.toks.len() { return Err(format!("unexpected trailing input: {:?}", &p.toks[p.pos..])); }
  Ok(e) }


// ---- evaluation: build the expression in a Base using and/or/xor/ite ----

/// Evaluate a parsed `Expr` against a `Base`, resolving named variables (from
/// `:` assignments or previously bound names) via `scope`. Assignment
/// updates `scope` and also returns the assigned NID.
///
/// Operators map onto `Base` methods directly, except for `=`, `<`, and `/`,
/// which have no dedicated `Base` method and are built from `and`/`or`/`xor`
/// per the definitions in the issue (`x=y` is `!(x^y)`, `x<y` is `y&!x`,
/// `x/y` is `!x|y`).
pub fn eval_expr<B:Base>(base:&mut B, expr:&Expr, scope:&mut HashMap<String, NID>)->Result<NID, String> {
  match expr {
    Expr::Nid(n) => Ok(*n),
    Expr::Var(name) => scope.get(name).copied()
      .or_else(|| base.get(name))
      .ok_or_else(|| format!("undefined variable: {}", name)),
    Expr::Not(x) => Ok(!eval_expr(base, x, scope)?),
    Expr::And(a,b) => { let (x,y) = (eval_expr(base,a,scope)?, eval_expr(base,b,scope)?); Ok(base.and(x,y)) }
    Expr::Xor(a,b) => { let (x,y) = (eval_expr(base,a,scope)?, eval_expr(base,b,scope)?); Ok(base.xor(x,y)) }
    Expr::Or(a,b)  => { let (x,y) = (eval_expr(base,a,scope)?, eval_expr(base,b,scope)?); Ok(base.or(x,y)) }
    Expr::Lt(a,b)  => { let (x,y) = (eval_expr(base,a,scope)?, eval_expr(base,b,scope)?); let nx = !x; Ok(base.and(nx,y)) }
    Expr::Le(a,b)  => { let (x,y) = (eval_expr(base,a,scope)?, eval_expr(base,b,scope)?); let nx = !x; Ok(base.or(nx,y)) }
    Expr::Eq(a,b)  => { let (x,y) = (eval_expr(base,a,scope)?, eval_expr(base,b,scope)?); Ok(!base.xor(x,y)) }
    Expr::Ite(i,t,e) => {
      let (iv,tv,ev) = (eval_expr(base,i,scope)?, eval_expr(base,t,scope)?, eval_expr(base,e,scope)?);
      Ok(base.ite(iv,tv,ev)) }
    Expr::Assign(name, rhs) => { let v = eval_expr(base, rhs, scope)?; scope.insert(name.clone(), v); Ok(v) }
    Expr::Apply(f, args) => {
      let fv = eval_expr(base, f, scope)?;
      let argv:Vec<NID> = args.iter().map(|a| eval_expr(base, a, scope)).collect::<Result<_,_>>()?;
      apply_bracket(base, fv, &argv) }}}


/// Apply bracket notation (`n[args...]`) to `n`:
/// - if `n` is a table nid, apply it as a function (arity must equal `args.len()`).
/// - if `n` is a VHL/BDD nid with an index, perform top-down substitution: the
///   first arg replaces `n`'s (top) branching variable, the next arg replaces
///   whatever variable now tops the result, and so on. (If there are more
///   args than remaining variables, the extra args are ignored -- see
///   doc/NOTATION.md.)
/// - if `n` is an AST nid (`@MMMM`), this is not supported yet (per the issue,
///   which offers disallowing it as an explicit option).
/// - anything else (a plain variable or a constant) has no "index" to
///   substitute into, so it is an error.
pub fn apply_bracket<B:Base>(base:&mut B, n:NID, args:&[NID])->Result<NID, String> {
  if let Some(f) = n.to_fun() {
    if f.arity() as usize != args.len() {
      return Err(format!("table nid {} has arity {} but {} arg(s) were given", n, f.arity(), args.len())); }
    // NidFun::tbl() reads raw() (drops INV); invert the evaluated result when n is inverted.
    Ok(apply_table(base, f, args).inv_if(n.is_inv()))
  } else if n.is_ixn() {
    Err("bracket substitution on AST nids is not supported yet".to_string())
  } else if n.is_lit() {
    // plain literal (constant or bare variable, no index): nothing to substitute into.
    Err(format!("cannot apply bracket notation to nid with no index: {}", n))
  } else {
    Ok(apply_vhl_sub(base, n, args)) }}

fn apply_table<B:Base>(base:&mut B, f:NidFun, args:&[NID])->NID {
  let ar = f.arity();
  if ar == 0 { return NID::from_bit(f.tbl() != 0); }
  let top = ar - 1;
  let hi = apply_table(base, f.when(top, true), &args[..top as usize]);
  let lo = apply_table(base, f.when(top, false), &args[..top as usize]);
  base.ite(args[top as usize], hi, lo) }

fn apply_vhl_sub<B:Base>(base:&mut B, n:NID, args:&[NID])->NID {
  let mut cur = n;
  for &arg in args {
    if cur.is_lit() { break; } // no variable left to substitute into
    let v = cur.vid();
    cur = base.sub(v, arg, cur); }
  cur }


// ---- JSON notation --------------------------------------------------------

fn op_key(e:&Expr)->Result<String, String> {
  match e {
    Expr::Nid(n) => Ok(n.to_string()),
    Expr::Var(name) => Ok(name.clone()),
    _ => Err("bracket-application target must be a nid or a name".to_string()) }}

/// Serialize an `Expr` to the JSON notation from the issue: primitive nids
/// and names are single JSON strings; compounds are `{"op": [args]}` with
/// `op` spelled `|`,`&`,`%`,`=`,`~`,`<`,`/`,`?:`; `~` (not) elides the list
/// wrapper (`{"~": "x3"}`); a nid or name used as an operator key means
/// bracket-notation application (`{"x5.234": [...]}`).
pub fn to_json(e:&Expr)->Result<JsonValue, String> {
  fn binop(op:&str, a:&Expr, b:&Expr)->Result<JsonValue, String> {
    let mut o = JsonValue::new_object();
    let arr = JsonValue::Array(vec![to_json(a)?, to_json(b)?]);
    o.insert(op, arr).map_err(|e| e.to_string())?;
    Ok(o) }
  match e {
    Expr::Nid(n) => Ok(JsonValue::from(n.to_string())),
    Expr::Var(name) => Ok(JsonValue::from(name.clone())),
    Expr::Not(x) => { let mut o = JsonValue::new_object(); o.insert("~", to_json(x)?).map_err(|e| e.to_string())?; Ok(o) }
    Expr::And(a,b) => binop("&", a, b),
    Expr::Xor(a,b) => binop("%", a, b),
    Expr::Or(a,b)  => binop("|", a, b),
    Expr::Lt(a,b)  => binop("<", a, b),
    Expr::Le(a,b)  => binop("/", a, b),
    Expr::Eq(a,b)  => binop("=", a, b),
    Expr::Ite(i,t,e2) => {
      let mut o = JsonValue::new_object();
      let arr = JsonValue::Array(vec![to_json(i)?, to_json(t)?, to_json(e2)?]);
      o.insert("?:", arr).map_err(|e| e.to_string())?;
      Ok(o) }
    Expr::Assign(..) => Err("assignment has no JSON representation".to_string()),
    Expr::Apply(f, args) => {
      let key = op_key(f)?;
      let arr = JsonValue::Array(args.iter().map(to_json).collect::<Result<Vec<_>,_>>()?);
      let mut o = JsonValue::new_object();
      o.insert(&key, arr).map_err(|e| e.to_string())?;
      Ok(o) }}}

/// Parse the JSON notation from the issue back into an `Expr`. See `to_json`.
pub fn from_json(v:&JsonValue)->Result<Expr, String> {
  if let Some(s) = v.as_str() {
    return Ok(match s.parse::<ParsedNid>() { Ok(pn) => Expr::Nid(pn.nid), Err(_) => Expr::Var(s.to_string()) }); }
  if v.is_object() {
    let mut entries:Vec<(&str, &JsonValue)> = v.entries().collect();
    if entries.len() != 1 { return Err(format!("expected exactly one operator key, found {}", entries.len())); }
    let (op, val) = entries.remove(0);
    return match op {
      "~" => Ok(Expr::Not(Box::new(from_json(val)?))),
      "&" | "%" | "|" | "<" | "/" | "=" => {
        let items:Vec<&JsonValue> = val.members().collect();
        if items.len() != 2 { return Err(format!("operator '{}' expects 2 args, found {}", op, items.len())); }
        let (a, b) = (from_json(items[0])?, from_json(items[1])?);
        Ok(match op {
          "&" => Expr::And(Box::new(a), Box::new(b)),
          "%" => Expr::Xor(Box::new(a), Box::new(b)),
          "|" => Expr::Or(Box::new(a), Box::new(b)),
          "<" => Expr::Lt(Box::new(a), Box::new(b)),
          "/" => Expr::Le(Box::new(a), Box::new(b)),
          "=" => Expr::Eq(Box::new(a), Box::new(b)),
          _ => unreachable!() }) }
      "?:" => {
        let items:Vec<&JsonValue> = val.members().collect();
        if items.len() != 3 { return Err(format!("'?:' expects 3 args, found {}", items.len())); }
        Ok(Expr::Ite(Box::new(from_json(items[0])?), Box::new(from_json(items[1])?), Box::new(from_json(items[2])?))) }
      _ => {
        // a nid or name used as an operator key means bracket-notation application.
        let f = match op.parse::<ParsedNid>() { Ok(pn) => Expr::Nid(pn.nid), Err(_) => Expr::Var(op.to_string()) };
        let args = val.members().map(from_json).collect::<Result<Vec<_>,_>>()?;
        Ok(Expr::Apply(Box::new(f), args)) }}}
  Err(format!("unsupported json value: {}", v)) }


#[cfg(test)]
include!("test-notation.rs");
