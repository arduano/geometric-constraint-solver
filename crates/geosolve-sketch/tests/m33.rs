// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(clippy::too_many_lines)]

use std::collections::BTreeSet;

use geosolve_sketch::{
    ContactId, CurveDefinition, CurveId, CurveSpan, DesignPointId, DesignScalarId,
    DocumentAngleOrientation, DocumentArcTangencySide, DocumentBSplineForm,
    DocumentCircleContainment, DocumentCircleTangencyMode, DocumentConicFeature,
    DocumentConicMeasurement, DocumentConstraintDefinition, DocumentCoordinateAxis,
    DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentCurveDirectionRelation,
    DocumentCurveMeasurementKind, DocumentCurveNormalSide, DocumentDimensionDefinition,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentHyperbolaBranch,
    DocumentLineOffsetOrientation, DocumentLineSide, FeatureEndpoint, FeatureRef, PersistentId,
    SKETCH_DOCUMENT_VERSION, ScalarDomain, ScalarUnit, TangentOrientation,
};

const MATRIX: &str = include_str!("../../../docs/M33_CAD_CAPABILITY_MATRIX.md");
const STATUSES: [&str; 7] = [
    "implemented_m32",
    "implemented_m36",
    "implemented_m37",
    "implemented_m38",
    "planned_m58",
    "unsupported_through_m64",
    "conditional",
];

#[derive(Debug)]
struct Table {
    header: Vec<&'static str>,
    rows: Vec<Vec<&'static str>>,
}

impl Table {
    fn column(&self, name: &str) -> usize {
        self.header
            .iter()
            .position(|column| *column == name)
            .unwrap_or_else(|| panic!("missing column {name:?} in {:?}", self.header))
    }

    fn ids(&self) -> Vec<&'static str> {
        self.rows.iter().map(|row| row[0]).collect()
    }
}

fn parse_row(line: &'static str) -> Vec<&'static str> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

fn table(name: &str) -> Table {
    let begin = format!("<!-- M33_TABLE:{name}:BEGIN -->");
    let end = format!("<!-- M33_TABLE:{name}:END -->");
    assert_eq!(MATRIX.matches(&begin).count(), 1, "marker {begin}");
    assert_eq!(MATRIX.matches(&end).count(), 1, "marker {end}");
    let body = MATRIX
        .split_once(&begin)
        .unwrap_or_else(|| panic!("missing marker {begin}"))
        .1
        .split_once(&end)
        .unwrap_or_else(|| panic!("missing marker {end}"))
        .0;
    let lines = body
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('|'))
        .collect::<Vec<_>>();
    assert!(lines.len() >= 3, "table {name} is empty");
    let header = parse_row(lines[0]);
    let separator = parse_row(lines[1]);
    assert_eq!(separator.len(), header.len(), "table {name} separator");
    assert!(separator.iter().all(|cell| {
        cell.len() >= 3 && cell.chars().all(|character| matches!(character, '-' | ':'))
    }));
    let rows = lines[2..]
        .iter()
        .map(|line| parse_row(line))
        .collect::<Vec<_>>();
    for row in &rows {
        assert_eq!(
            row.len(),
            header.len(),
            "malformed row in table {name}: {row:?}"
        );
        assert!(
            row.iter().all(|cell| !cell.is_empty()),
            "empty cell in table {name}: {row:?}"
        );
    }
    Table { header, rows }
}

fn assert_header(table: &Table, expected: &[&str]) {
    assert_eq!(table.header, expected);
}

fn assert_ids(table: &Table, expected: &[&str]) {
    assert_eq!(table.ids(), expected);
    let unique = table.ids().into_iter().collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), table.rows.len(), "duplicate row IDs");
}

fn assert_status_contract(table: &Table) {
    let status_column = table.column("status");
    let target_column = table.column("target");
    let reason_column = table.column("reason");
    for row in &table.rows {
        let expected_target = match row[status_column] {
            "implemented_m32" => "M32",
            "implemented_m36" => "M36",
            "implemented_m37" => "M37",
            "implemented_m38" => "M38",
            "planned_m58" => "M58",
            "unsupported_through_m64" | "conditional" => "M64",
            status => panic!("unknown M33 status {status:?}"),
        };
        assert_eq!(row[target_column], expected_target, "row {}", row[0]);
        assert!(
            !matches!(row[reason_column], "" | "none" | "-"),
            "row {} needs an explicit reason",
            row[0]
        );
    }
}

