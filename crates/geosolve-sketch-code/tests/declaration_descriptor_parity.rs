// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::Path;

use geosolve_sketch_code::{
    CODE_DECLARATION_FAMILIES, NativeDeclarationContract, code_declaration_family,
    declaration_result_catalog, typescript_declaration_result_catalog,
};
use geosolve_sketch_intent::{GeometryRecipeKind, IntentNodeKind};

#[test]
fn checked_in_typescript_result_catalog_is_exactly_rust_generated() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("code crate lives under the workspace crates directory");
    let path = workspace.join("packages/geosolve-sketch-code/src/generated-declaration-results.ts");
    let checked_in = fs::read_to_string(&path).expect("checked-in TypeScript result catalog");
    assert_eq!(checked_in, typescript_declaration_result_catalog());
}

#[test]
fn executable_code_families_are_unique_and_grounded_in_central_intent_schemas() {
    let result_catalog = declaration_result_catalog();
    let mut names = CODE_DECLARATION_FAMILIES
        .iter()
        .map(|descriptor| descriptor.family)
        .collect::<Vec<_>>();
    let original_len = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), original_len);

    for descriptor in &CODE_DECLARATION_FAMILIES {
        assert_eq!(code_declaration_family(descriptor.family), Some(descriptor));
        match descriptor.native {
            NativeDeclarationContract::Geometry(recipe) => {
                let schema = IntentNodeKind::Geometry { recipe }.schema(0);
                assert!(schema.maximum_children <= 4_096);
                assert!(GeometryRecipeKind::ALL.contains(&recipe));
            }
            NativeDeclarationContract::Aggregate(kind) => {
                let schema = IntentNodeKind::Aggregate { aggregate: kind }.schema(0);
                assert!(schema.maximum_children <= 4_096);
            }
            NativeDeclarationContract::CompositePolyline => {
                let schema = IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Polyline,
                }
                .schema(2);
                assert_eq!(schema.minimum_children, 2);
            }
            NativeDeclarationContract::HostAuthoredFillet => {}
        }
        if descriptor.family != "aggregate.chain" && descriptor.family != "aggregate.profile" {
            assert!(
                result_catalog.contains_key(descriptor.family),
                "{} is executable but absent from the typed result catalog",
                descriptor.family,
            );
        }
    }
}
