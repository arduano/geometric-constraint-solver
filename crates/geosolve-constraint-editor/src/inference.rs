// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation-independent drafting inference.
//!
//! This module owns the small, deterministic state machine between normalized
//! pointer samples and prospective construction relations.  It deliberately
//! does not allocate sketch objects or solve a document.  The retained
//! coordinator translates a confirmed candidate into one atomic construction
//! plan and remains responsible for independent solve validation.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveId, CurveSpan, DesignPointId, DocumentCoordinateAxis,
    GeometryRole, PreparedSketchInput, SketchDesignIdentity,
};
use thiserror::Error;

use crate::{
    EditorScene, GeometryInteractionPolicy, GeometryPickScope, ScenePointRoleIncidence,
    ScreenPoint, Viewport,
};

const MAX_CONFIGURED_CANDIDATES: usize = 32;
const MAX_CONFIGURED_REFERENCES: usize = 8;
const MAX_CONFIGURED_SCENE_ANCHORS: usize = 65_536;
const MAX_CONFIGURED_SCENE_CURVE_SEGMENTS: usize = 1_048_576;
// A positional anchor and direction adjustment may share one hard bundle only
// when their independently requested screen positions differ by floating-point
// noise, not by a model-coordinate tolerance that grows under translation.
const DIRECTION_COMBINATION_TOLERANCE_PIXELS: f64 = 1.0e-9;

/// M69 Profile/Construction overlap band retained by inference ranking.
///
/// In [`GeometryPickScope::All`], a Profile anchor may be this many CSS pixels
/// farther than the nearest equivalent Construction anchor and still win.  At
/// larger separation the genuinely nearer anchor wins.
pub const PROFILE_CONSTRUCTION_OVERLAP_PIXELS: f64 = 1.0;

/// Independent behavior switches for one drafting-inference family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DraftInferenceBehavior {
    pub show_guides: bool,
    pub adjust_coordinates: bool,
    pub persist_constraint: bool,
}

impl DraftInferenceBehavior {
    /// Ordinary CAD inference: preview a guide, snap the draft and retain the
    /// corresponding semantic relation when placement is confirmed.
    #[must_use]
    pub const fn constraint_backed() -> Self {
        Self {
            show_guides: true,
            adjust_coordinates: true,
            persist_constraint: true,
        }
    }

    /// Visual tracking only.  It neither moves the sample nor requests a
    /// durable sketch relation.
    #[must_use]
    pub const fn tracking_only() -> Self {
        Self {
            show_guides: true,
            adjust_coordinates: false,
            persist_constraint: false,
        }
    }

    const fn has_effect(self) -> bool {
        self.adjust_coordinates || self.persist_constraint
    }

    const fn guide_classification(self) -> DraftGuideClassification {
        if self.persist_constraint {
            DraftGuideClassification::ConstraintBacked
        } else {
            DraftGuideClassification::TrackingOnly
        }
    }
}

/// Screen- and angle-space enter/leave thresholds.
///
/// Entry and exit comparisons are inclusive.  A latched semantic owner remains
/// active through its exit threshold, preventing flicker around an entry edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftInferenceTolerances {
    pub point_enter_pixels: f64,
    pub point_exit_pixels: f64,
    pub curve_enter_pixels: f64,
    pub curve_exit_pixels: f64,
    /// Euclidean screen distance for entering the intrinsic origin datum.
    pub datum_origin_enter_pixels: f64,
    /// Euclidean screen distance for retaining the active origin datum.
    pub datum_origin_exit_pixels: f64,
    /// Perpendicular screen distance for entering an intrinsic axis datum.
    pub datum_axis_enter_pixels: f64,
    /// Perpendicular screen distance for retaining the active axis datum.
    pub datum_axis_exit_pixels: f64,
    pub direction_enter_radians: f64,
    pub direction_exit_radians: f64,
}

impl Default for DraftInferenceTolerances {
    fn default() -> Self {
        Self {
            point_enter_pixels: 6.0,
            point_exit_pixels: 9.0,
            curve_enter_pixels: 8.0,
            curve_exit_pixels: 12.0,
            datum_origin_enter_pixels: 6.0,
            datum_origin_exit_pixels: 9.0,
            datum_axis_enter_pixels: 4.0,
            datum_axis_exit_pixels: 7.0,
            direction_enter_radians: 3.0_f64.to_radians(),
            direction_exit_radians: 5.0_f64.to_radians(),
        }
    }
}

impl DraftInferenceTolerances {
    fn is_valid(self) -> bool {
        self.point_enter_pixels.is_finite()
            && self.point_exit_pixels.is_finite()
            && self.curve_enter_pixels.is_finite()
            && self.curve_exit_pixels.is_finite()
            && self.datum_origin_enter_pixels.is_finite()
            && self.datum_origin_exit_pixels.is_finite()
            && self.datum_axis_enter_pixels.is_finite()
            && self.datum_axis_exit_pixels.is_finite()
            && self.direction_enter_radians.is_finite()
            && self.direction_exit_radians.is_finite()
            && self.point_enter_pixels >= 0.0
            && self.point_exit_pixels >= self.point_enter_pixels
            && self.curve_enter_pixels >= 0.0
            && self.curve_exit_pixels >= self.curve_enter_pixels
            && self.datum_origin_enter_pixels >= 0.0
            && self.datum_origin_exit_pixels >= self.datum_origin_enter_pixels
            && self.datum_axis_enter_pixels >= 0.0
            && self.datum_axis_exit_pixels >= self.datum_axis_enter_pixels
            && self.direction_enter_radians >= 0.0
            && self.direction_exit_radians >= self.direction_enter_radians
            && self.direction_exit_radians <= std::f64::consts::FRAC_PI_2
    }
}

/// Explicit deterministic resource limits for one inference session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DraftInferenceLimits {
    pub max_candidates: usize,
    pub max_remembered_references: usize,
    /// Maximum native semantic anchors that one scene query may publish.
    pub max_scene_anchors: usize,
    /// Maximum tessellation chords that one scene query may inspect.
    pub max_scene_curve_segments: usize,
}

impl Default for DraftInferenceLimits {
    fn default() -> Self {
        Self {
            max_candidates: 32,
            max_remembered_references: 8,
            max_scene_anchors: 4_096,
            max_scene_curve_segments: 16_384,
        }
    }
}

impl DraftInferenceLimits {
    fn is_valid(self) -> bool {
        (1..=MAX_CONFIGURED_CANDIDATES).contains(&self.max_candidates)
            && (1..=MAX_CONFIGURED_REFERENCES).contains(&self.max_remembered_references)
            && (1..=MAX_CONFIGURED_SCENE_ANCHORS).contains(&self.max_scene_anchors)
            && (1..=MAX_CONFIGURED_SCENE_CURVE_SEGMENTS).contains(&self.max_scene_curve_segments)
    }
}

/// Complete reusable policy for the M70 core relation families.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftInferencePolicy {
    pub point_identity: DraftInferenceBehavior,
    pub point_on_curve: DraftInferenceBehavior,
    pub midpoint: DraftInferenceBehavior,
    /// Intrinsic sketch-origin inference for persistent point stages.
    pub datum_origin: DraftInferenceBehavior,
    /// Intrinsic Cartesian axis inference for persistent point stages.
    pub datum_axis: DraftInferenceBehavior,
    pub horizontal: DraftInferenceBehavior,
    pub vertical: DraftInferenceBehavior,
    pub parallel: DraftInferenceBehavior,
    pub perpendicular: DraftInferenceBehavior,
    /// Exact semantic-center equality for centered constructions.
    pub concentric: DraftInferenceBehavior,
    /// Certified native affine supporting-line extension.
    pub collinear: DraftInferenceBehavior,
    /// Remembered-origin horizontal/vertical guidance. M71 permits this to be
    /// constraint-backed for stored points and accepted native line/polyline
    /// midpoints; fillet-discarded and nonlinear derived anchors remain tracking-only.
    pub point_tracking: DraftInferenceBehavior,
    pub tolerances: DraftInferenceTolerances,
    pub limits: DraftInferenceLimits,
}

impl Default for DraftInferencePolicy {
    fn default() -> Self {
        let backed = DraftInferenceBehavior::constraint_backed();
        Self {
            point_identity: backed,
            point_on_curve: backed,
            midpoint: backed,
            datum_origin: backed,
            datum_axis: backed,
            horizontal: backed,
            vertical: backed,
            parallel: backed,
            perpendicular: backed,
            concentric: backed,
            collinear: backed,
            point_tracking: backed,
            tolerances: DraftInferenceTolerances::default(),
            limits: DraftInferenceLimits::default(),
        }
    }
}

impl DraftInferencePolicy {
    /// Validates thresholds, limits and point-family structural invariants.
    ///
    /// Persistent-point identity reuse necessarily places the authored operand
    /// at that accepted point. It therefore cannot persist while coordinate
    /// adjustment is disabled. Hosts that want a non-snapping point cue may use
    /// guide-only identity behavior instead.
    ///
    /// # Errors
    ///
    /// Returns [`DraftInferenceError::InvalidPolicy`] for an invalid policy.
    pub fn validate(self) -> Result<(), DraftInferenceError> {
        if !self.tolerances.is_valid() || !self.limits.is_valid() {
            return Err(DraftInferenceError::InvalidPolicy);
        }
        if (self.point_identity.persist_constraint && !self.point_identity.adjust_coordinates)
            || (self.point_tracking.persist_constraint && !self.point_tracking.adjust_coordinates)
        {
            return Err(DraftInferenceError::InvalidPolicy);
        }
        Ok(())
    }
}

/// Host-normalized semantic input for one inference sample.
///
/// A platform adapter may map Shift or another gesture to `suppressed`; the
/// headless API deliberately does not assign keyboard meanings.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DraftInferenceInput {
    pub suppressed: bool,
    pub preferred_candidate: Option<DraftInferenceCandidateId>,
}

/// Exact contact metadata for a prospective point-on-curve relation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftCurveContact {
    pub span: CurveSpan,
    pub domain: ContactDomain,
    pub parameter: f64,
    pub winding: i32,
    pub neighborhood: ContactNeighborhood,
}

/// Deterministic branch identity for multiple screen-local contacts on one
/// nonlinear span.
///
/// Ordinals are meaningful only within the current scene query and are sorted
/// by total curve parameter. They prevent a self-intersection from collapsing
/// two valid contacts into an arbitrary first tessellation chord.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DraftCurveBranchCandidate(u32);

impl DraftCurveBranchCandidate {
    #[must_use]
    pub const fn from_ordinal(ordinal: u32) -> Self {
        Self(ordinal)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl DraftCurveContact {
    fn is_valid(self) -> bool {
        if !self.parameter.is_finite() {
            return false;
        }
        match self.domain {
            ContactDomain::SupportingLine => {
                self.winding == 0
                    && unbounded_contact_neighborhood_contains(self.neighborhood, self.parameter)
            }
            ContactDomain::Bounded { lower, upper } => {
                // A non-zero bounded winding is valid only for periodic
                // B-spline/NURBS topology.  `CurveSpan` deliberately carries
                // no curve definition, so that family-dependent check remains
                // with the exact document/commit validation boundary.
                lower.is_finite()
                    && upper.is_finite()
                    && lower < upper
                    && (lower..=upper).contains(&self.parameter)
                    && bounded_contact_neighborhood_contains(
                        self.neighborhood,
                        self.parameter,
                        lower,
                        upper,
                    )
            }
            ContactDomain::Periodic { period } => {
                if !period.is_finite() || period <= 0.0 || !(0.0..period).contains(&self.parameter)
                {
                    return false;
                }
                let total = period.mul_add(f64::from(self.winding), self.parameter);
                total.is_finite()
                    && unbounded_contact_neighborhood_contains(self.neighborhood, total)
            }
        }
    }
}

/// One native semantic anchor supplied by the scene/document adapter.
///
/// The enum intentionally has no generated/computed-curve variant.  An implicit
/// Fillet-discarded occurrence may be represented only after it has resolved to
/// its complete native `CurveSpan` and retains its source-origin metadata.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DraftReferenceAnchor {
    PersistentPoint {
        point: DesignPointId,
        model_position: [f64; 2],
        role_incidence: ScenePointRoleIncidence,
    },
    Midpoint {
        span: CurveSpan,
        model_position: [f64; 2],
        affine_direction: [f64; 2],
        role: GeometryRole,
        source_role: GeometryRole,
        origin: DraftReferenceOrigin,
    },
    CurvePoint {
        contact: DraftCurveContact,
        branch_candidate: DraftCurveBranchCandidate,
        model_position: [f64; 2],
        role: GeometryRole,
        source_role: GeometryRole,
        origin: DraftReferenceOrigin,
    },
    /// A closest point on a genuine line/polyline span.  Only this explicit
    /// native affine variant can become a Parallel/Perpendicular reference.
    AffineSupport {
        contact: DraftCurveContact,
        model_position: [f64; 2],
        affine_direction: [f64; 2],
        role: GeometryRole,
        source_role: GeometryRole,
        origin: DraftReferenceOrigin,
    },
}

/// Presentation origin relevant to M69 role/scope filtering.
///
/// Exact Fillet-fragment provenance remains on `SceneCurve`; inference always
/// resolves such an occurrence to the native span carried by its contact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftReferenceOrigin {
    Native,
    FilletDiscarded,
}

/// Exact accepted curve-center evidence used by centered-curve inference.
///
/// The persistent curve identity is the prospective relation operand. The
/// stored center point remains separate semantic evidence: curves that share
/// it are still distinct retained operands with distinct deletion lifecycles.
/// Coordinate proximity alone never certifies semantic identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftSemanticCenterAnchor {
    pub curve: CurveId,
    pub center: DesignPointId,
    pub model_position: [f64; 2],
    pub role: GeometryRole,
}

impl DraftSemanticCenterAnchor {
    fn is_valid(self) -> bool {
        self.model_position.into_iter().all(f64::is_finite)
    }

    fn is_interactive(self, policy: GeometryInteractionPolicy) -> bool {
        match self.role {
            GeometryRole::Profile => !matches!(policy.scope, GeometryPickScope::Construction),
            GeometryRole::Construction => {
                policy.visibility.explicit_construction
                    && !matches!(policy.scope, GeometryPickScope::Profile)
            }
        }
    }
}

/// Scene resource whose deterministic pre-inference bound was exceeded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftInferenceSceneResource {
    Anchors,
    CurveSegments,
}

/// Exact fail-closed evidence for a bounded scene-anchor query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DraftInferenceSceneLimit {
    pub resource: DraftInferenceSceneResource,
    pub required: usize,
    pub limit: usize,
}

/// Complete result of collecting native scene anchors before inference.
///
/// Resource exhaustion never returns a truncated anchor prefix, because such a
/// prefix could silently select a different semantic winner.
#[derive(Clone, Debug, PartialEq)]
pub enum DraftInferenceAnchorCollection {
    Complete { anchors: Vec<DraftReferenceAnchor> },
    ResourceLimited(DraftInferenceSceneLimit),
}

/// Complete subject-specific scene inputs for one drafting-inference sample.
///
/// Ordinary anchors and semantic centers share one all-or-nothing resource
/// bound. A limited collection never exposes a prefix that could select a
/// different semantic winner.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftInferenceSceneInputs {
    pub anchors: Vec<DraftReferenceAnchor>,
    pub semantic_centers: Vec<DraftSemanticCenterAnchor>,
}

/// Bounded result of collecting every scene input relevant to one subject.
#[derive(Clone, Debug, PartialEq)]
pub enum DraftInferenceSceneInputCollection {
    Complete(DraftInferenceSceneInputs),
    ResourceLimited(DraftInferenceSceneLimit),
}

impl DraftReferenceOrigin {
    #[must_use]
    pub const fn is_implicit_construction(self) -> bool {
        matches!(self, Self::FilletDiscarded)
    }
}

impl DraftReferenceAnchor {
    fn is_valid(self) -> bool {
        let finite_position = self.model_position().into_iter().all(f64::is_finite);
        if !finite_position {
            return false;
        }
        match self {
            Self::PersistentPoint { .. } | Self::CurvePoint { .. } => {
                self.contact().is_none_or(DraftCurveContact::is_valid)
            }
            Self::Midpoint {
                affine_direction, ..
            }
            | Self::AffineSupport {
                affine_direction, ..
            } => {
                self.contact().is_none_or(DraftCurveContact::is_valid)
                    && normalized(affine_direction).is_some()
            }
        }
    }

    fn is_interactive(self, policy: GeometryInteractionPolicy) -> bool {
        match self {
            Self::PersistentPoint { role_incidence, .. } => match policy.scope {
                GeometryPickScope::Profile => role_incidence.profile,
                GeometryPickScope::Construction => {
                    role_incidence.construction && policy.visibility.explicit_construction
                }
                GeometryPickScope::All => {
                    role_incidence.profile
                        || (role_incidence.construction && policy.visibility.explicit_construction)
                }
            },
            Self::Midpoint { role, origin, .. }
            | Self::CurvePoint { role, origin, .. }
            | Self::AffineSupport { role, origin, .. } => {
                curve_role_is_interactive(role, origin, policy)
            }
        }
    }

    fn key(self) -> AnchorKey {
        match self {
            Self::PersistentPoint { point, .. } => AnchorKey::Point(point),
            Self::Midpoint { span, .. } => AnchorKey::Midpoint(span),
            Self::CurvePoint {
                contact,
                branch_candidate,
                ..
            } => {
                // Closed periodic curves paint their first and last chord at
                // the same topological seam.  Scene projection may therefore
                // publish the same principal contact from adjacent windings
                // as two tessellation branches.  They are one semantic anchor,
                // unlike genuine self-intersections at distinct principal
                // parameters, and must not manufacture ambiguity.
                let branch_candidate = if matches!(contact.domain, ContactDomain::Periodic { .. })
                    && contact.parameter == 0.0
                {
                    DraftCurveBranchCandidate::default()
                } else {
                    branch_candidate
                };
                AnchorKey::PointOnCurve(contact.span, branch_candidate)
            }
            Self::AffineSupport { contact, .. } => {
                AnchorKey::PointOnCurve(contact.span, DraftCurveBranchCandidate::from_ordinal(0))
            }
        }
    }

    fn model_position(self) -> [f64; 2] {
        match self {
            Self::PersistentPoint { model_position, .. }
            | Self::Midpoint { model_position, .. }
            | Self::CurvePoint { model_position, .. }
            | Self::AffineSupport { model_position, .. } => model_position,
        }
    }

    fn contact(self) -> Option<DraftCurveContact> {
        match self {
            Self::CurvePoint { contact, .. } | Self::AffineSupport { contact, .. } => Some(contact),
            Self::PersistentPoint { .. } | Self::Midpoint { .. } => None,
        }
    }

    fn affine_reference(self) -> Option<(CurveSpan, [f64; 2])> {
        match self {
            Self::Midpoint {
                span,
                affine_direction,
                ..
            } => Some((span, affine_direction)),
            Self::AffineSupport {
                contact,
                affine_direction,
                ..
            } => Some((contact.span, affine_direction)),
            Self::PersistentPoint { .. } | Self::CurvePoint { .. } => None,
        }
    }

    /// Whether this anchor can contribute after the pointer leaves its current
    /// contact neighborhood.
    ///
    /// Generic nonlinear curve contacts remain useful as immediate placement
    /// candidates, but they provide neither an affine direction nor a point
    /// tracking origin. Retaining them would therefore spend the bounded wake
    /// budget without making any later guide or relation available.
    const fn is_reusable_reference(self) -> bool {
        matches!(
            self,
            Self::PersistentPoint { .. } | Self::Midpoint { .. } | Self::AffineSupport { .. }
        )
    }

    fn anchor_priority(self) -> DraftAnchorPriority {
        match self {
            Self::PersistentPoint { .. } => DraftAnchorPriority::PointIdentity,
            Self::Midpoint { .. } => DraftAnchorPriority::Midpoint,
            Self::CurvePoint { .. } | Self::AffineSupport { .. } => {
                DraftAnchorPriority::PointOnCurve
            }
        }
    }

    fn role_priority(self, scope: GeometryPickScope) -> u8 {
        match self {
            Self::PersistentPoint { role_incidence, .. } => u8::from(match scope {
                GeometryPickScope::All | GeometryPickScope::Profile => !role_incidence.profile,
                GeometryPickScope::Construction => true,
            }),
            Self::Midpoint { role, .. }
            | Self::CurvePoint { role, .. }
            | Self::AffineSupport { role, .. } => u8::from(role == GeometryRole::Construction),
        }
    }

    fn inference_for_subject(
        self,
        subject: DraftInferenceSubject,
    ) -> Option<(DraftInferenceFamily, DraftInferenceRelation)> {
        match (subject, self) {
            (subject, Self::PersistentPoint { point, .. }) if subject.is_point_operand() => Some((
                DraftInferenceFamily::PointIdentity,
                DraftInferenceRelation::PointIdentity { point },
            )),
            (subject, Self::Midpoint { span, .. }) if subject.is_point_operand() => Some((
                DraftInferenceFamily::Midpoint,
                DraftInferenceRelation::Midpoint { span },
            )),
            (subject, Self::CurvePoint { contact, .. } | Self::AffineSupport { contact, .. })
                if subject.is_point_operand() =>
            {
                Some((
                    DraftInferenceFamily::PointOnCurve,
                    DraftInferenceRelation::PointOnCurve { contact },
                ))
            }
            (DraftInferenceSubject::CircleCircumference, Self::PersistentPoint { point, .. }) => {
                Some((
                    DraftInferenceFamily::PointOnCreatedCurve,
                    DraftInferenceRelation::PointOnCreatedCurve { point },
                ))
            }
            (
                DraftInferenceSubject::CircleCircumference,
                Self::Midpoint { .. } | Self::CurvePoint { .. } | Self::AffineSupport { .. },
            )
            | (
                DraftInferenceSubject::PointOperand
                | DraftInferenceSubject::CenteredPointOperand { .. },
                _,
            ) => None,
        }
    }
}

/// Semantic owner of the pointer coordinate currently being inferred.
///
/// A point operand may reuse an existing point or receive an ordinary
/// point-on-curve relation. A circle circumference click is not an allocated
/// point: snapping it to a persistent point instead means that point lies on
/// the newly created circle. Keeping these subjects distinct prevents a radius
/// sample from masquerading as structural point identity reuse.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DraftInferenceSubject {
    #[default]
    /// The coordinate becomes or structurally reuses a persistent point.
    PointOperand,
    /// The coordinate samples a prospective circle's radius without allocating a rim point.
    CircleCircumference,
    /// A persistent point operand that also centers the curve allocated by this
    /// construction. Ordinary point identity, midpoint, and point-on-curve
    /// inference remain eligible; exact accepted semantic centers additionally
    /// permit a Concentric candidate without structural point reuse.
    CenteredPointOperand { prospective_curve_index: usize },
}

impl DraftInferenceSubject {
    /// Whether the sampled coordinate is a persistent construction point.
    #[must_use]
    pub const fn is_point_operand(self) -> bool {
        matches!(self, Self::PointOperand | Self::CenteredPointOperand { .. })
    }

    /// Curve occurrence allocated around this point, when Concentric inference
    /// is semantically available.
    #[must_use]
    pub const fn prospective_centered_curve_index(self) -> Option<usize> {
        match self {
            Self::CenteredPointOperand {
                prospective_curve_index,
            } => Some(prospective_curve_index),
            Self::PointOperand | Self::CircleCircumference => None,
        }
    }
}

/// Normalized pointer sample for the current construction point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftInferenceSample {
    pub raw_screen_position: ScreenPoint,
    /// Semantic construction operand sampled by this pointer coordinate.
    pub subject: DraftInferenceSubject,
    /// Start of the live line/polyline span, when direction inference is
    /// semantically applicable.  Non-span construction stages leave this `None`.
    pub span_start: Option<[f64; 2]>,
}

/// One exact scene/frame input to the pure inference resolver.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftInferenceFrame {
    pub design_identity: SketchDesignIdentity,
    pub accepted_revision: u64,
    /// Exact accepted retained input when supplied by the scene owner.
    ///
    /// This participates in ephemeral inference invalidation and later plan
    /// authentication. Compatibility scenes may leave it absent, but cannot
    /// authorize inferred publication.
    pub prepared_input: Option<PreparedSketchInput>,
    pub viewport: Viewport,
    pub geometry_policy: GeometryInteractionPolicy,
    pub sample: DraftInferenceSample,
    pub anchors: Vec<DraftReferenceAnchor>,
    /// Exact accepted semantic centers. They are separate from point anchors:
    /// equality of a center coordinate never means structural point reuse.
    pub semantic_centers: Vec<DraftSemanticCenterAnchor>,
}

impl DraftInferenceFrame {
    /// Captures scene identity and viewport without transferring scene-owned
    /// rendering or picking state into the inference engine.
    #[must_use]
    pub fn from_scene(
        scene: &EditorScene,
        geometry_policy: GeometryInteractionPolicy,
        sample: DraftInferenceSample,
        anchors: Vec<DraftReferenceAnchor>,
    ) -> Self {
        Self {
            design_identity: scene.design_identity,
            accepted_revision: scene.accepted_revision,
            prepared_input: scene.authenticated_prepared_input(),
            viewport: scene.viewport,
            geometry_policy,
            sample,
            anchors,
            semantic_centers: Vec::new(),
        }
    }

    /// Captures a scene together with already bounded semantic-center inputs.
    ///
    /// Collection stays outside this pure frame constructor so suppression and
    /// subject-specific resource policy can avoid irrelevant scene traversal.
    #[must_use]
    pub fn from_scene_with_semantic_centers(
        scene: &EditorScene,
        geometry_policy: GeometryInteractionPolicy,
        sample: DraftInferenceSample,
        anchors: Vec<DraftReferenceAnchor>,
        semantic_centers: Vec<DraftSemanticCenterAnchor>,
    ) -> Self {
        Self {
            semantic_centers,
            ..Self::from_scene(scene, geometry_policy, sample, anchors)
        }
    }

    fn validate_sample(&self) -> Result<(), DraftInferenceError> {
        if !self.viewport.is_valid()
            || !self.sample.raw_screen_position.is_finite()
            || self
                .sample
                .span_start
                .is_some_and(|point| !point.into_iter().all(f64::is_finite))
        {
            return Err(DraftInferenceError::InvalidFrame);
        }
        Ok(())
    }

    fn validate_relevant_anchors(&self) -> Result<(), DraftInferenceError> {
        let invalid_ordinary = match self.sample.subject {
            DraftInferenceSubject::PointOperand
            | DraftInferenceSubject::CenteredPointOperand { .. } => self
                .anchors
                .iter()
                .copied()
                .any(|anchor| !anchor.is_valid()),
            DraftInferenceSubject::CircleCircumference => {
                self.anchors.iter().copied().any(|anchor| {
                    matches!(anchor, DraftReferenceAnchor::PersistentPoint { .. })
                        && !anchor.is_valid()
                })
            }
        };
        let invalid_centers = match self.sample.subject {
            DraftInferenceSubject::CenteredPointOperand { .. } => self
                .semantic_centers
                .iter()
                .copied()
                .any(|center| !center.is_valid()),
            DraftInferenceSubject::PointOperand | DraftInferenceSubject::CircleCircumference => {
                false
            }
        };
        if invalid_ordinary || invalid_centers {
            return Err(DraftInferenceError::InvalidFrame);
        }
        Ok(())
    }

    fn stamp(&self) -> FrameStamp {
        FrameStamp {
            design_identity: self.design_identity,
            accepted_revision: self.accepted_revision,
            prepared_input: self.prepared_input,
            viewport: self.viewport,
            geometry_policy: self.geometry_policy,
            subject: self.sample.subject,
        }
    }
}

/// Stable session-local identity of a prospective inference bundle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DraftInferenceCandidateId(u64);

impl DraftInferenceCandidateId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The existing semantic relation represented by one candidate component.
///
/// `PointIdentity` means construction must reuse the persistent point.  It is
/// not a request to add a redundant Coincident constraint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DraftInferenceRelation {
    PointIdentity {
        point: DesignPointId,
    },
    /// The prospective persistent point is coincident with the intrinsic
    /// sketch origin. No retained reference object is allocated.
    CoincidentWithOrigin,
    /// The prospective persistent point lies on one intrinsic Cartesian axis.
    PointOnDatumAxis {
        axis: DocumentCoordinateAxis,
    },
    PointOnCurve {
        contact: DraftCurveContact,
    },
    /// A persistent point lies on the curve allocated by this construction.
    ///
    /// This is the allocation-order reverse of [`Self::PointOnCurve`], not
    /// structural point reuse and not a hidden rim point.
    PointOnCreatedCurve {
        point: DesignPointId,
    },
    Midpoint {
        span: CurveSpan,
    },
    Horizontal,
    Vertical,
    Parallel {
        reference: CurveSpan,
    },
    Perpendicular {
        reference: CurveSpan,
    },
    HorizontalPoints {
        reference: DesignPointId,
    },
    VerticalPoints {
        reference: DesignPointId,
    },
    HorizontalPointToMidpoint {
        reference: CurveSpan,
    },
    VerticalPointToMidpoint {
        reference: CurveSpan,
    },
    Concentric {
        reference: CurveId,
        prospective_curve_index: usize,
    },
    Collinear {
        reference: CurveSpan,
    },
}

/// Relation family exposed to policy and presentation consumers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftInferenceFamily {
    PointIdentity,
    DatumOrigin,
    DatumAxis,
    PointOnCurve,
    PointOnCreatedCurve,
    Midpoint,
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    HorizontalPoints,
    VerticalPoints,
    HorizontalPointToMidpoint,
    VerticalPointToMidpoint,
    Concentric,
    Collinear,
    PointTracking,
}

/// Whether a guide denotes durable intent or ephemeral tracking only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftGuideClassification {
    TrackingOnly,
    ConstraintBacked,
}

/// Semantic guide shape.  Presentation maps these model coordinates through
/// the same viewport used to produce the inference frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DraftGuideGeometry {
    Point { position: [f64; 2] },
    Segment { start: [f64; 2], end: [f64; 2] },
}

/// Stable identity for a guide within one candidate or tracking resolution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DraftGuideId {
    pub candidate: Option<DraftInferenceCandidateId>,
    pub ordinal: u32,
}

/// One typed presentation-neutral inference guide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftGuide {
    pub id: DraftGuideId,
    pub family: DraftInferenceFamily,
    pub classification: DraftGuideClassification,
    pub geometry: DraftGuideGeometry,
    pub reference: Option<DraftReferenceAnchor>,
}

