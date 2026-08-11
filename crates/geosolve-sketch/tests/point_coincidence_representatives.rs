// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{DocumentConstraintDefinition, DocumentElementId, SketchDocument};

#[test]
fn active_coincident_constraints_form_transitive_point_components() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let first = document.add_point("first", [0.0, 0.0]).unwrap();
    let second = document.add_point("second", [1.0, 0.0]).unwrap();
    let third = document.add_point("third", [2.0, 0.0]).unwrap();
    let unrelated = document.add_point("unrelated", [3.0, 0.0]).unwrap();

    document
        .add_constraint(
            "second to third",
            DocumentConstraintDefinition::Coincident {
                first: second,
                second: third,
            },
        )
        .unwrap();
    document
        .add_constraint(
            "first to second",
            DocumentConstraintDefinition::Coincident { first, second },
        )
        .unwrap();

    let representatives = document.point_coincidence_representatives();
    let expected = first.min(second).min(third);
    assert_eq!(representatives.len(), 4);
    assert_eq!(representatives[&first], expected);
    assert_eq!(representatives[&second], expected);
    assert_eq!(representatives[&third], expected);
    assert_eq!(representatives[&unrelated], unrelated);
}

#[test]
fn suppressed_coincident_constraints_do_not_join_point_components() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let first = document.add_point("first", [0.0, 0.0]).unwrap();
    let second = document.add_point("second", [1.0, 0.0]).unwrap();
    let third = document.add_point("third", [2.0, 0.0]).unwrap();

    document
        .add_constraint(
            "active join",
            DocumentConstraintDefinition::Coincident { first, second },
        )
        .unwrap();
    let suppressed = document
        .add_constraint(
            "suppressed join",
            DocumentConstraintDefinition::Coincident {
                first: second,
                second: third,
            },
        )
        .unwrap();
    document
        .set_element_user_suppressed(DocumentElementId::Constraint(suppressed), true)
        .unwrap();

    let representatives = document.point_coincidence_representatives();
    let expected = first.min(second);
    assert_eq!(representatives[&first], expected);
    assert_eq!(representatives[&second], expected);
    assert_eq!(representatives[&third], third);
}

#[test]
fn coordinate_proximity_never_implies_point_coincidence() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let first = document.add_point("first", [4.0, -2.0]).unwrap();
    let exact_overlap = document.add_point("exact overlap", [4.0, -2.0]).unwrap();
    let near_overlap = document
        .add_point("near overlap", [4.0 + 1.0e-12, -2.0])
        .unwrap();

    let representatives = document.point_coincidence_representatives();
    assert_eq!(representatives.len(), 3);
    assert_eq!(representatives[&first], first);
    assert_eq!(representatives[&exact_overlap], exact_overlap);
    assert_eq!(representatives[&near_overlap], near_overlap);
}
