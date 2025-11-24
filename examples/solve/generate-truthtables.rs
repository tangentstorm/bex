//! Generate truth tables for testing variable ordering in BDDs.
//!
//! This tool generates truth tables of varying sizes (10-16 variables) using various methods
//! to create interesting Boolean functions with different structural properties.
//!
//! The size variation is key: it allows testing whether LSB alignment (bex) or MSB alignment
//! (traditional) results in more node sharing when functions of different sizes coexist.
//!
//! Usage: generate-truthtables <output-dir>

use std::env;
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::Path;
use bex::nid::{NID, O, I};
use bex::vid::VID;
use bex::ast::RawASTBase;
use bex::base::Base;

/// Check if a number is prime using trial division
fn is_prime(n: usize) -> bool {
    if n < 2 { return false; }
    if n == 2 { return true; }
    if n % 2 == 0 { return false; }

    let limit = (n as f64).sqrt() as usize;
    for i in (3..=limit).step_by(2) {
        if n % i == 0 { return false; }
    }
    true
}

/// Generate truth table where f(i) = 1 if i is prime
fn generate_primality(nvars: usize) -> Vec<u8> {
    let size = 1 << nvars;
    (0..size).map(|i| if is_prime(i) { 1 } else { 0 }).collect()
}

/// Generate truth table where f(i) = 1 if i mod k is in the set
fn generate_modulo(nvars: usize, k: usize, set: &[usize]) -> Vec<u8> {
    let size = 1 << nvars;
    (0..size).map(|i| if set.contains(&(i % k)) { 1 } else { 0 }).collect()
}

/// Generate truth table using SHA-256 from a seed string
fn generate_sha256(nvars: usize, seed: &str) -> Vec<u8> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let size = 1 << nvars;
    let mut result = Vec::with_capacity(size);
    let mut current = seed.to_string();

    // Generate bits using a simple hash chain
    while result.len() < size {
        let mut hasher = DefaultHasher::new();
        current.hash(&mut hasher);
        let hash = hasher.finish();
        current = hash.to_string();

        // Extract 64 bits from the hash
        for j in 0..64 {
            if result.len() < size {
                result.push(((hash >> j) & 1) as u8);
            }
        }
    }

    result
}

/// Generate a random Boolean expression AST with n nodes
fn generate_random_ast(nvars: usize, num_nodes: usize, seed: u64) -> Vec<u8> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut base = RawASTBase::new();
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);

    // Start with variables
    let mut nodes: Vec<NID> = (0..nvars).map(|i| NID::from_vid(VID::var(i as u32))).collect();

    // Build random expression tree
    for i in 0..num_nodes {
        (i + seed as usize).hash(&mut hasher);
        let hash = hasher.finish();

        let op = hash % 4; // 0=AND, 1=OR, 2=XOR, 3=NOT
        let idx1 = (hash >> 8) as usize % nodes.len();
        let idx2 = (hash >> 16) as usize % nodes.len();

        let new_node = match op {
            0 => base.and(nodes[idx1], nodes[idx2]),
            1 => base.or(nodes[idx1], nodes[idx2]),
            2 => base.xor(nodes[idx1], nodes[idx2]),
            3 => !nodes[idx1],
            _ => unreachable!(),
        };

        nodes.push(new_node);
    }

    // Evaluate the final expression for all input combinations
    let root = *nodes.last().unwrap();
    let size = 1 << nvars;
    let mut result = Vec::with_capacity(size);

    for i in 0..size {
        let mut vals = std::collections::HashMap::new();
        for v in 0..nvars {
            let bit = (i >> v) & 1;
            vals.insert(VID::var(v as u32), if bit == 1 { I } else { O });
        }
        let output = base.eval(root, &vals);
        result.push(if output == I { 1 } else { 0 });
    }

    result
}

/// Generate truth table for primorial factorization
/// f(i) = 1 if i can be expressed as a product of first n primes
fn generate_primorial_divisible(nvars: usize, n: usize) -> Vec<u8> {
    let primes = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47];
    let primorial: usize = primes.iter().take(n).product();
    let size = 1 << nvars;

    if primorial >= size {
        // If primorial is too large, check divisibility instead
        (0..size).map(|i| {
            if i == 0 { 0 }
            else {
                let is_div = primes.iter().take(n).any(|&p| i % p == 0);
                if is_div { 1 } else { 0 }
            }
        }).collect()
    } else {
        (0..size).map(|i| if i % primorial == 0 { 1 } else { 0 }).collect()
    }
}

/// Generate truth table for "has n prime factors"
fn generate_num_prime_factors(nvars: usize, target: usize) -> Vec<u8> {
    let primes = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47];
    let size = 1 << nvars;

    (0..size).map(|mut i| {
        if i == 0 || i == 1 { return 0; }

        let mut count = 0;
        for &p in &primes {
            while i % p == 0 {
                count += 1;
                i /= p;
                if count > target { return 0; }
            }
        }

        if count == target { 1 } else { 0 }
    }).collect()
}