impl DraftGuide {
    fn is_valid(self) -> bool {
        let finite_geometry = match self.geometry {
            DraftGuideGeometry::Point { position } => position.into_iter().all(f64::is_finite),
            DraftGuideGeometry::Segment { start, end } => {
                start.into_iter().chain(end).all(f64::is_finite)
            }
        };
        finite_geometry && self.reference.is_none_or(DraftReferenceAnchor::is_valid)
    }
}

/// Point-anchor precedence used in deterministic ranking evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftAnchorPriority {
    SemanticCenter,
    PointIdentity,
    Midpoint,
    PointOnCurve,
    DatumOrigin,
    DatumAxis,
    None,
}

impl DraftAnchorPriority {
    const fn rank(self) -> u8 {
        match self {
            Self::PointIdentity => 0,
            Self::SemanticCenter => 1,
            Self::Midpoint => 2,
            Self::PointOnCurve => 3,
            Self::DatumOrigin => 4,
            Self::DatumAxis => 5,
            Self::None => 6,
        }
    }
}

/// Direction precedence used in deterministic ranking evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftDirectionPriority {
    RememberedReference,
    WorldAxis,
    None,
}

impl DraftDirectionPriority {
    const fn rank(self) -> u8 {
        match self {
            Self::RememberedReference => 0,
            Self::WorldAxis => 1,
            Self::None => 2,
        }
    }
}

/// Public, reproducible lexicographic ranking evidence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftInferenceRankingEvidence {
    pub constraint_backed: bool,
    pub persistent_relation_count: u8,
    pub anchor_priority: DraftAnchorPriority,
    pub direction_priority: DraftDirectionPriority,
    /// Positional-source role: `0` is Profile, `1` is Construction and `2`
    /// means this candidate has no positional geometry source.
    pub positional_geometry_role_priority: u8,
    /// Directional-reference role: `0` is Profile, `1` is Construction and
    /// `2` means this candidate has no directional geometry source.
    pub directional_geometry_role_priority: u8,
    pub distance_pixels: f64,
    pub angular_error_radians: f64,
}

impl DraftInferenceRankingEvidence {
    fn is_valid(self) -> bool {
        self.distance_pixels.is_finite()
            && self.distance_pixels >= 0.0
            && self.angular_error_radians.is_finite()
            && self.angular_error_radians >= 0.0
            && self.angular_error_radians <= std::f64::consts::FRAC_PI_2
    }

    fn compare_for_subject(self, other: Self, subject: DraftInferenceSubject) -> Ordering {
        // `true` is preferred and every semantic/error rank is
        // smaller-is-better.  Relation count is published as evidence but is
        // deliberately not a discriminator: an incidental extra relation
        // cannot displace semantic anchor/direction intent. Persistent IDs
        // never resolve a semantic tie.
        other
            .constraint_backed
            .cmp(&self.constraint_backed)
            .then_with(|| {
                subject_anchor_rank(subject, self.anchor_priority)
                    .cmp(&subject_anchor_rank(subject, other.anchor_priority))
            })
            .then_with(|| {
                self.direction_priority
                    .rank()
                    .cmp(&other.direction_priority.rank())
            })
            .then_with(|| compare_positional_role_aware_distance(self, other))
            .then_with(|| {
                self.directional_geometry_role_priority
                    .cmp(&other.directional_geometry_role_priority)
            })
            .then_with(|| {
                self.angular_error_radians
                    .total_cmp(&other.angular_error_radians)
            })
    }
}

const fn subject_anchor_rank(subject: DraftInferenceSubject, priority: DraftAnchorPriority) -> u8 {
    match (subject, priority) {
        // A center operand expresses a semantic-center relationship more
        // precisely than structural reuse of the point that happens to store
        // that center. Every other point operand retains the ordinary M70
        // anchor order.
        (
            DraftInferenceSubject::CenteredPointOperand { .. },
            DraftAnchorPriority::SemanticCenter,
        ) => 0,
        (
            DraftInferenceSubject::CenteredPointOperand { .. },
            DraftAnchorPriority::PointIdentity,
        ) => 1,
        _ => priority.rank(),
    }
}

/// One complete prospective anchor/direction bundle.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftInferenceCandidate {
    pub id: DraftInferenceCandidateId,
    pub raw_model_position: [f64; 2],
    pub adjusted_model_position: [f64; 2],
    pub raw_screen_position: ScreenPoint,
    pub adjusted_screen_position: ScreenPoint,
    pub relations: Vec<DraftInferenceRelation>,
    pub references: Vec<DraftReferenceAnchor>,
    pub guides: Vec<DraftGuide>,
    pub ranking: DraftInferenceRankingEvidence,
}

impl DraftInferenceCandidate {
    fn is_valid(&self) -> bool {
        self.raw_model_position.into_iter().all(f64::is_finite)
            && self.adjusted_model_position.into_iter().all(f64::is_finite)
            && self.raw_screen_position.is_finite()
            && self.adjusted_screen_position.is_finite()
            && self
                .references
                .iter()
                .copied()
                .all(DraftReferenceAnchor::is_valid)
            && self.guides.iter().copied().all(DraftGuide::is_valid)
            && self
                .relations
                .iter()
                .copied()
                .all(|relation| match relation {
                    DraftInferenceRelation::PointOnCurve { contact } => contact.is_valid(),
                    DraftInferenceRelation::PointIdentity { .. }
                    | DraftInferenceRelation::CoincidentWithOrigin
                    | DraftInferenceRelation::PointOnDatumAxis { .. }
                    | DraftInferenceRelation::PointOnCreatedCurve { .. }
                    | DraftInferenceRelation::Midpoint { .. }
                    | DraftInferenceRelation::Horizontal
                    | DraftInferenceRelation::Vertical
                    | DraftInferenceRelation::Parallel { .. }
                    | DraftInferenceRelation::Perpendicular { .. }
                    | DraftInferenceRelation::HorizontalPoints { .. }
                    | DraftInferenceRelation::VerticalPoints { .. }
                    | DraftInferenceRelation::HorizontalPointToMidpoint { .. }
                    | DraftInferenceRelation::VerticalPointToMidpoint { .. }
                    | DraftInferenceRelation::Concentric { .. }
                    | DraftInferenceRelation::Collinear { .. } => true,
                })
            && self.ranking.is_valid()
    }
}

/// Whether candidate enumeration completed within the configured bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftInferenceCompleteness {
    Complete,
    /// Candidate generation stopped immediately after observing this many
    /// unique semantic bundles. `required` is the first proven lower bound
    /// above `limit`, not an unbounded full-scene count.
    CandidateLimit {
        required: usize,
        limit: usize,
    },
    SceneLimit(DraftInferenceSceneLimit),
}

/// Resolution outcome.  Ambiguity and stale explicit preference never silently
/// fall through to another candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DraftInferenceStatus {
    None,
    Resolved {
        candidate: DraftInferenceCandidateId,
    },
    Ambiguous {
        candidates: Vec<DraftInferenceCandidateId>,
    },
    Suppressed,
    ResourceLimited,
    StalePreferredCandidate {
        preferred: DraftInferenceCandidateId,
    },
}

/// Complete inference publication for one pointer sample.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftInferenceResolution {
    pub status: DraftInferenceStatus,
    pub completeness: DraftInferenceCompleteness,
    pub raw_model_position: [f64; 2],
    pub adjusted_model_position: [f64; 2],
    pub raw_screen_position: ScreenPoint,
    pub adjusted_screen_position: ScreenPoint,
    pub candidates: Vec<DraftInferenceCandidate>,
    pub guides: Vec<DraftGuide>,
}

