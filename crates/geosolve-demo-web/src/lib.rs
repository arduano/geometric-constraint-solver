//! WASM/SVG visual harness for the first live sketch scenario.

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
use geosolve_sketch::{
    DimensionKind, Sketch, SketchSolveRequest, SketchSolveResult, UnderconstrainedTriangleIds,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoScenario {
    UnderconstrainedTriangle,
    FourBar,
    SliderCrank,
}

#[cfg(any(target_arch = "wasm32", test))]
impl DemoScenario {
    fn from_value(value: &str) -> Self {
        match value {
            "four-bar" => Self::FourBar,
            "slider-crank" => Self::SliderCrank,
            _ => Self::UnderconstrainedTriangle,
        }
    }

    const fn placeholder_svg(self) -> Option<&'static str> {
        match self {
            Self::UnderconstrainedTriangle => None,
            Self::FourBar => Some(
                r#"<g class="geometry linkage-placeholder">
                    <path d="M 125 330 L 245 215 L 430 245 L 500 330" />
                    <path d="M 125 330 L 500 330" class="ground" />
                    <circle cx="125" cy="330" r="8" />
                    <circle cx="245" cy="215" r="8" />
                    <circle cx="430" cy="245" r="8" />
                    <circle cx="500" cy="330" r="8" />
                    <text x="28" y="46" class="scene-kicker">M6 STATIC PREVIEW</text>
                    <text x="28" y="74" class="scene-title">Four-bar / open assembly</text>
                </g>"#,
            ),
            Self::SliderCrank => Some(
                r#"<g class="geometry linkage-placeholder">
                    <path d="M 145 280 L 270 195 L 465 280" />
                    <path d="M 100 280 L 520 280" class="ground" />
                    <rect x="440" y="252" width="50" height="56" rx="5" />
                    <circle cx="145" cy="280" r="8" />
                    <circle cx="270" cy="195" r="8" />
                    <circle cx="465" cy="280" r="8" />
                    <text x="28" y="46" class="scene-kicker">M6 STATIC PREVIEW</text>
                    <text x="28" y="74" class="scene-title">Slider-crank / positive-x assembly</text>
                </g>"#,
            ),
        }
    }

    const fn placeholder_audit(self) -> Option<&'static str> {
        match self {
            Self::UnderconstrainedTriangle => None,
            Self::FourBar => Some(
                r#"<article class="placeholder-note">
                    <span class="kind placeholder">M6</span>
                    <h3>Four-bar is a static preview</h3>
                    <p>Live domain audit arrives in M6.</p>
                </article>"#,
            ),
            Self::SliderCrank => Some(
                r#"<article class="placeholder-note">
                    <span class="kind placeholder">M6</span>
                    <h3>Slider-crank is a static preview</h3>
                    <p>Live domain audit arrives in M6.</p>
                </article>"#,
            ),
        }
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
struct TriangleApp {
    sketch: Sketch,
    ids: UnderconstrainedTriangleIds,
    display: SketchSolveResult,
    retained_diagnostics: RetainedDiagnostics,
    attempt: AttemptSummary,
    drag_pointer: Option<i32>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl TriangleApp {
    fn new() -> Result<Self, String> {
        let (mut sketch, ids) = underconstrained_triangle().map_err(|error| error.to_string())?;
        let display = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .map_err(|error| error.to_string())?;
        if !display.accepted() {
            return Err(format!(
                "canonical S1 initial solve was rejected: {:?}",
                display.rejection
            ));
        }
        let retained_diagnostics = RetainedDiagnostics::from_accepted(&display)
            .ok_or_else(|| "canonical S1 accepted result has no diagnostics".to_owned())?;
        let attempt = AttemptSummary::Accepted {
            termination: display.core_report.termination,
        };
        Ok(Self {
            sketch,
            ids,
            display,
            retained_diagnostics,
            attempt,
            drag_pointer: None,
        })
    }

    #[cfg(target_arch = "wasm32")]
    fn solve_drag(&mut self, target: Point2<f64>) {
        let request = SketchSolveRequest::default().with_drag(self.ids.c, target);
        self.solve(request);
    }

    #[cfg(target_arch = "wasm32")]
    fn finish_drag(&mut self) {
        self.drag_pointer = None;
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

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct DemoApp {
    scenario: DemoScenario,
    triangle: TriangleApp,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, PartialEq)]
struct TriangleView {
    geometry: String,
    audit: String,
    status: String,
}

#[cfg(any(target_arch = "wasm32", test))]
fn triangle_view(app: &TriangleApp) -> Result<TriangleView, String> {
    Ok(TriangleView {
        geometry: triangle_geometry_markup(
            &app.sketch,
            &app.display,
            app.ids,
            app.drag_pointer.is_some(),
        )?,
        audit: audit_markup(&app.display.display_audit),
        status: status_markup(
            &app.sketch,
            app.ids,
            &app.retained_diagnostics,
            &app.attempt,
        ),
    })
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
            <circle id="point-c" class="drag-target{}" cx="{:.3}" cy="{:.3}" r="22"
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
fn audit_markup(audit: &AuditSnapshot) -> String {
    let mut html = String::new();
    for source in &audit.sources {
        let category = source
            .rows
            .first()
            .map_or("empty", |row| category_label(row.category));
        write!(
            html,
            r#"<article class="constraint source-group {}" data-source-id="{}">
                <header class="source-header">
                    <div><span class="source-id">{}</span><h3>{}</h3></div>
                    <span class="kind {}">{}</span>
                </header>
                <div class="source-diagnostics"><span>source diagnostics</span>{}</div>"#,
            category,
            escape_html(&format!("{:?}", source.source_id)),
            escape_html(&format!("{:?}", source.source_id)),
            escape_html(&source.source_label),
            category,
            category,
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
    ids: UnderconstrainedTriangleIds,
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
    let branch = branch_label(sketch, ids);
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
                <div><span>retained AB branch</span><strong>{}</strong></div>
                <div><span>retained singularity</span><strong>{}</strong></div>
                <div><span>retained conflict candidates</span><strong>{}</strong></div>
                <div><span>retained redundancy notices</span><strong>{}</strong></div>
            </div>"#,
        attempt,
        termination_label(retained.termination),
        validated_residual,
        rank,
        dof,
        retained.iterations,
        escape_html(&branch),
        singularity,
        conflicts,
        redundancies,
    )
    .expect("writing solve status markup to a String cannot fail");
    html
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
fn branch_label(sketch: &Sketch, ids: UnderconstrainedTriangleIds) -> String {
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
        ClientRect, DemoApp, DemoScenario, MODEL_TRANSFORM, SVG_VIEW_BOX, SvgPoint, TriangleApp,
        client_to_svg, pointer_start_allowed, triangle_view,
    };
    use geosolve_geometry::Point2;
    use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
    use web_sys::{Document, Element, Event, HtmlSelectElement, PointerEvent};

    fn required_element(document: &Document, id: &str) -> Result<Element, JsValue> {
        document
            .get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str(&format!("missing #{id} element")))
    }

    fn render(document: &Document, app: &DemoApp) -> Result<(), JsValue> {
        let viewport = required_element(document, "viewport")?;
        let equations = required_element(document, "equations")?;
        let status = required_element(document, "solve-status")?;
        let badge = required_element(document, "audit-badge")?;
        let instructions = required_element(document, "drag-instructions")?;

        match app.scenario {
            DemoScenario::UnderconstrainedTriangle => {
                let view =
                    triangle_view(&app.triangle).map_err(|error| JsValue::from_str(&error))?;
                viewport.set_inner_html(&view.geometry);
                equations.set_inner_html(&view.audit);
                status.set_inner_html(&view.status);
                badge.set_text_content(Some("live S1"));
                badge.set_class_name("live-badge");
                instructions.set_text_content(Some(
                    "Drag point C with a mouse, pen, or touch. It is projected onto the distance hard manifold; release keeps the accepted nearby position. A is fixed and B is free.",
                ));
            }
            scenario => {
                viewport.set_inner_html(
                    scenario
                        .placeholder_svg()
                        .ok_or_else(|| JsValue::from_str("missing placeholder SVG"))?,
                );
                equations.set_inner_html(
                    scenario
                        .placeholder_audit()
                        .ok_or_else(|| JsValue::from_str("missing placeholder audit"))?,
                );
                status.set_inner_html(
                    r#"<div class="attempt-banner placeholder"><strong>M6 placeholder</strong><span>No solver attempt is run for this static preview.</span></div>"#,
                );
                badge.set_text_content(Some("M6 placeholder"));
                badge.set_class_name("live-badge placeholder");
                instructions.set_text_content(Some(
                    "This linkage is a non-interactive M6 preview. Select Underconstrained triangle for the live S1 drag interaction.",
                ));
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

    fn pointer_model_position(event: &PointerEvent, viewport: &Element) -> Option<Point2<f64>> {
        let bounds = viewport.get_bounding_client_rect();
        let svg = client_to_svg(
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
            SVG_VIEW_BOX,
        )?;
        Some(MODEL_TRANSFORM.svg_to_model(svg))
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
            if let Some(pointer_id) = state.triangle.drag_pointer {
                let _ = callback_viewport.release_pointer_capture(pointer_id);
                state.triangle.finish_drag();
            }
            state.scenario = DemoScenario::from_value(&callback_select.value());
            drop(state);
            render_shared(&callback_document, &callback_app);
        });
        select.add_event_listener_with_callback("change", callback.as_ref().unchecked_ref())?;
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
            let (is_triangle, drag_active) = {
                let state = callback_app.borrow();
                (
                    state.scenario == DemoScenario::UnderconstrainedTriangle,
                    state.triangle.drag_pointer.is_some(),
                )
            };
            let is_point_c = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .is_some_and(|target| target.id() == "point-c");
            if !is_triangle
                || !is_point_c
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
            callback_app.borrow_mut().triangle.drag_pointer = Some(event.pointer_id());
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
            let active = callback_app.borrow().triangle.drag_pointer == Some(event.pointer_id());
            if !active {
                return;
            }
            event.prevent_default();
            let Some(target) = pointer_model_position(&event, &callback_viewport) else {
                return;
            };
            callback_app.borrow_mut().triangle.solve_drag(target);
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
            let active = callback_app.borrow().triangle.drag_pointer == Some(event.pointer_id());
            if !active {
                return;
            }
            event.prevent_default();
            let _ = callback_viewport.release_pointer_capture(event.pointer_id());
            callback_app.borrow_mut().triangle.finish_drag();
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
        let app = Rc::new(RefCell::new(DemoApp {
            scenario: DemoScenario::UnderconstrainedTriangle,
            triangle: TriangleApp::new().map_err(|error| JsValue::from_str(&error))?,
        }));

        render(&document, &app.borrow())?;
        install_scenario_listener(&document, &select, &viewport, &app)?;
        install_pointer_listeners(&document, &viewport, &app)?;
        Ok(())
    }
}

/// Number of scenarios selectable in the browser harness.
#[must_use]
pub const fn scenario_count() -> usize {
    3
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

    #[test]
    fn live_s1_view_comes_from_an_accepted_sketch_result() {
        let app = TriangleApp::new().unwrap();
        assert!(app.display.accepted());
        let view = triangle_view(&app).unwrap();
        assert!(view.geometry.contains("LIVE S1 / SOLVED SKETCH"));
        assert!(view.geometry.contains("data-model-x="));
        assert!(view.audit.contains("data-category=\"hard\""));
        assert!(view.audit.contains("data-category=\"preference\""));
        assert!(view.audit.contains("raw residual"));
        assert!(view.audit.contains("normalized"));
        assert!(view.audit.contains("evaluated"));
        assert!(view.status.contains("local DOF</span><strong>1"));
    }

    #[test]
    fn s1_has_no_static_audit_or_handwritten_equation_templates() {
        assert_eq!(
            DemoScenario::UnderconstrainedTriangle.placeholder_audit(),
            None
        );
        assert_eq!(
            DemoScenario::UnderconstrainedTriangle.placeholder_svg(),
            None
        );
        let source = include_str!("lib.rs");
        let old_horizontal = ["B.y", " - A.y"].concat();
        let old_distance = ["B - A", "|| - 4"].concat();
        assert!(!source.contains(&old_horizontal));
        assert!(!source.contains(&old_distance));
    }

    #[test]
    fn m6_placeholder_scenarios_remain_without_equation_markup() {
        assert_eq!(scenario_count(), 3);
        for scenario in [DemoScenario::FourBar, DemoScenario::SliderCrank] {
            let svg = scenario.placeholder_svg().unwrap();
            let audit = scenario.placeholder_audit().unwrap();
            assert!(svg.contains("M6 STATIC PREVIEW"));
            assert!(audit.contains("Live domain audit arrives in M6"));
            assert!(!audit.contains("<code"));
        }
        assert_eq!(
            DemoScenario::from_value("slider-crank"),
            DemoScenario::SliderCrank
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
    fn rejected_attempt_renders_retained_geometry_and_display_audit() {
        let mut app = TriangleApp::new().unwrap();
        let retained_diagnostics = app.retained_diagnostics.clone();
        app.sketch
            .set_point_position(app.ids.b, Point2::new(-3.0, 0.0))
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

        let view = triangle_view(&app).unwrap();
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
        let mut app = TriangleApp::new().unwrap();
        let retained_geometry = app.display.geometry.clone();
        let retained_audit = app.display.display_audit.clone();
        let retained_diagnostics = app.retained_diagnostics.clone();

        app.solve(SketchSolveRequest::default().with_drag(app.ids.c, Point2::new(f64::NAN, 1.0)));

        assert!(matches!(app.attempt, AttemptSummary::Error { .. }));
        assert_eq!(app.display.geometry, retained_geometry);
        assert_eq!(app.display.display_audit, retained_audit);
        assert_eq!(app.retained_diagnostics, retained_diagnostics);
        let view = triangle_view(&app).unwrap();
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
        let mut app = TriangleApp::new().unwrap();
        app.sketch
            .set_dimension_target(app.ids.distance_ac, 2.5)
            .unwrap();
        app.solve(SketchSolveRequest::default());
        assert!(app.display.accepted());
        let view = triangle_view(&app).unwrap();
        assert!(view.geometry.contains("r=\"150.000\""));
        assert!(view.geometry.contains("r = 2.500 hard manifold"));

        app.sketch.remove_dimension(app.ids.distance_ac).unwrap();
        let view_without_dimension = triangle_view(&app).unwrap();
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
        let app = TriangleApp::new().unwrap();
        let view = triangle_view(&app).unwrap();
        assert!(!view.geometry.contains("role=\"button\""));
        assert!(!view.geometry.contains("aria-pressed"));
        let styles = include_str!("../styles.css");
        let viewport_rules = styles
            .split("#viewport {")
            .nth(1)
            .and_then(|rules| rules.split('}').next())
            .unwrap();
        assert!(viewport_rules.contains("touch-action: none"));
    }

    #[test]
    fn dynamic_audit_strings_are_html_escaped() {
        assert_eq!(escape_html("<&\"'>"), "&lt;&amp;&quot;&#39;&gt;".to_owned());
    }
}
