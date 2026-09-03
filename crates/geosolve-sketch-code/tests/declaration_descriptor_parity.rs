// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use geosolve_sketch_code::{
    CODE_AUTHORING_FAMILIES, CodeAuthoringArgumentKind, CodeAuthoringAvailability, CodeResultShape,
    FeatureKind, code_authoring_family, declaration_result_catalog, public_code_authoring_families,
    resolve_code_authoring_declaration, typescript_declaration_result_catalog,
};

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
fn contact_results_are_typed_leaves_beneath_their_contact_objects() {
    let catalog = declaration_result_catalog();
    let point_on_curve = &catalog["constraint.pointOnCurve"].outputs;
    let CodeResultShape::Object { fields } = point_on_curve else {
        panic!("point-on-curve result must be an object")
    };
    let CodeResultShape::Object { fields: contact } = &fields["contact"] else {
        panic!("point-on-curve contact must be an object")
    };
    assert_eq!(
        contact["contact"],
        CodeResultShape::Leaf {
            kind: FeatureKind::Contact,
        }
    );
    assert_eq!(
        contact["parameter"],
        CodeResultShape::Leaf {
            kind: FeatureKind::Scalar,
        }
    );

    let paired = &catalog["constraint.curveCurveContact"].outputs;
    let CodeResultShape::Object { fields } = paired else {
        panic!("curve-curve contact result must be an object")
    };
    let CodeResultShape::Object { fields: contacts } = &fields["contacts"] else {
        panic!("paired contacts must be an object")
    };
    for side in ["first", "second"] {
        let CodeResultShape::Object { fields: contact } = &contacts[side] else {
            panic!("paired contact side must be an object")
        };
        assert_eq!(
            contact["contact"],
            CodeResultShape::Leaf {
                kind: FeatureKind::Contact,
            }
        );
    }
}

#[test]
fn result_catalog_contains_only_clean_methods_and_patch_private_composites() {
    let actual = declaration_result_catalog()
        .into_keys()
        .collect::<BTreeSet<_>>();
    let mut expected = CODE_AUTHORING_FAMILIES
        .iter()
        .map(|family| format!("{}.{}", family.namespace, family.method))
        .collect::<BTreeSet<_>>();
    expected.extend([
        "computed.fillet".to_owned(),
        "computed.roundedRectangleProfile".to_owned(),
    ]);
    assert_eq!(actual, expected);
    for retired in [
        "aggregate.chain",
        "aggregate.profile",
        "geometry.circle",
        "geometry.line",
        "geometry.rectangle",
        "geometry.rounded_rectangle",
    ] {
        assert!(
            !actual.contains(retired),
            "retired alias `{retired}` leaked"
        );
    }
}

#[test]
fn clean_authoring_catalog_is_publicly_consumable_and_exhaustive() {
    assert_eq!(CODE_AUTHORING_FAMILIES.len(), 85);
    assert_eq!(public_code_authoring_families().count(), 83);
    assert_eq!(
        CODE_AUTHORING_FAMILIES
            .iter()
            .filter(|family| {
                family.availability == CodeAuthoringAvailability::RequiresHostSnapshot
            })
            .count(),
        2
    );
    assert!(code_authoring_family("geometry", "quadraticBezier").is_some());
    assert!(code_authoring_family("geometry", "openControlBSpline").is_some());
    assert!(code_authoring_family("geometry", "openControlNurbs").is_some());
    let bezier = resolve_code_authoring_declaration("geometry", "quadraticBezier", 0).unwrap();
    assert_eq!(
        bezier
            .inputs
            .iter()
            .map(|input| input.name.as_str())
            .collect::<Vec<_>>(),
        ["start", "control", "end"]
    );
    assert!(
        bezier
            .inputs
            .iter()
            .all(|input| input.kind == CodeAuthoringArgumentKind::Point)
    );
}