/// Typed input/policy/resource failure.  The engine never turns malformed
/// coordinates into a no-inference success.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum DraftInferenceError {
    #[error("invalid drafting-inference policy")]
    InvalidPolicy,
    #[error("drafting-inference input or derived output contains invalid or non-finite geometry")]
    InvalidFrame,
    #[error("drafting-inference candidate identity space is exhausted")]
    CandidateIdentityExhausted,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AnchorKey {
    Point(DesignPointId),
    SemanticCenter(CurveId),
    Midpoint(CurveSpan),
    PointOnCurve(CurveSpan, DraftCurveBranchCandidate),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DirectionKey {
    Horizontal,
    Vertical,
    Parallel(CurveSpan),
    Perpendicular(CurveSpan),
    Collinear(CurveSpan),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PointTrackingAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DatumKey {
    Origin,
    XAxis,
    YAxis,
}

impl DatumKey {
    const fn coordinate_axis(self) -> Option<DocumentCoordinateAxis> {
        match self {
            Self::Origin => None,
            Self::XAxis => Some(DocumentCoordinateAxis::X),
            Self::YAxis => Some(DocumentCoordinateAxis::Y),
        }
    }

    const fn tracking_axis(self) -> Option<PointTrackingAxis> {
        match self {
            Self::Origin => None,
            Self::XAxis => Some(PointTrackingAxis::Horizontal),
            Self::YAxis => Some(PointTrackingAxis::Vertical),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PointTrackingKey {
    anchor: AnchorKey,
    axis: PointTrackingAxis,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CandidateKey {
    anchor: Option<AnchorKey>,
    datum: Option<DatumKey>,
    point_tracking: Option<PointTrackingKey>,
    secondary_point_tracking: Option<PointTrackingKey>,
    direction: Option<DirectionKey>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameStamp {
    design_identity: SketchDesignIdentity,
    accepted_revision: u64,
    prepared_input: Option<PreparedSketchInput>,
    viewport: Viewport,
    geometry_policy: GeometryInteractionPolicy,
    subject: DraftInferenceSubject,
}

#[derive(Clone, Copy, Debug)]
struct AnchorWork {
    anchor: DraftReferenceAnchor,
    key: AnchorKey,
    family: DraftInferenceFamily,
    behavior: DraftInferenceBehavior,
    distance_pixels: f64,
}

#[derive(Clone, Copy, Debug)]
struct DirectionWork {
    key: DirectionKey,
    behavior: DraftInferenceBehavior,
    cartesian_axis: Option<PointTrackingAxis>,
    adjusted_position: [f64; 2],
    target_is_on_axis: bool,
    angular_error: f64,
    reference: Option<DraftReferenceAnchor>,
    priority: DraftDirectionPriority,
    role_priority: u8,
}

#[derive(Clone, Copy, Debug)]
struct PointTrackingWork {
    key: PointTrackingKey,
    reference: DraftReferenceAnchor,
    origin: [f64; 2],
    relation: DraftInferenceRelation,
    family: DraftInferenceFamily,
    adjusted_position: [f64; 2],
    angular_error: f64,
}

#[derive(Clone, Copy, Debug)]
struct DatumWork {
    key: DatumKey,
    behavior: DraftInferenceBehavior,
    adjusted_position: [f64; 2],
    distance_pixels: f64,
}

#[derive(Clone, Debug)]
struct CandidateWork {
    key: CandidateKey,
    candidate: DraftInferenceCandidate,
}

/// Stateful, bounded drafting-inference resolver.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftInferenceEngine {
    policy: DraftInferencePolicy,
    frame_stamp: Option<FrameStamp>,
    remembered_references: Vec<DraftReferenceAnchor>,
    active_anchor: Option<AnchorKey>,
    active_datum: Option<DatumKey>,
    active_direction: Option<DirectionKey>,
    active_concentric: Option<CurveId>,
    active_point_tracking: BTreeSet<PointTrackingKey>,
    candidate_ids: Vec<(CandidateKey, DraftInferenceCandidateId)>,
    next_candidate_id: u64,
}

impl Default for DraftInferenceEngine {
    fn default() -> Self {
        Self {
            policy: DraftInferencePolicy::default(),
            frame_stamp: None,
            remembered_references: Vec::new(),
            active_anchor: None,
            active_datum: None,
            active_direction: None,
            active_concentric: None,
            active_point_tracking: BTreeSet::new(),
            candidate_ids: Vec::new(),
            next_candidate_id: 1,
        }
    }
}

impl DraftInferenceEngine {
    /// Creates an engine with a validated explicit policy.
    ///
    /// # Errors
    ///
    /// Returns [`DraftInferenceError::InvalidPolicy`] for invalid thresholds or limits.
    pub fn new(policy: DraftInferencePolicy) -> Result<Self, DraftInferenceError> {
        policy.validate()?;
        Ok(Self {
            policy,
            ..Self::default()
        })
    }

    #[must_use]
    pub const fn policy(&self) -> DraftInferencePolicy {
        self.policy
    }

    /// Replaces policy atomically and clears state acquired under the old one.
    ///
    /// # Errors
    ///
    /// Returns [`DraftInferenceError::InvalidPolicy`] without changing state.
    pub fn set_policy(&mut self, policy: DraftInferencePolicy) -> Result<(), DraftInferenceError> {
        policy.validate()?;
        if self.policy != policy {
            self.policy = policy;
            self.clear_session();
        }
        Ok(())
    }

    /// Returns bounded reusable semantic anchors in deterministic retention
    /// order. Explicit sequential wakes are oldest-to-newest; simultaneous
    /// scene wakes place stronger semantic priority later. Immediate-only
    /// nonlinear curve contacts are intentionally absent.
    #[must_use]
    pub fn remembered_references(&self) -> &[DraftReferenceAnchor] {
        &self.remembered_references
    }

    /// Explicitly wakes one semantic reference, for example after confirming a
    /// midpoint as the start of the next polyline span.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or malformed reference geometry.
    pub fn remember_reference(
        &mut self,
        reference: DraftReferenceAnchor,
    ) -> Result<(), DraftInferenceError> {
        if !reference.is_valid() {
            return Err(DraftInferenceError::InvalidFrame);
        }
        if reference.is_reusable_reference() {
            self.remember_valid_reference(reference);
        }
        Ok(())
    }

    /// Clears stage-local candidates, latches and wake memory.  Call this on a
    /// confirmed stage, Escape/cancel, tool exit, Undo/Redo or explicit reload.
    pub fn clear_stage(&mut self) {
        self.remembered_references.clear();
        self.active_anchor = None;
        self.active_datum = None;
        self.active_direction = None;
        self.active_concentric = None;
        self.active_point_tracking.clear();
        self.candidate_ids.clear();
    }

    /// Clears every session stamp and transient identity.
    pub fn clear_session(&mut self) {
        self.clear_stage();
        self.frame_stamp = None;
    }

    /// Resolves one normalized pointer sample.
    ///
    /// Suppression acquires no new reference, publishes no guide, clears the
    /// complete stage-local wake/latch state and returns the raw coordinate.  A
    /// changed design/revision or viewport clears all previous ephemeral memory
    /// before resolution.
    ///
    /// # Errors
    ///
    /// Rejects invalid policy/frame data or exhausted candidate identities.
    pub fn resolve(
        &mut self,
        frame: &DraftInferenceFrame,
        input: DraftInferenceInput,
    ) -> Result<DraftInferenceResolution, DraftInferenceError> {
        let mut staged = self.clone();
        let resolution = staged.resolve_staged(frame, input)?;
        *self = staged;
        Ok(resolution)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "candidate generation and fail-closed resolution are one auditable state transition"
    )]
    fn resolve_staged(
        &mut self,
        frame: &DraftInferenceFrame,
        input: DraftInferenceInput,
    ) -> Result<DraftInferenceResolution, DraftInferenceError> {
        self.policy.validate()?;
        frame.validate_sample()?;

        let raw_screen = frame.sample.raw_screen_position;
        let raw_model = frame.viewport.screen_to_model(raw_screen);
        if !raw_model.into_iter().all(f64::is_finite) {
            return Err(DraftInferenceError::InvalidFrame);
        }

        let stamp = frame.stamp();
        if self.frame_stamp.is_some_and(|current| current != stamp) {
            self.clear_session();
        }
        self.frame_stamp = Some(stamp);

        if input.suppressed {
            self.clear_stage();
            return Ok(empty_resolution(
                DraftInferenceStatus::Suppressed,
                raw_model,
                raw_screen,
            ));
        }

        // Enforce the public-engine boundary independently of EditorScene's
        // bounded collector. Count only semantic inputs relevant to this
        // subject, before traversing, sorting or allocating them. Suppression
        // above is intentionally traversal-free and never resource-limited.
        let semantic_anchor_count = match frame.sample.subject {
            DraftInferenceSubject::PointOperand => frame.anchors.len(),
            DraftInferenceSubject::CircleCircumference => frame
                .anchors
                .iter()
                .filter(|anchor| matches!(anchor, DraftReferenceAnchor::PersistentPoint { .. }))
                .count(),
            DraftInferenceSubject::CenteredPointOperand { .. } => frame
                .anchors
                .len()
                .saturating_add(frame.semantic_centers.len()),
        };
        if semantic_anchor_count > self.policy.limits.max_scene_anchors {
            self.clear_stage();
            return Ok(scene_limit_resolution(
                DraftInferenceSceneLimit {
                    resource: DraftInferenceSceneResource::Anchors,
                    required: semantic_anchor_count,
                    limit: self.policy.limits.max_scene_anchors,
                },
                raw_model,
                raw_screen,
            ));
        }
        frame.validate_relevant_anchors()?;

        let mut eligible_anchors = self.eligible_anchors(frame, raw_screen);
        deduplicate_anchor_works(&mut eligible_anchors, frame.geometry_policy.scope);

        let wake_references: Vec<_> = if frame.sample.subject.is_point_operand() {
            eligible_anchors
                .iter()
                .filter(|work| {
                    work.anchor.is_reusable_reference()
                        && work.distance_pixels <= self.enter_distance(work.family)
                })
                .copied()
                .collect()
        } else {
            Vec::new()
        };

        let active_anchor = self.active_anchor.and_then(|active| {
            eligible_anchors.iter().copied().find(|work| {
                work.key == active && work.distance_pixels <= self.exit_distance(work.family)
            })
        });
        let higher_priority_anchor_entered = active_anchor.is_some_and(|active| {
            eligible_anchors.iter().any(|work| {
                work.distance_pixels <= self.enter_distance(work.family)
                    && compare_wake_reference_priority(*work, active, frame.geometry_policy.scope)
                        == Ordering::Less
            })
        });
        if active_anchor.is_some() && !higher_priority_anchor_entered {
            let active = self.active_anchor;
            eligible_anchors.retain(|work| Some(work.key) == active);
        } else {
            self.active_anchor = None;
            eligible_anchors
                .retain(|work| work.distance_pixels <= self.enter_distance(work.family));
        }

        let retained_direction = frame
            .sample
            .subject
            .is_point_operand()
            .then(|| self.retained_direction(frame, raw_model))
            .flatten();
        if retained_direction.is_none() {
            self.active_direction = None;
        }

        let mut works = Vec::new();
        let mut standalone_guides = Vec::new();
        let mut centered_generation_complete = true;
        if let Some(prospective_curve_index) =
            frame.sample.subject.prospective_centered_curve_index()
        {
            let (candidates, complete) = self.concentric_candidates(
                frame,
                raw_model,
                raw_screen,
                prospective_curve_index,
                self.policy.limits.max_candidates,
            );
            works = candidates;
            centered_generation_complete = complete;
        }
        let no_anchor: Option<AnchorWork> = None;
        let mut generation_complete = centered_generation_complete;
        if generation_complete {
            generation_complete = self.generate_candidate_family(
                frame,
                raw_model,
                raw_screen,
                no_anchor,
                retained_direction,
                &mut works,
                &mut standalone_guides,
                self.policy.limits.max_candidates,
            );
        }
        for anchor in eligible_anchors.iter().copied() {
            if !generation_complete {
                break;
            }
            generation_complete = self.generate_candidate_family(
                frame,
                raw_model,
                raw_screen,
                Some(anchor),
                retained_direction,
                &mut works,
                &mut standalone_guides,
                self.policy.limits.max_candidates,
            );
        }

        if generation_complete
            && !self.generate_datum_candidates(
                frame,
                raw_model,
                raw_screen,
                retained_direction,
                &mut standalone_guides,
                &mut works,
                self.policy.limits.max_candidates,
            )
        {
            generation_complete = false;
        }

        if generation_complete
            && frame.sample.subject.is_point_operand()
            && !self.generate_point_tracking_candidates(
                frame,
                raw_model,
                raw_screen,
                retained_direction,
                &mut standalone_guides,
                &mut works,
                self.policy.limits.max_candidates,
            )
        {
            generation_complete = false;
        }
        if !generation_complete {
            self.active_anchor = None;
            self.active_datum = None;
            self.active_direction = None;
            self.active_concentric = None;
            self.active_point_tracking.clear();
            self.candidate_ids.clear();
            return Ok(DraftInferenceResolution {
                status: DraftInferenceStatus::ResourceLimited,
                completeness: DraftInferenceCompleteness::CandidateLimit {
                    required: self.policy.limits.max_candidates.saturating_add(1),
                    limit: self.policy.limits.max_candidates,
                },
                raw_model_position: raw_model,
                adjusted_model_position: raw_model,
                raw_screen_position: raw_screen,
                adjusted_screen_position: raw_screen,
                candidates: Vec::new(),
                guides: Vec::new(),
            });
        }
        // Scaling keeps direction projection stable for ordinary magnitudes,
        // but translating the projected delta back near an f64 extreme can
        // still overflow. Validate the complete semantic output before IDs,
        // wake memory, or any convergence-like resolution can be published.
        if works.iter().any(|work| !work.candidate.is_valid())
            || standalone_guides
                .iter()
                .copied()
                .any(|guide| !guide.is_valid())
        {
            return Err(DraftInferenceError::InvalidFrame);
        }
        assign_standalone_guide_ids(&mut standalone_guides);

        self.remember_wake_references(
            wake_references,
            frame.viewport,
            raw_screen,
            frame.geometry_policy.scope,
        );

        self.assign_candidate_ids(&mut works)?;
        for work in &mut works {
            assign_guide_ids(&mut work.candidate);
        }
        works.sort_by(|first, second| {
            first
                .candidate
                .ranking
                .compare_for_subject(second.candidate.ranking, frame.sample.subject)
                .then_with(|| first.key.cmp(&second.key))
        });

        if works.is_empty() {
            self.active_anchor = None;
            self.active_direction = None;
            self.active_concentric = None;
            self.candidate_ids.clear();
            return Ok(DraftInferenceResolution {
                guides: standalone_guides,
                ..empty_resolution(DraftInferenceStatus::None, raw_model, raw_screen)
            });
        }

        let selected_index = if let Some(preferred) = input.preferred_candidate {
            let Some(index) = works.iter().position(|work| work.candidate.id == preferred) else {
                self.clear_stage();
                return Ok(DraftInferenceResolution {
                    status: DraftInferenceStatus::StalePreferredCandidate { preferred },
                    completeness: DraftInferenceCompleteness::Complete,
                    raw_model_position: raw_model,
                    adjusted_model_position: raw_model,
                    raw_screen_position: raw_screen,
                    adjusted_screen_position: raw_screen,
                    candidates: works.into_iter().map(|work| work.candidate).collect(),
                    guides: standalone_guides,
                });
            };
            index
        } else {
            let best = works[0].candidate.ranking;
            let tied: Vec<_> = works
                .iter()
                .take_while(|work| {
                    work.candidate
                        .ranking
                        .compare_for_subject(best, frame.sample.subject)
                        == Ordering::Equal
                })
                .map(|work| work.candidate.id)
                .collect();
            if tied.len() > 1 {
                self.active_anchor = None;
                self.active_datum = None;
                self.active_direction = None;
                self.active_concentric = None;
                let mut guides = standalone_guides;
                for work in works.iter().take(tied.len()) {
                    guides.extend(work.candidate.guides.iter().copied());
                }
                return Ok(DraftInferenceResolution {
                    status: DraftInferenceStatus::Ambiguous { candidates: tied },
                    completeness: DraftInferenceCompleteness::Complete,
                    raw_model_position: raw_model,
                    adjusted_model_position: raw_model,
                    raw_screen_position: raw_screen,
                    adjusted_screen_position: raw_screen,
                    candidates: works.into_iter().map(|work| work.candidate).collect(),
                    guides,
                });
            }
            0
        };

        let selected_key = works[selected_index].key;
        let selected = &works[selected_index].candidate;
        self.active_anchor = selected_key.anchor;
        self.active_datum = selected_key.datum;
        self.active_direction = selected_key.direction;
        self.active_concentric = match selected_key.anchor {
            Some(AnchorKey::SemanticCenter(center)) => Some(center),
            Some(AnchorKey::Point(_) | AnchorKey::Midpoint(_) | AnchorKey::PointOnCurve(_, _))
            | None => None,
        };
        let mut guides = standalone_guides;
        guides.extend(selected.guides.iter().copied());
        let resolution = DraftInferenceResolution {
            status: DraftInferenceStatus::Resolved {
                candidate: selected.id,
            },
            completeness: DraftInferenceCompleteness::Complete,
            raw_model_position: raw_model,
            adjusted_model_position: selected.adjusted_model_position,
            raw_screen_position: raw_screen,
            adjusted_screen_position: selected.adjusted_screen_position,
            candidates: works.into_iter().map(|work| work.candidate).collect(),
            guides,
        };
        Ok(resolution)
    }

    fn eligible_anchors(
        &self,
        frame: &DraftInferenceFrame,
        raw_screen: ScreenPoint,
    ) -> Vec<AnchorWork> {
        frame
            .anchors
            .iter()
            .copied()
            .filter(|anchor| anchor.is_interactive(frame.geometry_policy))
            .filter_map(|anchor| {
                let (family, _) = anchor.inference_for_subject(frame.sample.subject)?;
                let behavior = self.behavior(family);
                let screen_position = frame.viewport.model_to_screen(anchor.model_position());
                Some(AnchorWork {
                    anchor,
                    key: anchor.key(),
                    family,
                    behavior,
                    distance_pixels: screen_distance(raw_screen, screen_position),
                })
            })
            .filter(|work| {
                work.behavior.show_guides
                    || work.behavior.has_effect()
                    || work.anchor.affine_reference().is_some()
                    || matches!(work.anchor, DraftReferenceAnchor::PersistentPoint { .. })
            })
            .collect()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "semantic-center grouping, hysteresis, ranking, and bounded publication remain reviewable as one candidate pipeline"
    )]
    fn concentric_candidates(
        &self,
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
        raw_screen: ScreenPoint,
        prospective_curve_index: usize,
        max_candidates: usize,
    ) -> (Vec<CandidateWork>, bool) {
        let behavior = self.policy.concentric;
        if !behavior.show_guides && !behavior.has_effect() {
            return (Vec::new(), true);
        }
        let threshold = |center| {
            if self.active_concentric == Some(center) {
                self.policy.tolerances.point_exit_pixels
            } else {
                self.policy.tolerances.point_enter_pixels
            }
        };
        // A curve may have several scene occurrences, but its persistent curve
        // identity owns the retained Concentric operand and its lifecycle.
        // Distinct curves remain distinct candidates even when they resolve to
        // one stored center; persistent IDs never break the resulting semantic
        // tie. Coordinate proximity is never an identity relation.
        let mut by_curve = BTreeMap::<CurveId, (DraftSemanticCenterAnchor, f64)>::new();
        for center in frame
            .semantic_centers
            .iter()
            .copied()
            .filter(|center| center.is_interactive(frame.geometry_policy))
        {
            let distance = screen_distance(
                raw_screen,
                frame.viewport.model_to_screen(center.model_position),
            );
            if distance > threshold(center.curve) {
                continue;
            }
            let replace = by_curve
                .get(&center.curve)
                .is_none_or(|(current, current_distance)| {
                    u8::from(center.role == GeometryRole::Construction)
                        .cmp(&u8::from(current.role == GeometryRole::Construction))
                        .then_with(|| distance.total_cmp(current_distance))
                        .then_with(|| center.curve.cmp(&current.curve))
                        == Ordering::Less
                });
            if replace {
                by_curve.insert(center.curve, (center, distance));
                // Distinct retained curve operands only increase during this
                // traversal. Once the public candidate budget is exceeded, no
                // later occurrence can make the complete result fit.
                if by_curve.len() > max_candidates {
                    return (Vec::new(), false);
                }
            }
        }
        let mut centers = by_curve.into_values().collect::<Vec<_>>();
        centers.sort_by(|(first, first_distance), (second, second_distance)| {
            first_distance
                .total_cmp(second_distance)
                .then_with(|| {
                    u8::from(first.role == GeometryRole::Construction)
                        .cmp(&u8::from(second.role == GeometryRole::Construction))
                })
                .then_with(|| first.center.cmp(&second.center))
                .then_with(|| first.curve.cmp(&second.curve))
        });
        let candidates = centers
            .into_iter()
            .map(|(center, distance_pixels)| {
                let adjusted = if behavior.adjust_coordinates {
                    center.model_position
                } else {
                    raw_model
                };
                let relation = DraftInferenceRelation::Concentric {
                    reference: center.curve,
                    prospective_curve_index,
                };
                let guide = DraftGuide {
                    id: DraftGuideId {
                        candidate: None,
                        ordinal: 0,
                    },
                    family: DraftInferenceFamily::Concentric,
                    classification: behavior.guide_classification(),
                    geometry: DraftGuideGeometry::Point {
                        position: center.model_position,
                    },
                    reference: None,
                };
                CandidateWork {
                    key: CandidateKey {
                        anchor: Some(AnchorKey::SemanticCenter(center.curve)),
                        datum: None,
                        point_tracking: None,
                        secondary_point_tracking: None,
                        direction: None,
                    },
                    candidate: DraftInferenceCandidate {
                        id: DraftInferenceCandidateId(0),
                        raw_model_position: raw_model,
                        adjusted_model_position: adjusted,
                        raw_screen_position: raw_screen,
                        adjusted_screen_position: frame.viewport.model_to_screen(adjusted),
                        relations: behavior
                            .persist_constraint
                            .then_some(relation)
                            .into_iter()
                            .collect(),
                        references: Vec::new(),
                        guides: behavior.show_guides.then_some(guide).into_iter().collect(),
                        ranking: DraftInferenceRankingEvidence {
                            constraint_backed: behavior.persist_constraint,
                            persistent_relation_count: u8::from(behavior.persist_constraint),
                            anchor_priority: DraftAnchorPriority::SemanticCenter,
                            direction_priority: DraftDirectionPriority::None,
                            positional_geometry_role_priority: u8::from(
                                center.role == GeometryRole::Construction,
                            ),
                            directional_geometry_role_priority: 2,
                            distance_pixels,
                            angular_error_radians: 0.0,
                        },
                    },
                }
            })
            .collect();
        (candidates, true)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one bounded candidate-family transition keeps its input/output state explicit"
    )]
    fn generate_candidate_family(
        &self,
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
        raw_screen: ScreenPoint,
        anchor: Option<AnchorWork>,
        retained_direction: Option<DirectionKey>,
        works: &mut Vec<CandidateWork>,
        standalone_guides: &mut Vec<DraftGuide>,
        candidate_limit: usize,
    ) -> bool {
        let anchor_effect = anchor.is_some_and(|work| work.behavior.has_effect());
        if let Some(anchor) = anchor {
            if anchor_effect {
                if !push_candidate_bounded(
                    works,
                    Self::build_candidate(frame, raw_model, raw_screen, Some(anchor), None),
                    candidate_limit,
                    frame.sample.subject,
                ) {
                    return false;
                }
            } else if anchor.behavior.show_guides {
                standalone_guides.push(anchor_guide(None, 0, anchor));
            }
        }

        if anchor.is_some() && !anchor_effect {
            return true;
        }

        let direction_target = anchor.map_or(raw_model, |work| work.anchor.model_position());
        let directions = self.direction_works(frame, direction_target, retained_direction);
        for direction in directions {
            if direction.behavior.has_effect() {
                let compatible_anchor = anchor.filter(|work| {
                    anchor_effect
                        && (!work.behavior.adjust_coordinates
                            || !direction.behavior.adjust_coordinates
                            || direction.target_is_on_axis)
                });
                if anchor.is_some() && compatible_anchor.is_none() {
                    continue;
                }
                if !push_candidate_bounded(
                    works,
                    Self::build_candidate(
                        frame,
                        raw_model,
                        raw_screen,
                        compatible_anchor,
                        Some(direction),
                    ),
                    candidate_limit,
                    frame.sample.subject,
                ) {
                    return false;
                }
            } else if direction.behavior.show_guides && anchor.is_none() {
                standalone_guides.push(direction_guide(
                    None,
                    0,
                    frame.sample.span_start,
                    direction,
                ));
            }
        }
        true
    }

    fn build_candidate(
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
        raw_screen: ScreenPoint,
        anchor: Option<AnchorWork>,
        direction: Option<DirectionWork>,
    ) -> CandidateWork {
        let key = CandidateKey {
            anchor: anchor.map(|work| work.key),
            datum: None,
            point_tracking: None,
            secondary_point_tracking: None,
            direction: direction.map(|work| work.key),
        };
        let mut adjusted = raw_model;
        if let Some(anchor) = anchor
            && anchor.behavior.adjust_coordinates
        {
            adjusted = anchor.anchor.model_position();
        }
        if let Some(direction) = direction
            && direction.behavior.adjust_coordinates
            && !anchor.is_some_and(|work| work.behavior.adjust_coordinates)
        {
            adjusted = direction.adjusted_position;
        }

        let mut relations = Vec::new();
        let mut references = Vec::new();
        let mut guides = Vec::new();
        if let Some(anchor) = anchor {
            references.push(anchor.anchor);
            if anchor.behavior.persist_constraint {
                let (_, relation) = anchor
                    .anchor
                    .inference_for_subject(frame.sample.subject)
                    .expect("eligible anchor must retain its frame-subject relation");
                relations.push(relation);
            }
            if anchor.behavior.show_guides {
                guides.push(anchor_guide(None, 0, anchor));
            }
        }
        if let Some(direction) = direction {
            if let Some(reference) = direction.reference
                && !references.contains(&reference)
            {
                references.push(reference);
            }
            if direction.behavior.persist_constraint {
                relations.push(direction_relation(direction.key));
            }
            if direction.behavior.show_guides {
                guides.push(direction_guide(
                    None,
                    u32::try_from(guides.len())
                        .expect("candidate guide count is structurally bounded"),
                    frame.sample.span_start,
                    direction,
                ));
            }
        }
        let persistent_relation_count = u8::try_from(relations.len()).unwrap_or(u8::MAX);
        let constraint_backed = persistent_relation_count > 0;
        let distance_pixels = anchor.map_or(0.0, |work| work.distance_pixels);
        let angular_error_radians = direction.map_or(0.0, |work| work.angular_error);
        let positional_geometry_role_priority = anchor.map_or(2, |work| {
            work.anchor.role_priority(frame.geometry_policy.scope)
        });
        let directional_geometry_role_priority = direction.map_or(2, |work| work.role_priority);
        CandidateWork {
            key,
            candidate: DraftInferenceCandidate {
                id: DraftInferenceCandidateId(0),
                raw_model_position: raw_model,
                adjusted_model_position: adjusted,
                raw_screen_position: raw_screen,
                adjusted_screen_position: frame.viewport.model_to_screen(adjusted),
                relations,
                references,
                guides,
                ranking: DraftInferenceRankingEvidence {
                    constraint_backed,
                    persistent_relation_count,
                    anchor_priority: anchor.map_or(DraftAnchorPriority::None, |work| {
                        work.anchor.anchor_priority()
                    }),
                    direction_priority: direction
                        .map_or(DraftDirectionPriority::None, |work| work.priority),
                    positional_geometry_role_priority,
                    directional_geometry_role_priority,
                    distance_pixels,
                    angular_error_radians,
                },
            },
        }
    }

    fn direction_works(
        &self,
        frame: &DraftInferenceFrame,
        target: [f64; 2],
        retained_direction: Option<DirectionKey>,
    ) -> Vec<DirectionWork> {
        let mut works = self.direction_works_without_latch(frame, target);

        let retained_work = retained_direction
            .and_then(|active| works.iter().find(|work| work.key == active).copied());
        let higher_priority_direction_entered = retained_work.is_some_and(|active| {
            works.iter().any(|work| {
                work.angular_error <= self.policy.tolerances.direction_enter_radians
                    && (work.behavior.show_guides || work.behavior.has_effect())
                    && compare_direction_work_priority(*work, active) == Ordering::Less
            })
        });
        let effective_retained = if higher_priority_direction_entered {
            None
        } else {
            retained_direction
        };
        let threshold = if effective_retained.is_some() {
            self.policy.tolerances.direction_exit_radians
        } else {
            self.policy.tolerances.direction_enter_radians
        };
        works.retain(|work| {
            (effective_retained.is_none() || effective_retained == Some(work.key))
                && work.angular_error <= threshold
                && (work.behavior.show_guides || work.behavior.has_effect())
        });
        works.sort_by(|first, second| {
            compare_direction_work_priority(*first, *second)
                .then_with(|| first.key.cmp(&second.key))
        });
        works
    }

    fn retained_direction(
        &self,
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
    ) -> Option<DirectionKey> {
        let active = self.active_direction?;
        self.direction_works_without_latch(frame, raw_model)
            .into_iter()
            .find(|work| {
                work.key == active
                    && work.angular_error <= self.policy.tolerances.direction_exit_radians
            })
            .map(|work| work.key)
    }

    fn direction_works_without_latch(
        &self,
        frame: &DraftInferenceFrame,
        target: [f64; 2],
    ) -> Vec<DirectionWork> {
        let Some(start) = frame.sample.span_start else {
            return Vec::new();
        };
        let delta = [target[0] - start[0], target[1] - start[1]];
        if normalized(delta).is_none() {
            return Vec::new();
        }
        let mut works = vec![
            direction_work(
                DirectionKey::Horizontal,
                self.policy.horizontal,
                start,
                target,
                [1.0, 0.0],
                frame.viewport.pixels_per_model_unit,
                None,
                DraftDirectionPriority::WorldAxis,
                2,
            ),
            direction_work(
                DirectionKey::Vertical,
                self.policy.vertical,
                start,
                target,
                [0.0, 1.0],
                frame.viewport.pixels_per_model_unit,
                None,
                DraftDirectionPriority::WorldAxis,
                2,
            ),
        ];
        let mut seen = BTreeSet::new();
        for reference in self.remembered_references.iter().rev().copied() {
            let Some((span, direction)) = reference.affine_reference() else {
                continue;
            };
            if !reference.is_interactive(frame.geometry_policy) {
                continue;
            }
            let support_relation = if reference_is_certified_native_support(reference)
                && point_is_on_affine_support(start, reference.model_position(), direction)
            {
                (DirectionKey::Collinear(span), self.policy.collinear)
            } else {
                (DirectionKey::Parallel(span), self.policy.parallel)
            };
            for (key, axis, behavior) in [
                (support_relation.0, direction, support_relation.1),
                (
                    DirectionKey::Perpendicular(span),
                    [-direction[1], direction[0]],
                    self.policy.perpendicular,
                ),
            ] {
                if seen.insert(key) {
                    works.push(direction_work(
                        key,
                        behavior,
                        start,
                        target,
                        axis,
                        frame.viewport.pixels_per_model_unit,
                        Some(reference),
                        DraftDirectionPriority::RememberedReference,
                        reference.role_priority(frame.geometry_policy.scope),
                    ));
                }
            }
        }
        works
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "bounded intrinsic-datum generation keeps policy, latch, and candidate publication explicit"
    )]
    fn generate_datum_candidates(
        &mut self,
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
        raw_screen: ScreenPoint,
        retained_direction: Option<DirectionKey>,
        standalone_guides: &mut Vec<DraftGuide>,
        works: &mut Vec<CandidateWork>,
        candidate_limit: usize,
    ) -> bool {
        if !frame.sample.subject.is_point_operand()
            || !frame.geometry_policy.visibility.reference_geometry
        {
            self.active_datum = None;
            return true;
        }

        if let Some(complete) = self.generate_origin_datum_candidate(
            frame,
            raw_model,
            raw_screen,
            standalone_guides,
            works,
            candidate_limit,
        ) {
            return complete;
        }

        let origin_screen = frame.viewport.model_to_screen([0.0, 0.0]);
        let axis_behavior = self.policy.datum_axis;
        if !axis_behavior.show_guides && !axis_behavior.has_effect() {
            self.active_datum = None;
            return true;
        }
        let directions = self.direction_works(frame, raw_model, retained_direction);
        let mut datums = [
            DatumWork {
                key: DatumKey::XAxis,
                behavior: axis_behavior,
                adjusted_position: [raw_model[0], 0.0],
                distance_pixels: (raw_screen.y - origin_screen.y).abs(),
            },
            DatumWork {
                key: DatumKey::YAxis,
                behavior: axis_behavior,
                adjusted_position: [0.0, raw_model[1]],
                distance_pixels: (raw_screen.x - origin_screen.x).abs(),
            },
        ]
        .into_iter()
        .filter(|datum| {
            let threshold = if self.active_datum == Some(datum.key) {
                self.policy.tolerances.datum_axis_exit_pixels
            } else {
                self.policy.tolerances.datum_axis_enter_pixels
            };
            datum.distance_pixels <= threshold
                && !(datum.behavior.adjust_coordinates
                    && datum.behavior.persist_constraint
                    && datum.key.tracking_axis().is_some_and(|axis| {
                        world_span_direction_supersedes_point_tracking(axis, &directions)
                    }))
        })
        .collect::<Vec<_>>();

        // An already published axis remains the sole datum owner through its
        // wider exit band. A same-coordinate live span can supersede it above;
        // in that case another axis may enter normally in this sample.
        if self
            .active_datum
            .is_some_and(|active| datums.iter().any(|datum| datum.key == active))
        {
            let active = self.active_datum;
            datums.retain(|datum| Some(datum.key) == active);
        }
        self.active_datum = (datums.len() == 1).then(|| datums[0].key);

        if !axis_behavior.has_effect() {
            if axis_behavior.show_guides {
                standalone_guides.extend(
                    datums
                        .into_iter()
                        .map(|datum| datum_guide(None, 0, datum, datum.adjusted_position)),
                );
            }
            return true;
        }

        Self::publish_datum_axis_candidates(
            frame,
            raw_model,
            raw_screen,
            datums,
            &directions,
            works,
            candidate_limit,
        )
    }

    fn generate_origin_datum_candidate(
        &mut self,
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
        raw_screen: ScreenPoint,
        standalone_guides: &mut Vec<DraftGuide>,
        works: &mut Vec<CandidateWork>,
        candidate_limit: usize,
    ) -> Option<bool> {
        let origin_screen = frame.viewport.model_to_screen([0.0, 0.0]);
        let behavior = self.policy.datum_origin;
        let distance_pixels = screen_distance(raw_screen, origin_screen);
        let threshold = if self.active_datum == Some(DatumKey::Origin) {
            self.policy.tolerances.datum_origin_exit_pixels
        } else {
            self.policy.tolerances.datum_origin_enter_pixels
        };
        if (!behavior.show_guides && !behavior.has_effect()) || distance_pixels > threshold {
            return None;
        }

        let datum = DatumWork {
            key: DatumKey::Origin,
            behavior,
            adjusted_position: [0.0, 0.0],
            distance_pixels,
        };
        self.active_datum = Some(DatumKey::Origin);
        if behavior.has_effect() {
            return Some(push_candidate_bounded(
                works,
                Self::build_datum_candidate(
                    frame,
                    raw_model,
                    raw_screen,
                    datum,
                    None,
                    datum.adjusted_position,
                ),
                candidate_limit,
                frame.sample.subject,
            ));
        }
        standalone_guides.push(datum_guide(None, 0, datum, datum.adjusted_position));
        Some(true)
    }

    fn publish_datum_axis_candidates(
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
        raw_screen: ScreenPoint,
        datums: Vec<DatumWork>,
        directions: &[DirectionWork],
        works: &mut Vec<CandidateWork>,
        candidate_limit: usize,
    ) -> bool {
        let composed_directions = directions
            .iter()
            .copied()
            .filter(|direction| {
                datums
                    .iter()
                    .any(|datum| datum_direction_adjustment(frame, *datum, *direction).is_some())
            })
            .map(|direction| direction.key)
            .collect::<BTreeSet<_>>();
        works.retain(|work| {
            work.key.anchor.is_some()
                || work.key.datum.is_some()
                || work.key.point_tracking.is_some()
                || work
                    .key
                    .direction
                    .is_none_or(|direction| !composed_directions.contains(&direction))
        });

        for datum in datums {
            let mut composed = false;
            for direction in directions.iter().copied() {
                let Some(adjusted) = datum_direction_adjustment(frame, datum, direction) else {
                    continue;
                };
                composed = true;
                if !push_candidate_bounded(
                    works,
                    Self::build_datum_candidate(
                        frame,
                        raw_model,
                        raw_screen,
                        datum,
                        Some(direction),
                        adjusted,
                    ),
                    candidate_limit,
                    frame.sample.subject,
                ) {
                    return false;
                }
            }
            if !composed
                && !push_candidate_bounded(
                    works,
                    Self::build_datum_candidate(
                        frame,
                        raw_model,
                        raw_screen,
                        datum,
                        None,
                        if datum.behavior.adjust_coordinates {
                            datum.adjusted_position
                        } else {
                            raw_model
                        },
                    ),
                    candidate_limit,
                    frame.sample.subject,
                )
            {
                return false;
            }
        }
        true
    }

    fn build_datum_candidate(
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
        raw_screen: ScreenPoint,
        datum: DatumWork,
        direction: Option<DirectionWork>,
        adjusted: [f64; 2],
    ) -> CandidateWork {
        let mut relations = Vec::new();
        let mut references = Vec::new();
        let mut guides = Vec::new();
        if datum.behavior.persist_constraint {
            relations.push(match datum.key.coordinate_axis() {
                None => DraftInferenceRelation::CoincidentWithOrigin,
                Some(axis) => DraftInferenceRelation::PointOnDatumAxis { axis },
            });
        }
        if datum.behavior.show_guides {
            guides.push(datum_guide(None, 0, datum, adjusted));
        }
        if let Some(direction) = direction {
            if direction.behavior.persist_constraint {
                relations.push(direction_relation(direction.key));
            }
            if let Some(reference) = direction.reference {
                references.push(reference);
            }
            if direction.behavior.show_guides {
                let mut guide = direction_guide(
                    None,
                    u32::try_from(guides.len())
                        .expect("datum bundle guide count is structurally bounded"),
                    frame.sample.span_start,
                    direction,
                );
                guide.geometry = DraftGuideGeometry::Segment {
                    start: frame
                        .sample
                        .span_start
                        .expect("composed datum direction requires a live span start"),
                    end: adjusted,
                };
                guides.push(guide);
            }
        }
        let persistent_relation_count = u8::try_from(relations.len()).unwrap_or(u8::MAX);
        CandidateWork {
            key: CandidateKey {
                anchor: None,
                datum: Some(datum.key),
                point_tracking: None,
                secondary_point_tracking: None,
                direction: direction.map(|work| work.key),
            },
            candidate: DraftInferenceCandidate {
                id: DraftInferenceCandidateId(0),
                raw_model_position: raw_model,
                adjusted_model_position: adjusted,
                raw_screen_position: raw_screen,
                adjusted_screen_position: frame.viewport.model_to_screen(adjusted),
                relations,
                references,
                guides,
                ranking: DraftInferenceRankingEvidence {
                    constraint_backed: persistent_relation_count > 0,
                    persistent_relation_count,
                    anchor_priority: match datum.key {
                        DatumKey::Origin => DraftAnchorPriority::DatumOrigin,
                        DatumKey::XAxis | DatumKey::YAxis => DraftAnchorPriority::DatumAxis,
                    },
                    direction_priority: direction
                        .map_or(DraftDirectionPriority::None, |work| work.priority),
                    positional_geometry_role_priority: 2,
                    directional_geometry_role_priority: direction
                        .map_or(2, |work| work.role_priority),
                    distance_pixels: datum.distance_pixels,
                    angular_error_radians: direction.map_or(0.0, |work| work.angular_error),
                },
            },
        }
    }

    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one bounded stored-point/native-midpoint traversal keeps latch, guide, and candidate state explicit"
    )]
    fn generate_point_tracking_candidates(
        &mut self,
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
        raw_screen: ScreenPoint,
        retained_direction: Option<DirectionKey>,
        standalone_guides: &mut Vec<DraftGuide>,
        works: &mut Vec<CandidateWork>,
        candidate_limit: usize,
    ) -> bool {
        if !self.policy.point_tracking.show_guides && !self.policy.point_tracking.has_effect() {
            self.active_point_tracking.clear();
            return true;
        }
        let directions = self.direction_works(frame, raw_model, retained_direction);
        let mut tracking_works = Vec::new();
        let mut next_active = BTreeSet::new();
        for reference in self.remembered_references.iter().rev().copied() {
            if !reference.is_interactive(frame.geometry_policy) {
                continue;
            }
            let origin = match reference {
                DraftReferenceAnchor::PersistentPoint { model_position, .. }
                | DraftReferenceAnchor::Midpoint { model_position, .. } => model_position,
                DraftReferenceAnchor::CurvePoint { .. }
                | DraftReferenceAnchor::AffineSupport { .. } => continue,
            };
            let delta = [raw_model[0] - origin[0], raw_model[1] - origin[1]];
            if normalized(delta).is_none() {
                continue;
            }
            for (axis, direction) in [
                (PointTrackingAxis::Horizontal, [1.0, 0.0]),
                (PointTrackingAxis::Vertical, [0.0, 1.0]),
            ] {
                let key = PointTrackingKey {
                    anchor: reference.key(),
                    axis,
                };
                let threshold = if self.active_point_tracking.contains(&key) {
                    self.policy.tolerances.direction_exit_radians
                } else {
                    self.policy.tolerances.direction_enter_radians
                };
                let angular_error = undirected_angle_error(delta, direction);
                if angular_error <= threshold {
                    let durable = match axis {
                        PointTrackingAxis::Horizontal => match reference {
                            DraftReferenceAnchor::PersistentPoint { point, .. } => Some((
                                DraftInferenceRelation::HorizontalPoints { reference: point },
                                DraftInferenceFamily::HorizontalPoints,
                            )),
                            DraftReferenceAnchor::Midpoint {
                                span,
                                origin: DraftReferenceOrigin::Native,
                                ..
                            } => Some((
                                DraftInferenceRelation::HorizontalPointToMidpoint {
                                    reference: span,
                                },
                                DraftInferenceFamily::HorizontalPointToMidpoint,
                            )),
                            DraftReferenceAnchor::Midpoint {
                                origin: DraftReferenceOrigin::FilletDiscarded,
                                ..
                            }
                            | DraftReferenceAnchor::CurvePoint { .. }
                            | DraftReferenceAnchor::AffineSupport { .. } => None,
                        },
                        PointTrackingAxis::Vertical => match reference {
                            DraftReferenceAnchor::PersistentPoint { point, .. } => Some((
                                DraftInferenceRelation::VerticalPoints { reference: point },
                                DraftInferenceFamily::VerticalPoints,
                            )),
                            DraftReferenceAnchor::Midpoint {
                                span,
                                origin: DraftReferenceOrigin::Native,
                                ..
                            } => Some((
                                DraftInferenceRelation::VerticalPointToMidpoint { reference: span },
                                DraftInferenceFamily::VerticalPointToMidpoint,
                            )),
                            DraftReferenceAnchor::Midpoint {
                                origin: DraftReferenceOrigin::FilletDiscarded,
                                ..
                            }
                            | DraftReferenceAnchor::CurvePoint { .. }
                            | DraftReferenceAnchor::AffineSupport { .. } => None,
                        },
                    };
                    // A constraint-backed live world axis owns this endpoint
                    // coordinate. Suppress only a competing durable same-axis
                    // tracker before it can publish a guide, consume candidate
                    // capacity, or acquire an invisible exit-band latch.
                    // Generic tracking-only cues retain their M70/M71 behavior,
                    // and orthogonal durable trackers remain available for a
                    // two-relation bundle.
                    if self.policy.point_tracking.persist_constraint
                        && durable.is_some()
                        && world_span_direction_supersedes_point_tracking(axis, &directions)
                    {
                        continue;
                    }
                    next_active.insert(key);
                    let guide = DraftGuide {
                        id: DraftGuideId {
                            candidate: None,
                            ordinal: 0,
                        },
                        family: DraftInferenceFamily::PointTracking,
                        classification: DraftGuideClassification::TrackingOnly,
                        geometry: DraftGuideGeometry::Segment {
                            start: origin,
                            end: raw_model,
                        },
                        reference: Some(reference),
                    };
                    if self.policy.point_tracking.persist_constraint
                        && let Some((relation, family)) = durable
                    {
                        let adjusted = match axis {
                            PointTrackingAxis::Horizontal => [raw_model[0], origin[1]],
                            PointTrackingAxis::Vertical => [origin[0], raw_model[1]],
                        };
                        tracking_works.push(PointTrackingWork {
                            key,
                            reference,
                            origin,
                            relation,
                            family,
                            adjusted_position: adjusted,
                            angular_error,
                        });
                    } else {
                        standalone_guides.push(guide);
                    }
                }
            }
        }
        self.active_point_tracking = next_active;

        // Two distinct remembered operands may jointly own the endpoint's
        // Cartesian coordinates: Horizontal supplies Y and Vertical supplies
        // X. Build those exact intersections before singleton tracking
        // alternatives so one bounded candidate owns both relations/guides.
        // The same operand is deliberately excluded because that would
        // disguise point-identity reuse as two redundant axis relations.
        let mut paired_tracking = BTreeSet::new();
        for horizontal in tracking_works
            .iter()
            .filter(|work| work.key.axis == PointTrackingAxis::Horizontal)
        {
            for vertical in tracking_works
                .iter()
                .filter(|work| work.key.axis == PointTrackingAxis::Vertical)
            {
                let Some(adjusted) = point_tracking_pair_adjustment(horizontal, vertical) else {
                    continue;
                };
                paired_tracking.insert(horizontal.key);
                paired_tracking.insert(vertical.key);
                if !push_candidate_bounded(
                    works,
                    self.build_point_tracking_candidate(
                        frame,
                        raw_model,
                        raw_screen,
                        horizontal,
                        Some(vertical),
                        None,
                        adjusted,
                    ),
                    candidate_limit,
                    frame.sample.subject,
                ) {
                    return false;
                }
            }
        }

        // Identify the direction singletons replaced by conjunction bundles
        // without first allocating every possible point-axis x direction
        // candidate. Candidate construction below remains streaming and stops
        // at the first unique bundle that proves the configured bound too low.
        let composed_directions = directions
            .iter()
            .copied()
            .filter(|direction| {
                tracking_works.iter().any(|tracking| {
                    point_tracking_direction_adjustment(frame, tracking, *direction).is_some()
                })
            })
            .map(|direction| direction.key)
            .collect::<BTreeSet<_>>();
        works.retain(|work| {
            work.key.anchor.is_some()
                || work.key.datum.is_some()
                || work.key.point_tracking.is_some()
                || work
                    .key
                    .direction
                    .is_none_or(|direction| !composed_directions.contains(&direction))
        });

        for tracking in &tracking_works {
            // Pair membership suppresses only the tracking singleton. A
            // complementary live-span direction still forms its conjunction
            // bundle. Eligible same-axis world directions were filtered
            // before guide/latch creation; remembered directions retain their
            // existing compatibility behavior.
            let mut composed = false;
            for direction in directions.iter().copied() {
                let Some(adjusted) =
                    point_tracking_direction_adjustment(frame, tracking, direction)
                else {
                    continue;
                };
                composed = true;
                if !push_candidate_bounded(
                    works,
                    self.build_point_tracking_candidate(
                        frame,
                        raw_model,
                        raw_screen,
                        tracking,
                        None,
                        Some(direction),
                        adjusted,
                    ),
                    candidate_limit,
                    frame.sample.subject,
                ) {
                    return false;
                }
            }
            if !composed
                && !paired_tracking.contains(&tracking.key)
                && !push_candidate_bounded(
                    works,
                    self.build_point_tracking_candidate(
                        frame,
                        raw_model,
                        raw_screen,
                        tracking,
                        None,
                        None,
                        tracking.adjusted_position,
                    ),
                    candidate_limit,
                    frame.sample.subject,
                )
            {
                return false;
            }
        }
        true
    }

    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one candidate constructor keeps ordered tracking components, guides, identity, and ranking evidence aligned"
    )]
    fn build_point_tracking_candidate(
        &self,
        frame: &DraftInferenceFrame,
        raw_model: [f64; 2],
        raw_screen: ScreenPoint,
        tracking: &PointTrackingWork,
        secondary_tracking: Option<&PointTrackingWork>,
        direction: Option<DirectionWork>,
        adjusted: [f64; 2],
    ) -> CandidateWork {
        let mut relations = vec![tracking.relation];
        let mut references = vec![tracking.reference];
        let mut guides = Vec::new();
        if self.policy.point_tracking.show_guides {
            guides.push(DraftGuide {
                id: DraftGuideId {
                    candidate: None,
                    ordinal: 0,
                },
                family: tracking.family,
                classification: DraftGuideClassification::ConstraintBacked,
                geometry: DraftGuideGeometry::Segment {
                    start: tracking.origin,
                    end: adjusted,
                },
                reference: Some(tracking.reference),
            });
        }
        if let Some(secondary) = secondary_tracking {
            relations.push(secondary.relation);
            if !references.contains(&secondary.reference) {
                references.push(secondary.reference);
            }
            if self.policy.point_tracking.show_guides {
                guides.push(DraftGuide {
                    id: DraftGuideId {
                        candidate: None,
                        ordinal: u32::try_from(guides.len())
                            .expect("candidate guide count is structurally bounded"),
                    },
                    family: secondary.family,
                    classification: DraftGuideClassification::ConstraintBacked,
                    geometry: DraftGuideGeometry::Segment {
                        start: secondary.origin,
                        end: adjusted,
                    },
                    reference: Some(secondary.reference),
                });
            }
        }
        if let Some(direction) = direction {
            if direction.behavior.persist_constraint {
                relations.push(direction_relation(direction.key));
            }
            if let Some(reference) = direction.reference
                && !references.contains(&reference)
            {
                references.push(reference);
            }
            if direction.behavior.show_guides {
                let mut guide = direction_guide(
                    None,
                    u32::try_from(guides.len())
                        .expect("candidate guide count is structurally bounded"),
                    frame.sample.span_start,
                    direction,
                );
                guide.geometry = DraftGuideGeometry::Segment {
                    start: frame
                        .sample
                        .span_start
                        .expect("composed direction requires a live span start"),
                    end: adjusted,
                };
                guides.push(guide);
            }
        }
        let persistent_relation_count = u8::try_from(relations.len()).unwrap_or(u8::MAX);
        // A conjunction is only as strong as its weaker angular component.
        // Publishing the maximum keeps this scalar evidence conservative and
        // prevents a near-perfect component from masking a marginal one.
        let tracking_angular_error = secondary_tracking.map_or(tracking.angular_error, |work| {
            tracking.angular_error.max(work.angular_error)
        });
        let angular_error_radians = direction.map_or(tracking_angular_error, |work| {
            tracking_angular_error.max(work.angular_error)
        });
        CandidateWork {
            key: CandidateKey {
                anchor: None,
                datum: None,
                point_tracking: Some(tracking.key),
                secondary_point_tracking: secondary_tracking.map(|work| work.key),
                direction: direction.map(|work| work.key),
            },
            candidate: DraftInferenceCandidate {
                id: DraftInferenceCandidateId(0),
                raw_model_position: raw_model,
                adjusted_model_position: adjusted,
                raw_screen_position: raw_screen,
                adjusted_screen_position: frame.viewport.model_to_screen(adjusted),
                relations,
                references,
                guides,
                ranking: DraftInferenceRankingEvidence {
                    constraint_backed: persistent_relation_count > 0,
                    persistent_relation_count,
                    anchor_priority: DraftAnchorPriority::None,
                    direction_priority: direction
                        .map_or(DraftDirectionPriority::WorldAxis, |work| work.priority),
                    positional_geometry_role_priority: 2,
                    directional_geometry_role_priority: direction
                        .map_or(2, |work| work.role_priority),
                    distance_pixels: 0.0,
                    angular_error_radians,
                },
            },
        }
    }

    fn assign_candidate_ids(
        &mut self,
        works: &mut [CandidateWork],
    ) -> Result<(), DraftInferenceError> {
        // Stage both the identity cursor and the complete key map before
        // publishing either.  Exhaustion is exceptionally remote in normal
        // use, but it must still leave the previous identities intact rather
        // than partially consuming the session-local identity space.
        let mut staged_next_candidate_id = self.next_candidate_id;
        let mut staged = Vec::with_capacity(works.len());
        for work in works.iter() {
            let id = if let Some((_, id)) =
                self.candidate_ids.iter().find(|(key, _)| *key == work.key)
            {
                *id
            } else {
                let value = staged_next_candidate_id;
                staged_next_candidate_id = staged_next_candidate_id
                    .checked_add(1)
                    .ok_or(DraftInferenceError::CandidateIdentityExhausted)?;
                DraftInferenceCandidateId(value)
            };
            staged.push((work.key, id));
        }

        for (work, (_, id)) in works.iter_mut().zip(&staged) {
            work.candidate.id = *id;
        }
        staged.sort_by_key(|(key, _)| *key);
        self.candidate_ids = staged;
        self.next_candidate_id = staged_next_candidate_id;
        Ok(())
    }

    fn remember_valid_reference(&mut self, reference: DraftReferenceAnchor) {
        let key = reference.key();
        if let Some(index) = self
            .remembered_references
            .iter()
            .position(|candidate| candidate.key() == key)
        {
            self.remembered_references.remove(index);
        }
        self.remembered_references.push(reference);
        while self.remembered_references.len() > self.policy.limits.max_remembered_references {
            self.remembered_references.remove(0);
        }
    }

    fn remember_wake_references(
        &mut self,
        references: Vec<AnchorWork>,
        viewport: Viewport,
        raw_screen: ScreenPoint,
        scope: GeometryPickScope,
    ) {
        if references.is_empty() {
            return;
        }

        // Re-rank the complete bounded memory together with this simultaneous
        // wake batch. This prevents a formerly admitted exact tie from later
        // being split by FIFO/identity order when another reference enters.
        let mut references = self
            .remembered_references
            .iter()
            .copied()
            .filter(|anchor| anchor.is_reusable_reference())
            .filter_map(|anchor| {
                let (family, _) =
                    anchor.inference_for_subject(DraftInferenceSubject::PointOperand)?;
                Some(AnchorWork {
                    anchor,
                    key: anchor.key(),
                    family,
                    behavior: self.behavior(family),
                    distance_pixels: screen_distance(
                        raw_screen,
                        viewport.model_to_screen(anchor.model_position()),
                    ),
                })
            })
            .chain(
                references
                    .into_iter()
                    .filter(|reference| reference.anchor.is_reusable_reference()),
            )
            .collect::<Vec<_>>();
        deduplicate_anchor_works(&mut references, scope);
        references.sort_by(|first, second| {
            compare_wake_reference_priority(*first, *second, scope)
                .then_with(|| compare_anchor_occurrences(*first, *second, scope))
                // Identity is allowed to stabilize storage order only after
                // semantic selection is complete.
                .then_with(|| first.key.cmp(&second.key))
        });

        let limit = self.policy.limits.max_remembered_references;
        if references.len() > limit {
            let mut retained = limit;
            if compare_wake_reference_priority(references[limit - 1], references[limit], scope)
                == Ordering::Equal
            {
                let boundary = references[limit - 1];
                retained = references[..limit]
                    .iter()
                    .rposition(|candidate| {
                        compare_wake_reference_priority(*candidate, boundary, scope)
                            != Ordering::Equal
                    })
                    .map_or(0, |index| index + 1);
            }
            references.truncate(retained);
        }

        // Keep lower-ranked references older and the strongest newest for the
        // documented public ordering. Selection has already completed without
        // identity; keys only stabilize ordering inside a fully admitted tie.
        self.remembered_references = references
            .into_iter()
            .rev()
            .map(|reference| reference.anchor)
            .collect();
    }

    const fn behavior(&self, family: DraftInferenceFamily) -> DraftInferenceBehavior {
        match family {
            DraftInferenceFamily::PointIdentity => self.policy.point_identity,
            DraftInferenceFamily::DatumOrigin => self.policy.datum_origin,
            DraftInferenceFamily::DatumAxis => self.policy.datum_axis,
            DraftInferenceFamily::PointOnCurve | DraftInferenceFamily::PointOnCreatedCurve => {
                self.policy.point_on_curve
            }
            DraftInferenceFamily::Midpoint => self.policy.midpoint,
            DraftInferenceFamily::Horizontal => self.policy.horizontal,
            DraftInferenceFamily::Vertical => self.policy.vertical,
            DraftInferenceFamily::Parallel => self.policy.parallel,
            DraftInferenceFamily::Perpendicular => self.policy.perpendicular,
            DraftInferenceFamily::Concentric => self.policy.concentric,
            DraftInferenceFamily::Collinear => self.policy.collinear,
            DraftInferenceFamily::PointTracking
            | DraftInferenceFamily::HorizontalPoints
            | DraftInferenceFamily::VerticalPoints
            | DraftInferenceFamily::HorizontalPointToMidpoint
            | DraftInferenceFamily::VerticalPointToMidpoint => self.policy.point_tracking,
        }
    }

    fn enter_distance(&self, family: DraftInferenceFamily) -> f64 {
        match family {
            DraftInferenceFamily::PointIdentity
            | DraftInferenceFamily::PointOnCreatedCurve
            | DraftInferenceFamily::Midpoint => self.policy.tolerances.point_enter_pixels,
            DraftInferenceFamily::PointOnCurve => self.policy.tolerances.curve_enter_pixels,
            DraftInferenceFamily::Horizontal
            | DraftInferenceFamily::Vertical
            | DraftInferenceFamily::DatumOrigin
            | DraftInferenceFamily::DatumAxis
            | DraftInferenceFamily::Parallel
            | DraftInferenceFamily::Perpendicular
            | DraftInferenceFamily::HorizontalPoints
            | DraftInferenceFamily::VerticalPoints
            | DraftInferenceFamily::HorizontalPointToMidpoint
            | DraftInferenceFamily::VerticalPointToMidpoint
            | DraftInferenceFamily::Concentric
            | DraftInferenceFamily::Collinear
            | DraftInferenceFamily::PointTracking => 0.0,
        }
    }

    fn exit_distance(&self, family: DraftInferenceFamily) -> f64 {
        match family {
            DraftInferenceFamily::PointIdentity
            | DraftInferenceFamily::PointOnCreatedCurve
            | DraftInferenceFamily::Midpoint => self.policy.tolerances.point_exit_pixels,
            DraftInferenceFamily::PointOnCurve => self.policy.tolerances.curve_exit_pixels,
            DraftInferenceFamily::Horizontal
            | DraftInferenceFamily::Vertical
            | DraftInferenceFamily::DatumOrigin
            | DraftInferenceFamily::DatumAxis
            | DraftInferenceFamily::Parallel
            | DraftInferenceFamily::Perpendicular
            | DraftInferenceFamily::HorizontalPoints
            | DraftInferenceFamily::VerticalPoints
            | DraftInferenceFamily::HorizontalPointToMidpoint
            | DraftInferenceFamily::VerticalPointToMidpoint
            | DraftInferenceFamily::Concentric
            | DraftInferenceFamily::Collinear
            | DraftInferenceFamily::PointTracking => 0.0,
        }
    }
}

