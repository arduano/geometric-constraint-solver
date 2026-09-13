// SPDX-License-Identifier: GPL-3.0-or-later
//! Authenticated data-only compiler patch context shared by every editing host.
use crate::{CodeProject, PatchModuleArtifact};
use std::collections::BTreeMap;

/// Resolves only exact pinned module/export bindings from a validated project.
/// # Errors
/// Rejects invalid project/artifact authority and ambiguous imported bindings.
pub fn managed_compiler_patches(
    project: &CodeProject,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    project.validate().map_err(|error| error.to_string())?;
    let mut patches = BTreeMap::new();
    for import in &project.managed.imports {
        for binding in &import.bindings {
            let matches = project
                .artifacts
                .values()
                .filter_map(|value| {
                    let artifact =
                        serde_json::from_value::<PatchModuleArtifact>(value.clone()).ok()?;
                    (artifact.module_specifier == import.module && artifact.export_name == *binding)
                        .then_some(value.clone())
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => {}
                [artifact] => {
                    if patches.insert(binding.clone(), artifact.clone()).is_some() {
                        return Err(format!(
                            "managed compiler patch binding `{binding}` is ambiguous"
                        ));
                    }
                }
                _ => {
                    return Err(format!(
                        "managed compiler patch binding `{binding}` resolves more than once"
                    ));
                }
            }
        }
    }
    Ok(patches)
}
