// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared source-text synchronization, independent of accepted geometry.

pub mod text;
pub use text::*;

pub mod authority;

pub mod protocol;

#[cfg(not(target_arch = "wasm32"))]
pub mod journal;

pub mod source_patch;

pub mod targets;

pub mod history;

pub mod document;
