// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::Path;

#[test]
fn solver_domain_intent_and_editor_manifests_do_not_depend_on_code_authoring() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("code crate lives under the workspace crates directory");
    for manifest in [
        "crates/geosolve-core/Cargo.toml",
        "crates/geosolve-sketch/Cargo.toml",
        "crates/geosolve-linkage/Cargo.toml",
        "crates/geosolve-sketch-intent/Cargo.toml",
        "crates/geosolve-constraint-editor/Cargo.toml",
        "packages/geosolve-intent/package.json",
    ] {
        let source = fs::read_to_string(workspace.join(manifest)).expect("dependency manifest");
        assert!(
            !source.contains("geosolve-sketch-code") && !source.contains("@geosolve/sketch-code"),
            "optional code authoring leaked into `{manifest}`",
        );
    }
}
