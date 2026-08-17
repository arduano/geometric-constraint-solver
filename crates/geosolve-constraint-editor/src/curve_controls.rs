// SPDX-License-Identifier: GPL-3.0-or-later

//! Selected-curve control-cage presentation and exact headless hit geometry.

use geosolve_sketch::{
    CurveDefinition, CurveId, CurveSpan, DesignPointId, DocumentArcSweep, DocumentCurveControl,
    DocumentCurveControlAvailability, DocumentCurveControlId, DocumentCurveControlKind,
    DocumentCurveControlTarget, DocumentCurveControlWithholdingReason,
    DocumentRationalConicControlMode, SketchDocument,
};

use super::{
    GeometryInteractionPolicy, PickTolerance, SceneCurveOrigin, ScreenPoint, Viewport,
    point_segment_projection, role_participates,
};

const CONTROL_GRIP_RADIUS_PIXELS: f64 = 4.5;
const STORED_POINT_GRIP_RADIUS_PIXELS: f64 = 3.5;
const HALF_SIZE_RAIL_PIXELS: f64 = 44.0;
const OVERLAPPING_DERIVED_GRIP_SHIFT_PIXELS: f64 = 16.0;

/// Stable screen-visible role of one selected-curve grip.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneCurveControlRole {
    StoredPoint,
    TrimEndpoint,
    Size,
    MiddleControl,
    ProjectiveVector,
}

/// Exact grip outline consumed by presentation and accessibility adapters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneCurveControlGripGeometry {
    Circle {
        center: ScreenPoint,
        radius_pixels: f64,
    },
    Square {
        center: ScreenPoint,
        half_extent_pixels: f64,
    },
    Diamond {
        center: ScreenPoint,
        radius_pixels: f64,
    },
}

impl SceneCurveControlGripGeometry {
    /// Common exact center used by the shared headless proximity resolver.
    #[must_use]
    pub const fn center(self) -> ScreenPoint {
        match self {
            Self::Circle { center, .. }
            | Self::Square { center, .. }
            | Self::Diamond { center, .. } => center,
        }
    }

    /// Whether a finite screen point lies on or inside the exact painted grip outline.
    ///
    /// The shared resolver unions this outline with the ordinary point-acquisition
    /// tolerance. That keeps the established CAD pick fringe without allowing a
    /// painted square corner or diamond edge to become an unclickable dead zone.
    #[must_use]
    pub fn contains(self, point: ScreenPoint) -> bool {
        if !point.is_finite() {
            return false;
        }
        match self {
            Self::Circle {
                center,
                radius_pixels,
            } => point.distance(center) <= radius_pixels,
            Self::Square {
                center,
                half_extent_pixels,
            } => {
                (point.x - center.x).abs() <= half_extent_pixels
                    && (point.y - center.y).abs() <= half_extent_pixels
            }
            Self::Diamond {
                center,
                radius_pixels,
            } => (point.x - center.x).abs() + (point.y - center.y).abs() <= radius_pixels,
        }
    }
}

/// Frozen positive one-dimensional rail for a selected size control.
///
/// Pointer deltas are projected onto `model_direction` at pointer-down. `model_zero`
/// is the signed zero boundary; crossing it is rejected before a domain projection can
/// reinterpret the sample on the opposite side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCurveControlRail {
    pub model_zero: [f64; 2],
    pub model_direction: [f64; 2],
    pub screen_start: ScreenPoint,
    pub screen_end: ScreenPoint,
}

impl SceneCurveControlRail {
    pub(crate) fn is_valid(self) -> bool {
        let norm = self.model_direction[0]
            .mul_add(self.model_direction[0], self.model_direction[1].powi(2));
        self.model_zero.into_iter().all(f64::is_finite)
            && self.model_direction.into_iter().all(f64::is_finite)
            && self.screen_start.is_finite()
            && self.screen_end.is_finite()
            && norm.is_finite()
            && (norm - 1.0).abs() <= 1.0e-9
    }
}

