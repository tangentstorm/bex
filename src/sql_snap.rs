//! Snapshot persistence for scaffolds, BDDs, and ANFs in SQLite.
//!
//! This module adds four tables (`snapshot`, `snapshot_vid`,
//! `snapshot_node`, `snapshot_root`) alongside the existing AST schema
//! in `sql.rs`.  Each snapshot captures the full graph at a point in
//! time and optionally chains to a parent snapshot to form a replay
//! trace.

use std::path::Path;
use std::str::FromStr;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, Transaction, Result};

use crate::nid::NID;
use crate::vid::VID;
use crate::vhl::Vhl;
use crate::swap::VhlScaffold;
use crate::bdd::BddBase;
use crate::anf::ANFBase;
use crate::vhl::Walkable;

// -----------------------------------------------------------------------
// Public types
// -----------------------------------------------------------------------

/// Opaque handle returned by every write and used by every read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotId(pub i64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotKind { Scaffold, Bdd, Anf }

impl SnapshotKind {
  pub fn as_str(&self) -> &'static str {
    match self { Self::Scaffold => "scaffold", Self::Bdd => "bdd", Self::Anf => "anf" }
  }
  pub fn from_str_kind(s: &str) -> Self {
    match s { "scaffold" => Self::Scaffold, "bdd" => Self::Bdd, "anf" => Self::Anf,
              _ => panic!("unknown snapshot kind: {}", s) }
  }
}

#[derive(Clone, Debug)]
pub struct SnapshotMeta {
  pub id: SnapshotId,
  pub parent: Option<SnapshotId>,
  pub kind: SnapshotKind,
  pub step: Option<u32>,
  pub rv: Option<VID>,
  pub note: Option<String>,
  pub created_at: i64,
  pub vids: Vec<VID>,
  pub num_nodes: usize,
}

/// Input struct for creating a new snapshot (id and created_at are auto-filled).
pub struct SnapshotMetaInput {
  pub parent: Option<SnapshotId>,
  pub step: Option<u32>,
  pub rv: Option<VID>,
  pub note: Option<String>,
}

// -----------------------------------------------------------------------
// Schema
// -----------------------------------------------------------------------

const SNAP_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS snapshot(
  id         INTEGER PRIMARY KEY,
  parent_id  INTEGER,
  kind       TEXT NOT NULL,
  step       INTEGER,
  rv         TEXT,
  created_at INTEGER NOT NULL,
  note       TEXT,
  FOREIGN KEY(parent_id) REFERENCES snapshot(id));
CREATE INDEX IF NOT EXISTS snapshot_parent ON snapshot(parent_id);

CREATE TABLE IF NOT EXISTS snapshot_vid(
  snapshot_id INTEGER NOT NULL,
  ord         INTEGER NOT NULL,
  vid         TEXT NOT NULL,
  PRIMARY KEY(snapshot_id, ord),
  FOREIGN KEY(snapshot_id) REFERENCES snapshot(id));

CREATE TABLE IF NOT EXISTS snapshot_node(
  snapshot_id INTEGER NOT NULL,
  ix          INTEGER NOT NULL,
  vid         TEXT NOT NULL,
  hi          TEXT NOT NULL,
  lo          TEXT NOT NULL,
  irc         INTEGER,
  erc         INTEGER,
  PRIMARY KEY(snapshot_id, ix),
  FOREIGN KEY(snapshot_id) REFERENCES snapshot(id));
CREATE INDEX IF NOT EXISTS snapshot_node_vid ON snapshot_node(snapshot_id, vid);

CREATE TABLE IF NOT EXISTS snapshot_root(
  snapshot_id INTEGER NOT NULL,
  name        TEXT NOT NULL,
  nid         TEXT NOT NULL,
  PRIMARY KEY(snapshot_id, name),
  FOREIGN KEY(snapshot_id) REFERENCES snapshot(id));
"#;

const SNAP_FMT_VERSION: &str = "0.1";