fn empty_resolution(
    status: DraftInferenceStatus,
    raw_model: [f64; 2],
    raw_screen: ScreenPoint,
) -> DraftInferenceResolution {
    DraftInferenceResolution {
        status,
        completeness: DraftInferenceCompleteness::Complete,
        raw_model_position: raw_model,
        adjusted_model_position: raw_model,
        raw_screen_position: raw_screen,
        adjusted_screen_position: raw_screen,
        candidates: Vec::new(),
        guides: Vec::new(),
    }
}

fn scene_limit_resolution(
    evidence: DraftInferenceSceneLimit,
    raw_model: [f64; 2],
    raw_screen: ScreenPoint,
) -> DraftInferenceResolution {
    DraftInferenceResolution {
        status: DraftInferenceStatus::ResourceLimited,
        completeness: DraftInferenceCompleteness::SceneLimit(evidence),
        raw_model_position: raw_model,
        adjusted_model_position: raw_model,
        raw_screen_position: raw_screen,
        adjusted_screen_position: raw_screen,
        candidates: Vec::new(),
        guides: Vec::new(),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "direction evidence is constructed once from explicit semantic inputs"
)]
fn direction_work(
    key: DirectionKey,
    behavior: DraftInferenceBehavior,
    start: [f64; 2],
    target: [f64; 2],
    axis: [f64; 2],
    pixels_per_model_unit: f64,
    reference: Option<DraftReferenceAnchor>,
    priority: DraftDirectionPriority,
    role_priority: u8,
) -> DirectionWork {
    let cartesian_axis = if axis[1] == 0.0 && axis[0] != 0.0 {
        Some(PointTrackingAxis::Horizontal)
    } else if axis[0] == 0.0 && axis[1] != 0.0 {
        Some(PointTrackingAxis::Vertical)
    } else {
        None
    };
    let unit = normalized(axis).expect("direction references are validated before resolution");
    let delta = [target[0] - start[0], target[1] - start[1]];
    let delta_scale = delta[0].abs().max(delta[1].abs());
    let scaled_delta = [delta[0] / delta_scale, delta[1] / delta_scale];
    let projection = dot(scaled_delta, unit);
    let transverse_ratio = dot(scaled_delta, [-unit[1], unit[0]]).abs();
    let span_scale_pixels = delta_scale * pixels_per_model_unit;
    let transverse_pixels = if transverse_ratio == 0.0 {
        0.0
    } else {
        transverse_ratio * span_scale_pixels
    };
    DirectionWork {
        key,
        behavior,
        cartesian_axis,
        adjusted_position: [
            (projection * unit[0]).mul_add(delta_scale, start[0]),
            (projection * unit[1]).mul_add(delta_scale, start[1]),
        ],
        target_is_on_axis: transverse_pixels.is_finite()
            && transverse_pixels <= DIRECTION_COMBINATION_TOLERANCE_PIXELS,
        angular_error: undirected_angle_error(delta, unit),
        reference,
        priority,
        role_priority,
    }
}

fn reference_is_certified_native_support(reference: DraftReferenceAnchor) -> bool {
    matches!(
        reference,
        DraftReferenceAnchor::AffineSupport {
            origin: DraftReferenceOrigin::Native,
            ..
        }
    )
}

fn point_is_on_affine_support(
    point: [f64; 2],
    support_point: [f64; 2],
    support_direction: [f64; 2],
) -> bool {
    let Some(unit) = normalized(support_direction) else {
        return false;
    };
    let delta = [point[0] - support_point[0], point[1] - support_point[1]];
    let transverse = dot(delta, [-unit[1], unit[0]]).abs();
    // Certification is relative to the represented support displacement, not
    // an implicit one-model-unit floor or the absolute world origin.  A unit
    // floor would classify genuinely offset micro-scale spans as collinear;
    // absolute-coordinate scaling would make the decision translation
    // dependent.  The zero-displacement case remains exact (`0 <= 0`).
    let scale = delta.into_iter().map(f64::abs).fold(0.0, f64::max);
    transverse.is_finite() && transverse <= 64.0 * f64::EPSILON * scale
}

fn compare_direction_work_priority(first: DirectionWork, second: DirectionWork) -> Ordering {
    second
        .behavior
        .persist_constraint
        .cmp(&first.behavior.persist_constraint)
        .then_with(|| first.priority.rank().cmp(&second.priority.rank()))
        .then_with(|| first.role_priority.cmp(&second.role_priority))
        .then_with(|| first.angular_error.total_cmp(&second.angular_error))
}

fn direction_relation(key: DirectionKey) -> DraftInferenceRelation {
    match key {
        DirectionKey::Horizontal => DraftInferenceRelation::Horizontal,
        DirectionKey::Vertical => DraftInferenceRelation::Vertical,
        DirectionKey::Parallel(reference) => DraftInferenceRelation::Parallel { reference },
        DirectionKey::Perpendicular(reference) => {
            DraftInferenceRelation::Perpendicular { reference }
        }
        DirectionKey::Collinear(reference) => DraftInferenceRelation::Collinear { reference },
    }
}

fn point_tracking_pair_adjustment(
    first: &PointTrackingWork,
    second: &PointTrackingWork,
) -> Option<[f64; 2]> {
    if first.key.anchor == second.key.anchor {
        return None;
    }
    match (first.key.axis, second.key.axis) {
        (PointTrackingAxis::Horizontal, PointTrackingAxis::Vertical) => {
            Some([second.origin[0], first.origin[1]])
        }
        (PointTrackingAxis::Vertical, PointTrackingAxis::Horizontal) => {
            Some([first.origin[0], second.origin[1]])
        }
        (PointTrackingAxis::Horizontal, PointTrackingAxis::Horizontal)
        | (PointTrackingAxis::Vertical, PointTrackingAxis::Vertical) => None,
    }
}

fn world_span_direction_supersedes_point_tracking(
    tracking_axis: PointTrackingAxis,
    directions: &[DirectionWork],
) -> bool {
    directions.iter().any(|direction| {
        direction.behavior.adjust_coordinates
            && direction.behavior.persist_constraint
            && matches!(
                (tracking_axis, direction.key),
                (PointTrackingAxis::Horizontal, DirectionKey::Horizontal)
                    | (PointTrackingAxis::Vertical, DirectionKey::Vertical)
            )
    })
}

fn datum_direction_adjustment(
    frame: &DraftInferenceFrame,
    datum: DatumWork,
    direction: DirectionWork,
) -> Option<[f64; 2]> {
    if !datum.behavior.adjust_coordinates
        || !datum.behavior.persist_constraint
        || !direction.behavior.adjust_coordinates
        || !direction.behavior.persist_constraint
    {
        return None;
    }
    let start = frame.sample.span_start?;
    match (datum.key.tracking_axis()?, direction.cartesian_axis?) {
        (PointTrackingAxis::Horizontal, PointTrackingAxis::Vertical) => Some([start[0], 0.0]),
        (PointTrackingAxis::Vertical, PointTrackingAxis::Horizontal) => Some([0.0, start[1]]),
        (PointTrackingAxis::Horizontal, PointTrackingAxis::Horizontal)
        | (PointTrackingAxis::Vertical, PointTrackingAxis::Vertical) => None,
    }
}

fn point_tracking_direction_adjustment(
    frame: &DraftInferenceFrame,
    tracking: &PointTrackingWork,
    direction: DirectionWork,
) -> Option<[f64; 2]> {
    if !direction.behavior.adjust_coordinates || !direction.behavior.persist_constraint {
        return None;
    }
    let start = frame.sample.span_start?;
    let direction_axis = direction.cartesian_axis?;
    match (tracking.key.axis, direction_axis) {
        (PointTrackingAxis::Horizontal, PointTrackingAxis::Vertical) => {
            Some([start[0], tracking.origin[1]])
        }
        (PointTrackingAxis::Vertical, PointTrackingAxis::Horizontal) => {
            Some([tracking.origin[0], start[1]])
        }
        // Eligible live world Horizontal/Vertical work was suppressed before
        // candidate construction. Same-axis work reaching this branch is a
        // remembered Parallel/Perpendicular/Collinear direction (or a policy
        // that does not both adjust and persist); it remains an explicit
        // alternative because only complementary coordinate ownership is
        // composed here.
        (PointTrackingAxis::Horizontal, PointTrackingAxis::Horizontal)
        | (PointTrackingAxis::Vertical, PointTrackingAxis::Vertical) => None,
    }
}

fn anchor_guide(
    candidate: Option<DraftInferenceCandidateId>,
    ordinal: u32,
    work: AnchorWork,
) -> DraftGuide {
    DraftGuide {
        id: DraftGuideId { candidate, ordinal },
        family: work.family,
        classification: work.behavior.guide_classification(),
        geometry: DraftGuideGeometry::Point {
            position: work.anchor.model_position(),
        },
        reference: Some(work.anchor),
    }
}

fn datum_guide(
    candidate: Option<DraftInferenceCandidateId>,
    ordinal: u32,
    work: DatumWork,
    adjusted: [f64; 2],
) -> DraftGuide {
    DraftGuide {
        id: DraftGuideId { candidate, ordinal },
        family: match work.key {
            DatumKey::Origin => DraftInferenceFamily::DatumOrigin,
            DatumKey::XAxis | DatumKey::YAxis => DraftInferenceFamily::DatumAxis,
        },
        classification: work.behavior.guide_classification(),
        geometry: match work.key {
            DatumKey::Origin => DraftGuideGeometry::Point {
                position: [0.0, 0.0],
            },
            DatumKey::XAxis | DatumKey::YAxis => DraftGuideGeometry::Segment {
                start: [0.0, 0.0],
                end: adjusted,
            },
        },
        reference: None,
    }
}

fn direction_guide(
    candidate: Option<DraftInferenceCandidateId>,
    ordinal: u32,
    start: Option<[f64; 2]>,
    work: DirectionWork,
) -> DraftGuide {
    DraftGuide {
        id: DraftGuideId { candidate, ordinal },
        family: match work.key {
            DirectionKey::Horizontal => DraftInferenceFamily::Horizontal,
            DirectionKey::Vertical => DraftInferenceFamily::Vertical,
            DirectionKey::Parallel(_) => DraftInferenceFamily::Parallel,
            DirectionKey::Perpendicular(_) => DraftInferenceFamily::Perpendicular,
            DirectionKey::Collinear(_) => DraftInferenceFamily::Collinear,
        },
        classification: work.behavior.guide_classification(),
        geometry: DraftGuideGeometry::Segment {
            start: start.expect("direction work requires a live span start"),
            end: work.adjusted_position,
        },
        reference: work.reference,
    }
}

fn assign_guide_ids(candidate: &mut DraftInferenceCandidate) {
    for (index, guide) in candidate.guides.iter_mut().enumerate() {
        guide.id = DraftGuideId {
            candidate: Some(candidate.id),
            ordinal: u32::try_from(index).expect("candidate guide count is structurally bounded"),
        };
    }
}

fn assign_standalone_guide_ids(guides: &mut [DraftGuide]) {
    for (index, guide) in guides.iter_mut().enumerate() {
        guide.id = DraftGuideId {
            candidate: None,
            ordinal: u32::try_from(index)
                .expect("configured scene/reference limits bound standalone guide count"),
        };
    }
}

fn deduplicate_anchor_works(works: &mut Vec<AnchorWork>, scope: GeometryPickScope) {
    // Group by semantic identity before choosing a presentation occurrence.
    // Sorting globally by role/distance first can separate equal keys and make
    // `dedup_by_key` retain both the native Profile occurrence and its
    // Fillet-discarded Construction occurrence.
    works.sort_by(|first, second| {
        first
            .key
            .cmp(&second.key)
            .then_with(|| compare_anchor_occurrences(*first, *second, scope))
    });
    works.dedup_by_key(|work| work.key);
}

fn compare_anchor_occurrences(
    first: AnchorWork,
    second: AnchorWork,
    scope: GeometryPickScope,
) -> Ordering {
    let ordering = compare_role_aware_values(
        first.anchor.role_priority(scope),
        first.distance_pixels,
        second.anchor.role_priority(scope),
        second.distance_pixels,
    );
    ordering
        .then_with(|| {
            first.anchor.model_position()[0].total_cmp(&second.anchor.model_position()[0])
        })
        .then_with(|| {
            first.anchor.model_position()[1].total_cmp(&second.anchor.model_position()[1])
        })
        .then_with(|| {
            anchor_occurrence_priority(first.anchor).cmp(&anchor_occurrence_priority(second.anchor))
        })
        .then_with(|| {
            first
                .anchor
                .contact()
                .map_or(0.0, |contact| contact.parameter)
                .total_cmp(
                    &second
                        .anchor
                        .contact()
                        .map_or(0.0, |contact| contact.parameter),
                )
        })
        .then_with(|| {
            first
                .anchor
                .contact()
                .map_or(0, |contact| contact.winding)
                .cmp(&second.anchor.contact().map_or(0, |contact| contact.winding))
        })
}

fn compare_wake_reference_priority(
    first: AnchorWork,
    second: AnchorWork,
    scope: GeometryPickScope,
) -> Ordering {
    second
        .behavior
        .persist_constraint
        .cmp(&first.behavior.persist_constraint)
        .then_with(|| {
            first
                .anchor
                .anchor_priority()
                .rank()
                .cmp(&second.anchor.anchor_priority().rank())
        })
        .then_with(|| {
            compare_role_aware_values(
                first.anchor.role_priority(scope),
                first.distance_pixels,
                second.anchor.role_priority(scope),
                second.distance_pixels,
            )
        })
}

const fn anchor_occurrence_priority(anchor: DraftReferenceAnchor) -> u8 {
    match anchor {
        DraftReferenceAnchor::PersistentPoint { .. }
        | DraftReferenceAnchor::Midpoint { .. }
        | DraftReferenceAnchor::AffineSupport {
            origin: DraftReferenceOrigin::Native,
            ..
        } => 0,
        DraftReferenceAnchor::CurvePoint {
            origin: DraftReferenceOrigin::Native,
            ..
        } => 1,
        DraftReferenceAnchor::AffineSupport {
            origin: DraftReferenceOrigin::FilletDiscarded,
            ..
        } => 2,
        DraftReferenceAnchor::CurvePoint {
            origin: DraftReferenceOrigin::FilletDiscarded,
            ..
        } => 3,
    }
}

fn push_candidate_bounded(
    works: &mut Vec<CandidateWork>,
    candidate: CandidateWork,
    limit: usize,
    subject: DraftInferenceSubject,
) -> bool {
    if let Some(existing) = works.iter_mut().find(|work| work.key == candidate.key) {
        if candidate
            .candidate
            .ranking
            .compare_for_subject(existing.candidate.ranking, subject)
            == Ordering::Less
        {
            *existing = candidate;
        }
        return true;
    }
    if works.len() >= limit {
        return false;
    }
    works.push(candidate);
    true
}

fn compare_positional_role_aware_distance(
    first: DraftInferenceRankingEvidence,
    second: DraftInferenceRankingEvidence,
) -> Ordering {
    compare_role_aware_values(
        first.positional_geometry_role_priority,
        first.distance_pixels,
        second.positional_geometry_role_priority,
        second.distance_pixels,
    )
}

fn compare_role_aware_values(
    first_role: u8,
    first_distance: f64,
    second_role: u8,
    second_distance: f64,
) -> Ordering {
    match (first_role, second_role) {
        (0, 1) if first_distance <= second_distance + PROFILE_CONSTRUCTION_OVERLAP_PIXELS => {
            Ordering::Less
        }
        (1, 0) if second_distance <= first_distance + PROFILE_CONSTRUCTION_OVERLAP_PIXELS => {
            Ordering::Greater
        }
        (0, 1) | (1, 0) => first_distance.total_cmp(&second_distance),
        (first_role, second_role) => first_role
            .cmp(&second_role)
            .then_with(|| first_distance.total_cmp(&second_distance)),
    }
}

fn curve_role_is_interactive(
    role: GeometryRole,
    origin: DraftReferenceOrigin,
    policy: GeometryInteractionPolicy,
) -> bool {
    let visible = match role {
        GeometryRole::Profile => true,
        GeometryRole::Construction if origin.is_implicit_construction() => {
            policy.visibility.implicit_construction
        }
        GeometryRole::Construction => policy.visibility.explicit_construction,
    };
    visible
        && match policy.scope {
            GeometryPickScope::All => true,
            GeometryPickScope::Profile => role == GeometryRole::Profile,
            GeometryPickScope::Construction => role == GeometryRole::Construction,
        }
}

fn unbounded_contact_neighborhood_contains(
    neighborhood: ContactNeighborhood,
    total_parameter: f64,
) -> bool {
    match neighborhood {
        ContactNeighborhood::Interior => true,
        ContactNeighborhood::Local { lower, upper } => {
            lower.is_finite()
                && upper.is_finite()
                && lower < total_parameter
                && total_parameter < upper
        }
        ContactNeighborhood::Start | ContactNeighborhood::End => false,
    }
}

fn bounded_contact_neighborhood_contains(
    neighborhood: ContactNeighborhood,
    parameter: f64,
    domain_lower: f64,
    domain_upper: f64,
) -> bool {
    match neighborhood {
        ContactNeighborhood::Interior => domain_lower < parameter && parameter < domain_upper,
        ContactNeighborhood::Local { lower, upper } => {
            lower.is_finite()
                && upper.is_finite()
                && domain_lower <= lower
                && lower < parameter
                && parameter < upper
                && upper <= domain_upper
        }
        ContactNeighborhood::Start => parameter.to_bits() == domain_lower.to_bits(),
        ContactNeighborhood::End => parameter.to_bits() == domain_upper.to_bits(),
    }
}

fn screen_distance(first: ScreenPoint, second: ScreenPoint) -> f64 {
    (first.x - second.x).hypot(first.y - second.y)
}

fn normalized(vector: [f64; 2]) -> Option<[f64; 2]> {
    if !vector.into_iter().all(f64::is_finite) {
        return None;
    }
    let scale = vector[0].abs().max(vector[1].abs());
    if scale == 0.0 {
        return None;
    }
    let scaled = [vector[0] / scale, vector[1] / scale];
    let norm = scaled[0].hypot(scaled[1]);
    Some([scaled[0] / norm, scaled[1] / norm])
}

fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[0], first[1] * second[1])
}

fn undirected_angle_error(first: [f64; 2], second: [f64; 2]) -> f64 {
    let Some(first) = normalized(first) else {
        return f64::INFINITY;
    };
    let Some(second) = normalized(second) else {
        return f64::INFINITY;
    };
    dot(first, second).abs().clamp(0.0, 1.0).acos()
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "DTO tests intentionally require exact raw/adjusted coordinate preservation"
)]
mod tests {
    use super::*;
    use geosolve_sketch::{
        CurveId, DocumentSolveRequest, PersistentId, RetainedSketchDocumentSession, SketchDocument,
        SolverConfig,
    };
    use std::sync::OnceLock;

    fn point_id(value: u128) -> DesignPointId {
        DesignPointId(PersistentId::from_u128(value))
    }

    fn curve_span(value: u128) -> CurveSpan {
        CurveSpan::line(CurveId(PersistentId::from_u128(value)))
    }

    fn viewport(scale: f64) -> Viewport {
        Viewport::new([1_000.0, 700.0], [0.0, 0.0], scale).expect("viewport")
    }

