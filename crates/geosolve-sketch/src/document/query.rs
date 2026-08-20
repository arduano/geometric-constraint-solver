use super::{
    CurveDefinition, CurveId, DesignPointId, DesignScalar, DesignScalarId, DocumentConicFeature,
    DocumentConicQueryError, DocumentCurveControl, DocumentCurveControlAvailability,
    DocumentCurveControlError, DocumentCurveControlId, DocumentCurveControlKind,
    DocumentCurveControlTarget, DocumentCurveEvaluationError, DocumentError,
    DocumentHyperbolaBranch, DocumentTrimProjectionError, FeatureEndpoint, ScalarDomain,
    ScalarUnit, SketchDocument, require_scalar_role,
};

pub(super) fn push_curve_control(
    controls: &mut Vec<DocumentCurveControl>,
    curve: CurveId,
    kind: DocumentCurveControlKind,
    position: [f64; 2],
    target: DocumentCurveControlTarget,
    availability: DocumentCurveControlAvailability,
) -> Result<(), DocumentCurveControlError> {
    let id = DocumentCurveControlId { curve, kind };
    if !position.iter().all(|value| value.is_finite()) {
        return Err(DocumentCurveControlError::NonFiniteResult { control: id });
    }
    controls.push(DocumentCurveControl {
        id,
        position,
        target,
        availability,
    });
    Ok(())
}

pub(super) fn push_trim_controls(
    document: &SketchDocument,
    controls: &mut Vec<DocumentCurveControl>,
    curve: CurveId,
    start_position: [f64; 2],
    end_position: [f64; 2],
    definition: &CurveDefinition,
    availability: DocumentCurveControlAvailability,
) -> Result<(), DocumentCurveControlError> {
    let (start, end) = match definition {
        CurveDefinition::CircularArc {
            start_angle,
            end_angle,
            ..
        }
        | CurveDefinition::EllipticalArc {
            start_angle,
            end_angle,
            ..
        } => (*start_angle, *end_angle),
        CurveDefinition::ParabolaSegment {
            trim_start,
            trim_end,
            ..
        }
        | CurveDefinition::HyperbolaSegment {
            trim_start,
            trim_end,
            ..
        } => (*trim_start, *trim_end),
        _ => {
            return Err(DocumentCurveControlError::UnknownControl {
                curve,
                kind: DocumentCurveControlKind::TrimStart,
            });
        }
    };
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::TrimStart,
        start_position,
        DocumentCurveControlTarget::Scalar(start),
        document.scalar_control_availability(start, availability),
    )?;
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::TrimEnd,
        end_position,
        DocumentCurveControlTarget::Scalar(end),
        document.scalar_control_availability(end, availability),
    )
}

pub(super) fn push_axis_controls(
    document: &SketchDocument,
    controls: &mut Vec<DocumentCurveControl>,
    curve: CurveId,
    center: DesignPointId,
    major_axis_point: DesignPointId,
    minor_axis_ratio: DesignScalarId,
    availability: DocumentCurveControlAvailability,
) -> Result<(), DocumentCurveControlError> {
    let definition = &document
        .curve(curve)
        .ok_or(DocumentCurveControlError::UnknownControl {
            curve,
            kind: DocumentCurveControlKind::MinorAxis,
        })?
        .definition;
    // Elliptical-arc trims may occupy either signed minor pole. Put the size
    // grip on the pole whose nearest trim endpoint is farther away, keeping the
    // ordinary positive pole for full ellipses and deterministic arc ties.
    let minor_axis_endpoint = if matches!(definition, CurveDefinition::EllipticalArc { .. }) {
        let start = document.evaluate_conic_feature(
            curve,
            DocumentConicFeature::BoundedEndpoint {
                endpoint: FeatureEndpoint::Start,
            },
        )?;
        let end = document.evaluate_conic_feature(
            curve,
            DocumentConicFeature::BoundedEndpoint {
                endpoint: FeatureEndpoint::End,
            },
        )?;
        let separation = |endpoint| -> Result<f64, DocumentCurveControlError> {
            let position = document.evaluate_conic_feature(
                curve,
                DocumentConicFeature::MinorAxisEndpoint { endpoint },
            )?;
            let distance = |trim: [f64; 2]| (position[0] - trim[0]).hypot(position[1] - trim[1]);
            Ok(distance(start).min(distance(end)))
        };
        let negative = separation(FeatureEndpoint::Start)?;
        let positive = separation(FeatureEndpoint::End)?;
        if negative > positive {
            FeatureEndpoint::Start
        } else {
            FeatureEndpoint::End
        }
    } else {
        FeatureEndpoint::End
    };
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::Center,
        document.require_point(center)?.position,
        DocumentCurveControlTarget::Point(center),
        availability,
    )?;
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::MajorAxisPoint,
        document.require_point(major_axis_point)?.position,
        DocumentCurveControlTarget::Point(major_axis_point),
        availability,
    )?;
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::MinorAxis,
        document.evaluate_conic_feature(
            curve,
            DocumentConicFeature::MinorAxisEndpoint {
                endpoint: minor_axis_endpoint,
            },
        )?,
        DocumentCurveControlTarget::Scalar(minor_axis_ratio),
        document.scalar_control_availability(minor_axis_ratio, availability),
    )
}