/// Create the snapshot tables if they don't already exist.
pub fn ensure_snap_schema(tx: &Transaction<'_>) -> Result<()> {
  tx.execute_batch(SNAP_SCHEMA)?;
  tx.execute(
    "INSERT OR REPLACE INTO meta(k, v) VALUES(?1, ?2)",
    params!["snap.fmt.version", SNAP_FMT_VERSION],
  )?;
  Ok(())
}

// -----------------------------------------------------------------------
// VID helpers (parse "x3" / "v7" / "T" / "NoV")
// -----------------------------------------------------------------------

fn vid_to_string(v: VID) -> String { format!("{}", v) }

fn vid_from_string(s: &str) -> VID {
  match s {
    "T" => VID::top(),
    "NoV" => VID::nov(),
    _ => {
      let nid = NID::from_str(s).unwrap_or_else(|e| panic!("bad vid string '{}': {}", s, e));
      nid.vid()
    }
  }
}

fn now_unix() -> i64 {
  SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

// -----------------------------------------------------------------------
// Insert helpers (shared by all three writers)
// -----------------------------------------------------------------------

fn insert_snapshot_row(
  tx: &Transaction<'_>, m: &SnapshotMetaInput, kind: &SnapshotKind,
) -> Result<SnapshotId> {
  tx.execute(
    "INSERT INTO snapshot(parent_id, kind, step, rv, created_at, note) VALUES(?1,?2,?3,?4,?5,?6)",
    params![
      m.parent.map(|p| p.0),
      kind.as_str(),
      m.step.map(|s| s as i64),
      m.rv.map(|v| vid_to_string(v)),
      now_unix(),
      m.note.as_deref(),
    ],
  )?;
  Ok(SnapshotId(tx.last_insert_rowid()))
}

fn insert_vids(tx: &Transaction<'_>, sid: SnapshotId, vids: &[VID]) -> Result<()> {
  let mut stmt = tx.prepare(
    "INSERT INTO snapshot_vid(snapshot_id, ord, vid) VALUES(?1,?2,?3)")?;
  for (ord, v) in vids.iter().enumerate() {
    stmt.execute(params![sid.0, ord as i64, vid_to_string(*v)])?;
  }
  Ok(())
}

fn insert_nodes(
  tx: &Transaction<'_>, sid: SnapshotId,
  nodes: &[(usize, VID, String, String, Option<usize>, Option<usize>)],
) -> Result<()> {
  let mut stmt = tx.prepare(
    "INSERT INTO snapshot_node(snapshot_id, ix, vid, hi, lo, irc, erc) VALUES(?1,?2,?3,?4,?5,?6,?7)")?;
  for (ix, v, hi, lo, irc, erc) in nodes {
    stmt.execute(params![
      sid.0, *ix as i64, vid_to_string(*v),
      hi.as_str(), lo.as_str(),
      irc.map(|x| x as i64), erc.map(|x| x as i64),
    ])?;
  }
  Ok(())
}

fn insert_roots(
  tx: &Transaction<'_>, sid: SnapshotId, roots: &[(String, NID)],
) -> Result<()> {
  let mut stmt = tx.prepare(
    "INSERT INTO snapshot_root(snapshot_id, name, nid) VALUES(?1,?2,?3)")?;
  for (name, nid) in roots {
    stmt.execute(params![sid.0, name.as_str(), format!("{}", nid)])?;
  }
  Ok(())
}

// -----------------------------------------------------------------------
// Writers
// -----------------------------------------------------------------------

/// Write a `VhlScaffold` snapshot. Returns the new snapshot id.
pub fn write_scaffold(
  tx: &Transaction<'_>, m: SnapshotMetaInput,
  sc: &VhlScaffold, roots: &[(String, NID)],
) -> Result<SnapshotId> {
  ensure_snap_schema(tx)?;
  let sid = insert_snapshot_row(tx, &m, &SnapshotKind::Scaffold)?;
  insert_vids(tx, sid, sc.vids())?;
  let raw = sc.iter_nodes();
  let nodes: Vec<_> = raw.iter().map(|(ix, vhl, irc, erc)| {
    (*ix, vhl.v, format!("{}", vhl.hi), format!("{}", vhl.lo),
     Some(*irc), Some(*erc))
  }).collect();
  insert_nodes(tx, sid, &nodes)?;
  insert_roots(tx, sid, roots)?;
  Ok(sid)
}

/// Write a BDD snapshot by walking from the given roots.
pub fn write_bdd(
  tx: &Transaction<'_>, m: SnapshotMetaInput,
  bdd: &BddBase, roots: &[(String, NID)],
) -> Result<SnapshotId> {
  ensure_snap_schema(tx)?;
  let sid = insert_snapshot_row(tx, &m, &SnapshotKind::Bdd)?;

  // Walk all roots bottom-up, assigning snapshot-local indices.
  let mut global_to_local: HashMap<NID, usize> = HashMap::new();
  let mut local_ix: usize = 1; // 0 reserved for sentinel
  let mut node_rows: Vec<(usize, VID, String, String, Option<usize>, Option<usize>)> = Vec::new();
  let mut vids_seen: Vec<VID> = Vec::new();
  let mut vid_set = std::collections::HashSet::new();

  let root_nids: Vec<NID> = roots.iter().map(|(_, n)| *n).collect();
  bdd.walk_up_each(&root_nids, &mut |nid, vid, hi, lo| {
    let raw = nid.raw();
    if global_to_local.contains_key(&raw) { return; }
    if raw.is_const() || raw.is_vid() { return; } // leaves don't need rows
    let ix = local_ix;
    local_ix += 1;
    global_to_local.insert(raw, ix);
    if vid_set.insert(vid) { vids_seen.push(vid); }
    let hi_s = map_bdd_child(hi, &global_to_local);
    let lo_s = map_bdd_child(lo, &global_to_local);
    node_rows.push((ix, vid, hi_s, lo_s, None, None));
  });

  insert_vids(tx, sid, &vids_seen)?;
  insert_nodes(tx, sid, &node_rows)?;

  // Map root nids to snapshot-local encoding
  let mapped_roots: Vec<(String, NID)> = roots.iter().map(|(name, nid)| {
    let mapped = map_bdd_root(*nid, &global_to_local);
    (name.clone(), mapped)
  }).collect();
  insert_roots(tx, sid, &mapped_roots)?;
  Ok(sid)
}

fn map_bdd_child(n: NID, g2l: &HashMap<NID, usize>) -> String {
  if n.is_const() || n.is_vid() { return format!("{}", n); }
  let raw = n.raw();
  let ix = g2l.get(&raw).unwrap_or_else(|| panic!("unmapped BDD child {:?}", n));
  let local = NID::ixn(*ix);
  format!("{}", if n.is_inv() { !local } else { local })
}

fn map_bdd_root(n: NID, g2l: &HashMap<NID, usize>) -> NID {
  if n.is_const() || n.is_vid() { return n; }
  let raw = n.raw();
  let ix = g2l.get(&raw).unwrap_or_else(|| panic!("unmapped BDD root {:?}", n));
  let local = NID::ixn(*ix);
  if n.is_inv() { !local } else { local }
}

/// Write an ANF snapshot by walking from the given roots.
pub fn write_anf(
  tx: &Transaction<'_>, m: SnapshotMetaInput,
  anf: &ANFBase, roots: &[(String, NID)],
) -> Result<SnapshotId> {
  ensure_snap_schema(tx)?;
  let sid = insert_snapshot_row(tx, &m, &SnapshotKind::Anf)?;

  // ANFBase stores nodes in a flat Vec<Vhl>. Walk roots to find
  // reachable nodes and assign snapshot-local indices.
  let mut global_to_local: HashMap<NID, usize> = HashMap::new();
  let mut local_ix: usize = 1;
  let mut node_rows: Vec<(usize, VID, String, String, Option<usize>, Option<usize>)> = Vec::new();
  let mut vids_seen: Vec<VID> = Vec::new();
  let mut vid_set = std::collections::HashSet::new();

  let root_nids: Vec<NID> = roots.iter().map(|(_, n)| *n).collect();
  anf.walk_up_each(&root_nids, &mut |nid, vid, hi, lo| {
    let raw = nid.raw();
    if global_to_local.contains_key(&raw) { return; }
    if raw.is_const() || raw.is_vid() { return; }
    let ix = local_ix;
    local_ix += 1;
    global_to_local.insert(raw, ix);
    if vid_set.insert(vid) { vids_seen.push(vid); }
    let hi_s = map_bdd_child(hi, &global_to_local);
    let lo_s = map_bdd_child(lo, &global_to_local);
    node_rows.push((ix, vid, hi_s, lo_s, None, None));
  });

  insert_vids(tx, sid, &vids_seen)?;
  insert_nodes(tx, sid, &node_rows)?;
  let mapped_roots: Vec<(String, NID)> = roots.iter().map(|(name, nid)| {
    let mapped = map_bdd_root(*nid, &global_to_local);
    (name.clone(), mapped)
  }).collect();
  insert_roots(tx, sid, &mapped_roots)?;
  Ok(sid)
}

// -----------------------------------------------------------------------
// Readers
// -----------------------------------------------------------------------

/// List all snapshots in the database.
pub fn list_snapshots(conn: &Connection) -> Result<Vec<SnapshotMeta>> {
  // If the snapshot table doesn't exist, return empty.
  let has_table: bool = conn.query_row(
    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='snapshot'",
    [], |row| row.get::<_, i64>(0),
  )? > 0;
  if !has_table { return Ok(Vec::new()); }

  let mut stmt = conn.prepare(
    "SELECT id, parent_id, kind, step, rv, created_at, note FROM snapshot ORDER BY id")?;
  let rows = stmt.query_map([], |row| {
    let id = SnapshotId(row.get::<_, i64>(0)?);
    let parent: Option<i64> = row.get(1)?;
    let kind_s: String = row.get(2)?;
    let step: Option<i64> = row.get(3)?;
    let rv_s: Option<String> = row.get(4)?;
    let created_at: i64 = row.get(5)?;
    let note: Option<String> = row.get(6)?;
    Ok(SnapshotMeta {
      id, parent: parent.map(SnapshotId),
      kind: SnapshotKind::from_str_kind(&kind_s),
      step: step.map(|s| s as u32),
      rv: rv_s.map(|s| vid_from_string(&s)),
      note, created_at,
      vids: Vec::new(), // filled below
      num_nodes: 0,
    })
  })?;
  let mut snaps: Vec<SnapshotMeta> = rows.collect::<Result<Vec<_>>>()?;

  // Fill vids and num_nodes for each snapshot
  for snap in snaps.iter_mut() {
    snap.vids = read_vids(conn, snap.id)?;
    snap.num_nodes = conn.query_row(
      "SELECT COUNT(*) FROM snapshot_node WHERE snapshot_id=?1",
      params![snap.id.0], |row| row.get::<_, i64>(0),
    )? as usize;
  }
  Ok(snaps)
}

fn read_vids(conn: &Connection, sid: SnapshotId) -> Result<Vec<VID>> {
  let mut stmt = conn.prepare(
    "SELECT vid FROM snapshot_vid WHERE snapshot_id=?1 ORDER BY ord")?;
  let rows = stmt.query_map(params![sid.0], |row| {
    let s: String = row.get(0)?;
    Ok(vid_from_string(&s))
  })?;
  rows.collect()
}

fn read_roots(conn: &Connection, sid: SnapshotId) -> Result<Vec<(String, NID)>> {
  let mut stmt = conn.prepare(
    "SELECT name, nid FROM snapshot_root WHERE snapshot_id=?1 ORDER BY name")?;
  let rows = stmt.query_map(params![sid.0], |row| {
    let name: String = row.get(0)?;
    let nid_s: String = row.get(1)?;
    let nid = NID::from_str(&nid_s)
      .map_err(|e| rusqlite::Error::InvalidParameterName(format!("bad root nid '{}': {}", nid_s, e)))?;
    Ok((name, nid))
  })?;
  rows.collect()
}

/// Reconstruct a `VhlScaffold` from a snapshot.
pub fn read_scaffold(
  conn: &Connection, id: SnapshotId,
) -> Result<(VhlScaffold, Vec<(String, NID)>)> {
  let vids = read_vids(conn, id)?;
  let roots = read_roots(conn, id)?;

  let mut stmt = conn.prepare(
    "SELECT ix, vid, hi, lo, irc, erc FROM snapshot_node WHERE snapshot_id=?1 ORDER BY ix")?;
  let rows = stmt.query_map(params![id.0], |row| {
    let ix: i64 = row.get(0)?;
    let vid_s: String = row.get(1)?;
    let hi_s: String = row.get(2)?;
    let lo_s: String = row.get(3)?;
    let irc: Option<i64> = row.get(4)?;
    let erc: Option<i64> = row.get(5)?;
    Ok((ix as usize, vid_s, hi_s, lo_s,
        irc.unwrap_or(0) as usize,
        erc.unwrap_or(0) as usize))
  })?;

  let mut nodes: Vec<(usize, Vhl, usize, usize)> = Vec::new();
  for row in rows {
    let (ix, vid_s, hi_s, lo_s, irc, erc) = row?;
    let v = vid_from_string(&vid_s);
    let hi = NID::from_str(&hi_s)
      .map_err(|e| rusqlite::Error::InvalidParameterName(format!("bad hi '{}': {}", hi_s, e)))?;
    let lo = NID::from_str(&lo_s)
      .map_err(|e| rusqlite::Error::InvalidParameterName(format!("bad lo '{}': {}", lo_s, e)))?;
    nodes.push((ix, Vhl::new(v, hi, lo), irc, erc));
  }

  let sc = VhlScaffold::from_raw(vids, &nodes);
  Ok((sc, roots))
}

/// Load a BDD snapshot into an existing `BddBase`, returning root mappings.
pub fn read_bdd_into(
  conn: &Connection, id: SnapshotId, bdd: &mut BddBase,
) -> Result<Vec<(String, NID)>> {
  let roots_raw = read_roots(conn, id)?;

  let mut stmt = conn.prepare(
    "SELECT ix, vid, hi, lo FROM snapshot_node WHERE snapshot_id=?1 ORDER BY ix")?;
  let rows = stmt.query_map(params![id.0], |row| {
    let ix: i64 = row.get(0)?;
    let vid_s: String = row.get(1)?;
    let hi_s: String = row.get(2)?;
    let lo_s: String = row.get(3)?;
    Ok((ix as usize, vid_s, hi_s, lo_s))
  })?;

  // Map snapshot-local ix → global NID produced by bdd.ite().
  let mut local_to_global: HashMap<usize, NID> = HashMap::new();

  for row in rows {
    let (ix, vid_s, hi_s, lo_s) = row?;
    let v = vid_from_string(&vid_s);
    let hi_raw = NID::from_str(&hi_s)
      .map_err(|e| rusqlite::Error::InvalidParameterName(format!("bad hi '{}': {}", hi_s, e)))?;
    let lo_raw = NID::from_str(&lo_s)
      .map_err(|e| rusqlite::Error::InvalidParameterName(format!("bad lo '{}': {}", lo_s, e)))?;
    let hi = resolve_local_nid(hi_raw, &local_to_global);
    let lo = resolve_local_nid(lo_raw, &local_to_global);
    let bv = NID::from_vid(v);
    let nid = bdd.ite(bv, hi, lo);
    local_to_global.insert(ix, nid);
  }

  // Remap roots
  let roots: Vec<(String, NID)> = roots_raw.into_iter().map(|(name, nid)| {
    (name, resolve_local_nid(nid, &local_to_global))
  }).collect();
  Ok(roots)
}

/// Load an ANF snapshot into an existing `ANFBase`, returning root mappings.
pub fn read_anf_into(
  conn: &Connection, id: SnapshotId, anf: &mut ANFBase,
) -> Result<Vec<(String, NID)>> {
  let roots_raw = read_roots(conn, id)?;

  let mut stmt = conn.prepare(
    "SELECT ix, vid, hi, lo FROM snapshot_node WHERE snapshot_id=?1 ORDER BY ix")?;
  let rows = stmt.query_map(params![id.0], |row| {
    let ix: i64 = row.get(0)?;
    let vid_s: String = row.get(1)?;
    let hi_s: String = row.get(2)?;
    let lo_s: String = row.get(3)?;
    Ok((ix as usize, vid_s, hi_s, lo_s))
  })?;

  let mut local_to_global: HashMap<usize, NID> = HashMap::new();

  for row in rows {
    let (ix, vid_s, hi_s, lo_s) = row?;
    let v = vid_from_string(&vid_s);
    let hi_raw = NID::from_str(&hi_s)
      .map_err(|e| rusqlite::Error::InvalidParameterName(format!("bad hi '{}': {}", hi_s, e)))?;
    let lo_raw = NID::from_str(&lo_s)
      .map_err(|e| rusqlite::Error::InvalidParameterName(format!("bad lo '{}': {}", lo_s, e)))?;
    let hi = resolve_local_nid(hi_raw, &local_to_global);
    let lo = resolve_local_nid(lo_raw, &local_to_global);
    let nid = anf.insert_vhl(v, hi, lo);
    local_to_global.insert(ix, nid);
  }

  let roots: Vec<(String, NID)> = roots_raw.into_iter().map(|(name, nid)| {
    (name, resolve_local_nid(nid, &local_to_global))
  }).collect();
  Ok(roots)
}

/// Resolve a snapshot-local NID: if it's an `@N` (ixn), look up N in the map.
/// Constants and vid-literals pass through unchanged.
fn resolve_local_nid(n: NID, l2g: &HashMap<usize, NID>) -> NID {
  if n.is_const() || n.is_vid() { return n; }
  if n.is_ixn() {
    let ix = n.raw().idx();
    let global = l2g.get(&ix).unwrap_or_else(|| panic!("unmapped local ix {} in snapshot", ix));
    if n.is_inv() { !*global } else { *global }
  } else {
    // vid.idx form — shouldn't appear in snapshot data, but pass through.
    n
  }
}

// -----------------------------------------------------------------------
// Path-based convenience wrappers
// -----------------------------------------------------------------------

/// Write a scaffold snapshot to a (possibly existing) SQLite file.
pub fn write_scaffold_to_path<P: AsRef<Path>>(
  path: P, m: SnapshotMetaInput, sc: &VhlScaffold, roots: &[(String, NID)],
) -> Result<SnapshotId> {
  let mut conn = Connection::open(path)?;
  // Ensure the AST meta table exists so we can write snap.fmt.version
  crate::sql::ensure_schema_pub(&conn)?;
  let tx = conn.transaction()?;
  let sid = write_scaffold(& tx, m, sc, roots)?;
  tx.commit()?;
  Ok(sid)
}

/// Read a scaffold snapshot from a SQLite file.
pub fn read_scaffold_from_path<P: AsRef<Path>>(
  path: P, id: SnapshotId,
) -> Result<(VhlScaffold, Vec<(String, NID)>)> {
  let conn = Connection::open(path)?;
  read_scaffold(&conn, id)
}

/// Write a BDD snapshot to a (possibly existing) SQLite file.
pub fn write_bdd_to_path<P: AsRef<Path>>(
  path: P, m: SnapshotMetaInput, bdd: &BddBase, roots: &[(String, NID)],
) -> Result<SnapshotId> {
  let mut conn = Connection::open(path)?;
  crate::sql::ensure_schema_pub(&conn)?;
  let tx = conn.transaction()?;
  let sid = write_bdd(&tx, m, bdd, roots)?;
  tx.commit()?;
  Ok(sid)
}

/// Write an ANF snapshot to a (possibly existing) SQLite file.
pub fn write_anf_to_path<P: AsRef<Path>>(
  path: P, m: SnapshotMetaInput, anf: &ANFBase, roots: &[(String, NID)],
) -> Result<SnapshotId> {
  let mut conn = Connection::open(path)?;
  crate::sql::ensure_schema_pub(&conn)?;
  let tx = conn.transaction()?;
  let sid = write_anf(&tx, m, anf, roots)?;
  tx.commit()?;
  Ok(sid)
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;
  use crate::vid::VID;
  use crate::nid;
  #[allow(unused_imports)]
  use crate::vhl::Vhl;
  use crate::base::Base;

  fn in_memory_conn() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    { let tx = conn.transaction().unwrap();
      crate::sql::ensure_schema_tx(&tx).unwrap();
      ensure_snap_schema(&tx).unwrap();
      tx.commit().unwrap(); }
    conn
  }

  #[test]
  fn schema_idempotent() {
    let mut conn = in_memory_conn();
    // Running ensure_snap_schema again should be fine
    let tx = conn.transaction().unwrap();
    ensure_snap_schema(&tx).unwrap();
    tx.commit().unwrap();
    let snaps = list_snapshots(&conn).unwrap();
    assert!(snaps.is_empty());
  }

  #[test]
  fn scaffold_roundtrip() {
    let mut conn = in_memory_conn();

    // Build a small scaffold: two vars, one internal node.
    let mut sc = VhlScaffold::new();
    sc.push(VID::var(0));
    sc.push(VID::var(1));
    // x1 branches to x0 (hi) and O (lo)
    let x0_nid = sc.add(VID::var(0), nid::I, nid::O, false);
    let root = sc.add(VID::var(1), x0_nid, nid::O, true);

    let tx = conn.transaction().unwrap();
    let sid = write_scaffold(&tx, SnapshotMetaInput {
      parent: None, step: Some(0), rv: None, note: Some("test".into()),
    }, &sc, &[("root".into(), root)]).unwrap();
    tx.commit().unwrap();

    // List should show one snapshot
    let snaps = list_snapshots(&conn).unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].kind, SnapshotKind::Scaffold);
    assert_eq!(snaps[0].step, Some(0));
    assert_eq!(snaps[0].vids.len(), 2);

    // Read it back
    let (sc2, roots2) = read_scaffold(&conn, sid).unwrap();
    assert_eq!(sc2.vids(), sc.vids());
    assert_eq!(sc2.num_nodes(), sc.num_nodes());
    assert_eq!(roots2.len(), 1);
    assert_eq!(roots2[0].0, "root");
    assert_eq!(roots2[0].1, root);

    // Verify structure matches
    let orig_nodes = sc.iter_nodes();
    let loaded_nodes = sc2.iter_nodes();
    assert_eq!(orig_nodes.len(), loaded_nodes.len());
    for (a, b) in orig_nodes.iter().zip(loaded_nodes.iter()) {
      assert_eq!(a.0, b.0, "ix mismatch");
      assert_eq!(a.1, b.1, "vhl mismatch");
      assert_eq!(a.2, b.2, "irc mismatch");
      assert_eq!(a.3, b.3, "erc mismatch");
    }
  }

  #[test]
  fn bdd_roundtrip() {
    let mut conn = in_memory_conn();
    let mut bdd = BddBase::new();
    let x0 = NID::from_vid(VID::var(0));
    let x1 = NID::from_vid(VID::var(1));
    let root = bdd.and(x0, x1); // x0 AND x1

    let tx = conn.transaction().unwrap();
    let sid = write_bdd(&tx, SnapshotMetaInput {
      parent: None, step: None, rv: None, note: None,
    }, &bdd, &[("root".into(), root)]).unwrap();
    tx.commit().unwrap();

    // Load into a fresh BDD
    let mut bdd2 = BddBase::new();
    let roots2 = read_bdd_into(&conn, sid, &mut bdd2).unwrap();
    assert_eq!(roots2.len(), 1);
    let root2 = roots2[0].1;

    // Verify extensional equivalence: enumerate all 4 assignments
    for bits in 0u32..4 {
      let v0 = (bits & 1) != 0;
      let v1 = (bits >> 1) != 0;
      let r1 = eval_bdd(&mut bdd, root, &[(VID::var(0), v0), (VID::var(1), v1)]);
      let r2 = eval_bdd(&mut bdd2, root2, &[(VID::var(0), v0), (VID::var(1), v1)]);
      assert_eq!(r1, r2, "mismatch at bits={:02b}", bits);
    }
  }

  #[test]
  fn anf_roundtrip() {
    let mut conn = in_memory_conn();
    let mut anf = ANFBase::new();
    let x0 = NID::from_vid(VID::var(0));
    let x1 = NID::from_vid(VID::var(1));
    let root = anf.xor(x0, x1); // x0 XOR x1

    let tx = conn.transaction().unwrap();
    let sid = write_anf(&tx, SnapshotMetaInput {
      parent: None, step: None, rv: None, note: None,
    }, &anf, &[("root".into(), root)]).unwrap();
    tx.commit().unwrap();

    let mut anf2 = ANFBase::new();
    let roots2 = read_anf_into(&conn, sid, &mut anf2).unwrap();
    assert_eq!(roots2.len(), 1);
    let root2 = roots2[0].1;

    // Verify extensional equivalence
    for bits in 0u32..4 {
      let v0 = (bits & 1) != 0;
      let v1 = (bits >> 1) != 0;
      let r1 = eval_anf(&mut anf, root, &[(VID::var(0), v0), (VID::var(1), v1)]);
      let r2 = eval_anf(&mut anf2, root2, &[(VID::var(0), v0), (VID::var(1), v1)]);
      assert_eq!(r1, r2, "mismatch at bits={:02b}", bits);
    }
  }

  #[test]
  fn snapshot_chain() {
    let mut conn = in_memory_conn();
    let mut sc = VhlScaffold::new();
    sc.push(VID::var(0));
    let root = sc.add(VID::var(0), nid::I, nid::O, true);

    let tx = conn.transaction().unwrap();
    let s1 = write_scaffold(&tx, SnapshotMetaInput {
      parent: None, step: Some(0), rv: None, note: None,
    }, &sc, &[("root".into(), root)]).unwrap();
    let _s2 = write_scaffold(&tx, SnapshotMetaInput {
      parent: Some(s1), step: Some(1), rv: Some(VID::var(0)), note: None,
    }, &sc, &[("root".into(), root)]).unwrap();
    tx.commit().unwrap();

    let snaps = list_snapshots(&conn).unwrap();
    assert_eq!(snaps.len(), 2);
    assert_eq!(snaps[0].parent, None);
    assert_eq!(snaps[1].parent, Some(s1));
  }

  #[test]
  fn backward_compat_ast_only() {
    // A file with only AST tables: list_snapshots should return empty.
    let conn = Connection::open_in_memory().unwrap();
    // We don't even create snap tables — just check it doesn't crash.
    let snaps = list_snapshots(&conn).unwrap();
    assert!(snaps.is_empty());
  }

  // -- test helpers --

  fn eval_bdd(bdd: &mut BddBase, root: NID, env: &[(VID, bool)]) -> bool {
    let mut cur = root;
    for &(v, val) in env {
      cur = if val { bdd.when_hi(v, cur) } else { bdd.when_lo(v, cur) };
    }
    assert!(cur.is_const(), "bdd didn't collapse: {:?}", cur);
    cur == nid::I
  }

  fn eval_anf(anf: &mut ANFBase, root: NID, env: &[(VID, bool)]) -> bool {
    let mut cur = root;
    for &(v, val) in env {
      cur = if val { anf.when_hi(v, cur) } else { anf.when_lo(v, cur) };
    }
    assert!(cur.is_const(), "anf didn't collapse: {:?}", cur);
    cur == nid::I
  }
}
