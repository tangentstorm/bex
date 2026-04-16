//! Solve an AST problem file, auto-committing snapshots after each step.
//!
//! Usage: bex-solve [options] <file.sdb>
//!
//! Options:
//!   --solver swap|bdd|anf   (default: swap)
//!   --save-every N          save a snapshot every N steps (default: 1)
//!   --resume <snap-id>      resume from a saved snapshot
//!   --timeout <secs>        stop after this many seconds
//!   -o <output.sdb>         write snapshots to a different file

use std::env;
use std::process;
use std::time::Instant;

use rusqlite::Connection;
use bex::nid::NID;
use bex::vid::VID;
use bex::base::Base;
use bex::solve::{self, SrcNid, DstNid, SubSolver};
use bex::swap::SwapSolver;
use bex::bdd::BddBase;
use bex::anf::ANFBase;
use bex::sql;
use bex::sql_snap::{self, SnapshotId, SnapshotMetaInput};
use bex::ast::RawASTBase;
use bex::ops::Ops;

#[derive(Debug, Clone)]
struct Config {
  input: String,
  output: Option<String>,
  solver: String,
  save_every: usize,
  resume: Option<i64>,
  timeout_secs: Option<u64>,
}

fn parse_args() -> Config {
  let args: Vec<String> = env::args().collect();
  let mut cfg = Config {
    input: String::new(), output: None, solver: "swap".into(),
    save_every: 1, resume: None, timeout_secs: None,
  };
  let mut i = 1;
  while i < args.len() {
    match args[i].as_str() {
      "--solver" => { i += 1; cfg.solver = args[i].clone(); }
      "--save-every" => { i += 1; cfg.save_every = args[i].parse().expect("bad --save-every"); }
      "--resume" => { i += 1; cfg.resume = Some(args[i].parse().expect("bad --resume")); }
      "--timeout" => { i += 1; cfg.timeout_secs = Some(args[i].parse().expect("bad --timeout")); }
      "-o" => { i += 1; cfg.output = Some(args[i].clone()); }
      "-h" | "--help" => { usage(); process::exit(0); }
      s if s.starts_with('-') => { eprintln!("unknown option: {}", s); usage(); process::exit(1); }
      _ => { cfg.input = args[i].clone(); }
    }
    i += 1;
  }
  if cfg.input.is_empty() { eprintln!("missing input file"); usage(); process::exit(1); }
  cfg
}

fn usage() {
  eprintln!("Usage: bex-solve [options] <file.sdb>");
  eprintln!("  --solver swap|bdd|anf   solver backend (default: swap)");
  eprintln!("  --save-every N          snapshot every N steps (default: 1)");
  eprintln!("  --resume <snap-id>      resume from a saved snapshot");
  eprintln!("  --timeout <secs>        wall-clock timeout");
  eprintln!("  -o <output.sdb>         write snapshots here instead of input file");
}

fn main() {
  let cfg = parse_args();
  let db_path = cfg.output.as_deref().unwrap_or(&cfg.input);

  // Load AST
  let (src0, _keep) = sql::import_raw_ast_from_path(&cfg.input)
    .unwrap_or_else(|e| { eprintln!("failed to load {}: {}", cfg.input, e); process::exit(1); });
  let top_nid = *src0.tags.get("top")
    .unwrap_or_else(|| { eprintln!("no 'top' tag in AST"); process::exit(1); });
  println!("loaded AST: {} nodes, top={:?}", src0.len(), top_nid);

  // Sort by cost (deterministic — same result on resume)
  let (src, sorted_top) = solve::sort_by_cost(&src0, SrcNid { n: top_nid });
  let total_steps = sorted_top.n.idx();
  println!("sorted AST: {} nodes, {} steps to solve", src.len(), total_steps + 1);

  match cfg.solver.as_str() {
    "swap" => run_swap(&cfg, db_path, &src, sorted_top, total_steps),
    "bdd"  => run_bdd(&cfg, db_path, &src, sorted_top, total_steps),
    "anf"  => run_anf(&cfg, db_path, &src, sorted_top, total_steps),
    _ => { eprintln!("unknown solver: {}", cfg.solver); process::exit(1); }
  }
}

// -----------------------------------------------------------------------
// SwapSolver driver
// -----------------------------------------------------------------------

