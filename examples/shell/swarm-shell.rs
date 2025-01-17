/// This is a (completely useless) shell for interacting with the swarm
/// while it's running. I wrote it to debug VhlSwarm and figure out how
/// to send messages to it while it was in a separate thread. (The answer
/// was to expose q_sender and poll that channel in swarm::run())

use std::io;
use std::io::Write;
use std::thread;

use bex::bdd::{NormIteKey, ITE};
use bex::swarm::SwarmCmd;
use bex::vhl_swarm::VhlQ;
use bex::NID;

extern crate bex;

fn readln()->String {
  let mut buf = String::new();
  print!("> ");
  io::stdout().flush()                 .expect("couldn't flush stdout.");
  io::stdin().read_line(&mut buf)      .expect("failed to read line.");
  buf}

include!(concat!(env!("OUT_DIR"), "/bex-build-info.rs"));
fn main() {
  println!("bex swarm-shell {BEX_VERSION} | compile flags: -O{BEX_OPT_LEVEL} | type 'q' to quit");
  let mut bdd = bex::bdd::BddBase::new();
  let to_swarm = bdd.swarm.q_sender();

  // Spawn a new thread for VhlSwarm. `bdd` is moved into the closure.
  thread::spawn(move || {
    // TODO: give name to VhlQ<NormIteKey> in bdd modules
    // !! also maybe swap the argument order for the types?
    bdd.swarm.run(|_wid, _qid, rmsg|->SwarmCmd<VhlQ<NormIteKey>,()> {
      if let Some(r) = rmsg {
        println!("received: {:?}", r);
        SwarmCmd::Pass }
      else { SwarmCmd::Pass }})});

  'main: loop {
    for word in readln().split_whitespace() {
      match word {
        "q" => { break 'main}
        "o" => { to_swarm.send(VhlQ::Job(NormIteKey(ITE {
          i: NID::var(1),
          t: NID::var(2),
          e: NID::var(3)}))).unwrap() }
        _ => { println!("you typed: {:?}", word) }}}}}