#[test]
fn marked_capability_tables_are_complete_deterministic_and_typed() {
    let statuses = table("status_vocabulary");
    assert_header(&statuses, &["status", "target", "meaning"]);
    assert_ids(&statuses, &STATUSES);
    for row in &statuses.rows {
        let expected_target = match row[0] {
            "implemented_m32" => "M32",
            "implemented_m36" => "M36",
            "implemented_m37" => "M37",
            "implemented_m38" => "M38",
            "planned_m58" => "M58",
            "unsupported_through_m64" | "conditional" => "M64",
            status => panic!("unknown status {status:?}"),
        };
        assert_eq!(row[1], expected_target);
        assert!(!row[2].is_empty());
    }

    let curves = table("curve_families");
    assert_header(
        &curves,
        &[
            "id",
            "public_definition",
            "form",
            "parameter_topology",
            "status",
            "target",
            "reason",
        ],
    );
    assert_ids(
        &curves,
        &[
            "line",
            "polyline",
            "circle",
            "circular_arc",
            "ellipse",
            "elliptical_arc",
            "rational_quadratic_conic",
            "parabola",
            "hyperbola",
            "quadratic_bezier",
            "cubic_bezier",
            "clamped_b_spline",
            "periodic_b_spline",
            "clamped_nurbs",
            "periodic_nurbs",
        ],
    );
    assert_eq!(curves.rows.len(), 15);

    let features = table("feature_kinds");
    assert_header(
        &features,
        &[
            "id",
            "public_variant_or_target",
            "value_kind",
            "family_applicability",
            "status",
            "target",
            "reason",
        ],
    );
    assert_ids(
        &features,
        &[
            "point",
            "curve_endpoint",
            "curve_center",
            "curve_axis",
            "curve_control",
            "curve_focus",
            "fixed_curve_location",
            "conic_query_center",
            "conic_query_focus",
            "conic_major_axis_endpoint",
            "conic_minor_axis_endpoint",
            "conic_bounded_endpoint",
            "conic_selected_branch_vertex",
            "direction",
            "line_support",
            "curve_span",
            "scalar_property",
        ],
    );
    assert_eq!(features.rows.len(), 17);

    let relations = table("relations");
    assert_header(
        &relations,
        &[
            "id",
            "public_variant_or_target",
            "operands",
            "unit",
            "sign",
            "branch_state",
            "row_emission",
            "status",
            "target",
            "reason",
        ],
    );
    assert_ids(
        &relations,
        &[
            "fixed_point",
            "fixed_coordinate",
            "coincident",
            "horizontal_line",
            "vertical_line",
            "point_on_curve",
            "parallel_lines",
            "perpendicular_lines",
            "equal_segment_length",
            "equal_circle_radius",
            "midpoint_on_line",
            "symmetric_points_about_line",
            "line_circle_tangency",
            "circle_circle_tangency",
            "circle_arc_tangency",
            "line_curve_tangency",
            "curve_curve_contact",
            "curve_curve_tangency",
            "curve_direction",
            "equal_curvature",
            "endpoint_continuity",
            "line_line_fillet",
            "curve_curve_fillet",
            "fixed_scalar",
            "equal_scalar",
            "concentric",
            "collinear",
            "horizontal_points",
            "vertical_points",
            "block_entity",
            "point_symmetry_about_center",
            "entity_symmetry_about_line",
            "equal_circular_radius",
            "equal_distance",
            "equal_angle",
            "contact_constructor",
            "tangent_constructor",
            "equal_path_length",
        ],
    );
    assert_eq!(relations.rows.len(), 38);
    for row in &relations.rows {
        assert_ne!(row[relations.column("unit")], "");
        assert_ne!(row[relations.column("sign")], "");
        assert_ne!(row[relations.column("branch_state")], "");
        assert_ne!(row[relations.column("row_emission")], "");
    }

    let dimensions = table("dimensions_measurements");
    assert_header(
        &dimensions,
        &[
            "id",
            "public_variant_or_target",
            "kind",
            "operands",
            "unit",
            "sign",
            "branch_state",
            "row_emission",
            "status",
            "target",
            "reason",
        ],
    );
    assert_ids(
        &dimensions,
        &[
            "dimension_point_distance",
            "dimension_curve_length",
            "dimension_radius",
            "dimension_diameter",
            "dimension_oriented_angle",
            "dimension_supporting_line_offset",
            "dimension_exact_translated_segment_offset",
            "measurement_signed_curvature",
            "measurement_unsigned_curvature",
            "measurement_osculating_radius",
            "measurement_conic_major_axis_length",
            "measurement_conic_minor_axis_length",
            "measurement_conic_linear_eccentricity",
            "measurement_conic_focal_distance",
            "measurement_conic_transverse_axis_length",
            "measurement_conic_conjugate_axis_length",
            "dimension_relative_horizontal",
            "dimension_relative_vertical",
            "dimension_datum_coordinate",
            "dimension_point_line_distance",
            "dimension_parallel_line_separation",
            "dimension_two_line_angle",
            "dimension_three_point_angle",
            "dimension_circular_sweep",
            "dimension_circular_arc_length",
            "dimension_ellipse_major_axis",
            "dimension_ellipse_minor_axis",
            "dimension_conic_linear_eccentricity",
            "dimension_conic_focal_distance",
            "dimension_conic_transverse_axis_length",
            "dimension_conic_conjugate_axis_length",
            "measurement_persistent_signed_curvature",
            "measurement_persistent_unsigned_curvature",
            "measurement_persistent_osculating_radius",
            "measurement_bounded_curve_length",
            "dimension_path_length",
            "dimension_segment_length",
        ],
    );
    assert_eq!(dimensions.rows.len(), 37);
    for row in &dimensions.rows {
        assert!(matches!(
            row[dimensions.column("kind")],
            "dimension" | "measurement"
        ));
        assert_ne!(row[dimensions.column("unit")], "");
        assert_ne!(row[dimensions.column("sign")], "");
        assert_ne!(row[dimensions.column("branch_state")], "");
        assert_ne!(row[dimensions.column("row_emission")], "");
    }

    let units = table("scalar_units");
    assert_header(
        &units,
        &[
            "id",
            "public_variant_or_target",
            "quantity",
            "sign_policy",
            "branch_state",
            "row_emission",
            "status",
            "target",
            "reason",
        ],
    );
    assert_ids(
        &units,
        &[
            "length",
            "angle",
            "parameter",
            "dimensionless",
            "curvature",
            "signed_length_semantics",
        ],
    );
    assert_eq!(units.rows.len(), 6);

    let domains = table("scalar_domains");
    assert_header(
        &domains,
        &[
            "id",
            "public_variant",
            "bounds",
            "sign_policy",
            "branch_state",
            "row_emission",
            "status",
            "target",
            "reason",
        ],
    );
    assert_ids(&domains, &["finite", "positive", "bounded", "periodic"]);
    assert_eq!(domains.rows.len(), 4);

    let unsupported = table("unsupported_combinations");
    assert_header(
        &unsupported,
        &["id", "capability", "operands", "status", "target", "reason"],
    );
    assert_ids(
        &unsupported,
        &[
            "periodic_curve_endpoint",
            "center_without_semantic_center",
            "focus_without_semantic_focus",
            "axis_without_semantic_axis",
            "spline_control_feature_current_gap",
            "public_curve_plugin",
            "implicit_coefficient_conic",
            "spatial_sketch_curve",
            "contact_invalid_domain",
            "tangency_zero_speed",
            "curvature_insufficient_regularity",
            "c2_insufficient_regularity",
            "fillet_parallel_parents",
            "fillet_singular_offset",
            "radius_non_circular",
            "diameter_non_circular",
            "equal_radius_non_circular",
            "concentric_without_centers",
            "collinear_non_linear",
            "horizontal_whole_non_linear",
            "vertical_whole_non_linear",
            "parallel_whole_non_linear",
            "perpendicular_whole_non_linear",
            "midpoint_non_linear",
            "symmetry_axis_non_linear",
            "equal_length_non_linear_current_gap",
            "curve_length_non_linear_current_gap",
            "generic_curve_angle",
            "arbitrary_curve_offset",
            "rational_conic_property_dimension",
            "driving_curvature",
            "path_length_unbounded",
            "path_length_invalid_derivative",
            "path_length_work_exhausted",
            "arbitrary_multi_fragment_trim",
            "solid_or_brep_operand",
        ],
    );
    assert_eq!(unsupported.rows.len(), 36);

    for contract in [
        &curves,
        &features,
        &relations,
        &dimensions,
        &units,
        &domains,
        &unsupported,
    ] {
        assert_status_contract(contract);
    }
}

