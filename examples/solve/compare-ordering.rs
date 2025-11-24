//! Compare BDD sizes between normal (x0 at bottom) and traditional (v0 at top) ordering.
//!
//! This program reads binary files containing truth tables (2^n bits each) and builds
//! BDDs using two different variable orderings:
//! 1. Normal bex ordering: vars (x0 at bottom)
//! 2. Traditional ordering: virs (v0 at top when tradord feature is enabled)
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
fn build_bdd_from_tt(base: &mut BddBase, tt: &[u8], var_fn: fn(usize) -> VID) -> NID {
    build_bdd_aux(base, tt, 0, var_fn)
}

/// Recursive helper for building BDD from truth table
fn build_bdd_aux(base: &mut BddBase, tt: &[u8], var_idx: usize, var_fn: fn(usize) -> VID) -> NID {
    // Base case: if all bits are the same, return constant
    if tt.is_empty() {
        return O;
    }

    let all_same = tt.iter().all(|&b| b == tt[0]);
    if all_same {
        return if tt[0] == 0 { O } else { I };
    }

    // Recursive case: split truth table and build ITE
    let mid = tt.len() / 2;
    let lo_half = &tt[0..mid];
    let hi_half = &tt[mid..];

    let lo = build_bdd_aux(base, lo_half, var_idx + 1, var_fn);
    let hi = build_bdd_aux(base, hi_half, var_idx + 1, var_fn);

    // If lo and hi are the same, no need for ITE
    if lo == hi {
        return lo;
    }

    // Create ITE node with current variable
    let var = var_fn(var_idx);
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
        std::process::exit(1);
    }

    println!("Comparing BDD sizes: normal ordering (x0 at bottom) vs traditional ordering (v0 at top)");
    println!("{}", "=".repeat(80));

    let mut total_normal = 0;
    let mut total_trad = 0;
    let mut file_count = 0;

    for filename in &args[1..] {
        match read_truth_table(filename) {
            Ok(tt) => {
                let num_vars = (tt.len() as f64).log2() as usize;
                println!("\nFile: {}", filename);
                println!("  Truth table size: {} bits ({} variables)", tt.len(), num_vars);

                // Build with normal ordering (using vars: x0, x1, ...)
                let mut base_normal = BddBase::new();
                let root_normal = build_bdd_from_tt(&mut base_normal, &tt, |i| VID::var(i as u32));
                let size_normal = count_nodes(&mut base_normal, root_normal);

                // Build with traditional ordering (using virs: v0, v1, ...)
                let mut base_trad = BddBase::new();
                let root_trad = build_bdd_from_tt(&mut base_trad, &tt, |i| VID::vir(i as u32));
                let size_trad = count_nodes(&mut base_trad, root_trad);

                println!("  Normal ordering (x-vars):      {} nodes", size_normal);
                println!("  Traditional ordering (v-vars): {} nodes", size_trad);

                if size_normal < size_trad {
                    println!("  → Normal is smaller by {} nodes ({:.1}%)",
                             size_trad - size_normal,
                             100.0 * (size_trad - size_normal) as f64 / size_trad as f64);
                } else if size_trad < size_normal {
                    println!("  → Traditional is smaller by {} nodes ({:.1}%)",
                             size_normal - size_trad,
                             100.0 * (size_normal - size_trad) as f64 / size_normal as f64);
                } else {
                    println!("  → Both orderings have the same size");
                }

                total_normal += size_normal;
                total_trad += size_trad;
                file_count += 1;
            }
            Err(e) => {
                eprintln!("Error reading {}: {}", filename, e);
            }
        }
    }

    if file_count > 1 {
        println!("\n{}", "=".repeat(80));
        println!("Summary:");
        println!("  Total nodes (normal):      {}", total_normal);
        println!("  Total nodes (traditional): {}", total_trad);
        if total_normal < total_trad {
            println!("  → Normal is smaller overall by {} nodes ({:.1}%)",
                     total_trad - total_normal,
                     100.0 * (total_trad - total_normal) as f64 / total_trad as f64);
        } else if total_trad < total_normal {
            println!("  → Traditional is smaller overall by {} nodes ({:.1}%)",
                     total_normal - total_trad,
                     100.0 * (total_normal - total_trad) as f64 / total_normal as f64);
        } else {
            println!("  → Both orderings have the same total size");
        }
    }
}
