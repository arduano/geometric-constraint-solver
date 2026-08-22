// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::{CurveNumericPropertyKind, GeometryToolVariant};
use geosolve_sketch::{DocumentConstraintDefinition, DocumentDimensionDefinition, GeometryRole};
use geosolve_sketch_lineage::{LineageActionKind, LineageOutputKind, LineageReservationKind};
use geosolve_sketch_ops::SketchOperationKind;

const GOLDEN: &str = include_str!("fixtures/m83_lineage_action_catalog.golden.tsv");

const CONSTRAINT_KEYS: [&str; 35] = [
    "fixed-point",
    "fixed-coordinate",
    "coincident-with-origin",
    "point-on-datum-axis",
    "coincident",
    "external-point-coincident",
    "horizontal",
    "vertical",
    "horizontal-points",
    "vertical-points",
    "horizontal-point-to-midpoint",
    "vertical-point-to-midpoint",
    "point-on-curve",
    "parallel",
    "perpendicular",
    "external-line-collinear",
    "collinear-with-datum-axis",
    "concentric",
    "collinear",
    "equal-length",
    "equal-radius",
    "midpoint",
    "symmetric-about-line",
    "symmetric-about-datum-axis",
    "line-circle-tangency",
    "circle-circle-tangency",
    "circle-arc-tangency",
    "line-curve-tangency",
    "curve-curve-contact",
    "curve-curve-tangency",
    "curve-direction",
    "equal-curvature",
    "endpoint-continuity",
    "line-line-fillet",
    "curve-curve-fillet",
];

const DIMENSION_KEYS: [&str; 8] = [
    "point-distance",
    "curve-length",
    "radius",
    "diameter",
    "oriented-angle",
    "supporting-line-offset",
    "exact-translated-segment-offset",
    "profile-offset",
];

const CURVE_CONTROL_KEYS: [&str; 14] = [
    "center",
    "start-point",
    "end-point",
    "control-point",
    "radius",
    "trim-start",
    "trim-end",
    "major-axis-point",
    "minor-axis",
    "rational-middle",
    "vertex",
    "focus",
    "transverse-axis-point",
    "conjugate-axis",
];

const CURVE_PROPERTY_KEYS: [&str; 7] = [
    "radius",
    "minor-axis-ratio",
    "trim-start",
    "trim-end",
    "semi-conjugate",
    "rational-weight",
    "nurbs-weight",
];

const BRANCH_FAMILY_KEYS: [&str; 27] = [
    "line-direction",
    "polyline-direction",
    "arc-sweep",
    "elliptical-arc-sweep",
    "hyperbola-branch",
    "contact-parameter",
    "contact-winding",
    "contact-neighborhood",
    "normal-side",
    "retained-endpoint",
    "periodic-anchor",
    "tangency-mode",
    "tangency-direction",
    "fillet-sides",
    "fillet-trim-endpoints",
    "fillet-endpoint-order",
    "fillet-sweep",
    "angle-orientation",
    "profile-offset-direction",
    "profile-offset-traversal",
    "profile-offset-junction",
    "profile-offset-terminal-policy",
    "spline-form",
    "spline-span-transition",
    "nurbs-gauge",
    "source-suppression",
    "element-activation",
];

