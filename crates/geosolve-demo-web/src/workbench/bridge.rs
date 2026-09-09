// SPDX-License-Identifier: GPL-3.0-or-later

//! Instance-scoped, DOM-free workbench bridge for presentation toolkits.
//!
//! React, a native shell, a worker, and tests all consume this same bounded
//! JSON contract. The bridge owns exactly one existing Rust document/editor
//! authority; it does not duplicate solver equations, accepted geometry,
//! managed-code history, or persistence state.

// Native unit tests compile this WASM bridge to exercise its DOM-free core,
// while pointer/RPC entry points are consumed by the wasm32 wrapper itself.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::str::FromStr as _;

mod bake;
mod authoring_metadata;
use authoring_metadata::{AuthoringDocumentSnapshot, AuthoringMetadataSnapshot};
