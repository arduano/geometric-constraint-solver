// SPDX-License-Identifier: GPL-3.0-or-later
//! Regenerate a target's seed corpus from `FuzzInput::golden_seeds()`.
//!
//! The seed corpus is the set of known-good inputs the fuzzer starts from and
//! mutates outward (FUZZING.md §5). This test wipes any polluted corpus (left
//! over from prior runs) and writes, for every family in `geosolve_survey::FAMILIES`,
//! the DETERMINISTIC witness crossed with the full option-index/flag latin
//! square. That is exactly `FuzzInput::golden_seeds(family_index)`.
//!
//! Usage:
//! ```sh
//! GOLDEN_CORPUS_DIR=fuzz/corpus/02_fixture_perturbation \
//!   cargo test --manifest-path fuzz/Cargo.toml --test prefill_corpus -- --exact --nocapture
//! ```

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use fuzz::model::{FuzzInput, LEN};
use geosolve_survey::FAMILIES;

#[test]
fn prefill_golden_corpus() {
    // `cargo test` runs with cwd = crate root (`fuzz/`), so resolve the corpus
    // path against the workspace root (parent of CARGO_MANIFEST_DIR) when it is
    // relative. Absolute paths pass through unchanged.
    let raw = env::var("GOLDEN_CORPUS_DIR").expect("GOLDEN_CORPUS_DIR must be set to a corpus dir");
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ws_root = manifest
        .parent()
        .expect("CARGO_MANIFEST_DIR has a parent")
        .to_path_buf();
    let dir = if Path::new(&raw).is_absolute() {
        PathBuf::from(&raw)
    } else {
        ws_root.join(&raw)
    };
    fs::create_dir_all(&dir).expect("create corpus dir");

    // Wipe any polluted corpus (fuzzed inputs from prior runs).
    for entry in fs::read_dir(&dir).expect("read corpus dir") {
        let entry = entry.expect("read dir entry");
        if entry.file_type().expect("dir entry file type").is_file() {
            fs::remove_file(entry.path()).expect("remove polluted corpus file");
        }
    }

    // Seed every family's golden witnesses (DETERMINISTIC + option-index latin
    // square). The corpus is identical for all four targets.
    let mut count = 0usize;
    for (family_index, family) in FAMILIES.iter().enumerate() {
        for (seed_index, seed) in FuzzInput::golden_seeds(family_index as u32)
            .iter()
            .enumerate()
        {
            let name = format!("{}_{:03}.bin", family.id(), seed_index);
            fs::write(dir.join(name), &seed[..LEN]).expect("write golden seed");
            count += 1;
        }
    }

    let expected = FAMILIES.len() * 193;
    assert_eq!(
        count, expected,
        "prefilled {} golden seeds, expected {}",
        count, expected
    );
    eprintln!("prefilled {dir:?} with {count} golden seeds");
}
