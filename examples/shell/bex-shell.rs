use std::io;
use std::io::Write;
use std::collections::HashMap;
use std::str::FromStr;

extern crate bex;
use bex::*;
use bex::nid::NID;
use bex::ast::ASTBase;
use bex::solve;
use bex::anf::ANFBase;
use bex::bdd::BddBase;
use bex::ops;

// forth-like REPL for the BDD  (helper routines)

fn readln()->String {
  let mut buf = String::new();
  print!("> ");
  io::stdout().flush()                 .expect("couldn't flush stdout.");
  io::stdin().read_line(&mut buf)      .expect("failed to read line.");
  buf}

fn swap(data: &mut [NID]) {
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
  let mut data: Vec<NID> = Vec::new();
  let mut bdds = BddBase::new();
  let mut anfs = ANFBase::new();

  'main: loop {
    print!("[ "); for x in &data { print!("{} ", *x); } println!("]");
    let line = readln();
    for word in line.split_whitespace() {
      match word {
        "~"|"not"|"!" => { let x = pop(&mut data); data.push(!x) }
        "and" => { let (x,y)=pop2(&mut data); data.push(base.and(x,y)) }
        "xor" => { let (x,y)=pop2(&mut data); data.push(base.xor(x,y)) }
        "or"  => { let (x,y)=pop2(&mut data); data.push(base.or(x,y)) }
        "'and" => { data.push(ops::AND.to_nid()) }
        "'or" |
        "'vel" => { data.push(ops::VEL.to_nid()) }
        "'xor" => { data.push(ops::XOR.to_nid()) }
        "'imp" => { data.push(ops::IMP.to_nid()) }
        "'nor" => { data.push(ops::NOR.to_nid()) }
        //"lt"  => { let (x,y)=pop2(&mut data); data.push(base.lt(x,y)) }
        // "gt"  => { let (x,y)=pop2(&mut data); data.push(base.gt(x,y)) }
        //todo "lo" => { let (x,y)=pop2(&mut data); data.push(base.when_lo(y,x)) }
        //todo "hi" => { let (x,y)=pop2(&mut data); data.push(base.when_hi(y,x)) }
        //todo "cnt" => { let x = pop(&mut data); data.push(base.node_count(x)) }
        // "ite" => { let (x,y,z) = pop3(&mut data); data.push(base.ite(x,y,z)); }
        //todo "shuf" => { let (n,x,y) = pop3(&mut data); data.push(base.swap(n,x,y)); }
        // "norm" => { let (x,y,z) = pop3(&mut data); println!("{:?}", base.norm(x,y,z)) }
        // "tup" => { let (v,hi,lo) = base.tup(data.pop().expect("underflow")); println!("({}, {}, {})", v,hi,lo); },
        //todo "rep" => { let (x,y,z)=pop3(&mut data); data.push(base.replace(x,y,z)); }
        //"var?" => { let x=pop(&mut data); data.push(to_io(base.is_var(x))); }
        //todo "dep?" => { let (x,y)=pop2(&mut data); data.push(to_io(base.might_depend(x,y))); }
        // "deep" => { let x = pop(&mut data); data.push(base.deep[x]); }
        "dot" => { let mut s=String::new(); base.dot(pop(&mut data),&mut s); print!("{}", s); }
        "sho" => base.show(pop(&mut data)),
        "bdd" => { let top=pop(&mut data); let n = solve::solve(&mut bdds,base.raw_ast(),top).n; bdds.show(n); data.push(n); }
        "bdd-dot" => { let mut s=String::new(); bdds.dot(pop(&mut data),&mut s); print!("{}", s); }
        "anf" => { let top=pop(&mut data); let n = solve::solve(&mut anfs,base.raw_ast(),top).n; anfs.show(n); data.push(n); }
        "anf-dot" => { let mut s=String::new(); anfs.dot(pop(&mut data),&mut s); print!("{}", s); }
  
        // generic forth commands
        "q" => break 'main,
        "." => { let nid = data.pop().expect("underflow"); println!("{}", nid); }
        "drop" => { let _ = pop(&mut data); }
        "dup" => { let x = pop(&mut data); data.push(x); data.push(x); }
        "swap" => swap(&mut data),
        "reset" => data = Vec::new(),
        //todo "save" => base.save("saved.bdd").expect("failed to save bdd"),
        //todo "load" => base.load("saved.bdd").expect("failed to load bdd"),
        // bdd commands
        "I" => data.push(nid::I),
        "O" => data.push(nid::O),
        _ => {
          // define a new binding
          if word.starts_with(':') {
            let var = word.to_string().split_off(1);
            let val = pop(&mut data);
            scope.insert(var,val); }
          // recall definition
          else if let Some(&val) = scope.get(word) { data.push(val); }
          // attempt to parse nid
          else { match NID::from_str(word) {
            Ok(nid) => data.push(nid),
            Err(err) => println!("{}", err)}}}}}}}

include!(concat!(env!("OUT_DIR"), "/bex-build-info.rs"));
fn main() {
  println!("bex {BEX_VERSION} | compile flags: -O{BEX_OPT_LEVEL} | type 'q' to quit");
  let mut base = ASTBase::empty();
  // for arg in ::std::env::args().skip(1) { load(arg.as_str()) }
  repl(&mut base) }
