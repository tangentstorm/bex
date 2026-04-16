//! Generate an AST `.sdb` file for a primorial factoring problem.
//!
//! Usage: bex-mkproblem -p <N> -o <file.sdb>
//!
//! Produces an AST representing "find x,y where x<y and x*y == primorial(N)"
//! using 8-bit factors (16-bit product). The top node is tagged "top".

use std::env;
use std::process;


use bex::int::{GBASE, BInt, X8, X16};
use bex::ast::ASTBase;
use bex::sql;

const PRIMES: &[usize] = &[2, 3, 5, 7, 11, 13, 17, 19, 23];

fn primorial(n: usize) -> usize {
  PRIMES.iter().take(n).product()
}

fn main() {
  let args: Vec<String> = env::args().collect();
  let mut which: usize = 4;
  let mut outpath: Option<String> = None;
  let mut i = 1;
  while i < args.len() {
    match args[i].as_str() {
      "-p" => { i += 1; which = args[i].parse().expect("bad -p value"); }
      "-o" => { i += 1; outpath = Some(args[i].clone()); }
      "-h" | "--help" => { usage(); process::exit(0); }
      _ => { eprintln!("unknown arg: {}", args[i]); usage(); process::exit(1); }
    }
    i += 1;
  }
  let outpath = outpath.unwrap_or_else(|| format!("primorial-{}.sdb", which));
  if which < 2 || which > PRIMES.len() {
    eprintln!("primorial must be between 2 and {}", PRIMES.len());
    process::exit(1);
  }
  let k = primorial(which);
  println!("primorial({}) = {}", which, k);
  println!("generating AST for: find x,y (8-bit) where x<y and x*y == {}", k);

  // Build the AST using the thread-local GBASE
  GBASE.with(|gb| gb.replace(ASTBase::empty()));
  let (y, x) = (X8::def("y", 0), X8::def("x", X8::n()));
  let lt = x.lt(&y);
  let xy: X16 = x.times(&y);
  let kv = X16::new(k);
  let eq = BInt::eq(&xy, &kv);
  let top = lt & eq;

  // Swap out the global base and get the raw AST
  let mut gb = GBASE.with(|gb| gb.replace(ASTBase::empty()));
  gb.raw_ast_mut().tags.insert("top".to_string(), top.n);
  let src = gb.raw_ast();

  // Export to .sdb
  let keep = vec![top.n];
  sql::export_raw_ast_to_path(src, &outpath, &keep)
    .unwrap_or_else(|e| { eprintln!("failed to write {}: {}", outpath, e); process::exit(1); });

  println!("wrote {} ({} AST nodes)", outpath, src.len());
}

fn usage() {
  eprintln!("Usage: bex-mkproblem -p <N> -o <file.sdb>");
  eprintln!("  -p N   which primorial (2..9, default 4)");
  eprintln!("  -o F   output file (default: primorial-N.sdb)");
}