fn curve_family(definition: &CurveDefinition) -> &'static str {
    match definition {
        CurveDefinition::Line { .. } => "line",
        CurveDefinition::Polyline { .. } => "polyline",
        CurveDefinition::Circle { .. } => "circle",
        CurveDefinition::CircularArc { .. } => "circular_arc",
        CurveDefinition::QuadraticBezier { .. } => "quadratic_bezier",
        CurveDefinition::CubicBezier { .. } => "cubic_bezier",
        CurveDefinition::Ellipse { .. } => "ellipse",
        CurveDefinition::EllipticalArc { .. } => "elliptical_arc",
        CurveDefinition::RationalQuadraticConic { .. } => "rational_quadratic_conic",
        CurveDefinition::ParabolaSegment { .. } => "parabola",
        CurveDefinition::HyperbolaSegment { .. } => "hyperbola",
        CurveDefinition::BSpline { form, .. } => match form {
            DocumentBSplineForm::Clamped => "clamped_b_spline",
            DocumentBSplineForm::Periodic => "periodic_b_spline",
        },
        CurveDefinition::Nurbs { form, .. } => match form {
            DocumentBSplineForm::Clamped => "clamped_nurbs",
            DocumentBSplineForm::Periodic => "periodic_nurbs",
        },
    }
}