fn constraint_key(definition: &DocumentConstraintDefinition) -> &'static str {
    match definition {
        DocumentConstraintDefinition::FixedPoint { .. } => "fixed-point",
        DocumentConstraintDefinition::FixedCoordinate { .. } => "fixed-coordinate",
        DocumentConstraintDefinition::CoincidentWithOrigin { .. } => "coincident-with-origin",
        DocumentConstraintDefinition::PointOnDatumAxis { .. } => "point-on-datum-axis",
        DocumentConstraintDefinition::Coincident { .. } => "coincident",
        DocumentConstraintDefinition::ExternalPointCoincident { .. } => "external-point-coincident",
        DocumentConstraintDefinition::Horizontal { .. } => "horizontal",
        DocumentConstraintDefinition::Vertical { .. } => "vertical",
        DocumentConstraintDefinition::HorizontalPoints { .. } => "horizontal-points",
        DocumentConstraintDefinition::VerticalPoints { .. } => "vertical-points",
        DocumentConstraintDefinition::HorizontalPointToMidpoint { .. } => {
            "horizontal-point-to-midpoint"
        }
        DocumentConstraintDefinition::VerticalPointToMidpoint { .. } => {
            "vertical-point-to-midpoint"
        }
        DocumentConstraintDefinition::PointOnCurve { .. } => "point-on-curve",
        DocumentConstraintDefinition::Parallel { .. } => "parallel",
        DocumentConstraintDefinition::Perpendicular { .. } => "perpendicular",
        DocumentConstraintDefinition::ExternalLineCollinear { .. } => "external-line-collinear",
        DocumentConstraintDefinition::CollinearWithDatumAxis { .. } => "collinear-with-datum-axis",
        DocumentConstraintDefinition::Concentric { .. } => "concentric",
        DocumentConstraintDefinition::Collinear { .. } => "collinear",
        DocumentConstraintDefinition::EqualLength { .. } => "equal-length",
        DocumentConstraintDefinition::EqualRadius { .. } => "equal-radius",
        DocumentConstraintDefinition::Midpoint { .. } => "midpoint",
        DocumentConstraintDefinition::SymmetricAboutLine { .. } => "symmetric-about-line",
        DocumentConstraintDefinition::SymmetricAboutDatumAxis { .. } => {
            "symmetric-about-datum-axis"
        }
        DocumentConstraintDefinition::LineCircleTangency { .. } => "line-circle-tangency",
        DocumentConstraintDefinition::CircleCircleTangency { .. } => "circle-circle-tangency",
        DocumentConstraintDefinition::CircleArcTangency { .. } => "circle-arc-tangency",
        DocumentConstraintDefinition::LineCurveTangency { .. } => "line-curve-tangency",
        DocumentConstraintDefinition::CurveCurveContact { .. } => "curve-curve-contact",
        DocumentConstraintDefinition::CurveCurveTangency { .. } => "curve-curve-tangency",
        DocumentConstraintDefinition::CurveDirection { .. } => "curve-direction",
        DocumentConstraintDefinition::EqualCurvature { .. } => "equal-curvature",
        DocumentConstraintDefinition::EndpointContinuity { .. } => "endpoint-continuity",
        DocumentConstraintDefinition::LineLineFillet { .. } => "line-line-fillet",
        DocumentConstraintDefinition::CurveCurveFillet { .. } => "curve-curve-fillet",
    }
}

fn dimension_key(definition: &DocumentDimensionDefinition) -> &'static str {
    match definition {
        DocumentDimensionDefinition::PointDistance { .. } => "point-distance",
        DocumentDimensionDefinition::CurveLength { .. } => "curve-length",
        DocumentDimensionDefinition::Radius { .. } => "radius",
        DocumentDimensionDefinition::Diameter { .. } => "diameter",
        DocumentDimensionDefinition::OrientedAngle { .. } => "oriented-angle",
        DocumentDimensionDefinition::SupportingLineOffset { .. } => "supporting-line-offset",
        DocumentDimensionDefinition::ExactTranslatedSegmentOffset { .. } => {
            "exact-translated-segment-offset"
        }
        DocumentDimensionDefinition::ProfileOffset { .. } => "profile-offset",
    }
}

const fn operation_key(kind: SketchOperationKind) -> &'static str {
    match kind {
        SketchOperationKind::Split => "split",
        SketchOperationKind::Break => "break",
        SketchOperationKind::Trim => "trim",
        SketchOperationKind::Extend => "extend",
        SketchOperationKind::Mirror => "mirror",
        SketchOperationKind::Chamfer => "chamfer",
        SketchOperationKind::AssociativeFillet => "associative-fillet",
        SketchOperationKind::Rectangle => "rectangle",
        SketchOperationKind::RegularPolygon => "regular-polygon",
        SketchOperationKind::Slot => "slot",
        SketchOperationKind::LinearPattern => "linear-pattern",
        SketchOperationKind::ProfileOffset => "profile-offset",
    }
}

const fn curve_property_key(kind: CurveNumericPropertyKind) -> &'static str {
    match kind {
        CurveNumericPropertyKind::Radius => "radius",
        CurveNumericPropertyKind::MinorAxisRatio => "minor-axis-ratio",
        CurveNumericPropertyKind::TrimStart => "trim-start",
        CurveNumericPropertyKind::TrimEnd => "trim-end",
        CurveNumericPropertyKind::SemiConjugate => "semi-conjugate",
        CurveNumericPropertyKind::RationalWeight => "rational-weight",
        CurveNumericPropertyKind::NurbsWeight { .. } => "nurbs-weight",
    }
}

const fn role_key(role: GeometryRole) -> &'static str {
    match role {
        GeometryRole::Profile => "profile",
        GeometryRole::Construction => "construction",
    }
}