/// How a selected-curve grip participates in pointer ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneCurveControlInteraction {
    /// The grip is only a selected-cage alias for an ordinary persistent point.
    PointAlias(DesignPointId),
    /// The grip requests an inverse configuration edit owned by the curve.
    Direct,
}

/// One finite selected-curve grip.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneCurveControl {
    pub id: DocumentCurveControlId,
    /// Exact selected semantic span retained for selection and hover ownership.
    pub owner: CurveSpan,
    pub target: DocumentCurveControlTarget,
    pub availability: DocumentCurveControlAvailability,
    pub role: SceneCurveControlRole,
    pub interaction: SceneCurveControlInteraction,
    pub model_position: [f64; 2],
    /// Painted grip center. This normally projects `model_position`; a crowded
    /// derived grip may move along its tangent or one-dimensional rail so
    /// coincident semantic controls remain independently acquirable.
    pub screen_position: ScreenPoint,
    pub grip: SceneCurveControlGripGeometry,
    pub rail: Option<SceneCurveControlRail>,
    pub accessible_name: String,
}

impl SceneCurveControl {
    #[must_use]
    pub const fn is_editable(&self) -> bool {
        matches!(
            self.availability,
            DocumentCurveControlAvailability::Editable
        )
    }
}

/// Visual relationship represented by one exact control-cage segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneCurveControlGuideKind {
    ControlPolygon,
    PrincipalAxis,
    FocusAxis,
    RadiusSpoke,
    MinorAxisSpoke,
    ConjugateAxisSpoke,
    ProjectiveVector,
    SizeRail,
}

/// One exact selected-curve cage/guide segment.
///
/// `control` is present only when the painted segment shares direct-manipulation
/// ownership with that grip. Passive control polygons and principal axes never
/// manufacture an enlarged curve-control hit surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCurveControlGuide {
    pub owner: CurveId,
    pub kind: SceneCurveControlGuideKind,
    pub control: Option<DocumentCurveControlId>,
    pub model_start: [f64; 2],
    pub model_end: [f64; 2],
    pub screen_start: ScreenPoint,
    pub screen_end: ScreenPoint,
}

/// Typed result of the selected-curve hit resolver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneCurveControlHit {
    PointAlias {
        control: DocumentCurveControlId,
        owner: CurveSpan,
        point: DesignPointId,
        distance_pixels: f64,
    },
    Direct {
        control: DocumentCurveControlId,
        owner: CurveSpan,
        distance_pixels: f64,
    },
}

impl SceneCurveControlHit {
    #[must_use]
    pub const fn control(self) -> DocumentCurveControlId {
        match self {
            Self::PointAlias { control, .. } | Self::Direct { control, .. } => control,
        }
    }

    #[must_use]
    pub const fn owner(self) -> CurveSpan {
        match self {
            Self::PointAlias { owner, .. } | Self::Direct { owner, .. } => owner,
        }
    }

    #[must_use]
    pub const fn distance_pixels(self) -> f64 {
        match self {
            Self::PointAlias {
                distance_pixels, ..
            }
            | Self::Direct {
                distance_pixels, ..
            } => distance_pixels,
        }
    }

    const fn semantic_priority(self) -> u8 {
        match self {
            Self::PointAlias { .. } => 0,
            Self::Direct { .. } => 1,
        }
    }
}

pub(crate) fn build_selected_curve_controls(
    document: &SketchDocument,
    owner: CurveSpan,
    viewport: Viewport,
) -> Result<
    (Vec<SceneCurveControl>, Vec<SceneCurveControlGuide>),
    geosolve_sketch::DocumentCurveControlError,
