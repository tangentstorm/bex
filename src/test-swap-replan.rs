#[test] fn test_replan_works() {
  use std::collections::{HashMap, HashSet};
  use crate::vid::VID;
  use crate::swap::{XVHLScaffold, XID, plan_regroup, XID_I, XID_O};
  
  // Helper macro to create a HashSet with variables
  macro_rules! group_set {
    { } => { HashSet::new() };
    {$( $x:expr ),+ } => {{ let mut tmp = HashSet::new(); $( tmp.insert($x); )* tmp }};
  }
  
  // Create a more complex reordering scenario that requires replanning
  let x0:VID = VID::var(0);
  let x1:VID = VID::var(1);
  let x2:VID = VID::var(2);
  let x3:VID = VID::var(3);
  let x4:VID = VID::var(4);
  let x5:VID = VID::var(5);
  
  // Set up scaffold with initial ordering [0,1,2,3,4,5]
  let mut scaffold = XVHLScaffold::new();
  for &x in &[x0, x1, x2, x3, x4, x5] {
    scaffold.push(x);
    scaffold.add(x, XID_I, XID_O, true);
  }
  
  // Target groups: [{0,4,2}, {1,5,3}]
  // This should result in a target ordering: [0,4,2,1,5,3]
  let groups = vec![group_set![x0, x4, x2], group_set![x1, x5, x3]];
  
  // Save initial vids
  let initial_vids = scaffold.vids.clone();
  println!("Initial order: {:?}", initial_vids);
  
  // Perform regroup
  scaffold.regroup(groups.clone());
  
  // Check that the final ordering matches our expected groups
  println!("Final order: {:?}", scaffold.vids);
  
  // Check if all variables in the first group come before all variables in the second group
  let first_group = &groups[0];
  let second_group = &groups[1];
  
  // Get positions of all variables in the final ordering
  let mut positions: HashMap<VID, usize> = HashMap::new();
  for (i, &v) in scaffold.vids.iter().enumerate() {
    positions.insert(v, i);
  }
  
  // For every variable in the first group
  for &v1 in first_group {
    // Its position should be before every variable in the second group
    for &v2 in second_group {
      assert!(positions[&v1] < positions[&v2], 
              "Variable {:?} from first group should be before variable {:?} from second group", 
              v1, v2);
    }
  }
  
  // Verify relative ordering is maintained within each group
  // First, determine the original relative ordering within each group
  let mut original_positions: HashMap<VID, usize> = HashMap::new();
  for (i, &v) in initial_vids.iter().enumerate() {
    original_positions.insert(v, i);
  }
  
  // For each group, check that the relative ordering is maintained
  for group in &groups {
    let mut group_vars: Vec<VID> = group.iter().cloned().collect();
    // Sort by original position
    group_vars.sort_by_key(|&v| original_positions[&v]);
    
    // Check that this order is maintained in the final positions
    for i in 0..group_vars.len()-1 {
      assert!(positions[&group_vars[i]] < positions[&group_vars[i+1]],
              "Relative ordering not maintained for {:?} and {:?} within group",
              group_vars[i], group_vars[i+1]);
    }
  }
  
  // Make sure the final plan is empty - indicating all variables are properly placed
  let final_plan = plan_regroup(&scaffold.vids, &groups);
  assert!(final_plan.is_empty(), "Final plan should be empty, but got: {:?}", final_plan);
}