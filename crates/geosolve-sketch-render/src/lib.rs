// SPDX-License-Identifier: GPL-3.0-or-later

//! Target-neutral presentation for an authoritative `GeoSolve` sketch scene.
//!
//! This crate owns the canonical canvas camera, deterministic SVG composition,
//! and self-contained export wrapper shared by native and WASM consumers. It
//! consumes already-composed editor/domain DTOs and owns no solver equations,
//! accepted-scene authority, DOM, event loop, storage, or download behavior.

pub mod icons;
#[cfg(not(target_arch = "wasm32"))]
mod native_png;
mod scene;
mod standalone;

#[cfg(not(target_arch = "wasm32"))]
pub use native_png::*;
pub use scene::*;
pub use standalone::*;
