//! Compare BDD sizes between bex ordering (x0 at bottom) and traditional ordering (v0 at top).
//!
//! This program tests node sharing by building ALL truth tables into a SINGLE BddBase
//! and comparing the total number of unique nodes between the two orderings.
//!
//! Usage: compare-ordering <file1> [file2] [file3] ...
//!
//! Each file should contain a truth table as a sequence of bits (0s and 1s as bytes).
//! The file size must be a power of 2 (representing 2^n bits for n variables).

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use bex::bdd::BddBase;
use bex::base::Base;
use bex::nid::{NID, O, I};
use bex::vid::VID;

/// Build a BDD from a truth table using recursive ITE construction
/// The truth table is indexed so that tt[i] = f(bit0(i), bit1(i), ..., bitN(i))
/// where bit0 is the LSB and bitN is the MSB.
///
/// For bex ordering (x0 at bottom), x0 should correspond to LSB, x(N-1) to MSB
/// For traditional ordering (v0 at top), v0 should correspond to MSB, v(N-1) to LSB
fn build_bdd_from_tt(base: &mut BddBase, tt: &[u8], use_vars: bool) -> NID {
    let nvars = (tt.len() as f64).log2() as usize;
    build_bdd_aux(base, tt, 0, nvars, use_vars)
}

/// Recursive helper for building BDD from truth table
/// depth: 0 = top of tree (MSB), increases going down
fn build_bdd_aux(base: &mut BddBase, tt: &[u8], depth: usize, nvars: usize, use_vars: bool) -> NID {
    // Base case: if all bits are the same, return constant
    if tt.is_empty() {
        return O;
    }

    let all_same = tt.iter().all(|&b| b == tt[0]);
    if all_same {
        return if tt[0] == 0 { O } else { I };
    }

    // Recursive case: split truth table and build ITE
    // The first half has the current bit = 0, second half has bit = 1
    // This bit is the MSB of the remaining bits
    let mid = tt.len() / 2;
    let lo_half = &tt[0..mid];
    let hi_half = &tt[mid..];

    let lo = build_bdd_aux(base, lo_half, depth + 1, nvars, use_vars);
    let hi = build_bdd_aux(base, hi_half, depth + 1, nvars, use_vars);

    // If lo and hi are the same, no need for ITE
    if lo == hi {
        return lo;
    }

    // Create ITE node with current variable
    // Both vars and virs use bex ordering by default (x0/v0 at bottom)
    // With tradord feature, virs use traditional ordering (v0 at top)
    let var = if use_vars {
        // vars always use bex ordering: x0 at bottom
        VID::var((nvars - 1 - depth) as u32) // x(N-1) at top, x0 at bottom
    } else {
        // virs ordering depends on tradord feature
        #[cfg(feature = "tradord")]
        {
            // With tradord: v0 at top (traditional)
            VID::vir(depth as u32)
        }
        #[cfg(not(feature = "tradord"))]
        {
            // Without tradord: v0 at bottom (bex ordering)
            VID::vir((nvars - 1 - depth) as u32)
        }
    };

    let var_nid = NID::from_vid(var);
    base.ite(var_nid, hi, lo)
}

/// Count the number of nodes in the BDD (by traversing from root)
fn count_nodes(base: &mut BddBase, root: NID) -> usize {
    use std::collections::HashSet;
    let mut visited = HashSet::new();
    count_nodes_aux(base, root, &mut visited);
    visited.len()
}

fn count_nodes_aux(base: &mut BddBase, n: NID, visited: &mut std::collections::HashSet<NID>) {
    // Remove inversion bit for comparison
    let n_abs = if n.is_inv() { !n } else { n };

    if n.is_const() || visited.contains(&n_abs) {
        return;
    }

    visited.insert(n_abs);

    // Traverse children
    let v = n.vid();
    let lo = base.when_lo(v, n);
    let hi = base.when_hi(v, n);

    count_nodes_aux(base, lo, visited);
    count_nodes_aux(base, hi, visited);
}

/// Count all unique nodes in a BddBase by traversing from multiple roots
fn count_all_nodes(base: &mut BddBase, roots: &[NID]) -> usize {
    use std::collections::HashSet;
    let mut visited = HashSet::new();

    for &root in roots {
        count_nodes_aux(base, root, &mut visited);
    }

    visited.len()
}

