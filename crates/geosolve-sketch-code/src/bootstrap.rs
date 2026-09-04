// SPDX-License-Identifier: GPL-3.0-or-later

//! Private compiler authority for a newly authored empty sketch.
//!
//! This is project bootstrap state, not a bundled sample or catalog entry.

#[cfg(test)]
pub(crate) const AUTHORED_EMPTY_SOURCE: &str =
    include_str!("../assets/bootstrap/authored-empty.sketch.ts");

pub(crate) const AUTHORED_EMPTY_COMPILED_SOURCE: &str =
    include_str!("../assets/bootstrap/authored-empty.compiled.json");
