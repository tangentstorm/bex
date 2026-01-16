extern crate json;

impl BddBase {
  pub fn to_json(&self, nids: &[NID]) -> String {
    let mut vhls = json::array![json::object!{}];
    let mut mapping: HashMap<NID, usize> = HashMap::new();

    for &n in nids {
      if !n.is_const() {
        let node = if n.is_inv() { !n } else { n };
        self.walk_up(node, &mut |n, v, hi, lo| {
          if mapping.contains_key(&n) || n.is_lit() { return }
          let idx = vhls.len();
          mapping.insert(n, idx);

          let process_child = |child: NID| -> json::JsonValue {
            if child.is_lit() { json::JsonValue::String(child.to_string()) }
            else {
              let underlying = child.raw();
              let child_idx = mapping[&underlying];
              if child.is_inv() { json::JsonValue::Number((-1 * child_idx as i32).into()) }
              else { json::JsonValue::Number((child_idx as i32).into()) }}};

          vhls.push(json::array![v.to_string(), process_child(hi), process_child(lo)])
              .expect("failed to push to vhls"); });}}

    let keep: Vec<i32> = nids.iter().filter(|&&n| !n.is_const())
      .map(|&n| {
        let idx = mapping[&n.raw()] as i32;
        if n.is_inv() { -idx } else { idx }
      }).collect();

    let out = json::object!{
      "format": "bex-bdd-0.01",
      "vhls": vhls,
      "keep": keep };
    out.dump()
  }

  /// Load nodes from JSON into this BddBase, returning the root NIDs
  pub fn load_json(&mut self, s: &str) -> Vec<NID> {
    let data = json::parse(s).unwrap();
    assert_eq!(data["format"].as_str().unwrap(), "bex-bdd-0.01");
    let vhls_arr = data["vhls"].members().collect::<Vec<_>>();
    let mut mapping: HashMap<usize, NID> = HashMap::new();

    let parse_child = |child: &json::JsonValue, mapping: &HashMap<usize, NID>| -> NID {
      if let Some(s) = child.as_str() {
        s.parse::<NID>().expect("failed to parse NID from string")
      } else if child.is_number() {
        let idx_i32 = child.as_i32().unwrap();
        let idx: usize = idx_i32.abs() as usize;
        mapping[&idx].inv_if(idx_i32 < 0)
      } else { panic!("unexpected value type: {}", child) }};

    for i in 1..vhls_arr.len() {
      let node = &vhls_arr[i];
      let nv = node[0].as_str().unwrap().parse::<NID>().unwrap();
      let hi = parse_child(&node[1], &mapping);
      let lo = parse_child(&node[2], &mapping);
      let nid = self.ite(nv, hi, lo);
      mapping.insert(i, nid);
    }

    data["keep"].members()
      .map(|idx| {
        let i = idx.as_i32().unwrap();
        let nid = mapping[&(i.abs() as usize)];
        if i < 0 { !nid } else { nid }
      }).collect()
  }

  /// Create a new BddBase from JSON
  pub fn from_json(s: &str) -> (Self, Vec<NID>) {
    let mut base = BddBase::new();
    let nids = base.load_json(s);
    (base, nids)
  }}


#[test] fn test_json_from_json() {
  use crate::nid::named::{x0, x1};
  let mut base = BddBase::new();
  let n = base.xor(x0, x1);
  let s = base.to_json(&[n]);
  println!("json: {}", s);
  assert!(s.contains(r#""format":"bex-bdd-0.01""#));
  assert!(s.contains(r#""vhls":[{},"#));
  assert!(s.contains(r#""keep":[1]"#));
  let (mut base2, nids) = BddBase::from_json(&s);
  assert_eq!(nids.len(), 1);
  let n2 = nids[0];
  assert_eq!(base.len(), base2.len());
  assert_eq!(base.tt(n, 3), base2.tt(n2, 3));
}

#[test] fn test_json_load_json() {
  use crate::nid::named::{x0, x1};
  let mut base = BddBase::new();
  let n = base.xor(x0, x1);
  let s = base.to_json(&[n]);
  // load into a new base using load_json
  let mut base2 = BddBase::new();
  let nids = base2.load_json(&s);
  assert_eq!(nids.len(), 1);
  assert_eq!(base.tt(n, 3), base2.tt(nids[0], 3));
}

#[test] fn test_json_inverted_root() {
  use crate::nid::named::{x0, x1};
  let mut base = BddBase::new();
  let n = base.xor(x0, x1);
  let inv_n = !n;  // inverted root
  let s = base.to_json(&[inv_n]);
  println!("json with inverted root: {}", s);
  // keep should have a negative index for inverted root
  assert!(s.contains(r#""keep":[-1]"#), "inverted root should have negative keep index");
  let (mut base2, nids) = BddBase::from_json(&s);
  assert_eq!(nids.len(), 1);
  // the loaded node should match the inverted original
  assert_eq!(base.tt(inv_n, 3), base2.tt(nids[0], 3));
  // and should NOT match the non-inverted original
  assert_ne!(base.tt(n, 3), base2.tt(nids[0], 3));
}
