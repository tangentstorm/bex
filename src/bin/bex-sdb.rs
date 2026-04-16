//! CLI tool for inspecting and manipulating bex `.sdb` SQLite databases.
//!
//! Usage:
//!   bex-sdb list    <file.sdb>
//!   bex-sdb info    <file.sdb> <snap-id>
//!   bex-sdb dump    <file.sdb> <snap-id>
//!   bex-sdb ast     <file.sdb>
//!   bex-sdb replay  <file.sdb> <head-id>

use std::env;
use std::process;
use rusqlite::Connection;
use bex::sql_snap::{self, SnapshotId};

fn main() {
  let args: Vec<String> = env::args().collect();
  if args.len() < 3 {
    usage();
    process::exit(1);
  }
  let cmd = &args[1];
  let path = &args[2];

  match cmd.as_str() {
    "list" => cmd_list(path),
    "info" => {
      let id = parse_snap_id(&args, 3);
      cmd_info(path, id);
    }
    "dump" => {
      let id = parse_snap_id(&args, 3);
      cmd_dump(path, id);
    }
    "ast" => cmd_ast(path),
    "replay" => {
      let id = parse_snap_id(&args, 3);
      cmd_replay(path, id);
    }
    _ => { eprintln!("unknown command: {}", cmd); usage(); process::exit(1); }
  }
}

fn usage() {
  eprintln!("Usage: bex-sdb <command> <file.sdb> [args...]");
  eprintln!("Commands:");
  eprintln!("  list    <file>            List all snapshots");
  eprintln!("  info    <file> <snap-id>  Show snapshot metadata");
  eprintln!("  dump    <file> <snap-id>  Dump snapshot nodes");
  eprintln!("  ast     <file>            Dump AST tags and node count");
  eprintln!("  replay  <file> <head-id>  Trace snapshot chain from root to head");
}

fn parse_snap_id(args: &[String], idx: usize) -> SnapshotId {
  if idx >= args.len() {
    eprintln!("missing snapshot id argument");
    usage();
    process::exit(1);
  }
  let n: i64 = args[idx].parse().unwrap_or_else(|_| {
    eprintln!("bad snapshot id: {}", args[idx]);
    process::exit(1);
  });
  SnapshotId(n)
}

fn cmd_list(path: &str) {
  let conn = Connection::open(path).unwrap_or_else(|e| {
    eprintln!("cannot open {}: {}", path, e); process::exit(1); });
  let snaps = sql_snap::list_snapshots(&conn).unwrap();
  if snaps.is_empty() {
    println!("(no snapshots)");
    return;
  }
  println!("{:>4}  {:>6}  {:>8}  {:>4}  {:>6}  {:>6}  {:>5}  {}",
           "id", "parent", "kind", "step", "rv", "#nodes", "#vids", "note");
  for s in &snaps {
    let parent = s.parent.map_or("-".to_string(), |p| format!("{}", p.0));
    let rv = s.rv.map_or("-".to_string(), |v| format!("{}", v));
    let step = s.step.map_or("-".to_string(), |s| format!("{}", s));
    let note = s.note.as_deref().unwrap_or("-");
    println!("{:>4}  {:>6}  {:>8}  {:>4}  {:>6}  {:>6}  {:>5}  {}",
             s.id.0, parent, s.kind.as_str(), step, rv,
             s.num_nodes, s.vids.len(), note);
  }
}