/// Write a truth table to a file
fn write_tt<P: AsRef<Path>>(path: P, data: &[u8]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(data)?;
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <output-dir>", args[0]);
        eprintln!();
        eprintln!("Generates truth tables of varying sizes (10-16 variables) using various methods.");
        eprintln!("Size variation allows testing LSB vs MSB alignment in node sharing.");
        std::process::exit(1);
    }

    let output_dir = &args[1];
    create_dir_all(output_dir).expect("Failed to create output directory");

    println!("Generating truth tables with varying sizes (10-16 variables)...");
    println!("This creates a \"ramp\" of sizes to test LSB vs MSB node sharing.");
    println!();

    // Create a ramp of sizes from 10 to 16 variables
    // Distribute different function types across the range
    let var_sizes = [10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16];
    let mut idx = 0;

    // 1. Primality test
    println!("Generating primality tests...");
    let nvars = var_sizes[idx]; idx += 1;
    let tt = generate_primality(nvars);
    let ones = tt.iter().filter(|&&b| b == 1).count();
    write_tt(format!("{}/prime-{}vars.tt", output_dir, nvars), &tt).unwrap();
    println!("  prime-{}vars.tt: {} bits, {} ones ({:.2}%)", nvars, tt.len(), ones, 100.0 * ones as f64 / tt.len() as f64);

    // 2. Modulo tests
    println!("\nGenerating modulo tests...");
    for (k, name, set) in [
        (3, "mod3-eq0", vec![0]),
        (3, "mod3-ne0", vec![1, 2]),
        (7, "mod7-eq0", vec![0]),
        (7, "mod7-prime", vec![1, 2, 3, 4, 6]),
        (16, "mod16-pow2", vec![0, 1, 2, 4, 8]),
        (256, "mod256-low", vec![0, 1, 2, 3, 4, 5, 6, 7]),
    ] {
        let nvars = var_sizes[idx]; idx += 1;
        let tt = generate_modulo(nvars, k, &set);
        let ones = tt.iter().filter(|&&b| b == 1).count();
        write_tt(format!("{}/{}-{}vars.tt", output_dir, name, nvars), &tt).unwrap();
        println!("  {}-{}vars.tt: {} bits, {} ones ({:.2}%)", name, nvars, tt.len(), ones, 100.0 * ones as f64 / tt.len() as f64);
    }

    // 3. SHA-256 based (pseudo-random but deterministic)
    println!("\nGenerating SHA-256 based truth tables...");
    for (seed, name) in [
        ("bex", "sha-bex"),
        ("ordering", "sha-ordering"),
        ("test123", "sha-test"),
    ] {
        let nvars = var_sizes[idx]; idx += 1;
        let tt = generate_sha256(nvars, seed);
        let ones = tt.iter().filter(|&&b| b == 1).count();
        write_tt(format!("{}/{}-{}vars.tt", output_dir, name, nvars), &tt).unwrap();
        println!("  {}-{}vars.tt: {} bits, {} ones ({:.2}%)", name, nvars, tt.len(), ones, 100.0 * ones as f64 / tt.len() as f64);
    }

    // 4. Random AST generation
    println!("\nGenerating random AST truth tables...");
    for (nodes, seed) in [
        (10, 42),
        (20, 123),
        (50, 999),
    ] {
        let nvars = var_sizes[idx]; idx += 1;
        let tt = generate_random_ast(nvars, nodes, seed);
        let ones = tt.iter().filter(|&&b| b == 1).count();
        let name = format!("ast-n{}-s{}", nodes, seed);
        write_tt(format!("{}/{}-{}vars.tt", output_dir, name, nvars), &tt).unwrap();
        println!("  {}-{}vars.tt: {} bits, {} ones ({:.2}%)", name, nvars, tt.len(), ones, 100.0 * ones as f64 / tt.len() as f64);
    }

    // 5. Primorial-based functions
    println!("\nGenerating primorial-based truth tables...");
    for n in [3, 4, 5, 6] {
        let nvars = var_sizes[idx]; idx += 1;
        let tt = generate_primorial_divisible(nvars, n);
        let ones = tt.iter().filter(|&&b| b == 1).count();
        write_tt(format!("{}/primorial-div-p{}-{}vars.tt", output_dir, n, nvars), &tt).unwrap();
        println!("  primorial-div-p{}-{}vars.tt: {} bits, {} ones ({:.2}%)", n, nvars, tt.len(), ones, 100.0 * ones as f64 / tt.len() as f64);
    }

    // 6. Number of prime factors
    println!("\nGenerating prime factor count truth tables...");
    for target in [1, 2, 3, 4] {
        let nvars = var_sizes[idx]; idx += 1;
        let tt = generate_num_prime_factors(nvars, target);
        let ones = tt.iter().filter(|&&b| b == 1).count();
        write_tt(format!("{}/num-factors-{}-{}vars.tt", output_dir, target, nvars), &tt).unwrap();
        println!("  num-factors-{}-{}vars.tt: {} bits, {} ones ({:.2}%)", target, nvars, tt.len(), ones, 100.0 * ones as f64 / tt.len() as f64);
    }

    println!("\nAll truth tables generated successfully in {}/", output_dir);
    let total_files = std::fs::read_dir(output_dir).unwrap().count();
    println!("Total files: {}", total_files);
    println!("Sizes range from 2^10 (1KB) to 2^16 (64KB)");
}
