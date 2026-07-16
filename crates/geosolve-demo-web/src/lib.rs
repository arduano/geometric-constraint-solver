//! WASM/SVG visual harness for live sketch and linkage verification fixtures.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

#[cfg(any(target_arch = "wasm32", test))]
mod playground;

#[cfg(any(target_arch = "wasm32", test))]
use std::f64::consts::{FRAC_PI_2, PI};
#[cfg(any(target_arch = "wasm32", test))]
use std::fmt::Write as _;

#[cfg(any(target_arch = "wasm32", test))]
use geosolve_core::{
    AuditAnnotations, AuditEvaluationStatus, AuditSnapshot, DiagnosticCompleteness,
    DiagnosticStatus, OneSidedMobility, ResidualCategory, SolveReport, SolveTermination,
    SolverConfig, SourceConstraintId, VariableValue,
};
#[cfg(any(target_arch = "wasm32", test))]
use geosolve_geometry::Point2;
#[cfg(any(target_arch = "wasm32", test))]
use geosolve_linkage::{
    BranchEvaluation, BranchMonitorId, BranchSign, DriveResult, FourBarAssemblyMode, FourBarIds,
    Linkage, LinkageGeometry, LinkageSolveDiagnostics, LinkageSolveResult, LinkageSource,
    SliderCrankAssemblyMode, SliderCrankIds, VelocityResult, four_bar_crossed, four_bar_open,
    slider_crank,
};
#[cfg(any(target_arch = "wasm32", test))]
use geosolve_sketch::{
    ArcCircleTangencySide, ArcId, ArcSweep, CircleContainment, CircleId, CircleTangencyMode,
    ConflictingRectangleIds, ContactState, DimensionKind, DimensionMode, LineParameterDomain,
    LineSide, PointId, SegmentId, Sketch, SketchConstraintId, SketchConstraintKind,
    SketchDimensionId, SketchSolveRequest, SketchSolveResult, SketchSource, SolveRejection,
    TangentCirclesIds, UnderconstrainedTriangleIds, conflicting_rectangle, tangent_circles,
    underconstrained_triangle,
};

#[cfg(any(target_arch = "wasm32", test))]
const SVG_VIEW_BOX: ViewBox = ViewBox {
    min_x: 0.0,
    min_y: 0.0,
    width: 640.0,
    height: 420.0,
};

#[cfg(any(target_arch = "wasm32", test))]
const MODEL_TRANSFORM: ModelSvgTransform = ModelSvgTransform {
    origin_x: 190.0,
    origin_y: 225.0,
    pixels_per_unit: 60.0,
};

#[cfg(any(target_arch = "wasm32", test))]
const FOUR_BAR_TRANSFORM: ModelSvgTransform = ModelSvgTransform {
    origin_x: 155.0,
    origin_y: 225.0,
    pixels_per_unit: 72.0,
};

#[cfg(any(target_arch = "wasm32", test))]
const SLIDER_CRANK_TRANSFORM: ModelSvgTransform = ModelSvgTransform {
    origin_x: 140.0,
    origin_y: 270.0,
    pixels_per_unit: 90.0,
};

#[cfg(any(target_arch = "wasm32", test))]
const CONFLICTING_RECTANGLE_TRANSFORM: ModelSvgTransform = ModelSvgTransform {
    origin_x: 170.0,
    origin_y: 300.0,
    pixels_per_unit: 70.0,
};

#[cfg(any(target_arch = "wasm32", test))]
const TANGENT_CIRCLES_TRANSFORM: ModelSvgTransform = ModelSvgTransform {
    origin_x: 240.0,
    origin_y: 245.0,
    pixels_per_unit: 70.0,
};

#[cfg(any(target_arch = "wasm32", test))]
const ARC_CONTACT_TRANSFORM: ModelSvgTransform = ModelSvgTransform {
    origin_x: 320.0,
    origin_y: 250.0,
    pixels_per_unit: 72.0,
};

#[cfg(any(target_arch = "wasm32", test))]
const ARC_CIRCLE_AUTO_RADIUS_TRANSFORM: ModelSvgTransform = ModelSvgTransform {
    origin_x: 290.0,
    origin_y: 255.0,
    pixels_per_unit: 58.0,
};

#[cfg(any(target_arch = "wasm32", test))]
const TANGENT_GLIDE_TRANSFORM: ModelSvgTransform = ModelSvgTransform {
    origin_x: 320.0,
    origin_y: 285.0,
    pixels_per_unit: 70.0,
};

#[cfg(any(target_arch = "wasm32", test))]
const DRAG_HIT_RADIUS: f64 = 47.0;

#[cfg(any(target_arch = "wasm32", test))]
const DRAG_CLAMP_MARGIN: f64 = DRAG_HIT_RADIUS;

#[cfg(any(target_arch = "wasm32", test))]
const _: () = assert!(DRAG_CLAMP_MARGIN >= DRAG_HIT_RADIUS);

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoScenario {
    UnderconstrainedTriangle,
    ConflictingRectangle,
    TangentCircles,
    ArcContactDrag,
    ArcCircleAutoRadius,
    LineCircleTangentGlide,
    HorizontalRail,
    CoincidentPair,
    FourBarOpen,
    FourBarCrossed,
    SliderCrank,
}

#[cfg(any(target_arch = "wasm32", test))]
impl DemoScenario {
    fn from_value(value: &str) -> Self {
        match value {
            "conflicting-rectangle" => Self::ConflictingRectangle,
            "tangent-circles" => Self::TangentCircles,
            "arc-contact-drag" => Self::ArcContactDrag,
            "arc-circle-auto-radius" => Self::ArcCircleAutoRadius,
            "line-circle-tangent-glide" => Self::LineCircleTangentGlide,
            "horizontal-rail" => Self::HorizontalRail,
            "coincident-pair" => Self::CoincidentPair,
            "four-bar-open" => Self::FourBarOpen,
            "four-bar-crossed" => Self::FourBarCrossed,
            "slider-crank" => Self::SliderCrank,
            _ => Self::UnderconstrainedTriangle,
        }
    }