    fn identity() -> SketchDesignIdentity {
        static IDENTITY: OnceLock<SketchDesignIdentity> = OnceLock::new();
        *IDENTITY.get_or_init(|| {
            RetainedSketchDocumentSession::new(
                SketchDocument::new(1.0).expect("document"),
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .expect("session")
            .design_identity()
        })
    }

    fn frame(
        viewport: Viewport,
        screen: ScreenPoint,
        span_start: Option<[f64; 2]>,
        anchors: Vec<DraftReferenceAnchor>,
    ) -> DraftInferenceFrame {
        frame_for_subject(
            viewport,
            screen,
            DraftInferenceSubject::PointOperand,
            span_start,
            anchors,
        )
    }

    fn frame_for_subject(
        viewport: Viewport,
        screen: ScreenPoint,
        subject: DraftInferenceSubject,
        span_start: Option<[f64; 2]>,
        anchors: Vec<DraftReferenceAnchor>,
    ) -> DraftInferenceFrame {
        let mut geometry_policy = GeometryInteractionPolicy::default();
        // Legacy inference tests isolate native geometry semantics. M74 datum
        // tests opt reference geometry in explicitly through `datum_frame`.
        geometry_policy.visibility.reference_geometry = false;
        DraftInferenceFrame {
            design_identity: identity(),
            accepted_revision: 1,
            prepared_input: None,
            viewport,
            geometry_policy,
            sample: DraftInferenceSample {
                raw_screen_position: screen,
                subject,
                span_start,
            },
            anchors,
            semantic_centers: Vec::new(),
        }
    }

    fn datum_frame(
        viewport: Viewport,
        screen: ScreenPoint,
        span_start: Option<[f64; 2]>,
        anchors: Vec<DraftReferenceAnchor>,
    ) -> DraftInferenceFrame {
        datum_frame_for_subject(
            viewport,
            screen,
            DraftInferenceSubject::PointOperand,
            span_start,
            anchors,
        )
    }

    fn datum_frame_for_subject(
        viewport: Viewport,
        screen: ScreenPoint,
        subject: DraftInferenceSubject,
        span_start: Option<[f64; 2]>,
        anchors: Vec<DraftReferenceAnchor>,
    ) -> DraftInferenceFrame {
        let mut frame = frame_for_subject(viewport, screen, subject, span_start, anchors);
        frame.geometry_policy.visibility.reference_geometry = true;
        frame
    }

    fn point_anchor(value: u128, position: [f64; 2]) -> DraftReferenceAnchor {
        DraftReferenceAnchor::PersistentPoint {
            point: point_id(value),
            model_position: position,
            role_incidence: ScenePointRoleIncidence {
                profile: true,
                construction: false,
            },
        }
    }

    fn semantic_center(
        curve: u128,
        center: u128,
        position: [f64; 2],
        role: GeometryRole,
    ) -> DraftSemanticCenterAnchor {
        DraftSemanticCenterAnchor {
            curve: curve_span(curve).curve,
            center: point_id(center),
            model_position: position,
            role,
        }
    }

    fn contact(span: CurveSpan, parameter: f64) -> DraftCurveContact {
        DraftCurveContact {
            span,
            domain: ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            parameter,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
        }
    }

    fn affine_anchor(value: u128, position: [f64; 2], direction: [f64; 2]) -> DraftReferenceAnchor {
        affine_anchor_with_role(value, position, direction, GeometryRole::Profile)
    }

    fn affine_anchor_with_role(
        value: u128,
        position: [f64; 2],
        direction: [f64; 2],
        role: GeometryRole,
    ) -> DraftReferenceAnchor {
        affine_anchor_occurrence(
            value,
            position,
            direction,
            role,
            role,
            DraftReferenceOrigin::Native,
        )
    }

    fn affine_anchor_occurrence(
        value: u128,
        position: [f64; 2],
        direction: [f64; 2],
        role: GeometryRole,
        source_role: GeometryRole,
        origin: DraftReferenceOrigin,
    ) -> DraftReferenceAnchor {
        DraftReferenceAnchor::AffineSupport {
            contact: contact(curve_span(value), 0.5),
            model_position: position,
            affine_direction: direction,
            role,
            source_role,
            origin,
        }
    }

    fn midpoint_anchor(
        value: u128,
        position: [f64; 2],
        direction: [f64; 2],
    ) -> DraftReferenceAnchor {
        DraftReferenceAnchor::Midpoint {
            span: curve_span(value),
            model_position: position,
            affine_direction: direction,
            role: GeometryRole::Profile,
            source_role: GeometryRole::Profile,
            origin: DraftReferenceOrigin::Native,
        }
    }

    fn resolved_candidate(resolution: &DraftInferenceResolution) -> &DraftInferenceCandidate {
        let DraftInferenceStatus::Resolved { candidate } = resolution.status else {
            panic!("expected resolved candidate, got {:?}", resolution.status);
        };
        resolution
            .candidates
            .iter()
            .find(|item| item.id == candidate)
            .expect("resolved candidate")
    }

    #[test]
    fn defaults_and_policy_validation_are_explicit() {
        let policy = DraftInferencePolicy::default();
        policy.validate().expect("default policy");
        assert_eq!(policy.tolerances.point_enter_pixels, 6.0);
        assert_eq!(policy.tolerances.point_exit_pixels, 9.0);
        assert_eq!(policy.tolerances.curve_enter_pixels, 8.0);
        assert_eq!(policy.tolerances.curve_exit_pixels, 12.0);
        assert_eq!(policy.tolerances.datum_origin_enter_pixels, 6.0);
        assert_eq!(policy.tolerances.datum_origin_exit_pixels, 9.0);
        assert_eq!(policy.tolerances.datum_axis_enter_pixels, 4.0);
        assert_eq!(policy.tolerances.datum_axis_exit_pixels, 7.0);
        assert_eq!(
            policy.tolerances.direction_enter_radians.to_bits(),
            3.0_f64.to_radians().to_bits()
        );
        assert_eq!(
            policy.tolerances.direction_exit_radians.to_bits(),
            5.0_f64.to_radians().to_bits()
        );
        assert_eq!(policy.limits.max_candidates, 32);
        assert_eq!(policy.limits.max_remembered_references, 8);
        assert_eq!(policy.limits.max_scene_anchors, 4_096);
        assert_eq!(policy.limits.max_scene_curve_segments, 16_384);

        let invalid = DraftInferencePolicy {
            point_tracking: DraftInferenceBehavior {
                show_guides: true,
                adjust_coordinates: false,
                persist_constraint: true,
            },
            ..policy
        };
        assert_eq!(
            DraftInferenceEngine::new(invalid)
                .expect_err("durable stored-point alignment must also adjust the draft"),
            DraftInferenceError::InvalidPolicy
        );
        let persist_only_identity = DraftInferencePolicy {
            point_identity: DraftInferenceBehavior {
                show_guides: true,
                adjust_coordinates: false,
                persist_constraint: true,
            },
            ..policy
        };
        assert_eq!(
            DraftInferenceEngine::new(persist_only_identity)
                .expect_err("structural point reuse must adjust to its accepted identity"),
            DraftInferenceError::InvalidPolicy
        );
        for limits in [
            DraftInferenceLimits {
                max_candidates: MAX_CONFIGURED_CANDIDATES + 1,
                ..policy.limits
            },
            DraftInferenceLimits {
                max_remembered_references: MAX_CONFIGURED_REFERENCES + 1,
                ..policy.limits
            },
            DraftInferenceLimits {
                max_scene_anchors: 0,
                ..policy.limits
            },
            DraftInferenceLimits {
                max_scene_curve_segments: 0,
                ..policy.limits
            },
        ] {
            assert_eq!(
                DraftInferenceEngine::new(DraftInferencePolicy { limits, ..policy })
                    .expect_err("configured resource limits must remain in bounds"),
                DraftInferenceError::InvalidPolicy
            );
        }
    }

    #[test]
    fn datum_origin_uses_point_hysteresis_and_suppresses_both_axis_candidates() {
        let view = viewport(50.0);
        let resolve = |engine: &mut DraftInferenceEngine, offset: [f64; 2]| {
            let origin = view.model_to_screen([0.0, 0.0]);
            engine
                .resolve(
                    &datum_frame(
                        view,
                        ScreenPoint {
                            x: origin.x + offset[0],
                            y: origin.y + offset[1],
                        },
                        None,
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("datum-origin resolution")
        };

        let mut engine = DraftInferenceEngine::default();
        let entered = resolve(&mut engine, [3.0, 3.0]);
        assert_eq!(entered.candidates.len(), 1);
        let candidate = resolved_candidate(&entered);
        assert_eq!(
            candidate.relations,
            vec![DraftInferenceRelation::CoincidentWithOrigin]
        );
        assert_eq!(candidate.adjusted_model_position, [0.0, 0.0]);
        assert_eq!(
            candidate.ranking.anchor_priority,
            DraftAnchorPriority::DatumOrigin
        );
        assert!(candidate.references.is_empty());
        assert!(candidate.guides.iter().all(|guide| {
            guide.family == DraftInferenceFamily::DatumOrigin
                && guide.geometry
                    == DraftGuideGeometry::Point {
                        position: [0.0, 0.0],
                    }
        }));
        assert_eq!(engine.active_datum, Some(DatumKey::Origin));

        let retained = resolve(&mut engine, [9.0, 0.0]);
        assert_eq!(
            resolved_candidate(&retained).relations,
            vec![DraftInferenceRelation::CoincidentWithOrigin]
        );
        let mut fresh = DraftInferenceEngine::default();
        assert_eq!(
            resolved_candidate(&resolve(&mut fresh, [9.0, 0.0])).relations,
            vec![DraftInferenceRelation::PointOnDatumAxis {
                axis: DocumentCoordinateAxis::X,
            }]
        );
    }

    #[test]
    fn datum_axes_use_perpendicular_screen_hysteresis_and_project_symmetrically() {
        let view = viewport(50.0);
        for (raw, expected, axis, key) in [
            (
                [4.0, 4.0 / 50.0],
                [4.0, 0.0],
                DocumentCoordinateAxis::X,
                DatumKey::XAxis,
            ),
            (
                [4.0 / 50.0, 4.0],
                [0.0, 4.0],
                DocumentCoordinateAxis::Y,
                DatumKey::YAxis,
            ),
        ] {
            let mut engine = DraftInferenceEngine::default();
            let entered = engine
                .resolve(
                    &datum_frame(view, view.model_to_screen(raw), None, Vec::new()),
                    DraftInferenceInput::default(),
                )
                .expect("datum-axis entry");
            let candidate = resolved_candidate(&entered);
            assert_eq!(
                candidate.relations,
                vec![DraftInferenceRelation::PointOnDatumAxis { axis }]
            );
            assert_eq!(candidate.adjusted_model_position, expected);
            assert_eq!(
                candidate.ranking.anchor_priority,
                DraftAnchorPriority::DatumAxis
            );
            assert!(candidate.guides.iter().any(|guide| {
                guide.family == DraftInferenceFamily::DatumAxis
                    && guide.geometry
                        == DraftGuideGeometry::Segment {
                            start: [0.0, 0.0],
                            end: expected,
                        }
            }));
            assert_eq!(engine.active_datum, Some(key));

            let retained_raw = match key {
                DatumKey::XAxis => [4.0, 7.0 / 50.0],
                DatumKey::YAxis => [7.0 / 50.0, 4.0],
                DatumKey::Origin => unreachable!(),
            };
            assert!(matches!(
                engine
                    .resolve(
                        &datum_frame(view, view.model_to_screen(retained_raw), None, Vec::new()),
                        DraftInferenceInput::default(),
                    )
                    .expect("datum-axis exit boundary")
                    .status,
                DraftInferenceStatus::Resolved { .. }
            ));
            let mut fresh = DraftInferenceEngine::default();
            assert_eq!(
                fresh
                    .resolve(
                        &datum_frame(view, view.model_to_screen(retained_raw), None, Vec::new()),
                        DraftInferenceInput::default(),
                    )
                    .expect("unlatched datum-axis exit boundary")
                    .status,
                DraftInferenceStatus::None
            );
        }
    }

    #[test]
    fn native_point_anchor_outranks_datum_and_datum_acquires_no_wake_memory() {
        let view = viewport(50.0);
        let target = [4.0, 0.0];
        let anchor = point_anchor(700, target);
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &datum_frame(view, view.model_to_screen(target), None, vec![anchor]),
                DraftInferenceInput::default(),
            )
            .expect("native point over datum axis");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![DraftInferenceRelation::PointIdentity {
                point: point_id(700),
            }]
        );
        assert!(resolution.candidates.iter().any(|candidate| {
            candidate.relations
                == vec![DraftInferenceRelation::PointOnDatumAxis {
                    axis: DocumentCoordinateAxis::X,
                }]
        }));
        assert_eq!(engine.active_datum, None);

        let mut datum_only = DraftInferenceEngine::default();
        datum_only
            .resolve(
                &datum_frame(view, view.model_to_screen(target), None, Vec::new()),
                DraftInferenceInput::default(),
            )
            .expect("datum-only axis");
        assert!(datum_only.remembered_references().is_empty());
    }

    #[test]
    fn datum_inference_is_point_stage_only_and_shift_suppression_clears_its_latch() {
        let view = viewport(50.0);
        let raw = [4.0, 0.0];
        let mut circle = DraftInferenceEngine::default();
        assert_eq!(
            circle
                .resolve(
                    &datum_frame_for_subject(
                        view,
                        view.model_to_screen(raw),
                        DraftInferenceSubject::CircleCircumference,
                        None,
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("circle circumference excludes datums")
                .status,
            DraftInferenceStatus::None
        );
        assert_eq!(circle.active_datum, None);

        let mut hidden_frame = datum_frame(view, view.model_to_screen(raw), None, Vec::new());
        hidden_frame.geometry_policy.visibility.reference_geometry = false;
        let mut hidden = DraftInferenceEngine::default();
        assert_eq!(
            hidden
                .resolve(&hidden_frame, DraftInferenceInput::default())
                .expect("hidden reference geometry excludes datums")
                .status,
            DraftInferenceStatus::None
        );
        assert_eq!(hidden.active_datum, None);

        let mut centered = DraftInferenceEngine::default();
        assert!(matches!(
            centered
                .resolve(
                    &datum_frame_for_subject(
                        view,
                        view.model_to_screen(raw),
                        DraftInferenceSubject::CenteredPointOperand {
                            prospective_curve_index: 0,
                        },
                        None,
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("center point admits datum")
                .status,
            DraftInferenceStatus::Resolved { .. }
        ));
        assert_eq!(centered.active_datum, Some(DatumKey::XAxis));

        let suppressed = centered
            .resolve(
                &datum_frame_for_subject(
                    view,
                    view.model_to_screen(raw),
                    DraftInferenceSubject::CenteredPointOperand {
                        prospective_curve_index: 0,
                    },
                    None,
                    Vec::new(),
                ),
                DraftInferenceInput {
                    suppressed: true,
                    preferred_candidate: None,
                },
            )
            .expect("shift suppression");
        assert_eq!(suppressed.status, DraftInferenceStatus::Suppressed);
        assert!(suppressed.candidates.is_empty());
        assert!(suppressed.guides.is_empty());
        assert_eq!(centered.active_datum, None);
        assert!(centered.remembered_references().is_empty());
    }

    #[test]
    fn live_world_direction_supersedes_same_coordinate_datum_axis() {
        let view = viewport(50.0);
        for (start, raw, relation, suppressed_axis) in [
            (
                [-4.0, 0.0],
                [4.0, 0.04],
                DraftInferenceRelation::Horizontal,
                DocumentCoordinateAxis::X,
            ),
            (
                [0.0, -4.0],
                [0.04, 4.0],
                DraftInferenceRelation::Vertical,
                DocumentCoordinateAxis::Y,
            ),
        ] {
            let mut engine = DraftInferenceEngine::default();
            let resolution = engine
                .resolve(
                    &datum_frame(view, view.model_to_screen(raw), Some(start), Vec::new()),
                    DraftInferenceInput::default(),
                )
                .expect("same-coordinate datum precedence");
            assert_eq!(resolution.candidates.len(), 1);
            assert_eq!(resolved_candidate(&resolution).relations, vec![relation]);
            assert!(resolution.candidates.iter().all(|candidate| {
                !candidate
                    .relations
                    .contains(&DraftInferenceRelation::PointOnDatumAxis {
                        axis: suppressed_axis,
                    })
            }));
            assert_eq!(engine.active_datum, None);
        }
    }

    #[test]
    fn datum_axis_composes_with_orthogonal_live_world_direction() {
        let view = viewport(50.0);
        for (start, raw, adjusted, axis, direction) in [
            (
                [2.0, -4.0],
                [2.04, 0.04],
                [2.0, 0.0],
                DocumentCoordinateAxis::X,
                DraftInferenceRelation::Vertical,
            ),
            (
                [-4.0, 2.0],
                [0.04, 2.04],
                [0.0, 2.0],
                DocumentCoordinateAxis::Y,
                DraftInferenceRelation::Horizontal,
            ),
        ] {
            let mut engine = DraftInferenceEngine::default();
            let resolution = engine
                .resolve(
                    &datum_frame(view, view.model_to_screen(raw), Some(start), Vec::new()),
                    DraftInferenceInput::default(),
                )
                .expect("orthogonal datum-direction bundle");
            assert_eq!(resolution.candidates.len(), 1);
            let candidate = resolved_candidate(&resolution);
            assert_eq!(
                candidate.relations,
                vec![DraftInferenceRelation::PointOnDatumAxis { axis }, direction,]
            );
            assert_eq!(candidate.adjusted_model_position, adjusted);
            assert_eq!(candidate.ranking.persistent_relation_count, 2);
            assert_eq!(candidate.guides.len(), 2);
            assert!(candidate.guides.iter().all(|guide| {
                guide.classification == DraftGuideClassification::ConstraintBacked
            }));
        }
    }

    #[test]
    fn m71_f006_tighter_default_capture_envelope_excludes_old_only_entry_samples() {
        let point_view = viewport(1.0);
        let point = DraftInferenceEngine::default()
            .resolve(
                &frame(
                    point_view,
                    ScreenPoint { x: 507.0, y: 350.0 },
                    None,
                    vec![point_anchor(901, [0.0, 0.0])],
                ),
                DraftInferenceInput::default(),
            )
            .expect("seven-pixel point sample");
        assert_eq!(point.status, DraftInferenceStatus::None);

        let curve = DraftInferenceEngine::default()
            .resolve(
                &frame(
                    point_view,
                    ScreenPoint { x: 509.0, y: 350.0 },
                    None,
                    vec![affine_anchor(902, [0.0, 0.0], [1.0, 0.0])],
                ),
                DraftInferenceInput::default(),
            )
            .expect("nine-pixel curve sample");
        assert_eq!(curve.status, DraftInferenceStatus::None);

        let direction_view = viewport(50.0);
        let angle = 3.5_f64.to_radians();
        let direction = DraftInferenceEngine::default()
            .resolve(
                &frame(
                    direction_view,
                    direction_view.model_to_screen([3.0 * angle.cos(), 3.0 * angle.sin()]),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("three-and-a-half-degree direction sample");
        assert_eq!(direction.status, DraftInferenceStatus::None);
    }

    #[test]
    fn public_engine_scene_anchor_limit_fails_closed_before_anchor_traversal() {
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_scene_anchors = 2;
        let mut engine = DraftInferenceEngine::new(policy).expect("bounded engine");
        let remembered = point_anchor(99, [-1.0, 0.0]);
        engine
            .remember_reference(remembered)
            .expect("valid remembered reference");
        let view = viewport(50.0);
        let target = [0.0, 0.0];
        let anchors = vec![
            point_anchor(1, target),
            point_anchor(2, [100.0, 100.0]),
            // The count bound must win before direct-engine traversal reaches
            // malformed excess input.
            point_anchor(3, [f64::NAN, 0.0]),
        ];
        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen(target), None, anchors),
                DraftInferenceInput::default(),
            )
            .expect("typed resource result");
        assert_eq!(resolution.status, DraftInferenceStatus::ResourceLimited);
        assert_eq!(
            resolution.completeness,
            DraftInferenceCompleteness::SceneLimit(DraftInferenceSceneLimit {
                resource: DraftInferenceSceneResource::Anchors,
                required: 3,
                limit: 2,
            })
        );
        assert_eq!(resolution.raw_model_position, target);
        assert_eq!(resolution.adjusted_model_position, target);
        assert!(resolution.candidates.is_empty());
        assert!(resolution.guides.is_empty());
        assert!(engine.remembered_references().is_empty());

        let exact_limit = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(target),
                    None,
                    vec![point_anchor(1, target), point_anchor(2, [100.0, 100.0])],
                ),
                DraftInferenceInput::default(),
            )
            .expect("exact anchor bound remains admissible");
        assert!(matches!(
            exact_limit.status,
            DraftInferenceStatus::Resolved { .. }
        ));
        assert_eq!(
            exact_limit.completeness,
            DraftInferenceCompleteness::Complete
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one table-driven topology contract keeps every domain/neighborhood case comparable"
    )]
    fn curve_contact_validation_matches_domain_and_neighborhood_topology() {
        let span = curve_span(9);
        let bounded_start = DraftCurveContact {
            span,
            domain: ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            parameter: 0.0,
            winding: 0,
            neighborhood: ContactNeighborhood::Start,
        };
        assert!(bounded_start.is_valid());
        assert!(
            !DraftCurveContact {
                neighborhood: ContactNeighborhood::Interior,
                ..bounded_start
            }
            .is_valid()
        );
        assert!(
            !DraftCurveContact {
                parameter: 0.5,
                neighborhood: ContactNeighborhood::Local {
                    lower: 0.6,
                    upper: 0.8,
                },
                ..bounded_start
            }
            .is_valid()
        );
        assert!(
            !DraftCurveContact {
                parameter: 0.5,
                neighborhood: ContactNeighborhood::Local {
                    lower: -0.1,
                    upper: 0.8,
                },
                ..bounded_start
            }
            .is_valid()
        );

        let supporting = DraftCurveContact {
            span,
            domain: ContactDomain::SupportingLine,
            parameter: 2.0,
            winding: 0,
            neighborhood: ContactNeighborhood::Local {
                lower: 1.0,
                upper: 3.0,
            },
        };
        assert!(supporting.is_valid());
        assert!(
            !DraftCurveContact {
                winding: 1,
                ..supporting
            }
            .is_valid()
        );
        assert!(
            !DraftCurveContact {
                neighborhood: ContactNeighborhood::End,
                ..supporting
            }
            .is_valid()
        );

        let period = std::f64::consts::TAU;
        let total = period.mul_add(2.0, 0.25);
        let periodic = DraftCurveContact {
            span,
            domain: ContactDomain::Periodic { period },
            parameter: 0.25,
            winding: 2,
            neighborhood: ContactNeighborhood::Local {
                lower: total - 0.1,
                upper: total + 0.1,
            },
        };
        assert!(periodic.is_valid());
        assert!(
            !DraftCurveContact {
                parameter: period,
                neighborhood: ContactNeighborhood::Interior,
                ..periodic
            }
            .is_valid()
        );
        assert!(
            !DraftCurveContact {
                neighborhood: ContactNeighborhood::Start,
                ..periodic
            }
            .is_valid()
        );
        assert!(
            !DraftCurveContact {
                neighborhood: ContactNeighborhood::Local {
                    lower: 0.1,
                    upper: 0.4,
                },
                ..periodic
            }
            .is_valid()
        );

        let invalid_anchor = DraftReferenceAnchor::CurvePoint {
            contact: DraftCurveContact {
                winding: 1,
                ..supporting
            },
            branch_candidate: DraftCurveBranchCandidate::default(),
            model_position: [0.0, 0.0],
            role: GeometryRole::Profile,
            source_role: GeometryRole::Profile,
            origin: DraftReferenceOrigin::Native,
        };
        assert_eq!(
            DraftInferenceEngine::default()
                .remember_reference(invalid_anchor)
                .expect_err("public reference ingestion rejects inconsistent metadata"),
            DraftInferenceError::InvalidFrame
        );
    }

    #[test]
    fn periodic_seam_chords_resolve_as_one_semantic_contact() {
        let period = std::f64::consts::TAU;
        let span = curve_span(10);
        let seam_anchor = |branch: u32, winding: i32| {
            let total = period * f64::from(winding);
            DraftReferenceAnchor::CurvePoint {
                contact: DraftCurveContact {
                    span,
                    domain: ContactDomain::Periodic { period },
                    parameter: 0.0,
                    winding,
                    neighborhood: ContactNeighborhood::Local {
                        lower: total - 0.25,
                        upper: total + 0.25,
                    },
                },
                branch_candidate: DraftCurveBranchCandidate::from_ordinal(branch),
                model_position: [2.0, 0.0],
                role: GeometryRole::Profile,
                source_role: GeometryRole::Profile,
                origin: DraftReferenceOrigin::Native,
            }
        };
        let view = viewport(50.0);
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([2.0, 0.0]),
                    None,
                    vec![seam_anchor(4, 1), seam_anchor(3, 0)],
                ),
                DraftInferenceInput::default(),
            )
            .expect("periodic seam inference");

        let selected = resolved_candidate(&resolution);
        assert_eq!(resolution.candidates.len(), 1);
        assert!(matches!(
            selected.relations.as_slice(),
            [DraftInferenceRelation::PointOnCurve { contact }]
                if contact.span == span
                    && contact.parameter.to_bits() == 0.0f64.to_bits()
                    && contact.winding == 0
        ));
    }

    #[test]
    fn persistent_point_identity_beats_midpoint_and_curve_contact() {
        let view = viewport(50.0);
        let position = [2.0, 1.0];
        let screen = view.model_to_screen(position);
        let span = curve_span(20);
        let anchors = vec![
            affine_anchor(20, position, [1.0, 0.0]),
            midpoint_anchor(20, position, [1.0, 0.0]),
            point_anchor(10, position),
        ];
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(view, screen, None, anchors),
                DraftInferenceInput::default(),
            )
            .expect("resolve");
        let selected = resolved_candidate(&resolution);
        assert_eq!(
            selected.relations,
            vec![DraftInferenceRelation::PointIdentity {
                point: point_id(10)
            }]
        );
        assert!(!selected.relations.iter().any(|relation| matches!(
            relation,
            DraftInferenceRelation::PointOnCurve { contact } if contact.span == span
        )));
    }