> {
    let curve = document.curve(owner.curve).ok_or({
        geosolve_sketch::DocumentCurveControlError::UnknownControl {
            curve: owner.curve,
            kind: DocumentCurveControlKind::StartPoint,
        }
    })?;
    let domain_controls = document.curve_controls(owner.curve)?;
    if domain_controls.iter().all(|control| {
        matches!(
            control.availability,
            DocumentCurveControlAvailability::ReadOnly(
                DocumentCurveControlWithholdingReason::AssociativeFilletOutput
                    | DocumentCurveControlWithholdingReason::InactiveCurve
            )
        )
    }) {
        return Ok((Vec::new(), Vec::new()));
    }

    let mut controls = domain_controls
        .into_iter()
        .map(|control| scene_control(curve.label.as_str(), owner, viewport, control))
        .collect::<Vec<_>>();
    separate_overlapping_elliptical_arc_trim_grips(&curve.definition, &mut controls);
    separate_overlapping_elliptical_arc_minor_grip(&curve.definition, &mut controls);
    attach_size_rails(&mut controls, viewport);
    let guides = build_guides(&curve.definition, &controls, viewport);
    Ok((controls, guides))
}

fn separate_overlapping_elliptical_arc_trim_grips(
    definition: &CurveDefinition,
    controls: &mut [SceneCurveControl],
) {
    let CurveDefinition::EllipticalArc { sweep, .. } = definition else {
        return;
    };
    let Some(center_screen) = controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::Center)
        .map(|control| control.screen_position)
    else {
        return;
    };
    let Some(major_screen) = controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::MajorAxisPoint)
        .map(|control| control.screen_position)
    else {
        return;
    };
    let axis = [
        major_screen.x - center_screen.x,
        major_screen.y - center_screen.y,
    ];
    let axis_length = axis[0].hypot(axis[1]);
    if !axis_length.is_finite() || axis_length <= 0.0 {
        return;
    }
    let sweep_sign = match sweep {
        DocumentArcSweep::CounterClockwise => 1.0,
        DocumentArcSweep::Clockwise => -1.0,
    };
    let traversal_tangent = [
        sweep_sign * axis[1] / axis_length,
        -sweep_sign * axis[0] / axis_length,
    ];
    let painted_clearance = STORED_POINT_GRIP_RADIUS_PIXELS + CONTROL_GRIP_RADIUS_PIXELS + 2.0;
    for control in controls.iter_mut().filter(|control| {
        matches!(
            control.id.kind,
            DocumentCurveControlKind::TrimStart | DocumentCurveControlKind::TrimEnd
        )
    }) {
        if control.screen_position.distance(major_screen) > painted_clearance {
            continue;
        }
        let endpoint_sign = match control.id.kind {
            DocumentCurveControlKind::TrimStart => 1.0,
            DocumentCurveControlKind::TrimEnd => -1.0,
            _ => unreachable!("filtered elliptical-arc trim"),
        };
        let shifted = ScreenPoint {
            x: (endpoint_sign * OVERLAPPING_DERIVED_GRIP_SHIFT_PIXELS)
                .mul_add(traversal_tangent[0], control.screen_position.x),
            y: (endpoint_sign * OVERLAPPING_DERIVED_GRIP_SHIFT_PIXELS)
                .mul_add(traversal_tangent[1], control.screen_position.y),
        };
        control.screen_position = shifted;
        control.grip = SceneCurveControlGripGeometry::Square {
            center: shifted,
            half_extent_pixels: CONTROL_GRIP_RADIUS_PIXELS,
        };
    }
}