pub(super) fn curve_owned_scalars(definition: &CurveDefinition) -> Vec<DesignScalarId> {
    match definition {
        CurveDefinition::Circle { radius, .. } => vec![*radius],
        CurveDefinition::CircularArc {
            radius,
            start_angle,
            end_angle,
            ..
        } => vec![*radius, *start_angle, *end_angle],
        CurveDefinition::Ellipse {
            minor_axis_ratio, ..
        } => vec![*minor_axis_ratio],
        CurveDefinition::EllipticalArc {
            minor_axis_ratio,
            start_angle,
            end_angle,
            ..
        } => vec![*minor_axis_ratio, *start_angle, *end_angle],
        CurveDefinition::RationalQuadraticConic { middle_weight, .. } => vec![*middle_weight],
        CurveDefinition::ParabolaSegment {
            trim_start,
            trim_end,
            ..
        } => vec![*trim_start, *trim_end],
        CurveDefinition::HyperbolaSegment {
            semi_conjugate,
            trim_start,
            trim_end,
            ..
        } => vec![*semi_conjugate, *trim_start, *trim_end],
        CurveDefinition::Line { .. }
        | CurveDefinition::Polyline { .. }
        | CurveDefinition::QuadraticBezier { .. }
        | CurveDefinition::CubicBezier { .. }
        | CurveDefinition::BSpline { .. } => Vec::new(),
        CurveDefinition::Nurbs { weights, .. } => weights.clone(),
    }
}

pub(crate) enum DocumentConicGeometryError {
    Document(DocumentError),
    Definition(geosolve_geometry::ConicDefinitionError),
}

impl From<DocumentError> for DocumentConicGeometryError {
    fn from(error: DocumentError) -> Self {
        Self::Document(error)
    }
}

impl From<geosolve_geometry::ConicDefinitionError> for DocumentConicGeometryError {
    fn from(error: geosolve_geometry::ConicDefinitionError) -> Self {
        Self::Definition(error)
    }
}

pub(super) fn document_curve_conic_geometry_error(
    error: DocumentConicGeometryError,
) -> DocumentCurveEvaluationError {
    match error {
        DocumentConicGeometryError::Document(error) => {
            DocumentCurveEvaluationError::Document(error)
        }
        DocumentConicGeometryError::Definition(error) => {
            DocumentCurveEvaluationError::ConicDefinition(error)
        }
    }
}

pub(super) fn document_bspline_curve_error(
    curve: CurveId,
    error: DocumentCurveEvaluationError,
) -> DocumentError {
    match error {
        DocumentCurveEvaluationError::Document(error) => error,
        DocumentCurveEvaluationError::BSplineDefinition(source) => {
            DocumentError::BSplineDefinition { curve, source }
        }
        DocumentCurveEvaluationError::BSplineEvaluation(source) => {
            DocumentError::BSplineEvaluation { curve, source }
        }
        other => DocumentError::InvalidField {
            field: "curve",
            message: other.to_string(),
        },
    }
}

