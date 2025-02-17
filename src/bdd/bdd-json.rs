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
      .map(|&n| { let node = n.raw(); mapping[&node] as i32 }).collect();

    let out = json::object!{
      "format": "bex-bdd-0.01",
      "vhls": vhls,
      "keep": keep };
    out.dump()
  }

  pub fn from_json(s: &str) -> (Self, Vec<NID>) {
    let data = json::parse(s).unwrap();
    assert_eq!(data["format"].as_str().unwrap(), "bex-bdd-0.01");
    let vhls_arr = data["vhls"].members().collect::<Vec<_>>();
    let mut mapping: HashMap<usize, NID> = HashMap::new();
    let mut base = BddBase::new();

    let parse_child = |child: &json::JsonValue, mapping: &HashMap<usize, NID>| -> NID {
      let s = child.as_str().unwrap();
      if let Ok(nid) = s.parse::<NID>() { nid }
      else if child.is_number() {
        let idx_i32 = child.as_i32().unwrap();
        let idx: usize = idx_i32.abs() as usize;
        mapping[&idx].inv_if(idx_i32 < 0)}
      else { panic!("unexpected value type: {}", child) }};

    for i in 1..vhls_arr.len() {
      let node = &vhls_arr[i];
      let nv = node[0].as_str().unwrap().parse::<NID>().unwrap();
      let hi = parse_child(&node[1], &mapping);
      let lo = parse_child(&node[2], &mapping);
      let nid = base.ite(nv, hi, lo);
      mapping.insert(i, nid);}

    let nids: Vec<NID> = data["keep"].members()
      .map(|idx| { let i = idx.as_i32().unwrap() as usize; mapping[&i]})
      .collect();

    (base, nids)}}


#[test] fn test_json() {
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
  assert_eq!(base.tt(n, 3), base2.tt(n2, 3));}