fn run_swap(cfg: &Config, db_path: &str, src: &RawASTBase, _sorted_top: SrcNid, total_steps: usize) {
  let mut conn = Connection::open(db_path)
    .unwrap_or_else(|e| { eprintln!("open {}: {}", db_path, e); process::exit(1); });

  // Ensure schema exists
  { let tx = conn.transaction().unwrap();
    sql::ensure_schema_tx(&tx).unwrap();
    sql_snap::ensure_snap_schema(&tx).unwrap();
    tx.commit().unwrap(); }

  let (mut solver, mut step, mut ctx, mut parent_snap) = if let Some(snap_id) = cfg.resume {
    // Resume from snapshot
    let (sc, roots) = sql_snap::read_scaffold(&conn, SnapshotId(snap_id))
      .unwrap_or_else(|e| { eprintln!("resume failed: {}", e); process::exit(1); });
    let snaps = sql_snap::list_snapshots(&conn).unwrap();
    let snap = snaps.iter().find(|s| s.id.0 == snap_id)
      .unwrap_or_else(|| { eprintln!("snapshot {} not found", snap_id); process::exit(1); });
    let saved_step = snap.step.unwrap_or(0) as usize;
    let dx = roots.iter().find(|(n, _)| n == "ctx").map(|(_, n)| *n)
      .unwrap_or_else(|| { eprintln!("no 'ctx' root in snapshot"); process::exit(1); });
    let s = SwapSolver::from_parts(sc, dx);
    println!("resumed from snapshot {} at step {}", snap_id, saved_step);
    (s, saved_step, DstNid { n: dx }, Some(SnapshotId(snap_id)))
  } else {
    // Fresh start
    let top_v = VID::vir(total_steps as u32);
    let mut s = SwapSolver::new();
    let ctx_nid = s.init(top_v);
    (s, total_steps, DstNid { n: ctx_nid }, None)
  };

  let start = Instant::now();
  let mut steps_done: usize = 0;

  // Main solve loop
  while !(ctx.n.is_var() || ctx.n.is_const()) {
    if let Some(timeout) = cfg.timeout_secs {
      if start.elapsed().as_secs() >= timeout {
        println!("timeout after {} steps ({:.1}s)", steps_done, start.elapsed().as_secs_f64());
        break;
      }
    }
    let v = VID::vir(step as u32);
    let step_start = Instant::now();
    let _old = ctx;
    ctx = refine_one_swap(&mut solver, v, src, ctx);
    let ms = step_start.elapsed().as_millis();

    steps_done += 1;
    let remaining = step;
    println!("step {:>5}/{:<5}  vir={:<6}  {:>6}ms  nodes={}",
             total_steps - remaining, total_steps,
             format!("v{:X}", step), ms,
             solver.scaffold().num_nodes());

    // Auto-save
    if cfg.save_every > 0 && steps_done.is_multiple_of(cfg.save_every) {
      let tx = conn.transaction().unwrap();
      let sid = sql_snap::write_scaffold(&tx, SnapshotMetaInput {
        parent: parent_snap,
        step: Some((total_steps - remaining) as u32),
        rv: Some(v),
        note: None,
      }, solver.scaffold(), &[("ctx".into(), ctx.n)]).unwrap();
      tx.commit().unwrap();
      parent_snap = Some(sid);
    }

    if step == 0 { break } else { step -= 1; }
  }

  // Final save
  { let tx = conn.transaction().unwrap();
    let sid = sql_snap::write_scaffold(&tx, SnapshotMetaInput {
      parent: parent_snap,
      step: Some(total_steps as u32),
      rv: None,
      note: Some("final".into()),
    }, solver.scaffold(), &[("ctx".into(), ctx.n)]).unwrap();
    tx.commit().unwrap();
    println!("saved final snapshot {}", sid.0); }

  let elapsed = start.elapsed();
  println!("done: {} steps in {:.3}s, result={:?}", steps_done, elapsed.as_secs_f64(), ctx.n);
}

fn refine_one_swap(solver: &mut SwapSolver, v: VID, src: &RawASTBase, d: DstNid) -> DstNid {
  let ctx = d.n;
  let ops = src.get_ops(NID::ixn(v.vir_ix()));
  let cn = |x0: &NID| -> NID {
    if x0.is_fun() { *x0 } else { solve::convert_nid(SrcNid { n: *x0 }).n }
  };
  let def: Ops = Ops::RPN(ops.to_rpn().map(cn).collect());
  DstNid { n: solver.subst(ctx, v, &def) }
}