const fn action_kind_key(kind: LineageActionKind) -> &'static str {
    match kind {
        LineageActionKind::ImportedBaseline => "imported-baseline",
        LineageActionKind::GeometryRecipe => "geometry-recipe",
        LineageActionKind::Constraint => "constraint",
        LineageActionKind::Dimension => "dimension",
        LineageActionKind::Trim => "trim",
        LineageActionKind::Parameter => "parameter",
        LineageActionKind::Binding => "binding",
        LineageActionKind::External => "external",
        LineageActionKind::Operation => "operation",
        LineageActionKind::ComputedFeature => "computed-feature",
        LineageActionKind::Annotation => "annotation",
    }
}

const fn output_kind_key(kind: LineageOutputKind) -> &'static str {
    match kind {
        LineageOutputKind::Point => "point",
        LineageOutputKind::Scalar => "scalar",
        LineageOutputKind::Curve => "curve",
        LineageOutputKind::CurveSpan => "curve-span",
        LineageOutputKind::TrimView => "trim-view",
        LineageOutputKind::Contact => "contact",
        LineageOutputKind::Constraint => "constraint",
        LineageOutputKind::Dimension => "dimension",
        LineageOutputKind::Source => "source",
        LineageOutputKind::Parameter => "parameter",
        LineageOutputKind::ParameterBinding => "parameter-binding",
        LineageOutputKind::ParameterOutput => "parameter-output",
        LineageOutputKind::ExternalBinding => "external-binding",
        LineageOutputKind::GeometryRole => "geometry-role",
        LineageOutputKind::Activation => "activation",
        LineageOutputKind::Profile => "profile",
        LineageOutputKind::Chain => "chain",
        LineageOutputKind::Operation => "operation",
        LineageOutputKind::Feature => "feature",
        LineageOutputKind::FeatureCorner => "feature-corner",
        LineageOutputKind::Annotation => "annotation",
        LineageOutputKind::Collection => "collection",
    }
}

const fn reservation_kind_key(kind: LineageReservationKind) -> &'static str {
    match kind {
        LineageReservationKind::Point => "point",
        LineageReservationKind::Scalar => "scalar",
        LineageReservationKind::Curve => "curve",
        LineageReservationKind::CurveSpan => "curve-span",
        LineageReservationKind::TrimView => "trim-view",
        LineageReservationKind::Contact => "contact",
        LineageReservationKind::Constraint => "constraint",
        LineageReservationKind::Dimension => "dimension",
        LineageReservationKind::Source => "source",
        LineageReservationKind::Parameter => "parameter",
        LineageReservationKind::ParameterBinding => "parameter-binding",
        LineageReservationKind::ParameterOutput => "parameter-output",
        LineageReservationKind::ExternalBinding => "external-binding",
        LineageReservationKind::Profile => "profile",
        LineageReservationKind::Chain => "chain",
        LineageReservationKind::Operation => "operation",
        LineageReservationKind::Feature => "feature",
        LineageReservationKind::FeatureCorner => "feature-corner",
        LineageReservationKind::Annotation => "annotation",
        LineageReservationKind::HiddenSupport => "hidden-support",
    }
}

