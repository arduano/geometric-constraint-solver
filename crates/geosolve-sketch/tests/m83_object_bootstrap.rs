// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentElementId, DocumentParameterKind, DocumentParameterTarget, GeometryRole, ScalarDomain,
    ScalarUnit, SketchDocument, SketchObjectBootstrap,
};

#[test]
fn per_object_bootstrap_reconstructs_exact_native_identity_and_side_tables() {
    let mut original = SketchDocument::new(3.0).unwrap();
    let rectangle = original
        .add_rectangle("fixture", [1.0, 2.0], 4.0, 3.0)
        .unwrap();
    original
        .set_geometry_role(rectangle.curves[0], GeometryRole::Construction)
        .unwrap();
    let reference_target = original
        .add_scalar(
            "reference target",
            4.0,
            ScalarUnit::Length,
            ScalarDomain::Finite,
        )
        .unwrap();
    let reference = original
        .add_dimension(
            "reference length",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(rectangle.curves[1]),
                target: reference_target,
            },
            DocumentDimensionMode::Reference,
        )
        .unwrap();
    let input = original
        .add_parameter("width", DocumentParameterKind::Length)
        .unwrap();
    original
        .add_parameter_binding(
            input,
            DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
        )
        .unwrap();
    let output = original
        .add_parameter("measured", DocumentParameterKind::Length)
        .unwrap();
    original.add_parameter_output(output, reference).unwrap();

    let mut bootstrap = SketchObjectBootstrap::new(
        original.model_scale(),
        original.persistent_identity_high_water(),
    )
    .unwrap();
    for value in original.points() {
        bootstrap.push_point(value.clone());
    }
    for value in original.scalars() {
        bootstrap.push_scalar(value.clone());
    }
    for value in original.curves() {
        bootstrap.push_curve(value.clone());
    }
    for value in original.contacts() {
        bootstrap.push_contact(value.clone());
    }
    for value in original.trim_views() {
        bootstrap.push_trim_view(*value);
    }
    for value in original.constraints() {
        bootstrap.push_constraint(value.clone());
    }
    for value in original.dimensions() {
        bootstrap.push_dimension(value.clone());
    }
    for value in original.parameters() {
        bootstrap.push_parameter(value.clone());
    }
    for value in original.parameter_bindings() {
        bootstrap.push_parameter_binding(*value);
    }
    for value in original.parameter_outputs() {
        bootstrap.push_parameter_output(*value);
    }
    for value in original.external_bindings() {
        bootstrap.push_external_binding(value.clone());
    }
    for source in original.source_order() {
        bootstrap.push_source(*source);
    }
    for role in original.explicit_geometry_roles() {
        bootstrap.push_geometry_role(role);
    }
    for element in original.user_inactive_elements() {
        bootstrap.push_user_inactive_element(element);
    }
    if let Some(activation) = original.host_configuration_activation() {
        bootstrap.set_host_activation(activation.clone());
    }
    bootstrap.set_semantic_source_reservations(original.semantic_source_reservations());

    let restored = bootstrap.finish().unwrap();
    assert_eq!(
        restored.to_draft_v5_json().unwrap(),
        original.to_draft_v5_json().unwrap()
    );
    assert_eq!(
        restored.persistent_identity_high_water(),
        original.persistent_identity_high_water()
    );
}

#[test]
fn per_object_bootstrap_rejects_missing_dependency_without_partial_document() {
    let mut original = SketchDocument::new(1.0).unwrap();
    let start = original.add_point("start", [0.0, 0.0]).unwrap();
    let end = original.add_point("end", [2.0, 0.0]).unwrap();
    let segment = original
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let mut bootstrap = SketchObjectBootstrap::new(
        original.model_scale(),
        original.persistent_identity_high_water(),
    )
    .unwrap();
    bootstrap.push_curve(original.curve(segment).unwrap().clone());
    assert!(bootstrap.finish().is_err());
}

#[test]
fn direct_dependency_audit_is_exact_and_non_transitive() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let end = document.add_point("end", [2.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let target = document
        .add_scalar("length", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let dimension = document
        .add_dimension(
            "length",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(line),
                target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();

    assert_eq!(
        document.direct_dependencies(DocumentElementId::Curve(line)),
        vec![
            DocumentElementId::Document(document.id()),
            DocumentElementId::Point(start),
            DocumentElementId::Point(end),
        ]
    );
    assert_eq!(
        document.direct_dependencies(DocumentElementId::Dimension(dimension)),
        vec![
            DocumentElementId::Document(document.id()),
            DocumentElementId::Curve(line),
            DocumentElementId::Scalar(target),
        ]
    );
    let source = document.dimension(dimension).unwrap().source_id;
    assert_eq!(
        document.direct_dependencies(DocumentElementId::Source(source)),
        vec![
            DocumentElementId::Document(document.id()),
            DocumentElementId::Dimension(dimension),
        ]
    );
}