fn feature_kind(feature: &FeatureRef) -> &'static str {
    match feature {
        FeatureRef::Point { .. } => "point",
        FeatureRef::CurveEndpoint { .. } => "curve_endpoint",
        FeatureRef::CurveCenter { .. } => "curve_center",
        FeatureRef::CurveAxis { .. } => "curve_axis",
        FeatureRef::CurveControl { .. } => "curve_control",
        FeatureRef::CurveFocus { .. } => "curve_focus",
        FeatureRef::FixedCurveLocation { .. } => "fixed_curve_location",
    }
}

const fn conic_feature_kind(feature: DocumentConicFeature) -> &'static str {
    match feature {
        DocumentConicFeature::Center => "conic_query_center",
        DocumentConicFeature::Focus { .. } => "conic_query_focus",
        DocumentConicFeature::MajorAxisEndpoint { .. } => "conic_major_axis_endpoint",
        DocumentConicFeature::MinorAxisEndpoint { .. } => "conic_minor_axis_endpoint",
        DocumentConicFeature::BoundedEndpoint { .. } => "conic_bounded_endpoint",
        DocumentConicFeature::SelectedBranchVertex => "conic_selected_branch_vertex",
    }
}

fn relation_kind(definition: &DocumentConstraintDefinition) -> &'static str {
    match definition {
        DocumentConstraintDefinition::FixedPoint { .. } => "fixed_point",
        DocumentConstraintDefinition::FixedCoordinate { .. } => "fixed_coordinate",
        DocumentConstraintDefinition::Coincident { .. } => "coincident",
        DocumentConstraintDefinition::Horizontal { .. } => "horizontal_line",
        DocumentConstraintDefinition::Vertical { .. } => "vertical_line",
        DocumentConstraintDefinition::HorizontalPoints { .. } => "horizontal_points",
        DocumentConstraintDefinition::VerticalPoints { .. } => "vertical_points",
        DocumentConstraintDefinition::HorizontalPointToMidpoint { .. } => {
            "horizontal_point_to_midpoint"
        }
        DocumentConstraintDefinition::VerticalPointToMidpoint { .. } => {
            "vertical_point_to_midpoint"
        }
        DocumentConstraintDefinition::PointOnCurve { .. } => "point_on_curve",
        DocumentConstraintDefinition::Parallel { .. } => "parallel_lines",
        DocumentConstraintDefinition::Perpendicular { .. } => "perpendicular_lines",
        DocumentConstraintDefinition::EqualLength { .. } => "equal_segment_length",
        DocumentConstraintDefinition::EqualRadius { .. } => "equal_circle_radius",
        DocumentConstraintDefinition::Midpoint { .. } => "midpoint_on_line",
        DocumentConstraintDefinition::SymmetricAboutLine { .. } => "symmetric_points_about_line",
        DocumentConstraintDefinition::LineCircleTangency { .. } => "line_circle_tangency",
        DocumentConstraintDefinition::CircleCircleTangency { .. } => "circle_circle_tangency",
        DocumentConstraintDefinition::CircleArcTangency { .. } => "circle_arc_tangency",
        DocumentConstraintDefinition::LineCurveTangency { .. } => "line_curve_tangency",
        DocumentConstraintDefinition::CurveCurveContact { .. } => "curve_curve_contact",
        DocumentConstraintDefinition::CurveCurveTangency { .. } => "curve_curve_tangency",
        DocumentConstraintDefinition::CurveDirection { .. } => "curve_direction",
        DocumentConstraintDefinition::EqualCurvature { .. } => "equal_curvature",
        DocumentConstraintDefinition::EndpointContinuity { .. } => "endpoint_continuity",
        DocumentConstraintDefinition::LineLineFillet { .. } => "line_line_fillet",
        DocumentConstraintDefinition::CurveCurveFillet { .. } => "curve_curve_fillet",
        DocumentConstraintDefinition::ExternalPointCoincident { .. } => "external_point_coincident",
        DocumentConstraintDefinition::ExternalLineCollinear { .. } => "external_line_collinear",
        DocumentConstraintDefinition::Concentric { .. } => "concentric",
        DocumentConstraintDefinition::Collinear { .. } => "collinear",
    }
}