fn push_rows(output: &mut String, category: &str, keys: impl IntoIterator<Item = &'static str>) {
    for key in keys {
        output.push_str(category);
        output.push('\t');
        output.push_str(key);
        output.push('\n');
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the byte-frozen catalog remains visibly grouped by its closed semantic families"
)]
fn catalog() -> String {
    let mut output = String::from("category\tkey\n");
    push_rows(
        &mut output,
        "action-kind",
        [
            LineageActionKind::ImportedBaseline,
            LineageActionKind::GeometryRecipe,
            LineageActionKind::Constraint,
            LineageActionKind::Dimension,
            LineageActionKind::Trim,
            LineageActionKind::Parameter,
            LineageActionKind::Binding,
            LineageActionKind::External,
            LineageActionKind::Operation,
            LineageActionKind::ComputedFeature,
            LineageActionKind::Annotation,
        ]
        .map(action_kind_key),
    );
    push_rows(
        &mut output,
        "output-kind",
        [
            LineageOutputKind::Point,
            LineageOutputKind::Scalar,
            LineageOutputKind::Curve,
            LineageOutputKind::CurveSpan,
            LineageOutputKind::TrimView,
            LineageOutputKind::Contact,
            LineageOutputKind::Constraint,
            LineageOutputKind::Dimension,
            LineageOutputKind::Source,
            LineageOutputKind::Parameter,
            LineageOutputKind::ParameterBinding,
            LineageOutputKind::ParameterOutput,
            LineageOutputKind::ExternalBinding,
            LineageOutputKind::GeometryRole,
            LineageOutputKind::Activation,
            LineageOutputKind::Profile,
            LineageOutputKind::Chain,
            LineageOutputKind::Operation,
            LineageOutputKind::Feature,
            LineageOutputKind::FeatureCorner,
            LineageOutputKind::Annotation,
            LineageOutputKind::Collection,
        ]
        .map(output_kind_key),
    );
    push_rows(
        &mut output,
        "reservation-kind",
        [
            LineageReservationKind::Point,
            LineageReservationKind::Scalar,
            LineageReservationKind::Curve,
            LineageReservationKind::CurveSpan,
            LineageReservationKind::TrimView,
            LineageReservationKind::Contact,
            LineageReservationKind::Constraint,
            LineageReservationKind::Dimension,
            LineageReservationKind::Source,
            LineageReservationKind::Parameter,
            LineageReservationKind::ParameterBinding,
            LineageReservationKind::ParameterOutput,
            LineageReservationKind::ExternalBinding,
            LineageReservationKind::Profile,
            LineageReservationKind::Chain,
            LineageReservationKind::Operation,
            LineageReservationKind::Feature,
            LineageReservationKind::FeatureCorner,
            LineageReservationKind::Annotation,
            LineageReservationKind::HiddenSupport,
        ]
        .map(reservation_kind_key),
    );
    push_rows(
        &mut output,
        "geometry",
        GeometryToolVariant::ALL.map(GeometryToolVariant::key),
    );
    push_rows(&mut output, "constraint", CONSTRAINT_KEYS);
    push_rows(&mut output, "dimension", DIMENSION_KEYS);
    push_rows(&mut output, "curve-control", CURVE_CONTROL_KEYS);
    push_rows(&mut output, "curve-property", CURVE_PROPERTY_KEYS);
    push_rows(
        &mut output,
        "geometry-role",
        [GeometryRole::Profile, GeometryRole::Construction].map(role_key),
    );
    push_rows(
        &mut output,
        "operation",
        [
            SketchOperationKind::Split,
            SketchOperationKind::Break,
            SketchOperationKind::Trim,
            SketchOperationKind::Extend,
            SketchOperationKind::Mirror,
            SketchOperationKind::Chamfer,
            SketchOperationKind::AssociativeFillet,
            SketchOperationKind::Rectangle,
            SketchOperationKind::RegularPolygon,
            SketchOperationKind::Slot,
            SketchOperationKind::LinearPattern,
            SketchOperationKind::ProfileOffset,
        ]
        .map(operation_key),
    );
    push_rows(&mut output, "native-publication", ["native-line-fillet"]);
    push_rows(&mut output, "computed-feature", ["fillet-set"]);
    push_rows(&mut output, "trim", ["curve-trim-view"]);
    push_rows(&mut output, "parameter", ["host-parameter"]);
    push_rows(
        &mut output,
        "binding",
        [
            "parameter-binding",
            "parameter-output",
            "host-activation",
            "geometry-role",
        ],
    );
    push_rows(
        &mut output,
        "external",
        ["external-binding", "external-snapshot"],
    );
    push_rows(&mut output, "annotation", ["annotation-layout"]);
    push_rows(&mut output, "branch", BRANCH_FAMILY_KEYS);
    output
}

#[test]
fn m83_lineage_catalog_is_exhaustive_and_byte_frozen_separately() {
    let _: fn(&DocumentConstraintDefinition) -> &'static str = constraint_key;
    let _: fn(&DocumentDimensionDefinition) -> &'static str = dimension_key;
    let _: fn(CurveNumericPropertyKind) -> &'static str = curve_property_key;

    let actual = catalog();
    if std::env::var_os("GEOSOLVE_SURVEY_M83_LINEAGE_CATALOG").is_some() {
        print!("{actual}");
        return;
    }
    assert_eq!(actual, GOLDEN);
    let mut seen = BTreeSet::new();
    for row in actual.lines().skip(1) {
        assert!(
            seen.insert(row),
            "duplicate M83 lineage catalog row `{row}`"
        );
    }
    assert_eq!(GeometryToolVariant::ALL.len(), 25);
    assert_eq!(CONSTRAINT_KEYS.len(), 35);
    assert_eq!(DIMENSION_KEYS.len(), 8);
}