    const fn selector_value(self) -> &'static str {
        match self {
            Self::UnderconstrainedTriangle => "triangle",
            Self::ConflictingRectangle => "conflicting-rectangle",
            Self::TangentCircles => "tangent-circles",
            Self::ArcContactDrag => "arc-contact-drag",
            Self::ArcCircleAutoRadius => "arc-circle-auto-radius",
            Self::LineCircleTangentGlide => "line-circle-tangent-glide",
            Self::HorizontalRail => "horizontal-rail",
            Self::CoincidentPair => "coincident-pair",
            Self::FourBarOpen => "four-bar-open",
            Self::FourBarCrossed => "four-bar-crossed",
            Self::SliderCrank => "slider-crank",
        }
    }

    const fn sketch_scene_kind(self) -> Option<LiveSceneKind> {
        match self {
            Self::UnderconstrainedTriangle => Some(LiveSceneKind::UnderconstrainedTriangle),
            Self::TangentCircles => Some(LiveSceneKind::TangentCircles),
            Self::ArcContactDrag => Some(LiveSceneKind::ArcContactDrag),
            Self::ArcCircleAutoRadius => Some(LiveSceneKind::ArcCircleAutoRadius),
            Self::LineCircleTangentGlide => Some(LiveSceneKind::LineCircleTangentGlide),
            Self::HorizontalRail => Some(LiveSceneKind::HorizontalRail),
            Self::CoincidentPair => Some(LiveSceneKind::CoincidentPair),
            Self::ConflictingRectangle
            | Self::FourBarOpen
            | Self::FourBarCrossed
            | Self::SliderCrank => None,
        }
    }

    const fn linkage_scene_kind(self) -> Option<LinkageSceneKind> {
        match self {
            Self::FourBarOpen => Some(LinkageSceneKind::FourBarOpen),
            Self::FourBarCrossed => Some(LinkageSceneKind::FourBarCrossed),
            Self::SliderCrank => Some(LinkageSceneKind::SliderCrank),
            Self::UnderconstrainedTriangle
            | Self::ConflictingRectangle
            | Self::TangentCircles
            | Self::ArcContactDrag
            | Self::ArcCircleAutoRadius
            | Self::LineCircleTangentGlide
            | Self::HorizontalRail
            | Self::CoincidentPair => None,
        }
    }

    const fn is_expected_conflict(self) -> bool {
        matches!(self, Self::ConflictingRectangle)
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinkageSceneKind {
    FourBarOpen,
    FourBarCrossed,
    SliderCrank,
}

#[cfg(any(target_arch = "wasm32", test))]
impl LinkageSceneKind {
    const fn scenario(self) -> DemoScenario {
        match self {
            Self::FourBarOpen => DemoScenario::FourBarOpen,
            Self::FourBarCrossed => DemoScenario::FourBarCrossed,
            Self::SliderCrank => DemoScenario::SliderCrank,
        }
    }

    const fn driver_range_degrees(self) -> (u16, u16) {
        match self {
            Self::FourBarOpen | Self::FourBarCrossed => (25, 135),
            Self::SliderCrank => (15, 165),
        }
    }

    const fn transform(self) -> ModelSvgTransform {
        match self {
            Self::FourBarOpen | Self::FourBarCrossed => FOUR_BAR_TRANSFORM,
            Self::SliderCrank => SLIDER_CRANK_TRANSFORM,
        }
    }

    const fn badge(self) -> &'static str {
        match self {
            Self::FourBarOpen => "live L1",
            Self::FourBarCrossed => "live L2",
            Self::SliderCrank => "live L3",
        }
    }

    const fn instructions(self) -> &'static str {
        match self {
            Self::FourBarOpen => {
                "Drive the input crank through the reviewed safe sweep. B remains on the explicit open assembly branch."
            }
            Self::FourBarCrossed => {
                "Drive the input crank through the reviewed safe sweep. B remains on the explicit crossed assembly branch."
            }
            Self::SliderCrank => {
                "Drive the crank through the reviewed safe sweep. The slider remains aligned to its guide on the positive-x branch."
            }
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveSceneKind {
    UnderconstrainedTriangle,
    TangentCircles,
    ArcContactDrag,
    ArcCircleAutoRadius,
    LineCircleTangentGlide,
    HorizontalRail,
    CoincidentPair,
}

#[cfg(any(target_arch = "wasm32", test))]
impl LiveSceneKind {
    const fn scenario(self) -> DemoScenario {
        match self {
            Self::UnderconstrainedTriangle => DemoScenario::UnderconstrainedTriangle,
            Self::TangentCircles => DemoScenario::TangentCircles,
            Self::ArcContactDrag => DemoScenario::ArcContactDrag,
            Self::ArcCircleAutoRadius => DemoScenario::ArcCircleAutoRadius,
            Self::LineCircleTangentGlide => DemoScenario::LineCircleTangentGlide,
            Self::HorizontalRail => DemoScenario::HorizontalRail,
            Self::CoincidentPair => DemoScenario::CoincidentPair,
        }
    }

    const fn badge(self) -> &'static str {
        match self {
            Self::UnderconstrainedTriangle => "live S1",
            Self::TangentCircles => "live S3",
            Self::ArcContactDrag => "live arc contact",
            Self::ArcCircleAutoRadius => "live auto radius",
            Self::LineCircleTangentGlide => "live tangent glide",
            Self::HorizontalRail => "live rail",
            Self::CoincidentPair => "live coincident",
        }
    }

    const fn instructions(self) -> &'static str {
        match self {
            Self::UnderconstrainedTriangle => {
                "Drag point C with a mouse, pen, or touch. It is projected onto the distance hard manifold; release keeps the accepted nearby position. A is fixed and B is free."
            }
            Self::TangentCircles => {
                "Use the scene action to switch the explicit tangency mode. The positive-x center branch and retained audit stay synchronized with accepted geometry."
            }
            Self::ArcContactDrag => {
                "Drag the contact point along the bounded counterclockwise arc. Targets on the span project onto it; targets beyond the visible endpoints are rejected and retain the prior state."
            }
            Self::ArcCircleAutoRadius => {
                "Drag the circle center in x and y outside the bounded arc. Center mobility comes from the retained solve report; radius and contact variables are solved. Invalid requests retain the prior accepted state."
            }
            Self::LineCircleTangentGlide => {
                "Drag the circle center parallel to the bounded segment. Tangency stays on the explicit Left side; requests beyond either endpoint are rejected and retain the prior state."
            }
            Self::HorizontalRail => {
                "Drag point B with a mouse, pen, or touch. The hard horizontal constraint projects it onto the rail; release keeps the accepted position. The displayed length is an equation-free reference measurement."
            }
            Self::CoincidentPair => {
                "Drag the inner B mark with a mouse, pen, or touch. The hard coincidence relation moves A and B together to the target; release keeps their common position."
            }
        }
    }

    const fn transform(self) -> ModelSvgTransform {
        match self {
            Self::TangentCircles => TANGENT_CIRCLES_TRANSFORM,
            Self::ArcContactDrag => ARC_CONTACT_TRANSFORM,
            Self::ArcCircleAutoRadius => ARC_CIRCLE_AUTO_RADIUS_TRANSFORM,
            Self::LineCircleTangentGlide => TANGENT_GLIDE_TRANSFORM,
            Self::UnderconstrainedTriangle | Self::HorizontalRail | Self::CoincidentPair => {
                MODEL_TRANSFORM
            }
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HorizontalRailIds {
    a: PointId,
    b: PointId,
    ab: SegmentId,
    reference_length: SketchDimensionId,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CoincidentPairIds {
    a: PointId,
    b: PointId,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ArcContactIds {
    center: PointId,
    point: PointId,
    arc: ArcId,
    contact: SketchConstraintId,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ArcCircleAutoRadiusIds {
    arc_center: PointId,
    circle_center: PointId,
    arc: ArcId,
    circle: CircleId,
    tangency: SketchConstraintId,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TangentGlideIds {
    line_start: PointId,
    line_end: PointId,
    center: PointId,
    line: SegmentId,
    circle: CircleId,
    tangency: SketchConstraintId,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveScene {
    UnderconstrainedTriangle(UnderconstrainedTriangleIds),
    TangentCircles(TangentCirclesIds),
    ArcContactDrag(ArcContactIds),
    ArcCircleAutoRadius(ArcCircleAutoRadiusIds),
    LineCircleTangentGlide(TangentGlideIds),
    HorizontalRail(HorizontalRailIds),
    CoincidentPair(CoincidentPairIds),
}

#[cfg(any(target_arch = "wasm32", test))]
impl LiveScene {
    const fn kind(self) -> LiveSceneKind {
        match self {
            Self::UnderconstrainedTriangle(_) => LiveSceneKind::UnderconstrainedTriangle,
            Self::TangentCircles(_) => LiveSceneKind::TangentCircles,
            Self::ArcContactDrag(_) => LiveSceneKind::ArcContactDrag,
            Self::ArcCircleAutoRadius(_) => LiveSceneKind::ArcCircleAutoRadius,
            Self::LineCircleTangentGlide(_) => LiveSceneKind::LineCircleTangentGlide,
            Self::HorizontalRail(_) => LiveSceneKind::HorizontalRail,
            Self::CoincidentPair(_) => LiveSceneKind::CoincidentPair,
        }
    }

    const fn draggable_point(self) -> Option<PointId> {
        match self {
            Self::UnderconstrainedTriangle(ids) => Some(ids.c),
            Self::ArcContactDrag(ids) => Some(ids.point),
            Self::ArcCircleAutoRadius(ids) => Some(ids.circle_center),
            Self::LineCircleTangentGlide(ids) => Some(ids.center),
            Self::HorizontalRail(ids) => Some(ids.b),
            Self::CoincidentPair(ids) => Some(ids.b),
            Self::TangentCircles(_) => None,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn build_live_scene(kind: LiveSceneKind) -> Result<(Sketch, LiveScene), String> {
    match kind {
        LiveSceneKind::UnderconstrainedTriangle => {
            let (sketch, ids) = underconstrained_triangle().map_err(|error| error.to_string())?;
            Ok((sketch, LiveScene::UnderconstrainedTriangle(ids)))
        }
        LiveSceneKind::TangentCircles => tangent_circles()
            .map(|(sketch, ids)| (sketch, LiveScene::TangentCircles(ids)))
            .map_err(|error| error.to_string()),
        LiveSceneKind::ArcContactDrag => {
            let mut sketch = Sketch::new(2.0).map_err(|error| error.to_string())?;
            let center = sketch
                .add_named_point("arc center", Point2::new(0.0, 0.0))
                .map_err(|error| error.to_string())?;
            let arc = sketch
                .add_named_arc(
                    "bounded CCW arc",
                    center,
                    2.0,
                    -PI / 6.0,
                    7.0 * PI / 6.0,
                    ArcSweep::CounterClockwise,
                )
                .map_err(|error| error.to_string())?;
            let initial_parameter = 0.38;
            let initial_point = sketch
                .evaluate_arc(arc, initial_parameter)
                .map_err(|error| error.to_string())?;
            let point = sketch
                .add_named_point("arc contact", initial_point)
                .map_err(|error| error.to_string())?;
            sketch
                .add_fixed_point(center)
                .map_err(|error| error.to_string())?;
            sketch
                .add_arc_radius(arc, 2.0, DimensionMode::Driving)
                .map_err(|error| error.to_string())?;
            let contact = sketch
                .add_point_on_arc(point, arc, initial_parameter)
                .map_err(|error| error.to_string())?;
            Ok((
                sketch,
                LiveScene::ArcContactDrag(ArcContactIds {
                    center,
                    point,
                    arc,
                    contact,
                }),
            ))
        }
        LiveSceneKind::ArcCircleAutoRadius => {
            let mut sketch = Sketch::new(2.2).map_err(|error| error.to_string())?;
            let arc_center = sketch
                .add_named_point("fixed arc center", Point2::new(0.0, 0.0))
                .map_err(|error| error.to_string())?;
            let circle_center = sketch
                .add_named_point("free circle center", Point2::new(3.4, 0.0))
                .map_err(|error| error.to_string())?;
            let arc = sketch
                .add_named_arc(
                    "300 degree CCW arc",
                    arc_center,
                    2.2,
                    -5.0 * PI / 6.0,
                    5.0 * PI / 6.0,
                    ArcSweep::CounterClockwise,
                )
                .map_err(|error| error.to_string())?;
            let circle = sketch
                .add_named_circle("auto-radius circle", circle_center, 1.2)
                .map_err(|error| error.to_string())?;
            sketch
                .add_fixed_point(arc_center)
                .map_err(|error| error.to_string())?;
            sketch
                .add_arc_radius(arc, 2.2, DimensionMode::Driving)
                .map_err(|error| error.to_string())?;
            let tangency = sketch
                .add_circle_arc_tangency(circle, arc, ArcCircleTangencySide::OutsideArc, 0.5, PI)
                .map_err(|error| error.to_string())?;
            Ok((
                sketch,
                LiveScene::ArcCircleAutoRadius(ArcCircleAutoRadiusIds {
                    arc_center,
                    circle_center,
                    arc,
                    circle,
                    tangency,
                }),
            ))
        }
        LiveSceneKind::LineCircleTangentGlide => {
            let mut sketch = Sketch::new(2.0).map_err(|error| error.to_string())?;
            let line_start = sketch
                .add_named_point("line A", Point2::new(-3.0, 0.0))
                .map_err(|error| error.to_string())?;
            let line_end = sketch
                .add_named_point("line B", Point2::new(3.0, 0.0))
                .map_err(|error| error.to_string())?;
            let center = sketch
                .add_named_point("circle center", Point2::new(-1.2, 1.0))
                .map_err(|error| error.to_string())?;
            let line = sketch
                .add_named_segment("bounded tangent segment", line_start, line_end)
                .map_err(|error| error.to_string())?;
            let circle = sketch
                .add_named_circle("gliding circle", center, 1.0)
                .map_err(|error| error.to_string())?;
            sketch
                .add_fixed_point(line_start)
                .map_err(|error| error.to_string())?;
            sketch
                .add_fixed_point(line_end)
                .map_err(|error| error.to_string())?;
            sketch
                .add_circle_radius(circle, 1.0, DimensionMode::Driving)
                .map_err(|error| error.to_string())?;
            let tangency = sketch
                .add_line_circle_tangency(
                    line,
                    circle,
                    LineParameterDomain::BoundedSegment,
                    LineSide::Left,
                    0.3,
                    -FRAC_PI_2,
                )
                .map_err(|error| error.to_string())?;
            Ok((
                sketch,
                LiveScene::LineCircleTangentGlide(TangentGlideIds {
                    line_start,
                    line_end,
                    center,
                    line,
                    circle,
                    tangency,
                }),
            ))
        }
        LiveSceneKind::HorizontalRail => {
            let mut sketch = Sketch::new(1.0).map_err(|error| error.to_string())?;
            let a = sketch
                .add_named_point("A", Point2::new(0.0, 0.0))
                .map_err(|error| error.to_string())?;
            let b = sketch
                .add_named_point("B", Point2::new(3.0, 0.0))
                .map_err(|error| error.to_string())?;
            let ab = sketch
                .add_named_segment("AB", a, b)
                .map_err(|error| error.to_string())?;
            sketch
                .add_fixed_point(a)
                .map_err(|error| error.to_string())?;
            sketch
                .add_horizontal(ab)
                .map_err(|error| error.to_string())?;
            let reference_length = sketch
                .add_segment_length(ab, 3.0, DimensionMode::Reference)
                .map_err(|error| error.to_string())?;
            Ok((
                sketch,
                LiveScene::HorizontalRail(HorizontalRailIds {
                    a,
                    b,
                    ab,
                    reference_length,
                }),
            ))
        }
        LiveSceneKind::CoincidentPair => {
            let mut sketch = Sketch::new(1.0).map_err(|error| error.to_string())?;
            let a = sketch
                .add_named_point("A", Point2::new(-1.0, 0.75))
                .map_err(|error| error.to_string())?;
            let b = sketch
                .add_named_point("B", Point2::new(1.0, -0.75))
                .map_err(|error| error.to_string())?;
            sketch
                .add_coincident(a, b)
                .map_err(|error| error.to_string())?;
            Ok((
                sketch,
                LiveScene::CoincidentPair(CoincidentPairIds { a, b }),
            ))
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
struct ExpectedConflictSource {
    source: SketchSource,
    diagnostic_label: &'static str,
    source_label: String,
    core_source_id: SourceConstraintId,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug)]
struct ConflictingRectangleState {
    ids: ConflictingRectangleIds,
    display: SketchSolveResult,
    conflicts: Vec<ExpectedConflictSource>,
    scene_error: Option<String>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl ConflictingRectangleState {
    fn new() -> Result<Self, String> {
        let (mut sketch, ids) = conflicting_rectangle().map_err(|error| error.to_string())?;
        let display = sketch
            .solve(
                SketchSolveRequest::default().without_previous_state_preferences(),
                SolverConfig::default(),
            )
            .map_err(|error| error.to_string())?;
        if display.accepted() || display.core_report.termination == SolveTermination::Converged {
            return Err("S2 unexpectedly produced an accepted/converged state".to_owned());
        }
        if !sketch_geometry_is_finite(&display.geometry) {
            return Err("S2 retained geometry is non-finite".to_owned());
        }
        let conflicts = expected_rectangle_conflicts(&display, ids)?;
        Ok(Self {
            ids,
            display,
            conflicts,
            scene_error: None,
        })
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn sketch_geometry_is_finite(geometry: &geosolve_sketch::SketchGeometry) -> bool {
    geometry
        .points
        .iter()
        .all(|point| point.position.x.is_finite() && point.position.y.is_finite())
        && geometry.circles.iter().all(|circle| {
            circle.center.x.is_finite()
                && circle.center.y.is_finite()
                && circle.radius.is_finite()
                && circle.radius > 0.0
        })
        && geometry.arcs.iter().all(|arc| {
            arc.center.x.is_finite()
                && arc.center.y.is_finite()
                && arc.radius.is_finite()
                && arc.radius > 0.0
                && arc.start_angle.is_finite()
                && arc.end_angle.is_finite()
                && arc.signed_sweep.is_finite()
        })
}

#[cfg(any(target_arch = "wasm32", test))]
fn expected_rectangle_conflicts(
    result: &SketchSolveResult,
    ids: ConflictingRectangleIds,
) -> Result<Vec<ExpectedConflictSource>, String> {
    let mapped: Vec<_> = result
        .core_report
        .conflicting_sources
        .iter()
        .map(|core_source_id| {
            let mapping = result
                .source_mappings
                .iter()
                .find(|mapping| mapping.core_source_id == Some(*core_source_id))
                .ok_or_else(|| format!("S2 conflict source {core_source_id:?} is not mapped"))?;
            let SketchSource::Dimension(dimension_id) = mapping.source else {
                return Err(format!(
                    "S2 unexpectedly blamed non-dimension source {:?}",
                    mapping.source
                ));
            };
            let diagnostic_label = if dimension_id == ids.width_4 {
                "width-4"
            } else if dimension_id == ids.width_5 {
                "width-5"
            } else {
                return Err(format!("S2 unexpectedly blamed dimension {dimension_id:?}"));
            };
            Ok(ExpectedConflictSource {
                source: mapping.source,
                diagnostic_label,
                source_label: mapping.source_label.clone(),
                core_source_id: *core_source_id,
            })
        })
        .collect::<Result<_, String>>()?;
    if mapped.len() != 2 {
        return Err(format!(
            "S2 expected two width conflicts, got {}",
            mapped.len()
        ));
    }
    [
        (SketchSource::Dimension(ids.width_4), "width-4"),
        (SketchSource::Dimension(ids.width_5), "width-5"),
    ]
    .into_iter()
    .map(|(source, label)| {
        mapped
            .iter()
            .find(|mapping| mapping.source == source && mapping.diagnostic_label == label)
            .cloned()
            .ok_or_else(|| format!("S2 is missing typed {label} conflict mapping"))
    })
    .collect()
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinkageScene {
    FourBar(FourBarIds),
    SliderCrank(SliderCrankIds),
}

#[cfg(any(target_arch = "wasm32", test))]
impl LinkageScene {
    const fn kind(self) -> LinkageSceneKind {
        match self {
            Self::FourBar(ids) => match ids.assembly_mode {
                FourBarAssemblyMode::Open => LinkageSceneKind::FourBarOpen,
                FourBarAssemblyMode::Crossed => LinkageSceneKind::FourBarCrossed,
            },
            Self::SliderCrank(_) => LinkageSceneKind::SliderCrank,
        }
    }

    const fn driver(self) -> geosolve_linkage::DriverId {
        match self {
            Self::FourBar(ids) => ids.driver,
            Self::SliderCrank(ids) => ids.driver,
        }
    }

    const fn mode_label(self) -> &'static str {
        match self {
            Self::FourBar(ids) => match ids.assembly_mode {
                FourBarAssemblyMode::Open => "Open",
                FourBarAssemblyMode::Crossed => "Crossed",
            },
            Self::SliderCrank(ids) => match ids.assembly_mode {
                SliderCrankAssemblyMode::PositiveX => "Positive-X",
            },
        }
    }

    const fn branch_monitor(self) -> BranchMonitorId {
        match self {
            Self::FourBar(ids) => ids.orientation_monitor,
            Self::SliderCrank(ids) => ids.positive_x_monitor,
        }
    }

    fn branch_evaluation(
        self,
        linkage: &Linkage,
        geometry: &LinkageGeometry,
    ) -> Result<BranchEvaluation, String> {
        linkage
            .evaluate_branch_monitor(self.branch_monitor(), geometry)
            .map_err(|error| error.to_string())
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn build_linkage_scene(kind: LinkageSceneKind) -> Result<(Linkage, LinkageScene), String> {
    match kind {
        LinkageSceneKind::FourBarOpen => four_bar_open()
            .map(|(linkage, ids)| (linkage, LinkageScene::FourBar(ids)))
            .map_err(|error| error.to_string()),
        LinkageSceneKind::FourBarCrossed => four_bar_crossed()
            .map(|(linkage, ids)| (linkage, LinkageScene::FourBar(ids)))
            .map_err(|error| error.to_string()),
        LinkageSceneKind::SliderCrank => slider_crank()
            .map(|(linkage, ids)| (linkage, LinkageScene::SliderCrank(ids)))
            .map_err(|error| error.to_string()),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn degrees_to_radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

#[cfg(any(target_arch = "wasm32", test))]
fn radians_to_degrees(radians: f64) -> f64 {
    radians * 180.0 / std::f64::consts::PI
}

#[cfg(any(target_arch = "wasm32", test))]
fn linkage_point(
    geometry: &LinkageGeometry,
    feature: geosolve_linkage::PointFeatureId,
    label: &str,
) -> Result<Point2<f64>, String> {
    let point = geometry
        .point(feature)
        .ok_or_else(|| format!("linkage result is missing {label}"))?;
    if point.x.is_finite() && point.y.is_finite() {
        Ok(point)
    } else {
        Err(format!("refusing to use non-finite {label}"))
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn linkage_geometry_is_finite(geometry: &LinkageGeometry) -> bool {
    geometry.bodies.iter().all(|body| {
        body.pose.translation.iter().all(|value| value.is_finite()) && body.pose.angle.is_finite()
    }) && geometry.points.iter().all(|point| {
        point.planar.coords.iter().all(|value| value.is_finite())
            && point.world.coords.iter().all(|value| value.is_finite())
    }) && geometry.axes.iter().all(|axis| {
        axis.planar.iter().all(|value| value.is_finite())
            && axis.world.iter().all(|value| value.is_finite())
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn scene_geometry_inside_view_box(
    scene: LinkageScene,
    geometry: &LinkageGeometry,
    margin: f64,
) -> bool {
    if !margin.is_finite() || margin < 0.0 {
        return false;
    }
    let feature_ids: Vec<_> = match scene {
        LinkageScene::FourBar(ids) => {
            vec![ids.ground_o2, ids.input_a, ids.coupler_b, ids.ground_o4]
        }
        LinkageScene::SliderCrank(ids) => vec![ids.ground_o, ids.crank_a, ids.slider_pin],
    };
    feature_ids.into_iter().all(|feature| {
        geometry.point(feature).is_some_and(|point| {
            let svg = scene.kind().transform().model_to_svg(point);
            svg.x >= SVG_VIEW_BOX.min_x + margin
                && svg.x <= SVG_VIEW_BOX.min_x + SVG_VIEW_BOX.width - margin
                && svg.y >= SVG_VIEW_BOX.min_y + margin
                && svg.y <= SVG_VIEW_BOX.min_y + SVG_VIEW_BOX.height - margin
        })
    })
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
struct LinkageRetainedDiagnostics {
    termination: SolveTermination,
    validated_hard_residual_max: Option<f64>,
    rank: Option<usize>,
    local_degrees_of_freedom: Option<usize>,
    iterations: usize,
    is_singular: Option<bool>,
    solve_diagnostics: LinkageSolveDiagnostics,
    conflict_sources: Vec<String>,
    redundancy_sources: Vec<String>,
    conflict_diagnostics: DiagnosticCompleteness,
    redundancy_diagnostics: DiagnosticCompleteness,
}

#[cfg(any(target_arch = "wasm32", test))]
impl LinkageRetainedDiagnostics {
    fn from_accepted(result: &LinkageSolveResult) -> Option<Self> {
        if !result.accepted() {
            return None;
        }
        let report = &result.core_report;
        Some(Self {
            termination: report.termination,
            validated_hard_residual_max: result
                .acceptance_hard_residual_max
                .filter(|value| value.is_finite())
                .or_else(|| {
                    (report.hard_residuals_validated && report.hard_residual_max.is_finite())
                        .then_some(report.hard_residual_max)
                })
                .or_else(|| audit_hard_residual_max(&result.display_audit)),
            rank: report.rank_is_valid.then_some(report.rank),
            local_degrees_of_freedom: report
                .rank_is_valid
                .then_some(report.local_degrees_of_freedom),
            iterations: report.iterations,
            is_singular: report.rank_is_valid.then_some(report.is_singular),
            solve_diagnostics: result.diagnostics,
            conflict_sources: source_labels(&report.conflicting_sources, &result.display_audit),
            redundancy_sources: source_labels(
                &report.sources_containing_redundant_rows,
                &result.display_audit,
            ),
            conflict_diagnostics: report.conflict_diagnostics,
            redundancy_diagnostics: report.redundancy_diagnostics,
        })
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
struct LinkageSampleSummary {
    target: f64,
    step: f64,
    accepted: bool,
    termination: SolveTermination,
    hard_residual_max: Option<f64>,
    checks: LinkageSampleChecks,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
struct LinkageSampleChecks {
    branch_evaluation: Option<BranchEvaluation>,
    geometry_is_finite: bool,
    render_points_inside: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
struct ContinuationSummary {
    initial_target: f64,
    requested_target: f64,
    accepted_target: f64,
    completed: bool,
    total_iterations: usize,
    samples: Vec<LinkageSampleSummary>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl ContinuationSummary {
    fn from_drive(drive: &DriveResult, scene: LinkageScene, linkage: &Linkage) -> Self {
        Self {
            initial_target: drive.initial_target,
            requested_target: drive.requested_target,
            accepted_target: drive.accepted_target,
            completed: drive.completed(),
            total_iterations: drive
                .samples
                .iter()
                .map(|sample| sample.solve.core_report.iterations)
                .sum(),
            samples: drive
                .samples
                .iter()
                .map(|sample| LinkageSampleSummary {
                    target: sample.target,
                    step: sample.step,
                    accepted: sample.solve.accepted(),
                    termination: sample.solve.core_report.termination,
                    hard_residual_max: sample.solve.acceptance_hard_residual_max,
                    checks: LinkageSampleChecks {
                        branch_evaluation: scene
                            .branch_evaluation(linkage, &sample.solve.geometry)
                            .ok(),
                        geometry_is_finite: linkage_geometry_is_finite(&sample.solve.geometry),
                        render_points_inside: scene_geometry_inside_view_box(
                            scene,
                            &sample.solve.geometry,
                            30.0,
                        ),
                    },
                })
                .collect(),
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
enum LinkageAttemptSummary {
    Accepted {
        termination: SolveTermination,
    },
    Rejected {
        termination: SolveTermination,
        rejection: String,
    },
    VelocityValidationFailed {
        termination: SolveTermination,
        position_target: f64,
        retained_target: f64,
        message: String,
    },
    Error {
        message: String,
    },
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug)]
struct InteractiveLinkageState {
    linkage: Linkage,
    scene: LinkageScene,
    display: LinkageSolveResult,
    retained_diagnostics: LinkageRetainedDiagnostics,
    attempt: LinkageAttemptSummary,
    continuation: Option<ContinuationSummary>,
    velocity: VelocityResult,
}

#[cfg(any(target_arch = "wasm32", test))]
impl InteractiveLinkageState {
    fn new(kind: LinkageSceneKind) -> Result<Self, String> {
        let (mut linkage, scene) = build_linkage_scene(kind)?;
        let display = linkage
            .solve(SolverConfig::default())
            .map_err(|error| error.to_string())?;
        if !display.accepted() {
            return Err(format!(
                "initial {:?} solve was rejected: {:?}",
                kind, display.rejection
            ));
        }
        let retained_diagnostics = LinkageRetainedDiagnostics::from_accepted(&display)
            .ok_or_else(|| format!("accepted {kind:?} result has no diagnostics"))?;
        let velocity = linkage
            .velocity(scene.driver(), 1.0)
            .map_err(|error| error.to_string())?;
        let attempt = LinkageAttemptSummary::Accepted {
            termination: display.core_report.termination,
        };
        Ok(Self {
            linkage,
            scene,
            display,
            retained_diagnostics,
            attempt,
            continuation: None,
            velocity,
        })
    }

    fn driver_degrees(&self) -> Result<f64, String> {
        self.linkage
            .driver(self.scene.driver())
            .map(|driver| radians_to_degrees(driver.target()))
            .ok_or_else(|| "linkage driver is unavailable".to_owned())
    }

    fn drive_to_degrees(&mut self, degrees: f64) {
        self.drive_to_degrees_with_velocity(degrees, |linkage, driver| {
            linkage
                .velocity(driver, 1.0)
                .map_err(|error| error.to_string())
        });
    }

    fn drive_to_degrees_with_velocity<F>(&mut self, degrees: f64, velocity_solver: F)
    where
        F: FnOnce(&Linkage, geosolve_linkage::DriverId) -> Result<VelocityResult, String>,
    {
        let target = degrees_to_radians(degrees);
        let previous_linkage = self.linkage.clone();
        let drive =
            match self
                .linkage
                .drive_to(self.scene.driver(), target, SolverConfig::default())
            {
                Ok(drive) => drive,
                Err(error) => {
                    self.linkage = previous_linkage;
                    self.attempt = LinkageAttemptSummary::Error {
                        message: format!("position request failed and was rolled back: {error}"),
                    };
                    self.continuation = None;
                    return;
                }
            };

        let position_accepted_target = drive.accepted_target;
        let mut summary = ContinuationSummary::from_drive(&drive, self.scene, &self.linkage);
        let attempt = if drive.completed() {
            LinkageAttemptSummary::Accepted {
                termination: drive
                    .samples
                    .last()
                    .map_or(self.retained_diagnostics.termination, |sample| {
                        sample.solve.core_report.termination
                    }),
            }
        } else {
            let failed = drive.samples.iter().find(|sample| !sample.solve.accepted());
            LinkageAttemptSummary::Rejected {
                termination: failed.map_or(self.retained_diagnostics.termination, |sample| {
                    sample.solve.core_report.termination
                }),
                rejection: failed.map_or_else(
                    || "continuation stopped before the requested target".to_owned(),
                    |sample| format!("{:?}", sample.solve.rejection),
                ),
            }
        };

        let latest_accepted = drive
            .samples
            .into_iter()
            .filter_map(|sample| sample.solve.accepted().then_some(sample.solve))
            .next_back();
        if let Some(display) = latest_accepted {
            let termination = display.core_report.termination;
            let publication = LinkageRetainedDiagnostics::from_accepted(&display)
                .ok_or_else(|| "accepted position has no retained diagnostics".to_owned())
                .and_then(|diagnostics| {
                    velocity_solver(&self.linkage, self.scene.driver())
                        .map(|velocity| (diagnostics, velocity))
                });
            match publication {
                Ok((diagnostics, velocity)) => {
                    self.display = display;
                    self.retained_diagnostics = diagnostics;
                    self.velocity = velocity;
                }
                Err(message) => {
                    self.linkage = previous_linkage;
                    summary.accepted_target = summary.initial_target;
                    summary.completed = false;
                    self.attempt = LinkageAttemptSummary::VelocityValidationFailed {
                        termination,
                        position_target: position_accepted_target,
                        retained_target: summary.accepted_target,
                        message,
                    };
                    self.continuation = Some(summary);
                    return;
                }
            }
        }
        self.attempt = attempt;
        self.continuation = Some(summary);
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct SvgPoint {
    x: f64,
    y: f64,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ModelSvgTransform {
    origin_x: f64,
    origin_y: f64,
    pixels_per_unit: f64,
}

#[cfg(any(target_arch = "wasm32", test))]
impl ModelSvgTransform {
    fn model_to_svg(self, point: Point2<f64>) -> SvgPoint {
        SvgPoint {
            x: self.origin_x + point.x * self.pixels_per_unit,
            y: self.origin_y - point.y * self.pixels_per_unit,
        }
    }

    fn svg_to_model(self, point: SvgPoint) -> Point2<f64> {
        Point2::new(
            (point.x - self.origin_x) / self.pixels_per_unit,
            (self.origin_y - point.y) / self.pixels_per_unit,
        )
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ViewBox {
    min_x: f64,
    min_y: f64,
    width: f64,
    height: f64,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ClientRect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

#[cfg(any(target_arch = "wasm32", test))]
fn client_to_svg(client: SvgPoint, bounds: ClientRect, view_box: ViewBox) -> Option<SvgPoint> {
    let values = [
        client.x,
        client.y,
        bounds.left,
        bounds.top,
        bounds.width,
        bounds.height,
        view_box.min_x,
        view_box.min_y,
        view_box.width,
        view_box.height,
    ];
    if values.iter().any(|value| !value.is_finite())
        || bounds.width <= 0.0
        || bounds.height <= 0.0
        || view_box.width <= 0.0
        || view_box.height <= 0.0
    {
        return None;
    }
    Some(SvgPoint {
        x: view_box.min_x + (client.x - bounds.left) * view_box.width / bounds.width,
        y: view_box.min_y + (client.y - bounds.top) * view_box.height / bounds.height,
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn clamp_drag_svg_point(kind: LiveSceneKind, point: SvgPoint) -> SvgPoint {
    match kind {
        LiveSceneKind::UnderconstrainedTriangle
        | LiveSceneKind::TangentCircles
        | LiveSceneKind::ArcContactDrag
        | LiveSceneKind::ArcCircleAutoRadius
        | LiveSceneKind::LineCircleTangentGlide => point,
        LiveSceneKind::HorizontalRail | LiveSceneKind::CoincidentPair => SvgPoint {
            x: point.x.clamp(
                SVG_VIEW_BOX.min_x + DRAG_CLAMP_MARGIN,
                SVG_VIEW_BOX.min_x + SVG_VIEW_BOX.width - DRAG_CLAMP_MARGIN,
            ),
            y: point.y.clamp(
                SVG_VIEW_BOX.min_y + DRAG_CLAMP_MARGIN,
                SVG_VIEW_BOX.min_y + SVG_VIEW_BOX.height - DRAG_CLAMP_MARGIN,
            ),
        },
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn client_to_drag_target(
    kind: LiveSceneKind,
    client: SvgPoint,
    bounds: ClientRect,
) -> Option<Point2<f64>> {
    let svg = client_to_svg(client, bounds, SVG_VIEW_BOX)?;
    Some(
        kind.transform()
            .svg_to_model(clamp_drag_svg_point(kind, svg)),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn pointer_start_allowed(
    is_primary: bool,
    pointer_type: &str,
    button: i16,
    drag_active: bool,
) -> bool {
    is_primary && !drag_active && (pointer_type != "mouse" || button == 0)
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
struct RetainedDiagnostics {
    termination: SolveTermination,
    validated_hard_residual_max: Option<f64>,
    rank: Option<usize>,
    local_degrees_of_freedom: Option<usize>,
    bounded_bidirectional_degrees_of_freedom: Option<usize>,
    one_sided_mobility: Option<OneSidedMobility>,
    iterations: usize,
    is_singular: Option<bool>,
    conflict_sources: Vec<String>,
    redundancy_sources: Vec<String>,
    bounds: Vec<String>,
    conflict_diagnostics: DiagnosticCompleteness,
    redundancy_diagnostics: DiagnosticCompleteness,
}

#[cfg(any(target_arch = "wasm32", test))]
impl RetainedDiagnostics {
    fn from_accepted(result: &SketchSolveResult) -> Option<Self> {
        if !result.accepted() {
            return None;
        }
        let report = &result.core_report;
        Some(Self {
            termination: report.termination,
            validated_hard_residual_max: result
                .acceptance_hard_residual_max
                .filter(|value| value.is_finite())
                .or_else(|| {
                    (report.hard_residuals_validated && report.hard_residual_max.is_finite())
                        .then_some(report.hard_residual_max)
                })
                .or_else(|| audit_hard_residual_max(&result.display_audit)),
            rank: report.rank_is_valid.then_some(report.rank),
            local_degrees_of_freedom: report.rank_is_valid.then_some(report.right_nullity),
            bounded_bidirectional_degrees_of_freedom: report
                .rank_is_valid
                .then_some(report.bidirectional_degrees_of_freedom),
            one_sided_mobility: report.rank_is_valid.then_some(report.one_sided_mobility),
            iterations: report.iterations,
            is_singular: report.rank_is_valid.then_some(report.is_singular),
            conflict_sources: source_labels(&report.conflicting_sources, &result.display_audit),
            redundancy_sources: source_labels(
                &report.sources_containing_redundant_rows,
                &result.display_audit,
            ),
            bounds: report
                .bounds
                .iter()
                .map(|bound| format!("{}: {:?}", bound.label, bound.status))
                .collect(),
            conflict_diagnostics: report.conflict_diagnostics,
            redundancy_diagnostics: report.redundancy_diagnostics,
        })
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Debug, PartialEq)]
enum AttemptSummary {
    Accepted {
        termination: SolveTermination,
    },
    Rejected {
        termination: SolveTermination,
        rejection: SolveRejection,
    },
    Error {
        message: String,
    },
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug)]
struct InteractiveSketchState {
    sketch: Sketch,
    scene: LiveScene,
    display: SketchSolveResult,
    retained_diagnostics: RetainedDiagnostics,
    attempt: AttemptSummary,
    active_pointer: Option<i32>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl InteractiveSketchState {
    fn new(kind: LiveSceneKind) -> Result<Self, String> {
        let (mut sketch, scene) = build_live_scene(kind)?;
        let display = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .map_err(|error| error.to_string())?;
        if !display.accepted() {
            return Err(format!(
                "initial {:?} solve was rejected: {:?}",
                scene.kind(),
                display.rejection
            ));
        }
        let retained_diagnostics = RetainedDiagnostics::from_accepted(&display)
            .ok_or_else(|| format!("accepted {:?} result has no diagnostics", scene.kind()))?;
        let attempt = AttemptSummary::Accepted {
            termination: display.core_report.termination,
        };
        Ok(Self {
            sketch,
            scene,
            display,
            retained_diagnostics,
            attempt,
            active_pointer: None,
        })
    }

    fn solve_drag(&mut self, target: Point2<f64>) {
        let Some(point) = self.scene.draggable_point() else {
            return;
        };
        let request = SketchSolveRequest::default().with_drag(point, target);
        self.solve_transactionally(request);
    }

    fn finish_drag(&mut self) {
        self.active_pointer = None;
        if matches!(self.attempt, AttemptSummary::Accepted { .. }) {
            self.solve(SketchSolveRequest::default());
        }
    }

    fn action_label(&self) -> Result<Option<&'static str>, String> {
        let LiveScene::TangentCircles(ids) = self.scene else {
            return Ok(None);
        };
        let label = match self
            .sketch
            .circle_tangency_mode(ids.tangency)
            .map_err(|error| error.to_string())?
        {
            CircleTangencyMode::External => "Switch to internal",
            CircleTangencyMode::Internal { .. } => "Switch to external",
        };
        Ok(Some(label))
    }

    fn trigger_action(&mut self) {
        let LiveScene::TangentCircles(ids) = self.scene else {
            return;
        };
        let next_mode = match self.sketch.circle_tangency_mode(ids.tangency) {
            Ok(CircleTangencyMode::External) => CircleTangencyMode::Internal {
                containment: CircleContainment::FirstContainsSecond,
            },
            Ok(CircleTangencyMode::Internal { .. }) => CircleTangencyMode::External,
            Err(error) => {
                self.attempt = AttemptSummary::Error {
                    message: error.to_string(),
                };
                return;
            }
        };
        self.set_tangent_mode(next_mode);
    }

    fn set_tangent_mode(&mut self, mode: CircleTangencyMode) {
        let LiveScene::TangentCircles(ids) = self.scene else {
            return;
        };
        let retained_sketch = self.sketch.clone();
        if let Err(error) = self.sketch.set_circle_tangency_mode(ids.tangency, mode) {
            self.attempt = AttemptSummary::Error {
                message: format!("tangency mode edit rejected: {error}"),
            };
            return;
        }
        match self
            .sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
        {
            Ok(result) if result.accepted() => self.apply_result(result),
            Ok(result) => {
                self.sketch = retained_sketch;
                self.record_rejection(&result);
            }
            Err(error) => {
                self.sketch = retained_sketch;
                self.attempt = AttemptSummary::Error {
                    message: error.to_string(),
                };
            }
        }
    }

    fn solve(&mut self, request: SketchSolveRequest) {
        match self.sketch.solve(request, SolverConfig::default()) {
            Ok(result) => self.apply_result(result),
            Err(error) => {
                self.attempt = AttemptSummary::Error {
                    message: error.to_string(),
                };
            }
        }
    }

    fn solve_transactionally(&mut self, request: SketchSolveRequest) {
        match self.sketch.solve(request, SolverConfig::default()) {
            Ok(result) if result.accepted() => self.apply_result(result),
            Ok(result) => self.record_rejection(&result),
            Err(error) => {
                self.attempt = AttemptSummary::Error {
                    message: error.to_string(),
                };
            }
        }
    }

    fn record_rejection(&mut self, result: &SketchSolveResult) {
        self.attempt = result.rejection.clone().map_or_else(
            || AttemptSummary::Error {
                message: "solve result was not accepted but supplied no typed rejection".to_owned(),
            },
            |rejection| AttemptSummary::Rejected {
                termination: result.core_report.termination,
                rejection,
            },
        );
    }

    fn apply_result(&mut self, result: SketchSolveResult) {
        if result.accepted() {
            if let Some(diagnostics) = RetainedDiagnostics::from_accepted(&result) {
                self.retained_diagnostics = diagnostics;
            }
            self.attempt = AttemptSummary::Accepted {
                termination: result.core_report.termination,
            };
        } else {
            self.record_rejection(&result);
        }
        self.display = result;
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug)]
enum DemoState {
    Sketch(Box<InteractiveSketchState>),
    ExpectedConflict(Box<ConflictingRectangleState>),
    Linkage(Box<InteractiveLinkageState>),
}

#[cfg(any(target_arch = "wasm32", test))]
impl DemoState {
    const fn scenario(&self) -> DemoScenario {
        match self {
            Self::Sketch(state) => state.scene.kind().scenario(),
            Self::ExpectedConflict(_) => DemoScenario::ConflictingRectangle,
            Self::Linkage(state) => state.scene.kind().scenario(),
        }
    }

    const fn selector_value(&self) -> &'static str {
        self.scenario().selector_value()
    }

    fn has_action(&self) -> bool {
        matches!(
            self,
            Self::Sketch(state) if matches!(state.scene, LiveScene::TangentCircles(_))
        )
    }

    fn action_label(&self) -> Result<Option<&'static str>, String> {
        match self {
            Self::Sketch(state) => state.action_label(),
            Self::ExpectedConflict(_) | Self::Linkage(_) => Ok(None),
        }
    }

    fn trigger_action(&mut self) {
        if let Self::Sketch(state) = self {
            state.trigger_action();
        }
    }

    fn drag_active(&self) -> bool {
        matches!(self, Self::Sketch(state) if state.active_pointer.is_some())
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct DemoApp {
    state: DemoState,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, PartialEq)]
struct LiveSketchView {
    geometry: String,
    audit: String,
    status: String,
    announcement: String,
    instructions: &'static str,
    badge: &'static str,
    action: Option<SceneActionView>,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SceneActionView {
    label: &'static str,
    help: &'static str,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, PartialEq)]
struct ExpectedConflictView {
    geometry: String,
    audit: String,
    status: String,
    announcement: String,
    instructions: &'static str,
    badge: &'static str,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct DriverControlView {
    min: u16,
    max: u16,
    step: u16,
    value: f64,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, PartialEq)]
struct LiveLinkageView {
    geometry: String,
    audit: String,
    status: String,
    announcement: String,
    instructions: &'static str,
    badge: &'static str,
    driver_control: DriverControlView,
}

#[cfg(any(target_arch = "wasm32", test))]
fn live_sketch_view(app: &InteractiveSketchState) -> Result<LiveSketchView, String> {
    let geometry = match app.scene {
        LiveScene::UnderconstrainedTriangle(ids) => {
            triangle_geometry_markup(&app.sketch, &app.display, ids, app.active_pointer.is_some())?
        }
        LiveScene::TangentCircles(ids) => tangent_circles_geometry_markup(app, ids)?,
        LiveScene::ArcContactDrag(ids) => {
            arc_contact_geometry_markup(app, ids, app.active_pointer.is_some())?
        }
        LiveScene::ArcCircleAutoRadius(ids) => {
            arc_circle_auto_radius_geometry_markup(app, ids, app.active_pointer.is_some())?
        }
        LiveScene::LineCircleTangentGlide(ids) => {
            tangent_glide_geometry_markup(app, ids, app.active_pointer.is_some())?
        }
        LiveScene::HorizontalRail(ids) => {
            horizontal_rail_geometry_markup(&app.display, ids, app.active_pointer.is_some())?
        }
        LiveScene::CoincidentPair(ids) => {
            coincident_pair_geometry_markup(&app.display, ids, app.active_pointer.is_some())?
        }
    };
    let mut audit = audit_markup(&app.display.display_audit, &[]);
    audit.push_str(&reference_measurements_markup(&app.display));
    Ok(LiveSketchView {
        geometry,
        audit,
        status: status_markup(
            &app.sketch,
            app.scene,
            &app.display,
            &app.retained_diagnostics,
            &app.attempt,
        ),
        announcement: sketch_announcement(&app.attempt),
        instructions: app.scene.kind().instructions(),
        badge: app.scene.kind().badge(),
        action: app.action_label()?.map(|label| SceneActionView {
            label,
            help: "Changes explicit sketch branch state, solves, and publishes only an accepted result.",
        }),
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn expected_conflict_view(
    state: &ConflictingRectangleState,
) -> Result<ExpectedConflictView, String> {
    Ok(ExpectedConflictView {
        geometry: conflicting_rectangle_geometry_markup(state)?,
        audit: audit_markup(&state.display.display_audit, &[]),
        status: conflicting_rectangle_status_markup(state),
        announcement: state.scene_error.as_ref().map_or_else(
            || {
                "Expected sketch conflict diagnosed. Retained rectangle geometry is displayed."
                    .to_owned()
            },
            |_| {
                "Scenario change failed. The retained conflicting rectangle remains displayed."
                    .to_owned()
            },
        ),
        instructions: "Expected diagnostic fixture. The canonical rectangle is retained for display; it has no pointer interaction.",
        badge: "expected S2 conflict",
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn conflicting_rectangle_geometry_markup(
    state: &ConflictingRectangleState,
) -> Result<String, String> {
    let geometry = &state.display.geometry;
    let a = sketch_geometry_point(geometry, state.ids.a, "S2 point A")?;
    let b = sketch_geometry_point(geometry, state.ids.b, "S2 point B")?;
    let c = sketch_geometry_point(geometry, state.ids.c, "S2 point C")?;
    let d = sketch_geometry_point(geometry, state.ids.d, "S2 point D")?;
    let a_svg = CONFLICTING_RECTANGLE_TRANSFORM.model_to_svg(a);
    let b_svg = CONFLICTING_RECTANGLE_TRANSFORM.model_to_svg(b);
    let c_svg = CONFLICTING_RECTANGLE_TRANSFORM.model_to_svg(c);
    let d_svg = CONFLICTING_RECTANGLE_TRANSFORM.model_to_svg(d);
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry conflicting-rectangle-geometry" data-sketch-scene="S2">
            <line class="rectangle-edge" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" data-segment="AB" />
            <line class="rectangle-edge" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" data-segment="BC" />
            <line class="rectangle-edge" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" data-segment="CD" />
            <line class="rectangle-edge" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" data-segment="DA" />
            <path class="conflict-width-cue" d="M {:.3} {:.3} L {:.3} {:.3}" />
            <circle class="point retained-conflict" cx="{:.3}" cy="{:.3}" r="8" data-point="A" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="point retained-conflict" cx="{:.3}" cy="{:.3}" r="8" data-point="B" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="point retained-conflict" cx="{:.3}" cy="{:.3}" r="8" data-point="C" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="point retained-conflict" cx="{:.3}" cy="{:.3}" r="8" data-point="D" data-model-x="{:.6}" data-model-y="{:.6}" />
            <text class="point-label" x="{:.3}" y="{:.3}">A</text>
            <text class="point-label" x="{:.3}" y="{:.3}">B</text>
            <text class="point-label" x="{:.3}" y="{:.3}">C</text>
            <text class="point-label" x="{:.3}" y="{:.3}">D</text>
            <text x="28" y="42" class="scene-kicker conflict">EXPECTED S2 CONFLICT / RETAINED GEOMETRY</text>
            <text x="28" y="68" class="scene-title">Conflicting rectangle / not a converged solution</text>
            <text x="{:.3}" y="{:.3}" class="conflict-label">width-4 versus width-5</text>
        </g>"#,
        a_svg.x,
        a_svg.y,
        b_svg.x,
        b_svg.y,
        b_svg.x,
        b_svg.y,
        c_svg.x,
        c_svg.y,
        c_svg.x,
        c_svg.y,
        d_svg.x,
        d_svg.y,
        d_svg.x,
        d_svg.y,
        a_svg.x,
        a_svg.y,
        a_svg.x,
        a_svg.y + 28.0,
        b_svg.x,
        b_svg.y + 28.0,
        a_svg.x,
        a_svg.y,
        a.x,
        a.y,
        b_svg.x,
        b_svg.y,
        b.x,
        b.y,
        c_svg.x,
        c_svg.y,
        c.x,
        c.y,
        d_svg.x,
        d_svg.y,
        d.x,
        d.y,
        a_svg.x - 26.0,
        a_svg.y + 24.0,
        b_svg.x + 12.0,
        b_svg.y + 24.0,
        c_svg.x + 12.0,
        c_svg.y - 12.0,
        d_svg.x - 26.0,
        d_svg.y - 12.0,
        (a_svg.x + b_svg.x) * 0.5,
        a_svg.y + 49.0,
    )
    .expect("writing conflicting rectangle SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
fn sketch_geometry_point(
    geometry: &geosolve_sketch::SketchGeometry,
    point: PointId,
    label: &str,
) -> Result<Point2<f64>, String> {
    let point = geometry
        .point(point)
        .ok_or_else(|| format!("retained geometry is missing {label}"))?;
    if point.x.is_finite() && point.y.is_finite() {
        Ok(point)
    } else {
        Err(format!("retained {label} is non-finite"))
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn conflicting_rectangle_status_markup(state: &ConflictingRectangleState) -> String {
    let report = &state.display.core_report;
    let hard_max = if report.hard_residual_max.is_finite() {
        format_metric(report.hard_residual_max)
    } else {
        "unavailable".to_owned()
    };
    let rank = report
        .rank_is_valid
        .then_some(report.rank)
        .map_or_else(|| "unavailable".to_owned(), |rank| rank.to_string());
    let dof = report
        .rank_is_valid
        .then_some(report.local_degrees_of_freedom)
        .map_or_else(|| "unavailable".to_owned(), |dof| dof.to_string());
    let conflict_labels = state
        .conflicts
        .iter()
        .map(|conflict| conflict.diagnostic_label)
        .collect::<Vec<_>>()
        .join(", ");
    let scene_error = state.scene_error.as_ref().map_or_else(String::new, |error| {
        format!(
            r#"<div class="attempt-banner rejected"><span class="attempt-light"></span><strong>scenario change failed / S2 retained</strong><span>{}</span></div>"#,
            escape_html(error)
        )
    });
    format!(
        r#"{}
            <div class="attempt-banner expected-conflict">
                <span class="attempt-light"></span>
                <strong>expected conflict diagnosed / retained geometry shown</strong>
                <span>The attempted solve did not converge and no candidate state was accepted.</span>
            </div>
            <div class="status-grid conflict-status-grid">
                <div><span>attempted termination</span><strong>{}</strong></div>
                <div><span>accepted state</span><strong>no / expected rejected diagnosis</strong></div>
                <div><span>attempted validated max hard residual</span><strong>{}</strong></div>
                <div><span>attempted rank / local DOF</span><strong>{} / {}</strong></div>
                <div><span>attempted conflict candidates</span><strong>{}</strong></div>
                <div><span>non-width sources blamed</span><strong>none</strong></div>
                <div><span>retained geometry</span><strong>canonical finite input geometry</strong></div>
                <div><span>display audit state</span><strong>retained geometry / not candidate geometry</strong></div>
                <div><span>solve rejection</span><strong>{}</strong></div>
            </div>"#,
        scene_error,
        termination_label(report.termination),
        hard_max,
        rank,
        dof,
        escape_html(&conflict_labels),
        escape_html(&format!("{:?}", state.display.rejection)),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn live_linkage_view(app: &InteractiveLinkageState) -> Result<LiveLinkageView, String> {
    let driver_source_ids: Vec<_> = app
        .display
        .source_mappings
        .iter()
        .filter_map(|mapping| {
            matches!(mapping.source, LinkageSource::Driver(_)).then_some(mapping.core_source_id)
        })
        .collect();
    let (min_degrees, max_degrees) = app.scene.kind().driver_range_degrees();
    Ok(LiveLinkageView {
        geometry: linkage_geometry_markup(app)?,
        audit: audit_markup(&app.display.display_audit, &driver_source_ids),
        status: linkage_status_markup(app)?,
        announcement: linkage_announcement(&app.attempt, app.continuation.as_ref()),
        instructions: app.scene.kind().instructions(),
        badge: app.scene.kind().badge(),
        driver_control: DriverControlView {
            min: min_degrees,
            max: max_degrees,
            step: 1,
            value: app.driver_degrees()?,
        },
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn linkage_geometry_markup(app: &InteractiveLinkageState) -> Result<String, String> {
    if !linkage_geometry_is_finite(&app.display.geometry) {
        return Err("refusing to render non-finite linkage geometry".to_owned());
    }
    match app.scene {
        LinkageScene::FourBar(ids) => four_bar_geometry_markup(app, ids),
        LinkageScene::SliderCrank(ids) => slider_crank_geometry_markup(app, ids),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn four_bar_geometry_markup(
    app: &InteractiveLinkageState,
    ids: FourBarIds,
) -> Result<String, String> {
    let geometry = &app.display.geometry;
    let o2 = linkage_point(geometry, ids.ground_o2, "four-bar O2")?;
    let o4 = linkage_point(geometry, ids.ground_o4, "four-bar O4")?;
    let a = linkage_point(geometry, ids.input_a, "four-bar A")?;
    let b = linkage_point(geometry, ids.coupler_b, "four-bar B")?;
    let transform = app.scene.kind().transform();
    let o2_svg = transform.model_to_svg(o2);
    let o4_svg = transform.model_to_svg(o4);
    let a_svg = transform.model_to_svg(a);
    let b_svg = transform.model_to_svg(b);
    let branch = app.scene.branch_evaluation(&app.linkage, geometry)?;
    let metric = branch.signed_metric;
    let driver_degrees = app.driver_degrees()?;
    let cue = angle_cue_points(o2, a, transform, 42.0)?;
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry linkage-geometry four-bar-geometry" data-linkage-scene="{:?}">
            <line class="ground-link" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <path class="ground-support" d="M {:.3} {:.3} l -12 18 h 24 Z M {:.3} {:.3} l -12 18 h 24 Z" />
            <line class="mechanism-link input-link" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="mechanism-link coupler-link" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="mechanism-link rocker-link" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <path class="driver-angle-cue" d="M {:.3} {:.3} A 42 42 0 0 0 {:.3} {:.3}" />
            <line class="driver-zero-cue" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <circle class="linkage-joint grounded" cx="{:.3}" cy="{:.3}" r="8" data-joint="O2" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="linkage-joint" cx="{:.3}" cy="{:.3}" r="8" data-joint="A" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="linkage-joint" cx="{:.3}" cy="{:.3}" r="8" data-joint="B" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="linkage-joint grounded" cx="{:.3}" cy="{:.3}" r="8" data-joint="O4" data-model-x="{:.6}" data-model-y="{:.6}" />
            <text class="joint-label" x="{:.3}" y="{:.3}">O2</text>
            <text class="joint-label" x="{:.3}" y="{:.3}">A</text>
            <text class="joint-label" x="{:.3}" y="{:.3}">B</text>
            <text class="joint-label" x="{:.3}" y="{:.3}">O4</text>
            <text class="driver-label" x="{:.3}" y="{:.3}">input {:.0} deg</text>
            <text x="28" y="42" class="scene-kicker">LIVE {} / SOLVED LINKAGE</text>
            <text x="28" y="68" class="scene-title">Four-bar / {} assembly</text>
            <text x="28" y="92" class="branch-label">orientation expected {} / metric {:.6} / retained {}</text>
        </g>"#,
        app.scene.kind(),
        o2_svg.x,
        o2_svg.y,
        o4_svg.x,
        o4_svg.y,
        o2_svg.x,
        o2_svg.y + 8.0,
        o4_svg.x,
        o4_svg.y + 8.0,
        o2_svg.x,
        o2_svg.y,
        a_svg.x,
        a_svg.y,
        a_svg.x,
        a_svg.y,
        b_svg.x,
        b_svg.y,
        b_svg.x,
        b_svg.y,
        o4_svg.x,
        o4_svg.y,
        cue.start.x,
        cue.start.y,
        cue.end.x,
        cue.end.y,
        o2_svg.x,
        o2_svg.y,
        o2_svg.x + 52.0,
        o2_svg.y,
        o2_svg.x,
        o2_svg.y,
        o2.x,
        o2.y,
        a_svg.x,
        a_svg.y,
        a.x,
        a.y,
        b_svg.x,
        b_svg.y,
        b.x,
        b.y,
        o4_svg.x,
        o4_svg.y,
        o4.x,
        o4.y,
        o2_svg.x - 28.0,
        o2_svg.y - 14.0,
        a_svg.x + 12.0,
        a_svg.y - 12.0,
        b_svg.x + 12.0,
        b_svg.y - 12.0,
        o4_svg.x + 12.0,
        o4_svg.y - 14.0,
        cue.end.x + 8.0,
        cue.end.y - 6.0,
        driver_degrees,
        match ids.assembly_mode {
            FourBarAssemblyMode::Open => "L1",
            FourBarAssemblyMode::Crossed => "L2",
        },
        app.scene.mode_label(),
        branch_sign_label(branch.expected_sign),
        metric,
        if branch.retained { "yes" } else { "no" },
    )
    .expect("writing four-bar SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn slider_crank_geometry_markup(
    app: &InteractiveLinkageState,
    ids: SliderCrankIds,
) -> Result<String, String> {
    let geometry = &app.display.geometry;
    let o = linkage_point(geometry, ids.ground_o, "slider-crank O")?;
    let a = linkage_point(geometry, ids.crank_a, "slider-crank A")?;
    let slider = linkage_point(geometry, ids.slider_pin, "slider pin")?;
    let guide_origin = linkage_point(geometry, ids.ground_guide_origin, "guide origin")?;
    let guide_axis = geometry
        .axis(ids.ground_guide_axis)
        .ok_or_else(|| "slider result is missing its guide axis".to_owned())?;
    let slider_axis = geometry
        .axis(ids.slider_axis)
        .ok_or_else(|| "slider result is missing its slider axis".to_owned())?;
    if guide_axis
        .iter()
        .chain(slider_axis.iter())
        .any(|v| !v.is_finite())
    {
        return Err("refusing to render non-finite slider axes".to_owned());
    }
    let transform = app.scene.kind().transform();
    let o_svg = transform.model_to_svg(o);
    let a_svg = transform.model_to_svg(a);
    let slider_svg = transform.model_to_svg(slider);
    let guide_start = transform.model_to_svg(guide_origin - guide_axis * 0.45);
    let guide_end = transform.model_to_svg(guide_origin + guide_axis * 5.35);
    let cue = angle_cue_points(o, a, transform, 42.0)?;
    let slider_rotation = (-slider_axis.y.atan2(slider_axis.x)).to_degrees();
    let branch = app.scene.branch_evaluation(&app.linkage, geometry)?;
    let metric = branch.signed_metric;
    let driver_degrees = app.driver_degrees()?;
    let aligned_start = transform.model_to_svg(slider - slider_axis * 0.42);
    let aligned_end = transform.model_to_svg(slider + slider_axis * 0.42);
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry linkage-geometry slider-crank-geometry" data-linkage-scene="SliderCrank">
            <line class="slider-guide" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="guide-centerline" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="mechanism-link input-link" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="mechanism-link connecting-rod" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <g class="slider-body" transform="translate({:.3} {:.3}) rotate({:.6})">
                <rect x="-29" y="-20" width="58" height="40" rx="5" />
            </g>
            <line class="aligned-axis-cue" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <path class="driver-angle-cue" d="M {:.3} {:.3} A 42 42 0 0 0 {:.3} {:.3}" />
            <line class="driver-zero-cue" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <circle class="linkage-joint grounded" cx="{:.3}" cy="{:.3}" r="8" data-joint="O" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="linkage-joint" cx="{:.3}" cy="{:.3}" r="8" data-joint="A" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="linkage-joint slider-pin" cx="{:.3}" cy="{:.3}" r="8" data-joint="slider" data-model-x="{:.6}" data-model-y="{:.6}" />
            <text class="joint-label" x="{:.3}" y="{:.3}">O</text>
            <text class="joint-label" x="{:.3}" y="{:.3}">A</text>
            <text class="joint-label" x="{:.3}" y="{:.3}">slider</text>
            <text class="driver-label" x="{:.3}" y="{:.3}">crank {:.0} deg</text>
            <text x="28" y="42" class="scene-kicker">LIVE L3 / SOLVED LINKAGE</text>
            <text x="28" y="68" class="scene-title">Slider-crank / Positive-X assembly</text>
            <text x="28" y="92" class="branch-label">positive-x expected {} / displacement {:.6} / retained {}</text>
            <text x="{:.3}" y="{:.3}" class="guide-label">aligned guide axis</text>
        </g>"#,
        guide_start.x,
        guide_start.y - 10.0,
        guide_end.x,
        guide_end.y - 10.0,
        guide_start.x,
        guide_start.y,
        guide_end.x,
        guide_end.y,
        o_svg.x,
        o_svg.y,
        a_svg.x,
        a_svg.y,
        a_svg.x,
        a_svg.y,
        slider_svg.x,
        slider_svg.y,
        slider_svg.x,
        slider_svg.y,
        slider_rotation,
        aligned_start.x,
        aligned_start.y,
        aligned_end.x,
        aligned_end.y,
        cue.start.x,
        cue.start.y,
        cue.end.x,
        cue.end.y,
        o_svg.x,
        o_svg.y,
        o_svg.x + 52.0,
        o_svg.y,
        o_svg.x,
        o_svg.y,
        o.x,
        o.y,
        a_svg.x,
        a_svg.y,
        a.x,
        a.y,
        slider_svg.x,
        slider_svg.y,
        slider.x,
        slider.y,
        o_svg.x - 24.0,
        o_svg.y - 14.0,
        a_svg.x + 12.0,
        a_svg.y - 12.0,
        slider_svg.x + 14.0,
        slider_svg.y - 28.0,
        cue.end.x + 8.0,
        cue.end.y - 6.0,
        driver_degrees,
        branch_sign_label(branch.expected_sign),
        metric,
        if branch.retained { "yes" } else { "no" },
        guide_start.x + 8.0,
        guide_start.y + 25.0,
    )
    .expect("writing slider-crank SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct AngleCue {
    start: SvgPoint,
    end: SvgPoint,
}

#[cfg(any(target_arch = "wasm32", test))]
fn angle_cue_points(
    center: Point2<f64>,
    endpoint: Point2<f64>,
    transform: ModelSvgTransform,
    radius: f64,
) -> Result<AngleCue, String> {
    let direction = endpoint - center;
    let norm = direction.x.hypot(direction.y);
    if !norm.is_finite() || norm <= f64::EPSILON || !radius.is_finite() || radius <= 0.0 {
        return Err("driver angle cue has invalid solved geometry".to_owned());
    }
    let center_svg = transform.model_to_svg(center);
    Ok(AngleCue {
        start: SvgPoint {
            x: center_svg.x + radius,
            y: center_svg.y,
        },
        end: SvgPoint {
            x: center_svg.x + radius * direction.x / norm,
            y: center_svg.y - radius * direction.y / norm,
        },
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn metric_sign_label(metric: f64) -> &'static str {
    if metric > 0.0 {
        "positive"
    } else if metric < 0.0 {
        "negative"
    } else {
        "zero"
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ArrowHead {
    left: SvgPoint,
    tip: SvgPoint,
    right: SvgPoint,
}

#[cfg(any(target_arch = "wasm32", test))]
fn arrow_head(from: SvgPoint, tip: SvgPoint, size: f64) -> Result<ArrowHead, String> {
    let direction_x = tip.x - from.x;
    let direction_y = tip.y - from.y;
    let length = direction_x.hypot(direction_y);
    if !length.is_finite() || length <= f64::EPSILON || !size.is_finite() || size <= 0.0 {
        return Err("arrow cue has invalid solved geometry".to_owned());
    }
    let unit_x = direction_x / length;
    let unit_y = direction_y / length;
    let base_x = tip.x - size * unit_x;
    let base_y = tip.y - size * unit_y;
    let half_width = size * 0.48;
    Ok(ArrowHead {
        left: SvgPoint {
            x: base_x - half_width * unit_y,
            y: base_y + half_width * unit_x,
        },
        tip,
        right: SvgPoint {
            x: base_x + half_width * unit_y,
            y: base_y - half_width * unit_x,
        },
    })
}

#[cfg(any(target_arch = "wasm32", test))]
const fn circle_tangency_mode_label(mode: CircleTangencyMode) -> &'static str {
    match mode {
        CircleTangencyMode::External => "External",
        CircleTangencyMode::Internal {
            containment: CircleContainment::FirstContainsSecond,
        } => "Internal / A contains B",
        CircleTangencyMode::Internal {
            containment: CircleContainment::SecondContainsFirst,
        } => "Internal / B contains A",
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn tangent_circles_geometry_markup(
    app: &InteractiveSketchState,
    ids: TangentCirclesIds,
) -> Result<String, String> {
    let first = *app
        .display
        .geometry
        .circle(ids.circle_a)
        .ok_or_else(|| "S3 result is missing circle A".to_owned())?;
    let second = *app
        .display
        .geometry
        .circle(ids.circle_b)
        .ok_or_else(|| "S3 result is missing circle B".to_owned())?;
    let mode = app
        .sketch
        .circle_tangency_mode(ids.tangency)
        .map_err(|error| error.to_string())?;
    let SketchConstraintKind::CircleCircleTangency {
        center_direction, ..
    } = app
        .sketch
        .constraint(ids.tangency)
        .ok_or_else(|| "S3 tangency source is unavailable".to_owned())?
        .kind()
    else {
        return Err("S3 source is not circle-circle tangency".to_owned());
    };
    let displacement = second.center - first.center;
    let center_distance = displacement.norm();
    if !sketch_geometry_is_finite(&app.display.geometry)
        || !center_distance.is_finite()
        || center_distance <= 0.0
    {
        return Err("refusing to render invalid S3 geometry".to_owned());
    }
    let contact_angle = displacement.y.atan2(displacement.x);
    let contact = first
        .evaluate(contact_angle)
        .ok_or_else(|| "S3 contact evaluation failed".to_owned())?;
    let direction_cosine = center_direction
        .direction_cosine(first.center, second.center)
        .ok_or_else(|| "S3 center branch is undefined".to_owned())?;
    let transform = LiveSceneKind::TangentCircles.transform();
    let first_svg = transform.model_to_svg(first.center);
    let second_svg = transform.model_to_svg(second.center);
    let contact_svg = transform.model_to_svg(contact);
    let branch_tip = SvgPoint {
        x: first_svg.x + 0.62 * (second_svg.x - first_svg.x),
        y: first_svg.y + 0.62 * (second_svg.y - first_svg.y),
    };
    let branch_from = SvgPoint {
        x: first_svg.x + 0.38 * (second_svg.x - first_svg.x),
        y: first_svg.y + 0.38 * (second_svg.y - first_svg.y),
    };
    let arrow = arrow_head(branch_from, branch_tip, 12.0)?;
    let dimension_y = 406.0;
    let mode_label = circle_tangency_mode_label(mode);
    let first_radius = first.radius * transform.pixels_per_unit;
    let second_radius = second.radius * transform.pixels_per_unit;
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry tangent-circles-geometry" data-sketch-scene="S3" data-tangency-mode="{mode_label}">
            <line class="center-rail" x1="78" y1="{rail_y:.3}" x2="570" y2="{rail_y:.3}" />
            <line class="center-branch" x1="{ax:.3}" y1="{ay:.3}" x2="{bx:.3}" y2="{by:.3}" />
            <path class="branch-arrow" d="M {arrow_lx:.3} {arrow_ly:.3} L {arrow_tx:.3} {arrow_ty:.3} L {arrow_rx:.3} {arrow_ry:.3}" />
            <circle class="curve-circle circle-a" cx="{ax:.3}" cy="{ay:.3}" r="{ar:.3}" />
            <circle class="curve-circle circle-b" cx="{bx:.3}" cy="{by:.3}" r="{br:.3}" />
            <circle class="curve-center fixed-center" cx="{ax:.3}" cy="{ay:.3}" r="6" />
            <circle class="curve-center" cx="{bx:.3}" cy="{by:.3}" r="6" />
            <circle class="contact-marker" cx="{cx:.3}" cy="{cy:.3}" r="7" data-contact-x="{contact_x:.6}" data-contact-y="{contact_y:.6}" />
            <line class="dimension-extension" x1="{ax:.3}" y1="{ay:.3}" x2="{ax:.3}" y2="{dimension_y:.3}" />
            <line class="dimension-extension" x1="{bx:.3}" y1="{by:.3}" x2="{bx:.3}" y2="{dimension_y:.3}" />
            <line class="center-dimension" x1="{ax:.3}" y1="{dimension_y:.3}" x2="{bx:.3}" y2="{dimension_y:.3}" />
            <path class="dimension-arrows" d="M {ax:.3} {dimension_y:.3} l 10 -5 M {ax:.3} {dimension_y:.3} l 10 5 M {bx:.3} {dimension_y:.3} l -10 -5 M {bx:.3} {dimension_y:.3} l -10 5" />
            <text class="dimension-label" x="{dimension_mid:.3}" y="397">center distance {center_distance:.3}</text>
            <text class="curve-label" x="{a_label_x:.3}" y="{a_label_y:.3}">A / r 2</text>
            <text class="curve-label" x="{b_label_x:.3}" y="{b_label_y:.3}">B / r 1</text>
            <text class="contact-label" x="{contact_label_x:.3}" y="{contact_label_y:.3}">contact</text>
            <text x="28" y="42" class="scene-kicker">LIVE S3 / EXPLICIT CURVE BRANCH</text>
            <text x="28" y="68" class="scene-title">Tangent circles / {mode_label}</text>
            <text x="28" y="92" class="branch-label">positive-x branch / cosine {direction_cosine:.6} / contact distance {center_distance:.3}</text>
        </g>"#,
        rail_y = first_svg.y,
        ax = first_svg.x,
        ay = first_svg.y,
        bx = second_svg.x,
        by = second_svg.y,
        arrow_lx = arrow.left.x,
        arrow_ly = arrow.left.y,
        arrow_tx = arrow.tip.x,
        arrow_ty = arrow.tip.y,
        arrow_rx = arrow.right.x,
        arrow_ry = arrow.right.y,
        ar = first_radius,
        br = second_radius,
        cx = contact_svg.x,
        cy = contact_svg.y,
        contact_x = contact.x,
        contact_y = contact.y,
        dimension_mid = (first_svg.x + second_svg.x) * 0.5,
        a_label_x = first_svg.x - first_radius + 8.0,
        a_label_y = first_svg.y - 12.0,
        b_label_x = second_svg.x - 24.0,
        b_label_y = second_svg.y - second_radius - 12.0,
        contact_label_x = contact_svg.x + 11.0,
        contact_label_y = contact_svg.y - 10.0,
    )
    .expect("writing S3 SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn arc_contact_geometry_markup(
    app: &InteractiveSketchState,
    ids: ArcContactIds,
    dragging: bool,
) -> Result<String, String> {
    let arc = *app
        .display
        .geometry
        .arc(ids.arc)
        .ok_or_else(|| "arc contact result is missing its arc".to_owned())?;
    let point = sketch_geometry_point(&app.display.geometry, ids.point, "arc contact point")?;
    let center = sketch_geometry_point(&app.display.geometry, ids.center, "arc center")?;
    let ContactState::PointOnArc { span_parameter } = app
        .sketch
        .contact_state(ids.contact)
        .map_err(|error| error.to_string())?
    else {
        return Err("arc contact source has the wrong latent state".to_owned());
    };
    let (start, end) = arc
        .endpoints()
        .ok_or_else(|| "arc endpoints are invalid".to_owned())?;
    let evaluated = arc
        .evaluate(span_parameter)
        .ok_or_else(|| "committed arc contact parameter is invalid".to_owned())?;
    if (evaluated - point).norm() > 1.0e-7 || !sketch_geometry_is_finite(&app.display.geometry) {
        return Err("arc display geometry and committed contact disagree".to_owned());
    }
    let transform = LiveSceneKind::ArcContactDrag.transform();
    let center_svg = transform.model_to_svg(center);
    let start_svg = transform.model_to_svg(start);
    let end_svg = transform.model_to_svg(end);
    let point_svg = transform.model_to_svg(point);
    let cue_from = transform.model_to_svg(
        arc.evaluate(0.1)
            .ok_or_else(|| "arc orientation cue is invalid".to_owned())?,
    );
    let cue_tip = transform.model_to_svg(
        arc.evaluate(0.17)
            .ok_or_else(|| "arc orientation cue is invalid".to_owned())?,
    );
    let arrow = arrow_head(cue_from, cue_tip, 12.0)?;
    let large_arc = u8::from(arc.signed_sweep.abs() > PI);
    let svg_sweep = u8::from(arc.signed_sweep < 0.0);
    let radius = arc.radius * transform.pixels_per_unit;
    let active_class = if dragging { " active" } else { "" };
    let indicator_start = 78.0;
    let indicator_end = 270.0;
    let indicator_x = indicator_start + span_parameter * (indicator_end - indicator_start);
    let sweep_degrees = arc.signed_sweep.to_degrees();
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry arc-contact-geometry" data-sketch-scene="ArcContact" data-span-parameter="{span_parameter:.6}">
            <line class="radius-guide endpoint-guide" x1="{ox:.3}" y1="{oy:.3}" x2="{sx:.3}" y2="{sy:.3}" />
            <line class="radius-guide endpoint-guide" x1="{ox:.3}" y1="{oy:.3}" x2="{ex:.3}" y2="{ey:.3}" />
            <line class="radius-guide contact-guide" x1="{ox:.3}" y1="{oy:.3}" x2="{px:.3}" y2="{py:.3}" />
            <path class="bounded-arc" d="M {sx:.3} {sy:.3} A {radius:.3} {radius:.3} 0 {large_arc} {svg_sweep} {ex:.3} {ey:.3}" />
            <line class="arc-direction-cue" x1="{cue_from_x:.3}" y1="{cue_from_y:.3}" x2="{cue_tip_x:.3}" y2="{cue_tip_y:.3}" />
            <path class="arc-arrow" d="M {arrow_lx:.3} {arrow_ly:.3} L {arrow_tx:.3} {arrow_ty:.3} L {arrow_rx:.3} {arrow_ry:.3}" />
            <circle class="arc-endpoint start" cx="{sx:.3}" cy="{sy:.3}" r="7" />
            <circle class="arc-endpoint end" cx="{ex:.3}" cy="{ey:.3}" r="7" />
            <path class="fixed-center-mark" d="M {center_left:.3} {oy:.3} L {center_right:.3} {oy:.3} M {ox:.3} {center_top:.3} L {ox:.3} {center_bottom:.3}" />
            <circle id="drag-handle" class="drag-target{active_class}" cx="{px:.3}" cy="{py:.3}" r="{drag_radius:.0}" data-drag-point="arc-contact" data-model-x="{model_x:.6}" data-model-y="{model_y:.6}" />
            <circle class="point draggable arc-contact-point{active_class}" cx="{px:.3}" cy="{py:.3}" r="8" />
            <line class="span-track" x1="{indicator_start:.3}" y1="390" x2="{indicator_end:.3}" y2="390" />
            <circle class="span-indicator" cx="{indicator_x:.3}" cy="390" r="5" />
            <text class="endpoint-label" x="{start_label_x:.3}" y="{start_label_y:.3}">start / t 0</text>
            <text class="endpoint-label" x="{end_label_x:.3}" y="{end_label_y:.3}">end / t 1</text>
            <text class="contact-label" x="{contact_label_x:.3}" y="{contact_label_y:.3}">contact / t {span_parameter:.3}</text>
            <text class="span-label" x="{indicator_start:.3}" y="378">bounded span / {sweep_degrees:.0} deg CCW</text>
            <text x="28" y="42" class="scene-kicker">LIVE M7 / BOUNDED CURVE CONTACT</text>
            <text x="28" y="68" class="scene-title">Arc contact drag / actual CCW span only</text>
            <text x="28" y="92" class="branch-label">committed span parameter {span_parameter:.6} / radius {arc_radius:.3}</text>
        </g>"#,
        ox = center_svg.x,
        oy = center_svg.y,
        sx = start_svg.x,
        sy = start_svg.y,
        ex = end_svg.x,
        ey = end_svg.y,
        px = point_svg.x,
        py = point_svg.y,
        cue_from_x = cue_from.x,
        cue_from_y = cue_from.y,
        cue_tip_x = cue_tip.x,
        cue_tip_y = cue_tip.y,
        arrow_lx = arrow.left.x,
        arrow_ly = arrow.left.y,
        arrow_tx = arrow.tip.x,
        arrow_ty = arrow.tip.y,
        arrow_rx = arrow.right.x,
        arrow_ry = arrow.right.y,
        center_left = center_svg.x - 8.0,
        center_right = center_svg.x + 8.0,
        center_top = center_svg.y - 8.0,
        center_bottom = center_svg.y + 8.0,
        drag_radius = DRAG_HIT_RADIUS,
        model_x = point.x,
        model_y = point.y,
        start_label_x = start_svg.x + 10.0,
        start_label_y = start_svg.y + 20.0,
        end_label_x = end_svg.x - 82.0,
        end_label_y = end_svg.y + 20.0,
        contact_label_x = point_svg.x + 13.0,
        contact_label_y = point_svg.y - 12.0,
        arc_radius = arc.radius,
    )
    .expect("writing arc contact SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn arc_circle_auto_radius_geometry_markup(
    app: &InteractiveSketchState,
    ids: ArcCircleAutoRadiusIds,
    dragging: bool,
) -> Result<String, String> {
    let arc = *app
        .display
        .geometry
        .arc(ids.arc)
        .ok_or_else(|| "auto-radius result is missing its arc".to_owned())?;
    let circle = *app
        .display
        .geometry
        .circle(ids.circle)
        .ok_or_else(|| "auto-radius result is missing its circle".to_owned())?;
    let arc_center = sketch_geometry_point(
        &app.display.geometry,
        ids.arc_center,
        "auto-radius arc center",
    )?;
    let circle_center = sketch_geometry_point(
        &app.display.geometry,
        ids.circle_center,
        "auto-radius circle center",
    )?;
    let ContactState::CircleArcTangency {
        arc_span_parameter,
        circle_angle,
    } = app
        .sketch
        .contact_state(ids.tangency)
        .map_err(|error| error.to_string())?
    else {
        return Err("auto-radius source has the wrong latent state".to_owned());
    };
    let side = app
        .sketch
        .circle_arc_tangency_side(ids.tangency)
        .map_err(|error| error.to_string())?;
    let SketchConstraintKind::CircleArcTangency {
        circle: source_circle,
        arc: source_arc,
        ..
    } = app
        .sketch
        .constraint(ids.tangency)
        .ok_or_else(|| "auto-radius tangency source is unavailable".to_owned())?
        .kind()
    else {
        return Err("auto-radius source is not circle-arc tangency".to_owned());
    };
    if source_circle != ids.circle
        || source_arc != ids.arc
        || side != ArcCircleTangencySide::OutsideArc
        || (arc.center - arc_center).norm() > 1.0e-9
        || (circle.center - circle_center).norm() > 1.0e-9
    {
        return Err("auto-radius source identity or solved centers changed".to_owned());
    }

    let (arc_start, arc_end) = arc
        .endpoints()
        .ok_or_else(|| "auto-radius arc endpoints are invalid".to_owned())?;
    let arc_contact = arc
        .evaluate(arc_span_parameter)
        .ok_or_else(|| "committed auto-radius arc contact is invalid".to_owned())?;
    let circle_contact = circle
        .evaluate(circle_angle)
        .ok_or_else(|| "committed auto-radius circle contact is invalid".to_owned())?;
    let radius_endpoint = circle
        .evaluate(circle_angle + PI)
        .ok_or_else(|| "auto-radius dimension evaluation failed".to_owned())?;
    if !sketch_geometry_is_finite(&app.display.geometry)
        || (arc_contact - circle_contact).norm() > 1.0e-7
    {
        return Err("auto-radius solved evaluators disagree at contact".to_owned());
    }

    let transform = LiveSceneKind::ArcCircleAutoRadius.transform();
    let arc_center_svg = transform.model_to_svg(arc_center);
    let circle_center_svg = transform.model_to_svg(circle_center);
    let arc_start_svg = transform.model_to_svg(arc_start);
    let arc_end_svg = transform.model_to_svg(arc_end);
    let contact_svg = transform.model_to_svg(arc_contact);
    let radius_endpoint_svg = transform.model_to_svg(radius_endpoint);
    let cue_from = transform.model_to_svg(
        arc.evaluate(0.06)
            .ok_or_else(|| "auto-radius sweep cue is invalid".to_owned())?,
    );
    let cue_tip = transform.model_to_svg(
        arc.evaluate(0.11)
            .ok_or_else(|| "auto-radius sweep cue is invalid".to_owned())?,
    );
    let sweep_arrow = arrow_head(cue_from, cue_tip, 11.0)?;
    let radius_arrow_end = arrow_head(circle_center_svg, radius_endpoint_svg, 10.0)?;
    let radius_arrow_center = arrow_head(radius_endpoint_svg, circle_center_svg, 10.0)?;
    let large_arc = u8::from(arc.signed_sweep.abs() > PI);
    let svg_sweep = u8::from(arc.signed_sweep < 0.0);
    let arc_radius = arc.radius * transform.pixels_per_unit;
    let circle_radius = circle.radius * transform.pixels_per_unit;
    let active_class = if dragging { " active" } else { "" };
    let dimension_label_x = (circle_center_svg.x + radius_endpoint_svg.x) * 0.5;
    let dimension_label_y = (circle_center_svg.y + radius_endpoint_svg.y) * 0.5 - 10.0;
    let scene_title = auto_radius_scene_title(&app.display.core_report);
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry arc-circle-auto-radius-geometry" data-sketch-scene="ArcCircleAutoRadius" data-arc-span-parameter="{arc_span_parameter:.6}" data-circle-angle="{circle_angle:.6}" data-auto-radius="{model_radius:.6}">
            <path class="auto-radius-active-field" d="M {field_x:.3} {field_y:.3} h 292 v 208 h -292 Z" />
            <line class="radius-guide endpoint-guide" x1="{arc_ox:.3}" y1="{arc_oy:.3}" x2="{arc_sx:.3}" y2="{arc_sy:.3}" />
            <line class="radius-guide endpoint-guide" x1="{arc_ox:.3}" y1="{arc_oy:.3}" x2="{arc_ex:.3}" y2="{arc_ey:.3}" />
            <line class="auto-contact-guide arc-radial" x1="{arc_ox:.3}" y1="{arc_oy:.3}" x2="{contact_x:.3}" y2="{contact_y:.3}" />
            <line class="auto-contact-guide circle-radial" x1="{circle_x:.3}" y1="{circle_y:.3}" x2="{contact_x:.3}" y2="{contact_y:.3}" />
            <path class="bounded-arc auto-radius-arc" d="M {arc_sx:.3} {arc_sy:.3} A {arc_radius:.3} {arc_radius:.3} 0 {large_arc} {svg_sweep} {arc_ex:.3} {arc_ey:.3}" />
            <line class="arc-direction-cue" x1="{cue_from_x:.3}" y1="{cue_from_y:.3}" x2="{cue_tip_x:.3}" y2="{cue_tip_y:.3}" />
            <path class="arc-arrow" d="M {sweep_lx:.3} {sweep_ly:.3} L {sweep_tx:.3} {sweep_ty:.3} L {sweep_rx:.3} {sweep_ry:.3}" />
            <circle class="arc-endpoint start" cx="{arc_sx:.3}" cy="{arc_sy:.3}" r="7" />
            <circle class="arc-endpoint end" cx="{arc_ex:.3}" cy="{arc_ey:.3}" r="7" />
            <path class="fixed-center-mark" d="M {arc_center_left:.3} {arc_oy:.3} L {arc_center_right:.3} {arc_oy:.3} M {arc_ox:.3} {arc_center_top:.3} L {arc_ox:.3} {arc_center_bottom:.3}" />
            <circle class="auto-radius-circle" cx="{circle_x:.3}" cy="{circle_y:.3}" r="{circle_radius:.3}" />
            <line class="auto-radius-dimension" x1="{circle_x:.3}" y1="{circle_y:.3}" x2="{radius_x:.3}" y2="{radius_y:.3}" />
            <path class="auto-radius-dimension-arrows" d="M {radius_end_lx:.3} {radius_end_ly:.3} L {radius_end_tx:.3} {radius_end_ty:.3} L {radius_end_rx:.3} {radius_end_ry:.3} M {radius_center_lx:.3} {radius_center_ly:.3} L {radius_center_tx:.3} {radius_center_ty:.3} L {radius_center_rx:.3} {radius_center_ry:.3}" />
            <circle class="contact-marker shared-auto-contact" cx="{contact_x:.3}" cy="{contact_y:.3}" r="7" data-contact-x="{contact_model_x:.6}" data-contact-y="{contact_model_y:.6}" />
            <circle id="drag-handle" class="drag-target{active_class}" cx="{circle_x:.3}" cy="{circle_y:.3}" r="{drag_radius:.0}" data-drag-point="auto-radius-circle-center" data-model-x="{center_model_x:.6}" data-model-y="{center_model_y:.6}" />
            <circle class="point draggable auto-radius-center{active_class}" cx="{circle_x:.3}" cy="{circle_y:.3}" r="9" />
            <text class="endpoint-label" x="{start_label_x:.3}" y="{start_label_y:.3}">start / -150 deg</text>
            <text class="endpoint-label" x="{end_label_x:.3}" y="{end_label_y:.3}">end / +150 deg</text>
            <text class="contact-label auto-contact-label" x="{contact_label_x:.3}" y="{contact_label_y:.3}">shared contact / t {arc_span_parameter:.3}</text>
            <text class="auto-radius-dimension-label" x="{dimension_label_x:.3}" y="{dimension_label_y:.3}">AUTO r={model_radius:.3}</text>
            <text x="28" y="42" class="scene-kicker auto-radius-kicker">LIVE M7 / SOLVED FREE RADIUS</text>
            <text x="28" y="68" class="scene-title">{scene_title}</text>
            <text x="28" y="92" class="branch-label auto-radius-branch">OutsideArc / center ({center_model_x:.3}, {center_model_y:.3}) / AUTO RADIUS r={model_radius:.3}</text>
        </g>"#,
        field_x = arc_center_svg.x - 18.0,
        field_y = arc_center_svg.y - 104.0,
        arc_ox = arc_center_svg.x,
        arc_oy = arc_center_svg.y,
        arc_sx = arc_start_svg.x,
        arc_sy = arc_start_svg.y,
        arc_ex = arc_end_svg.x,
        arc_ey = arc_end_svg.y,
        contact_x = contact_svg.x,
        contact_y = contact_svg.y,
        circle_x = circle_center_svg.x,
        circle_y = circle_center_svg.y,
        cue_from_x = cue_from.x,
        cue_from_y = cue_from.y,
        cue_tip_x = cue_tip.x,
        cue_tip_y = cue_tip.y,
        sweep_lx = sweep_arrow.left.x,
        sweep_ly = sweep_arrow.left.y,
        sweep_tx = sweep_arrow.tip.x,
        sweep_ty = sweep_arrow.tip.y,
        sweep_rx = sweep_arrow.right.x,
        sweep_ry = sweep_arrow.right.y,
        arc_center_left = arc_center_svg.x - 8.0,
        arc_center_right = arc_center_svg.x + 8.0,
        arc_center_top = arc_center_svg.y - 8.0,
        arc_center_bottom = arc_center_svg.y + 8.0,
        radius_x = radius_endpoint_svg.x,
        radius_y = radius_endpoint_svg.y,
        radius_end_lx = radius_arrow_end.left.x,
        radius_end_ly = radius_arrow_end.left.y,
        radius_end_tx = radius_arrow_end.tip.x,
        radius_end_ty = radius_arrow_end.tip.y,
        radius_end_rx = radius_arrow_end.right.x,
        radius_end_ry = radius_arrow_end.right.y,
        radius_center_lx = radius_arrow_center.left.x,
        radius_center_ly = radius_arrow_center.left.y,
        radius_center_tx = radius_arrow_center.tip.x,
        radius_center_ty = radius_arrow_center.tip.y,
        radius_center_rx = radius_arrow_center.right.x,
        radius_center_ry = radius_arrow_center.right.y,
        contact_model_x = arc_contact.x,
        contact_model_y = arc_contact.y,
        drag_radius = DRAG_HIT_RADIUS,
        center_model_x = circle_center.x,
        center_model_y = circle_center.y,
        start_label_x = arc_start_svg.x - 99.0,
        start_label_y = arc_start_svg.y + 20.0,
        end_label_x = arc_end_svg.x - 96.0,
        end_label_y = arc_end_svg.y - 12.0,
        contact_label_x = contact_svg.x - 12.0,
        contact_label_y = contact_svg.y - 14.0,
        dimension_label_x = dimension_label_x,
        dimension_label_y = dimension_label_y,
        model_radius = circle.radius,
        scene_title = escape_html(&scene_title),
    )
    .expect("writing auto-radius SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn tangent_glide_geometry_markup(
    app: &InteractiveSketchState,
    ids: TangentGlideIds,
    dragging: bool,
) -> Result<String, String> {
    let line_start =
        sketch_geometry_point(&app.display.geometry, ids.line_start, "tangent line start")?;
    let line_end = sketch_geometry_point(&app.display.geometry, ids.line_end, "tangent line end")?;
    let circle = *app
        .display
        .geometry
        .circle(ids.circle)
        .ok_or_else(|| "tangent glide result is missing its circle".to_owned())?;
    let ContactState::LineCircleTangency {
        line_parameter,
        circle_angle,
    } = app
        .sketch
        .contact_state(ids.tangency)
        .map_err(|error| error.to_string())?
    else {
        return Err("line-circle source has the wrong latent state".to_owned());
    };
    let SketchConstraintKind::LineCircleTangency {
        line,
        circle: source_circle,
        domain,
        side,
        ..
    } = app
        .sketch
        .constraint(ids.tangency)
        .ok_or_else(|| "line-circle tangency source is unavailable".to_owned())?
        .kind()
    else {
        return Err("source is not line-circle tangency".to_owned());
    };
    if line != ids.line || source_circle != ids.circle {
        return Err("line-circle source identity changed".to_owned());
    }
    let contact = circle
        .evaluate(circle_angle)
        .ok_or_else(|| "committed circle contact evaluation failed".to_owned())?;
    if !sketch_geometry_is_finite(&app.display.geometry) || !domain.contains(line_parameter) {
        return Err("refusing to render invalid tangent glide geometry".to_owned());
    }
    let transform = LiveSceneKind::LineCircleTangentGlide.transform();
    let start_svg = transform.model_to_svg(line_start);
    let end_svg = transform.model_to_svg(line_end);
    let center_svg = transform.model_to_svg(circle.center);
    let contact_svg = transform.model_to_svg(contact);
    let side_tip = SvgPoint {
        x: contact_svg.x + 0.72 * (center_svg.x - contact_svg.x),
        y: contact_svg.y + 0.72 * (center_svg.y - contact_svg.y),
    };
    let side_arrow = arrow_head(contact_svg, side_tip, 12.0)?;
    let radius = circle.radius * transform.pixels_per_unit;
    let active_class = if dragging { " active" } else { "" };
    let indicator_x = start_svg.x + line_parameter * (end_svg.x - start_svg.x);
    let side_label = match side {
        LineSide::Left => "Left",
        LineSide::Right => "Right",
    };
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry tangent-glide-geometry" data-sketch-scene="LineCircleTangentGlide" data-line-parameter="{line_parameter:.6}" data-circle-angle="{circle_angle:.6}">
            <line class="bounded-tangent-line" x1="{sx:.3}" y1="{sy:.3}" x2="{ex:.3}" y2="{ey:.3}" />
            <line class="line-domain-track" x1="{sx:.3}" y1="365" x2="{ex:.3}" y2="365" />
            <circle class="line-parameter-indicator" cx="{indicator_x:.3}" cy="365" r="5" />
            <circle class="line-endpoint start" cx="{sx:.3}" cy="{sy:.3}" r="8" />
            <circle class="line-endpoint end" cx="{ex:.3}" cy="{ey:.3}" r="8" />
            <circle class="gliding-circle" cx="{center_x:.3}" cy="{center_y:.3}" r="{radius:.3}" />
            <line class="radius-normal" x1="{center_x:.3}" y1="{center_y:.3}" x2="{contact_x:.3}" y2="{contact_y:.3}" />
            <line class="tangent-side-arrow" x1="{contact_x:.3}" y1="{contact_y:.3}" x2="{side_tip_x:.3}" y2="{side_tip_y:.3}" />
            <path class="tangent-arrow-head" d="M {arrow_lx:.3} {arrow_ly:.3} L {arrow_tx:.3} {arrow_ty:.3} L {arrow_rx:.3} {arrow_ry:.3}" />
            <circle class="contact-marker" cx="{contact_x:.3}" cy="{contact_y:.3}" r="7" data-contact-x="{contact_model_x:.6}" data-contact-y="{contact_model_y:.6}" />
            <circle id="drag-handle" class="drag-target{active_class}" cx="{center_x:.3}" cy="{center_y:.3}" r="{drag_radius:.0}" data-drag-point="circle-center" data-model-x="{center_model_x:.6}" data-model-y="{center_model_y:.6}" />
            <circle class="point draggable circle-center{active_class}" cx="{center_x:.3}" cy="{center_y:.3}" r="8" />
            <text class="endpoint-label" x="{start_label_x:.3}" y="{start_label_y:.3}">A / t 0</text>
            <text class="endpoint-label" x="{end_label_x:.3}" y="{end_label_y:.3}">B / t 1</text>
            <text class="contact-label" x="{contact_label_x:.3}" y="{contact_label_y:.3}">contact / t {line_parameter:.3}</text>
            <text class="side-label" x="{side_label_x:.3}" y="{side_label_y:.3}">{side_label} side</text>
            <text class="span-label" x="{sx:.3}" y="385">bounded segment parameter / {line_parameter:.3}</text>
            <text x="28" y="42" class="scene-kicker">LIVE M7 / BOUNDED TANGENCY</text>
            <text x="28" y="68" class="scene-title">Line-circle tangent glide / explicit {side_label} side</text>
            <text x="28" y="92" class="branch-label">committed line t {line_parameter:.6} / circle angle {circle_angle:.6}</text>
        </g>"#,
        sx = start_svg.x,
        sy = start_svg.y,
        ex = end_svg.x,
        ey = end_svg.y,
        center_x = center_svg.x,
        center_y = center_svg.y,
        contact_x = contact_svg.x,
        contact_y = contact_svg.y,
        side_tip_x = side_tip.x,
        side_tip_y = side_tip.y,
        arrow_lx = side_arrow.left.x,
        arrow_ly = side_arrow.left.y,
        arrow_tx = side_arrow.tip.x,
        arrow_ty = side_arrow.tip.y,
        arrow_rx = side_arrow.right.x,
        arrow_ry = side_arrow.right.y,
        contact_model_x = contact.x,
        contact_model_y = contact.y,
        drag_radius = DRAG_HIT_RADIUS,
        center_model_x = circle.center.x,
        center_model_y = circle.center.y,
        start_label_x = start_svg.x - 8.0,
        start_label_y = start_svg.y + 43.0,
        end_label_x = end_svg.x - 42.0,
        end_label_y = end_svg.y + 43.0,
        contact_label_x = contact_svg.x + 12.0,
        contact_label_y = contact_svg.y + 23.0,
        side_label_x = side_tip.x + 10.0,
        side_label_y = side_tip.y - 5.0,
    )
    .expect("writing tangent glide SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn triangle_geometry_markup(
    sketch: &Sketch,
    result: &SketchSolveResult,
    ids: UnderconstrainedTriangleIds,
    dragging: bool,
) -> Result<String, String> {
    let a = result
        .geometry
        .point(ids.a)
        .ok_or_else(|| "S1 result is missing point A".to_owned())?;
    let b = result
        .geometry
        .point(ids.b)
        .ok_or_else(|| "S1 result is missing point B".to_owned())?;
    let c = result
        .geometry
        .point(ids.c)
        .ok_or_else(|| "S1 result is missing point C".to_owned())?;
    if [a.x, a.y, b.x, b.y, c.x, c.y]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err("refusing to render non-finite S1 geometry".to_owned());
    }

    let a_svg = MODEL_TRANSFORM.model_to_svg(a);
    let b_svg = MODEL_TRANSFORM.model_to_svg(b);
    let c_svg = MODEL_TRANSFORM.model_to_svg(c);
    let manifold = manifold_markup(sketch, ids, a_svg, c_svg);
    let active_class = if dragging { " active" } else { "" };
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry s1-geometry">
            {}
            <path class="triangle-edge" d="M {:.3} {:.3} L {:.3} {:.3} L {:.3} {:.3} Z" />
            <line class="fixed-mark" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="fixed-mark" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <circle class="point fixed" cx="{:.3}" cy="{:.3}" r="7" />
            <circle class="point free" cx="{:.3}" cy="{:.3}" r="7"
                data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle id="drag-handle" class="drag-target{}" cx="{:.3}" cy="{:.3}" r="{:.0}"
                data-drag-point="C"
                data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="point draggable{}" cx="{:.3}" cy="{:.3}" r="8" />
            <text class="point-label" x="{:.3}" y="{:.3}">A</text>
            <text class="point-role" x="{:.3}" y="{:.3}">fixed</text>
            <text class="point-label" x="{:.3}" y="{:.3}">B</text>
            <text class="point-role" x="{:.3}" y="{:.3}">free</text>
            <text class="point-label" x="{:.3}" y="{:.3}">C</text>
            <text class="point-role" x="{:.3}" y="{:.3}">{}</text>
            <text x="28" y="42" class="scene-kicker">LIVE S1 / SOLVED SKETCH</text>
            <text x="28" y="68" class="scene-title">Underconstrained triangle</text>
        </g>"#,
        manifold,
        a_svg.x,
        a_svg.y,
        b_svg.x,
        b_svg.y,
        c_svg.x,
        c_svg.y,
        a_svg.x - 12.0,
        a_svg.y - 12.0,
        a_svg.x + 12.0,
        a_svg.y + 12.0,
        a_svg.x - 12.0,
        a_svg.y + 12.0,
        a_svg.x + 12.0,
        a_svg.y - 12.0,
        a_svg.x,
        a_svg.y,
        b_svg.x,
        b_svg.y,
        b.x,
        b.y,
        active_class,
        c_svg.x,
        c_svg.y,
        DRAG_HIT_RADIUS,
        c.x,
        c.y,
        active_class,
        c_svg.x,
        c_svg.y,
        a_svg.x + 13.0,
        a_svg.y - 13.0,
        a_svg.x + 13.0,
        a_svg.y + 19.0,
        b_svg.x + 13.0,
        b_svg.y - 13.0,
        b_svg.x + 13.0,
        b_svg.y + 19.0,
        c_svg.x + 14.0,
        c_svg.y - 13.0,
        c_svg.x + 14.0,
        c_svg.y + 19.0,
        if dragging { "dragging" } else { "drag me" },
    )
    .expect("writing SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn horizontal_rail_geometry_markup(
    result: &SketchSolveResult,
    ids: HorizontalRailIds,
    dragging: bool,
) -> Result<String, String> {
    let a = result
        .geometry
        .point(ids.a)
        .ok_or_else(|| "rail result is missing point A".to_owned())?;
    let b = result
        .geometry
        .point(ids.b)
        .ok_or_else(|| "rail result is missing point B".to_owned())?;
    let reference_length = reference_dimension_value(result, ids.reference_length)
        .ok_or_else(|| "rail result is missing its reference length".to_owned())?;
    if [a.x, a.y, b.x, b.y, reference_length]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err("refusing to render non-finite rail geometry".to_owned());
    }

    let a_svg = MODEL_TRANSFORM.model_to_svg(a);
    let b_svg = MODEL_TRANSFORM.model_to_svg(b);
    let dimension_y = a_svg.y + 48.0;
    let dimension_midpoint = (a_svg.x + b_svg.x) * 0.5;
    let active_class = if dragging { " active" } else { "" };
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry rail-geometry">
            <line class="hard-manifold rail-cue" x1="55" y1="{:.3}" x2="595" y2="{:.3}" />
            <line class="motion-cue rail-motion" x1="75" y1="{:.3}" x2="565" y2="{:.3}" />
            <path class="rail-arrow" d="M 75 {:.3} l 12 -7 M 75 {:.3} l 12 7 M 565 {:.3} l -12 -7 M 565 {:.3} l -12 7" />
            <line class="rail-edge" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="reference-extension" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="reference-extension" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="reference-dimension" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <text class="reference-label" x="{:.3}" y="{:.3}">reference length {:.3}</text>
            <line class="fixed-mark" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <line class="fixed-mark" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <circle class="point fixed" cx="{:.3}" cy="{:.3}" r="7" />
            <circle id="drag-handle" class="drag-target{}" cx="{:.3}" cy="{:.3}" r="{:.0}"
                data-drag-point="B" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="point draggable{}" cx="{:.3}" cy="{:.3}" r="8" />
            <text class="point-label" x="{:.3}" y="{:.3}">A</text>
            <text class="point-role" x="{:.3}" y="{:.3}">fixed</text>
            <text class="point-label" x="{:.3}" y="{:.3}">B</text>
            <text class="point-role" x="{:.3}" y="{:.3}">{}</text>
            <text x="28" y="42" class="scene-kicker">LIVE VERIFICATION / SOLVED SKETCH</text>
            <text x="28" y="68" class="scene-title">Horizontal rail / 1 continuous DOF</text>
            <text x="58" y="{:.3}" class="manifold-label">horizontal hard rail</text>
        </g>"#,
        a_svg.y,
        a_svg.y,
        a_svg.y,
        a_svg.y,
        a_svg.y,
        a_svg.y,
        a_svg.y,
        a_svg.y,
        a_svg.x,
        a_svg.y,
        b_svg.x,
        b_svg.y,
        a_svg.x,
        a_svg.y + 10.0,
        a_svg.x,
        dimension_y + 7.0,
        b_svg.x,
        b_svg.y + 10.0,
        b_svg.x,
        dimension_y + 7.0,
        a_svg.x,
        dimension_y,
        b_svg.x,
        dimension_y,
        dimension_midpoint,
        dimension_y - 8.0,
        reference_length,
        a_svg.x - 12.0,
        a_svg.y - 12.0,
        a_svg.x + 12.0,
        a_svg.y + 12.0,
        a_svg.x - 12.0,
        a_svg.y + 12.0,
        a_svg.x + 12.0,
        a_svg.y - 12.0,
        a_svg.x,
        a_svg.y,
        active_class,
        b_svg.x,
        b_svg.y,
        DRAG_HIT_RADIUS,
        b.x,
        b.y,
        active_class,
        b_svg.x,
        b_svg.y,
        a_svg.x + 13.0,
        a_svg.y - 13.0,
        a_svg.x + 13.0,
        a_svg.y + 19.0,
        b_svg.x + 14.0,
        b_svg.y - 13.0,
        b_svg.x + 14.0,
        b_svg.y + 19.0,
        if dragging { "dragging" } else { "drag me" },
        a_svg.y - 14.0,
    )
    .expect("writing rail SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
fn coincident_pair_geometry_markup(
    result: &SketchSolveResult,
    ids: CoincidentPairIds,
    dragging: bool,
) -> Result<String, String> {
    let a = result
        .geometry
        .point(ids.a)
        .ok_or_else(|| "coincident result is missing point A".to_owned())?;
    let b = result
        .geometry
        .point(ids.b)
        .ok_or_else(|| "coincident result is missing point B".to_owned())?;
    if [a.x, a.y, b.x, b.y].iter().any(|value| !value.is_finite()) {
        return Err("refusing to render non-finite coincident geometry".to_owned());
    }

    let a_svg = MODEL_TRANSFORM.model_to_svg(a);
    let b_svg = MODEL_TRANSFORM.model_to_svg(b);
    let active_class = if dragging { " active" } else { "" };
    let mut svg = String::new();
    write!(
        svg,
        r#"<g class="geometry coincident-geometry">
            <circle class="coincident-cue" cx="{:.3}" cy="{:.3}" r="38" />
            <path class="coincident-leader" d="M {:.3} {:.3} L {:.3} {:.3} M {:.3} {:.3} L {:.3} {:.3}" />
            <circle id="drag-handle" class="drag-target{}" cx="{:.3}" cy="{:.3}" r="{:.0}"
                data-drag-point="B" data-model-x="{:.6}" data-model-y="{:.6}" />
            <circle class="point coincident-point-a" cx="{:.3}" cy="{:.3}" r="12" />
            <circle class="point draggable coincident-point-b{}" cx="{:.3}" cy="{:.3}" r="6" />
            <text class="point-label coincident-label-a" x="{:.3}" y="{:.3}">A</text>
            <text class="point-role" x="{:.3}" y="{:.3}">outer mark</text>
            <text class="point-label coincident-label-b" x="{:.3}" y="{:.3}">B</text>
            <text class="point-role" x="{:.3}" y="{:.3}">{}</text>
            <text x="28" y="42" class="scene-kicker">LIVE VERIFICATION / SOLVED SKETCH</text>
            <text x="28" y="68" class="scene-title">Coincident pair / 2 translational DOF</text>
            <text x="28" y="94" class="manifold-label">concentric marks show A and B at one solved point</text>
        </g>"#,
        (a_svg.x + b_svg.x) * 0.5,
        (a_svg.y + b_svg.y) * 0.5,
        a_svg.x - 11.0,
        a_svg.y - 9.0,
        a_svg.x - 30.0,
        a_svg.y - 27.0,
        b_svg.x + 7.0,
        b_svg.y + 7.0,
        b_svg.x + 30.0,
        b_svg.y + 27.0,
        active_class,
        b_svg.x,
        b_svg.y,
        DRAG_HIT_RADIUS,
        b.x,
        b.y,
        a_svg.x,
        a_svg.y,
        active_class,
        b_svg.x,
        b_svg.y,
        a_svg.x - 43.0,
        a_svg.y - 31.0,
        a_svg.x - 78.0,
        a_svg.y - 13.0,
        b_svg.x + 33.0,
        b_svg.y + 35.0,
        b_svg.x + 33.0,
        b_svg.y + 51.0,
        if dragging { "dragging inner mark" } else { "drag inner mark" },
    )
    .expect("writing coincident SVG markup to a String cannot fail");
    Ok(svg)
}

#[cfg(any(target_arch = "wasm32", test))]
fn reference_dimension_value(
    result: &SketchSolveResult,
    dimension: SketchDimensionId,
) -> Option<f64> {
    result.reference_values.iter().find_map(|measurement| {
        (measurement.dimension_id == dimension).then_some(measurement.value)
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn manifold_markup(
    sketch: &Sketch,
    ids: UnderconstrainedTriangleIds,
    a: SvgPoint,
    c: SvgPoint,
) -> String {
    let Some(radius) = distance_ac_target(sketch, ids) else {
        return String::new();
    };
    let radial_x = c.x - a.x;
    let radial_y = c.y - a.y;
    let radial_norm = radial_x.hypot(radial_y);
    if !radial_norm.is_finite() || radial_norm <= f64::EPSILON {
        return String::new();
    }
    let tangent_x = -radial_y * 40.0 / radial_norm;
    let tangent_y = radial_x * 40.0 / radial_norm;
    format!(
        r#"<circle class="hard-manifold" cx="{:.3}" cy="{:.3}" r="{:.3}" />
            <line class="motion-cue" x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" />
            <text x="{:.3}" y="{:.3}" class="manifold-label">r = {:.3} hard manifold</text>"#,
        a.x,
        a.y,
        MODEL_TRANSFORM.pixels_per_unit * radius,
        c.x - tangent_x,
        c.y - tangent_y,
        c.x + tangent_x,
        c.y + tangent_y,
        a.x - 2.0,
        a.y - MODEL_TRANSFORM.pixels_per_unit * radius - 10.0,
        radius,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn distance_ac_target(sketch: &Sketch, ids: UnderconstrainedTriangleIds) -> Option<f64> {
    let dimension = sketch.dimension(ids.distance_ac)?;
    let DimensionKind::PointDistance {
        first,
        second,
        target,
    } = dimension.kind()
    else {
        return None;
    };
    let is_ac = (first == ids.a && second == ids.c) || (first == ids.c && second == ids.a);
    (is_ac && target.is_finite() && target > 0.0).then_some(target)
}

#[cfg(any(target_arch = "wasm32", test))]
fn audit_markup(audit: &AuditSnapshot, driver_source_ids: &[SourceConstraintId]) -> String {
    let mut html = String::new();
    for source in &audit.sources {
        let category = source
            .rows
            .first()
            .map_or("empty", |row| category_label(row.category));
        let is_driver = driver_source_ids.contains(&source.source_id);
        let driver_class = if is_driver { " driver-source" } else { "" };
        let kind_class = if is_driver { "driver" } else { category };
        let kind_label = if is_driver { "driver / hard" } else { category };
        write!(
            html,
            r#"<article class="constraint source-group {}{}" data-source-id="{}" data-linkage-driver="{}">
                <header class="source-header">
                    <div><span class="source-id">{}</span><h3>{}</h3></div>
                    <span class="kind {}">{}</span>
                </header>
                <div class="source-diagnostics"><span>source diagnostics</span>{}</div>"#,
            category,
            driver_class,
            escape_html(&format!("{:?}", source.source_id)),
            is_driver,
            escape_html(&format!("{:?}", source.source_id)),
            escape_html(&source.source_label),
            kind_class,
            kind_label,
            annotations_markup(source.annotations),
        )
        .expect("writing audit source markup to a String cannot fail");

        for (row_index, row) in source.rows.iter().enumerate() {
            let row_category = category_label(row.category);
            write!(
                html,
                r#"<section class="audit-row" data-category="{}">
                    <div class="row-heading">
                        <span>row {}.{}</span>
                        <span class="kind {}">{}</span>
                    </div>
                    <code class="row-template">{}</code>
                    <dl class="row-facts">
                        <dt>bindings</dt><dd>{}</dd>
                        <dt>incident values</dt><dd>{}</dd>
                        <dt>unit</dt><dd>{}</dd>
                        <dt>scale</dt><dd>{}</dd>
                        <dt>raw residual</dt><dd>{}</dd>
                        <dt>normalized</dt><dd>{}</dd>
                        <dt>evaluation</dt><dd>{}</dd>
                        <dt>row diagnostics</dt><dd>{}</dd>
                    </dl>
                </section>"#,
                row_category,
                row_index + 1,
                row.row_in_block + 1,
                row_category,
                row_category,
                escape_html(&row.template),
                binding_markup(&row.bindings),
                incident_values_markup(&row.incident_variables),
                escape_html(&row.unit),
                format_metric(row.scale),
                format_metric(row.raw_residual),
                format_metric(row.normalized_residual),
                evaluation_markup(row.evaluation_status, row.evaluation_error.as_deref()),
                annotations_markup(row.annotations),
            )
            .expect("writing audit row markup to a String cannot fail");
        }
        html.push_str("</article>");
    }
    if audit.sources.is_empty() {
        html.push_str(
            r#"<p class="audit-note">The retained display state has no executable audit rows.</p>"#,
        );
    }
    html
}

#[cfg(any(target_arch = "wasm32", test))]
fn reference_measurements_markup(result: &SketchSolveResult) -> String {
    let mut html = String::new();
    for measurement in &result.reference_values {
        let label = result
            .source_mappings
            .iter()
            .find_map(|mapping| {
                (mapping.source == SketchSource::Dimension(measurement.dimension_id))
                    .then_some(mapping.source_label.as_str())
            })
            .unwrap_or("reference dimension");
        write!(
            html,
            r#"<article class="constraint reference-measurement" data-reference-dimension="{:?}">
                <header class="source-header">
                    <div><span class="source-id">{:?}</span><h3>{}</h3></div>
                    <span class="kind reference">reference</span>
                </header>
                <dl class="row-facts reference-facts">
                    <dt>equation rows</dt><dd>none; display-only measurement</dd>
                    <dt>evaluated value</dt><dd>{}</dd>
                </dl>
            </article>"#,
            measurement.dimension_id,
            measurement.dimension_id,
            escape_html(label),
            format_metric(measurement.value),
        )
        .expect("writing reference measurement markup to a String cannot fail");
    }
    html
}

#[cfg(any(target_arch = "wasm32", test))]
fn binding_markup(bindings: &[geosolve_core::AuditBinding]) -> String {
    if bindings.is_empty() {
        return "<span class=\"muted\">none</span>".to_owned();
    }
    let mut html = String::new();
    for binding in bindings {
        write!(
            html,
            "<span class=\"binding\"><b>{}</b> = {}</span>",
            escape_html(&binding.name),
            escape_html(&binding.value),
        )
        .expect("writing binding markup to a String cannot fail");
    }
    html
}

#[cfg(any(target_arch = "wasm32", test))]
fn incident_values_markup(values: &[geosolve_core::AuditVariableSnapshot]) -> String {
    if values.is_empty() {
        return "<span class=\"muted\">none</span>".to_owned();
    }
    let mut html = String::new();
    for variable in values {
        write!(
            html,
            "<span class=\"binding\"><b>{}</b> = {}</span>",
            escape_html(&format!("{:?}", variable.variable_id)),
            escape_html(&variable_value_text(variable.value)),
        )
        .expect("writing incident value markup to a String cannot fail");
    }
    html
}

#[cfg(any(target_arch = "wasm32", test))]
fn variable_value_text(value: VariableValue) -> String {
    match value {
        VariableValue::Scalar(value) => format_value(value),
        VariableValue::Vec2([x, y]) => format!("[{}, {}]", format_value(x), format_value(y)),
        VariableValue::Pose2([x, y, angle]) => format!(
            "[{}, {}, {}]",
            format_value(x),
            format_value(y),
            format_value(angle)
        ),
        VariableValue::Vec3([x, y, z]) => format!(
            "[{}, {}, {}]",
            format_value(x),
            format_value(y),
            format_value(z)
        ),
        VariableValue::Pose3([x, y, z, qw, qx, qy, qz]) => format!(
            "[{}, {}, {}, {}, {}, {}, {}]",
            format_value(x),
            format_value(y),
            format_value(z),
            format_value(qw),
            format_value(qx),
            format_value(qy),
            format_value(qz)
        ),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn evaluation_markup(status: AuditEvaluationStatus, error: Option<&str>) -> String {
    let (class, label) = match status {
        AuditEvaluationStatus::Evaluated => ("evaluated", "evaluated"),
        AuditEvaluationStatus::Failed => ("failed", "failed"),
        _ => ("failed", "unknown"),
    };
    match error {
        Some(error) => format!(
            "<span class=\"evaluation {}\">{}</span><span class=\"evaluation-error\">{}</span>",
            class,
            label,
            escape_html(error)
        ),
        None => format!("<span class=\"evaluation {class}\">{label}</span>"),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn annotations_markup(annotations: AuditAnnotations) -> String {
    let mut labels = Vec::new();
    if annotations.eliminated {
        labels.push("eliminated");
    }
    if annotations.suppressed {
        labels.push("suppressed");
    }
    if annotations.redundant {
        labels.push("redundant");
    }
    if annotations.conflicting {
        labels.push("conflicting");
    }
    if annotations.singular {
        labels.push("singular");
    }
    if annotations.active_bound {
        labels.push("active-bound");
    }
    if labels.is_empty() {
        return "<span class=\"annotation none\">none</span>".to_owned();
    }
    let mut html = String::new();
    for label in labels {
        write!(html, "<span class=\"annotation\">{label}</span>")
            .expect("writing annotation markup to a String cannot fail");
    }
    html
}

#[cfg(any(target_arch = "wasm32", test))]
fn status_markup(
    sketch: &Sketch,
    scene: LiveScene,
    display: &SketchSolveResult,
    retained: &RetainedDiagnostics,
    attempt: &AttemptSummary,
) -> String {
    let attempt_banner = attempt_markup(attempt);
    let validated_residual = retained
        .validated_hard_residual_max
        .map_or_else(|| "unavailable".to_owned(), format_metric);
    let rank = retained
        .rank
        .map_or_else(|| "unavailable".to_owned(), |rank| rank.to_string());
    let dof = retained
        .local_degrees_of_freedom
        .map_or_else(|| "unavailable".to_owned(), |dof| dof.to_string());
    let bounded_dof = retained
        .bounded_bidirectional_degrees_of_freedom
        .map_or_else(|| "unavailable".to_owned(), |dof| dof.to_string());
    let one_sided = retained.one_sided_mobility.map_or_else(
        || "unavailable".to_owned(),
        |mobility| format!("{mobility:?}"),
    );
    let (motion_label, motion_state) = scene_motion_state(sketch, scene);
    let conflicts = diagnostic_notice(&retained.conflict_sources, retained.conflict_diagnostics);
    let redundancies = diagnostic_notice(
        &retained.redundancy_sources,
        retained.redundancy_diagnostics,
    );
    let bounds = text_notice(&retained.bounds);
    let conflict_status = diagnostic_status(retained.conflict_diagnostics);
    let redundancy_status = diagnostic_status(retained.redundancy_diagnostics);
    let singularity =
        retained.is_singular.map_or(
            "unavailable",
            |is_singular| {
                if is_singular { "yes" } else { "none" }
            },
        );
    let curve_status = curve_status_markup(sketch, scene, display, attempt);
    let (early_curve_status, late_curve_status) =
        if matches!(scene, LiveScene::ArcCircleAutoRadius(_)) {
            (curve_status.as_str(), "")
        } else {
            ("", curve_status.as_str())
        };
    let mut html = String::new();
    write!(
        html,
        r#"{}{}
            <div class="status-grid">
                <div><span>retained termination</span><strong>{}</strong></div>
                <div><span>retained validated max hard residual</span><strong>{}</strong></div>
                <div><span>retained rank</span><strong>{}</strong></div>
                <div><span>equality right nullity</span><strong>{}</strong></div>
                <div><span>bounded bidirectional DOF</span><strong>{}</strong></div>
                <div><span>one-sided mobility</span><strong>{}</strong></div>
                <div><span>retained total iterations</span><strong>{}</strong></div>
                <div><span>{}</span><strong>{}</strong></div>
                <div><span>retained singularity</span><strong>{}</strong></div>
                <div><span>retained conflict candidates</span><strong>{}</strong></div>
                <div><span>conflict diagnostic</span><strong>{}</strong></div>
                <div><span>retained redundancy notices</span><strong>{}</strong></div>
                <div><span>redundancy diagnostic</span><strong>{}</strong></div>
                <div><span>bound states</span><strong>{}</strong></div>
            </div>{}{}"#,
        attempt_banner,
        early_curve_status,
        termination_label(retained.termination),
        validated_residual,
        rank,
        dof,
        bounded_dof,
        one_sided,
        retained.iterations,
        motion_label,
        escape_html(&motion_state),
        singularity,
        conflicts,
        conflict_status,
        redundancies,
        redundancy_status,
        bounds,
        reference_status_markup(scene, display),
        late_curve_status,
    )
    .expect("writing solve status markup to a String cannot fail");
    html
}

#[cfg(any(target_arch = "wasm32", test))]
fn linkage_status_markup(app: &InteractiveLinkageState) -> Result<String, String> {
    let retained = &app.retained_diagnostics;
    let attempt = linkage_attempt_markup(&app.attempt, app.continuation.as_ref());
    let validated_residual = retained
        .validated_hard_residual_max
        .map_or_else(|| "unavailable".to_owned(), format_metric);
    let rank = retained
        .rank
        .map_or_else(|| "unavailable".to_owned(), |rank| rank.to_string());
    let dof = retained
        .local_degrees_of_freedom
        .map_or_else(|| "unavailable".to_owned(), |dof| dof.to_string());
    let singularity = retained
        .is_singular
        .map_or("unavailable", |value| if value { "yes" } else { "none" });
    let ratio = retained
        .solve_diagnostics
        .singular_value_ratio
        .map_or_else(|| "unavailable".to_owned(), format_metric);
    let branch = app
        .scene
        .branch_evaluation(&app.linkage, &app.display.geometry)?;
    let metric = branch.signed_metric;
    let continuation_samples = app
        .continuation
        .as_ref()
        .map_or(0, |summary| summary.samples.len());
    let accepted_samples = app.continuation.as_ref().map_or(0, |summary| {
        summary
            .samples
            .iter()
            .filter(|sample| sample.accepted)
            .count()
    });
    let total_iterations = app
        .continuation
        .as_ref()
        .map_or(retained.iterations, |summary| summary.total_iterations);
    let mut html = String::new();
    write!(
        html,
        r#"{}
            <div class="status-grid linkage-status-grid">
                <div><span>retained termination</span><strong>{}</strong></div>
                <div><span>independently validated max hard residual</span><strong>{}</strong></div>
                <div><span>retained rank / local DOF</span><strong>{} / {}</strong></div>
                <div><span>latest request total iterations</span><strong>{}</strong></div>
                <div><span>continuation samples / accepted</span><strong>{} / {}</strong></div>
                <div><span>explicit assembly mode</span><strong>{}</strong></div>
                <div><span>expected branch sign</span><strong>{}</strong></div>
                <div><span>retained branch sign / metric</span><strong>{} / {}</strong></div>
                <div><span>branch monitor kind / ID</span><strong>{:?} / {:?}</strong></div>
                <div><span>domain branch retained</span><strong>{}</strong></div>
                <div><span>retained driver target</span><strong>{:.3} deg</strong></div>
                <div><span>retained singularity</span><strong>{}</strong></div>
                <div><span>rank warning</span><strong>{}</strong></div>
                <div><span>smallest/largest singular value ratio</span><strong>{}</strong></div>
                <div><span>unit angular-rate velocity residual</span><strong>{}</strong></div>
                <div><span>velocity rank / local DOF</span><strong>{} / {}</strong></div>
                <div><span>retained conflict candidates</span><strong>{}</strong></div>
                <div><span>retained redundancy notices</span><strong>{}</strong></div>
            </div>"#,
        attempt,
        termination_label(retained.termination),
        validated_residual,
        rank,
        dof,
        total_iterations,
        continuation_samples,
        accepted_samples,
        app.scene.mode_label(),
        branch_sign_label(branch.expected_sign),
        metric_sign_label(metric),
        format_metric(metric),
        branch.kind,
        branch.monitor_id,
        if branch.retained { "yes" } else { "no" },
        app.driver_degrees()?,
        singularity,
        if retained.solve_diagnostics.has_rank_warning {
            "yes"
        } else {
            "none"
        },
        ratio,
        format_metric(app.velocity.differentiated_residual_max),
        app.velocity.rank,
        app.velocity.local_degrees_of_freedom,
        diagnostic_notice(&retained.conflict_sources, retained.conflict_diagnostics),
        diagnostic_notice(
            &retained.redundancy_sources,
            retained.redundancy_diagnostics,
        ),
    )
    .expect("writing linkage status markup to a String cannot fail");
    Ok(html)
}

#[cfg(any(target_arch = "wasm32", test))]
fn linkage_attempt_markup(
    attempt: &LinkageAttemptSummary,
    continuation: Option<&ContinuationSummary>,
) -> String {
    let (class, label, detail) = match attempt {
        LinkageAttemptSummary::Accepted { termination } => {
            let detail = continuation.map_or_else(
                || {
                    format!(
                        "initial position solve termination: {}; retained state and unit-rate velocity validated",
                        termination_label(*termination)
                    )
                },
                |summary| {
                    format!(
                        "requested {:.3} deg; accepted {:.3} deg in {} samples; termination: {}",
                        radians_to_degrees(summary.requested_target),
                        radians_to_degrees(summary.accepted_target),
                        summary.samples.len(),
                        termination_label(*termination),
                    )
                },
            );
            ("accepted", "attempt accepted", detail)
        }
        LinkageAttemptSummary::Rejected {
            termination,
            rejection,
        } => {
            let prefix = continuation.map_or_else(
                || "continuation failed".to_owned(),
                |summary| {
                    format!(
                        "requested {:.3} deg; retained {:.3} deg after {}/{} accepted samples",
                        radians_to_degrees(summary.requested_target),
                        radians_to_degrees(summary.accepted_target),
                        summary
                            .samples
                            .iter()
                            .filter(|sample| sample.accepted)
                            .count(),
                        summary.samples.len(),
                    )
                },
            );
            (
                "rejected",
                "attempt rejected / retained state shown",
                format!(
                    "{prefix}; attempt termination: {}; rejection: {rejection}",
                    termination_label(*termination)
                ),
            )
        }
        LinkageAttemptSummary::VelocityValidationFailed {
            termination,
            position_target,
            retained_target,
            message,
        } => (
            "rejected",
            "position accepted / velocity failed / rolled back",
            format!(
                "position termination: {}; attempted accepted target {:.3} deg; velocity validation failed: {}; rolled back to retained target {:.3} deg",
                termination_label(*termination),
                radians_to_degrees(*position_target),
                message,
                radians_to_degrees(*retained_target),
            ),
        ),
        LinkageAttemptSummary::Error { message } => (
            "rejected",
            "attempt error / retained state shown",
            format!("API error: {message}; no candidate state was displayed"),
        ),
    };
    format!(
        r#"<div class="attempt-banner {class}">
                <span class="attempt-light"></span>
                <strong>{}</strong>
                <span>{}</span>
            </div>"#,
        escape_html(label),
        escape_html(&detail),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn linkage_announcement(
    attempt: &LinkageAttemptSummary,
    continuation: Option<&ContinuationSummary>,
) -> String {
    match attempt {
        LinkageAttemptSummary::Accepted { .. } => {
            "Linkage accepted. Position and unit-rate velocity validated.".to_owned()
        }
        LinkageAttemptSummary::Rejected { .. }
            if continuation.is_some_and(|summary| {
                summary.samples.iter().any(|sample| sample.accepted)
            }) =>
        {
            "Linkage stopped early. The latest position and velocity validated together are displayed."
                .to_owned()
        }
        LinkageAttemptSummary::Rejected { .. } => {
            "Linkage rejected. The retained state remains displayed.".to_owned()
        }
        LinkageAttemptSummary::VelocityValidationFailed { .. } => {
            "Position accepted, but velocity validation failed. The prior linkage state was restored."
                .to_owned()
        }
        LinkageAttemptSummary::Error { .. } => {
            "Linkage request failed. The prior state remains displayed.".to_owned()
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn branch_sign_label(sign: BranchSign) -> &'static str {
    match sign {
        BranchSign::Positive => "positive",
        BranchSign::Negative => "negative",
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn scene_motion_state(sketch: &Sketch, scene: LiveScene) -> (&'static str, String) {
    match scene {
        LiveScene::UnderconstrainedTriangle(ids) => {
            ("retained AB branch", triangle_branch_label(sketch, ids))
        }
        LiveScene::HorizontalRail(ids) => {
            let state = match sketch.segment_has_enforced_branch(ids.ab) {
                Ok(false) => "continuous horizontal motion; no discrete branch",
                Ok(true) => "discrete segment branch is unexpectedly enforced",
                Err(_) => "motion state unavailable",
            };
            ("retained motion state", state.to_owned())
        }
        LiveScene::CoincidentPair(_) => (
            "retained branch state",
            "no discrete branch; common point translates in 2D".to_owned(),
        ),
        LiveScene::TangentCircles(ids) => {
            let state = sketch.circle_tangency_mode(ids.tangency).map_or_else(
                |_| "unavailable".to_owned(),
                |mode| {
                    format!(
                        "{}; positive-x center direction",
                        circle_tangency_mode_label(mode)
                    )
                },
            );
            ("retained tangency branch", state)
        }
        LiveScene::ArcContactDrag(ids) => {
            let state = match sketch.contact_state(ids.contact) {
                Ok(ContactState::PointOnArc { span_parameter }) => {
                    format!("bounded CCW span; committed t = {span_parameter:.6}")
                }
                _ => "bounded arc contact unavailable".to_owned(),
            };
            ("retained contact branch", state)
        }
        LiveScene::ArcCircleAutoRadius(ids) => {
            let state = match (
                sketch.circle_arc_tangency_side(ids.tangency),
                sketch.contact_state(ids.tangency),
            ) {
                (
                    Ok(side),
                    Ok(ContactState::CircleArcTangency {
                        arc_span_parameter, ..
                    }),
                ) => format!("{side:?}; bounded arc t = {arc_span_parameter:.6}"),
                _ => "circle-arc tangency state unavailable".to_owned(),
            };
            ("retained tangency branch", state)
        }
        LiveScene::LineCircleTangentGlide(ids) => {
            let state = match sketch.contact_state(ids.tangency) {
                Ok(ContactState::LineCircleTangency { line_parameter, .. }) => {
                    format!("Left side; bounded segment t = {line_parameter:.6}")
                }
                _ => "bounded line-circle contact unavailable".to_owned(),
            };
            ("retained tangency branch", state)
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(clippy::too_many_lines)]
fn curve_status_markup(
    sketch: &Sketch,
    scene: LiveScene,
    display: &SketchSolveResult,
    attempt: &AttemptSummary,
) -> String {
    match scene {
        LiveScene::TangentCircles(ids) => {
            let mode = sketch
                .circle_tangency_mode(ids.tangency)
                .map_or("unavailable", circle_tangency_mode_label);
            let first = display.geometry.circle(ids.circle_a);
            let second = display.geometry.circle(ids.circle_b);
            let center_distance = first.zip(second).map_or_else(
                || "unavailable".to_owned(),
                |(first, second)| format_metric((second.center - first.center).norm()),
            );
            let branch = sketch
                .constraint(ids.tangency)
                .and_then(|constraint| {
                    let SketchConstraintKind::CircleCircleTangency {
                        center_direction, ..
                    } = constraint.kind()
                    else {
                        return None;
                    };
                    let (first, second) = first.zip(second)?;
                    let cosine = center_direction.direction_cosine(first.center, second.center)?;
                    Some(format!("positive-x / cosine {cosine:.6}"))
                })
                .unwrap_or_else(|| "unavailable".to_owned());
            format!(
                r#"<div class="status-grid curve-status-grid">
                    <div><span>explicit circle tangency mode</span><strong>{}</strong></div>
                    <div><span>retained center contact distance</span><strong>{}</strong></div>
                    <div><span>retained center-direction branch</span><strong>{}</strong></div>
                    <div><span>retained audit snapshot</span><strong>{}</strong></div>
                </div>"#,
                escape_html(mode),
                center_distance,
                escape_html(&branch),
                audit_snapshot_status(&display.display_audit),
            )
        }
        LiveScene::ArcContactDrag(ids) => {
            let span = match sketch.contact_state(ids.contact) {
                Ok(ContactState::PointOnArc { span_parameter }) => {
                    format!("{span_parameter:.6} / [0, 1]")
                }
                _ => "unavailable".to_owned(),
            };
            let sweep = display.geometry.arc(ids.arc).map_or_else(
                || "unavailable".to_owned(),
                |arc| {
                    format!(
                        "{:.3} deg / {:?}",
                        arc.signed_sweep.to_degrees().abs(),
                        arc.sweep
                    )
                },
            );
            format!(
                r#"<div class="status-grid curve-status-grid">
                    <div><span>committed arc span parameter</span><strong>{}</strong></div>
                    <div><span>explicit bounded sweep</span><strong>{}</strong></div>
                    <div><span>bounded escape guard</span><strong>{}</strong></div>
                    <div><span>retained audit snapshot</span><strong>{}</strong></div>
                </div>"#,
                escape_html(&span),
                escape_html(&sweep),
                bounded_attempt_status(attempt, "arc span"),
                audit_snapshot_status(&display.display_audit),
            )
        }
        LiveScene::ArcCircleAutoRadius(ids) => {
            let circle = display.geometry.circle(ids.circle);
            let center = circle.map_or_else(
                || "unavailable".to_owned(),
                |circle| format!("({:.3}, {:.3})", circle.center.x, circle.center.y),
            );
            let radius = circle.map_or_else(
                || "unavailable".to_owned(),
                |circle| format!("AUTO RADIUS r={:.3}", circle.radius),
            );
            let side = sketch
                .circle_arc_tangency_side(ids.tangency)
                .map_or_else(|_| "unavailable".to_owned(), |side| format!("{side:?}"));
            let (span, circle_angle) = match sketch.contact_state(ids.tangency) {
                Ok(ContactState::CircleArcTangency {
                    arc_span_parameter,
                    circle_angle,
                }) => (
                    format!("committed arc span t={arc_span_parameter:.6}"),
                    format!("{circle_angle:.6} rad"),
                ),
                _ => ("unavailable".to_owned(), "unavailable".to_owned()),
            };
            let dof = report_local_dof_label(&display.core_report);
            format!(
                r#"<div class="auto-radius-hud">
                    <div class="auto-radius-primary"><span>local mobility</span><strong>{}</strong></div>
                    <div class="auto-radius-primary"><span>solved circle</span><strong>{}</strong></div>
                    <div><span>current center (x, y)</span><strong>{}</strong></div>
                    <div><span>explicit tangency side</span><strong>{}</strong></div>
                    <div><span>accepted contact state</span><strong>{}</strong></div>
                    <div><span>circle contact angle</span><strong>{}</strong></div>
                    <div class="auto-radius-retention"><span>latest request / retention</span><strong>{}</strong></div>
                    <div><span>retained audit snapshot</span><strong>{}</strong></div>
                    <p>Center has x/y freedom; radius and contact variables are solved.</p>
                </div>"#,
                escape_html(&dof),
                escape_html(&radius),
                escape_html(&center),
                escape_html(&side),
                escape_html(&span),
                escape_html(&circle_angle),
                auto_radius_attempt_status(attempt),
                audit_snapshot_status(&display.display_audit),
            )
        }
        LiveScene::LineCircleTangentGlide(ids) => {
            let (parameter, angle) = match sketch.contact_state(ids.tangency) {
                Ok(ContactState::LineCircleTangency {
                    line_parameter,
                    circle_angle,
                }) => (
                    format!("{line_parameter:.6} / [0, 1]"),
                    format!("{circle_angle:.6} rad"),
                ),
                _ => ("unavailable".to_owned(), "unavailable".to_owned()),
            };
            let source_state = sketch
                .constraint(ids.tangency)
                .and_then(|constraint| {
                    let SketchConstraintKind::LineCircleTangency { domain, side, .. } =
                        constraint.kind()
                    else {
                        return None;
                    };
                    Some(format!("{side:?} / {}", domain.label()))
                })
                .unwrap_or_else(|| "unavailable".to_owned());
            format!(
                r#"<div class="status-grid curve-status-grid">
                    <div><span>committed line contact parameter</span><strong>{}</strong></div>
                    <div><span>committed circle contact angle</span><strong>{}</strong></div>
                    <div><span>explicit side / domain</span><strong>{}</strong></div>
                    <div><span>endpoint escape guard</span><strong>{}</strong></div>
                    <div><span>retained audit snapshot</span><strong>{}</strong></div>
                </div>"#,
                escape_html(&parameter),
                escape_html(&angle),
                escape_html(&source_state),
                bounded_attempt_status(attempt, "segment endpoint"),
                audit_snapshot_status(&display.display_audit),
            )
        }
        LiveScene::UnderconstrainedTriangle(_)
        | LiveScene::HorizontalRail(_)
        | LiveScene::CoincidentPair(_) => String::new(),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn auto_radius_attempt_status(attempt: &AttemptSummary) -> String {
    match attempt {
        AttemptSummary::Accepted { .. } => {
            "ACCEPTED / center, radius, contact, and audit committed".to_owned()
        }
        AttemptSummary::Rejected { rejection, .. } => format!(
            "REJECTED / {}; prior center/radius/contact/audit retained",
            escape_html(&sketch_rejection_summary(rejection))
        ),
        AttemptSummary::Error { message } => format!(
            "REQUEST ERROR / {}; prior center/radius/contact/audit retained",
            escape_html(message)
        ),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn report_local_dof_label(report: &SolveReport) -> String {
    if report.rank_is_valid {
        format!("{} local DOF", report.local_degrees_of_freedom)
    } else {
        "DOF unavailable".to_owned()
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn auto_radius_scene_title(report: &SolveReport) -> String {
    format!(
        "Arc-circle auto radius / {}",
        report_local_dof_label(report)
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn bounded_attempt_status(attempt: &AttemptSummary, boundary: &str) -> String {
    match attempt {
        AttemptSummary::Accepted { .. } => "accepted contact is inside bounds".to_owned(),
        AttemptSummary::Rejected { rejection, .. } => {
            if matches!(rejection, SolveRejection::ContactParameterOutOfDomain(_)) {
                format!("{boundary} escape rejected; prior geometry/contact retained")
            } else {
                format!(
                    "{}; prior geometry/contact retained",
                    escape_html(&sketch_rejection_summary(rejection))
                )
            }
        }
        AttemptSummary::Error { message } => format!(
            "request error; prior geometry/contact retained ({})",
            escape_html(message)
        ),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn sketch_rejection_summary(rejection: &SolveRejection) -> String {
    match rejection {
        SolveRejection::CoreTermination(termination) => format!(
            "core solve terminated as {}",
            termination_label(*termination)
        ),
        SolveRejection::HardResidual { maximum, tolerance } => format!(
            "hard residual validation rejected maximum {} above tolerance {}",
            format_metric(*maximum),
            format_metric(*tolerance)
        ),
        SolveRejection::IndependentValidationFailed(message) => {
            format!("independent validation rejected the candidate: {message}")
        }
        SolveRejection::SegmentBranchFlipped(_) => {
            "explicit segment branch rejected the candidate".to_owned()
        }
        SolveRejection::NonPositiveCircleRadius(_) => {
            "nonpositive circle radius rejected the candidate".to_owned()
        }
        SolveRejection::NonPositiveArcRadius(_) => {
            "nonpositive arc radius rejected the candidate".to_owned()
        }
        SolveRejection::DegenerateSegment(_) => {
            "degenerate segment rejected the candidate".to_owned()
        }
        SolveRejection::ContactParameterOutOfDomain(_) => {
            "contact parameter left its bounded domain".to_owned()
        }
        SolveRejection::LineSideFlipped(_) => {
            "explicit line-side branch rejected the candidate".to_owned()
        }
        SolveRejection::InvalidTangencyMode(_) => {
            "explicit tangency mode rejected the candidate".to_owned()
        }
        SolveRejection::AmbiguousTangencyScale(_) => {
            "tangency feature scales are numerically ambiguous".to_owned()
        }
        SolveRejection::CenterDirectionFlipped(_) => {
            "explicit center-direction branch rejected the candidate".to_owned()
        }
        _ => "unknown pre-1.0 sketch rejection".to_owned(),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn audit_snapshot_status(audit: &AuditSnapshot) -> String {
    let rows: Vec<_> = audit
        .sources
        .iter()
        .flat_map(|source| &source.rows)
        .collect();
    let evaluated = rows
        .iter()
        .filter(|row| row.evaluation_status == AuditEvaluationStatus::Evaluated)
        .count();
    format!(
        "{evaluated}/{} retained domain/core rows evaluated",
        rows.len()
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn reference_status_markup(scene: LiveScene, display: &SketchSolveResult) -> String {
    let LiveScene::HorizontalRail(ids) = scene else {
        return String::new();
    };
    let value = reference_dimension_value(display, ids.reference_length)
        .map_or_else(|| "unavailable".to_owned(), format_metric);
    format!(
        r#"<div class="reference-status"><span>retained equation-free reference length</span><strong>{}</strong></div>"#,
        escape_html(&value),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn attempt_markup(attempt: &AttemptSummary) -> String {
    let (class, label, detail) = match attempt {
        AttemptSummary::Accepted { termination } => (
            "accepted",
            "attempt accepted",
            format!(
                "attempt termination: {}; retained state updated",
                termination_label(*termination)
            ),
        ),
        AttemptSummary::Rejected {
            termination,
            rejection,
        } => (
            "rejected",
            "attempt rejected / retained state shown",
            format!(
                "attempt termination: {}; {}; prior geometry/audit remains displayed",
                termination_label(*termination),
                sketch_rejection_summary(rejection)
            ),
        ),
        AttemptSummary::Error { message } => (
            "rejected",
            "attempt error / retained state shown",
            format!("API error: {message}; no candidate report available"),
        ),
    };
    format!(
        r#"<div class="attempt-banner {class}">
                <span class="attempt-light"></span>
                <strong>{}</strong>
                <span>{}</span>
            </div>"#,
        escape_html(label),
        escape_html(&detail),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn sketch_announcement(attempt: &AttemptSummary) -> String {
    match attempt {
        AttemptSummary::Accepted { .. } => "Sketch solve accepted.".to_owned(),
        AttemptSummary::Rejected { .. } => {
            "Sketch solve rejected. The retained geometry remains displayed.".to_owned()
        }
        AttemptSummary::Error { .. } => {
            "Sketch request failed. The retained geometry remains displayed.".to_owned()
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn triangle_branch_label(sketch: &Sketch, ids: UnderconstrainedTriangleIds) -> String {
    let direction = sketch
        .segment(ids.ab)
        .map_or([f64::NAN, f64::NAN], |segment| {
            segment.branch().reference_direction()
        });
    let selected = if direction[0].is_finite() && direction[0] >= 0.0 {
        "rightward (+x)"
    } else if direction[0].is_finite() {
        "leftward (-x)"
    } else {
        "unavailable"
    };
    let preservation = match sketch.segment_branch_is_preserved(ids.ab) {
        Ok(true) => "preserved",
        Ok(false) => "not preserved",
        Err(_) => "unavailable",
    };
    format!("{selected}; {preservation}")
}

#[cfg(any(target_arch = "wasm32", test))]
fn source_labels(ids: &[SourceConstraintId], audit: &AuditSnapshot) -> Vec<String> {
    ids.iter()
        .map(|id| {
            audit
                .sources
                .iter()
                .find(|source| source.source_id == *id)
                .map_or_else(|| format!("{id:?}"), |source| source.source_label.clone())
        })
        .collect()
}

#[cfg(any(target_arch = "wasm32", test))]
fn text_notice(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values
            .iter()
            .map(|value| escape_html(value))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn diagnostic_status(diagnostic: DiagnosticCompleteness) -> String {
    diagnostic.reason.map_or_else(
        || format!("{:?}", diagnostic.status),
        |reason| format!("{:?} / {reason:?}", diagnostic.status),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn diagnostic_notice(values: &[String], diagnostic: DiagnosticCompleteness) -> String {
    if !values.is_empty() {
        return text_notice(values);
    }
    if diagnostic.status == DiagnosticStatus::Complete {
        "none".to_owned()
    } else {
        format!("not reported ({})", diagnostic_status(diagnostic))
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn audit_hard_residual_max(audit: &AuditSnapshot) -> Option<f64> {
    let mut maximum = 0.0_f64;
    let mut has_hard_row = false;
    for row in audit.sources.iter().flat_map(|source| &source.rows) {
        if row.category != ResidualCategory::Hard {
            continue;
        }
        has_hard_row = true;
        if row.evaluation_status != AuditEvaluationStatus::Evaluated
            || !row.normalized_residual.is_finite()
        {
            return None;
        }
        maximum = maximum.max(row.normalized_residual.abs());
    }
    has_hard_row.then_some(maximum)
}

#[cfg(any(target_arch = "wasm32", test))]
const fn category_label(category: ResidualCategory) -> &'static str {
    match category {
        ResidualCategory::Hard => "hard",
        ResidualCategory::Temporary => "temporary",
        ResidualCategory::Preference => "preference",
    }
}

#[cfg(any(target_arch = "wasm32", test))]
const fn termination_label(termination: SolveTermination) -> &'static str {
    match termination {
        SolveTermination::Converged => "converged",
        SolveTermination::Stalled => "stalled",
        SolveTermination::IterationLimit => "iteration limit",
        SolveTermination::InvalidGeometry => "invalid geometry",
        SolveTermination::NumericalFailure => "numerical failure",
        _ => "unknown termination",
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn format_metric(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.3e}")
    } else {
        "unavailable".to_owned()
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn format_value(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.6}")
    } else {
        "unavailable".to_owned()
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::{cell::RefCell, rc::Rc};

    use super::{
        ClientRect, ConflictingRectangleState, DemoApp, DemoScenario, DemoState,
        InteractiveLinkageState, InteractiveSketchState, LinkageAttemptSummary, LiveSceneKind,
        SceneActionView, SvgPoint, client_to_drag_target, expected_conflict_view,
        live_linkage_view, live_sketch_view, pointer_start_allowed,
    };
    use geosolve_geometry::Point2;
    use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
    use web_sys::{
        Document, Element, Event, HtmlInputElement, HtmlOutputElement, HtmlSelectElement,
        PointerEvent,
    };

    fn required_element(document: &Document, id: &str) -> Result<Element, JsValue> {
        document
            .get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str(&format!("missing #{id} element")))
    }

    fn render(document: &Document, app: &DemoApp) -> Result<(), JsValue> {
        let viewport = required_element(document, "viewport")?;
        let equations = required_element(document, "equations")?;
        let status = required_element(document, "solve-status")?;
        let announcement = required_element(document, "solve-announcement")?;
        let badge = required_element(document, "audit-badge")?;
        let instructions = required_element(document, "drag-instructions")?;
        let controls = required_element(document, "driver-controls")?;
        let action_controls = required_element(document, "scene-action-controls")?;
        let action_button = required_element(document, "scene-action")?;
        let action_help = required_element(document, "scene-action-help")?;
        let driver = required_element(document, "driver-angle")?.dyn_into::<HtmlInputElement>()?;
        let output =
            required_element(document, "driver-output")?.dyn_into::<HtmlOutputElement>()?;
        let state_action = app
            .state
            .action_label()
            .map_err(|error| JsValue::from_str(&error))?
            .map(|label| SceneActionView {
                label,
                help: "Changes explicit sketch branch state, solves, and publishes only an accepted result.",
            });
        if app.state.drag_active() {
            viewport.set_attribute("data-drag-active", "true")?;
        } else {
            viewport.remove_attribute("data-drag-active")?;
        }

        match &app.state {
            DemoState::Sketch(state) => {
                let view = live_sketch_view(state).map_err(|error| JsValue::from_str(&error))?;
                viewport.set_inner_html(&view.geometry);
                equations.set_inner_html(&view.audit);
                status.set_inner_html(&view.status);
                announcement.set_text_content(Some(&view.announcement));
                badge.set_text_content(Some(view.badge));
                badge.set_class_name("live-badge");
                instructions.set_text_content(Some(view.instructions));
                controls.set_attribute("hidden", "")?;
                driver.set_disabled(true);
                render_scene_action(&action_controls, &action_button, &action_help, state_action)?;
            }
            DemoState::ExpectedConflict(state) => {
                let view =
                    expected_conflict_view(state).map_err(|error| JsValue::from_str(&error))?;
                viewport.set_inner_html(&view.geometry);
                equations.set_inner_html(&view.audit);
                status.set_inner_html(&view.status);
                announcement.set_text_content(Some(&view.announcement));
                badge.set_text_content(Some(view.badge));
                badge.set_class_name("live-badge expected-conflict");
                instructions.set_text_content(Some(view.instructions));
                controls.set_attribute("hidden", "")?;
                driver.set_disabled(true);
                render_scene_action(&action_controls, &action_button, &action_help, None)?;
            }
            DemoState::Linkage(state) => {
                let view = live_linkage_view(state).map_err(|error| JsValue::from_str(&error))?;
                viewport.set_inner_html(&view.geometry);
                equations.set_inner_html(&view.audit);
                status.set_inner_html(&view.status);
                announcement.set_text_content(Some(&view.announcement));
                badge.set_text_content(Some(view.badge));
                badge.set_class_name("live-badge linkage");
                instructions.set_text_content(Some(view.instructions));
                controls.remove_attribute("hidden")?;
                driver.set_disabled(false);
                driver.set_min(&view.driver_control.min.to_string());
                driver.set_max(&view.driver_control.max.to_string());
                driver.set_step(&view.driver_control.step.to_string());
                driver.set_value(&format!("{:.0}", view.driver_control.value));
                driver.set_attribute(
                    "aria-valuetext",
                    &format!("{:.0} degrees", view.driver_control.value),
                )?;
                output.set_value(&format!("{:.0} deg", view.driver_control.value));
                render_scene_action(&action_controls, &action_button, &action_help, None)?;
            }
        }
        Ok(())
    }

    fn render_scene_action(
        controls: &Element,
        button: &Element,
        help: &Element,
        action: Option<SceneActionView>,
    ) -> Result<(), JsValue> {
        if let Some(action) = action {
            controls.remove_attribute("hidden")?;
            button.remove_attribute("disabled")?;
            button.set_text_content(Some(action.label));
            help.set_text_content(Some(action.help));
        } else {
            controls.set_attribute("hidden", "")?;
            button.set_attribute("disabled", "")?;
            button.set_text_content(Some("Scene action unavailable"));
            help.set_text_content(None);
        }
        Ok(())
    }

    fn render_shared(document: &Document, app: &Rc<RefCell<DemoApp>>) {
        if let Err(error) = render(document, &app.borrow()) {
            if let Some(status) = document.get_element_by_id("solve-status") {
                status.set_text_content(Some(&format!("Rendering error: {error:?}")));
            }
        }
    }

    fn pointer_model_position(
        event: &PointerEvent,
        viewport: &Element,
        kind: LiveSceneKind,
    ) -> Option<Point2<f64>> {
        let bounds = viewport.get_bounding_client_rect();
        client_to_drag_target(
            kind,
            SvgPoint {
                x: f64::from(event.client_x()),
                y: f64::from(event.client_y()),
            },
            ClientRect {
                left: bounds.left(),
                top: bounds.top(),
                width: bounds.width(),
                height: bounds.height(),
            },
        )
    }

    fn build_demo_state(scenario: DemoScenario) -> Result<DemoState, String> {
        if scenario.is_expected_conflict() {
            ConflictingRectangleState::new()
                .map(|state| DemoState::ExpectedConflict(Box::new(state)))
        } else if let Some(kind) = scenario.sketch_scene_kind() {
            InteractiveSketchState::new(kind).map(|state| DemoState::Sketch(Box::new(state)))
        } else if let Some(kind) = scenario.linkage_scene_kind() {
            InteractiveLinkageState::new(kind).map(|state| DemoState::Linkage(Box::new(state)))
        } else {
            Err("scenario has no domain state".to_owned())
        }
    }

    fn set_state_error(state: &mut DemoState, message: String) {
        match state {
            DemoState::Sketch(state) => state.attempt = super::AttemptSummary::Error { message },
            DemoState::ExpectedConflict(state) => state.scene_error = Some(message),
            DemoState::Linkage(state) => {
                state.attempt = LinkageAttemptSummary::Error { message };
                state.continuation = None;
            }
        }
    }

    fn install_scenario_listener(
        document: &Document,
        select: &HtmlSelectElement,
        viewport: &Element,
        app: &Rc<RefCell<DemoApp>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_select = select.clone();
        let callback_viewport = viewport.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            let mut state = callback_app.borrow_mut();
            if let DemoState::Sketch(sketch) = &mut state.state
                && let Some(pointer_id) = sketch.active_pointer
            {
                let _ = callback_viewport.release_pointer_capture(pointer_id);
                sketch.finish_drag();
            }
            let next = DemoScenario::from_value(&callback_select.value());
            match build_demo_state(next) {
                Ok(next_state) => {
                    state.state = next_state;
                }
                Err(message) => {
                    set_state_error(&mut state.state, message);
                    callback_select.set_value(state.state.selector_value());
                }
            }
            drop(state);
            render_shared(&callback_document, &callback_app);
        });
        select.add_event_listener_with_callback("change", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_driver_listener(
        document: &Document,
        driver: &HtmlInputElement,
        app: &Rc<RefCell<DemoApp>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_driver = driver.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            let target_degrees = callback_driver.value_as_number();
            let mut app = callback_app.borrow_mut();
            let DemoState::Linkage(linkage) = &mut app.state else {
                return;
            };
            linkage.drive_to_degrees(target_degrees);
            drop(app);
            render_shared(&callback_document, &callback_app);
        });
        driver.add_event_listener_with_callback("input", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_scene_action_listener(
        document: &Document,
        button: &Element,
        app: &Rc<RefCell<DemoApp>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            event.prevent_default();
            let mut app = callback_app.borrow_mut();
            if !app.state.has_action() {
                return;
            }
            app.state.trigger_action();
            drop(app);
            render_shared(&callback_document, &callback_app);
        });
        button.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_pointer_listeners(
        document: &Document,
        viewport: &Element,
        app: &Rc<RefCell<DemoApp>>,
    ) -> Result<(), JsValue> {
        install_pointer_down(document, viewport, app)?;
        install_pointer_move(document, viewport, app)?;
        install_pointer_end(document, viewport, app, "pointerup")?;
        install_pointer_end(document, viewport, app, "pointercancel")?;
        Ok(())
    }

    fn install_pointer_down(
        document: &Document,
        viewport: &Element,
        app: &Rc<RefCell<DemoApp>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_viewport = viewport.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let drag_active = {
                let state = callback_app.borrow();
                let DemoState::Sketch(sketch) = &state.state else {
                    return;
                };
                sketch.active_pointer.is_some()
            };
            let is_drag_handle = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .is_some_and(|target| {
                    target.id() == "drag-handle" && target.has_attribute("data-drag-point")
                });
            if !is_drag_handle
                || !pointer_start_allowed(
                    event.is_primary(),
                    &event.pointer_type(),
                    event.button(),
                    drag_active,
                )
            {
                return;
            }
            event.prevent_default();
            if callback_viewport
                .set_pointer_capture(event.pointer_id())
                .is_err()
            {
                return;
            }
            if let DemoState::Sketch(sketch) = &mut callback_app.borrow_mut().state {
                sketch.active_pointer = Some(event.pointer_id());
            }
            render_shared(&callback_document, &callback_app);
        });
        viewport
            .add_event_listener_with_callback("pointerdown", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_pointer_move(
        document: &Document,
        viewport: &Element,
        app: &Rc<RefCell<DemoApp>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_viewport = viewport.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let kind = {
                let state = callback_app.borrow();
                let DemoState::Sketch(sketch) = &state.state else {
                    return;
                };
                (sketch.active_pointer == Some(event.pointer_id())).then_some(sketch.scene.kind())
            };
            let Some(kind) = kind else {
                return;
            };
            event.prevent_default();
            let Some(target) = pointer_model_position(&event, &callback_viewport, kind) else {
                return;
            };
            if let DemoState::Sketch(sketch) = &mut callback_app.borrow_mut().state {
                sketch.solve_drag(target);
            }
            render_shared(&callback_document, &callback_app);
        });
        viewport
            .add_event_listener_with_callback("pointermove", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_pointer_end(
        document: &Document,
        viewport: &Element,
        app: &Rc<RefCell<DemoApp>>,
        event_name: &str,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_viewport = viewport.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let active = {
                let state = callback_app.borrow();
                matches!(
                    &state.state,
                    DemoState::Sketch(sketch)
                        if sketch.active_pointer == Some(event.pointer_id())
                )
            };
            if !active {
                return;
            }
            event.prevent_default();
            let _ = callback_viewport.release_pointer_capture(event.pointer_id());
            if let DemoState::Sketch(sketch) = &mut callback_app.borrow_mut().state {
                sketch.finish_drag();
            }
            render_shared(&callback_document, &callback_app);
        });
        viewport.add_event_listener_with_callback(event_name, callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| JsValue::from_str("browser document is unavailable"))?;
        crate::playground::wasm::install(&document)
    }
}

const SCENARIO_NAMES: [&str; 11] = [
    "S1 underconstrained triangle",
    "S2 conflicting rectangle",
    "S3 tangent circles",
    "Arc contact drag",
    "M7 / Arc-circle auto radius",
    "Line-circle tangent glide",
    "Horizontal rail",
    "Coincident pair",
    "L1 four-bar open",
    "L2 four-bar crossed",
    "L3 slider-crank",
];

/// Browser-facing names in selector order.
#[must_use]
pub const fn scenario_names() -> &'static [&'static str] {
    &SCENARIO_NAMES
}

/// Number of scenarios selectable in the browser harness.
#[must_use]
pub const fn scenario_count() -> usize {
    SCENARIO_NAMES.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_row<'a>(
        audit: &'a AuditSnapshot,
        source_label: &str,
    ) -> &'a geosolve_core::AuditRowSnapshot {
        &audit
            .sources
            .iter()
            .find(|source| source.source_label.contains(source_label))
            .unwrap_or_else(|| panic!("missing source containing {source_label:?}"))
            .rows[0]
    }

    fn triangle_ids(app: &InteractiveSketchState) -> UnderconstrainedTriangleIds {
        let LiveScene::UnderconstrainedTriangle(ids) = app.scene else {
            panic!("expected S1 state");
        };
        ids
    }

    fn rail_ids(app: &InteractiveSketchState) -> HorizontalRailIds {
        let LiveScene::HorizontalRail(ids) = app.scene else {
            panic!("expected rail state");
        };
        ids
    }

    fn coincident_ids(app: &InteractiveSketchState) -> CoincidentPairIds {
        let LiveScene::CoincidentPair(ids) = app.scene else {
            panic!("expected coincident state");
        };
        ids
    }

    fn tangent_circle_ids(app: &InteractiveSketchState) -> TangentCirclesIds {
        let LiveScene::TangentCircles(ids) = app.scene else {
            panic!("expected S3 state");
        };
        ids
    }

    fn arc_contact_ids(app: &InteractiveSketchState) -> ArcContactIds {
        let LiveScene::ArcContactDrag(ids) = app.scene else {
            panic!("expected arc contact state");
        };
        ids
    }

    fn arc_circle_auto_radius_ids(app: &InteractiveSketchState) -> ArcCircleAutoRadiusIds {
        let LiveScene::ArcCircleAutoRadius(ids) = app.scene else {
            panic!("expected arc-circle auto-radius state");
        };
        ids
    }

    fn tangent_glide_ids(app: &InteractiveSketchState) -> TangentGlideIds {
        let LiveScene::LineCircleTangentGlide(ids) = app.scene else {
            panic!("expected tangent glide state");
        };
        ids
    }

    fn assert_live_result(result: &SketchSolveResult, expected_dof: usize) {
        assert!(result.accepted(), "rejected: {:?}", result.rejection);
        assert_eq!(result.core_report.termination, SolveTermination::Converged);
        assert_eq!(result.core_report.local_degrees_of_freedom, expected_dof);
        assert!(
            result.acceptance_hard_residual_max.unwrap() <= 1.0e-9,
            "hard residual was {:?}",
            result.acceptance_hard_residual_max
        );
    }

    fn assert_point_near(actual: Point2<f64>, expected: Point2<f64>) {
        assert!(
            (actual - expected).norm() <= 1.0e-9,
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn assert_point_within(actual: Point2<f64>, expected: Point2<f64>, tolerance: f64) {
        assert!(
            (actual - expected).norm() <= tolerance,
            "expected {expected:?} within {tolerance}, got {actual:?}"
        );
    }

    fn display_audit_row_count(result: &SketchSolveResult) -> usize {
        audit_row_count(&result.display_audit)
    }

    fn audit_row_count(audit: &AuditSnapshot) -> usize {
        audit.sources.iter().map(|source| source.rows.len()).sum()
    }

    fn svg_scene_title(markup: &str) -> &str {
        let prefix = r#"<text x="28" y="68" class="scene-title">"#;
        markup
            .split_once(prefix)
            .and_then(|(_, remainder)| remainder.split_once("</text>"))
            .map(|(title, _)| title)
            .expect("rendered SVG must contain one scene title")
    }

    fn assert_drag_handle_inside_margin(app: &InteractiveSketchState) {
        let point = app
            .display
            .geometry
            .point(
                app.scene
                    .draggable_point()
                    .expect("live drag test scene must expose a point"),
            )
            .unwrap();
        let svg = MODEL_TRANSFORM.model_to_svg(point);
        let tolerance = 1.0e-6;
        assert!(
            svg.x - DRAG_HIT_RADIUS >= SVG_VIEW_BOX.min_x - tolerance,
            "left edge outside viewBox: {svg:?}"
        );
        assert!(
            svg.x + DRAG_HIT_RADIUS <= SVG_VIEW_BOX.min_x + SVG_VIEW_BOX.width + tolerance,
            "right edge outside viewBox: {svg:?}"
        );
        assert!(
            svg.y - DRAG_HIT_RADIUS >= SVG_VIEW_BOX.min_y - tolerance,
            "top edge outside viewBox: {svg:?}"
        );
        assert!(
            svg.y + DRAG_HIT_RADIUS <= SVG_VIEW_BOX.min_y + SVG_VIEW_BOX.height + tolerance,
            "bottom edge outside viewBox: {svg:?}"
        );
    }

    #[test]
    fn live_s1_view_comes_from_an_accepted_sketch_result() {
        let app = InteractiveSketchState::new(LiveSceneKind::UnderconstrainedTriangle).unwrap();
        assert!(app.display.accepted());
        let view = live_sketch_view(&app).unwrap();
        assert!(view.geometry.contains("LIVE S1 / SOLVED SKETCH"));
        assert!(view.geometry.contains("data-model-x="));
        assert!(view.audit.contains("data-category=\"hard\""));
        assert!(view.audit.contains("data-category=\"preference\""));
        assert!(view.audit.contains("raw residual"));
        assert!(view.audit.contains("normalized"));
        assert!(view.audit.contains("evaluated"));
        assert!(
            view.status
                .contains("equality right nullity</span><strong>1")
        );
        assert!(view.status.contains("bounded bidirectional DOF"));
        assert!(view.status.contains("one-sided mobility"));
        assert!(view.status.contains("conflict diagnostic"));
        assert!(view.status.contains("redundancy diagnostic"));
        assert!(view.status.contains("bound states"));
        assert!(view.status.contains("rightward (+x); preserved"));
        assert_eq!(view.announcement, "Sketch solve accepted.");
    }

    #[test]
    fn s1_has_no_static_audit_or_handwritten_equation_templates() {
        let source = include_str!("lib.rs");
        let old_horizontal = ["B.y", " - A.y"].concat();
        let old_distance = ["B - A", "|| - 4"].concat();
        assert!(!source.contains(&old_horizontal));
        assert!(!source.contains(&old_distance));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn all_eleven_selectors_and_names_map_to_fresh_domain_scene_kinds() {
        assert_eq!(scenario_count(), 11);
        assert_eq!(
            scenario_names(),
            [
                "S1 underconstrained triangle",
                "S2 conflicting rectangle",
                "S3 tangent circles",
                "Arc contact drag",
                "M7 / Arc-circle auto radius",
                "Line-circle tangent glide",
                "Horizontal rail",
                "Coincident pair",
                "L1 four-bar open",
                "L2 four-bar crossed",
                "L3 slider-crank",
            ]
        );
        for scenario in [
            DemoScenario::UnderconstrainedTriangle,
            DemoScenario::ConflictingRectangle,
            DemoScenario::TangentCircles,
            DemoScenario::ArcContactDrag,
            DemoScenario::ArcCircleAutoRadius,
            DemoScenario::LineCircleTangentGlide,
            DemoScenario::HorizontalRail,
            DemoScenario::CoincidentPair,
            DemoScenario::FourBarOpen,
            DemoScenario::FourBarCrossed,
            DemoScenario::SliderCrank,
        ] {
            assert_eq!(
                DemoScenario::from_value(scenario.selector_value()),
                scenario
            );
        }
        assert_eq!(
            DemoScenario::from_value("slider-crank"),
            DemoScenario::SliderCrank
        );
        assert_eq!(
            DemoScenario::from_value("horizontal-rail"),
            DemoScenario::HorizontalRail
        );
        assert_eq!(
            DemoScenario::from_value("coincident-pair"),
            DemoScenario::CoincidentPair
        );
        assert_eq!(
            DemoScenario::from_value("tangent-circles"),
            DemoScenario::TangentCircles
        );
        assert_eq!(
            DemoScenario::from_value("arc-contact-drag"),
            DemoScenario::ArcContactDrag
        );
        assert_eq!(
            DemoScenario::from_value("line-circle-tangent-glide"),
            DemoScenario::LineCircleTangentGlide
        );
        assert_eq!(
            DemoScenario::from_value("arc-circle-auto-radius"),
            DemoScenario::ArcCircleAutoRadius
        );
        assert_eq!(
            DemoScenario::HorizontalRail.sketch_scene_kind(),
            Some(LiveSceneKind::HorizontalRail)
        );
        assert_eq!(
            DemoScenario::from_value("four-bar-open").linkage_scene_kind(),
            Some(LinkageSceneKind::FourBarOpen)
        );
        assert_eq!(
            DemoScenario::from_value("four-bar-crossed").linkage_scene_kind(),
            Some(LinkageSceneKind::FourBarCrossed)
        );
        assert_eq!(DemoScenario::SliderCrank.sketch_scene_kind(), None);
        assert!(DemoScenario::ConflictingRectangle.is_expected_conflict());
        assert_eq!(
            DemoScenario::from_value("conflicting-rectangle"),
            DemoScenario::ConflictingRectangle
        );
        let page = include_str!("../index.html");
        assert!(page.contains(
            "<option value=\"conflicting-rectangle\">S2 / Conflicting rectangle</option>"
        ));
        assert!(page.contains("value=\"horizontal-rail\""));
        assert!(page.contains("value=\"coincident-pair\""));
        assert!(page.contains("value=\"tangent-circles\""));
        assert!(page.contains("value=\"arc-contact-drag\""));
        assert!(page.contains(
            "<option value=\"arc-circle-auto-radius\">M7 / Arc-circle auto radius</option>"
        ));
        assert!(page.contains("value=\"line-circle-tangent-glide\""));
        assert!(page.contains("value=\"four-bar-open\""));
        assert!(page.contains("value=\"four-bar-crossed\""));
        assert!(page.contains("value=\"slider-crank\""));
        let scenario_options = page
            .split_once("<select id=\"scenario\">")
            .unwrap()
            .1
            .split_once("</select>")
            .unwrap()
            .0;
        assert_eq!(
            scenario_options.matches("<option value=").count(),
            scenario_count()
        );
        let arc_contact_index = page.find("value=\"arc-contact-drag\"").unwrap();
        let auto_radius_index = page.find("value=\"arc-circle-auto-radius\"").unwrap();
        let tangent_glide_index = page.find("value=\"line-circle-tangent-glide\"").unwrap();
        assert!(arc_contact_index < auto_radius_index);
        assert!(auto_radius_index < tangent_glide_index);

        let sketch_state = DemoState::Sketch(Box::new(
            InteractiveSketchState::new(LiveSceneKind::HorizontalRail).unwrap(),
        ));
        let linkage_state = DemoState::Linkage(Box::new(
            InteractiveLinkageState::new(LinkageSceneKind::FourBarCrossed).unwrap(),
        ));
        let conflict_state =
            DemoState::ExpectedConflict(Box::new(ConflictingRectangleState::new().unwrap()));
        assert_eq!(sketch_state.selector_value(), "horizontal-rail");
        assert_eq!(conflict_state.selector_value(), "conflicting-rectangle");
        assert_eq!(linkage_state.selector_value(), "four-bar-crossed");
        let auto_radius_state = DemoState::Sketch(Box::new(
            InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap(),
        ));
        assert_eq!(auto_radius_state.selector_value(), "arc-circle-auto-radius");
        let DemoState::ExpectedConflict(conflict) = conflict_state else {
            panic!("expected retained S2 state");
        };
        assert_eq!(
            conflict.conflicts[0].source,
            SketchSource::Dimension(conflict.ids.width_4)
        );
    }

    #[test]
    fn s3_action_switches_explicit_modes_on_the_positive_branch_transactionally() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::TangentCircles).unwrap();
        let ids = tangent_circle_ids(&app);
        assert_live_result(&app.display, 0);
        assert_eq!(
            app.sketch.circle_tangency_mode(ids.tangency).unwrap(),
            CircleTangencyMode::External
        );
        assert_point_within(
            app.display.geometry.point(ids.center_b).unwrap(),
            Point2::new(3.0, 0.0),
            3.0e-9,
        );
        assert_eq!(app.action_label().unwrap(), Some("Switch to internal"));

        let external_view = live_sketch_view(&app).unwrap();
        assert!(!external_view.geometry.is_empty());
        assert!(!external_view.audit.is_empty());
        assert!(external_view.geometry.contains("tangent-circles-geometry"));
        assert!(external_view.geometry.contains("External"));
        assert!(external_view.geometry.contains("center distance 3.000"));
        assert!(external_view.geometry.contains("contact-marker"));
        assert!(
            external_view
                .status
                .contains("explicit circle tangency mode")
        );
        assert!(
            external_view
                .status
                .contains("positive-x / cosine 1.000000")
        );
        assert!(external_view.status.contains("retained audit snapshot"));
        assert_eq!(
            external_view.action,
            Some(SceneActionView {
                label: "Switch to internal",
                help: "Changes explicit sketch branch state, solves, and publishes only an accepted result.",
            })
        );

        app.trigger_action();
        assert_live_result(&app.display, 0);
        assert_eq!(
            app.sketch.circle_tangency_mode(ids.tangency).unwrap(),
            CircleTangencyMode::Internal {
                containment: CircleContainment::FirstContainsSecond
            }
        );
        assert_point_within(
            app.display.geometry.point(ids.center_b).unwrap(),
            Point2::new(1.0, 0.0),
            3.0e-9,
        );
        assert_eq!(app.action_label().unwrap(), Some("Switch to external"));
        let internal_view = live_sketch_view(&app).unwrap();
        assert!(internal_view.geometry.contains("Internal / A contains B"));
        assert!(internal_view.geometry.contains("center distance 1.000"));
        assert!(internal_view.status.contains("Internal / A contains B"));
        let SketchConstraintKind::CircleCircleTangency {
            center_direction, ..
        } = app.sketch.constraint(ids.tangency).unwrap().kind()
        else {
            panic!("expected S3 tangency source");
        };
        let first = app.display.geometry.point(ids.center_a).unwrap();
        let second = app.display.geometry.point(ids.center_b).unwrap();
        assert!(center_direction.projection(first, second) > 0.0);

        app.trigger_action();
        assert_live_result(&app.display, 0);
        assert_eq!(
            app.sketch.circle_tangency_mode(ids.tangency).unwrap(),
            CircleTangencyMode::External
        );
        assert_point_within(
            app.display.geometry.point(ids.center_b).unwrap(),
            Point2::new(3.0, 0.0),
            3.0e-9,
        );

        let retained_geometry = app.display.geometry.clone();
        let retained_audit = app.display.display_audit.clone();
        app.set_tangent_mode(CircleTangencyMode::Internal {
            containment: CircleContainment::SecondContainsFirst,
        });
        assert!(matches!(app.attempt, AttemptSummary::Error { .. }));
        assert_eq!(app.display.geometry, retained_geometry);
        assert_eq!(app.display.display_audit, retained_audit);
        assert_eq!(
            app.sketch.circle_tangency_mode(ids.tangency).unwrap(),
            CircleTangencyMode::External
        );
        assert!(
            live_sketch_view(&app)
                .unwrap()
                .status
                .contains("attempt error / retained state shown")
        );
    }

    #[test]
    fn arc_drag_updates_committed_span_and_rejects_escape_without_republishing() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::ArcContactDrag).unwrap();
        let ids = arc_contact_ids(&app);
        assert_live_result(&app.display, 1);
        let initial_point = app.display.geometry.point(ids.point).unwrap();
        let accepted_target = app
            .display
            .geometry
            .arc(ids.arc)
            .unwrap()
            .evaluate(0.72)
            .unwrap();

        app.active_pointer = Some(701);
        app.solve_drag(accepted_target);
        assert_live_result(&app.display, 1);
        let ContactState::PointOnArc { span_parameter } =
            app.sketch.contact_state(ids.contact).unwrap()
        else {
            panic!("expected committed arc span");
        };
        assert!((span_parameter - 0.72).abs() <= 2.0e-8);
        assert!(
            (app.display.geometry.point(ids.point).unwrap() - accepted_target).norm() <= 2.0e-8
        );
        assert!((app.display.geometry.point(ids.point).unwrap() - initial_point).norm() > 0.5);

        let retained_geometry = app.display.geometry.clone();
        let retained_audit = app.display.display_audit.clone();
        let retained_contact = app.sketch.contact_state(ids.contact).unwrap();
        let arc = *app.display.geometry.arc(ids.arc).unwrap();
        let escape_parameter = 1.2;
        let escape_angle = arc.start_angle + arc.signed_sweep * escape_parameter;
        let escape_target = Point2::new(
            arc.center.x + arc.radius * escape_angle.cos(),
            arc.center.y + arc.radius * escape_angle.sin(),
        );
        app.solve_drag(escape_target);

        let AttemptSummary::Rejected { rejection, .. } = &app.attempt else {
            panic!("expected stalled arc-drag rejection: {:#?}", app.attempt);
        };
        assert_eq!(
            *rejection,
            SolveRejection::CoreTermination(SolveTermination::Stalled)
        );
        assert_eq!(app.display.geometry, retained_geometry);
        assert_eq!(app.display.display_audit, retained_audit);
        assert_eq!(
            app.sketch.contact_state(ids.contact).unwrap(),
            retained_contact
        );
        let rejected_view = live_sketch_view(&app).unwrap();
        assert!(!rejected_view.geometry.is_empty());
        assert!(!rejected_view.audit.is_empty());
        assert!(rejected_view.geometry.contains("bounded-arc"));
        assert!(rejected_view.geometry.contains("arc-direction-cue"));
        assert!(!rejected_view.geometry.contains("hard-manifold"));
        assert!(
            rejected_view
                .status
                .contains("core solve terminated as stalled")
        );
        assert!(
            rejected_view
                .status
                .contains("prior geometry/contact retained")
        );
        assert!(rejected_view.status.contains("retained audit snapshot"));

        app.finish_drag();
        assert_eq!(app.active_pointer, None);
        assert!(matches!(app.attempt, AttemptSummary::Rejected { .. }));
        assert_eq!(app.display.geometry, retained_geometry);
        assert_eq!(app.display.display_audit, retained_audit);
    }

    #[test]
    fn auto_radius_scene_starts_accepted_with_two_dof_and_no_circle_driver() {
        let app = InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap();
        let ids = arc_circle_auto_radius_ids(&app);
        assert_live_result(&app.display, 2);
        assert_eq!(
            app.display.geometry.point(ids.arc_center).unwrap(),
            Point2::new(0.0, 0.0)
        );
        assert_point_within(
            app.display.geometry.point(ids.circle_center).unwrap(),
            Point2::new(3.4, 0.0),
            5.0e-9,
        );
        assert!((app.display.geometry.circle(ids.circle).unwrap().radius - 1.2).abs() <= 5.0e-9);
        assert_eq!(
            app.sketch.circle_arc_tangency_side(ids.tangency).unwrap(),
            ArcCircleTangencySide::OutsideArc
        );
        assert_eq!(app.sketch.dimensions().count(), 1);
        assert!(app.sketch.dimensions().all(|(_, dimension)| {
            !matches!(
                dimension.kind(),
                DimensionKind::CircleRadius { circle, .. }
                    | DimensionKind::CircleDiameter { circle, .. }
                    if circle == ids.circle
            )
        }));
        assert!(matches!(
            app.sketch.dimensions().next().unwrap().1.kind(),
            DimensionKind::ArcRadius { arc, target }
                if arc == ids.arc && (target - 2.2).abs() <= f64::EPSILON
        ));
        let ContactState::CircleArcTangency {
            arc_span_parameter,
            circle_angle,
        } = app.sketch.contact_state(ids.tangency).unwrap()
        else {
            panic!("expected circle-arc contact state");
        };
        assert!((arc_span_parameter - 0.5).abs() <= 1.0e-9);
        assert!((circle_angle - PI).abs() <= 1.0e-9);
        let arc_contact = app
            .display
            .geometry
            .arc(ids.arc)
            .unwrap()
            .evaluate(arc_span_parameter)
            .unwrap();
        let circle_contact = app
            .display
            .geometry
            .circle(ids.circle)
            .unwrap()
            .evaluate(circle_angle)
            .unwrap();
        assert_point_within(arc_contact, circle_contact, 1.0e-9);

        let view = live_sketch_view(&app).unwrap();
        assert!(!view.geometry.is_empty());
        assert!(!view.status.is_empty());
        assert!(!view.audit.is_empty());
        assert!(view.geometry.contains("arc-circle-auto-radius-geometry"));
        assert!(view.geometry.contains("bounded-arc auto-radius-arc"));
        assert!(view.geometry.contains("auto-radius-circle"));
        assert!(view.geometry.contains("shared-auto-contact"));
        assert!(view.geometry.contains("auto-radius-dimension"));
        assert!(view.geometry.contains("id=\"drag-handle\""));
        assert!(
            view.geometry
                .contains("data-drag-point=\"auto-radius-circle-center\"")
        );
        assert!(
            view.geometry
                .contains(&format!("r=\"{DRAG_HIT_RADIUS:.0}\""))
        );
        assert!(view.geometry.contains("2 local DOF"));
        assert!(view.geometry.contains("AUTO RADIUS r=1.200"));
        assert!(view.status.contains("2 local DOF"));
        assert!(view.status.contains("AUTO RADIUS r=1.200"));
        assert!(view.status.contains("OutsideArc"));
        assert!(view.status.contains("committed arc span t=0.500000"));
        assert!(
            view.status
                .contains("ACCEPTED / center, radius, contact, and audit committed")
        );
        assert_eq!(
            svg_scene_title(&view.geometry),
            "Arc-circle auto radius / 2 local DOF"
        );
        assert!(view.instructions.contains("retained solve report"));
        assert!(!view.instructions.contains("2 DOF"));
        assert!(
            view.instructions
                .contains("radius and contact variables are solved")
        );
        assert_eq!(
            view.audit.matches("class=\"audit-row\"").count(),
            display_audit_row_count(&app.display)
        );
        assert!(!view.audit.contains("circle.radius - target"));
    }

    #[test]
    fn auto_radius_svg_title_uses_rank_valid_report_mobility() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap();
        assert_eq!(
            auto_radius_scene_title(&app.display.core_report),
            "Arc-circle auto radius / 2 local DOF"
        );

        app.display.core_report.local_degrees_of_freedom = 7;
        let seven_dof = live_sketch_view(&app).unwrap();
        assert_eq!(
            svg_scene_title(&seven_dof.geometry),
            "Arc-circle auto radius / 7 local DOF"
        );
        assert!(seven_dof.status.contains("7 local DOF"));

        app.display.core_report.local_degrees_of_freedom = 2;
        app.display.core_report.rank_is_valid = false;
        let unavailable = live_sketch_view(&app).unwrap();
        assert_eq!(
            svg_scene_title(&unavailable.geometry),
            "Arc-circle auto radius / DOF unavailable"
        );
        assert!(unavailable.status.contains("DOF unavailable"));
    }

    #[test]
    fn auto_radius_two_dimensional_drags_solve_distinct_radii_contacts_and_release() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap();
        let ids = arc_circle_auto_radius_ids(&app);
        let mut radii = Vec::new();
        let mut contacts = Vec::new();
        for (pointer, distance, angle) in [
            (901, 3.7, 0.35_f64),
            (902, 3.05, -0.55_f64),
            (903, 4.0, 0.1_f64),
        ] {
            let target = Point2::new(distance * angle.cos(), distance * angle.sin());
            app.active_pointer = Some(pointer);
            app.solve_drag(target);
            assert_live_result(&app.display, 2);
            assert_point_within(
                app.display.geometry.point(ids.circle_center).unwrap(),
                target,
                6.0e-9,
            );
            let solved_circle = *app.display.geometry.circle(ids.circle).unwrap();
            assert!((solved_circle.radius - (distance - 2.2)).abs() <= 6.0e-9);
            let state = app.sketch.contact_state(ids.tangency).unwrap();
            let ContactState::CircleArcTangency {
                arc_span_parameter,
                circle_angle,
            } = state
            else {
                panic!("expected circle-arc contact state");
            };
            let arc_contact = app
                .display
                .geometry
                .arc(ids.arc)
                .unwrap()
                .evaluate(arc_span_parameter)
                .unwrap();
            let circle_contact = solved_circle.evaluate(circle_angle).unwrap();
            assert_point_within(arc_contact, circle_contact, 1.0e-9);
            radii.push(solved_circle.radius);
            contacts.push(state);

            let retained_geometry = app.display.geometry.clone();
            let retained_contact = state;
            app.finish_drag();
            assert_eq!(app.active_pointer, None);
            assert_live_result(&app.display, 2);
            assert_eq!(app.display.geometry, retained_geometry);
            assert_eq!(
                app.sketch.contact_state(ids.tangency).unwrap(),
                retained_contact
            );
        }
        assert!(radii.windows(2).all(|pair| (pair[0] - pair[1]).abs() > 0.1));
        assert!(contacts.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn auto_radius_invalid_span_side_and_zero_radius_requests_retain_all_published_state() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap();
        let ids = arc_circle_auto_radius_ids(&app);
        app.solve_drag(Point2::new(3.6, 0.7));
        assert_live_result(&app.display, 2);
        let retained_geometry = app.display.geometry.clone();
        let retained_audit = app.display.display_audit.clone();
        let retained_contact = app.sketch.contact_state(ids.tangency).unwrap();
        let retained_diagnostics = app.retained_diagnostics.clone();
        let missing_span_angle = 155.0_f64.to_radians();

        for (target, expected_kind, expected_rejection) in [
            (
                Point2::new(
                    3.5 * missing_span_angle.cos(),
                    3.5 * missing_span_angle.sin(),
                ),
                "span",
                SolveRejection::CoreTermination(SolveTermination::Stalled),
            ),
            (
                Point2::new(1.8, 0.0),
                "side",
                SolveRejection::CoreTermination(SolveTermination::Stalled),
            ),
            (
                Point2::new(2.2, 0.0),
                "zero",
                SolveRejection::AmbiguousTangencyScale(ids.tangency),
            ),
        ] {
            app.solve_drag(target);
            let AttemptSummary::Rejected {
                termination,
                rejection,
            } = &app.attempt
            else {
                panic!(
                    "expected typed {expected_kind} rejection: {:#?}",
                    app.attempt
                );
            };
            assert_eq!(*termination, SolveTermination::Stalled, "{expected_kind}");
            assert_eq!(*rejection, expected_rejection, "{expected_kind}");
            assert_eq!(app.display.geometry, retained_geometry);
            assert_eq!(app.display.display_audit, retained_audit);
            assert_eq!(
                app.sketch.contact_state(ids.tangency).unwrap(),
                retained_contact
            );
            assert_eq!(app.retained_diagnostics, retained_diagnostics);
        }

        let rejected = live_sketch_view(&app).unwrap();
        assert!(rejected.status.contains("REJECTED /"));
        assert!(
            rejected
                .status
                .contains("prior center/radius/contact/audit retained")
        );
        assert!(
            rejected
                .status
                .contains(&sketch_rejection_summary(match &app.attempt {
                    AttemptSummary::Rejected { rejection, .. } => rejection,
                    _ => unreachable!(),
                }))
        );
        assert!(!rejected.geometry.is_empty());
        assert!(!rejected.audit.is_empty());
        app.finish_drag();
        assert_eq!(app.display.geometry, retained_geometry);
        assert_eq!(app.display.display_audit, retained_audit);
        assert_eq!(
            app.sketch.contact_state(ids.tangency).unwrap(),
            retained_contact
        );
    }

    #[test]
    fn tangent_glide_updates_contacts_and_rejects_supporting_line_escape() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::LineCircleTangentGlide).unwrap();
        let ids = tangent_glide_ids(&app);
        assert_live_result(&app.display, 1);
        let initial_center = app.display.geometry.point(ids.center).unwrap();

        app.active_pointer = Some(811);
        app.solve_drag(Point2::new(1.4, 2.4));
        assert_live_result(&app.display, 1);
        let solved_center = app.display.geometry.point(ids.center).unwrap();
        assert!((solved_center.x - 1.4).abs() <= 2.0e-8);
        assert!((solved_center.y - 1.0).abs() <= 2.0e-8);
        assert!((solved_center - initial_center).norm() > 2.0);
        let ContactState::LineCircleTangency {
            line_parameter,
            circle_angle,
        } = app.sketch.contact_state(ids.tangency).unwrap()
        else {
            panic!("expected committed line-circle contact");
        };
        assert!((line_parameter - (1.4 + 3.0) / 6.0).abs() <= 2.0e-8);
        assert!((circle_angle + FRAC_PI_2).abs() <= 2.0e-8);
        let circle_contact = app
            .display
            .geometry
            .circle(ids.circle)
            .unwrap()
            .evaluate(circle_angle)
            .unwrap();
        let line_start = app.display.geometry.point(ids.line_start).unwrap();
        let line_end = app.display.geometry.point(ids.line_end).unwrap();
        let line_contact = line_start + (line_end - line_start) * line_parameter;
        assert!((circle_contact - line_contact).norm() <= 2.0e-8);

        let retained_geometry = app.display.geometry.clone();
        let retained_audit = app.display.display_audit.clone();
        let retained_contact = app.sketch.contact_state(ids.tangency).unwrap();
        app.solve_drag(Point2::new(4.2, 1.0));

        let AttemptSummary::Rejected { rejection, .. } = &app.attempt else {
            panic!("expected stalled line-drag rejection: {:#?}", app.attempt);
        };
        assert_eq!(
            *rejection,
            SolveRejection::CoreTermination(SolveTermination::Stalled)
        );
        assert_eq!(app.display.geometry, retained_geometry);
        assert_eq!(app.display.display_audit, retained_audit);
        assert_eq!(
            app.sketch.contact_state(ids.tangency).unwrap(),
            retained_contact
        );
        let rejected_view = live_sketch_view(&app).unwrap();
        assert!(!rejected_view.geometry.is_empty());
        assert!(!rejected_view.audit.is_empty());
        assert!(rejected_view.geometry.contains("bounded-tangent-line"));
        assert!(rejected_view.geometry.contains("gliding-circle"));
        assert!(rejected_view.geometry.contains("radius-normal"));
        assert!(rejected_view.geometry.contains("tangent-side-arrow"));
        assert!(
            rejected_view
                .status
                .contains("core solve terminated as stalled")
        );
        assert!(
            rejected_view
                .status
                .contains("Left / bounded-segment domain")
        );
        assert!(rejected_view.status.contains("retained audit snapshot"));
        assert!(
            rejected_view
                .geometry
                .contains("class=\"line-endpoint start\"")
        );
        assert!(
            rejected_view
                .geometry
                .contains("class=\"line-endpoint end\"")
        );
    }

    #[test]
    fn rejection_wording_uses_typed_classification_in_banner_and_curve_hud() {
        let tangent = InteractiveSketchState::new(LiveSceneKind::LineCircleTangentGlide).unwrap();
        let tangent_ids = tangent_glide_ids(&tangent);
        let s3 = InteractiveSketchState::new(LiveSceneKind::TangentCircles).unwrap();
        let s3_ids = tangent_circle_ids(&s3);
        let cases = [
            (
                SolveRejection::HardResidual {
                    maximum: 2.0e-4,
                    tolerance: 1.0e-9,
                },
                "hard residual validation rejected",
            ),
            (
                SolveRejection::CoreTermination(SolveTermination::Stalled),
                "core solve terminated as stalled",
            ),
            (
                SolveRejection::LineSideFlipped(tangent_ids.tangency),
                "explicit line-side branch rejected",
            ),
            (
                SolveRejection::CenterDirectionFlipped(s3_ids.tangency),
                "explicit center-direction branch rejected",
            ),
            (
                SolveRejection::SegmentBranchFlipped(tangent_ids.line),
                "explicit segment branch rejected",
            ),
        ];
        for (rejection, expected) in cases {
            let attempt = AttemptSummary::Rejected {
                termination: SolveTermination::Stalled,
                rejection,
            };
            let hud = bounded_attempt_status(&attempt, "segment endpoint");
            let banner = attempt_markup(&attempt);
            assert!(hud.contains(expected), "{hud}");
            assert!(banner.contains(expected), "{banner}");
            assert!(!hud.contains("escape rejected"), "{hud}");
            assert!(!banner.contains("escape rejected"), "{banner}");
            assert!(hud.contains("prior geometry/contact retained"));
            assert!(banner.contains("prior geometry/audit remains displayed"));
        }

        let bounded = AttemptSummary::Rejected {
            termination: SolveTermination::Converged,
            rejection: SolveRejection::ContactParameterOutOfDomain(tangent_ids.tangency),
        };
        let hud = bounded_attempt_status(&bounded, "segment endpoint");
        let banner = attempt_markup(&bounded);
        assert!(hud.contains("segment endpoint escape rejected"));
        assert!(banner.contains("contact parameter left its bounded domain"));
        assert!(!banner.contains("hard residual"));
    }

    #[test]
    fn ambiguous_auto_radius_scale_has_truthful_typed_retention_ui() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap();
        let ids = arc_circle_auto_radius_ids(&app);
        let retained_geometry = app.display.geometry.clone();
        let retained_audit = app.display.display_audit.clone();
        let rejection = SolveRejection::AmbiguousTangencyScale(ids.tangency);
        assert_eq!(
            sketch_rejection_summary(&rejection),
            "tangency feature scales are numerically ambiguous"
        );
        app.attempt = AttemptSummary::Rejected {
            termination: SolveTermination::Converged,
            rejection,
        };

        let view = live_sketch_view(&app).unwrap();
        assert!(
            view.status
                .contains("tangency feature scales are numerically ambiguous")
        );
        assert!(
            view.status
                .contains("prior center/radius/contact/audit retained")
        );
        assert!(
            view.status
                .contains("attempt rejected / retained state shown")
        );
        assert_eq!(app.display.geometry, retained_geometry);
        assert_eq!(app.display.display_audit, retained_audit);
    }

    #[test]
    fn generic_scene_action_is_visible_only_for_s3_and_uses_a_native_button() {
        let mut s3 = DemoState::Sketch(Box::new(
            InteractiveSketchState::new(LiveSceneKind::TangentCircles).unwrap(),
        ));
        assert!(s3.has_action());
        assert_eq!(s3.action_label().unwrap(), Some("Switch to internal"));
        s3.trigger_action();
        assert_eq!(s3.action_label().unwrap(), Some("Switch to external"));
        for state in [
            DemoState::Sketch(Box::new(
                InteractiveSketchState::new(LiveSceneKind::ArcContactDrag).unwrap(),
            )),
            DemoState::Sketch(Box::new(
                InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap(),
            )),
            DemoState::Sketch(Box::new(
                InteractiveSketchState::new(LiveSceneKind::LineCircleTangentGlide).unwrap(),
            )),
            DemoState::Linkage(Box::new(
                InteractiveLinkageState::new(LinkageSceneKind::FourBarOpen).unwrap(),
            )),
        ] {
            assert!(!state.has_action());
            assert_eq!(state.action_label().unwrap(), None);
        }

        let page = include_str!("../index.html");
        assert!(page.contains("id=\"scene-action-controls\""));
        assert!(page.contains("aria-label=\"Scene action\""));
        assert!(page.contains("<button id=\"scene-action\" type=\"button\""));
        assert!(page.contains("aria-describedby=\"scene-action-help\""));
        assert!(page.contains("disabled"));
        assert!(page.contains("hidden"));
        let styles = include_str!("../styles.css");
        assert!(styles.contains(".scene-action-controls[hidden]"));
        assert!(styles.contains("#scene-action:focus-visible"));
    }

    #[test]
    fn rebuilding_an_m7_scene_resets_geometry_branch_and_contact_state() {
        let mut s3 = InteractiveSketchState::new(LiveSceneKind::TangentCircles).unwrap();
        let s3_ids = tangent_circle_ids(&s3);
        s3.trigger_action();
        assert!(matches!(
            s3.sketch.circle_tangency_mode(s3_ids.tangency).unwrap(),
            CircleTangencyMode::Internal { .. }
        ));
        let reset_s3 = InteractiveSketchState::new(LiveSceneKind::TangentCircles).unwrap();
        let reset_s3_ids = tangent_circle_ids(&reset_s3);
        assert_eq!(
            reset_s3
                .sketch
                .circle_tangency_mode(reset_s3_ids.tangency)
                .unwrap(),
            CircleTangencyMode::External
        );

        let mut arc = InteractiveSketchState::new(LiveSceneKind::ArcContactDrag).unwrap();
        let arc_ids = arc_contact_ids(&arc);
        let target = arc
            .display
            .geometry
            .arc(arc_ids.arc)
            .unwrap()
            .evaluate(0.7)
            .unwrap();
        arc.solve_drag(target);
        let moved = arc.sketch.contact_state(arc_ids.contact).unwrap();
        let reset_arc = InteractiveSketchState::new(LiveSceneKind::ArcContactDrag).unwrap();
        let reset_arc_ids = arc_contact_ids(&reset_arc);
        assert_ne!(
            reset_arc
                .sketch
                .contact_state(reset_arc_ids.contact)
                .unwrap(),
            moved
        );
        assert_eq!(
            reset_arc
                .sketch
                .contact_state(reset_arc_ids.contact)
                .unwrap(),
            ContactState::PointOnArc {
                span_parameter: 0.38
            }
        );

        let mut auto_radius =
            InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap();
        let auto_ids = arc_circle_auto_radius_ids(&auto_radius);
        auto_radius.solve_drag(Point2::new(3.8, 1.0));
        assert_live_result(&auto_radius.display, 2);
        assert!(
            (auto_radius
                .display
                .geometry
                .circle(auto_ids.circle)
                .unwrap()
                .radius
                - 1.2)
                .abs()
                > 0.1
        );
        let reset_auto = InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap();
        let reset_auto_ids = arc_circle_auto_radius_ids(&reset_auto);
        assert_point_within(
            reset_auto
                .display
                .geometry
                .point(reset_auto_ids.circle_center)
                .unwrap(),
            Point2::new(3.4, 0.0),
            5.0e-9,
        );
        assert!(
            (reset_auto
                .display
                .geometry
                .circle(reset_auto_ids.circle)
                .unwrap()
                .radius
                - 1.2)
                .abs()
                <= 5.0e-9
        );
        assert_eq!(
            reset_auto
                .sketch
                .contact_state(reset_auto_ids.tangency)
                .unwrap(),
            ContactState::CircleArcTangency {
                arc_span_parameter: 0.5,
                circle_angle: PI,
            }
        );
    }

    #[test]
    fn s2_initializes_from_expected_rejection_with_only_typed_width_conflicts() {
        let state = ConflictingRectangleState::new().unwrap();
        assert!(!state.display.accepted());
        assert_ne!(
            state.display.core_report.termination,
            SolveTermination::Converged
        );
        assert!(state.display.acceptance_hard_residual_max.is_none());
        assert!(state.display.core_report.hard_residuals_validated);
        assert!(state.display.core_report.hard_residual_max > 1.0e-9);
        assert!(sketch_geometry_is_finite(&state.display.geometry));
        for point in [state.ids.a, state.ids.b, state.ids.c, state.ids.d] {
            let point = state.display.geometry.point(point).unwrap();
            assert!(point.x.is_finite() && point.y.is_finite());
        }

        assert_eq!(
            state
                .conflicts
                .iter()
                .map(|conflict| (conflict.source, conflict.diagnostic_label))
                .collect::<Vec<_>>(),
            vec![
                (SketchSource::Dimension(state.ids.width_4), "width-4"),
                (SketchSource::Dimension(state.ids.width_5), "width-5"),
            ]
        );
        assert_eq!(state.display.core_report.conflicting_sources.len(), 2);
        assert_eq!(state.conflicts.len(), 2);
        for conflict in &state.conflicts {
            assert!(
                state
                    .display
                    .core_report
                    .conflicting_sources
                    .contains(&conflict.core_source_id)
            );
            let mapping = state
                .display
                .source_mappings
                .iter()
                .find(|mapping| mapping.core_source_id == Some(conflict.core_source_id))
                .unwrap();
            assert_eq!(mapping.source, conflict.source);
            assert_eq!(mapping.source_label, conflict.source_label);
        }
        for constraint in [
            state.ids.fixed_a,
            state.ids.horizontal_ab,
            state.ids.horizontal_cd,
            state.ids.vertical_bc,
            state.ids.vertical_da,
        ] {
            let mapping = state
                .display
                .source_mappings
                .iter()
                .find(|mapping| mapping.source == SketchSource::Constraint(constraint))
                .unwrap();
            assert!(mapping.core_source_id.is_none_or(|source| {
                !state
                    .display
                    .core_report
                    .conflicting_sources
                    .contains(&source)
            }));
        }
    }

    #[test]
    fn s2_render_uses_retained_geometry_display_audit_and_expected_conflict_status() {
        let state = ConflictingRectangleState::new().unwrap();
        let view = expected_conflict_view(&state).unwrap();
        let row_count = audit_row_count(&state.display.display_audit);
        assert!(view.geometry.contains("conflicting-rectangle-geometry"));
        assert_eq!(view.geometry.matches("class=\"rectangle-edge\"").count(), 4);
        for label in ["A", "B", "C", "D"] {
            assert!(view.geometry.contains(&format!("data-point=\"{label}\"")));
        }
        assert!(
            view.geometry
                .contains("EXPECTED S2 CONFLICT / RETAINED GEOMETRY")
        );
        assert!(view.geometry.contains("not a converged solution"));
        assert!(!view.geometry.contains("id=\"drag-handle\""));
        assert!(
            view.status
                .contains("expected conflict diagnosed / retained geometry shown")
        );
        assert!(
            view.status
                .contains("accepted state</span><strong>no / expected rejected diagnosis")
        );
        assert!(
            view.status
                .contains("attempted conflict candidates</span><strong>width-4, width-5")
        );
        assert!(
            view.status
                .contains("non-width sources blamed</span><strong>none")
        );
        assert!(
            view.status
                .contains("attempted validated max hard residual")
        );
        assert!(view.status.contains("attempted rank / local DOF"));
        assert_eq!(view.audit.matches("class=\"audit-row\"").count(), row_count);
        assert_eq!(
            view.audit.matches("class=\"evaluation evaluated\"").count(),
            row_count
        );
        assert!(
            state
                .display
                .display_audit
                .sources
                .iter()
                .flat_map(|source| &source.rows)
                .all(
                    |row| row.evaluation_status == AuditEvaluationStatus::Evaluated
                        && row.raw_residual.is_finite()
                        && row.normalized_residual.is_finite()
                )
        );
        assert_eq!(view.badge, "expected S2 conflict");
        assert!(view.instructions.contains("no pointer interaction"));
        assert_eq!(
            view.announcement,
            "Expected sketch conflict diagnosed. Retained rectangle geometry is displayed."
        );

        let source = include_str!("lib.rs");
        let duplicate_equation = ["distance(A, B)", " - target"].concat();
        assert!(!source.contains(&duplicate_equation));
    }

    #[test]
    fn horizontal_rail_drag_projects_to_one_dof_and_release_preserves_position() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::HorizontalRail).unwrap();
        let ids = rail_ids(&app);
        assert_live_result(&app.display, 1);
        assert_point_near(
            app.display.geometry.point(ids.a).unwrap(),
            Point2::new(0.0, 0.0),
        );
        assert_point_near(
            app.display.geometry.point(ids.b).unwrap(),
            Point2::new(3.0, 0.0),
        );
        assert!(
            (reference_dimension_value(&app.display, ids.reference_length).unwrap() - 3.0).abs()
                <= 1.0e-9
        );

        for (pointer, target) in [(17, Point2::new(4.25, 2.0)), (29, Point2::new(-1.5, -3.25))] {
            app.active_pointer = Some(pointer);
            app.solve_drag(target);
            assert_live_result(&app.display, 1);
            let solved = app.display.geometry.point(ids.b).unwrap();
            assert_point_near(solved, Point2::new(target.x, 0.0));
            let reference = reference_dimension_value(&app.display, ids.reference_length).unwrap();
            assert!((reference - target.x.abs()).abs() <= 1.0e-8);

            app.finish_drag();
            assert_eq!(app.active_pointer, None);
            assert_live_result(&app.display, 1);
            assert_point_near(app.display.geometry.point(ids.b).unwrap(), solved);
            assert!((app.sketch.point(ids.b).unwrap().position() - solved).norm() <= 1.0e-12);
        }
    }

    #[test]
    fn coincident_pair_drag_moves_both_points_and_release_preserves_common_position() {
        let (initial_sketch, initial_scene) =
            build_live_scene(LiveSceneKind::CoincidentPair).unwrap();
        let LiveScene::CoincidentPair(initial_ids) = initial_scene else {
            panic!("expected coincident scene");
        };
        assert_ne!(
            initial_sketch.point(initial_ids.a).unwrap().position(),
            initial_sketch.point(initial_ids.b).unwrap().position()
        );

        let mut app = InteractiveSketchState::new(LiveSceneKind::CoincidentPair).unwrap();
        let ids = coincident_ids(&app);
        assert_live_result(&app.display, 2);
        let initial_a = app.display.geometry.point(ids.a).unwrap();
        let initial_b = app.display.geometry.point(ids.b).unwrap();
        assert_point_near(initial_a, Point2::new(0.0, 0.0));
        assert_point_near(initial_b, initial_a);

        for (pointer, target) in [(31, Point2::new(2.75, 1.5)), (47, Point2::new(-1.25, -2.0))] {
            app.active_pointer = Some(pointer);
            app.solve_drag(target);
            assert_live_result(&app.display, 2);
            let solved_a = app.display.geometry.point(ids.a).unwrap();
            let solved_b = app.display.geometry.point(ids.b).unwrap();
            assert_point_near(solved_a, target);
            assert_point_near(solved_b, target);
            assert_point_near(solved_a, solved_b);

            app.finish_drag();
            assert_eq!(app.active_pointer, None);
            assert_live_result(&app.display, 2);
            assert_point_near(app.display.geometry.point(ids.a).unwrap(), solved_a);
            assert_point_near(app.display.geometry.point(ids.b).unwrap(), solved_b);
        }
    }

    #[test]
    fn every_live_scene_renders_only_its_evaluated_display_audit_rows() {
        for (kind, geometry_text, instruction_text, status_text) in [
            (
                LiveSceneKind::UnderconstrainedTriangle,
                "s1-geometry",
                "distance hard manifold",
                "rightward (+x); preserved",
            ),
            (
                LiveSceneKind::HorizontalRail,
                "rail-geometry",
                "equation-free reference measurement",
                "continuous horizontal motion; no discrete branch",
            ),
            (
                LiveSceneKind::CoincidentPair,
                "coincident-geometry",
                "moves A and B together",
                "no discrete branch; common point translates in 2D",
            ),
        ] {
            let app = InteractiveSketchState::new(kind).unwrap();
            let view = live_sketch_view(&app).unwrap();
            let row_count = display_audit_row_count(&app.display);
            assert!(view.geometry.contains(geometry_text));
            assert!(view.instructions.contains(instruction_text));
            assert!(view.status.contains(status_text));
            assert_eq!(view.audit.matches("class=\"audit-row\"").count(), row_count);
            assert_eq!(
                view.audit.matches("class=\"evaluation evaluated\"").count(),
                row_count
            );
            assert!(view.geometry.contains("id=\"drag-handle\""));
            assert!(view.geometry.contains("data-drag-point="));
            assert!(
                view.geometry
                    .contains(&format!("r=\"{DRAG_HIT_RADIUS:.0}\""))
            );
        }

        let rail = InteractiveSketchState::new(LiveSceneKind::HorizontalRail).unwrap();
        let rail_view = live_sketch_view(&rail).unwrap();
        assert!(rail_view.geometry.contains("horizontal hard rail"));
        assert!(rail_view.geometry.contains("reference length 3.000"));
        assert!(rail_view.audit.contains("reference-measurement"));
        assert!(rail_view.audit.contains("none; display-only measurement"));
        assert!(
            rail_view
                .status
                .contains("equality right nullity</span><strong>1")
        );

        let coincident = InteractiveSketchState::new(LiveSceneKind::CoincidentPair).unwrap();
        let coincident_view = live_sketch_view(&coincident).unwrap();
        assert!(coincident_view.geometry.contains("coincident-point-a"));
        assert!(coincident_view.geometry.contains("coincident-point-b"));
        assert!(
            coincident_view
                .status
                .contains("equality right nullity</span><strong>2")
        );
    }

    #[test]
    fn model_svg_and_client_view_box_transforms_round_trip() {
        for model in [
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(-2.125, 3.75),
        ] {
            let round_trip = MODEL_TRANSFORM.svg_to_model(MODEL_TRANSFORM.model_to_svg(model));
            assert!((round_trip - model).norm() <= 1.0e-12);
        }

        let bounds = ClientRect {
            left: 30.0,
            top: 50.0,
            width: 1280.0,
            height: 840.0,
        };
        let client = SvgPoint { x: 670.0, y: 470.0 };
        let svg = client_to_svg(client, bounds, SVG_VIEW_BOX).unwrap();
        assert!((svg.x - 320.0).abs() <= f64::EPSILON);
        assert!((svg.y - 210.0).abs() <= f64::EPSILON);
    }

    #[test]
    fn m7_arc_and_tangent_client_model_transforms_follow_the_responsive_viewport() {
        let bounds = ClientRect {
            left: 17.0,
            top: 31.0,
            width: 960.0,
            height: 630.0,
        };
        for (kind, model) in [
            (LiveSceneKind::ArcContactDrag, Point2::new(-1.25, 1.4)),
            (LiveSceneKind::ArcCircleAutoRadius, Point2::new(3.45, 0.85)),
            (
                LiveSceneKind::LineCircleTangentGlide,
                Point2::new(2.35, 0.8),
            ),
        ] {
            let transform = kind.transform();
            let svg = transform.model_to_svg(model);
            let client = SvgPoint {
                x: bounds.left + svg.x * bounds.width / SVG_VIEW_BOX.width,
                y: bounds.top + svg.y * bounds.height / SVG_VIEW_BOX.height,
            };
            let recovered = client_to_drag_target(kind, client, bounds).unwrap();
            assert!((recovered - model).norm() <= 1.0e-12);
            assert!((transform.svg_to_model(svg) - model).norm() <= 1.0e-12);

            let outside = SvgPoint {
                x: SVG_VIEW_BOX.width + 80.0,
                y: -45.0,
            };
            assert_eq!(clamp_drag_svg_point(kind, outside), outside);
        }
    }

    #[test]
    fn auto_radius_mobile_transform_and_center_hit_target_remain_usable() {
        let kind = LiveSceneKind::ArcCircleAutoRadius;
        let bounds = ClientRect {
            left: 5.0,
            top: 180.0,
            width: 380.0,
            height: 380.0 * SVG_VIEW_BOX.height / SVG_VIEW_BOX.width,
        };
        let model = Point2::new(3.3, -0.65);
        let svg = kind.transform().model_to_svg(model);
        let client = SvgPoint {
            x: bounds.left + svg.x * bounds.width / SVG_VIEW_BOX.width,
            y: bounds.top + svg.y * bounds.height / SVG_VIEW_BOX.height,
        };
        assert_point_within(
            client_to_drag_target(kind, client, bounds).unwrap(),
            model,
            1.0e-12,
        );
        let hit_diameter_css = 2.0 * DRAG_HIT_RADIUS * bounds.width / SVG_VIEW_BOX.width;
        assert!(hit_diameter_css >= 44.0, "got {hit_diameter_css}");

        let app = InteractiveSketchState::new(kind).unwrap();
        let view = live_sketch_view(&app).unwrap();
        assert!(
            view.geometry
                .contains("data-drag-point=\"auto-radius-circle-center\"")
        );
        assert!(
            view.geometry
                .contains(&format!("r=\"{DRAG_HIT_RADIUS:.0}\""))
        );
        let styles = include_str!("../styles.css");
        assert!(styles.contains(".drag-target"));
        assert!(styles.contains("cursor: grab"));
        assert!(styles.contains("#viewport[data-drag-active=\"true\"]"));
        assert!(styles.contains(".auto-radius-hud"));
    }

    #[test]
    fn arc_ccw_240_svg_path_has_exact_large_arc_and_screen_sweep_flags() {
        let app = InteractiveSketchState::new(LiveSceneKind::ArcContactDrag).unwrap();
        let ids = arc_contact_ids(&app);
        let arc = app.display.geometry.arc(ids.arc).unwrap();
        assert_eq!(arc.sweep, ArcSweep::CounterClockwise);
        assert!((arc.signed_sweep.to_degrees() - 240.0).abs() <= 1.0e-12);

        let geometry = live_sketch_view(&app).unwrap().geometry;
        let expected =
            r#"class="bounded-arc" d="M 444.708 322.000 A 144.000 144.000 0 1 0 195.292 322.000""#;
        assert!(geometry.contains(expected), "{geometry}");
        assert!(!geometry.contains("A 144.000 144.000 0 1 1"));
    }

    #[test]
    fn auto_radius_ccw_300_svg_path_has_exact_large_arc_and_screen_sweep_flags() {
        let app = InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap();
        let ids = arc_circle_auto_radius_ids(&app);
        let arc = app.display.geometry.arc(ids.arc).unwrap();
        assert_eq!(arc.sweep, ArcSweep::CounterClockwise);
        assert!((arc.signed_sweep.to_degrees() - 300.0).abs() <= 1.0e-12);

        let geometry = live_sketch_view(&app).unwrap().geometry;
        let expected = r#"class="bounded-arc auto-radius-arc" d="M 179.495 318.800 A 127.600 127.600 0 1 0 179.495 191.200""#;
        assert!(geometry.contains(expected), "{geometry}");
        assert!(!geometry.contains("A 127.600 127.600 0 1 1"));
    }

    #[test]
    fn viewport_drag_state_and_tangent_endpoint_styles_are_explicit() {
        let mut state = DemoState::Sketch(Box::new(
            InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap(),
        ));
        assert!(!state.drag_active());
        let DemoState::Sketch(sketch) = &mut state else {
            panic!("expected sketch state");
        };
        sketch.active_pointer = Some(91);
        assert!(state.drag_active());
        let DemoState::Sketch(sketch) = &mut state else {
            panic!("expected sketch state");
        };
        sketch.finish_drag();
        assert!(!state.drag_active());
        assert!(
            !DemoState::Sketch(Box::new(
                InteractiveSketchState::new(LiveSceneKind::ArcCircleAutoRadius).unwrap(),
            ))
            .drag_active()
        );

        let styles = include_str!("../styles.css");
        assert!(styles.contains("#viewport[data-drag-active=\"true\"]"));
        assert!(styles.contains(".line-endpoint.end"));
        assert!(!styles.contains(".line-endpoint:last-of-type"));
        let source = include_str!("lib.rs");
        assert!(source.contains("viewport.set_attribute(\"data-drag-active\", \"true\")"));
        assert!(source.contains("viewport.remove_attribute(\"data-drag-active\")"));
        assert!(source.contains("install_pointer_end(document, viewport, app, \"pointerup\")"));
        assert!(source.contains("install_pointer_end(document, viewport, app, \"pointercancel\")"));
    }

    #[test]
    fn outside_rail_and_coincident_drags_retain_fully_visible_handles() {
        let low = SvgPoint {
            x: -1.0e6,
            y: -1.0e6,
        };
        let high = SvgPoint { x: 1.0e6, y: 1.0e6 };
        assert_eq!(
            clamp_drag_svg_point(LiveSceneKind::UnderconstrainedTriangle, low),
            low
        );
        for kind in [LiveSceneKind::HorizontalRail, LiveSceneKind::CoincidentPair] {
            assert_eq!(
                clamp_drag_svg_point(kind, low),
                SvgPoint {
                    x: DRAG_CLAMP_MARGIN,
                    y: DRAG_CLAMP_MARGIN,
                }
            );
            assert_eq!(
                clamp_drag_svg_point(kind, high),
                SvgPoint {
                    x: SVG_VIEW_BOX.width - DRAG_CLAMP_MARGIN,
                    y: SVG_VIEW_BOX.height - DRAG_CLAMP_MARGIN,
                }
            );

            let bounds = ClientRect {
                left: 40.0,
                top: 70.0,
                width: 312.0,
                height: 312.0 * SVG_VIEW_BOX.height / SVG_VIEW_BOX.width,
            };
            let expected_dof = if kind == LiveSceneKind::HorizontalRail {
                1
            } else {
                2
            };
            let mut app = InteractiveSketchState::new(kind).unwrap();
            for (pointer, client) in [(71, low), (83, high)] {
                let raw_svg = client_to_svg(client, bounds, SVG_VIEW_BOX).unwrap();
                assert!(raw_svg.x < SVG_VIEW_BOX.min_x || raw_svg.x > SVG_VIEW_BOX.width);
                assert!(raw_svg.y < SVG_VIEW_BOX.min_y || raw_svg.y > SVG_VIEW_BOX.height);
                let target = client_to_drag_target(kind, client, bounds).unwrap();
                let target_svg = MODEL_TRANSFORM.model_to_svg(target);
                assert!(target_svg.x >= DRAG_CLAMP_MARGIN);
                assert!(target_svg.x <= SVG_VIEW_BOX.width - DRAG_CLAMP_MARGIN);
                assert!(target_svg.y >= DRAG_CLAMP_MARGIN);
                assert!(target_svg.y <= SVG_VIEW_BOX.height - DRAG_CLAMP_MARGIN);

                app.active_pointer = Some(pointer);
                app.solve_drag(target);
                assert_live_result(&app.display, expected_dof);
                assert_drag_handle_inside_margin(&app);
                app.finish_drag();
                assert_live_result(&app.display, expected_dof);
                assert_drag_handle_inside_margin(&app);
            }
        }
    }

    #[test]
    fn rejected_attempt_renders_retained_geometry_and_display_audit() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::UnderconstrainedTriangle).unwrap();
        let ids = triangle_ids(&app);
        let retained_diagnostics = app.retained_diagnostics.clone();
        app.sketch
            .set_point_position(ids.b, Point2::new(-3.0, 0.0))
            .unwrap();
        let retained = app.sketch.geometry();
        let mut rejected = app
            .sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert!(!rejected.accepted());
        assert_eq!(rejected.geometry, retained);

        let display_row = source_row(&rejected.display_audit, "length AB");
        let candidate_row = source_row(&rejected.core_report.audit, "length AB");
        assert!((display_row.raw_residual + 1.0).abs() <= 1.0e-12);
        assert!(candidate_row.raw_residual.abs() <= 1.0e-9);
        let display_raw_residual = display_row.raw_residual;

        rejected.core_report.hard_residual_max = 9.9;
        rejected.core_report.rank_is_valid = true;
        rejected.core_report.rank = 91;
        rejected.core_report.local_degrees_of_freedom = 92;
        rejected.core_report.iterations = 939;
        rejected.core_report.is_singular = true;
        app.apply_result(rejected);
        assert_eq!(app.retained_diagnostics, retained_diagnostics);

        let view = live_sketch_view(&app).unwrap();
        assert!(view.geometry.contains("data-model-x=\"-3.000000\""));
        assert!(view.audit.contains(&format_metric(display_raw_residual)));
        assert!(
            view.status
                .contains("attempt rejected / retained state shown")
        );
        assert!(view.status.contains("retained rank</span><strong>3"));
        assert!(
            view.status
                .contains("retained singularity</span><strong>none")
        );
        for candidate_value in ["9.900e0", ">91<", ">92<", ">939<"] {
            assert!(!view.status.contains(candidate_value));
        }
    }

    #[test]
    fn api_error_keeps_display_and_diagnostics_without_a_stale_attempt_report() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::UnderconstrainedTriangle).unwrap();
        let ids = triangle_ids(&app);
        let retained_geometry = app.display.geometry.clone();
        let retained_audit = app.display.display_audit.clone();
        let retained_diagnostics = app.retained_diagnostics.clone();

        app.solve(SketchSolveRequest::default().with_drag(ids.c, Point2::new(f64::NAN, 1.0)));

        assert!(matches!(app.attempt, AttemptSummary::Error { .. }));
        assert_eq!(app.display.geometry, retained_geometry);
        assert_eq!(app.display.display_audit, retained_audit);
        assert_eq!(app.retained_diagnostics, retained_diagnostics);
        let view = live_sketch_view(&app).unwrap();
        assert!(view.status.contains("attempt error / retained state shown"));
        assert!(view.status.contains("no candidate report available"));
        assert!(!view.status.contains("attempt termination:"));
    }

    #[test]
    fn retained_diagnostics_can_fall_back_to_audit_and_hide_invalid_rank() {
        let (mut sketch, _) = underconstrained_triangle().unwrap();
        let mut accepted = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        let audit_max = audit_hard_residual_max(&accepted.display_audit).unwrap();
        accepted.acceptance_hard_residual_max = None;
        accepted.core_report.hard_residuals_validated = false;
        accepted.core_report.hard_residual_max = f64::NAN;
        accepted.core_report.rank_is_valid = false;
        accepted.core_report.is_singular = true;

        let retained = RetainedDiagnostics::from_accepted(&accepted).unwrap();
        assert_eq!(retained.validated_hard_residual_max, Some(audit_max));
        assert_eq!(retained.rank, None);
        assert_eq!(retained.local_degrees_of_freedom, None);
        assert_eq!(retained.bounded_bidirectional_degrees_of_freedom, None);
        assert_eq!(retained.one_sided_mobility, None);
        assert_eq!(retained.is_singular, None);
    }

    #[test]
    fn incomplete_empty_diagnostics_are_never_rendered_as_none() {
        let mut sketch = Sketch::new(1.0).unwrap();
        let point = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        sketch.add_fixed_point(point).unwrap();
        let accepted = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        let mut diagnostic = accepted.core_report.conflict_diagnostics;
        diagnostic.status = DiagnosticStatus::Truncated;
        diagnostic.reason = Some(geosolve_core::DiagnosticIncompleteReason::TrialBudget);
        assert_eq!(
            diagnostic_notice(&[], diagnostic),
            "not reported (Truncated / TrialBudget)"
        );
        diagnostic.status = DiagnosticStatus::Skipped;
        diagnostic.reason = Some(geosolve_core::DiagnosticIncompleteReason::Disabled);
        assert_eq!(
            diagnostic_notice(&[], diagnostic),
            "not reported (Skipped / Disabled)"
        );
        diagnostic.status = DiagnosticStatus::Complete;
        diagnostic.reason = None;
        assert_eq!(diagnostic_notice(&[], diagnostic), "none");
    }

    #[test]
    fn radius_cue_uses_the_public_distance_dimension_target() {
        let mut app = InteractiveSketchState::new(LiveSceneKind::UnderconstrainedTriangle).unwrap();
        let ids = triangle_ids(&app);
        app.sketch
            .set_dimension_target(ids.distance_ac, 2.5)
            .unwrap();
        app.solve(SketchSolveRequest::default());
        assert!(app.display.accepted());
        let view = live_sketch_view(&app).unwrap();
        assert!(view.geometry.contains("r=\"150.000\""));
        assert!(view.geometry.contains("r = 2.500 hard manifold"));

        app.sketch.remove_dimension(ids.distance_ac).unwrap();
        let view_without_dimension = live_sketch_view(&app).unwrap();
        assert!(!view_without_dimension.geometry.contains("hard-manifold"));
        assert!(!view_without_dimension.geometry.contains("motion-cue"));
    }

    #[test]
    fn pointer_start_requires_one_primary_pointer_and_left_mouse_button() {
        assert!(pointer_start_allowed(true, "mouse", 0, false));
        assert!(pointer_start_allowed(true, "touch", 0, false));
        assert!(pointer_start_allowed(true, "pen", 2, false));
        assert!(!pointer_start_allowed(false, "touch", 0, false));
        assert!(!pointer_start_allowed(true, "mouse", 1, false));
        assert!(!pointer_start_allowed(true, "mouse", 2, false));
        assert!(!pointer_start_allowed(true, "touch", 0, true));
    }

    #[test]
    fn interaction_does_not_advertise_an_inaccessible_svg_button() {
        for kind in [
            LiveSceneKind::UnderconstrainedTriangle,
            LiveSceneKind::HorizontalRail,
            LiveSceneKind::CoincidentPair,
        ] {
            let app = InteractiveSketchState::new(kind).unwrap();
            let view = live_sketch_view(&app).unwrap();
            assert!(!view.geometry.contains("role=\"button\""));
            assert!(!view.geometry.contains("aria-pressed"));
        }
        let styles = include_str!("../styles.css");
        let viewport_rules = styles
            .split("#viewport {")
            .nth(1)
            .and_then(|rules| rules.split('}').next())
            .unwrap();
        assert!(viewport_rules.contains("touch-action: none"));
    }

    #[test]
    fn viewport_css_preserves_exact_ratio_and_hit_target_is_large_enough_when_narrow() {
        let styles = include_str!("../styles.css");
        let viewport_blocks: Vec<_> = styles
            .split("#viewport {")
            .skip(1)
            .map(|rules| rules.split('}').next().unwrap())
            .collect();
        assert_eq!(viewport_blocks.len(), 1);
        assert!(viewport_blocks[0].contains("width: 100%"));
        assert!(viewport_blocks[0].contains("aspect-ratio: 640 / 420"));
        assert!(
            viewport_blocks
                .iter()
                .all(|rules| !rules.contains("min-height"))
        );

        // A 320 px viewport leaves 302 px inside the mobile main/panel gutters.
        let narrow_viewport_width = 302.0;
        let narrow_viewport_height =
            narrow_viewport_width * SVG_VIEW_BOX.height / SVG_VIEW_BOX.width;
        assert!(
            (narrow_viewport_height / narrow_viewport_width
                - SVG_VIEW_BOX.height / SVG_VIEW_BOX.width)
                .abs()
                <= f64::EPSILON
        );
        let hit_diameter_css_px =
            2.0 * DRAG_HIT_RADIUS * narrow_viewport_width / SVG_VIEW_BOX.width;
        assert!(hit_diameter_css_px >= 44.0, "got {hit_diameter_css_px}");
    }

    #[test]
    fn l1_l2_l3_states_start_accepted_with_explicit_branches_and_valid_velocity() {
        let mut four_bar_signs = Vec::new();
        for kind in [
            LinkageSceneKind::FourBarOpen,
            LinkageSceneKind::FourBarCrossed,
            LinkageSceneKind::SliderCrank,
        ] {
            let state = InteractiveLinkageState::new(kind).unwrap();
            assert!(state.display.accepted());
            assert_eq!(
                state.display.core_report.termination,
                SolveTermination::Converged
            );
            assert!(
                state.display.acceptance_hard_residual_max.unwrap() <= 1.0e-9,
                "{kind:?}: {:?}",
                state.display.acceptance_hard_residual_max
            );
            assert!(linkage_geometry_is_finite(&state.display.geometry));
            let branch = state
                .scene
                .branch_evaluation(&state.linkage, &state.display.geometry)
                .unwrap();
            assert_eq!(branch.monitor_id, state.scene.branch_monitor());
            assert!(branch.retained);
            assert!(state.velocity.differentiated_residual_max <= 1.0e-9);
            assert!(state.velocity.rank_is_valid);
            assert!(
                state
                    .linkage
                    .driver(state.scene.driver())
                    .unwrap()
                    .max_continuation_step()
                    <= degrees_to_radians(2.0)
            );
            if matches!(state.scene, LinkageScene::FourBar(_)) {
                four_bar_signs.push(metric_sign_label(branch.signed_metric));
            }
        }
        assert_eq!(four_bar_signs, vec!["positive", "negative"]);
    }

    #[test]
    fn linkage_state_drives_low_mid_high_with_bounded_validated_continuation() {
        for (kind, targets) in [
            (LinkageSceneKind::FourBarOpen, [25.0, 80.0, 135.0]),
            (LinkageSceneKind::FourBarCrossed, [25.0, 80.0, 135.0]),
            (LinkageSceneKind::SliderCrank, [15.0, 90.0, 165.0]),
        ] {
            let mut state = InteractiveLinkageState::new(kind).unwrap();
            let max_step = state
                .linkage
                .driver(state.scene.driver())
                .unwrap()
                .max_continuation_step();
            for target in targets {
                state.drive_to_degrees(target);
                assert!(matches!(
                    state.attempt,
                    LinkageAttemptSummary::Accepted { .. }
                ));
                let summary = state.continuation.as_ref().unwrap();
                assert!(summary.completed, "{kind:?} at {target}: {summary:#?}");
                assert!(!summary.samples.is_empty());
                assert!((radians_to_degrees(summary.accepted_target) - target).abs() <= 1.0e-10);
                for sample in &summary.samples {
                    assert!(sample.accepted, "{kind:?} at {target}: {sample:#?}");
                    assert_eq!(sample.termination, SolveTermination::Converged);
                    assert!(sample.step.abs() <= max_step * (1.0 + 1.0e-14));
                    assert!(sample.hard_residual_max.unwrap() <= 1.0e-9);
                    let branch = sample.checks.branch_evaluation.unwrap();
                    assert_eq!(branch.monitor_id, state.scene.branch_monitor());
                    assert!(branch.retained);
                    assert!(sample.checks.geometry_is_finite);
                    assert!(
                        sample.checks.render_points_inside,
                        "render bounds failed for {kind:?} at {target}: {sample:#?}"
                    );
                    assert!(sample.target.is_finite());
                }
                assert!(state.display.accepted());
                assert!(
                    state
                        .scene
                        .branch_evaluation(&state.linkage, &state.display.geometry)
                        .unwrap()
                        .retained
                );
                assert!(linkage_geometry_is_finite(&state.display.geometry));
                assert!(state.velocity.differentiated_residual_max <= 1.0e-9);
            }
        }
    }

    #[test]
    fn linkage_rendering_uses_display_geometry_audit_and_driver_source_identity() {
        for (kind, geometry_class, mode, min, max) in [
            (
                LinkageSceneKind::FourBarOpen,
                "four-bar-geometry",
                "Open assembly",
                25,
                135,
            ),
            (
                LinkageSceneKind::FourBarCrossed,
                "four-bar-geometry",
                "Crossed assembly",
                25,
                135,
            ),
            (
                LinkageSceneKind::SliderCrank,
                "slider-crank-geometry",
                "Positive-X assembly",
                15,
                165,
            ),
        ] {
            let state = InteractiveLinkageState::new(kind).unwrap();
            let view = live_linkage_view(&state).unwrap();
            let branch = state
                .scene
                .branch_evaluation(&state.linkage, &state.display.geometry)
                .unwrap();
            let row_count = audit_row_count(&state.display.display_audit);
            let driver_count = state
                .display
                .source_mappings
                .iter()
                .filter(|mapping| matches!(mapping.source, LinkageSource::Driver(_)))
                .count();
            assert!(view.geometry.contains(geometry_class));
            assert!(view.geometry.contains(mode));
            assert!(
                view.geometry
                    .contains(branch_sign_label(branch.expected_sign))
            );
            assert!(
                view.geometry
                    .contains(&format!("{:.6}", branch.signed_metric))
            );
            assert!(view.geometry.contains("retained yes"));
            assert!(view.geometry.contains("data-model-x="));
            assert!(view.geometry.contains("driver-angle-cue"));
            assert_eq!(view.audit.matches("class=\"audit-row\"").count(), row_count);
            assert_eq!(
                view.audit.matches("class=\"evaluation evaluated\"").count(),
                row_count
            );
            assert_eq!(view.audit.matches("driver-source").count(), driver_count);
            assert_eq!(
                view.audit.matches("data-linkage-driver=\"true\"").count(),
                driver_count
            );
            assert!(view.audit.contains("driver / hard"));
            assert!(view.status.contains("rank warning"));
            assert!(
                view.status
                    .contains("smallest/largest singular value ratio")
            );
            assert!(view.status.contains("unit angular-rate velocity residual"));
            assert!(view.status.contains("retained conflict candidates"));
            assert!(
                view.status
                    .contains("not reported (Skipped / HardConstraintsValid)")
            );
            assert!(view.status.contains("retained redundancy notices"));
            assert!(view.status.contains(&format!("{:?}", branch.kind)));
            assert!(view.status.contains(&format!("{:?}", branch.monitor_id)));
            assert!(view.status.contains("domain branch retained"));
            assert_eq!(
                view.announcement,
                "Linkage accepted. Position and unit-rate velocity validated."
            );
            assert_eq!(view.driver_control.min, min);
            assert_eq!(view.driver_control.max, max);
            assert_eq!(view.driver_control.step, 1);
            assert!(
                (view.driver_control.value - if max == 135 { 60.0 } else { 45.0 }).abs() <= 1.0e-10
            );
        }
        let source = include_str!("lib.rs").to_ascii_lowercase();
        assert!(!source.contains(&["static", " preview"].concat()));
        assert!(!source.contains(&["live domain", " audit arrives"].concat()));
    }

    #[test]
    fn exact_toggle_failure_keeps_retained_linkage_display_and_diagnostics() {
        let mut state = InteractiveLinkageState::new(LinkageSceneKind::FourBarOpen).unwrap();
        let near_toggle_degrees = radians_to_degrees(std::f64::consts::PI - 1.0e-3);
        state.drive_to_degrees(near_toggle_degrees);
        assert!(matches!(
            state.attempt,
            LinkageAttemptSummary::Accepted { .. }
        ));
        let retained_geometry = state.display.geometry.clone();
        let retained_audit = state.display.display_audit.clone();
        let retained_diagnostics = state.retained_diagnostics.clone();
        let retained_velocity = state.velocity.clone();
        let retained_degrees = state.driver_degrees().unwrap();

        state.drive_to_degrees(180.0);

        assert!(matches!(
            state.attempt,
            LinkageAttemptSummary::Rejected { .. }
        ));
        let summary = state.continuation.as_ref().unwrap();
        assert!(!summary.completed);
        assert_eq!(summary.samples.len(), 1);
        assert!(!summary.samples[0].accepted);
        assert_eq!(state.display.geometry, retained_geometry);
        assert_eq!(state.display.display_audit, retained_audit);
        assert_eq!(state.retained_diagnostics, retained_diagnostics);
        assert_eq!(state.velocity, retained_velocity);
        assert!((state.driver_degrees().unwrap() - retained_degrees).abs() <= 1.0e-12);

        let view = live_linkage_view(&state).unwrap();
        assert!(
            view.status
                .contains("attempt rejected / retained state shown")
        );
        assert!(view.status.contains("requested 180.000 deg"));
        assert_eq!(
            view.audit.matches("class=\"audit-row\"").count(),
            audit_row_count(&retained_audit)
        );
        assert!(view.geometry.contains("four-bar-geometry"));
    }

    #[test]
    fn accepted_position_with_forced_velocity_failure_rolls_back_atomically() {
        let mut state = InteractiveLinkageState::new(LinkageSceneKind::FourBarOpen).unwrap();
        let retained_target = state.linkage.driver(state.scene.driver()).unwrap().target();
        let retained_domain_geometry = state.linkage.geometry().unwrap();
        let retained_display_geometry = state.display.geometry.clone();
        let retained_audit = state.display.display_audit.clone();
        let retained_mappings = state.display.source_mappings.clone();
        let retained_acceptance_max = state.display.acceptance_hard_residual_max;
        let retained_report = (
            state.display.core_report.termination,
            state.display.core_report.hard_residual_max,
            state.display.core_report.rank,
            state.display.core_report.local_degrees_of_freedom,
            state.display.core_report.iterations,
        );
        let retained_diagnostics = state.retained_diagnostics.clone();
        let retained_velocity = state.velocity.clone();
        let velocity_called = std::cell::Cell::new(false);

        state.drive_to_degrees_with_velocity(80.0, |_, _| {
            velocity_called.set(true);
            Err("forced unit-rate velocity failure".to_owned())
        });

        assert!(velocity_called.get());
        let LinkageAttemptSummary::VelocityValidationFailed {
            termination,
            position_target,
            retained_target: summary_retained_target,
            message,
        } = &state.attempt
        else {
            panic!("expected an atomic velocity rollback: {:#?}", state.attempt);
        };
        assert_eq!(*termination, SolveTermination::Converged);
        assert!((radians_to_degrees(*position_target) - 80.0).abs() <= 1.0e-10);
        assert_eq!(summary_retained_target.to_bits(), retained_target.to_bits());
        assert!(message.contains("forced unit-rate velocity failure"));

        let summary = state.continuation.as_ref().unwrap();
        assert!(!summary.completed);
        assert!(summary.samples.iter().all(|sample| sample.accepted));
        assert_eq!(summary.initial_target.to_bits(), retained_target.to_bits());
        assert_eq!(summary.accepted_target.to_bits(), retained_target.to_bits());
        assert_eq!(
            state
                .linkage
                .driver(state.scene.driver())
                .unwrap()
                .target()
                .to_bits(),
            retained_target.to_bits()
        );
        assert_eq!(state.linkage.geometry().unwrap(), retained_domain_geometry);
        assert_eq!(state.display.geometry, retained_display_geometry);
        assert_eq!(state.display.display_audit, retained_audit);
        assert_eq!(state.display.source_mappings, retained_mappings);
        assert_eq!(
            state.display.acceptance_hard_residual_max,
            retained_acceptance_max
        );
        assert_eq!(
            (
                state.display.core_report.termination,
                state.display.core_report.hard_residual_max,
                state.display.core_report.rank,
                state.display.core_report.local_degrees_of_freedom,
                state.display.core_report.iterations,
            ),
            retained_report
        );
        assert_eq!(state.retained_diagnostics, retained_diagnostics);
        assert_eq!(state.velocity, retained_velocity);

        let view = live_linkage_view(&state).unwrap();
        assert!(
            view.status
                .contains("position accepted / velocity failed / rolled back")
        );
        assert!(view.status.contains("attempted accepted target 80.000 deg"));
        assert!(view.status.contains("retained target 60.000 deg"));
        assert!(
            view.status
                .contains("retained driver target</span><strong>60.000 deg")
        );
        assert_eq!(
            view.announcement,
            "Position accepted, but velocity validation failed. The prior linkage state was restored."
        );
    }

    #[test]
    fn linkage_degree_controls_and_scene_transforms_are_pure_and_accessible() {
        for degrees in [-720.0, -1.0, 0.0, 15.0, 60.0, 135.0, 720.0] {
            let round_trip = radians_to_degrees(degrees_to_radians(degrees));
            assert!((round_trip - degrees).abs() <= 1.0e-12);
        }
        for kind in [
            LinkageSceneKind::FourBarOpen,
            LinkageSceneKind::FourBarCrossed,
            LinkageSceneKind::SliderCrank,
        ] {
            let transform = kind.transform();
            for point in [
                Point2::new(0.0, 0.0),
                Point2::new(1.25, 0.75),
                Point2::new(4.0, -1.5),
            ] {
                let round_trip = transform.svg_to_model(transform.model_to_svg(point));
                assert!((round_trip - point).norm() <= 1.0e-12);
            }
            let state = InteractiveLinkageState::new(kind).unwrap();
            assert!(scene_geometry_inside_view_box(
                state.scene,
                &state.display.geometry,
                30.0
            ));
        }

        let page = include_str!("../index.html");
        assert!(page.contains("id=\"driver-controls\""));
        assert!(page.contains("aria-label=\"Linkage driver\""));
        assert!(page.contains("<label for=\"driver-angle\">"));
        assert!(page.contains("<output id=\"driver-output\" for=\"driver-angle\">"));
        assert!(page.contains("id=\"driver-angle\""));
        assert!(page.contains("type=\"range\""));
        assert!(page.contains("step=\"1\""));
        assert!(page.contains("disabled"));
        assert!(page.contains("hidden"));
        assert!(page.contains(
            "id=\"solve-announcement\" class=\"visually-hidden\" aria-live=\"polite\" aria-atomic=\"true\""
        ));
        let detailed_status = page
            .split("<section id=\"solve-status\"")
            .nth(1)
            .and_then(|markup| markup.split("></section>").next())
            .unwrap();
        assert!(!detailed_status.contains("aria-live"));
        let styles = include_str!("../styles.css");
        assert!(styles.contains(".driver-controls[hidden]"));
        assert!(styles.contains("#driver-angle:focus-visible"));
        assert!(styles.contains(".visually-hidden"));
    }

    #[test]
    fn dynamic_audit_strings_are_html_escaped() {
        assert_eq!(escape_html("<&\"'>"), "&lt;&amp;&quot;&#39;&gt;".to_owned());
    }
}