fn dimension_kind(definition: &DocumentDimensionDefinition) -> &'static str {
    match definition {
        DocumentDimensionDefinition::PointDistance { .. } => "dimension_point_distance",
        DocumentDimensionDefinition::CurveLength { .. } => "dimension_curve_length",
        DocumentDimensionDefinition::Radius { .. } => "dimension_radius",
        DocumentDimensionDefinition::Diameter { .. } => "dimension_diameter",
        DocumentDimensionDefinition::OrientedAngle { .. } => "dimension_oriented_angle",
        DocumentDimensionDefinition::SupportingLineOffset { .. } => {
            "dimension_supporting_line_offset"
        }
        DocumentDimensionDefinition::ExactTranslatedSegmentOffset { .. } => {
            "dimension_exact_translated_segment_offset"
        }
    }
}

const fn curve_measurement_kind(kind: DocumentCurveMeasurementKind) -> &'static str {
    match kind {
        DocumentCurveMeasurementKind::SignedCurvature => "measurement_signed_curvature",
        DocumentCurveMeasurementKind::UnsignedCurvature => "measurement_unsigned_curvature",
        DocumentCurveMeasurementKind::OsculatingRadius => "measurement_osculating_radius",
    }
}

const fn conic_measurement_kind(kind: DocumentConicMeasurement) -> &'static str {
    match kind {
        DocumentConicMeasurement::MajorAxisLength => "measurement_conic_major_axis_length",
        DocumentConicMeasurement::MinorAxisLength => "measurement_conic_minor_axis_length",
        DocumentConicMeasurement::LinearEccentricity => "measurement_conic_linear_eccentricity",
        DocumentConicMeasurement::FocalDistance => "measurement_conic_focal_distance",
        DocumentConicMeasurement::TransverseAxisLength => {
            "measurement_conic_transverse_axis_length"
        }
        DocumentConicMeasurement::ConjugateAxisLength => "measurement_conic_conjugate_axis_length",
    }
}

const fn scalar_unit_kind(unit: ScalarUnit) -> &'static str {
    match unit {
        ScalarUnit::Length => "length",
        ScalarUnit::Angle => "angle",
        ScalarUnit::Parameter => "parameter",
    }
}

