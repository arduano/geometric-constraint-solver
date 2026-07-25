// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    DocumentCommandEffect, DocumentConicMeasurement, DocumentCurveMeasurementKind,
    DocumentDimensionDefinition, DocumentEdit,
};

macro_rules! characterized_enum {
    ($name:ident { $($variant:ident => $code:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub const fn code(self) -> &'static str {
                match self {
                    $(Self::$variant => $code),+
                }
            }
        }
    };
}

characterized_enum!(CurrentDocumentCommandKind {
    CreatePoint => "create_point",
    CreateScalar => "create_scalar",
    CreateCurve => "create_curve",
    CreateContact => "create_contact",
    CreateConstraint => "create_constraint",
    CreateDimension => "create_dimension",
    CreateRectangle => "create_rectangle",
    CreateMirroredCurve => "create_mirrored_curve",
    CreateLineLineFillet => "create_line_line_fillet",
    CreateCurveCurveFillet => "create_curve_curve_fillet",
    SetPointPosition => "set_point_position",
    SetScalarValue => "set_scalar_value",
    SetCurveBranch => "set_curve_branch",
    SetArcSweep => "set_arc_sweep",
    SetLineLineFilletBranch => "set_line_line_fillet_branch",
    SetCurveCurveFilletBranch => "set_curve_curve_fillet_branch",
    SetConicWeightedMiddle => "set_conic_weighted_middle",
    SetHyperbolaBranch => "set_hyperbola_branch",
    InsertBSplineKnot => "insert_b_spline_knot",
    InsertMirroredBSplineKnot => "insert_mirrored_b_spline_knot",
    TransitionBSplineContact => "transition_b_spline_contact",
    InsertNurbsKnot => "insert_nurbs_knot",
    TransitionNurbsContact => "transition_nurbs_contact",
    SetNurbsWeightGauge => "set_nurbs_weight_gauge",
    SetContactStates => "set_contact_states",
    SetCircleTangencyBranch => "set_circle_tangency_branch",
    SetDimensionMode => "set_dimension_mode",
    SetOrientedAngleOrientation => "set_oriented_angle_orientation",
    SetSourceSuppressed => "set_source_suppressed",
    Delete => "delete",
});

characterized_enum!(CurrentDocumentEffectKind {
    CreatedPoint => "created_point",
    CreatedScalar => "created_scalar",
    CreatedCurve => "created_curve",
    CreatedContact => "created_contact",
    CreatedConstraint => "created_constraint",
    CreatedDimension => "created_dimension",
    CreatedRectangle => "created_rectangle",
    CreatedMirroredCurve => "created_mirrored_curve",
    CreatedLineLineFillet => "created_line_line_fillet",
    CreatedCurveCurveFillet => "created_curve_curve_fillet",
    UpdatedPoint => "updated_point",
    UpdatedScalar => "updated_scalar",
    UpdatedCurve => "updated_curve",
    UpdatedConicWeightedMiddle => "updated_conic_weighted_middle",
    UpdatedHyperbolaBranch => "updated_hyperbola_branch",
    InsertedBSplineKnot => "inserted_b_spline_knot",
    InsertedMirroredBSplineKnot => "inserted_mirrored_b_spline_knot",
    InsertedNurbsKnot => "inserted_nurbs_knot",
    UpdatedNurbsWeightGauge => "updated_nurbs_weight_gauge",
    UpdatedContacts => "updated_contacts",
    UpdatedConstraint => "updated_constraint",
    UpdatedDimension => "updated_dimension",
    UpdatedSource => "updated_source",
    Deleted => "deleted",
    Transaction => "transaction",
    Imported => "imported",
    Undo => "undo",
    Redo => "redo",
});

characterized_enum!(CurrentMeasurementKind {
    DimensionPointDistance => "dimension_point_distance",
    DimensionCurveLength => "dimension_curve_length",
    DimensionRadius => "dimension_radius",
    DimensionDiameter => "dimension_diameter",
    DimensionOrientedAngle => "dimension_oriented_angle",
    DimensionSupportingLineOffset => "dimension_supporting_line_offset",
    DimensionExactTranslatedSegmentOffset => "dimension_exact_translated_segment_offset",
    CurveSignedCurvature => "curve_signed_curvature",
    CurveUnsignedCurvature => "curve_unsigned_curvature",
    CurveOsculatingRadius => "curve_osculating_radius",
    ConicMajorAxisLength => "conic_major_axis_length",
    ConicMinorAxisLength => "conic_minor_axis_length",
    ConicLinearEccentricity => "conic_linear_eccentricity",
    ConicFocalDistance => "conic_focal_distance",
    ConicTransverseAxisLength => "conic_transverse_axis_length",
    ConicConjugateAxisLength => "conic_conjugate_axis_length",
});

