//! Causal-profiler integration via the [`coz`](https://github.com/plasma-umass/coz) crate.
//!
//! `coz` is a *causal* profiler: you annotate meaningful milestones with
//! *progress points*, and coz simulates virtual speedups of individual
//! lines of code to estimate how much each one actually contributes to
//! end-to-end throughput. This is especially useful for parallel code
//! where the hottest function on a flamegraph is often not the critical
//! path. See the comments on issue #9 for motivation.
//!
//! The macros defined here (`coz_progress!`, `coz_begin!`, `coz_end!`,
//! `coz_scope!`) expand to calls into the `coz` crate when the `coz`
//! cargo feature is enabled, and to no-ops otherwise, so they are safe
//! to leave sprinkled throughout the codebase.
//!
//! # Usage
//!
//! ```ignore
//! use bex::coz_progress;
//! // mark the completion of one unit of work:
//! coz_progress!("bdd-ite");
//! ```
//!
//! To profile a binary:
//!
//! ```text
//! cargo build --release --features coz --example bdd-solve
//! coz run --- target/release/examples/bdd-solve
//! # then open profile.coz in the coz web viewer.
//! ```
//!
//! # Constraints
//!
//! - Linux only (uses `perf_event_open`).
//! - Needs release builds with debug symbols; the existing
//!   `debug = "line-tables-only"` in `Cargo.toml` is sufficient.
//! - Expect 2-5x slowdown while profiling.

/// Emit a named coz progress point. Expands to a no-op unless the `coz`
/// cargo feature is enabled.
#[cfg(feature = "coz")]
#[macro_export]
macro_rules! coz_progress {
  ($name:literal) => { ::coz::progress!($name) };
  () => { ::coz::progress!() };
}

#[cfg(not(feature = "coz"))]
#[macro_export]
macro_rules! coz_progress {
  ($name:literal) => { () };
  () => { () };
}

/// Begin a coz throughput scope. Pair with `coz_end!`. No-op without
/// the `coz` feature.
#[cfg(feature = "coz")]
#[macro_export]
macro_rules! coz_begin {
  ($name:literal) => { ::coz::begin!($name) };
}

#[cfg(not(feature = "coz"))]
#[macro_export]
macro_rules! coz_begin {
  ($name:literal) => { () };
}

/// End a coz throughput scope opened with `coz_begin!`. No-op without
/// the `coz` feature.
#[cfg(feature = "coz")]
#[macro_export]
macro_rules! coz_end {
  ($name:literal) => { ::coz::end!($name) };
}

#[cfg(not(feature = "coz"))]
#[macro_export]
macro_rules! coz_end {
  ($name:literal) => { () };
}

/// Create a coz throughput scope guard bound to the current lexical
/// scope. When the guard drops, the scope ends. `coz::scope!` itself
/// introduces a `_coz_scope_guard` binding; this wrapper just forwards
/// to it (and is a no-op without the `coz` feature).
#[cfg(feature = "coz")]
#[macro_export]
macro_rules! coz_scope {
  ($name:literal) => { ::coz::scope!($name); };
}

#[cfg(not(feature = "coz"))]
#[macro_export]
macro_rules! coz_scope {
  ($name:literal) => { () };
}