/// Read truth table from file
fn read_truth_table<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Validate that the size is a power of 2
    let size = buffer.len();
    if size == 0 || (size & (size - 1)) != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("File size {} is not a power of 2", size)
        ));
    }

    // Validate that all bytes are 0 or 1
    for (i, &byte) in buffer.iter().enumerate() {
        if byte != 0 && byte != 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid byte {} at position {}, expected 0 or 1", byte, i)
            ));
        }
    }

    Ok(buffer)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <file1> [file2] [file3] ...", args[0]);
        eprintln!();
        eprintln!("Each file should contain a truth table as bytes (0 or 1).");
        eprintln!("File size must be a power of 2.");
        eprintln!();
        eprintln!("This tool tests node sharing by building ALL truth tables into a");
        eprintln!("SINGLE BddBase and comparing total node counts between orderings.");
        std::process::exit(1);
    }

    println!("Comparing BDD node sharing: bex ordering (x0 at bottom) vs traditional (v0 at top)");
    println!("{}", "=".repeat(80));
    println!();
    println!("Loading truth tables...");

    // Load all truth tables first
    let mut truth_tables = Vec::new();
    for filename in &args[1..] {
        match read_truth_table(filename) {
            Ok(tt) => {
                let num_vars = (tt.len() as f64).log2() as usize;
                println!("  {} - {} bits ({} variables)", filename, tt.len(), num_vars);
                truth_tables.push((filename.clone(), tt));
            }
            Err(e) => {
                eprintln!("Error reading {}: {}", filename, e);
            }
        }
    }

    if truth_tables.is_empty() {
        eprintln!("No valid truth tables loaded!");
        std::process::exit(1);
    }

    println!();
    println!("Building BDDs with bex ordering (x-vars, x0 at bottom)...");

    // Build all functions into a SINGLE BddBase using x-vars
    let mut base_normal = BddBase::new();
    let mut roots_normal = Vec::new();

    for (filename, tt) in &truth_tables {
        let root = build_bdd_from_tt(&mut base_normal, tt, true);
        roots_normal.push(root);
        println!("  Built {}", filename);
    }

    let total_nodes_normal = count_all_nodes(&mut base_normal, &roots_normal);
    println!("  Total unique nodes: {}", total_nodes_normal);

    println!();
    println!("Building BDDs with traditional ordering (v-vars, v0 at top)...");

    // Build all functions into a SINGLE BddBase using v-vars
    let mut base_trad = BddBase::new();
    let mut roots_trad = Vec::new();

    for (filename, tt) in &truth_tables {
        let root = build_bdd_from_tt(&mut base_trad, tt, false);
        roots_trad.push(root);
        println!("  Built {}", filename);
    }

    let total_nodes_trad = count_all_nodes(&mut base_trad, &roots_trad);
    println!("  Total unique nodes: {}", total_nodes_trad);

    println!();
    println!("{}", "=".repeat(80));
    println!("RESULTS:");
    println!("{}", "=".repeat(80));
    println!();
    println!("Functions tested: {}", truth_tables.len());
    println!("Total nodes (bex ordering):         {} nodes", total_nodes_normal);
    println!("Total nodes (traditional ordering): {} nodes", total_nodes_trad);
    println!();

    if total_nodes_normal < total_nodes_trad {
        let saved = total_nodes_trad - total_nodes_normal;
        let percent = 100.0 * saved as f64 / total_nodes_trad as f64;
        println!("✓ Bex ordering is SMALLER by {} nodes ({:.2}%)", saved, percent);
        println!();
        println!("This means bex's ordering (x0=LSB at bottom) results in more");
        println!("node sharing between functions, confirming the hypothesis!");
    } else if total_nodes_trad < total_nodes_normal {
        let saved = total_nodes_normal - total_nodes_trad;
        let percent = 100.0 * saved as f64 / total_nodes_normal as f64;
        println!("✗ Traditional ordering is SMALLER by {} nodes ({:.2}%)", saved, percent);
        println!();
        println!("This suggests traditional ordering (v0=MSB at top) results in");
        println!("more node sharing for these particular functions.");
    } else {
        println!("= Both orderings have IDENTICAL node counts");
        println!();
        println!("This means both orderings result in the same amount of node");
        println!("sharing for these particular functions.");
    }
}
