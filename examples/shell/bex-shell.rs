use std::io;
use std::io::Write;

extern crate bex;
use bex::shell::Shell;

include!(concat!(env!("OUT_DIR"), "/bex-build-info.rs"));

fn main() {
  println!("bex {BEX_VERSION} | compile flags: -O{BEX_OPT_LEVEL} | type 'q' to quit");
  let mut sh = Shell::new();
  sh.graphical = true;
  loop {
    print!("[ "); for x in sh.stack() { print!("{} ", x); } println!("]");
    print!("> "); io::stdout().flush().expect("couldn't flush stdout.");
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).expect("failed to read line.");
    print!("{}", sh.eval_line(&buf));
    if sh.quit { break; }}}