impl DocumentEdit {
    /// Exhaustive characterization of this current command variant.
    #[must_use]
    pub const fn current_kind(&self) -> CurrentDocumentCommandKind {
        match self {
            Self::CreatePoint { .. } => CurrentDocumentCommandKind::CreatePoint,
            Self::CreateScalar { .. } => CurrentDocumentCommandKind::CreateScalar,
            Self::CreateCurve { .. } => CurrentDocumentCommandKind::CreateCurve,
            Self::CreateContact { .. } => CurrentDocumentCommandKind::CreateContact,
            Self::CreateConstraint { .. } => CurrentDocumentCommandKind::CreateConstraint,
            Self::CreateDimension { .. } => CurrentDocumentCommandKind::CreateDimension,
            Self::CreateRectangle { .. } => CurrentDocumentCommandKind::CreateRectangle,
            Self::CreateMirroredCurve { .. } => CurrentDocumentCommandKind::CreateMirroredCurve,
            Self::CreateLineLineFillet { .. } => CurrentDocumentCommandKind::CreateLineLineFillet,
            Self::CreateCurveCurveFillet { .. } => {
                CurrentDocumentCommandKind::CreateCurveCurveFillet
            }
            Self::SetPointPosition { .. } => CurrentDocumentCommandKind::SetPointPosition,
            Self::SetScalarValue { .. } => CurrentDocumentCommandKind::SetScalarValue,
            Self::SetCurveBranch { .. } => CurrentDocumentCommandKind::SetCurveBranch,
            Self::SetArcSweep { .. } => CurrentDocumentCommandKind::SetArcSweep,
            Self::SetLineLineFilletBranch { .. } => {
                CurrentDocumentCommandKind::SetLineLineFilletBranch
            }
            Self::SetCurveCurveFilletBranch { .. } => {
                CurrentDocumentCommandKind::SetCurveCurveFilletBranch
            }
            Self::SetConicWeightedMiddle { .. } => {
                CurrentDocumentCommandKind::SetConicWeightedMiddle
            }
            Self::SetHyperbolaBranch { .. } => CurrentDocumentCommandKind::SetHyperbolaBranch,
            Self::InsertBSplineKnot { .. } => CurrentDocumentCommandKind::InsertBSplineKnot,
            Self::InsertMirroredBSplineKnot { .. } => {
                CurrentDocumentCommandKind::InsertMirroredBSplineKnot
            }
            Self::TransitionBSplineContact { .. } => {
                CurrentDocumentCommandKind::TransitionBSplineContact
            }
            Self::InsertNurbsKnot { .. } => CurrentDocumentCommandKind::InsertNurbsKnot,
            Self::TransitionNurbsContact { .. } => {
                CurrentDocumentCommandKind::TransitionNurbsContact
            }
            Self::SetNurbsWeightGauge { .. } => CurrentDocumentCommandKind::SetNurbsWeightGauge,
            Self::SetContactStates { .. } => CurrentDocumentCommandKind::SetContactStates,
            Self::SetCircleTangencyBranch { .. } => {
                CurrentDocumentCommandKind::SetCircleTangencyBranch
            }
            Self::SetDimensionMode { .. } => CurrentDocumentCommandKind::SetDimensionMode,
            Self::SetOrientedAngleOrientation { .. } => {
                CurrentDocumentCommandKind::SetOrientedAngleOrientation
            }
            Self::SetSourceSuppressed { .. } => CurrentDocumentCommandKind::SetSourceSuppressed,
            Self::Delete { .. } => CurrentDocumentCommandKind::Delete,
        }
    }
}

