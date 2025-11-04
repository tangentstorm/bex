//! Persistence helpers for storing graphs in SQLite.
use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;

use rusqlite::{params, Connection, OptionalExtension, Result, Transaction};

use crate::{ast::RawASTBase, nid::NID, ops};

const CREATE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS ast_node(
  id  INTEGER PRIMARY KEY,
  op  TEXT
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
  nid     INTEGER NOT NULL,
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

const EXPECTED_FMT_VERSION: &str = "0.1";

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

fn sql_to_nid(value: i64) -> NID {
  let bytes = value.to_le_bytes();
  let raw = u64::from_le_bytes(bytes);
  NID::_from_u64(raw)
}

fn insert_nodes(tx: &Transaction<'_>, base: &RawASTBase) -> Result<()> {
  let mut stmt = tx.prepare(
    "INSERT INTO ast_node(id, op) VALUES(?1, ?2)"
  )?;
  for (idx, ops) in base.bits.iter().enumerate() {
    let rpn: Vec<NID> = ops.to_rpn().cloned().collect();
    let op = rpn
      .last()
      .map(|f| format!("{}", f))
      .unwrap_or_else(|| String::from("")); // fallback for unexpected empty ops
    stmt.execute(params![
      idx as i64,
      op
    ])?;
  }
  Ok(())
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
    "INSERT OR REPLACE INTO tag(name, nid, aid) VALUES(?1, ?2, ?3)"
  )?;
  for (name, nid) in base.tags.iter() {
    let node_id = if nid.is_ixn() { Some(nid.raw().idx() as i64) } else { None };
    stmt.execute(params![
      name,
      nid_to_sql(*nid),
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
    params!["fmt.version", EXPECTED_FMT_VERSION],
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

pub fn import_raw_ast_from_conn(conn: &Connection) -> Result<(RawASTBase, Vec<NID>)> {
  let mut base = RawASTBase::empty();

  {
    let version: Option<String> =
      conn.query_row(
        "SELECT v FROM meta WHERE k = 'fmt.version'",
        [],
        |row| row.get(0),
      ).optional()?;
    match version {
      Some(v) if v == EXPECTED_FMT_VERSION => {}
      Some(v) => {
        return Err(rusqlite::Error::FromSqlConversionFailure(
          0,
          rusqlite::types::Type::Text,
          Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unexpected fmt.version {v}, expected {EXPECTED_FMT_VERSION}"),
          )),
        ));
      }
      None => {
        return Err(rusqlite::Error::FromSqlConversionFailure(
          0,
          rusqlite::types::Type::Null,
          Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "missing fmt.version metadata",
          )),
        ));
      }
    }
  }

  {
    let mut node_stmt = conn.prepare("SELECT id, op FROM ast_node ORDER BY id")?;
    let mut edge_stmt = conn.prepare("SELECT arg FROM ast_edge WHERE aid = ?1 ORDER BY ord")?;
    let mut node_rows = node_stmt.query([])?;
    while let Some(row) = node_rows.next()? {
      let node_id: i64 = row.get(0)?;
      let op_text: String = row.get(1)?;
      let op_text_trimmed = op_text.trim();
      if op_text_trimmed.is_empty() {
        return Err(rusqlite::Error::InvalidQuery);
      }
      let fun = NID::from_str(op_text_trimmed)
        .map_err(|err| rusqlite::Error::InvalidParameterName(format!("invalid op '{}': {}", op_text_trimmed, err)))?;
      let mut args = Vec::new();
      let mut edge_rows = edge_stmt.query([node_id])?;
      while let Some(edge_row) = edge_rows.next()? {
        let arg_val: i64 = edge_row.get(0)?;
        args.push(sql_to_nid(arg_val));
      }
      let mut rpn = args;
      rpn.push(fun);
      let ops = ops::rpn(&rpn);
      base.push_raw_ops(ops);
    }
  }

  {
    let mut tag_stmt = conn.prepare("SELECT name, nid, aid FROM tag ORDER BY name")?;
    let mut tag_rows = tag_stmt.query([])?;
    while let Some(row) = tag_rows.next()? {
      let name: String = row.get(0)?;
      let nid_val: i64 = row.get(1)?;
      let aid: Option<i64> = row.get(2)?;
      let nid = sql_to_nid(nid_val);
      base.tags.insert(name.clone(), nid);
      if let Some(aid) = aid {
        debug_assert_eq!(nid.raw(), NID::ixn(aid as usize));
      }
    }
  }

  let mut keep = Vec::new();
  {
    let mut keep_stmt = conn.prepare("SELECT id FROM keep ORDER BY rowid")?;
    let mut keep_rows = keep_stmt.query([])?;
    while let Some(row) = keep_rows.next()? {
      let id: i64 = row.get(0)?;
      keep.push(sql_to_nid(id));
    }
  }

  Ok((base, keep))
}