fn separate_overlapping_elliptical_arc_minor_grip(
    definition: &CurveDefinition,
    controls: &mut [SceneCurveControl],
) {
    if !matches!(definition, CurveDefinition::EllipticalArc { .. }) {
        return;
    }
    let Some(minor_index) = controls
        .iter()
        .position(|control| control.id.kind == DocumentCurveControlKind::MinorAxis)
    else {
        return;
    };
    let minor_screen = controls[minor_index].screen_position;
    let painted_separation = 2.0 * CONTROL_GRIP_RADIUS_PIXELS + 2.0;
    let overlaps_trim = controls.iter().any(|control| {
        matches!(
            control.id.kind,
            DocumentCurveControlKind::TrimStart | DocumentCurveControlKind::TrimEnd
        ) && minor_screen.distance(control.screen_position) <= painted_separation
    });
    if !overlaps_trim {
        return;
    }
    let Some(center_screen) = controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::Center)
        .map(|control| control.screen_position)
    else {
        return;
    };
    let direction = [
        minor_screen.x - center_screen.x,
        minor_screen.y - center_screen.y,
    ];
    let length = direction[0].hypot(direction[1]);
    if !length.is_finite() || length <= 0.0 {
        return;
    }
    let shift = OVERLAPPING_DERIVED_GRIP_SHIFT_PIXELS / length;
    let shifted = ScreenPoint {
        x: shift.mul_add(direction[0], minor_screen.x),
        y: shift.mul_add(direction[1], minor_screen.y),
    };
    let control = &mut controls[minor_index];
    control.screen_position = shifted;
    control.grip = SceneCurveControlGripGeometry::Diamond {
        center: shifted,
        radius_pixels: CONTROL_GRIP_RADIUS_PIXELS,
    };
}

fn scene_control(
    curve_label: &str,
    owner: CurveSpan,
    viewport: Viewport,
    control: DocumentCurveControl,
) -> SceneCurveControl {
    let interaction = match control.target {
        DocumentCurveControlTarget::Point(point) => SceneCurveControlInteraction::PointAlias(point),
        _ => SceneCurveControlInteraction::Direct,
    };
    let role = control_role(control.id.kind, control.target);
    let screen_position = viewport.model_to_screen(control.position);
    let grip = match role {
        SceneCurveControlRole::StoredPoint => SceneCurveControlGripGeometry::Circle {
            center: screen_position,
            radius_pixels: STORED_POINT_GRIP_RADIUS_PIXELS,
        },
        SceneCurveControlRole::TrimEndpoint | SceneCurveControlRole::MiddleControl => {
            SceneCurveControlGripGeometry::Square {
                center: screen_position,
                half_extent_pixels: CONTROL_GRIP_RADIUS_PIXELS,
            }
        }
        SceneCurveControlRole::Size | SceneCurveControlRole::ProjectiveVector => {
            SceneCurveControlGripGeometry::Diamond {
                center: screen_position,
                radius_pixels: CONTROL_GRIP_RADIUS_PIXELS,
            }
        }
    };
    SceneCurveControl {
        id: control.id,
        owner,
        target: control.target,
        availability: control.availability,
        role,
        interaction,
        model_position: control.position,
        screen_position,
        grip,
        rail: None,
        accessible_name: format!(
            "{} — {curve_label}",
            control_role_name(control.id.kind, control.target)
        ),
    }
}

fn control_role(
    kind: DocumentCurveControlKind,
    target: DocumentCurveControlTarget,
) -> SceneCurveControlRole {
    match kind {
        DocumentCurveControlKind::TrimStart | DocumentCurveControlKind::TrimEnd => {
            SceneCurveControlRole::TrimEndpoint
        }
        DocumentCurveControlKind::Radius
        | DocumentCurveControlKind::MinorAxis
        | DocumentCurveControlKind::ConjugateAxis => SceneCurveControlRole::Size,
        DocumentCurveControlKind::RationalMiddle => match target {
            DocumentCurveControlTarget::RationalMiddle {
                mode: DocumentRationalConicControlMode::Projective,
                ..
            } => SceneCurveControlRole::ProjectiveVector,
            _ => SceneCurveControlRole::MiddleControl,
        },
        _ => SceneCurveControlRole::StoredPoint,
    }
}