fn scalar_domain_kind(domain: &ScalarDomain) -> &'static str {
    match domain {
        ScalarDomain::Finite => "finite",
        ScalarDomain::Positive => "positive",
        ScalarDomain::Bounded { .. } => "bounded",
        ScalarDomain::Periodic { .. } => "periodic",
    }
}

#[test]
fn current_public_enums_are_exhaustively_joined_to_the_matrix_and_v4_stays_frozen() {
    assert_eq!(SKETCH_DOCUMENT_VERSION, 4);

    let point = DesignPointId(PersistentId::from_u128(1));
    let second_point = DesignPointId(PersistentId::from_u128(2));
    let third_point = DesignPointId(PersistentId::from_u128(3));
    let fourth_point = DesignPointId(PersistentId::from_u128(4));
    let scalar = DesignScalarId(PersistentId::from_u128(5));
    let second_scalar = DesignScalarId(PersistentId::from_u128(6));
    let curve = CurveId(PersistentId::from_u128(7));
    let second_curve = CurveId(PersistentId::from_u128(8));
    let contact = ContactId(PersistentId::from_u128(9));
    let second_contact = ContactId(PersistentId::from_u128(10));
    let span = CurveSpan::line(curve);
    let second_span = CurveSpan::line(second_curve);

    let spline = |form| CurveDefinition::BSpline {
        form,
        degree: 1,
        controls: vec![point, second_point],
        knots: vec![0.0, 0.0, 1.0, 1.0],
        span_ids: vec![0],
        next_span_id: 1,
    };
    let nurbs = |form| CurveDefinition::Nurbs {
        form,
        degree: 1,
        controls: vec![point, second_point],
        weights: vec![scalar, second_scalar],
        gauge_weight: scalar,
        knots: vec![0.0, 0.0, 1.0, 1.0],
        span_ids: vec![0],
        next_span_id: 1,
    };
    let curves = vec![
        CurveDefinition::Line {
            start: point,
            end: second_point,
            branch_direction: [1.0, 0.0],
        },
        CurveDefinition::Polyline {
            points: vec![point, second_point],
            closed: false,
            branch_directions: vec![[1.0, 0.0]],
        },
        CurveDefinition::Circle {
            center: point,
            radius: scalar,
        },
        CurveDefinition::CircularArc {
            center: point,
            radius: scalar,
            start_angle: scalar,
            end_angle: second_scalar,
            sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
        },
        CurveDefinition::Ellipse {
            center: point,
            major_axis_point: second_point,
            minor_axis_ratio: scalar,
        },
        CurveDefinition::EllipticalArc {
            center: point,
            major_axis_point: second_point,
            minor_axis_ratio: scalar,
            start_angle: scalar,
            end_angle: second_scalar,
            sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
        },
        CurveDefinition::RationalQuadraticConic {
            start: point,
            weighted_middle: [1.0, 1.0],
            middle_weight: scalar,
            end: second_point,
        },
        CurveDefinition::ParabolaSegment {
            vertex: point,
            focus: second_point,
            trim_start: scalar,
            trim_end: second_scalar,
        },
        CurveDefinition::HyperbolaSegment {
            center: point,
            transverse_axis_point: second_point,
            semi_conjugate: scalar,
            branch: DocumentHyperbolaBranch::Positive,
            trim_start: scalar,
            trim_end: second_scalar,
        },
        CurveDefinition::QuadraticBezier {
            controls: [point, second_point, third_point],
        },
        CurveDefinition::CubicBezier {
            controls: [point, second_point, third_point, fourth_point],
        },
        spline(DocumentBSplineForm::Clamped),
        spline(DocumentBSplineForm::Periodic),
        nurbs(DocumentBSplineForm::Clamped),
        nurbs(DocumentBSplineForm::Periodic),
    ];
    assert_eq!(
        curves.iter().map(curve_family).collect::<Vec<_>>(),
        table("curve_families").ids()
    );

    let features = [
        FeatureRef::Point { point },
        FeatureRef::CurveEndpoint {
            curve,
            endpoint: FeatureEndpoint::Start,
        },
        FeatureRef::CurveCenter { curve },
        FeatureRef::CurveAxis { curve },
        FeatureRef::CurveControl { curve, index: 0 },
        FeatureRef::CurveFocus { curve, index: 0 },
        FeatureRef::FixedCurveLocation { contact },
    ];
    let implemented_features = table("feature_kinds")
        .rows
        .into_iter()
        .filter(|row| row[4] == "implemented_m32")
        .map(|row| row[0])
        .collect::<Vec<_>>();
    assert_eq!(
        features.iter().map(feature_kind).collect::<Vec<_>>(),
        &implemented_features[..features.len()]
    );

    let conic_features = [
        DocumentConicFeature::Center,
        DocumentConicFeature::Focus { index: 0 },
        DocumentConicFeature::MajorAxisEndpoint {
            endpoint: FeatureEndpoint::Start,
        },
        DocumentConicFeature::MinorAxisEndpoint {
            endpoint: FeatureEndpoint::Start,
        },
        DocumentConicFeature::BoundedEndpoint {
            endpoint: FeatureEndpoint::Start,
        },
        DocumentConicFeature::SelectedBranchVertex,
    ];
    assert_eq!(
        conic_features
            .into_iter()
            .map(conic_feature_kind)
            .collect::<Vec<_>>(),
        implemented_features[features.len()..]
    );

    let relations = vec![
        DocumentConstraintDefinition::FixedPoint {
            point,
            target: [0.0, 0.0],
        },
        DocumentConstraintDefinition::FixedCoordinate {
            point,
            axis: DocumentCoordinateAxis::X,
            target: 0.0,
        },
        DocumentConstraintDefinition::Coincident {
            first: point,
            second: second_point,
        },
        DocumentConstraintDefinition::Horizontal { line: span },
        DocumentConstraintDefinition::Vertical { line: span },
        DocumentConstraintDefinition::PointOnCurve { point, contact },
        DocumentConstraintDefinition::Parallel {
            first: span,
            second: second_span,
        },
        DocumentConstraintDefinition::Perpendicular {
            first: span,
            second: second_span,
        },
        DocumentConstraintDefinition::EqualLength {
            first: span,
            second: second_span,
        },
        DocumentConstraintDefinition::EqualRadius {
            first: curve,
            second: second_curve,
        },
        DocumentConstraintDefinition::Midpoint { point, line: span },
        DocumentConstraintDefinition::SymmetricAboutLine {
            first: point,
            second: second_point,
            line: span,
        },
        DocumentConstraintDefinition::LineCircleTangency {
            line_contact: contact,
            circle_contact: second_contact,
            side: DocumentLineSide::Left,
        },
        DocumentConstraintDefinition::CircleCircleTangency {
            first: curve,
            second: second_curve,
            mode: DocumentCircleTangencyMode::Internal {
                containment: DocumentCircleContainment::FirstContainsSecond,
            },
            center_direction: [1.0, 0.0],
        },
        DocumentConstraintDefinition::CircleArcTangency {
            circle_contact: contact,
            arc_contact: second_contact,
            side: DocumentArcTangencySide::OutsideArc,
        },
        DocumentConstraintDefinition::LineCurveTangency {
            line: span,
            endpoint: FeatureEndpoint::Start,
            curve_contact: contact,
        },
        DocumentConstraintDefinition::CurveCurveContact {
            first_contact: contact,
            second_contact,
        },
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: contact,
            second_contact,
        },
        DocumentConstraintDefinition::CurveDirection {
            line: span,
            curve_contact: contact,
            relation: DocumentCurveDirectionRelation::Tangent {
                orientation: TangentOrientation::Aligned,
            },
        },
        DocumentConstraintDefinition::EqualCurvature {
            first_contact: contact,
            second_contact,
            relation: DocumentCurveCurvatureRelation::Signed,
        },
        DocumentConstraintDefinition::EndpointContinuity {
            first_contact: contact,
            second_contact,
            continuity: DocumentCurveContinuity::G0,
        },
        DocumentConstraintDefinition::LineLineFillet {
            arc: curve,
            first_contact: contact,
            first_side: DocumentCurveNormalSide::Left,
            second_contact,
            second_side: DocumentCurveNormalSide::Right,
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        },
        DocumentConstraintDefinition::CurveCurveFillet {
            arc: curve,
            first_contact: contact,
            first_side: DocumentCurveNormalSide::Left,
            first_trim_endpoint: DocumentFilletTrimEndpoint::End,
            second_contact,
            second_side: DocumentCurveNormalSide::Right,
            second_trim_endpoint: DocumentFilletTrimEndpoint::Start,
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        },
    ];
    let implemented_relations = table("relations")
        .rows
        .into_iter()
        .filter(|row| row[7] == "implemented_m32")
        .map(|row| row[0])
        .collect::<Vec<_>>();
    assert_eq!(
        relations.iter().map(relation_kind).collect::<Vec<_>>(),
        implemented_relations
    );

    let dimensions = [
        DocumentDimensionDefinition::PointDistance {
            first: point,
            second: second_point,
            target: scalar,
        },
        DocumentDimensionDefinition::CurveLength {
            curve: span,
            target: scalar,
        },
        DocumentDimensionDefinition::Radius {
            curve,
            target: scalar,
        },
        DocumentDimensionDefinition::Diameter {
            curve,
            target: scalar,
        },
        DocumentDimensionDefinition::OrientedAngle {
            first: span,
            second: second_span,
            target: scalar,
            orientation: DocumentAngleOrientation::CounterClockwise,
        },
        DocumentDimensionDefinition::SupportingLineOffset {
            source: span,
            target_segment: second_span,
            target: scalar,
            side: DocumentLineSide::Left,
            orientation: DocumentLineOffsetOrientation::Same,
        },
        DocumentDimensionDefinition::ExactTranslatedSegmentOffset {
            source: span,
            target_segment: second_span,
            target: scalar,
            side: DocumentLineSide::Left,
            orientation: DocumentLineOffsetOrientation::Same,
        },
    ];
    let matrix_dimensions = table("dimensions_measurements");
    let implemented_dimensions = matrix_dimensions
        .rows
        .iter()
        .filter(|row| row[2] == "dimension" && row[8] == "implemented_m32")
        .map(|row| row[0])
        .collect::<Vec<_>>();
    assert_eq!(
        dimensions.iter().map(dimension_kind).collect::<Vec<_>>(),
        implemented_dimensions
    );

    let curve_measurements = [
        DocumentCurveMeasurementKind::SignedCurvature,
        DocumentCurveMeasurementKind::UnsignedCurvature,
        DocumentCurveMeasurementKind::OsculatingRadius,
    ];
    let conic_measurements = [
        DocumentConicMeasurement::MajorAxisLength,
        DocumentConicMeasurement::MinorAxisLength,
        DocumentConicMeasurement::LinearEccentricity,
        DocumentConicMeasurement::FocalDistance,
        DocumentConicMeasurement::TransverseAxisLength,
        DocumentConicMeasurement::ConjugateAxisLength,
    ];
    let implemented_measurements = matrix_dimensions
        .rows
        .iter()
        .filter(|row| row[2] == "measurement" && row[8] == "implemented_m32")
        .map(|row| row[0])
        .collect::<Vec<_>>();
    let enum_measurements = curve_measurements
        .into_iter()
        .map(curve_measurement_kind)
        .chain(conic_measurements.into_iter().map(conic_measurement_kind))
        .collect::<Vec<_>>();
    assert_eq!(enum_measurements, implemented_measurements);

    let units = [ScalarUnit::Length, ScalarUnit::Angle, ScalarUnit::Parameter];
    let implemented_units = table("scalar_units")
        .rows
        .into_iter()
        .filter(|row| row[6] == "implemented_m32")
        .map(|row| row[0])
        .collect::<Vec<_>>();
    assert_eq!(
        units.into_iter().map(scalar_unit_kind).collect::<Vec<_>>(),
        implemented_units
    );

    let domains = [
        ScalarDomain::Finite,
        ScalarDomain::Positive,
        ScalarDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
        ScalarDomain::Periodic {
            period: std::f64::consts::TAU,
        },
    ];
    assert_eq!(
        domains.iter().map(scalar_domain_kind).collect::<Vec<_>>(),
        table("scalar_domains").ids()
    );
}
