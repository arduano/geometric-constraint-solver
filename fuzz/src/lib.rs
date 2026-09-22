//! Shared codec and harness for the geosolve fuzzing corpus.
//!
//! The four fuzz targets in `fuzz/targets/` are separate bin crates in the
//! same package; this lib target (name `fuzz`) holds the shared input codec
//! (`model`) and the oracle/solver harnesses (`harness`) so the targets do
//! `use fuzz::{model, harness};`.
pub mod harness;
pub mod model;
pub mod random_doc;

pub use model::FuzzInput;