fn control_role_name(
    kind: DocumentCurveControlKind,
    target: DocumentCurveControlTarget,
) -> &'static str {
    match kind {
        DocumentCurveControlKind::Center => "Center",
        DocumentCurveControlKind::StartPoint | DocumentCurveControlKind::TrimStart => {
            "Start endpoint"
        }
        DocumentCurveControlKind::EndPoint | DocumentCurveControlKind::TrimEnd => "End endpoint",
        DocumentCurveControlKind::ControlPoint { .. } => "Control point",
        DocumentCurveControlKind::Radius => "Radius",
        DocumentCurveControlKind::MajorAxisPoint => "Major axis point",
        DocumentCurveControlKind::MinorAxis => "Minor axis",
        DocumentCurveControlKind::RationalMiddle => match target {
            DocumentCurveControlTarget::RationalMiddle {
                mode: DocumentRationalConicControlMode::Projective,
                ..
            } => "Projective middle Qh",
            _ => "Middle control P1",
        },
        DocumentCurveControlKind::Vertex => "Vertex",
        DocumentCurveControlKind::Focus => "Focus",
        DocumentCurveControlKind::TransverseAxisPoint => "Transverse axis point",
        DocumentCurveControlKind::ConjugateAxis => "Conjugate size",
        _ => "Curve control",
    }
}

