//! Persistence helpers for storing graphs in SQLite.
use std::collections::HashSet;
use std::path::Path;

use rusqlite::{params, Connection, Result, Transaction};

use crate::{ast::RawASTBase, nid::NID};

const CREATE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS ast_node(
  id     INTEGER PRIMARY KEY,
  op     TEXT,
  level  INTEGER,
  color  INTEGER
);

CREATE TABLE IF NOT EXISTS ast_edge(
  aid     INTEGER NOT NULL,
  ord     INTEGER NOT NULL,
  arg     INTEGER NOT NULL,
  PRIMARY KEY(aid, ord),
  FOREIGN KEY(aid) REFERENCES ast_node(id)
);

CREATE TABLE IF NOT EXISTS tag(
  name    TEXT PRIMARY KEY,
  aid     INTEGER,
  FOREIGN KEY(aid) REFERENCES ast_node(id)
);

CREATE TABLE IF NOT EXISTS keep(
  id    INTEGER PRIMARY KEY,
  aid   INTEGER,
  FOREIGN KEY(aid) REFERENCES ast_node(id)
);

CREATE VIEW IF NOT EXISTS edge_src_bits AS
SELECT
  e.aid,
  e.ord,
  e.arg,
  CASE WHEN (e.arg & 0x8000000000000000) != 0 THEN 1 ELSE 0 END AS src_inv,
  CASE WHEN (e.arg & 0x4000000000000000) != 0 THEN 1 ELSE 0 END AS src_var,
  CASE
    WHEN (e.arg & 0x0800000000000000) != 0 THEN 'tbl'
    WHEN (e.arg & 0x2000000000000000) != 0 THEN 'const'
    WHEN (e.arg & 0x4000000000000000) != 0 THEN 'lit'
    ELSE 'ixn'
  END AS src_type
FROM ast_edge e;

CREATE TABLE IF NOT EXISTS meta(
  k  TEXT PRIMARY KEY,
  v  TEXT NOT NULL
);
"#;

fn ensure_schema(tx: &Transaction<'_>) -> Result<()> {
  tx.execute_batch(CREATE_SCHEMA)
}


fn clear_schema(tx: &Transaction<'_>) -> Result<()> {
  tx.execute("DELETE FROM ast_edge", [])?;
  tx.execute("DELETE FROM ast_node", [])?;
  tx.execute("DELETE FROM tag", [])?;
  tx.execute("DELETE FROM keep", [])?;
  tx.execute("DELETE FROM meta", [])?;
  Ok(())
}

fn nid_to_sql(nid: NID) -> i64 {
  i64::from_le_bytes(nid._to_u64().to_le_bytes())
}

fn node_levels(base: &RawASTBase) -> Vec<i64> {
  let mut levels = vec![0i64; base.len()];
  for (idx, ops) in base.bits.iter().enumerate() {
    let mut max_child = 0i64;
    let rpn: Vec<NID> = ops.to_rpn().cloned().collect();
    if let Some((_fun, args)) = rpn.split_last() {
      for arg in args.iter() {
        if arg.is_ixn() {
          let level = levels[arg.idx()];
          if level > max_child { max_child = level; }
        }
      }
    }
    levels[idx] = max_child + 1;
  }
  levels
}

fn insert_nodes(tx: &Transaction<'_>, base: &RawASTBase) -> Result<Vec<i64>> {
  let levels = node_levels(base);
  let mut stmt = tx.prepare(
    "INSERT INTO ast_node(id, op, level, color) VALUES(?1, ?2, ?3, ?4)"
  )?;
  for (idx, ops) in base.bits.iter().enumerate() {
    let rpn: Vec<NID> = ops.to_rpn().cloned().collect();
    let op = rpn
      .last()
      .map(|f| format!("{}", f))
      .unwrap_or_else(|| String::from("")); // fallback for unexpected empty ops
    let level = levels[idx];
    stmt.execute(params![
      idx as i64,
      op,
      level,
      Option::<i64>::None
    ])?;
  }
  Ok(levels)
}

fn insert_edges(tx: &Transaction<'_>, base: &RawASTBase) -> Result<()> {
  let mut stmt = tx.prepare(
    "INSERT OR REPLACE INTO ast_edge(aid, ord, arg) VALUES(?1, ?2, ?3)"
  )?;
  for (idx, ops) in base.bits.iter().enumerate() {
    let rpn: Vec<NID> = ops.to_rpn().cloned().collect();
    if let Some((_fun, args)) = rpn.split_last() {
      for (ord, arg) in args.iter().enumerate() {
        stmt.execute(params![
          idx as i64,
          ord as i64,
          nid_to_sql(*arg)
        ])?;
      }
    }
  }
  Ok(())
}

fn insert_tags(tx: &Transaction<'_>, base: &RawASTBase) -> Result<()> {
  let mut stmt = tx.prepare(
    "INSERT OR REPLACE INTO tag(name, aid) VALUES(?1, ?2)"
  )?;
  for (name, nid) in base.tags.iter() {
    let node_id = if nid.is_ixn() { Some(nid.raw().idx() as i64) } else { None };
    stmt.execute(params![
      name,
      node_id
    ])?;
  }
  Ok(())
}

