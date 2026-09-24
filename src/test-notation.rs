// Tests for src/notation.rs (issue #10: standard bex notations).
#[cfg(test)]
mod notation_tests {
  use super::*;
  use crate::bdd::BddBase;
  use crate::ast::ASTBase;
  use crate::nid::named::{x0, x1, x2};

  // ---- ParsedNid: every nid form parses, and round-trips through Display ----

  fn roundtrip(s:&str) {
    let pn:ParsedNid = s.parse().unwrap_or_else(|e| panic!("failed to parse {}: {}", s, e));
    let printed = pn.to_string();
    let pn2:ParsedNid = printed.parse().unwrap_or_else(|e| panic!("failed to re-parse {} (from {}): {}", printed, s, e));
    assert_eq!(pn, pn2, "round trip failed for {} (printed as {})", s, printed); }

  #[test] fn test_roundtrip_all_forms() {
    for s in ["O", "I",
              "x0", "xEB", "v0", "vF3", "!x0", "!v3",
              "t0001", "t0110", "t01101001", "f2.6", "f3.33", "fA",
              "x2.A3", "v2.A3", "!x2.A3", "@1A", "!@1A",
              "T{x3,x7:1110}", "T{x1,x3,x5:FC}",
              "!t0001", "!T{x3,x7:1110}",
              "2:x0", "5:x5.234", "3:@FF", "7:O", "3:!t0001"] {
      roundtrip(s); }}

  #[test] fn test_tbl_oi_alternates_via_parsed_nid() {
    let a:ParsedNid = "tOIIO".parse().unwrap();
    let b:ParsedNid = "t0110".parse().unwrap();
    assert_eq!(a, b); }

  #[test] fn test_namespace_prefix() {
    let pn:ParsedNid = "2:x0".parse().unwrap();
    assert_eq!(pn.ns, Some(2));
    assert_eq!(pn.nid, x0);
    assert_eq!(pn.to_string(), "2:x0");
    let plain:ParsedNid = "x0".parse().unwrap();
    assert_eq!(plain.ns, None); }

  // ---- table equivalences ----

  #[test] fn test_table_equivalences() {
    let f26:NID = "f2.6".parse::<ParsedNid>().unwrap().nid;
    let t0110:NID = "t0110".parse::<ParsedNid>().unwrap().nid;
    assert_eq!(f26, t0110, "f2.6 should equal t0110 (XOR)");

    let t0001:NID = "t0001".parse::<ParsedNid>().unwrap().nid;
    assert_eq!(t0001, crate::ops::AND.to_nid(), "t0001 should equal the dyadic AND function"); }

  // ---- invalid inputs rejected ----

  #[test] fn test_invalid_table_length() {
    assert!("t010".parse::<ParsedNid>().is_err(), "3-bit table should be rejected");
    let bits64 = format!("t{}", "0".repeat(64));
    assert!(bits64.parse::<ParsedNid>().is_err(), "64-bit table should be rejected"); }

  #[test] fn test_lowercase_hex_rejected() {
    assert!("xeb".parse::<ParsedNid>().is_err(), "lowercase hex variable should be rejected");
    assert!("f2.f".parse::<ParsedNid>().is_err(), "lowercase hex table digit should be rejected");
    assert!("@1a".parse::<ParsedNid>().is_err(), "lowercase hex ast index should be rejected"); }

  // ---- infix grammar: precedence ----

  #[test] fn test_precedence_and_over_or() {
    let e = parse_expr("x0 | x1 & x2").unwrap();
    match e {
      Expr::Or(a, b) => {
        assert_eq!(*a, Expr::Nid(x0));
        match *b {
          Expr::And(bx, by) => { assert_eq!(*bx, Expr::Nid(x1)); assert_eq!(*by, Expr::Nid(x2)); }
          other => panic!("expected And on rhs of Or, got {:?}", other) }}
      other => panic!("expected top-level Or, got {:?}", other) }}

  #[test] fn test_precedence_xor_between_and_and_or() {
    // x0 & x1 % x2 | x0  ==  (( x0 & x1 ) % x2 ) | x0
    let e = parse_expr("x0 & x1 % x2 | x0").unwrap();
    match e {
      Expr::Or(l, r) => {
        assert_eq!(*r, Expr::Nid(x0));
        match *l {
          Expr::Xor(xl, xr) => {
            assert_eq!(*xl, Expr::And(Box::new(Expr::Nid(x0)), Box::new(Expr::Nid(x1))));
            assert_eq!(*xr, Expr::Nid(x2)); }
          other => panic!("expected Xor, got {:?}", other) }}
      other => panic!("expected top-level Or, got {:?}", other) }}

  #[test] fn test_precedence_rel_below_or() {
    // x0 < x1 | x2  ==  x0 < (x1 | x2)
    let e = parse_expr("x0 < x1 | x2").unwrap();
    match e {
      Expr::Lt(a, b) => {
        assert_eq!(*a, Expr::Nid(x0));
        assert_eq!(*b, Expr::Or(Box::new(Expr::Nid(x1)), Box::new(Expr::Nid(x2)))); }
      other => panic!("expected top-level Lt, got {:?}", other) }}

  #[test] fn test_eq_non_associative() {
    assert!(parse_expr("x0 = x1").is_ok());
    assert!(parse_expr("x0 = x1 = x2").is_err(), "chained '=' without parens should be rejected");
    assert!(parse_expr("(x0 = x1) = x2").is_ok(), "parens should allow chaining"); }

  #[test] fn test_ternary_right_assoc_and_eval() {
    let e = parse_expr("x0 ? x1 : x2").unwrap();
    let mut base = BddBase::new();
    let mut scope = HashMap::new();
    let n = eval_expr(&mut base, &e, &mut scope).unwrap();
    assert_eq!(n, base.ite(x0, x1, x2)); }

  #[test] fn test_unary_not_binds_tighter_than_and() {
    let e = parse_expr("!x0 & x1").unwrap();
    assert_eq!(e, Expr::And(Box::new(Expr::Not(Box::new(Expr::Nid(x0)))), Box::new(Expr::Nid(x1)))); }

  #[test] fn test_parens() {
    let e = parse_expr("x0 & (x1 | x2)").unwrap();
    assert_eq!(e, Expr::And(Box::new(Expr::Nid(x0)),
                             Box::new(Expr::Or(Box::new(Expr::Nid(x1)), Box::new(Expr::Nid(x2)))))); }

  // ---- assignment + evaluation ----

  #[test] fn test_assignment_and_reference() {
    let mut base = BddBase::new();
    let mut scope = HashMap::new();
    let a = eval_expr(&mut base, &parse_expr("a : x0 & x1").unwrap(), &mut scope).unwrap();
    let n = eval_expr(&mut base, &parse_expr("a | x2").unwrap(), &mut scope).unwrap();
    assert_eq!(n, base.or(a, x2)); }

  #[test] fn test_undefined_variable_errors() {
    let mut base = BddBase::new();
    let mut scope = HashMap::new();
    assert!(eval_expr(&mut base, &parse_expr("undefined_name").unwrap(), &mut scope).is_err()); }

  // ---- bracket notation ----

  #[test] fn test_bracket_table_apply() {
    let mut base = BddBase::new();
    let and_tbl = crate::ops::AND.to_nid();
    let n = apply_bracket(&mut base, and_tbl, &[x0, x1]).unwrap();
    assert_eq!(n, base.and(x0, x1)); }

  #[test] fn test_bracket_table_arity_mismatch() {
    let mut base = BddBase::new();
    let and_tbl = crate::ops::AND.to_nid();
    assert!(apply_bracket(&mut base, and_tbl, &[x0]).is_err()); }

  #[test] fn test_bracket_via_grammar() {
    let mut base = BddBase::new();
    let mut scope = HashMap::new();
    let e = parse_expr("t0001[x0 x1]").unwrap();
    let n = eval_expr(&mut base, &e, &mut scope).unwrap();
    assert_eq!(n, base.and(x0, x1)); }

  #[test] fn test_bracket_vhl_sub() {
    let mut base = BddBase::new();
    // combine enough variables that the result is a real (indexed) BDD node,
    // rather than a small truth-table nid.
    let mut n = x0;
    for i in 1..7u32 { n = base.and(n, NID::var(i)); }
    assert!(!n.is_fun(), "test setup expected a real BDD node, not a table nid");
    let top_var = n.vid();
    let replacement = NID::var(20);
    let subbed = apply_bracket(&mut base, n, &[replacement]).unwrap();
    let expected = base.sub(top_var, replacement, n);
    assert_eq!(subbed, expected); }

  #[test] fn test_bracket_on_plain_var_is_error() {
    let mut base = BddBase::new();
    assert!(apply_bracket(&mut base, x0, &[x1]).is_err()); }

  #[test] fn test_bracket_ast_error() {
    let mut base = ASTBase::empty();
    let n = base.and(x0, x1);
    assert!(n.is_ixn(), "test setup expected a genuine AST node");
    let err = apply_bracket(&mut base, n, &[x2]).unwrap_err();
    assert!(err.contains("AST"), "error should mention AST nids, got: {}", err); }

  // ---- JSON notation ----

  #[test] fn test_json_roundtrip_primitives() {
    for s in ["O", "I", "x0", "!x0", "t0110", "@3"] {
      let n:NID = s.parse().unwrap();
      let e = Expr::Nid(n);
      let j = to_json(&e).unwrap();
      assert_eq!(j.as_str().unwrap(), n.to_string());
      let e2 = from_json(&j).unwrap();
      assert_eq!(e2, e); }}

  #[test] fn test_json_roundtrip_compound() {
    let e = parse_expr("x0 & x1 | !x2").unwrap();
    let j = to_json(&e).unwrap();
    let e2 = from_json(&j).unwrap();
    assert_eq!(e, e2); }

  #[test] fn test_json_not_elides_list() {
    let e = Expr::Not(Box::new(Expr::Nid(x0)));
    let j = to_json(&e).unwrap();
    assert_eq!(j["~"].as_str().unwrap(), "x0"); }

  #[test] fn test_json_ternary() {
    let e = parse_expr("x0 ? x1 : x2").unwrap();
    let j = to_json(&e).unwrap();
    assert_eq!(j["?:"].len(), 3);
    let e2 = from_json(&j).unwrap();
    assert_eq!(e, e2); }

  #[test] fn test_json_apply_key_roundtrip_and_eval() {
    let mut base = BddBase::new();
    let and_tbl = crate::ops::AND.to_nid();
    let e = Expr::Apply(Box::new(Expr::Nid(and_tbl)), vec![Expr::Nid(x0), Expr::Nid(x1)]);
    let j = to_json(&e).unwrap();
    let e2 = from_json(&j).unwrap();
    assert_eq!(e, e2);
    let mut scope = HashMap::new();
    let n = eval_expr(&mut base, &e2, &mut scope).unwrap();
    assert_eq!(n, base.and(x0, x1)); }

  #[test] fn test_json_assignment_has_no_representation() {
    let e = parse_expr("a : x0").unwrap();
    assert!(to_json(&e).is_err()); }

  // ---- PR #34 review regressions (Memnar #1538) ----

  /// Claim 1: inverted table NIDs must keep `!` through Display / ParsedNid round-trip.
  #[test] fn test_inverted_table_nid_roundtrip() {
    for s in ["!t0001", "3:!t0001", "!T{x3,x7:1110}", "5:!t0110", "!fA"] {
      let pn:ParsedNid = s.parse().unwrap_or_else(|e| panic!("parse {}: {}", s, e));
      assert!(pn.nid.is_inv(), "{} should parse as inverted", s);
      let printed = pn.to_string();
      assert!(printed.contains('!'),
        "{} printed as {} (lost inversion)", s, printed);
      let pn2:ParsedNid = printed.parse().unwrap_or_else(|e| panic!("reparse {} from {}: {}", printed, s, e));
      assert_eq!(pn, pn2, "round-trip for {}", s);
      // to_json uses NID::Display directly (no namespace)
      let j = to_json(&Expr::Nid(pn.nid)).unwrap();
      let js = j.as_str().unwrap();
      assert!(js.starts_with('!'), "to_json lost inversion for {} -> {}", s, js);
    }
  }

  /// Claim 2: `!t0001[x0 x1]` must evaluate as NAND, not AND.
  #[test] fn test_inverted_table_bracket_apply() {
    let mut base = BddBase::new();
    let mut scope = HashMap::new();
    let n = eval_expr(&mut base, &parse_expr("!t0001[x0 x1]").unwrap(), &mut scope).unwrap();
    let expected = !base.and(x0, x1);
    assert!(n.is_inv(), "!t0001[x0 x1] result should be inverted, got {}", n);
    assert_eq!(n, expected, "!t0001[x0 x1] should be NAND");
    // also via apply_bracket directly
    let inv_and:NID = "!t0001".parse().unwrap();
    assert!(inv_and.is_inv());
    let n2 = apply_bracket(&mut base, inv_and, &[x0, x1]).unwrap();
    assert!(n2.is_inv());
    assert_eq!(n2, expected);
    // non-inverted still AND
    let and_n = apply_bracket(&mut base, "t0001".parse().unwrap(), &[x0, x1]).unwrap();
    assert_eq!(and_n, base.and(x0, x1));
  }

  /// Claim 3: empty / bang-only NID text must return Err, not panic.
  #[test] fn test_empty_nid_text_is_err_not_panic() {
    for s in ["", "!", "3:", "3:!"] {
      let r = s.parse::<ParsedNid>();
      assert!(r.is_err(), "expected Err for {:?}, got {:?}", s, r);
    }
    // from_json must not panic on empty / bang-only strings either
    for s in ["", "!"] {
      let v = JsonValue::from(s);
      let r = from_json(&v);
      // empty/"!" are not valid nids; they become Var names (or Err if we reject).
      // Either Err or Var is fine — just must not panic.
      let _ = r;
    }
  }
}