fn cmd_info(path: &str, id: SnapshotId) {
  let conn = Connection::open(path).unwrap_or_else(|e| {
    eprintln!("cannot open {}: {}", path, e); process::exit(1); });
  let snaps = sql_snap::list_snapshots(&conn).unwrap();
  let snap = snaps.iter().find(|s| s.id == id).unwrap_or_else(|| {
    eprintln!("snapshot {} not found", id.0); process::exit(1); });
  println!("Snapshot {}", snap.id.0);
  println!("  kind:       {}", snap.kind.as_str());
  println!("  parent:     {}", snap.parent.map_or("-".into(), |p| format!("{}", p.0)));
  println!("  step:       {}", snap.step.map_or("-".into(), |s| format!("{}", s)));
  println!("  rv:         {}", snap.rv.map_or("-".into(), |v| format!("{}", v)));
  println!("  created_at: {}", snap.created_at);
  println!("  note:       {}", snap.note.as_deref().unwrap_or("-"));
  println!("  nodes:      {}", snap.num_nodes);
  println!("  vids ({}):  {:?}", snap.vids.len(), snap.vids);

  // Print roots
  let mut stmt = conn.prepare(
    "SELECT name, nid FROM snapshot_root WHERE snapshot_id=?1 ORDER BY name").unwrap();
  let mut rows = stmt.query(rusqlite::params![id.0]).unwrap();
  println!("  roots:");
  while let Some(row) = rows.next().unwrap() {
    let name: String = row.get(0).unwrap();
    let nid: String = row.get(1).unwrap();
    println!("    {} = {}", name, nid);
  }
}

fn cmd_dump(path: &str, id: SnapshotId) {
  let conn = Connection::open(path).unwrap_or_else(|e| {
    eprintln!("cannot open {}: {}", path, e); process::exit(1); });
  let mut stmt = conn.prepare(
    "SELECT ix, vid, hi, lo, irc, erc FROM snapshot_node WHERE snapshot_id=?1 ORDER BY ix"
  ).unwrap();
  let mut rows = stmt.query(rusqlite::params![id.0]).unwrap();
  println!("{:>6}  {:>6}  {:>10}  {:>10}  {:>4}  {:>4}",
           "ix", "vid", "hi", "lo", "irc", "erc");
  while let Some(row) = rows.next().unwrap() {
    let ix: i64 = row.get(0).unwrap();
    let vid: String = row.get(1).unwrap();
    let hi: String = row.get(2).unwrap();
    let lo: String = row.get(3).unwrap();
    let irc: Option<i64> = row.get(4).unwrap();
    let erc: Option<i64> = row.get(5).unwrap();
    println!("{:>6}  {:>6}  {:>10}  {:>10}  {:>4}  {:>4}",
             ix, vid, hi, lo,
             irc.map_or("-".into(), |n| format!("{}", n)),
             erc.map_or("-".into(), |n| format!("{}", n)));
  }
}

fn cmd_ast(path: &str) {
  let conn = Connection::open(path).unwrap_or_else(|e| {
    eprintln!("cannot open {}: {}", path, e); process::exit(1); });
  let node_count: i64 = conn.query_row(
    "SELECT COUNT(*) FROM ast_node", [], |row| row.get(0)
  ).unwrap_or(0);
  println!("AST nodes: {}", node_count);
  let mut stmt = conn.prepare("SELECT name, nid FROM tag ORDER BY name").unwrap();
  let mut rows = stmt.query([]).unwrap();
  println!("Tags:");
  while let Some(row) = rows.next().unwrap() {
    let name: String = row.get(0).unwrap();
    let nid: String = row.get(1).unwrap();
    println!("  {} = {}", name, nid);
  }
}

fn cmd_replay(path: &str, head_id: SnapshotId) {
  let conn = Connection::open(path).unwrap_or_else(|e| {
    eprintln!("cannot open {}: {}", path, e); process::exit(1); });
  let snaps = sql_snap::list_snapshots(&conn).unwrap();
  // Walk backwards from head to root
  let mut chain: Vec<&sql_snap::SnapshotMeta> = Vec::new();
  let mut cur = Some(head_id);
  while let Some(id) = cur {
    let snap = snaps.iter().find(|s| s.id == id).unwrap_or_else(|| {
      eprintln!("snapshot {} not found in chain", id.0); process::exit(1); });
    chain.push(snap);
    cur = snap.parent;
  }
  chain.reverse();
  println!("{:>4}  {:>4}  {:>6}  {:>8}",
           "id", "step", "rv", "kind");
  for s in &chain {
    let rv = s.rv.map_or("-".to_string(), |v| format!("{}", v));
    let step = s.step.map_or("-".to_string(), |s| format!("{}", s));
    println!("{:>4}  {:>4}  {:>6}  {:>8}",
             s.id.0, step, rv, s.kind.as_str());
  }
}

