//! WASM/SVG visual harness for live sketch and linkage verification fixtures.

#[cfg(any(target_arch = "wasm32", test))]
use std::fmt::Write as _;

#[cfg(any(target_arch = "wasm32", test))]
use geosolve_core::{
    AuditAnnotations, AuditEvaluationStatus, AuditSnapshot, ResidualCategory, SolveTermination,
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
    ConflictingRectangleIds, DimensionKind, DimensionMode, PointId, SegmentId, Sketch,
    SketchDimensionId, SketchSolveRequest, SketchSolveResult, SketchSource,
    UnderconstrainedTriangleIds, conflicting_rectangle, underconstrained_triangle,
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
    HorizontalRail,
    CoincidentPair,
}

#[cfg(any(target_arch = "wasm32", test))]
impl LiveSceneKind {
    const fn scenario(self) -> DemoScenario {
        match self {
            Self::UnderconstrainedTriangle => DemoScenario::UnderconstrainedTriangle,
            Self::HorizontalRail => DemoScenario::HorizontalRail,
            Self::CoincidentPair => DemoScenario::CoincidentPair,
        }
    }

    const fn badge(self) -> &'static str {
        match self {
            Self::UnderconstrainedTriangle => "live S1",
            Self::HorizontalRail => "live rail",
            Self::CoincidentPair => "live coincident",
        }
    }

    const fn instructions(self) -> &'static str {
        match self {
            Self::UnderconstrainedTriangle => {
                "Drag point C with a mouse, pen, or touch. It is projected onto the distance hard manifold; release keeps the accepted nearby position. A is fixed and B is free."
            }
            Self::HorizontalRail => {
                "Drag point B with a mouse, pen, or touch. The hard horizontal constraint projects it onto the rail; release keeps the accepted position. The displayed length is an equation-free reference measurement."
            }
            Self::CoincidentPair => {
                "Drag the inner B mark with a mouse, pen, or touch. The hard coincidence relation moves A and B together to the target; release keeps their common position."
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
enum LiveScene {
    UnderconstrainedTriangle(UnderconstrainedTriangleIds),
    HorizontalRail(HorizontalRailIds),
    CoincidentPair(CoincidentPairIds),
}

#[cfg(any(target_arch = "wasm32", test))]
impl LiveScene {
    const fn kind(self) -> LiveSceneKind {
        match self {
            Self::UnderconstrainedTriangle(_) => LiveSceneKind::UnderconstrainedTriangle,
            Self::HorizontalRail(_) => LiveSceneKind::HorizontalRail,
            Self::CoincidentPair(_) => LiveSceneKind::CoincidentPair,
        }
    }

    const fn draggable_point(self) -> PointId {
        match self {
            Self::UnderconstrainedTriangle(ids) => ids.c,
            Self::HorizontalRail(ids) => ids.b,
            Self::CoincidentPair(ids) => ids.b,
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn build_live_scene(kind: LiveSceneKind) -> Result<(Sketch, LiveScene), String> {
    match kind {
        LiveSceneKind::UnderconstrainedTriangle => {
            let (sketch, ids) = underconstrained_triangle().map_err(|error| error.to_string())?;
            Ok((sketch, LiveScene::UnderconstrainedTriangle(ids)))
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
        LiveSceneKind::UnderconstrainedTriangle => point,
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
    Some(MODEL_TRANSFORM.svg_to_model(clamp_drag_svg_point(kind, svg)))
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
    iterations: usize,
    is_singular: Option<bool>,
    conflict_sources: Vec<String>,
    redundancy_sources: Vec<String>,
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
            local_degrees_of_freedom: report
                .rank_is_valid
                .then_some(report.local_degrees_of_freedom),
            iterations: report.iterations,
            is_singular: report.rank_is_valid.then_some(report.is_singular),
            conflict_sources: source_labels(&report.conflicting_sources, &result.display_audit),
            redundancy_sources: source_labels(
                &report.sources_containing_redundant_rows,
                &result.display_audit,
            ),
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
        rejection: String,
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
        let request = SketchSolveRequest::default().with_drag(self.scene.draggable_point(), target);
        self.solve(request);
    }

    fn finish_drag(&mut self) {
        self.active_pointer = None;
        self.solve(SketchSolveRequest::default());
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

    fn apply_result(&mut self, result: SketchSolveResult) {
        if result.accepted() {
            if let Some(diagnostics) = RetainedDiagnostics::from_accepted(&result) {
                self.retained_diagnostics = diagnostics;
            }
            self.attempt = AttemptSummary::Accepted {
                termination: result.core_report.termination,
            };
        } else {
            self.attempt = AttemptSummary::Rejected {
                termination: result.core_report.termination,
                rejection: format!("{:?}", result.rejection),
            };
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
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn evaluation_markup(status: AuditEvaluationStatus, error: Option<&str>) -> String {
    let (class, label) = match status {
        AuditEvaluationStatus::Evaluated => ("evaluated", "evaluated"),
        AuditEvaluationStatus::Failed => ("failed", "failed"),
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
    let attempt = attempt_markup(attempt);
    let validated_residual = retained
        .validated_hard_residual_max
        .map_or_else(|| "unavailable".to_owned(), format_metric);
    let rank = retained
        .rank
        .map_or_else(|| "unavailable".to_owned(), |rank| rank.to_string());
    let dof = retained
        .local_degrees_of_freedom
        .map_or_else(|| "unavailable".to_owned(), |dof| dof.to_string());
    let (motion_label, motion_state) = scene_motion_state(sketch, scene);
    let conflicts = text_notice(&retained.conflict_sources);
    let redundancies = text_notice(&retained.redundancy_sources);
    let singularity =
        retained.is_singular.map_or(
            "unavailable",
            |is_singular| {
                if is_singular { "yes" } else { "none" }
            },
        );
    let mut html = String::new();
    write!(
        html,
        r#"{}
            <div class="status-grid">
                <div><span>retained termination</span><strong>{}</strong></div>
                <div><span>retained validated max hard residual</span><strong>{}</strong></div>
                <div><span>retained rank</span><strong>{}</strong></div>
                <div><span>retained local DOF</span><strong>{}</strong></div>
                <div><span>retained total iterations</span><strong>{}</strong></div>
                <div><span>{}</span><strong>{}</strong></div>
                <div><span>retained singularity</span><strong>{}</strong></div>
                <div><span>retained conflict candidates</span><strong>{}</strong></div>
                <div><span>retained redundancy notices</span><strong>{}</strong></div>
            </div>{}"#,
        attempt,
        termination_label(retained.termination),
        validated_residual,
        rank,
        dof,
        retained.iterations,
        motion_label,
        escape_html(&motion_state),
        singularity,
        conflicts,
        redundancies,
        reference_status_markup(scene, display),
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
        text_notice(&retained.conflict_sources),
        text_notice(&retained.redundancy_sources),
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
    }
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
                "attempt termination: {}; rejection: {}",
                termination_label(*termination),
                rejection
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
        SvgPoint, client_to_drag_target, expected_conflict_view, live_linkage_view,
        live_sketch_view, pointer_start_allowed,
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
        let driver = required_element(document, "driver-angle")?.dyn_into::<HtmlInputElement>()?;
        let output =
            required_element(document, "driver-output")?.dyn_into::<HtmlOutputElement>()?;

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
            }
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
        let select = required_element(&document, "scenario")?.dyn_into::<HtmlSelectElement>()?;
        let viewport = required_element(&document, "viewport")?;
        let driver = required_element(&document, "driver-angle")?.dyn_into::<HtmlInputElement>()?;
        let app = Rc::new(RefCell::new(DemoApp {
            state: DemoState::Sketch(Box::new(
                InteractiveSketchState::new(LiveSceneKind::UnderconstrainedTriangle)
                    .map_err(|error| JsValue::from_str(&error))?,
            )),
        }));

        render(&document, &app.borrow())?;
        install_scenario_listener(&document, &select, &viewport, &app)?;
        install_pointer_listeners(&document, &viewport, &app)?;
        install_driver_listener(&document, &driver, &app)?;
        Ok(())
    }
}

/// Number of scenarios selectable in the browser harness.
#[must_use]
pub const fn scenario_count() -> usize {
    7
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

    fn display_audit_row_count(result: &SketchSolveResult) -> usize {
        audit_row_count(&result.display_audit)
    }

    fn audit_row_count(audit: &AuditSnapshot) -> usize {
        audit.sources.iter().map(|source| source.rows.len()).sum()
    }

    fn assert_drag_handle_inside_margin(app: &InteractiveSketchState) {
        let point = app
            .display
            .geometry
            .point(app.scene.draggable_point())
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
        assert!(view.status.contains("local DOF</span><strong>1"));
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
    fn all_seven_selectors_map_to_fresh_domain_scene_kinds() {
        assert_eq!(scenario_count(), 7);
        for scenario in [
            DemoScenario::UnderconstrainedTriangle,
            DemoScenario::ConflictingRectangle,
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
        assert!(page.contains("value=\"four-bar-open\""));
        assert!(page.contains("value=\"four-bar-crossed\""));
        assert!(page.contains("value=\"slider-crank\""));

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
        let DemoState::ExpectedConflict(conflict) = conflict_state else {
            panic!("expected retained S2 state");
        };
        assert_eq!(
            conflict.conflicts[0].source,
            SketchSource::Dimension(conflict.ids.width_4)
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
        assert!(rail_view.status.contains("local DOF</span><strong>1"));

        let coincident = InteractiveSketchState::new(LiveSceneKind::CoincidentPair).unwrap();
        let coincident_view = live_sketch_view(&coincident).unwrap();
        assert!(coincident_view.geometry.contains("coincident-point-a"));
        assert!(coincident_view.geometry.contains("coincident-point-b"));
        assert!(coincident_view.status.contains("local DOF</span><strong>2"));
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
        assert_eq!(retained.is_singular, None);
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