pub(super) fn document_nurbs_curve_error(
    curve: CurveId,
    error: DocumentCurveEvaluationError,
) -> DocumentError {
    match error {
        DocumentCurveEvaluationError::Document(error) => error,
        DocumentCurveEvaluationError::BSplineDefinition(source) => {
            DocumentError::BSplineDefinition { curve, source }
        }
        DocumentCurveEvaluationError::BSplineEvaluation(source) => {
            DocumentError::BSplineEvaluation { curve, source }
        }
        DocumentCurveEvaluationError::NurbsDefinition(source) => {
            DocumentError::NurbsDefinition { curve, source }
        }
        DocumentCurveEvaluationError::NurbsEvaluation(source) => {
            DocumentError::NurbsEvaluation { curve, source }
        }
        other => DocumentError::InvalidField {
            field: "curve",
            message: other.to_string(),
        },
    }
}

pub(super) fn document_query_conic_geometry_error(
    error: DocumentConicGeometryError,
) -> DocumentConicQueryError {
    match error {
        DocumentConicGeometryError::Document(error) => DocumentConicQueryError::Document(error),
        DocumentConicGeometryError::Definition(error) => DocumentConicQueryError::Definition(error),
    }
}

pub(super) fn document_trim_projection_geometry_error(
    curve: CurveId,
    error: DocumentConicGeometryError,
) -> DocumentTrimProjectionError {
    match error {
        DocumentConicGeometryError::Document(error) => DocumentTrimProjectionError::Document(error),
        DocumentConicGeometryError::Definition(source) => {
            DocumentTrimProjectionError::ConicDefinition { curve, source }
        }
    }
}

pub(super) fn angular_target_difference(
    curve: CurveId,
    center: [f64; 2],
    target: [f64; 2],
) -> Result<[f64; 2], DocumentTrimProjectionError> {
    let difference = [target[0] - center[0], target[1] - center[1]];
    if !difference.iter().all(|value| value.is_finite()) {
        return Err(DocumentTrimProjectionError::NonFiniteResult { curve });
    }
    if difference[0] == 0.0 && difference[1] == 0.0 {
        return Err(DocumentTrimProjectionError::AmbiguousCenterTarget { curve });
    }
    Ok(difference)
}

pub(super) fn document_conic_geometry_document_error(
    curve: CurveId,
    error: DocumentConicGeometryError,
) -> DocumentError {
    match error {
        DocumentConicGeometryError::Document(error) => error,
        DocumentConicGeometryError::Definition(source) => {
            DocumentError::ConicDefinition { curve, source }
        }
    }
}

pub(super) fn indexed_point(
    points: [geosolve_geometry::Point2<f64>; 2],
    index: u32,
) -> Option<geosolve_geometry::Point2<f64>> {
    usize::try_from(index)
        .ok()
        .and_then(|index| points.get(index).copied())
}

pub(super) fn finite_query_point(
    curve: CurveId,
    point: geosolve_geometry::Point2<f64>,
) -> Result<[f64; 2], DocumentConicQueryError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok([point.x, point.y])
    } else {
        Err(DocumentConicQueryError::NonFiniteResult { curve })
    }
}

pub(super) const fn conic_ratio_domain() -> ScalarDomain {
    ScalarDomain::Bounded {
        lower: f64::from_bits(1),
        upper: 1.0,
    }
}

pub(super) const fn conic_weight_domain() -> ScalarDomain {
    ScalarDomain::Bounded {
        lower: crate::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
        upper: f64::MAX,
    }
}

pub(super) fn require_trim_scalar(
    scalar: &DesignScalar,
    field: &'static str,
) -> Result<(), DocumentError> {
    require_scalar_role(scalar, ScalarUnit::Parameter, ScalarDomain::Finite, field)
}

pub(super) const fn is_conic_definition(definition: &CurveDefinition) -> bool {
    matches!(
        definition,
        CurveDefinition::Ellipse { .. }
            | CurveDefinition::EllipticalArc { .. }
            | CurveDefinition::RationalQuadraticConic { .. }
            | CurveDefinition::ParabolaSegment { .. }
            | CurveDefinition::HyperbolaSegment { .. }
    )
}

pub(crate) const fn document_hyperbola_branch(
    branch: DocumentHyperbolaBranch,
) -> geosolve_geometry::HyperbolaBranch {
    match branch {
        DocumentHyperbolaBranch::Positive => geosolve_geometry::HyperbolaBranch::Positive,
        DocumentHyperbolaBranch::Negative => geosolve_geometry::HyperbolaBranch::Negative,
    }
}