pub fn import_raw_ast_from_path<P: AsRef<Path>>(path: P) -> Result<(RawASTBase, Vec<NID>)> {
  let conn = Connection::open(path)?;
  import_raw_ast_from_conn(&conn)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{base::Base, ops, vid::VID};

  #[test]
  fn test_save() -> Result<()> {
    let mut base = RawASTBase::empty();
    let x = base.def("x".into(), VID::var(0));
    let y = base.def("y".into(), VID::var(1));
    let and = base.and(x, y);
    base.tag(and, "and".into());
    let inv_and = !and;
    let inv_x = !x;
    base.tag(x, "var_x".into());
    base.tag(inv_x, "not_var_x".into());

    let mut conn = Connection::open_in_memory()?;
    export_raw_ast_to_conn(&mut conn, &base, &[and, inv_and, x, inv_x])?;

    let node_count: i64 = conn.query_row("SELECT COUNT(*) FROM ast_node", [], |row| row.get(0))?;
    assert_eq!(node_count, base.len() as i64);

    let op: String =
      conn.query_row("SELECT op FROM ast_node WHERE id = 0", [], |row| row.get(0))?;
    assert_eq!(op, format!("{}", ops::AND.to_nid()));

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

    let mut tag_stmt = conn.prepare("SELECT name, nid, aid FROM tag ORDER BY name")?;
    let mut tag_rows = tag_stmt.query([])?;
    let mut observed_tags = Vec::new();
    while let Some(row) = tag_rows.next()? {
      let name: String = row.get(0)?;
      let nid_val: i64 = row.get(1)?;
      let aid_val: Option<i64> = row.get(2)?;
      observed_tags.push((name, sql_to_nid(nid_val), aid_val));
    }
    assert_eq!(observed_tags.len(), base.tags.len());
    for (name, nid) in base.tags.iter() {
      let Some((_, observed_nid, observed_aid)) = observed_tags.iter().find(|(n, _, _)| n == name) else {
        panic!("missing tag {}", name);
      };
      assert_eq!(observed_nid, nid);
      let expected_aid = if nid.is_ixn() { Some(nid.raw().idx() as i64) } else { None };
      assert_eq!(*observed_aid, expected_aid);
    }

    let mut keep_stmt = conn.prepare("SELECT id, aid FROM keep ORDER BY rowid")?;
    let mut keep_rows = keep_stmt.query([])?;
    let mut observed_keep = Vec::new();
    while let Some(row) = keep_rows.next()? {
      let id: i64 = row.get(0)?;
      let aid: Option<i64> = row.get(1)?;
      observed_keep.push((sql_to_nid(id), aid));
    }
    observed_keep.sort_by_key(|(nid, _)| nid._to_u64());
    let mut expected_keep = vec![
      (and, Some(0)),
      (inv_and, Some(0)),
      (x, None),
      (inv_x, None)
    ];
    expected_keep.sort_by_key(|(nid, _)| nid._to_u64());
    assert_eq!(observed_keep, expected_keep);

    let fmt_version: String =
      conn.query_row("SELECT v FROM meta WHERE k = 'fmt.version'", [], |row| row.get(0))?;
    assert_eq!(fmt_version, EXPECTED_FMT_VERSION);

    Ok(())
  }

  #[test]
  fn test_roundtrip() -> Result<()> {
    let mut base = RawASTBase::empty();
    let x = base.def("x".into(), VID::var(0));
    let y = base.def("y".into(), VID::var(1));
    let and = base.and(x, y);
    base.tag(and, "and".into());
    let inv_and = !and;
    let inv_x = !x;
    base.tag(x, "var_x".into());
    base.tag(inv_x, "not_var_x".into());

    let mut conn = Connection::open_in_memory()?;
    export_raw_ast_to_conn(&mut conn, &base, &[and, inv_and, x, inv_x])?;

    let (loaded, keep) = import_raw_ast_from_conn(&conn)?;
    assert_eq!(loaded.bits, base.bits);
    assert_eq!(loaded.tags, base.tags);
    let mut keep_sorted = keep.clone();
    keep_sorted.sort();
    let mut expected_keep = vec![and, inv_and, x, inv_x];
    expected_keep.sort();
    assert_eq!(keep_sorted, expected_keep);

    Ok(())
  }
}