// -----------------------------------------------------------------------
// BDD driver
// -----------------------------------------------------------------------

fn run_bdd(cfg: &Config, db_path: &str, src: &RawASTBase, _sorted_top: SrcNid, total_steps: usize) {
  let mut conn = Connection::open(db_path)
    .unwrap_or_else(|e| { eprintln!("open {}: {}", db_path, e); process::exit(1); });
  { let tx = conn.transaction().unwrap();
    sql::ensure_schema_tx(&tx).unwrap();
    sql_snap::ensure_snap_schema(&tx).unwrap();
    tx.commit().unwrap(); }

  let mut bdd = BddBase::new();
  let mut step = total_steps;
  let top_v = VID::vir(step as u32);
  let mut ctx = DstNid { n: bdd.init(top_v) };
  let mut parent_snap: Option<SnapshotId> = None;
  let start = Instant::now();
  let mut steps_done: usize = 0;

  while !(ctx.n.is_var() || ctx.n.is_const()) {
    if let Some(timeout) = cfg.timeout_secs {
      if start.elapsed().as_secs() >= timeout {
        println!("timeout after {} steps", steps_done);
        break;
      }
    }
    let v = VID::vir(step as u32);
    let step_start = Instant::now();
    ctx = solve::refine_one(&mut bdd, v, src, ctx);
    let ms = step_start.elapsed().as_millis();
    steps_done += 1;
    println!("step {:>5}/{:<5}  vir={:<6}  {:>6}ms",
             total_steps - step, total_steps, format!("v{:X}", step), ms);

    if cfg.save_every > 0 && steps_done.is_multiple_of(cfg.save_every) {
      let tx = conn.transaction().unwrap();
      let sid = sql_snap::write_bdd(&tx, SnapshotMetaInput {
        parent: parent_snap, step: Some((total_steps - step) as u32),
        rv: Some(v), note: None,
      }, &bdd, &[("ctx".into(), ctx.n)]).unwrap();
      tx.commit().unwrap();
      parent_snap = Some(sid);
    }
    if step == 0 { break } else { step -= 1; }
  }

  println!("done: {} steps in {:.3}s, result={:?}",
           steps_done, start.elapsed().as_secs_f64(), ctx.n);
}

// -----------------------------------------------------------------------
// ANF driver
// -----------------------------------------------------------------------

fn run_anf(cfg: &Config, db_path: &str, src: &RawASTBase, _sorted_top: SrcNid, total_steps: usize) {
  let mut conn = Connection::open(db_path)
    .unwrap_or_else(|e| { eprintln!("open {}: {}", db_path, e); process::exit(1); });
  { let tx = conn.transaction().unwrap();
    sql::ensure_schema_tx(&tx).unwrap();
    sql_snap::ensure_snap_schema(&tx).unwrap();
    tx.commit().unwrap(); }

  let mut anf = ANFBase::new();
  let mut step = total_steps;
  let top_v = VID::vir(step as u32);
  let mut ctx = DstNid { n: anf.init(top_v) };
  let mut parent_snap: Option<SnapshotId> = None;
  let start = Instant::now();
  let mut steps_done: usize = 0;

  while !(ctx.n.is_var() || ctx.n.is_const()) {
    if let Some(timeout) = cfg.timeout_secs {
      if start.elapsed().as_secs() >= timeout {
        println!("timeout after {} steps", steps_done);
        break;
      }
    }
    let v = VID::vir(step as u32);
    let step_start = Instant::now();
    ctx = solve::refine_one(&mut anf, v, src, ctx);
    let ms = step_start.elapsed().as_millis();
    steps_done += 1;
    println!("step {:>5}/{:<5}  vir={:<6}  {:>6}ms",
             total_steps - step, total_steps, format!("v{:X}", step), ms);

    if cfg.save_every > 0 && steps_done.is_multiple_of(cfg.save_every) {
      let tx = conn.transaction().unwrap();
      let sid = sql_snap::write_anf(&tx, SnapshotMetaInput {
        parent: parent_snap, step: Some((total_steps - step) as u32),
        rv: Some(v), note: None,
      }, &anf, &[("ctx".into(), ctx.n)]).unwrap();
      tx.commit().unwrap();
      parent_snap = Some(sid);
    }
    if step == 0 { break } else { step -= 1; }
  }

  println!("done: {} steps in {:.3}s, result={:?}",
           steps_done, start.elapsed().as_secs_f64(), ctx.n);
}