fn insert_keep(tx: &Transaction<'_>, keep: &[NID]) -> Result<()> {
  let mut stmt = tx.prepare(
    "INSERT OR REPLACE INTO keep(id, aid) VALUES(?1, ?2)"
  )?;
  let mut seen: HashSet<NID> = HashSet::new();
  for &nid in keep {
    if seen.insert(nid) {
      let node_id = if nid.is_ixn() { Some(nid.raw().idx() as i64) } else { None };
      stmt.execute(params![
        nid_to_sql(nid),
        node_id
      ])?;
    }
  }
  Ok(())
}

fn insert_meta(tx: &Transaction<'_>) -> Result<()> {
  tx.execute(
    "INSERT OR REPLACE INTO meta(k, v) VALUES(?1, ?2)",
    params!["fmt.version", "0.1"],
  )?;
  Ok(())
}

/// Write the contents of a [`RawASTBase`] into the provided SQLite connection.
pub fn export_raw_ast_to_conn(conn: &mut Connection, base: &RawASTBase, keep: &[NID]) -> Result<()> {
  let tx = conn.transaction()?;
  ensure_schema(&tx)?;
  clear_schema(&tx)?;
  insert_nodes(&tx, base)?;
  insert_edges(&tx, base)?;
  insert_tags(&tx, base)?;
  insert_keep(&tx, keep)?;
  insert_meta(&tx)?;
  tx.commit()
}

/// Convenience helper that opens or creates a SQLite file on disk and exports
/// a [`RawASTBase`] into it.
pub fn export_raw_ast_to_path<P: AsRef<Path>>(base: &RawASTBase, path: P, keep: &[NID]) -> Result<()> {
  let mut conn = Connection::open(path)?;
  export_raw_ast_to_conn(&mut conn, base, keep)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{base::Base, ops, vid::VID};

  fn sql_to_nid(value: i64) -> NID {
    let bytes = value.to_le_bytes();
    let raw = u64::from_le_bytes(bytes);
    NID::_from_u64(raw)
  }

  #[test]
  fn test_roundtrip() -> Result<()> {
    let mut base = RawASTBase::empty();
    let x = base.def("x".into(), VID::var(0));
    let y = base.def("y".into(), VID::var(1));
    let and = base.and(x, y);
    base.tag(and, "and".into());

    let mut conn = Connection::open_in_memory()?;
    export_raw_ast_to_conn(&mut conn, &base, &[and])?;

    let node_count: i64 = conn.query_row("SELECT COUNT(*) FROM ast_node", [], |row| row.get(0))?;
    assert_eq!(node_count, base.len() as i64);

    let (op, level): (String, i64) =
      conn.query_row("SELECT op, level FROM ast_node WHERE id = 0", [], |row| Ok((row.get(0)?, row.get(1)?)))?;
    assert_eq!(op, format!("{}", ops::AND.to_nid()));
    assert_eq!(level, 1);

    let mut edge_stmt = conn.prepare("SELECT aid, ord, arg FROM ast_edge ORDER BY ord")?;
    let mut edge_rows = edge_stmt.query([])?;
    let mut src_values = Vec::new();
    while let Some(row) = edge_rows.next()? {
      let aid: i64 = row.get(0)?;
      let ord: i64 = row.get(1)?;
      assert_eq!(aid, 0);
      assert!(ord == 0 || ord == 1);
      let arg: i64 = row.get(2)?;
      src_values.push(sql_to_nid(arg));
    }
    src_values.sort();
    let mut expected = vec![x, y];
    expected.sort();
    assert_eq!(src_values, expected);

    let mut bits_stmt = conn.prepare(
      "SELECT ord, src_inv, src_var, src_type FROM edge_src_bits WHERE aid = 0 ORDER BY ord"
    )?;
    let mut bits_rows = bits_stmt.query([])?;
    let mut view_entries = Vec::new();
    while let Some(row) = bits_rows.next()? {
      let ord: i64 = row.get(0)?;
      let inv_flag: i64 = row.get(1)?;
      let var_flag: i64 = row.get(2)?;
      let kind: String = row.get(3)?;
      view_entries.push((ord, inv_flag != 0, var_flag != 0, kind));
    }
    assert_eq!(
      view_entries,
      vec![
        (0, false, true, String::from("lit")),
        (1, false, true, String::from("lit"))
      ]
    );

    let tag_aid: Option<i64> =
      conn.query_row("SELECT aid FROM tag WHERE name = 'and'", [], |row| row.get(0))?;
    assert_eq!(tag_aid, Some(0));

    let (keep_nid, keep_aid): (i64, Option<i64>) =
      conn.query_row("SELECT id, aid FROM keep", [], |row| Ok((row.get(0)?, row.get(1)?)))?;
    assert_eq!(keep_nid, nid_to_sql(and));
    assert_eq!(keep_aid, Some(0));

    let fmt_version: String =
      conn.query_row("SELECT v FROM meta WHERE k = 'fmt.version'", [], |row| row.get(0))?;
    assert_eq!(fmt_version, "0.1");

    Ok(())
  }
}