    #[test]
    fn midpoint_beats_generic_point_on_same_affine_support() {
        let view = viewport(40.0);
        let position = [1.0, 2.0];
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(position),
                    None,
                    vec![
                        affine_anchor(31, position, [1.0, 0.0]),
                        midpoint_anchor(31, position, [1.0, 0.0]),
                    ],
                ),
                DraftInferenceInput::default(),
            )
            .expect("resolve");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![DraftInferenceRelation::Midpoint {
                span: curve_span(31)
            }]
        );
    }

    #[test]
    fn point_hysteresis_uses_inclusive_enter_and_exit_boundaries() {
        let view = viewport(1.0);
        let anchor = point_anchor(1, [0.0, 0.0]);
        let policy = DraftInferencePolicy {
            point_tracking: DraftInferenceBehavior::tracking_only(),
            ..DraftInferencePolicy::default()
        };
        let mut engine = DraftInferenceEngine::new(policy).expect("point boundary policy");
        let enter = frame(view, ScreenPoint { x: 506.0, y: 350.0 }, None, vec![anchor]);
        assert!(matches!(
            engine
                .resolve(&enter, DraftInferenceInput::default())
                .expect("enter")
                .status,
            DraftInferenceStatus::Resolved { .. }
        ));
        let leave_boundary = frame(view, ScreenPoint { x: 509.0, y: 350.0 }, None, vec![anchor]);
        assert!(matches!(
            engine
                .resolve(&leave_boundary, DraftInferenceInput::default())
                .expect("exit boundary")
                .status,
            DraftInferenceStatus::Resolved { .. }
        ));
        let outside = frame(
            view,
            ScreenPoint {
                x: 509.000_001,
                y: 350.0,
            },
            None,
            vec![anchor],
        );
        assert_eq!(
            engine
                .resolve(&outside, DraftInferenceInput::default())
                .expect("outside")
                .status,
            DraftInferenceStatus::None
        );
    }

    #[test]
    fn curve_hysteresis_uses_inclusive_enter_and_exit_boundaries() {
        let view = viewport(1.0);
        let anchor = affine_anchor(2, [0.0, 0.0], [1.0, 0.0]);
        let mut engine = DraftInferenceEngine::default();
        let enter = frame(view, ScreenPoint { x: 508.0, y: 350.0 }, None, vec![anchor]);
        assert!(matches!(
            engine
                .resolve(&enter, DraftInferenceInput::default())
                .expect("curve enter boundary")
                .status,
            DraftInferenceStatus::Resolved { .. }
        ));
        let exit = frame(view, ScreenPoint { x: 512.0, y: 350.0 }, None, vec![anchor]);
        assert!(matches!(
            engine
                .resolve(&exit, DraftInferenceInput::default())
                .expect("curve exit boundary")
                .status,
            DraftInferenceStatus::Resolved { .. }
        ));
        let outside = frame(
            view,
            ScreenPoint {
                x: 512.000_001,
                y: 350.0,
            },
            None,
            vec![anchor],
        );
        assert_eq!(
            engine
                .resolve(&outside, DraftInferenceInput::default())
                .expect("outside curve exit boundary")
                .status,
            DraftInferenceStatus::None
        );

        let mut fresh = DraftInferenceEngine::default();
        assert_eq!(
            fresh
                .resolve(&exit, DraftInferenceInput::default())
                .expect("unlatched curve exit boundary")
                .status,
            DraftInferenceStatus::None,
            "the curve exit band is available only to the retained semantic owner"
        );
    }

    #[test]
    fn direction_hysteresis_uses_inclusive_enter_and_exit_boundaries() {
        let view = viewport(50.0);
        let start = [0.0, 0.0];
        let enter_angle = 4.0_f64.to_radians();
        let exit_angle = 6.0_f64.to_radians();
        let enter_target = [3.0 * enter_angle.cos(), 3.0 * enter_angle.sin()];
        let exit_target = [3.0 * exit_angle.cos(), 3.0 * exit_angle.sin()];
        let enter_screen = view.model_to_screen(enter_target);
        let exit_screen = view.model_to_screen(exit_target);
        let enter_sample = view.screen_to_model(enter_screen);
        let exit_sample = view.screen_to_model(exit_screen);

        // Use the exact values produced by the resolver's public vector inputs
        // so this test exercises inclusive comparisons rather than relying on
        // a platform-specific trigonometric rounding side of the boundary.
        let mut policy = DraftInferencePolicy::default();
        policy.tolerances.direction_enter_radians =
            undirected_angle_error(enter_sample, [1.0, 0.0]);
        policy.tolerances.direction_exit_radians = undirected_angle_error(exit_sample, [1.0, 0.0]);
        let mut engine = DraftInferenceEngine::new(policy).expect("boundary policy");

        let enter = frame(view, enter_screen, Some(start), Vec::new());
        assert_eq!(
            resolved_candidate(
                &engine
                    .resolve(&enter, DraftInferenceInput::default())
                    .expect("direction enter boundary")
            )
            .relations,
            vec![DraftInferenceRelation::Horizontal]
        );

        let exit = frame(view, exit_screen, Some(start), Vec::new());
        assert_eq!(
            resolved_candidate(
                &engine
                    .resolve(&exit, DraftInferenceInput::default())
                    .expect("direction exit boundary")
            )
            .relations,
            vec![DraftInferenceRelation::Horizontal]
        );

        let outside_angle = exit_angle + 1.0e-6;
        let outside_target = [3.0 * outside_angle.cos(), 3.0 * outside_angle.sin()];
        let outside = frame(
            view,
            view.model_to_screen(outside_target),
            Some(start),
            Vec::new(),
        );
        assert_eq!(
            engine
                .resolve(&outside, DraftInferenceInput::default())
                .expect("outside direction exit boundary")
                .status,
            DraftInferenceStatus::None
        );

        let mut fresh = DraftInferenceEngine::new(policy).expect("fresh boundary policy");
        assert_eq!(
            fresh
                .resolve(&exit, DraftInferenceInput::default())
                .expect("unlatched exit boundary")
                .status,
            DraftInferenceStatus::None,
            "the exit boundary is retained only after entering the tighter band"
        );
    }

    #[test]
    fn bare_point_tracking_uses_inclusive_enter_and_exit_hysteresis() {
        let view = viewport(50.0);
        let origin = [0.0, 0.0];
        let enter_angle = 4.0_f64.to_radians();
        let exit_angle = 6.0_f64.to_radians();
        let enter_target = [3.0 * enter_angle.cos(), 3.0 * enter_angle.sin()];
        let exit_target = [3.0 * exit_angle.cos(), 3.0 * exit_angle.sin()];
        let enter_screen = view.model_to_screen(enter_target);
        let exit_screen = view.model_to_screen(exit_target);
        let enter_sample = view.screen_to_model(enter_screen);
        let exit_sample = view.screen_to_model(exit_screen);
        let mut policy = DraftInferencePolicy {
            point_tracking: DraftInferenceBehavior::tracking_only(),
            ..DraftInferencePolicy::default()
        };
        policy.tolerances.direction_enter_radians =
            undirected_angle_error(enter_sample, [1.0, 0.0]);
        policy.tolerances.direction_exit_radians = undirected_angle_error(exit_sample, [1.0, 0.0]);

        let resolve = |engine: &mut DraftInferenceEngine, target: [f64; 2]| {
            engine
                .resolve(
                    &frame(view, view.model_to_screen(target), None, Vec::new()),
                    DraftInferenceInput::default(),
                )
                .expect("point-tracking resolution")
        };
        let has_tracking = |resolution: &DraftInferenceResolution| {
            resolution.guides.iter().any(|guide| {
                guide.family == DraftInferenceFamily::PointTracking
                    && guide.reference == Some(point_anchor(60, origin))
            })
        };

        let mut engine = DraftInferenceEngine::new(policy).expect("boundary policy");
        engine
            .remember_reference(point_anchor(60, origin))
            .expect("remember point");
        assert!(has_tracking(&resolve(&mut engine, enter_sample)));
        assert!(has_tracking(&resolve(&mut engine, exit_sample)));

        let outside_angle = exit_angle + 1.0e-6;
        let outside_target = [3.0 * outside_angle.cos(), 3.0 * outside_angle.sin()];
        assert!(!has_tracking(&resolve(&mut engine, outside_target)));

        let mut fresh = DraftInferenceEngine::new(policy).expect("fresh boundary policy");
        fresh
            .remember_reference(point_anchor(60, origin))
            .expect("remember point");
        assert!(
            !has_tracking(&resolve(&mut fresh, exit_sample)),
            "the wider exit band is available only to an already latched point guide"
        );
    }

    #[test]
    fn higher_priority_point_overrides_a_curve_anchor_latch() {
        let view = viewport(1.0);
        let support = affine_anchor(4, [0.0, 0.0], [1.0, 0.0]);
        let mut engine = DraftInferenceEngine::default();
        engine
            .resolve(
                &frame(view, view.model_to_screen([0.0, 0.0]), None, vec![support]),
                DraftInferenceInput::default(),
            )
            .expect("latch curve");

        let point = point_anchor(5, [5.0, 0.0]);
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([5.0, 0.0]),
                    None,
                    vec![support, point],
                ),
                DraftInferenceInput::default(),
            )
            .expect("higher-priority point");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![DraftInferenceRelation::PointIdentity { point: point_id(5) }]
        );
    }

    #[test]
    fn remembered_collinear_beats_equivalent_world_axis() {
        let view = viewport(50.0);
        let reference = affine_anchor(40, [0.0, 0.0], [1.0, 0.0]);
        let mut engine = DraftInferenceEngine::default();
        engine.remember_reference(reference).expect("remember");
        let endpoint = [3.0, 0.1];
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(endpoint),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("resolve");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![DraftInferenceRelation::Collinear {
                reference: curve_span(40)
            }]
        );
    }

    #[test]
    fn offset_span_start_keeps_parallel_distinct_from_collinear() {
        let view = viewport(50.0);
        let reference = affine_anchor(401, [0.0, 0.0], [1.0, 0.0]);
        let mut engine = DraftInferenceEngine::default();
        engine.remember_reference(reference).expect("remember");
        let start = [0.0, 1.0];
        let endpoint = [3.0, 1.1];
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(endpoint),
                    Some(start),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("resolve");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![DraftInferenceRelation::Parallel {
                reference: curve_span(401)
            }]
        );
        assert!(resolution.candidates.iter().all(|candidate| {
            !candidate.relations.iter().any(|relation| {
                matches!(
                    relation,
                    DraftInferenceRelation::Collinear { reference }
                        if *reference == curve_span(401)
                )
            })
        }));
    }

    #[test]
    fn remembered_reference_overrides_a_world_axis_latch() {
        let view = viewport(50.0);
        let endpoint = [3.0, 0.1];
        let mut engine = DraftInferenceEngine::default();
        let input_frame = frame(
            view,
            view.model_to_screen(endpoint),
            Some([0.0, 0.0]),
            Vec::new(),
        );
        assert_eq!(
            resolved_candidate(
                &engine
                    .resolve(&input_frame, DraftInferenceInput::default())
                    .expect("world-axis latch")
            )
            .relations,
            vec![DraftInferenceRelation::Horizontal]
        );

        let reference = affine_anchor(41, [0.0, 0.0], endpoint);
        engine.remember_reference(reference).expect("remember");
        assert_eq!(
            resolved_candidate(
                &engine
                    .resolve(&input_frame, DraftInferenceInput::default())
                    .expect("remembered override")
            )
            .relations,
            vec![DraftInferenceRelation::Collinear {
                reference: curve_span(41)
            }]
        );
    }

    #[test]
    fn remembered_midpoint_supports_a_normal_bundle() {
        let view = viewport(50.0);
        let midpoint = midpoint_anchor(50, [1.0, 1.0], [1.0, 0.0]);
        let mut engine = DraftInferenceEngine::default();
        engine.remember_reference(midpoint).expect("remember");
        let endpoint = [1.05, 4.0];
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(endpoint),
                    Some([1.0, 1.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("resolve");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![DraftInferenceRelation::Perpendicular {
                reference: curve_span(50)
            }]
        );
    }

    #[test]
    fn remembered_native_midpoint_alignment_is_constraint_backed_on_both_axes() {
        let view = viewport(50.0);
        let point = midpoint_anchor(60, [0.0, 1.0], [1.0, 0.0]);
        for (raw, adjusted, expected_relation, expected_family) in [
            (
                [4.0, 1.05],
                [4.0, 1.0],
                DraftInferenceRelation::HorizontalPointToMidpoint {
                    reference: curve_span(60),
                },
                DraftInferenceFamily::HorizontalPointToMidpoint,
            ),
            (
                [0.05, 5.0],
                [0.0, 5.0],
                DraftInferenceRelation::VerticalPointToMidpoint {
                    reference: curve_span(60),
                },
                DraftInferenceFamily::VerticalPointToMidpoint,
            ),
        ] {
            let mut engine = DraftInferenceEngine::default();
            engine.remember_reference(point).expect("remember");
            let resolution = engine
                .resolve(
                    &frame(view, view.model_to_screen(raw), None, Vec::new()),
                    DraftInferenceInput::default(),
                )
                .expect("resolve");
            let candidate = resolved_candidate(&resolution);
            assert_eq!(candidate.adjusted_model_position, adjusted);
            assert_eq!(candidate.relations, vec![expected_relation]);
            assert!(candidate.guides.iter().any(|guide| {
                guide.classification == DraftGuideClassification::ConstraintBacked
                    && guide.family == expected_family
            }));
        }
    }

    #[test]
    fn orthogonal_point_axes_and_world_directions_form_exact_bundles() {
        let view = viewport(50.0);
        for (reference, start, raw, adjusted, expected) in [
            (
                point_anchor(605, [-4.0, 4.0]),
                [0.0, 0.0],
                [0.04, 4.05],
                [0.0, 4.0],
                vec![
                    DraftInferenceRelation::HorizontalPoints {
                        reference: point_id(605),
                    },
                    DraftInferenceRelation::Vertical,
                ],
            ),
            (
                point_anchor(606, [4.0, -4.0]),
                [0.0, 0.0],
                [4.05, 0.04],
                [4.0, 0.0],
                vec![
                    DraftInferenceRelation::VerticalPoints {
                        reference: point_id(606),
                    },
                    DraftInferenceRelation::Horizontal,
                ],
            ),
            (
                midpoint_anchor(607, [-4.0, 4.0], [1.0, 1.0]),
                [0.0, 0.0],
                [0.04, 4.05],
                [0.0, 4.0],
                vec![
                    DraftInferenceRelation::HorizontalPointToMidpoint {
                        reference: curve_span(607),
                    },
                    DraftInferenceRelation::Vertical,
                ],
            ),
            (
                midpoint_anchor(608, [4.0, -4.0], [1.0, 1.0]),
                [0.0, 0.0],
                [4.05, 0.04],
                [4.0, 0.0],
                vec![
                    DraftInferenceRelation::VerticalPointToMidpoint {
                        reference: curve_span(608),
                    },
                    DraftInferenceRelation::Horizontal,
                ],
            ),
        ] {
            let mut engine = DraftInferenceEngine::default();
            engine.remember_reference(reference).expect("remember axis");
            let input = frame(view, view.model_to_screen(raw), Some(start), Vec::new());
            let first = engine
                .resolve(&input, DraftInferenceInput::default())
                .expect("composed axis resolution");
            let candidate = resolved_candidate(&first);
            assert_eq!(candidate.adjusted_model_position, adjusted);
            assert_eq!(candidate.relations, expected);
            assert_eq!(candidate.ranking.persistent_relation_count, 2);
            assert_eq!(first.candidates.len(), 1);
            assert!(candidate.guides.iter().all(|guide| {
                guide.classification == DraftGuideClassification::ConstraintBacked
            }));

            let id = candidate.id;
            let repeated = engine
                .resolve(&input, DraftInferenceInput::default())
                .expect("stable composed axis resolution");
            assert!(matches!(
                repeated.status,
                DraftInferenceStatus::Resolved { candidate } if candidate == id
            ));
        }
    }

    #[test]
    fn axis_bundle_composes_axis_aligned_remembered_directions_too() {
        let view = viewport(50.0);
        let midpoint = midpoint_anchor(609, [-4.0, 4.0], [1.0, 0.0]);
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(midpoint)
            .expect("remember midpoint");
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([0.04, 4.05]),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("midpoint axis plus normal");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![
                DraftInferenceRelation::HorizontalPointToMidpoint {
                    reference: curve_span(609),
                },
                DraftInferenceRelation::Perpendicular {
                    reference: curve_span(609),
                },
            ]
        );
        assert_eq!(resolution.adjusted_model_position, [0.0, 4.0]);
    }

    #[test]
    fn orthogonal_point_axes_from_distinct_references_form_one_candidate() {
        let view = viewport(50.0);
        let horizontal_origin = [-4.0, 4.0];
        let vertical_origin = [3.0, -4.0];
        let horizontal = point_anchor(621, horizontal_origin);
        let vertical = point_anchor(622, vertical_origin);
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(horizontal)
            .expect("horizontal reference");
        engine
            .remember_reference(vertical)
            .expect("vertical reference");

        let input = frame(
            view,
            view.model_to_screen([3.04, 4.05]),
            Some([0.0, 0.0]),
            Vec::new(),
        );
        let resolution = engine
            .resolve(&input, DraftInferenceInput::default())
            .expect("orthogonal point-axis resolution");
        let candidate = resolved_candidate(&resolution);
        assert_eq!(resolution.candidates.len(), 1);
        assert_eq!(candidate.adjusted_model_position, [3.0, 4.0]);
        assert_eq!(
            candidate.relations,
            vec![
                DraftInferenceRelation::HorizontalPoints {
                    reference: point_id(621),
                },
                DraftInferenceRelation::VerticalPoints {
                    reference: point_id(622),
                },
            ]
        );
        assert_eq!(candidate.ranking.persistent_relation_count, 2);
        let sampled = candidate.raw_model_position;
        let horizontal_error = undirected_angle_error(
            [
                sampled[0] - horizontal_origin[0],
                sampled[1] - horizontal_origin[1],
            ],
            [1.0, 0.0],
        );
        let vertical_error = undirected_angle_error(
            [
                sampled[0] - vertical_origin[0],
                sampled[1] - vertical_origin[1],
            ],
            [0.0, 1.0],
        );
        assert_eq!(
            candidate.ranking.angular_error_radians.to_bits(),
            horizontal_error.max(vertical_error).to_bits()
        );
        assert_eq!(candidate.guides.len(), 2);
        assert!(candidate.guides.iter().all(|guide| {
            guide.classification == DraftGuideClassification::ConstraintBacked
                && matches!(
                    guide.family,
                    DraftInferenceFamily::HorizontalPoints | DraftInferenceFamily::VerticalPoints
                )
        }));

        let candidate_id = candidate.id;
        let repeated = engine
            .resolve(&input, DraftInferenceInput::default())
            .expect("stable orthogonal point-axis resolution");
        assert!(matches!(
            repeated.status,
            DraftInferenceStatus::Resolved { candidate } if candidate == candidate_id
        ));
    }

    #[test]
    fn orthogonal_point_axis_semantic_ties_remain_ambiguous() {
        let view = viewport(50.0);
        let first_horizontal = point_anchor(623, [-4.0, 4.0]);
        let second_horizontal = point_anchor(624, [-4.0, 4.0]);
        let vertical = point_anchor(625, [3.0, -4.0]);
        let mut engine = DraftInferenceEngine::default();
        for reference in [first_horizontal, second_horizontal, vertical] {
            engine
                .remember_reference(reference)
                .expect("remember reference");
        }

        let raw = [3.04, 4.05];
        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen(raw), None, Vec::new()),
                DraftInferenceInput::default(),
            )
            .expect("ambiguous orthogonal point axes");
        assert!(matches!(
            resolution.status,
            DraftInferenceStatus::Ambiguous { ref candidates } if candidates.len() == 2
        ));
        assert_eq!(resolution.candidates.len(), 2);
        assert!(resolution.candidates.iter().all(|candidate| {
            candidate.adjusted_model_position == [3.0, 4.0]
                && candidate.relations.len() == 2
                && matches!(
                    candidate.relations.as_slice(),
                    [
                        DraftInferenceRelation::HorizontalPoints { .. },
                        DraftInferenceRelation::VerticalPoints { reference },
                    ] if *reference == point_id(625)
                )
        }));

        let preferred = resolution.candidates[0].id;
        let selected = engine
            .resolve(
                &frame(view, view.model_to_screen(raw), None, Vec::new()),
                DraftInferenceInput {
                    suppressed: false,
                    preferred_candidate: Some(preferred),
                },
            )
            .expect("explicit semantic preference");
        assert_eq!(
            selected.status,
            DraftInferenceStatus::Resolved {
                candidate: preferred,
            }
        );
        assert_eq!(selected.adjusted_model_position, [3.0, 4.0]);
    }

    #[test]
    fn orthogonal_point_axis_pair_limit_failure_publishes_no_prefix() {
        let view = viewport(50.0);
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_candidates = 1;
        let mut engine = DraftInferenceEngine::new(policy).expect("one-candidate engine");
        for reference in [
            point_anchor(626, [-4.0, 4.0]),
            point_anchor(627, [-4.0, 4.0]),
            point_anchor(628, [3.0, -4.0]),
        ] {
            engine
                .remember_reference(reference)
                .expect("remember reference");
        }

        let raw = [3.04, 4.05];
        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen(raw), None, Vec::new()),
                DraftInferenceInput::default(),
            )
            .expect("bounded orthogonal point-axis generation");
        assert_eq!(resolution.status, DraftInferenceStatus::ResourceLimited);
        assert_eq!(
            resolution.completeness,
            DraftInferenceCompleteness::CandidateLimit {
                required: 2,
                limit: 1,
            }
        );
        assert_eq!(resolution.adjusted_model_position, raw);
        assert!(resolution.candidates.is_empty());
        assert!(resolution.guides.is_empty());
        assert!(engine.active_point_tracking.is_empty());
        assert!(engine.candidate_ids.is_empty());
    }

    #[test]
    fn one_orthogonal_point_axis_pair_fits_one_candidate_limit() {
        let view = viewport(50.0);
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_candidates = 1;
        let mut engine = DraftInferenceEngine::new(policy).expect("one-candidate engine");
        engine
            .remember_reference(point_anchor(634, [-4.0, 4.0]))
            .expect("horizontal reference");
        engine
            .remember_reference(point_anchor(635, [3.0, -4.0]))
            .expect("vertical reference");

        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen([3.04, 4.05]), None, Vec::new()),
                DraftInferenceInput::default(),
            )
            .expect("bounded single orthogonal point-axis pair");
        assert_eq!(
            resolution.completeness,
            DraftInferenceCompleteness::Complete
        );
        assert_eq!(resolution.candidates.len(), 1);
        let candidate = resolved_candidate(&resolution);
        assert_eq!(candidate.adjusted_model_position, [3.0, 4.0]);
        assert_eq!(
            candidate.relations,
            vec![
                DraftInferenceRelation::HorizontalPoints {
                    reference: point_id(634),
                },
                DraftInferenceRelation::VerticalPoints {
                    reference: point_id(635),
                },
            ]
        );
        assert_eq!(candidate.guides.len(), 2);
        assert!(candidate.guides.iter().all(|guide| {
            guide.classification == DraftGuideClassification::ConstraintBacked
                && matches!(
                    guide.family,
                    DraftInferenceFamily::HorizontalPoints | DraftInferenceFamily::VerticalPoints
                )
        }));
    }

    #[test]
    fn orthogonal_point_axis_pair_retains_both_latches_through_exit_band() {
        let view = viewport(50.0);
        let horizontal_origin = [-1.0, 4.0];
        let vertical_origin = [3.0, 0.0];
        let symmetric_target = |angle_degrees: f64| {
            let tangent = angle_degrees.to_radians().tan();
            let offset = 4.0 * tangent / (1.0 - tangent);
            [3.0 + offset, 4.0 + offset]
        };
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(point_anchor(629, horizontal_origin))
            .expect("horizontal reference");
        engine
            .remember_reference(point_anchor(630, vertical_origin))
            .expect("vertical reference");

        let entered = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(symmetric_target(2.9)),
                    None,
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("pair enter band");
        let entered_id = resolved_candidate(&entered).id;
        assert_eq!(engine.active_point_tracking.len(), 2);

        let retained = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(symmetric_target(4.9)),
                    None,
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("pair exit-only band");
        assert!(matches!(
            retained.status,
            DraftInferenceStatus::Resolved { candidate } if candidate == entered_id
        ));
        assert_eq!(retained.adjusted_model_position, [3.0, 4.0]);
        assert_eq!(engine.active_point_tracking.len(), 2);

        let outside = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(symmetric_target(5.1)),
                    None,
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("outside pair exit band");
        assert_eq!(outside.status, DraftInferenceStatus::None);
        assert!(engine.active_point_tracking.is_empty());

        let mut fresh = DraftInferenceEngine::default();
        fresh
            .remember_reference(point_anchor(629, horizontal_origin))
            .expect("fresh horizontal reference");
        fresh
            .remember_reference(point_anchor(630, vertical_origin))
            .expect("fresh vertical reference");
        assert_eq!(
            fresh
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen(symmetric_target(4.9)),
                        None,
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("fresh exit-only sample")
                .status,
            DraftInferenceStatus::None
        );
    }

    #[test]
    fn one_semantic_anchor_never_composes_its_two_point_axes() {
        let view = viewport(50.0);
        let mut policy = DraftInferencePolicy::default();
        policy.tolerances.direction_enter_radians = std::f64::consts::FRAC_PI_2;
        policy.tolerances.direction_exit_radians = std::f64::consts::FRAC_PI_2;
        let mut engine = DraftInferenceEngine::new(policy).expect("wide-angle policy");
        engine
            .remember_reference(point_anchor(631, [0.0, 0.0]))
            .expect("reference");

        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen([3.0, 3.0]), None, Vec::new()),
                DraftInferenceInput::default(),
            )
            .expect("same-anchor alternatives");
        assert!(matches!(
            resolution.status,
            DraftInferenceStatus::Ambiguous { ref candidates } if candidates.len() == 2
        ));
        assert!(
            resolution
                .candidates
                .iter()
                .all(|candidate| candidate.relations.len() == 1)
        );
        assert!(resolution.candidates.iter().any(|candidate| {
            candidate.relations
                == vec![DraftInferenceRelation::HorizontalPoints {
                    reference: point_id(631),
                }]
        }));
        assert!(resolution.candidates.iter().any(|candidate| {
            candidate.relations
                == vec![DraftInferenceRelation::VerticalPoints {
                    reference: point_id(631),
                }]
        }));
    }

    #[test]
    fn same_axis_span_direction_wins_before_cross_axis_pairing() {
        let view = viewport(50.0);
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(point_anchor(632, [-4.0, 4.0]))
            .expect("horizontal reference");
        engine
            .remember_reference(point_anchor(633, [0.0, -4.0]))
            .expect("vertical reference");

        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([0.0, 4.0]),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("coexisting point and span evidence");
        assert_eq!(resolution.candidates.len(), 1);
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![
                DraftInferenceRelation::HorizontalPoints {
                    reference: point_id(632),
                },
                DraftInferenceRelation::Vertical,
            ]
        );
        assert!(resolution.candidates.iter().all(|candidate| {
            !candidate
                .relations
                .contains(&DraftInferenceRelation::VerticalPoints {
                    reference: point_id(633),
                })
        }));
    }

    #[test]
    fn same_axis_span_precedence_happens_before_candidate_limit_accounting() {
        let view = viewport(50.0);
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_candidates = 1;
        let mut engine = DraftInferenceEngine::new(policy).expect("one-candidate engine");
        engine
            .remember_reference(point_anchor(636, [-4.0, 0.0]))
            .expect("same-axis reference");

        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([4.0, 0.05]),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("same-axis precedence within one candidate");

        assert_eq!(
            resolution.completeness,
            DraftInferenceCompleteness::Complete
        );
        assert_eq!(resolution.candidates.len(), 1);
        let candidate = resolved_candidate(&resolution);
        assert_eq!(
            candidate.relations,
            vec![DraftInferenceRelation::Horizontal]
        );
        assert!(candidate.references.is_empty());
        assert!(resolution.guides.iter().all(|guide| {
            guide.family == DraftInferenceFamily::Horizontal
                && guide.classification == DraftGuideClassification::ConstraintBacked
        }));
        assert!(engine.active_point_tracking.is_empty());
    }

    #[test]
    fn same_axis_span_does_not_leave_a_hidden_durable_tracker_latch() {
        let view = viewport(50.0);
        let reference = point_anchor(637, [-4.0, 0.0]);
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(reference)
            .expect("same-axis reference");

        let entered = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([4.0, 0.05]),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("world-axis precedence");
        assert_eq!(
            resolved_candidate(&entered).relations,
            vec![DraftInferenceRelation::Horizontal]
        );
        assert!(entered.guides.iter().all(|guide| {
            guide.family != DraftInferenceFamily::HorizontalPoints
                && guide.reference != Some(reference)
        }));
        assert!(engine.active_point_tracking.is_empty());

        // The world-axis latch has now left its five-degree exit band. From
        // this remembered point the same sample is just outside the tighter
        // three-degree tracking enter band, so a suppressed tracker must not
        // reappear through an unpublished latch.
        let outside_world_axis = 6.0_f64.to_radians();
        let raw = [4.0, 4.0 * outside_world_axis.tan()];
        let released = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(raw),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("released world axis without hidden point latch");
        assert_eq!(released.status, DraftInferenceStatus::None);
        assert!(released.guides.iter().all(|guide| {
            guide.family != DraftInferenceFamily::HorizontalPoints
                && guide.reference != Some(reference)
        }));
        assert!(engine.active_point_tracking.is_empty());
    }

    #[test]
    fn same_axis_span_preserves_generic_tracking_only_cues() {
        let view = viewport(50.0);
        let policy = DraftInferencePolicy {
            point_tracking: DraftInferenceBehavior::tracking_only(),
            ..DraftInferencePolicy::default()
        };
        let reference = point_anchor(638, [-4.0, 0.0]);
        let mut engine = DraftInferenceEngine::new(policy).expect("tracking-only point policy");
        engine
            .remember_reference(reference)
            .expect("same-axis reference");

        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([4.0, 0.05]),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("world axis with generic tracking cue");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![DraftInferenceRelation::Horizontal]
        );
        assert!(resolution.guides.iter().any(|guide| {
            guide.family == DraftInferenceFamily::PointTracking
                && guide.classification == DraftGuideClassification::TrackingOnly
                && guide.reference == Some(reference)
        }));
        assert_eq!(engine.active_point_tracking.len(), 1);
    }

    #[test]
    fn finite_non_cartesian_direction_cannot_become_an_axis_bundle_after_normalization() {
        let view = viewport(50.0);
        let point = point_anchor(610, [4.0, -4.0]);
        let support = affine_anchor(611, [0.0, 1.0], [f64::MAX, f64::MIN_POSITIVE]);
        let policy = DraftInferencePolicy {
            horizontal: DraftInferenceBehavior {
                show_guides: false,
                adjust_coordinates: false,
                persist_constraint: false,
            },
            ..DraftInferencePolicy::default()
        };
        let mut engine = DraftInferenceEngine::new(policy).expect("engine");
        engine.remember_reference(point).expect("point reference");
        engine
            .remember_reference(support)
            .expect("non-Cartesian support");
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([4.05, 0.04]),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("non-Cartesian direction resolution");
        assert!(resolution.candidates.iter().all(|candidate| {
            !(candidate
                .relations
                .contains(&DraftInferenceRelation::VerticalPoints {
                    reference: point_id(610),
                })
                && candidate
                    .relations
                    .contains(&DraftInferenceRelation::Parallel {
                        reference: curve_span(611),
                    }))
        }));
        assert!(matches!(
            resolved_candidate(&resolution).relations.as_slice(),
            [DraftInferenceRelation::Parallel { reference }]
                if *reference == curve_span(611)
        ));
    }

    #[test]
    fn same_axis_span_point_and_midpoint_guides_yield_to_live_world_direction() {
        let view = viewport(50.0);
        for (reference, raw, expected_relation, expected_family) in [
            (
                point_anchor(612, [-4.0, 0.0]),
                [4.0, 0.05],
                DraftInferenceRelation::Horizontal,
                DraftInferenceFamily::Horizontal,
            ),
            (
                midpoint_anchor(613, [-4.0, 0.0], [1.0, 1.0]),
                [4.0, 0.05],
                DraftInferenceRelation::Horizontal,
                DraftInferenceFamily::Horizontal,
            ),
            (
                point_anchor(614, [0.0, -4.0]),
                [0.05, 4.0],
                DraftInferenceRelation::Vertical,
                DraftInferenceFamily::Vertical,
            ),
            (
                midpoint_anchor(615, [0.0, -4.0], [1.0, 1.0]),
                [0.05, 4.0],
                DraftInferenceRelation::Vertical,
                DraftInferenceFamily::Vertical,
            ),
        ] {
            let mut engine = DraftInferenceEngine::default();
            engine.remember_reference(reference).expect("reference");
            let resolution = engine
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen(raw),
                        Some([0.0, 0.0]),
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("same-axis span precedence");
            assert_eq!(resolution.candidates.len(), 1);
            let candidate = resolved_candidate(&resolution);
            assert_eq!(candidate.relations, vec![expected_relation]);
            assert_eq!(candidate.references, Vec::new());
            assert_eq!(candidate.guides.len(), 1);
            assert_eq!(resolution.guides.len(), 1);
            assert_eq!(resolution.guides, candidate.guides);
            assert!(resolution.guides.iter().all(|guide| {
                guide.family == expected_family
                    && guide.classification == DraftGuideClassification::ConstraintBacked
                    && guide.reference.is_none()
            }));
            assert!(engine.active_point_tracking.is_empty());
        }
    }

    #[test]
    fn remembered_cartesian_direction_keys_do_not_suppress_same_axis_tracking() {
        let view = viewport(50.0);
        let policy = DraftInferencePolicy {
            horizontal: DraftInferenceBehavior {
                show_guides: false,
                adjust_coordinates: false,
                persist_constraint: false,
            },
            ..DraftInferencePolicy::default()
        };
        for (point_value, support, expected_direction) in [
            (
                616,
                affine_anchor(617, [0.0, 2.0], [1.0, 0.0]),
                DraftInferenceRelation::Parallel {
                    reference: curve_span(617),
                },
            ),
            (
                618,
                affine_anchor(619, [2.0, 0.0], [0.0, 1.0]),
                DraftInferenceRelation::Perpendicular {
                    reference: curve_span(619),
                },
            ),
            (
                620,
                affine_anchor(621, [0.0, 0.0], [1.0, 0.0]),
                DraftInferenceRelation::Collinear {
                    reference: curve_span(621),
                },
            ),
        ] {
            let mut engine = DraftInferenceEngine::new(policy).expect("policy");
            engine
                .remember_reference(point_anchor(point_value, [-4.0, 0.0]))
                .expect("point reference");
            engine
                .remember_reference(support)
                .expect("support reference");
            let resolution = engine
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen([4.0, 0.04]),
                        Some([0.0, 0.0]),
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("remembered direction resolution");
            assert!(resolution.candidates.iter().any(|candidate| {
                candidate.relations
                    == vec![DraftInferenceRelation::HorizontalPoints {
                        reference: point_id(point_value),
                    }]
            }));
            assert!(
                resolution
                    .candidates
                    .iter()
                    .any(|candidate| candidate.relations == vec![expected_direction])
            );
        }
    }

    #[test]
    fn distinct_axis_bundle_operands_remain_ambiguous_and_candidate_ids_do_not_alias_singletons() {
        let view = viewport(50.0);
        let first = point_anchor(614, [-4.0, 4.0]);
        let second = point_anchor(615, [-5.0, 4.0]);
        let raw = [0.0, 4.0];
        let mut ambiguous_engine = DraftInferenceEngine::default();
        ambiguous_engine
            .remember_reference(first)
            .expect("first reference");
        ambiguous_engine
            .remember_reference(second)
            .expect("second reference");
        let ambiguous = ambiguous_engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(raw),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("ambiguous composed axes");
        assert!(matches!(
            ambiguous.status,
            DraftInferenceStatus::Ambiguous { ref candidates } if candidates.len() == 2
        ));
        assert_eq!(ambiguous.candidates.len(), 2);
        assert!(
            ambiguous
                .candidates
                .iter()
                .all(|candidate| candidate.relations.len() == 2)
        );

        let mut identity_engine = DraftInferenceEngine::default();
        identity_engine
            .remember_reference(first)
            .expect("identity reference");
        let standalone = identity_engine
            .resolve(
                &frame(view, view.model_to_screen(raw), None, Vec::new()),
                DraftInferenceInput::default(),
            )
            .expect("standalone point axis");
        let standalone_id = resolved_candidate(&standalone).id;
        let stale = identity_engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(raw),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput {
                    suppressed: false,
                    preferred_candidate: Some(standalone_id),
                },
            )
            .expect("singleton preference must not alias composed semantics");
        assert_eq!(
            stale.status,
            DraftInferenceStatus::StalePreferredCandidate {
                preferred: standalone_id,
            }
        );
        assert!(identity_engine.remembered_references().is_empty());
    }

    #[test]
    fn one_axis_bundle_fits_one_candidate_limit() {
        let view = viewport(50.0);
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_candidates = 1;
        let mut engine = DraftInferenceEngine::new(policy).expect("one-candidate engine");
        engine
            .remember_reference(point_anchor(616, [-4.0, 4.0]))
            .expect("reference");
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([0.04, 4.05]),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("one bounded composed candidate");
        assert!(matches!(
            resolved_candidate(&resolution).relations.as_slice(),
            [
                DraftInferenceRelation::HorizontalPoints { reference },
                DraftInferenceRelation::Vertical,
            ] if *reference == point_id(616)
        ));
        assert_eq!(resolution.candidates.len(), 1);
    }

    #[test]
    fn axis_bundle_ranking_publishes_the_worse_component_angular_error() {
        let view = viewport(50.0);
        let origin = [-4.0, 4.0];
        let start = [0.0, 0.0];
        let raw = [0.04, 4.05];
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(point_anchor(617, origin))
            .expect("reference");
        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen(raw), Some(start), Vec::new()),
                DraftInferenceInput::default(),
            )
            .expect("ranked axis bundle");
        let candidate = resolved_candidate(&resolution);
        let sampled = candidate.raw_model_position;
        let tracking_error =
            undirected_angle_error([sampled[0] - origin[0], sampled[1] - origin[1]], [1.0, 0.0]);
        let direction_error =
            undirected_angle_error([sampled[0] - start[0], sampled[1] - start[1]], [0.0, 1.0]);
        assert_eq!(
            candidate.ranking.angular_error_radians.to_bits(),
            tracking_error.max(direction_error).to_bits()
        );
    }

    #[test]
    fn axis_bundle_retains_both_components_through_the_shared_exit_band() {
        let view = viewport(50.0);
        let origin = [-4.0, 4.0];
        let start = [0.0, 0.0];
        let symmetric_target = |angle_degrees: f64| {
            let tangent = angle_degrees.to_radians().tan();
            let offset = 4.0 * tangent / (1.0 - tangent);
            [offset, 4.0 + offset]
        };
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(point_anchor(620, origin))
            .expect("reference");

        let entered = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(symmetric_target(3.0)),
                    Some(start),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("bundle enter band");
        let entered_candidate = resolved_candidate(&entered);
        let entered_id = entered_candidate.id;
        assert_eq!(
            entered_candidate.relations,
            vec![
                DraftInferenceRelation::HorizontalPoints {
                    reference: point_id(620),
                },
                DraftInferenceRelation::Vertical,
            ]
        );

        let retained = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(symmetric_target(5.0)),
                    Some(start),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("bundle exit-only band");
        assert!(matches!(
            retained.status,
            DraftInferenceStatus::Resolved { candidate } if candidate == entered_id
        ));
        assert_eq!(
            resolved_candidate(&retained)
                .adjusted_model_position
                .map(f64::to_bits),
            [0.0, 4.0].map(f64::to_bits)
        );

        let outside = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(symmetric_target(6.1)),
                    Some(start),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("outside both exit bands");
        assert_eq!(outside.status, DraftInferenceStatus::None);
        assert!(engine.active_point_tracking.is_empty());
        assert!(engine.active_direction.is_none());
    }

    #[test]
    fn second_axis_bundle_proves_candidate_overflow_without_publishing_a_prefix() {
        let view = viewport(50.0);
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_candidates = 1;
        let mut engine = DraftInferenceEngine::new(policy).expect("one-candidate engine");
        engine
            .remember_reference(point_anchor(618, [-4.0, 4.0]))
            .expect("first reference");
        engine
            .remember_reference(point_anchor(619, [-5.0, 4.0]))
            .expect("second reference");
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen([0.04, 4.05]),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("bounded bundle overflow");
        assert_eq!(resolution.status, DraftInferenceStatus::ResourceLimited);
        assert_eq!(
            resolution.completeness,
            DraftInferenceCompleteness::CandidateLimit {
                required: 2,
                limit: 1,
            }
        );
        assert_eq!(resolution.adjusted_model_position, [0.04, 4.05]);
        assert!(resolution.candidates.is_empty());
        assert!(resolution.guides.is_empty());
        assert!(engine.active_point_tracking.is_empty());
        assert!(engine.active_direction.is_none());
        assert!(engine.candidate_ids.is_empty());
    }

    #[test]
    fn midpoint_axis_uses_tracking_hysteresis_and_suppression_clears_wake_state() {
        let view = viewport(50.0);
        let midpoint = midpoint_anchor(602, [0.0, 0.0], [1.0, 0.0]);
        let enter_angle = 4.0_f64.to_radians();
        let exit_angle = 6.0_f64.to_radians();
        let enter_target = [3.0 * enter_angle.cos(), 3.0 * enter_angle.sin()];
        let exit_target = [3.0 * exit_angle.cos(), 3.0 * exit_angle.sin()];
        let enter_sample = view.screen_to_model(view.model_to_screen(enter_target));
        let exit_sample = view.screen_to_model(view.model_to_screen(exit_target));
        let mut policy = DraftInferencePolicy::default();
        policy.tolerances.direction_enter_radians =
            undirected_angle_error(enter_sample, [1.0, 0.0]);
        policy.tolerances.direction_exit_radians = undirected_angle_error(exit_sample, [1.0, 0.0]);
        let resolve = |engine: &mut DraftInferenceEngine, target: [f64; 2]| {
            engine
                .resolve(
                    &frame(view, view.model_to_screen(target), None, Vec::new()),
                    DraftInferenceInput::default(),
                )
                .expect("midpoint-axis resolution")
        };
        let is_horizontal_midpoint = |resolution: &DraftInferenceResolution| {
            matches!(
                &resolution.status,
                DraftInferenceStatus::Resolved { candidate }
                    if resolution.candidates.iter().any(|value| {
                        value.id == *candidate
                            && matches!(
                                value.relations.as_slice(),
                                [DraftInferenceRelation::HorizontalPointToMidpoint { reference }]
                                    if *reference == curve_span(602)
                            )
                    })
            )
        };

        let mut engine = DraftInferenceEngine::new(policy).expect("boundary policy");
        engine
            .remember_reference(midpoint)
            .expect("remember midpoint");
        assert!(is_horizontal_midpoint(&resolve(&mut engine, enter_sample)));
        assert!(is_horizontal_midpoint(&resolve(&mut engine, exit_sample)));
        let outside_angle = exit_angle + 1.0e-6;
        assert!(!is_horizontal_midpoint(&resolve(
            &mut engine,
            [3.0 * outside_angle.cos(), 3.0 * outside_angle.sin(),],
        )));

        let mut fresh = DraftInferenceEngine::new(policy).expect("fresh boundary policy");
        fresh
            .remember_reference(midpoint)
            .expect("remember midpoint");
        assert!(!is_horizontal_midpoint(&resolve(&mut fresh, exit_sample)));
        fresh
            .remember_reference(midpoint)
            .expect("remember midpoint");
        let suppressed = fresh
            .resolve(
                &frame(view, view.model_to_screen(enter_sample), None, Vec::new()),
                DraftInferenceInput {
                    suppressed: true,
                    preferred_candidate: None,
                },
            )
            .expect("suppression");
        assert_eq!(suppressed.status, DraftInferenceStatus::Suppressed);
        assert!(suppressed.candidates.is_empty());
        assert!(suppressed.guides.is_empty());
        assert!(fresh.remembered_references().is_empty());
    }

    #[test]
    fn distinct_remembered_midpoints_tie_ambiguously_and_stale_preference_fails_closed() {
        let view = viewport(50.0);
        let first = midpoint_anchor(603, [0.0, 0.0], [1.0, 0.0]);
        let second = midpoint_anchor(604, [0.0, 0.0], [1.0, 0.0]);
        let target = [3.0, 0.05];
        let input_frame = frame(view, view.model_to_screen(target), None, Vec::new());
        let mut engine = DraftInferenceEngine::default();
        engine.remember_reference(first).expect("first midpoint");
        engine.remember_reference(second).expect("second midpoint");
        let ambiguous = engine
            .resolve(&input_frame, DraftInferenceInput::default())
            .expect("ambiguous midpoint axes");
        let DraftInferenceStatus::Ambiguous { candidates } = &ambiguous.status else {
            panic!("distinct midpoint operands must not be selected by identity")
        };
        assert_eq!(candidates.len(), 2);
        assert_eq!(ambiguous.adjusted_model_position, target);
        assert_eq!(ambiguous.candidates.len(), 2);
        assert!(ambiguous.candidates.iter().all(|candidate| {
            matches!(
                candidate.relations.as_slice(),
                [DraftInferenceRelation::HorizontalPointToMidpoint { reference }]
                    if [curve_span(603), curve_span(604)].contains(reference)
            )
        }));

        let preferred = candidates[0];
        let resolved = engine
            .resolve(
                &input_frame,
                DraftInferenceInput {
                    suppressed: false,
                    preferred_candidate: Some(preferred),
                },
            )
            .expect("explicit midpoint preference");
        assert!(matches!(
            resolved.status,
            DraftInferenceStatus::Resolved { candidate } if candidate == preferred
        ));

        let stale = engine
            .resolve(
                &input_frame,
                DraftInferenceInput {
                    suppressed: false,
                    preferred_candidate: Some(DraftInferenceCandidateId(u64::MAX)),
                },
            )
            .expect("stale midpoint preference");
        assert!(matches!(
            stale.status,
            DraftInferenceStatus::StalePreferredCandidate { .. }
        ));
        assert_eq!(stale.adjusted_model_position, target);
        assert!(engine.remembered_references().is_empty());
        assert!(engine.active_point_tracking.is_empty());
    }

    #[test]
    fn implicit_fillet_midpoint_alignment_cannot_create_a_retained_axis_relation() {
        let view = viewport(50.0);
        let raw = [4.0, 1.05];
        let mut midpoint = midpoint_anchor(601, [0.0, 1.0], [1.0, 0.0]);
        let DraftReferenceAnchor::Midpoint { origin, .. } = &mut midpoint else {
            unreachable!()
        };
        *origin = DraftReferenceOrigin::FilletDiscarded;

        let mut engine = DraftInferenceEngine::default();
        engine.remember_reference(midpoint).expect("remember");
        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen(raw), None, Vec::new()),
                DraftInferenceInput::default(),
            )
            .expect("resolve");

        assert_eq!(resolution.status, DraftInferenceStatus::None);
        assert_eq!(resolution.adjusted_model_position, raw);
        assert!(resolution.candidates.is_empty());
        assert!(resolution.guides.iter().any(|guide| {
            guide.classification == DraftGuideClassification::TrackingOnly
                && guide.family == DraftInferenceFamily::PointTracking
                && guide.reference == Some(midpoint)
        }));
    }

    #[test]
    fn guide_only_anchor_cannot_manufacture_a_hard_direction_candidate() {
        let policy = DraftInferencePolicy {
            point_identity: DraftInferenceBehavior::tracking_only(),
            ..DraftInferencePolicy::default()
        };
        let mut engine = DraftInferenceEngine::new(policy).expect("engine");
        let view = viewport(50.0);
        let raw = [1.0, 0.15];
        let guide_only_anchor = point_anchor(54, [1.0, 0.05]);
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(raw),
                    Some([0.0, 0.0]),
                    vec![guide_only_anchor],
                ),
                DraftInferenceInput::default(),
            )
            .expect("guide-only resolution");

        assert_eq!(resolution.status, DraftInferenceStatus::None);
        assert_eq!(resolution.adjusted_model_position, raw);
        assert!(resolution.candidates.is_empty());
        assert!(resolution.guides.iter().any(|guide| {
            guide.family == DraftInferenceFamily::PointIdentity
                && guide.classification == DraftGuideClassification::TrackingOnly
        }));
    }

    #[test]
    fn standalone_guide_ids_are_unique_for_valid_tracking_only_policies() {
        let tracking = DraftInferenceBehavior::tracking_only();
        let policy = DraftInferencePolicy {
            point_identity: tracking,
            point_on_curve: tracking,
            midpoint: tracking,
            horizontal: tracking,
            vertical: tracking,
            parallel: tracking,
            perpendicular: tracking,
            point_tracking: tracking,
            ..DraftInferencePolicy::default()
        };
        let mut engine = DraftInferenceEngine::new(policy).expect("tracking-only policy");
        engine
            .remember_reference(point_anchor(61, [0.0, 0.0]))
            .expect("tracking origin");
        let view = viewport(50.0);
        let target = [2.0, 0.0];
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(target),
                    Some([0.0, 0.0]),
                    vec![point_anchor(62, target)],
                ),
                DraftInferenceInput::default(),
            )
            .expect("guide-only resolution");

        assert_eq!(resolution.status, DraftInferenceStatus::None);
        assert!(resolution.guides.len() >= 3);
        assert!(
            resolution
                .guides
                .iter()
                .all(|guide| guide.id.candidate.is_none())
        );
        let unique_ids = resolution
            .guides
            .iter()
            .map(|guide| guide.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(unique_ids.len(), resolution.guides.len());
    }

    #[test]
    fn anchor_and_direction_form_one_candidate_bundle() {
        let view = viewport(100.0);
        let target = [2.0, 0.01];
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(target),
                    Some([0.0, 0.0]),
                    vec![point_anchor(70, [2.0, 0.0])],
                ),
                DraftInferenceInput::default(),
            )
            .expect("resolve");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![
                DraftInferenceRelation::PointIdentity {
                    point: point_id(70)
                },
                DraftInferenceRelation::Horizontal,
            ]
        );
        assert_eq!(resolution.adjusted_model_position, [2.0, 0.0]);
    }

    #[test]
    fn profile_direction_outranks_construction_inside_a_combined_bundle() {
        let view = viewport(100.0);
        let target = [2.0, 0.0];
        let construction =
            affine_anchor_with_role(74, [0.0, 0.0], [1.0, 0.0], GeometryRole::Construction);
        let profile = affine_anchor_with_role(75, [0.0, 0.0], [1.0, 0.0], GeometryRole::Profile);
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(construction)
            .expect("remember construction direction");
        engine
            .remember_reference(profile)
            .expect("remember profile direction");

        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(target),
                    Some([0.0, 0.0]),
                    vec![point_anchor(73, target)],
                ),
                DraftInferenceInput::default(),
            )
            .expect("role-aware combined bundle");
        let selected = resolved_candidate(&resolution);
        assert_eq!(
            selected.relations,
            vec![
                DraftInferenceRelation::PointIdentity {
                    point: point_id(73),
                },
                DraftInferenceRelation::Collinear {
                    reference: curve_span(75),
                },
            ]
        );
        assert_eq!(selected.ranking.positional_geometry_role_priority, 0);
        assert_eq!(selected.ranking.directional_geometry_role_priority, 0);

        let construction_bundle = resolution
            .candidates
            .iter()
            .find(|candidate| {
                candidate.relations.iter().any(|relation| {
                    matches!(
                        relation,
                        DraftInferenceRelation::Collinear { reference }
                            if *reference == curve_span(74)
                    )
                })
            })
            .expect("construction-reference bundle remains inspectable");
        assert_eq!(
            construction_bundle
                .ranking
                .positional_geometry_role_priority,
            0
        );
        assert_eq!(
            construction_bundle
                .ranking
                .directional_geometry_role_priority,
            1
        );
    }

    #[test]
    fn near_direction_does_not_bundle_with_an_incompatible_exact_anchor() {
        let view = viewport(50.0);
        let target = [2.0, 0.1];
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(target),
                    Some([0.0, 0.0]),
                    vec![point_anchor(73, target)],
                ),
                DraftInferenceInput::default(),
            )
            .expect("resolve");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![DraftInferenceRelation::PointIdentity {
                point: point_id(73)
            }]
        );
        assert_eq!(resolution.adjusted_model_position, target);
        assert!(resolution.candidates.iter().all(|candidate| {
            !(candidate.relations.iter().any(|relation| {
                matches!(relation, DraftInferenceRelation::PointIdentity { point } if *point == point_id(73))
            }) && candidate
                .relations
                .iter()
                .any(|relation| matches!(relation, DraftInferenceRelation::Horizontal)))
        }));
    }

    #[test]
    fn incompatible_anchor_direction_bundle_is_translation_invariant() {
        for translation in [[0.0, 0.0], [1.0e12, -1.0e12]] {
            let view =
                Viewport::new([1_000.0, 700.0], translation, 50.0).expect("translated viewport");
            let start = translation;
            let target = [translation[0] + 2.0, translation[1] + 0.01];
            let mut engine = DraftInferenceEngine::default();
            let resolution = engine
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen(target),
                        Some(start),
                        vec![point_anchor(731, target)],
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("translated resolution");

            assert_eq!(
                resolved_candidate(&resolution).relations,
                vec![DraftInferenceRelation::PointIdentity {
                    point: point_id(731)
                }]
            );
            assert!(resolution.candidates.iter().all(|candidate| {
                !(candidate.relations.iter().any(|relation| {
                    matches!(
                        relation,
                        DraftInferenceRelation::PointIdentity { point }
                            if *point == point_id(731)
                    )
                }) && candidate
                    .relations
                    .iter()
                    .any(|relation| matches!(relation, DraftInferenceRelation::Horizontal)))
            }));

            let compatible_target = [translation[0] + 2.0, translation[1]];
            let compatible = DraftInferenceEngine::default()
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen(compatible_target),
                        Some(start),
                        vec![point_anchor(733, compatible_target)],
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("translated compatible resolution");
            assert_eq!(
                resolved_candidate(&compatible).relations,
                vec![
                    DraftInferenceRelation::PointIdentity {
                        point: point_id(733)
                    },
                    DraftInferenceRelation::Horizontal,
                ]
            );
        }
    }

    #[test]
    fn directional_inference_is_invariant_to_uniform_coordinate_scaling() {
        for model_scale in [1.0e-15, 1.0, 1.0e15] {
            let view = viewport(50.0 / model_scale);
            let start = [0.0, 0.0];
            let horizontal_target = [2.0 * model_scale, 0.0];
            let horizontal = DraftInferenceEngine::default()
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen(horizontal_target),
                        Some(start),
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("scaled horizontal resolution");
            assert_eq!(
                resolved_candidate(&horizontal).relations,
                vec![DraftInferenceRelation::Horizontal]
            );

            let direction = [2.0 * model_scale, model_scale];
            let reference = affine_anchor(732, start, direction);
            let mut parallel_engine = DraftInferenceEngine::default();
            parallel_engine
                .remember_reference(reference)
                .expect("scaled affine reference");
            let parallel = parallel_engine
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen(direction),
                        Some(start),
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("scaled parallel resolution");
            assert_eq!(
                resolved_candidate(&parallel).relations,
                vec![DraftInferenceRelation::Collinear {
                    reference: curve_span(732),
                }]
            );

            let offset_start = [0.0, 2.0 * model_scale];
            let offset_target = [
                offset_start[0] + direction[0],
                offset_start[1] + direction[1],
            ];
            let mut offset_parallel_engine = DraftInferenceEngine::default();
            offset_parallel_engine
                .remember_reference(reference)
                .expect("scaled affine reference");
            let offset_parallel = offset_parallel_engine
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen(offset_target),
                        Some(offset_start),
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("scaled offset parallel resolution");
            assert_eq!(
                resolved_candidate(&offset_parallel).relations,
                vec![DraftInferenceRelation::Parallel {
                    reference: curve_span(732),
                }]
            );

            let perpendicular_target = [-model_scale, 2.0 * model_scale];
            let mut perpendicular_engine = DraftInferenceEngine::default();
            perpendicular_engine
                .remember_reference(reference)
                .expect("scaled affine reference");
            let perpendicular = perpendicular_engine
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen(perpendicular_target),
                        Some(start),
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect("scaled perpendicular resolution");
            assert_eq!(
                resolved_candidate(&perpendicular).relations,
                vec![DraftInferenceRelation::Perpendicular {
                    reference: curve_span(732),
                }]
            );
        }
    }

    #[test]
    fn extra_relation_does_not_displace_higher_priority_anchor() {
        let view = viewport(20.0);
        let raw = [2.0, 0.0];
        let point = point_anchor(71, [2.0, 0.2]);
        let curve = affine_anchor(72, raw, [1.0, 0.0]);
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(raw),
                    Some([0.0, 0.0]),
                    vec![curve, point],
                ),
                DraftInferenceInput::default(),
            )
            .expect("semantic ranking");
        assert_eq!(
            resolved_candidate(&resolution).relations,
            vec![DraftInferenceRelation::PointIdentity {
                point: point_id(71)
            }]
        );
    }

    #[test]
    fn exact_semantic_tie_is_ambiguous_until_preferred() {
        let view = viewport(50.0);
        let target = [1.0, 1.0];
        let anchors = vec![
            affine_anchor(81, target, [1.0, 0.0]),
            affine_anchor(82, target, [1.0, 0.0]),
        ];
        let mut engine = DraftInferenceEngine::default();
        let first = engine
            .resolve(
                &frame(view, view.model_to_screen(target), None, anchors.clone()),
                DraftInferenceInput::default(),
            )
            .expect("ambiguous");
        let DraftInferenceStatus::Ambiguous { candidates } = first.status else {
            panic!("expected ambiguity");
        };
        assert_eq!(candidates.len(), 2);
        let preferred = candidates[1];
        let second = engine
            .resolve(
                &frame(view, view.model_to_screen(target), None, anchors),
                DraftInferenceInput {
                    suppressed: false,
                    preferred_candidate: Some(preferred),
                },
            )
            .expect("preferred");
        assert_eq!(
            second.status,
            DraftInferenceStatus::Resolved {
                candidate: preferred
            }
        );
    }

    #[test]
    fn profile_wins_equivalent_construction_at_one_pixel_overlap_boundary() {
        let view = viewport(100.0);
        let raw = [0.0, 0.0];
        let profile_span = curve_span(83);
        let anchors = vec![
            affine_anchor_with_role(
                83,
                [
                    PROFILE_CONSTRUCTION_OVERLAP_PIXELS / view.pixels_per_model_unit,
                    0.0,
                ],
                [1.0, 0.0],
                GeometryRole::Profile,
            ),
            affine_anchor_with_role(84, raw, [1.0, 0.0], GeometryRole::Construction),
        ];
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen(raw), None, anchors),
                DraftInferenceInput::default(),
            )
            .expect("overlap resolution");
        assert!(matches!(
            resolved_candidate(&resolution).relations.as_slice(),
            [DraftInferenceRelation::PointOnCurve { contact }] if contact.span == profile_span
        ));
        assert_eq!(
            resolved_candidate(&resolution)
                .ranking
                .positional_geometry_role_priority,
            0
        );
        assert_eq!(
            resolved_candidate(&resolution)
                .ranking
                .directional_geometry_role_priority,
            2
        );
        assert_eq!(
            resolved_candidate(&resolution).ranking.distance_pixels,
            PROFILE_CONSTRUCTION_OVERLAP_PIXELS
        );
    }

    #[test]
    fn newly_entered_profile_anchor_overrides_a_latched_construction_occurrence() {
        let view = viewport(100.0);
        let raw = [0.0, 0.0];
        let construction =
            affine_anchor_with_role(184, raw, [1.0, 0.0], GeometryRole::Construction);
        let profile = affine_anchor_with_role(
            185,
            [
                PROFILE_CONSTRUCTION_OVERLAP_PIXELS / view.pixels_per_model_unit,
                0.0,
            ],
            [1.0, 0.0],
            GeometryRole::Profile,
        );
        let mut engine = DraftInferenceEngine::default();
        let first = engine
            .resolve(
                &frame(view, view.model_to_screen(raw), None, vec![construction]),
                DraftInferenceInput::default(),
            )
            .expect("construction latch");
        assert!(matches!(
            resolved_candidate(&first).relations.as_slice(),
            [DraftInferenceRelation::PointOnCurve { contact }]
                if contact.span == curve_span(184)
        ));

        let second = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(raw),
                    None,
                    vec![construction, profile],
                ),
                DraftInferenceInput::default(),
            )
            .expect("profile override");
        assert!(matches!(
            resolved_candidate(&second).relations.as_slice(),
            [DraftInferenceRelation::PointOnCurve { contact }]
                if contact.span == curve_span(185)
        ));
    }

    #[test]
    fn circle_circumference_inference_resolves_only_persistent_points() {
        let view = viewport(50.0);
        let target = [2.0, 1.0];
        let screen = view.model_to_screen([2.08, 1.04]);
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame_for_subject(
                    view,
                    screen,
                    DraftInferenceSubject::CircleCircumference,
                    None,
                    vec![
                        affine_anchor(20, target, [1.0, 0.0]),
                        midpoint_anchor(20, target, [1.0, 0.0]),
                        point_anchor(10, target),
                    ],
                ),
                DraftInferenceInput::default(),
            )
            .expect("circle circumference inference");
        let selected = resolved_candidate(&resolution);
        assert_eq!(selected.adjusted_model_position, target);
        assert_eq!(
            selected.adjusted_screen_position,
            view.model_to_screen(target)
        );
        assert_eq!(
            selected.relations,
            vec![DraftInferenceRelation::PointOnCreatedCurve {
                point: point_id(10)
            }]
        );
        assert!(matches!(
            selected.references.as_slice(),
            [DraftReferenceAnchor::PersistentPoint { point, .. }] if *point == point_id(10)
        ));
        assert!(selected.guides.iter().all(|guide| {
            guide.family == DraftInferenceFamily::PointOnCreatedCurve
                && guide.classification == DraftGuideClassification::ConstraintBacked
        }));

        let curve_only = engine
            .resolve(
                &frame_for_subject(
                    view,
                    screen,
                    DraftInferenceSubject::CircleCircumference,
                    None,
                    vec![
                        affine_anchor(20, target, [1.0, 0.0]),
                        midpoint_anchor(20, target, [1.0, 0.0]),
                    ],
                ),
                DraftInferenceInput::default(),
            )
            .expect("curve anchors are ineligible for a circle circumference");
        assert_eq!(curve_only.status, DraftInferenceStatus::None);
        assert!(curve_only.candidates.is_empty());
        assert!(curve_only.guides.is_empty());
    }

    #[test]
    fn concentric_inference_deduplicates_curve_occurrences_and_keeps_enter_exit_hysteresis() {
        let view = viewport(50.0);
        let target = [2.0, 1.0];
        let mut input = frame_for_subject(
            view,
            view.model_to_screen([2.11, 1.0]),
            DraftInferenceSubject::CenteredPointOperand {
                prospective_curve_index: 3,
            },
            None,
            Vec::new(),
        );
        input.semantic_centers = vec![
            semantic_center(31, 30, target, GeometryRole::Profile),
            semantic_center(31, 30, target, GeometryRole::Profile),
        ];
        let mut engine = DraftInferenceEngine::default();
        let entered = engine
            .resolve(&input, DraftInferenceInput::default())
            .expect("concentric enter");
        let candidate = resolved_candidate(&entered);
        assert_eq!(candidate.adjusted_model_position, target);
        assert!(matches!(
            candidate.relations.as_slice(),
            [DraftInferenceRelation::Concentric {
                reference,
                prospective_curve_index: 3,
            }] if *reference == curve_span(31).curve
        ));
        assert_eq!(entered.candidates.len(), 1);

        input.sample.raw_screen_position = view.model_to_screen([2.17, 1.0]);
        let retained = engine
            .resolve(&input, DraftInferenceInput::default())
            .expect("concentric exit band");
        assert_eq!(
            resolved_candidate(&retained).adjusted_model_position,
            target
        );

        input.sample.raw_screen_position = view.model_to_screen([2.19, 1.0]);
        let left = engine
            .resolve(&input, DraftInferenceInput::default())
            .expect("concentric leave");
        assert_eq!(left.status, DraftInferenceStatus::None);
    }

    #[test]
    fn distinct_curves_sharing_a_center_remain_ambiguous_retained_operands() {
        let view = viewport(50.0);
        let target = [2.0, 1.0];
        let mut input = frame_for_subject(
            view,
            view.model_to_screen(target),
            DraftInferenceSubject::CenteredPointOperand {
                prospective_curve_index: 3,
            },
            None,
            Vec::new(),
        );
        input.semantic_centers = vec![
            semantic_center(31, 30, target, GeometryRole::Profile),
            semantic_center(32, 30, target, GeometryRole::Profile),
        ];
        let mut engine = DraftInferenceEngine::default();
        let ambiguous = engine
            .resolve(&input, DraftInferenceInput::default())
            .expect("shared-center curve ambiguity");
        let DraftInferenceStatus::Ambiguous { candidates } = &ambiguous.status else {
            panic!("distinct retained operands must remain ambiguous");
        };
        assert_eq!(candidates.len(), 2);
        assert_eq!(ambiguous.candidates.len(), 2);
        assert_eq!(ambiguous.adjusted_model_position, target);

        let selected_id = ambiguous
            .candidates
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.relations.as_slice(),
                    [DraftInferenceRelation::Concentric { reference, .. }]
                        if *reference == curve_span(32).curve
                )
            })
            .expect("second curve remains selectable")
            .id;
        let preferred = engine
            .resolve(
                &input,
                DraftInferenceInput {
                    suppressed: false,
                    preferred_candidate: Some(selected_id),
                },
            )
            .expect("explicit shared-center curve preference");
        assert!(matches!(
            resolved_candidate(&preferred).relations.as_slice(),
            [DraftInferenceRelation::Concentric { reference, .. }]
                if *reference == curve_span(32).curve
        ));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one property test keeps ordinary anchors, centered precedence, explicit choice, and fallback together"
    )]
    fn centered_subject_preserves_m70_point_anchors_beside_concentric_candidates() {
        let view = viewport(50.0);
        let target = [2.0, 1.0];
        let raw_screen = view.model_to_screen([2.05, 1.02]);
        let subject = DraftInferenceSubject::CenteredPointOperand {
            prospective_curve_index: 0,
        };
        let ordinary = [
            point_anchor(10, target),
            midpoint_anchor(20, target, [1.0, 0.0]),
            affine_anchor(30, target, [1.0, 0.0]),
        ];
        let expected = [
            DraftInferenceFamily::PointIdentity,
            DraftInferenceFamily::Midpoint,
            DraftInferenceFamily::PointOnCurve,
        ];
        for (anchor, family) in ordinary.into_iter().zip(expected) {
            let frame = frame_for_subject(view, raw_screen, subject, None, vec![anchor]);
            let resolution = DraftInferenceEngine::default()
                .resolve(&frame, DraftInferenceInput::default())
                .expect("centered ordinary inference");
            let candidate = resolved_candidate(&resolution);
            assert_eq!(candidate.adjusted_model_position, target);
            assert!(candidate.guides.iter().all(|guide| guide.family == family));
        }

        let mut compound = frame_for_subject(
            view,
            raw_screen,
            subject,
            None,
            vec![point_anchor(10, target)],
        );
        compound.semantic_centers = vec![semantic_center(40, 10, target, GeometryRole::Profile)];
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(&compound, DraftInferenceInput::default())
            .expect("compound centered inference");
        assert_eq!(resolution.candidates.len(), 2);
        let point = resolution
            .candidates
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.relations.as_slice(),
                    [DraftInferenceRelation::PointIdentity { point }] if *point == point_id(10)
                )
            })
            .expect("point identity candidate");
        let concentric = resolution
            .candidates
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.relations.as_slice(),
                    [DraftInferenceRelation::Concentric { reference, .. }]
                        if *reference == curve_span(40).curve
                )
            })
            .expect("concentric candidate");
        assert_ne!(point.id, concentric.id);
        assert_eq!(resolved_candidate(&resolution).id, concentric.id);

        let preferred = engine
            .resolve(
                &compound,
                DraftInferenceInput {
                    suppressed: false,
                    preferred_candidate: Some(point.id),
                },
            )
            .expect("preferred point identity");
        assert!(matches!(
            resolved_candidate(&preferred).relations.as_slice(),
            [DraftInferenceRelation::PointIdentity { point }] if *point == point_id(10)
        ));

        let policy = DraftInferencePolicy {
            concentric: DraftInferenceBehavior {
                show_guides: false,
                adjust_coordinates: false,
                persist_constraint: false,
            },
            ..DraftInferencePolicy::default()
        };
        let fallback = DraftInferenceEngine::new(policy)
            .expect("policy")
            .resolve(&compound, DraftInferenceInput::default())
            .expect("ordinary fallback");
        assert!(matches!(
            resolved_candidate(&fallback).relations.as_slice(),
            [DraftInferenceRelation::PointIdentity { point }] if *point == point_id(10)
        ));

        let ordinary = DraftInferenceEngine::default()
            .resolve(
                &frame_for_subject(
                    view,
                    raw_screen,
                    DraftInferenceSubject::PointOperand,
                    None,
                    vec![point_anchor(10, target)],
                ),
                DraftInferenceInput::default(),
            )
            .expect("ordinary point identity");
        assert!(matches!(
            resolved_candidate(&ordinary).relations.as_slice(),
            [DraftInferenceRelation::PointIdentity { point }] if *point == point_id(10)
        ));
    }

    #[test]
    fn irrelevant_subject_inputs_are_ignored_but_centered_inputs_share_one_bound() {
        let view = viewport(50.0);
        let screen = view.model_to_screen([0.0, 0.0]);
        let mut point_frame = frame_for_subject(
            view,
            screen,
            DraftInferenceSubject::PointOperand,
            None,
            vec![point_anchor(1, [0.0, 0.0])],
        );
        point_frame.semantic_centers = vec![semantic_center(
            2,
            3,
            [f64::NAN, 0.0],
            GeometryRole::Profile,
        )];
        assert!(matches!(
            DraftInferenceEngine::default()
                .resolve(&point_frame, DraftInferenceInput::default())
                .expect("irrelevant center ignored")
                .status,
            DraftInferenceStatus::Resolved { .. }
        ));

        let mut circumference = frame_for_subject(
            view,
            screen,
            DraftInferenceSubject::CircleCircumference,
            None,
            vec![
                point_anchor(1, [0.0, 0.0]),
                midpoint_anchor(4, [f64::NAN, 0.0], [1.0, 0.0]),
            ],
        );
        circumference.semantic_centers = point_frame.semantic_centers.clone();
        assert!(matches!(
            DraftInferenceEngine::default()
                .resolve(&circumference, DraftInferenceInput::default())
                .expect("irrelevant curve and center inputs ignored")
                .status,
            DraftInferenceStatus::Resolved { .. }
        ));

        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_scene_anchors = 1;
        let mut centered = frame_for_subject(
            view,
            screen,
            DraftInferenceSubject::CenteredPointOperand {
                prospective_curve_index: 0,
            },
            None,
            vec![point_anchor(1, [0.0, 0.0])],
        );
        centered.semantic_centers = vec![semantic_center(2, 3, [0.0, 0.0], GeometryRole::Profile)];
        let limited = DraftInferenceEngine::new(policy)
            .expect("bounded policy")
            .resolve(&centered, DraftInferenceInput::default())
            .expect("typed mixed bound");
        assert_eq!(
            limited.completeness,
            DraftInferenceCompleteness::SceneLimit(DraftInferenceSceneLimit {
                resource: DraftInferenceSceneResource::Anchors,
                required: 2,
                limit: 1,
            })
        );
        assert!(limited.candidates.is_empty());
        assert!(limited.guides.is_empty());
    }

    #[test]
    fn concentric_exact_tie_is_ambiguous_and_scale_invariant() {
        for scale in [1.0e-6, 1.0, 1.0e6] {
            let view = viewport(50.0 / scale);
            let raw = [0.0, 0.0];
            let mut input = frame_for_subject(
                view,
                view.model_to_screen(raw),
                DraftInferenceSubject::CenteredPointOperand {
                    prospective_curve_index: 0,
                },
                None,
                Vec::new(),
            );
            input.semantic_centers = vec![
                semantic_center(41, 40, [-0.1 * scale, 0.0], GeometryRole::Profile),
                semantic_center(43, 42, [0.1 * scale, 0.0], GeometryRole::Profile),
            ];
            let resolution = DraftInferenceEngine::default()
                .resolve(&input, DraftInferenceInput::default())
                .expect("concentric tie");
            assert!(matches!(
                resolution.status,
                DraftInferenceStatus::Ambiguous { ref candidates } if candidates.len() == 2
            ));
            assert_eq!(resolution.adjusted_model_position, raw);
        }
    }

    #[test]
    fn concentric_candidate_limit_and_suppression_are_fail_closed() {
        let view = viewport(50.0);
        let mut input = frame_for_subject(
            view,
            view.model_to_screen([0.0, 0.0]),
            DraftInferenceSubject::CenteredPointOperand {
                prospective_curve_index: 0,
            },
            None,
            Vec::new(),
        );
        input.semantic_centers = (1..=4_096_u128)
            .map(|value| semantic_center(value, value + 10_000, [0.0, 0.0], GeometryRole::Profile))
            .collect();
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_scene_anchors = 8_192;
        policy.limits.max_candidates = 2;
        let mut engine = DraftInferenceEngine::new(policy).expect("bounded engine");
        let limited = engine
            .resolve(&input, DraftInferenceInput::default())
            .expect("candidate bound");
        assert_eq!(limited.status, DraftInferenceStatus::ResourceLimited);
        assert_eq!(
            limited.completeness,
            DraftInferenceCompleteness::CandidateLimit {
                required: 3,
                limit: 2,
            }
        );
        assert!(limited.candidates.is_empty());

        policy.limits.max_scene_anchors = 1;
        let mut suppressed_engine = DraftInferenceEngine::new(policy).expect("suppression engine");
        let suppressed = suppressed_engine
            .resolve(
                &input,
                DraftInferenceInput {
                    suppressed: true,
                    preferred_candidate: None,
                },
            )
            .expect("traversal-free suppression");
        assert_eq!(suppressed.status, DraftInferenceStatus::Suppressed);
        assert!(suppressed.candidates.is_empty());
    }

    #[test]
    fn semantic_center_candidates_obey_scope_visibility_and_reacquire_after_exhaustion() {
        let view = viewport(50.0);
        let subject = DraftInferenceSubject::CenteredPointOperand {
            prospective_curve_index: 0,
        };
        let mut input = frame_for_subject(
            view,
            view.model_to_screen([0.0, 0.0]),
            subject,
            None,
            Vec::new(),
        );
        input.semantic_centers = vec![
            semantic_center(1, 11, [0.0, 0.0], GeometryRole::Profile),
            semantic_center(2, 12, [0.01, 0.0], GeometryRole::Construction),
        ];

        input.geometry_policy.scope = GeometryPickScope::Profile;
        let profile = DraftInferenceEngine::default()
            .resolve(&input, DraftInferenceInput::default())
            .expect("profile center");
        assert!(matches!(
            resolved_candidate(&profile).relations.as_slice(),
            [DraftInferenceRelation::Concentric { reference, .. }]
                if *reference == curve_span(1).curve
        ));

        input.geometry_policy.scope = GeometryPickScope::Construction;
        let construction = DraftInferenceEngine::default()
            .resolve(&input, DraftInferenceInput::default())
            .expect("construction center");
        assert!(matches!(
            resolved_candidate(&construction).relations.as_slice(),
            [DraftInferenceRelation::Concentric { reference, .. }]
                if *reference == curve_span(2).curve
        ));

        input.geometry_policy.visibility.explicit_construction = false;
        assert_eq!(
            DraftInferenceEngine::default()
                .resolve(&input, DraftInferenceInput::default())
                .expect("hidden construction center")
                .status,
            DraftInferenceStatus::None
        );

        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_candidates = 1;
        policy.limits.max_scene_anchors = 3;
        let mut engine = DraftInferenceEngine::new(policy).expect("bounded engine");
        input.geometry_policy = GeometryInteractionPolicy::default();
        input.geometry_policy.visibility.reference_geometry = false;
        let limited = engine
            .resolve(&input, DraftInferenceInput::default())
            .expect("distinct-center overflow");
        assert_eq!(limited.status, DraftInferenceStatus::ResourceLimited);
        assert!(limited.candidates.is_empty());

        input.semantic_centers.truncate(1);
        let recovered = engine
            .resolve(&input, DraftInferenceInput::default())
            .expect("post-limit reacquisition");
        assert!(matches!(
            recovered.status,
            DraftInferenceStatus::Resolved { .. }
        ));
    }

    #[test]
    fn durable_point_tracking_works_without_guides_and_projects_visible_guide_endpoint() {
        let view = viewport(50.0);
        let reference = point_anchor(52, [0.0, 1.0]);
        let raw = [3.0, 1.05];
        let input = frame(view, view.model_to_screen(raw), None, Vec::new());

        let mut policy = DraftInferencePolicy::default();
        policy.point_tracking.show_guides = false;
        let mut headless = DraftInferenceEngine::new(policy).expect("headless policy");
        headless.remember_reference(reference).expect("reference");
        let resolution = headless
            .resolve(&input, DraftInferenceInput::default())
            .expect("durable no-guide tracking");
        assert_eq!(
            resolved_candidate(&resolution).adjusted_model_position,
            [3.0, 1.0]
        );
        assert!(resolution.guides.is_empty());

        let mut visible = DraftInferenceEngine::default();
        visible.remember_reference(reference).expect("reference");
        let resolution = visible
            .resolve(&input, DraftInferenceInput::default())
            .expect("visible tracking");
        assert!(resolution.guides.iter().any(|guide| {
            matches!(
                guide.geometry,
                DraftGuideGeometry::Segment {
                    start: [0.0, 1.0],
                    end: [3.0, 1.0],
                }
            )
        }));
    }

    #[test]
    fn newly_entered_profile_direction_overrides_a_latched_construction_reference() {
        let view = viewport(50.0);
        let raw = [2.0, 0.01];
        let construction =
            affine_anchor_with_role(186, [0.0, 1.0], [1.0, 0.0], GeometryRole::Construction);
        let profile = affine_anchor_with_role(187, [0.0, -1.0], [1.0, 0.0], GeometryRole::Profile);
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(construction)
            .expect("construction reference");
        let first = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(raw),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("construction direction latch");
        assert!(matches!(
            resolved_candidate(&first).relations.as_slice(),
            [DraftInferenceRelation::Parallel { reference }] if *reference == curve_span(186)
        ));

        engine
            .remember_reference(profile)
            .expect("profile reference");
        let second = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(raw),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("profile direction override");
        assert!(matches!(
            resolved_candidate(&second).relations.as_slice(),
            [DraftInferenceRelation::Parallel { reference }] if *reference == curve_span(187)
        ));
    }

    #[test]
    fn newly_entered_backed_axis_overrides_an_adjustment_only_direction_latch() {
        let policy = DraftInferencePolicy {
            parallel: DraftInferenceBehavior {
                show_guides: true,
                adjust_coordinates: true,
                persist_constraint: false,
            },
            ..DraftInferencePolicy::default()
        };
        let mut engine = DraftInferenceEngine::new(policy).expect("engine");
        let view = viewport(50.0);
        let five_degrees = 5.0_f64.to_radians();
        let reference = affine_anchor(188, [0.0, 1.0], [five_degrees.cos(), five_degrees.sin()]);
        engine
            .remember_reference(reference)
            .expect("adjustment-only reference");
        let first_angle = 5.0_f64.to_radians();
        let first_raw = [2.0 * first_angle.cos(), 2.0 * first_angle.sin()];
        let first = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(first_raw),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("adjustment-only latch");
        assert_eq!(
            resolved_candidate(&first).ranking.direction_priority,
            DraftDirectionPriority::RememberedReference
        );
        assert!(resolved_candidate(&first).relations.is_empty());

        let second_angle = 2.5_f64.to_radians();
        let second_raw = [2.0 * second_angle.cos(), 2.0 * second_angle.sin()];
        let second = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(second_raw),
                    Some([0.0, 0.0]),
                    Vec::new(),
                ),
                DraftInferenceInput::default(),
            )
            .expect("backed axis override");
        assert_eq!(
            resolved_candidate(&second).relations,
            vec![DraftInferenceRelation::Horizontal]
        );
    }

    #[test]
    fn nearer_construction_wins_beyond_one_pixel_overlap_band() {
        let view = viewport(100.0);
        let raw = [0.0, 0.0];
        let construction_span = curve_span(86);
        let anchors = vec![
            affine_anchor_with_role(
                85,
                [1.01 / view.pixels_per_model_unit, 0.0],
                [1.0, 0.0],
                GeometryRole::Profile,
            ),
            affine_anchor_with_role(86, raw, [1.0, 0.0], GeometryRole::Construction),
        ];
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen(raw), None, anchors),
                DraftInferenceInput::default(),
            )
            .expect("separated resolution");
        assert!(matches!(
            resolved_candidate(&resolution).relations.as_slice(),
            [DraftInferenceRelation::PointOnCurve { contact }]
                if contact.span == construction_span
        ));
        assert_eq!(
            resolved_candidate(&resolution)
                .ranking
                .positional_geometry_role_priority,
            1
        );
        assert_eq!(resolved_candidate(&resolution).ranking.distance_pixels, 0.0);
    }

    #[test]
    fn duplicate_native_span_uses_profile_occurrence_inside_overlap_band() {
        let view = viewport(100.0);
        let raw = [0.0, 0.0];
        let shared_profile = affine_anchor_occurrence(
            87,
            [0.009, 0.0],
            [1.0, 0.0],
            GeometryRole::Profile,
            GeometryRole::Profile,
            DraftReferenceOrigin::Native,
        );
        let interposed_profile = affine_anchor(88, [0.0095, 0.0], [1.0, 0.0]);
        let shared_discarded = affine_anchor_occurrence(
            87,
            raw,
            [1.0, 0.0],
            GeometryRole::Construction,
            GeometryRole::Profile,
            DraftReferenceOrigin::FilletDiscarded,
        );
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(raw),
                    None,
                    vec![shared_discarded, interposed_profile, shared_profile],
                ),
                DraftInferenceInput::default(),
            )
            .expect("deduplicated occurrence");

        let shared = resolution
            .candidates
            .iter()
            .find(|candidate| {
                candidate.relations.iter().any(|relation| {
                    matches!(
                        relation,
                        DraftInferenceRelation::PointOnCurve { contact }
                            if contact.span == curve_span(87)
                    )
                })
            })
            .expect("shared span candidate");
        assert_eq!(shared.ranking.positional_geometry_role_priority, 0);
        assert_eq!(shared.ranking.directional_geometry_role_priority, 2);
        assert!((shared.ranking.distance_pixels - 0.9).abs() <= 1.0e-12);
        assert_eq!(
            engine
                .remembered_references()
                .iter()
                .filter(|reference| {
                    reference.key()
                        == AnchorKey::PointOnCurve(
                            curve_span(87),
                            DraftCurveBranchCandidate::default(),
                        )
                })
                .count(),
            1
        );
        assert!(engine.remembered_references().iter().any(|reference| {
            matches!(
                reference,
                DraftReferenceAnchor::AffineSupport {
                    contact,
                    role: GeometryRole::Profile,
                    origin: DraftReferenceOrigin::Native,
                    ..
                } if contact.span == curve_span(87)
            )
        }));
    }

    #[test]
    fn suppression_clears_latch_and_applies_nothing() {
        let view = viewport(50.0);
        let target = [0.0, 0.0];
        let anchor = point_anchor(90, target);
        let mut engine = DraftInferenceEngine::default();
        let input_frame = frame(view, view.model_to_screen(target), None, vec![anchor]);
        engine
            .resolve(&input_frame, DraftInferenceInput::default())
            .expect("initial");
        let suppressed = engine
            .resolve(
                &input_frame,
                DraftInferenceInput {
                    suppressed: true,
                    preferred_candidate: None,
                },
            )
            .expect("suppressed");
        assert_eq!(suppressed.status, DraftInferenceStatus::Suppressed);
        assert!(suppressed.candidates.is_empty());
        assert!(suppressed.guides.is_empty());
    }

    #[test]
    fn suppression_clears_wake_memory_before_inference_resumes() {
        let view = viewport(50.0);
        let reference = affine_anchor(91, [0.0, 0.0], [1.0, 0.0]);
        let endpoint = [3.0, 0.1];
        let input_frame = frame(
            view,
            view.model_to_screen(endpoint),
            Some([0.0, 0.0]),
            Vec::new(),
        );
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(reference)
            .expect("wake reference");
        let before = engine
            .resolve(&input_frame, DraftInferenceInput::default())
            .expect("remembered inference");
        assert_eq!(
            resolved_candidate(&before).relations,
            vec![DraftInferenceRelation::Collinear {
                reference: curve_span(91)
            }]
        );

        engine
            .resolve(
                &input_frame,
                DraftInferenceInput {
                    suppressed: true,
                    preferred_candidate: None,
                },
            )
            .expect("suppression");
        assert!(engine.remembered_references().is_empty());

        let after = engine
            .resolve(&input_frame, DraftInferenceInput::default())
            .expect("fresh inference");
        assert_eq!(
            resolved_candidate(&after).relations,
            vec![DraftInferenceRelation::Horizontal]
        );
        assert!(after.candidates.iter().all(|candidate| {
            candidate.relations.iter().all(|relation| {
                !matches!(
                    relation,
                    DraftInferenceRelation::Parallel { .. }
                        | DraftInferenceRelation::Perpendicular { .. }
                        | DraftInferenceRelation::Collinear { .. }
                )
            })
        }));
    }

    #[test]
    fn candidate_limit_fails_closed() {
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_candidates = 1;
        let mut engine = DraftInferenceEngine::new(policy).expect("engine");
        let view = viewport(50.0);
        let target = [0.0, 0.0];
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(target),
                    None,
                    vec![point_anchor(101, target), point_anchor(102, target)],
                ),
                DraftInferenceInput::default(),
            )
            .expect("resource result");
        assert_eq!(resolution.status, DraftInferenceStatus::ResourceLimited);
        assert!(matches!(
            resolution.completeness,
            DraftInferenceCompleteness::CandidateLimit {
                required: 2,
                limit: 1
            }
        ));
        assert!(resolution.candidates.is_empty());
    }

    #[test]
    fn candidate_generation_stops_at_the_first_proven_overflow() {
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_candidates = 2;
        let mut engine = DraftInferenceEngine::new(policy).expect("engine");
        let view = viewport(50.0);
        let target = [0.0, 0.0];
        let anchors = (1..=4_096_u128)
            .map(|id| point_anchor(id, target))
            .collect();
        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen(target), None, anchors),
                DraftInferenceInput::default(),
            )
            .expect("bounded resource result");
        assert_eq!(resolution.status, DraftInferenceStatus::ResourceLimited);
        assert_eq!(
            resolution.completeness,
            DraftInferenceCompleteness::CandidateLimit {
                required: 3,
                limit: 2,
            }
        );
        assert!(resolution.candidates.is_empty());
        assert!(resolution.guides.is_empty());
        assert!(engine.remembered_references().is_empty());
    }

    #[test]
    fn candidate_identity_exhaustion_is_typed_instead_of_falling_through() {
        let view = viewport(50.0);
        let target = [0.0, 0.0];
        let retained_key = CandidateKey {
            anchor: Some(AnchorKey::Point(point_id(105))),
            datum: None,
            point_tracking: None,
            secondary_point_tracking: None,
            direction: None,
        };
        let retained_id = DraftInferenceCandidateId(41);
        let mut engine = DraftInferenceEngine {
            candidate_ids: vec![(retained_key, retained_id)],
            next_candidate_id: u64::MAX,
            ..DraftInferenceEngine::default()
        };
        let before = engine.clone();
        assert_eq!(
            engine
                .resolve(
                    &frame(
                        view,
                        view.model_to_screen(target),
                        None,
                        vec![point_anchor(105, target), point_anchor(106, target)],
                    ),
                    DraftInferenceInput::default(),
                )
                .expect_err("candidate identity exhaustion must fail closed"),
            DraftInferenceError::CandidateIdentityExhausted
        );
        assert_eq!(engine, before);
    }

    #[test]
    fn stale_preferred_candidate_does_not_fall_through() {
        let view = viewport(50.0);
        let target = [0.0, 0.0];
        let mut engine = DraftInferenceEngine::default();
        let resolution = engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(target),
                    None,
                    vec![point_anchor(110, target)],
                ),
                DraftInferenceInput {
                    suppressed: false,
                    preferred_candidate: Some(DraftInferenceCandidateId(999)),
                },
            )
            .expect("stale result");
        assert!(matches!(
            resolution.status,
            DraftInferenceStatus::StalePreferredCandidate { .. }
        ));
        assert_eq!(resolution.adjusted_model_position, target);
        assert!(engine.remembered_references().is_empty());
        assert!(engine.active_point_tracking.is_empty());
        assert!(engine.candidate_ids.is_empty());
    }

    #[test]
    fn non_finite_frames_reject_without_convergence_like_result() {
        let view = viewport(50.0);
        let mut engine = DraftInferenceEngine::default();
        let invalid = frame(
            view,
            ScreenPoint {
                x: f64::NAN,
                y: 0.0,
            },
            None,
            Vec::new(),
        );
        assert_eq!(
            engine
                .resolve(&invalid, DraftInferenceInput::default())
                .expect_err("non-finite rejection"),
            DraftInferenceError::InvalidFrame
        );

        let malformed_viewport = Viewport {
            screen_size: [1_000.0, 700.0],
            model_center: [0.0, 0.0],
            pixels_per_model_unit: -50.0,
        };
        assert_eq!(
            engine
                .resolve(
                    &frame(
                        malformed_viewport,
                        ScreenPoint { x: 500.0, y: 350.0 },
                        None,
                        Vec::new(),
                    ),
                    DraftInferenceInput::default(),
                )
                .expect_err("malformed viewport rejection"),
            DraftInferenceError::InvalidFrame
        );
    }

    #[test]
    fn finite_extreme_direction_projection_rejects_non_finite_output_transactionally() {
        let view = Viewport::new([1_000.0, 700.0], [0.0, 1.75e308], 1.0e-307)
            .expect("finite extreme viewport");
        let start = [0.0, 1.75e308];
        let target = [1.0e308, 1.75e308];
        let direction_radians = 3.0_f64.to_radians();
        let reference = affine_anchor(
            119,
            [0.0, 0.0],
            [direction_radians.cos(), direction_radians.sin()],
        );
        let pointer = view.model_to_screen(target);
        assert!(start.into_iter().chain(target).all(f64::is_finite));
        assert!(pointer.is_finite());

        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(reference)
            .expect("finite affine reference");
        let before = engine.clone();
        assert_eq!(
            engine
                .resolve(
                    &frame(view, pointer, Some(start), Vec::new()),
                    DraftInferenceInput::default(),
                )
                .expect_err("overflowed adjusted position must fail closed"),
            DraftInferenceError::InvalidFrame
        );
        assert_eq!(engine, before);
    }

    #[test]
    fn construction_scope_and_visibility_are_enforced_headlessly() {
        let view = viewport(50.0);
        let target = [0.0, 0.0];
        let construction = DraftReferenceAnchor::PersistentPoint {
            point: point_id(120),
            model_position: target,
            role_incidence: ScenePointRoleIncidence {
                profile: false,
                construction: true,
            },
        };
        let mut hidden = frame(view, view.model_to_screen(target), None, vec![construction]);
        hidden.geometry_policy.scope = GeometryPickScope::Profile;
        let mut engine = DraftInferenceEngine::default();
        assert_eq!(
            engine
                .resolve(&hidden, DraftInferenceInput::default())
                .expect("hidden")
                .status,
            DraftInferenceStatus::None
        );
        hidden.geometry_policy.scope = GeometryPickScope::Construction;
        assert!(matches!(
            engine
                .resolve(&hidden, DraftInferenceInput::default())
                .expect("construction")
                .status,
            DraftInferenceStatus::Resolved { .. }
        ));
    }

    #[test]
    fn construction_scope_ranks_dual_incidence_points_as_construction() {
        let view = viewport(100.0);
        let target = [0.0, 0.0];
        let construction_only = DraftReferenceAnchor::PersistentPoint {
            point: point_id(121),
            model_position: target,
            role_incidence: ScenePointRoleIncidence {
                profile: false,
                construction: true,
            },
        };
        let dual_incidence = DraftReferenceAnchor::PersistentPoint {
            point: point_id(122),
            model_position: [0.005, 0.0],
            role_incidence: ScenePointRoleIncidence {
                profile: true,
                construction: true,
            },
        };
        let mut input = frame(
            view,
            view.model_to_screen(target),
            None,
            vec![dual_incidence, construction_only],
        );
        input.geometry_policy.scope = GeometryPickScope::Construction;

        let resolution = DraftInferenceEngine::default()
            .resolve(&input, DraftInferenceInput::default())
            .expect("construction-scoped point ranking");
        let selected = resolved_candidate(&resolution);
        assert_eq!(
            selected.relations,
            vec![DraftInferenceRelation::PointIdentity {
                point: point_id(121),
            }]
        );
        assert_eq!(selected.ranking.positional_geometry_role_priority, 1);
        assert_eq!(selected.ranking.distance_pixels, 0.0);
        assert!(
            resolution
                .candidates
                .iter()
                .all(|candidate| { candidate.ranking.positional_geometry_role_priority == 1 })
        );
    }

    #[test]
    fn geometry_policy_change_clears_reference_memory_without_editor_help() {
        let view = viewport(50.0);
        let target = [2.0, 1.0];
        let reference = point_anchor(125, [0.0, 1.0]);
        let mut engine = DraftInferenceEngine::default();
        engine.remember_reference(reference).expect("remember");

        let initial = frame(view, view.model_to_screen(target), None, Vec::new());
        engine
            .resolve(&initial, DraftInferenceInput::default())
            .expect("initial resolution");
        assert_eq!(engine.remembered_references(), &[reference]);

        let mut changed = initial;
        changed.geometry_policy.scope = GeometryPickScope::Profile;
        engine
            .resolve(&changed, DraftInferenceInput::default())
            .expect("policy transition");
        assert!(engine.remembered_references().is_empty());
    }

    #[test]
    fn simultaneous_wake_retention_uses_semantics_instead_of_persistent_ids() {
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_remembered_references = 2;
        let view = viewport(1.0);
        let raw = [0.0, 0.0];
        let point = point_anchor(1, [6.0, 0.0]);
        let midpoint = midpoint_anchor(2, [5.0, 0.0], [1.0, 0.0]);
        let curve = affine_anchor(3, [1.0, 0.0], [1.0, 0.0]);

        for anchors in [
            vec![point, midpoint, curve],
            vec![curve, point, midpoint],
            vec![midpoint, curve, point],
        ] {
            let mut engine = DraftInferenceEngine::new(policy).expect("bounded memory");
            engine
                .resolve(
                    &frame(view, view.model_to_screen(raw), None, anchors),
                    DraftInferenceInput::default(),
                )
                .expect("simultaneous wake");
            let retained = engine
                .remembered_references()
                .iter()
                .copied()
                .map(DraftReferenceAnchor::key)
                .collect::<BTreeSet<_>>();
            assert_eq!(
                retained,
                BTreeSet::from([
                    AnchorKey::Point(point_id(1)),
                    AnchorKey::Midpoint(curve_span(2)),
                ])
            );
        }
    }

    #[test]
    fn nonlinear_contacts_do_not_consume_reusable_reference_capacity() {
        let view = viewport(50.0);
        let target = [0.0, 0.0];
        let affine = affine_anchor(150, [-1.0, 0.0], [1.0, 0.0]);
        let nonlinear_contacts = (0_u128..8)
            .map(|ordinal| DraftReferenceAnchor::CurvePoint {
                contact: contact(curve_span(151 + ordinal), 0.5),
                branch_candidate: DraftCurveBranchCandidate::default(),
                model_position: target,
                role: GeometryRole::Profile,
                source_role: GeometryRole::Profile,
                origin: DraftReferenceOrigin::Native,
            })
            .collect::<Vec<_>>();
        let mut engine = DraftInferenceEngine::default();
        engine.remember_reference(affine).expect("affine reference");
        for contact in nonlinear_contacts.iter().copied() {
            engine
                .remember_reference(contact)
                .expect("valid immediate-only contact");
        }
        assert_eq!(engine.remembered_references(), &[affine]);

        let resolution = engine
            .resolve(
                &frame(view, view.model_to_screen(target), None, nonlinear_contacts),
                DraftInferenceInput::default(),
            )
            .expect("nonlinear contact wake batch");
        assert!(matches!(
            resolution.status,
            DraftInferenceStatus::Ambiguous { .. }
        ));
        assert_eq!(engine.remembered_references(), &[affine]);
    }

    #[test]
    fn simultaneous_wake_does_not_split_an_exact_capacity_tie_by_identity() {
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_remembered_references = 1;
        let view = viewport(50.0);
        let target = [0.0, 0.0];
        for anchors in [
            vec![point_anchor(10, target), point_anchor(20, target)],
            vec![point_anchor(20, target), point_anchor(10, target)],
        ] {
            let mut engine = DraftInferenceEngine::new(policy).expect("single reference limit");
            let resolution = engine
                .resolve(
                    &frame(view, view.model_to_screen(target), None, anchors),
                    DraftInferenceInput::default(),
                )
                .expect("ambiguous tied wake");
            assert!(matches!(
                resolution.status,
                DraftInferenceStatus::Ambiguous { .. }
            ));
            assert!(
                engine.remembered_references().is_empty(),
                "an exact boundary tie cannot be resolved through persistent-ID order"
            );
        }

        let mut engine = DraftInferenceEngine::new(policy).expect("single reference limit");
        engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(target),
                    None,
                    vec![point_anchor(10, target)],
                ),
                DraftInferenceInput::default(),
            )
            .expect("first wake");
        assert_eq!(engine.remembered_references().len(), 1);
        engine
            .resolve(
                &frame(
                    view,
                    view.model_to_screen(target),
                    None,
                    vec![point_anchor(20, target)],
                ),
                DraftInferenceInput::default(),
            )
            .expect("later equivalent wake");
        assert!(
            engine.remembered_references().is_empty(),
            "later wakes must re-rank the complete memory rather than FIFO-evict by identity"
        );
    }

    #[test]
    fn reference_memory_is_bounded_and_deterministic() {
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_remembered_references = 2;
        let mut engine = DraftInferenceEngine::new(policy).expect("engine");
        engine
            .remember_reference(point_anchor(1, [0.0, 0.0]))
            .expect("first");
        engine
            .remember_reference(point_anchor(2, [1.0, 0.0]))
            .expect("second");
        engine
            .remember_reference(point_anchor(3, [2.0, 0.0]))
            .expect("third");
        assert_eq!(
            engine.remembered_references(),
            &[point_anchor(2, [1.0, 0.0]), point_anchor(3, [2.0, 0.0])]
        );
    }

    #[test]
    fn ordering_and_zoom_do_not_change_semantic_winner() {
        for scale in [25.0, 100.0] {
            let view = viewport(scale);
            let target = [2.0, 3.0];
            let first = point_anchor(130, target);
            let second = midpoint_anchor(131, target, [1.0, 0.0]);
            for anchors in [vec![first, second], vec![second, first]] {
                let mut engine = DraftInferenceEngine::default();
                let resolution = engine
                    .resolve(
                        &frame(view, view.model_to_screen(target), None, anchors),
                        DraftInferenceInput::default(),
                    )
                    .expect("resolve");
                assert!(matches!(
                    resolved_candidate(&resolution).relations.as_slice(),
                    [DraftInferenceRelation::PointIdentity { point }] if *point == point_id(130)
                ));
            }
        }
    }
}