fn attach_size_rails(controls: &mut [SceneCurveControl], viewport: Viewport) {
    let center = controls.iter().find_map(|control| {
        matches!(control.id.kind, DocumentCurveControlKind::Center)
            .then_some(control.model_position)
    });
    let Some(model_zero) = center else {
        return;
    };
    for control in controls.iter_mut().filter(|control| {
        matches!(
            control.id.kind,
            DocumentCurveControlKind::Radius
                | DocumentCurveControlKind::MinorAxis
                | DocumentCurveControlKind::ConjugateAxis
        )
    }) {
        let delta = [
            control.model_position[0] - model_zero[0],
            control.model_position[1] - model_zero[1],
        ];
        let length = delta[0].hypot(delta[1]);
        if !length.is_finite() || length <= 0.0 {
            continue;
        }
        let model_direction = [delta[0] / length, delta[1] / length];
        let direction_tip = viewport.model_to_screen([
            control.model_position[0] + model_direction[0],
            control.model_position[1] + model_direction[1],
        ]);
        let true_screen_position = viewport.model_to_screen(control.model_position);
        let screen_direction = [
            direction_tip.x - true_screen_position.x,
            direction_tip.y - true_screen_position.y,
        ];
        let screen_length = screen_direction[0].hypot(screen_direction[1]);
        if !screen_length.is_finite() || screen_length <= 0.0 {
            continue;
        }
        let unit = [
            screen_direction[0] / screen_length,
            screen_direction[1] / screen_length,
        ];
        let rail = SceneCurveControlRail {
            model_zero,
            model_direction,
            screen_start: ScreenPoint {
                x: (-HALF_SIZE_RAIL_PIXELS).mul_add(unit[0], control.screen_position.x),
                y: (-HALF_SIZE_RAIL_PIXELS).mul_add(unit[1], control.screen_position.y),
            },
            screen_end: ScreenPoint {
                x: HALF_SIZE_RAIL_PIXELS.mul_add(unit[0], control.screen_position.x),
                y: HALF_SIZE_RAIL_PIXELS.mul_add(unit[1], control.screen_position.y),
            },
        };
        if rail.is_valid() {
            control.rail = Some(rail);
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive family match keeps every selected-curve cage topology explicit"
)]
fn build_guides(
    definition: &CurveDefinition,
    controls: &[SceneCurveControl],
    viewport: Viewport,
) -> Vec<SceneCurveControlGuide> {
    let mut guides = Vec::new();
    let Some(owner) = controls.first().map(|control| control.id.curve) else {
        return guides;
    };
    let find = |kind| {
        controls
            .iter()
            .find(|control| control.id.kind == kind)
            .map(|control| control.model_position)
    };
    let connect_all = |guides: &mut Vec<SceneCurveControlGuide>, values: &[SceneCurveControl]| {
        for pair in values.windows(2) {
            guides.push(SceneCurveControlGuide {
                owner: pair[0].id.curve,
                kind: SceneCurveControlGuideKind::ControlPolygon,
                control: None,
                model_start: pair[0].model_position,
                model_end: pair[1].model_position,
                screen_start: pair[0].screen_position,
                screen_end: pair[1].screen_position,
            });
        }
    };

    match definition {
        CurveDefinition::QuadraticBezier { .. }
        | CurveDefinition::CubicBezier { .. }
        | CurveDefinition::BSpline { .. }
        | CurveDefinition::Nurbs { .. } => connect_all(&mut guides, controls),
        CurveDefinition::RationalQuadraticConic { .. } => {
            let middle = controls
                .iter()
                .find(|control| control.id.kind == DocumentCurveControlKind::RationalMiddle);
            if let Some(middle) = middle
                && middle.role == SceneCurveControlRole::ProjectiveVector
                && let Some(start) = find(DocumentCurveControlKind::StartPoint)
            {
                push_guide(
                    &mut guides,
                    owner,
                    viewport,
                    SceneCurveControlGuideKind::ProjectiveVector,
                    Some(middle.id),
                    start,
                    middle.model_position,
                );
            } else {
                connect_all(&mut guides, controls);
            }
        }
        CurveDefinition::Circle { .. } | CurveDefinition::CircularArc { .. } => {
            if let (Some(center), Some(radius)) = (
                find(DocumentCurveControlKind::Center),
                controls
                    .iter()
                    .find(|control| control.id.kind == DocumentCurveControlKind::Radius),
            ) {
                push_guide(
                    &mut guides,
                    owner,
                    viewport,
                    SceneCurveControlGuideKind::RadiusSpoke,
                    Some(radius.id),
                    center,
                    radius.model_position,
                );
            }
        }
        CurveDefinition::Ellipse { .. } | CurveDefinition::EllipticalArc { .. } => {
            if let Some(center) = find(DocumentCurveControlKind::Center) {
                if let Some(major) = find(DocumentCurveControlKind::MajorAxisPoint) {
                    push_guide(
                        &mut guides,
                        owner,
                        viewport,
                        SceneCurveControlGuideKind::PrincipalAxis,
                        None,
                        center,
                        major,
                    );
                }
                if let Some(minor) = controls
                    .iter()
                    .find(|control| control.id.kind == DocumentCurveControlKind::MinorAxis)
                {
                    push_guide(
                        &mut guides,
                        owner,
                        viewport,
                        SceneCurveControlGuideKind::MinorAxisSpoke,
                        Some(minor.id),
                        center,
                        minor.model_position,
                    );
                }
            }
        }
        CurveDefinition::ParabolaSegment { .. } => {
            if let (Some(vertex), Some(focus)) = (
                find(DocumentCurveControlKind::Vertex),
                find(DocumentCurveControlKind::Focus),
            ) {
                push_guide(
                    &mut guides,
                    owner,
                    viewport,
                    SceneCurveControlGuideKind::FocusAxis,
                    None,
                    vertex,
                    focus,
                );
            }
        }
        CurveDefinition::HyperbolaSegment { .. } => {
            if let Some(center) = find(DocumentCurveControlKind::Center) {
                if let Some(transverse) = find(DocumentCurveControlKind::TransverseAxisPoint) {
                    push_guide(
                        &mut guides,
                        owner,
                        viewport,
                        SceneCurveControlGuideKind::PrincipalAxis,
                        None,
                        center,
                        transverse,
                    );
                }
                if let Some(conjugate) = controls
                    .iter()
                    .find(|control| control.id.kind == DocumentCurveControlKind::ConjugateAxis)
                {
                    push_guide(
                        &mut guides,
                        owner,
                        viewport,
                        SceneCurveControlGuideKind::ConjugateAxisSpoke,
                        Some(conjugate.id),
                        center,
                        conjugate.model_position,
                    );
                }
            }
        }
        CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => {}
    }

    for control in controls.iter().filter(|control| control.rail.is_some()) {
        let rail = control.rail.expect("filtered finite rail");
        guides.push(SceneCurveControlGuide {
            owner: control.id.curve,
            kind: SceneCurveControlGuideKind::SizeRail,
            control: Some(control.id),
            model_start: viewport.screen_to_model(rail.screen_start),
            model_end: viewport.screen_to_model(rail.screen_end),
            screen_start: rail.screen_start,
            screen_end: rail.screen_end,
        });
    }
    guides
}

fn push_guide(
    guides: &mut Vec<SceneCurveControlGuide>,
    owner: CurveId,
    viewport: Viewport,
    kind: SceneCurveControlGuideKind,
    control: Option<DocumentCurveControlId>,
    start: [f64; 2],
    end: [f64; 2],
) {
    if start.into_iter().all(f64::is_finite) && end.into_iter().all(f64::is_finite) {
        guides.push(SceneCurveControlGuide {
            owner,
            kind,
            control,
            model_start: start,
            model_end: end,
            screen_start: viewport.model_to_screen(start),
            screen_end: viewport.model_to_screen(end),
        });
    }
}

pub(crate) fn curve_control_hit_test(
    controls: &[SceneCurveControl],
    guides: &[SceneCurveControlGuide],
    curves: &[super::SceneCurve],
    position: ScreenPoint,
    tolerance: PickTolerance,
    policy: GeometryInteractionPolicy,
) -> Option<SceneCurveControlHit> {
    if !position.is_finite() || !tolerance.is_valid() {
        return None;
    }
    controls
        .iter()
        .filter(|control| control.is_editable())
        .filter(|control| {
            curves.iter().any(|curve| {
                curve.span == control.owner
                    && curve.authoring_eligible
                    && matches!(
                        curve.origin,
                        SceneCurveOrigin::Native | SceneCurveOrigin::FilletDiscarded { .. }
                    )
                    && curve.is_interactive(policy)
                    && role_participates(curve.role, policy.scope)
            })
        })
        .filter_map(|control| {
            let grip_distance = position.distance(control.grip.center());
            let grip_hit =
                control.grip.contains(position) || grip_distance <= tolerance.point_pixels;
            let guide_distance = guides
                .iter()
                .filter(|guide| guide.control == Some(control.id))
                .map(|guide| {
                    point_segment_projection(position, guide.screen_start, guide.screen_end).0
                })
                .filter(|distance| *distance <= tolerance.curve_pixels)
                .min_by(f64::total_cmp);
            let distance = grip_hit
                .then_some(grip_distance)
                .into_iter()
                .chain(guide_distance)
                .min_by(f64::total_cmp)?;
            let hit = match control.interaction {
                SceneCurveControlInteraction::PointAlias(point) => {
                    SceneCurveControlHit::PointAlias {
                        control: control.id,
                        owner: control.owner,
                        point,
                        distance_pixels: distance,
                    }
                }
                SceneCurveControlInteraction::Direct => SceneCurveControlHit::Direct {
                    control: control.id,
                    owner: control.owner,
                    distance_pixels: distance,
                },
            };
            // Painted/acquired grips precede guide-only hits before distance
            // comparison. A direct spoke therefore cannot steal its stored-point
            // origin with a zero segment distance, while direct grips and the
            // guide beyond point acquisition remain unchanged.
            Some((u8::from(!grip_hit), hit))
        })
        .min_by(|(first_priority, first), (second_priority, second)| {
            first_priority
                .cmp(second_priority)
                .then_with(|| first.distance_pixels().total_cmp(&second.distance_pixels()))
                .then_with(|| first.semantic_priority().cmp(&second.semantic_priority()))
                .then_with(|| first.control().cmp(&second.control()))
        })
        .map(|(_, hit)| hit)
}
