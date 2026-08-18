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
    CreateProfileOffset => "create_profile_offset",
    CreateProfileOffsetGeometry => "create_profile_offset_geometry",
    CreateParameter => "create_parameter",
    AddParameterBinding => "add_parameter_binding",
    RemoveParameterBinding => "remove_parameter_binding",
    AddParameterOutput => "add_parameter_output",
    RemoveParameterOutput => "remove_parameter_output",
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
    SetRationalConicControl => "set_rational_conic_control",
    SetHyperbolaBranch => "set_hyperbola_branch",
    InsertBSplineKnot => "insert_b_spline_knot",
    InsertMirroredBSplineKnot => "insert_mirrored_b_spline_knot",
    TransitionBSplineContact => "transition_b_spline_contact",
    InsertNurbsKnot => "insert_nurbs_knot",
    TransitionNurbsContact => "transition_nurbs_contact",
    SetNurbsWeightGauge => "set_nurbs_weight_gauge",
    SetContactStates => "set_contact_states",
    SetContactBranches => "set_contact_branches",
    SetCircleTangencyBranch => "set_circle_tangency_branch",
    SetDimensionMode => "set_dimension_mode",
    SetProfileOffsetOperand => "set_profile_offset_operand",
    SetOrientedAngleOrientation => "set_oriented_angle_orientation",
    SetSourceSuppressed => "set_source_suppressed",
    SetGeometryRole => "set_geometry_role",
    SetGeometryRoles => "set_geometry_roles",
    SetElementUserSuppressed => "set_element_user_suppressed",
    SetHostConfigurationActivation => "set_host_configuration_activation",
    Delete => "delete",
});

characterized_enum!(CurrentDocumentEffectKind {
    CreatedPoint => "created_point",
    CreatedScalar => "created_scalar",
    CreatedCurve => "created_curve",
    CreatedContact => "created_contact",
    CreatedConstraint => "created_constraint",
    CreatedDimension => "created_dimension",
    CreatedProfileOffset => "created_profile_offset",
    CreatedParameter => "created_parameter",
    AddedParameterBinding => "added_parameter_binding",
    RemovedParameterBinding => "removed_parameter_binding",
    AddedParameterOutput => "added_parameter_output",
    RemovedParameterOutput => "removed_parameter_output",
    CreatedRectangle => "created_rectangle",
    CreatedMirroredCurve => "created_mirrored_curve",
    CreatedLineLineFillet => "created_line_line_fillet",
    CreatedCurveCurveFillet => "created_curve_curve_fillet",
    UpdatedPoint => "updated_point",
    UpdatedScalar => "updated_scalar",
    UpdatedCurve => "updated_curve",
    UpdatedConicWeightedMiddle => "updated_conic_weighted_middle",
    UpdatedRationalConicControl => "updated_rational_conic_control",
    UpdatedHyperbolaBranch => "updated_hyperbola_branch",
    InsertedBSplineKnot => "inserted_b_spline_knot",
    InsertedMirroredBSplineKnot => "inserted_mirrored_b_spline_knot",
    InsertedNurbsKnot => "inserted_nurbs_knot",
    UpdatedNurbsWeightGauge => "updated_nurbs_weight_gauge",
    UpdatedContacts => "updated_contacts",
    UpdatedConstraint => "updated_constraint",
    UpdatedDimension => "updated_dimension",
    UpdatedProfileOffset => "updated_profile_offset",
    UpdatedSource => "updated_source",
    UpdatedGeometryRole => "updated_geometry_role",
    UpdatedGeometryRoles => "updated_geometry_roles",
    UpdatedElementUserSuppression => "updated_element_user_suppression",
    UpdatedHostConfigurationActivation => "updated_host_configuration_activation",
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
    DimensionProfileOffset => "dimension_profile_offset",
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
            Self::CreateProfileOffset { .. } => CurrentDocumentCommandKind::CreateProfileOffset,
            Self::CreateProfileOffsetGeometry { .. }
            | Self::CreatePreparedProfileOffsetGeometry { .. } => {
                CurrentDocumentCommandKind::CreateProfileOffsetGeometry
            }
            Self::CreateParameter { .. } => CurrentDocumentCommandKind::CreateParameter,
            Self::AddParameterBinding { .. } => CurrentDocumentCommandKind::AddParameterBinding,
            Self::RemoveParameterBinding { .. } => {
                CurrentDocumentCommandKind::RemoveParameterBinding
            }
            Self::AddParameterOutput { .. } => CurrentDocumentCommandKind::AddParameterOutput,
            Self::RemoveParameterOutput { .. } => CurrentDocumentCommandKind::RemoveParameterOutput,
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
            Self::SetRationalConicControl { .. } => {
                CurrentDocumentCommandKind::SetRationalConicControl
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
            Self::SetContactBranches { .. } => CurrentDocumentCommandKind::SetContactBranches,
            Self::SetCircleTangencyBranch { .. } => {
                CurrentDocumentCommandKind::SetCircleTangencyBranch
            }
            Self::SetDimensionMode { .. } => CurrentDocumentCommandKind::SetDimensionMode,
            Self::SetProfileOffsetOperand { .. } => {
                CurrentDocumentCommandKind::SetProfileOffsetOperand
            }
            Self::SetOrientedAngleOrientation { .. } => {
                CurrentDocumentCommandKind::SetOrientedAngleOrientation
            }
            Self::SetSourceSuppressed { .. } => CurrentDocumentCommandKind::SetSourceSuppressed,
            Self::SetGeometryRole { .. } => CurrentDocumentCommandKind::SetGeometryRole,
            Self::SetGeometryRoles { .. } => CurrentDocumentCommandKind::SetGeometryRoles,
            Self::SetElementUserSuppressed { .. } => {
                CurrentDocumentCommandKind::SetElementUserSuppressed
            }
            Self::SetHostConfigurationActivation { .. } => {
                CurrentDocumentCommandKind::SetHostConfigurationActivation
            }
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
            Self::CreatedProfileOffset(_) => CurrentDocumentEffectKind::CreatedProfileOffset,
            Self::CreatedParameter(_) => CurrentDocumentEffectKind::CreatedParameter,
            Self::AddedParameterBinding { .. } => CurrentDocumentEffectKind::AddedParameterBinding,
            Self::RemovedParameterBinding { .. } => {
                CurrentDocumentEffectKind::RemovedParameterBinding
            }
            Self::AddedParameterOutput { .. } => CurrentDocumentEffectKind::AddedParameterOutput,
            Self::RemovedParameterOutput { .. } => {
                CurrentDocumentEffectKind::RemovedParameterOutput
            }
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
            Self::UpdatedRationalConicControl(_) => {
                CurrentDocumentEffectKind::UpdatedRationalConicControl
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
            Self::UpdatedProfileOffset(_) => CurrentDocumentEffectKind::UpdatedProfileOffset,
            Self::UpdatedSource(_) => CurrentDocumentEffectKind::UpdatedSource,
            Self::UpdatedGeometryRole(_) => CurrentDocumentEffectKind::UpdatedGeometryRole,
            Self::UpdatedGeometryRoles(_) => CurrentDocumentEffectKind::UpdatedGeometryRoles,
            Self::UpdatedElementUserSuppression(_) => {
                CurrentDocumentEffectKind::UpdatedElementUserSuppression
            }
            Self::UpdatedHostConfigurationActivation => {
                CurrentDocumentEffectKind::UpdatedHostConfigurationActivation
            }
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
            Self::ProfileOffset { .. } => CurrentMeasurementKind::DimensionProfileOffset,
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
