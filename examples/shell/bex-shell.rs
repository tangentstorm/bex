use std::io;
use std::io::Write;
use std::collections::HashMap;

extern crate bex;
use bex::*;
use bex::nid::NID;
use bex::ast::{ASTBase};


// forth-like REPL for the BDD  (helper routines)

fn readln()->String {
  let mut buf = String::new();
  print!("> ");
  io::stdout().flush()                 .expect("couldn't flush stdout.");
  io::stdin().read_line(&mut buf)      .expect("failed to read line.");
  buf}

fn swap(data: &mut Vec<NID>) {
  let p = data.len()-1;
  if p > 0 { data.swap(p-1,p) }}

fn pop<T>(data: &mut Vec<T>)->T {
  data.pop().expect("underflow")}

fn pop2<T>(data: &mut Vec<T>)->(T,T){
  let y=pop(data); let x=pop(data); (x,y) }

/*fn pop3<T>(data: &mut Vec<T>)->(T,T,T){
  let (y,z)=pop2(data); let x=pop(data); (x,y,z) }*/


// forth-like REPL for the BDD  (main loop)

// fn to_io(b:bool)->NID { if b {Op::I} else {Op::O} }

// enum Item { Vid(VID), Nid(NID), Int(u32) }


fn repl(base:&mut ASTBase) {
  let mut scope = HashMap::new();
  println!("hint: no variables defined. type '8 vars' to define 8 of them.");
  let mut data: Vec<NID> = Vec::new();
  'main: loop {
    print!("[ "); for x in &data { print!("{} ", *x); } println!("]");
    let line = readln();
    for word in line.split_whitespace() {
      match word {
        "vars" => { let x = pop(&mut data);
                    for i in base.num_vars()..nid::idx(x) { base.var(i as u32); }}
        // bdd commands
        "i"|"I" => data.push(base.i()),
        "o"|"O" => data.push(base.o()),
        "~"|"not" => { let x = pop(&mut data); data.push(base.not(x)) }
        "and" => { let (x,y)=pop2(&mut data); data.push(base.and(x,y)) }
        "xor" => { let (x,y)=pop2(&mut data); data.push(base.xor(x,y)) }
        "or"  => { let (x,y)=pop2(&mut data); data.push(base.or(x,y)) }
        //"lt"  => { let (x,y)=pop2(&mut data); data.push(base.lt(x,y)) }
        // "gt"  => { let (x,y)=pop2(&mut data); data.push(base.gt(x,y)) }
        //todo "lo" => { let (x,y)=pop2(&mut data); data.push(base.when_lo(y,x)) }
        //todo "hi" => { let (x,y)=pop2(&mut data); data.push(base.when_hi(y,x)) }
        //todo "cnt" => { let x = pop(&mut data); data.push(base.node_count(x)) }
        // "ite" => { let (x,y,z) = pop3(&mut data); data.push(base.ite(x,y,z)); }
        //todo "swp" => { let (n,x,y) = pop3(&mut data); data.push(base.swap(n,x,y)); }
        // "norm" => { let (x,y,z) = pop3(&mut data); println!("{:?}", base.norm(x,y,z)) }
        // "tup" => { let (v,hi,lo) = base.tup(data.pop().expect("underflow")); println!("({}, {}, {})", v,hi,lo); },
        //todo "rep" => { let (x,y,z)=pop3(&mut data); data.push(base.replace(x,y,z)); }
        //"var?" => { let x=pop(&mut data); data.push(to_io(base.is_var(x))); }
        //todo "dep?" => { let (x,y)=pop2(&mut data); data.push(to_io(base.might_depend(x,y))); }
        // "deep" => { let x = pop(&mut data); data.push(base.deep[x]); }
        "dot" => { let mut s=String::new(); base.dot(pop(&mut data),&mut s); print!("{}", s); }
        "sho" => base.show(pop(&mut data)),

        // generic forth commands
        "q" => break 'main,
        "." => { let nid = data.pop().expect("underflow"); println!("{}", nid); }
        "drop" => { let _ = pop(&mut data); }
        "dup" => { let x = pop(&mut data); data.push(x); data.push(x); }
        "swap" => swap(&mut data),
        "reset" => data = Vec::new(),
        //todo "save" => base.save("saved.bdd").expect("failed to save bdd"),
        //todo "load" => base.load("saved.bdd").expect("failed to load bdd"),
        _ => {
          // parse number:
          if let Ok(w)=usize::from_str_radix(word, 10) { data.push(nid::ixn(w as u32)); }
          // parse input variable
          else if word.starts_with('$') {
            let s = word.to_string().split_off(1);
            if let Ok(n) = usize::from_str_radix(&s, 10) {
              data.push(base.var(n as u32)); }
            else { println!("bad var: {}", word) } }
          // define:
          else if word.starts_with(':') {
            let var = word.to_string().split_off(1);
            let val = pop(&mut data);
            scope.insert(var,val); }
          // retrieve:
          else if let Some(&val) = scope.get(word) { data.push(val); }
          else { println!("{}?", word) }}}}}}


fn main() {
  let mut base = ASTBase::empty();
  let args = ::std::env::args().skip(1);
  if args.count() == 0 { repl(&mut base) }
  else { for arg in ::std::env::args().skip(1) { match arg.as_str() {
    // "norms" => { gen_norms(); },
    "repl"  => { repl(&mut base); },
    _ => repl(&mut base) }}}}