impl DocumentCommandEffect {
    /// Exhaustive characterization of this current accepted effect.
    #[must_use]
    pub const fn current_kind(&self) -> CurrentDocumentEffectKind {
        match self {
            Self::CreatedPoint(_) => CurrentDocumentEffectKind::CreatedPoint,
            Self::CreatedScalar(_) => CurrentDocumentEffectKind::CreatedScalar,
            Self::CreatedCurve(_) => CurrentDocumentEffectKind::CreatedCurve,
            Self::CreatedContact(_) => CurrentDocumentEffectKind::CreatedContact,
            Self::CreatedConstraint(_) => CurrentDocumentEffectKind::CreatedConstraint,
            Self::CreatedDimension(_) => CurrentDocumentEffectKind::CreatedDimension,
            Self::CreatedRectangle(_) => CurrentDocumentEffectKind::CreatedRectangle,
            Self::CreatedMirroredCurve(_) => CurrentDocumentEffectKind::CreatedMirroredCurve,
            Self::CreatedLineLineFillet(_) => CurrentDocumentEffectKind::CreatedLineLineFillet,
            Self::CreatedCurveCurveFillet(_) => CurrentDocumentEffectKind::CreatedCurveCurveFillet,
            Self::UpdatedPoint(_) => CurrentDocumentEffectKind::UpdatedPoint,
            Self::UpdatedScalar(_) => CurrentDocumentEffectKind::UpdatedScalar,
            Self::UpdatedCurve(_) => CurrentDocumentEffectKind::UpdatedCurve,
            Self::UpdatedConicWeightedMiddle(_) => {
                CurrentDocumentEffectKind::UpdatedConicWeightedMiddle
            }
            Self::UpdatedHyperbolaBranch(_) => CurrentDocumentEffectKind::UpdatedHyperbolaBranch,
            Self::InsertedBSplineKnot(_) => CurrentDocumentEffectKind::InsertedBSplineKnot,
            Self::InsertedMirroredBSplineKnot(_) => {
                CurrentDocumentEffectKind::InsertedMirroredBSplineKnot
            }
            Self::InsertedNurbsKnot(_) => CurrentDocumentEffectKind::InsertedNurbsKnot,
            Self::UpdatedNurbsWeightGauge(_) => CurrentDocumentEffectKind::UpdatedNurbsWeightGauge,
            Self::UpdatedContacts(_) => CurrentDocumentEffectKind::UpdatedContacts,
            Self::UpdatedConstraint(_) => CurrentDocumentEffectKind::UpdatedConstraint,
            Self::UpdatedDimension(_) => CurrentDocumentEffectKind::UpdatedDimension,
            Self::UpdatedSource(_) => CurrentDocumentEffectKind::UpdatedSource,
            Self::Deleted(_) => CurrentDocumentEffectKind::Deleted,
            Self::Transaction(_) => CurrentDocumentEffectKind::Transaction,
            Self::Imported => CurrentDocumentEffectKind::Imported,
            Self::Undo => CurrentDocumentEffectKind::Undo,
            Self::Redo => CurrentDocumentEffectKind::Redo,
        }
    }
}

impl DocumentDimensionDefinition {
    /// Characterizes the current driving/reference dimension measurement.
    #[must_use]
    pub const fn current_measurement_kind(&self) -> CurrentMeasurementKind {
        match self {
            Self::PointDistance { .. } => CurrentMeasurementKind::DimensionPointDistance,
            Self::CurveLength { .. } => CurrentMeasurementKind::DimensionCurveLength,
            Self::Radius { .. } => CurrentMeasurementKind::DimensionRadius,
            Self::Diameter { .. } => CurrentMeasurementKind::DimensionDiameter,
            Self::OrientedAngle { .. } => CurrentMeasurementKind::DimensionOrientedAngle,
            Self::SupportingLineOffset { .. } => {
                CurrentMeasurementKind::DimensionSupportingLineOffset
            }
            Self::ExactTranslatedSegmentOffset { .. } => {
                CurrentMeasurementKind::DimensionExactTranslatedSegmentOffset
            }
        }
    }
}

impl DocumentCurveMeasurementKind {
    /// Characterizes the current equation-free differential measurement.
    #[must_use]
    pub const fn current_measurement_kind(self) -> CurrentMeasurementKind {
        match self {
            Self::SignedCurvature => CurrentMeasurementKind::CurveSignedCurvature,
            Self::UnsignedCurvature => CurrentMeasurementKind::CurveUnsignedCurvature,
            Self::OsculatingRadius => CurrentMeasurementKind::CurveOsculatingRadius,
        }
    }
}

impl DocumentConicMeasurement {
    /// Characterizes the current equation-free conic measurement.
    #[must_use]
    pub const fn current_measurement_kind(self) -> CurrentMeasurementKind {
        match self {
            Self::MajorAxisLength => CurrentMeasurementKind::ConicMajorAxisLength,
            Self::MinorAxisLength => CurrentMeasurementKind::ConicMinorAxisLength,
            Self::LinearEccentricity => CurrentMeasurementKind::ConicLinearEccentricity,
            Self::FocalDistance => CurrentMeasurementKind::ConicFocalDistance,
            Self::TransverseAxisLength => CurrentMeasurementKind::ConicTransverseAxisLength,
            Self::ConjugateAxisLength => CurrentMeasurementKind::ConicConjugateAxisLength,
        }
    }
}
