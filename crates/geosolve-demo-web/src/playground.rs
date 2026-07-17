#![cfg_attr(test, allow(dead_code))]

use std::f64::consts::TAU;
use std::fmt::Write as _;

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_sketch::{
    AlphaPerformanceSize, AlphaScenarioKind, ContactId, ContactNeighborhood, ContactStateEdit,
    CurveDefinition, CurveSpan, DesignPointId, DocumentAngleOrientation, DocumentArcSweep,
    DocumentCommand, DocumentCommandEffect, DocumentConstraintDefinition, DocumentConstraintId,
    DocumentDimensionDefinition, DocumentDimensionId, DocumentDimensionMode, DocumentEdit,
    DocumentObjectId, DocumentSolveRequest, DocumentSolveResult, PersistentId, ScalarDomain,
    ScalarUnit, SketchDocument, SketchDocumentSession, TangentOrientation,
    alpha_performance_document, alpha_scenario,
};

const CANVAS_WIDTH: f64 = 1000.0;
const CANVAS_HEIGHT: f64 = 700.0;
const CURVE_SAMPLES: u32 = 48;
const HIT_RADIUS_PX: f64 = 14.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DrawTool {
    Point,
    Line,
    Polyline,
    Rectangle,
    Circle,
    Arc,
    Quadratic,
    Cubic,
}

impl DrawTool {
    const fn label(self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::Line => "Line",
            Self::Polyline => "Polyline",
            Self::Rectangle => "Rectangle",
            Self::Circle => "Circle",
            Self::Arc => "Arc",
            Self::Quadratic => "Quadratic Bezier",
            Self::Cubic => "Cubic Bezier",
        }
    }

    const fn required_points(self) -> Option<usize> {
        match self {
            Self::Point => Some(1),
            Self::Line | Self::Rectangle | Self::Circle => Some(2),
            Self::Arc | Self::Quadratic => Some(3),
            Self::Cubic => Some(4),
            Self::Polyline => None,
        }
    }

    fn stage_prompt(self, count: usize) -> String {
        match self {
            Self::Point => "Place the point.".into(),
            Self::Line => ["Place line start.", "Place line end."]
                .get(count)
                .unwrap_or(&"Line ready.")
                .to_string(),
            Self::Polyline => format!(
                "{} staged vert{}; add another or finish.",
                count,
                if count == 1 { "ex" } else { "ices" }
            ),
            Self::Rectangle => ["Place first corner.", "Place opposite corner."]
                .get(count)
                .unwrap_or(&"Rectangle ready.")
                .to_string(),
            Self::Circle => ["Place circle center.", "Place radius point."]
                .get(count)
                .unwrap_or(&"Circle ready.")
                .to_string(),
            Self::Arc => ["Place arc center.", "Place arc start.", "Place arc end."]
                .get(count)
                .unwrap_or(&"Arc ready.")
                .to_string(),
            Self::Quadratic => [
                "Place P0 endpoint.",
                "Place P1 handle.",
                "Place P2 endpoint.",
            ]
            .get(count)
            .unwrap_or(&"Quadratic Bézier ready.")
            .to_string(),
            Self::Cubic => [
                "Place P0 endpoint.",
                "Place P1 handle.",
                "Place P2 handle.",
                "Place P3 endpoint.",
            ]
            .get(count)
            .unwrap_or(&"Cubic Bézier ready.")
            .to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Tool {
    Select,
    Pan,
    Draw(DrawTool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NeighborhoodChoice {
    Picked,
    Interior,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContactBranchOptions {
    neighborhood: NeighborhoodChoice,
    tangent_orientation: TangentOrientation,
    winding: i32,
}

impl Tool {
    const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Pan => "Pan",
            Self::Draw(tool) => tool.label(),
        }
    }

    const fn key(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Pan => "pan",
            Self::Draw(DrawTool::Point) => "point",
            Self::Draw(DrawTool::Line) => "line",
            Self::Draw(DrawTool::Polyline) => "polyline",
            Self::Draw(DrawTool::Rectangle) => "rectangle",
            Self::Draw(DrawTool::Circle) => "circle",
            Self::Draw(DrawTool::Arc) => "arc",
            Self::Draw(DrawTool::Quadratic) => "quadratic",
            Self::Draw(DrawTool::Cubic) => "cubic",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SelectionItem {
    Point(DesignPointId),
    Curve { span: CurveSpan, parameter: f64 },
    Contact(ContactId),
    Constraint(DocumentConstraintId),
    Dimension(DocumentDimensionId),
}

impl SelectionItem {
    const fn object_id(self) -> DocumentObjectId {
        match self {
            Self::Point(id) => DocumentObjectId::Point(id),
            Self::Curve { span, .. } => DocumentObjectId::Curve(span.curve),
            Self::Contact(id) => DocumentObjectId::Contact(id),
            Self::Constraint(id) => DocumentObjectId::Constraint(id),
            Self::Dimension(id) => DocumentObjectId::Dimension(id),
        }
    }

    fn same_object(self, other: Self) -> bool {
        match (self, other) {
            (Self::Curve { span: first, .. }, Self::Curve { span: second, .. }) => first == second,
            _ => self.object_id() == other.object_id(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Viewport {
    pub center: [f64; 2],
    pub pixels_per_unit: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            pixels_per_unit: 70.0,
        }
    }
}

impl Viewport {
    pub(crate) fn model_to_svg(self, point: [f64; 2]) -> [f64; 2] {
        [
            CANVAS_WIDTH * 0.5
                + finite_screen_offset(point[0], self.center[0], self.pixels_per_unit),
            CANVAS_HEIGHT * 0.5
                - finite_screen_offset(point[1], self.center[1], self.pixels_per_unit),
        ]
    }

    pub(crate) fn svg_to_model(self, point: [f64; 2]) -> [f64; 2] {
        [
            self.center[0] + (point[0] - CANVAS_WIDTH * 0.5) / self.pixels_per_unit,
            self.center[1] - (point[1] - CANVAS_HEIGHT * 0.5) / self.pixels_per_unit,
        ]
    }

    pub(crate) fn zoom_at(&mut self, svg: [f64; 2], factor: f64) {
        let before = self.svg_to_model(svg);
        self.pixels_per_unit = (self.pixels_per_unit * factor).clamp(1.0e-12, 1.0e12);
        let after = self.svg_to_model(svg);
        self.center[0] += before[0] - after[0];
        self.center[1] += before[1] - after[1];
    }
}

#[derive(Clone, Debug)]
struct InferenceProposal {
    base_revision: u64,
    label: String,
    edit: DocumentEdit,
}

#[derive(Clone, Debug)]
enum PointerGesture {
    DragPoint {
        pointer_id: i32,
        point: DesignPointId,
        start_svg: [f64; 2],
        moved: bool,
    },
    Pan {
        pointer_id: i32,
        last_svg: [f64; 2],
    },
    BoxSelect {
        pointer_id: i32,
        start_svg: [f64; 2],
        current_svg: [f64; 2],
        additive: bool,
    },
    PlaceDraft {
        pointer_id: i32,
        current_svg: [f64; 2],
    },
}

#[derive(Clone, Debug)]
struct DragPreview {
    session: SketchDocumentSession,
}

#[derive(Debug)]
pub(crate) struct PlaygroundState {
    session: SketchDocumentSession,
    tool: Tool,
    selection: Vec<SelectionItem>,
    draft: Vec<[f64; 2]>,
    draft_cursor: Option<[f64; 2]>,
    viewport: Viewport,
    arc_sweep: DocumentArcSweep,
    contact_neighborhood: NeighborhoodChoice,
    second_contact_neighborhood: NeighborhoodChoice,
    tangent_orientation: TangentOrientation,
    second_tangent_orientation: TangentOrientation,
    contact_winding: i32,
    second_contact_winding: i32,
    angle_orientation: DocumentAngleOrientation,
    inference: Option<InferenceProposal>,
    gesture: Option<PointerGesture>,
    drag_preview: Option<DragPreview>,
    last_attempt: String,
    last_attempt_result: Option<DocumentSolveResult>,
    storage_dirty: bool,
}

impl PlaygroundState {
    pub(crate) fn empty() -> Result<Self, String> {
        let document = SketchDocument::new(10.0).map_err(|error| error.to_string())?;
        Self::from_document(document, true)
    }

    pub(crate) fn from_json(json: &str) -> Result<Self, String> {
        let document = SketchDocument::from_json(json).map_err(|error| error.to_string())?;
        Self::from_document(document, false)
    }

    fn from_document(document: SketchDocument, storage_dirty: bool) -> Result<Self, String> {
        Self::from_document_request(document, DocumentSolveRequest::default(), storage_dirty)
    }

    fn from_document_request(
        document: SketchDocument,
        request: DocumentSolveRequest,
        storage_dirty: bool,
    ) -> Result<Self, String> {
        let session = SketchDocumentSession::new(document, request, SolverConfig::default())
            .map_err(|error| error.to_string())?;
        let mut state = Self {
            session,
            tool: Tool::Select,
            selection: Vec::new(),
            draft: Vec::new(),
            draft_cursor: None,
            viewport: Viewport::default(),
            arc_sweep: DocumentArcSweep::CounterClockwise,
            contact_neighborhood: NeighborhoodChoice::Picked,
            second_contact_neighborhood: NeighborhoodChoice::Picked,
            tangent_orientation: TangentOrientation::Aligned,
            second_tangent_orientation: TangentOrientation::Aligned,
            contact_winding: 0,
            second_contact_winding: 0,
            angle_orientation: DocumentAngleOrientation::CounterClockwise,
            inference: None,
            gesture: None,
            drag_preview: None,
            last_attempt: "Accepted document loaded.".into(),
            last_attempt_result: None,
            storage_dirty,
        };
        state.fit_view();
        Ok(state)
    }

    pub(crate) fn example(kind: AlphaScenarioKind, scale: f64) -> Result<Self, String> {
        let fixture = alpha_scenario(kind, scale).map_err(|error| error.to_string())?;
        let mut state = Self::from_document_request(fixture.document, fixture.request, true)?;
        state.last_attempt = format!(
            "Loaded canonical {} example at scale {scale:e}.",
            kind.key()
        );
        Ok(state)
    }

    pub(crate) fn medium_performance_example() -> Result<Self, String> {
        let document = alpha_performance_document(AlphaPerformanceSize::Medium)
            .map_err(|error| error.to_string())?;
        let mut state = Self::from_document(document, true)?;
        state.last_attempt = "Loaded deterministic M14 medium performance document.".into();
        Ok(state)
    }

    pub(crate) const fn tool(&self) -> Tool {
        self.tool
    }

    pub(crate) const fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub(crate) fn document(&self) -> &SketchDocument {
        self.display_session().document()
    }

    pub(crate) fn session(&self) -> &SketchDocumentSession {
        &self.session
    }

    fn display_session(&self) -> &SketchDocumentSession {
        self.drag_preview
            .as_ref()
            .map_or(&self.session, |preview| &preview.session)
    }

    const fn preview_active(&self) -> bool {
        self.drag_preview.is_some()
    }

    fn set_startup_notice(&mut self, message: impl Into<String>) {
        self.last_attempt = message.into();
        self.last_attempt_result = None;
        self.storage_dirty = false;
    }

    pub(crate) fn set_tool(&mut self, tool: Tool) {
        let canceled = !self.draft.is_empty();
        self.cancel_interaction();
        self.tool = tool;
        if canceled {
            self.last_attempt = "Unfinished drawing canceled when the tool changed.".into();
            self.last_attempt_result = None;
        }
    }

    pub(crate) fn set_draft_cursor(&mut self, point: [f64; 2]) {
        self.draft_cursor = matches!(self.tool, Tool::Draw(_)).then_some(point);
    }

    fn set_branch_options(
        &mut self,
        arc_sweep: DocumentArcSweep,
        first: ContactBranchOptions,
        second: ContactBranchOptions,
        angle_orientation: DocumentAngleOrientation,
    ) {
        self.arc_sweep = arc_sweep;
        self.contact_neighborhood = first.neighborhood;
        self.tangent_orientation = first.tangent_orientation;
        self.contact_winding = first.winding;
        self.second_contact_neighborhood = second.neighborhood;
        self.second_tangent_orientation = second.tangent_orientation;
        self.second_contact_winding = second.winding;
        self.angle_orientation = angle_orientation;
    }

    fn accepted_change(&mut self, message: impl Into<String>) {
        self.last_attempt = message.into();
        self.last_attempt_result = None;
        self.inference = None;
        self.prune_selection();
        self.storage_dirty = true;
    }

    fn rejected_change(&mut self, message: impl Into<String>) {
        self.last_attempt = message.into();
        self.last_attempt_result = None;
    }

    fn rejected_result(&mut self, message: impl Into<String>, result: DocumentSolveResult) {
        self.last_attempt = message.into();
        self.last_attempt_result = Some(result);
    }

    fn apply_edit(&mut self, edit: DocumentEdit) -> Option<DocumentCommandEffect> {
        match self
            .session
            .apply(DocumentCommand::new(self.session.revision(), edit))
        {
            Ok(outcome) if outcome.accepted() => {
                let effect = outcome.effect.clone();
                self.accepted_change("Edit accepted and autosaved.");
                effect
            }
            Ok(outcome) => {
                let message = format!(
                    "Edit rejected; accepted geometry retained: {:?}",
                    outcome.result.solve().rejection
                );
                self.rejected_result(message, outcome.result);
                None
            }
            Err(error) => {
                self.rejected_change(format!("Edit failed without mutation: {error}"));
                None
            }
        }
    }

    pub(crate) fn draw_click(&mut self, point: [f64; 2]) {
        let Tool::Draw(tool) = self.tool else {
            return;
        };
        if tool == DrawTool::Point {
            self.create_point(point);
            return;
        }
        self.draft.push(point);
        if tool
            .required_points()
            .is_some_and(|required| self.draft.len() == required)
        {
            self.finish_draft();
        }
    }

    pub(crate) fn undo_draft_point(&mut self) {
        if self.draft.pop().is_some() {
            self.draft_cursor = None;
            self.last_attempt = "Removed the last staged drawing point.".into();
            self.last_attempt_result = None;
        }
    }

    pub(crate) fn cancel_draft(&mut self) {
        if !self.draft.is_empty() || self.draft_cursor.is_some() {
            self.draft.clear();
            self.draft_cursor = None;
            self.gesture = None;
            self.last_attempt =
                "Unfinished drawing canceled; accepted geometry was unchanged.".into();
            self.last_attempt_result = None;
        }
    }

    fn create_point(&mut self, position: [f64; 2]) {
        let nearby = self
            .session
            .document()
            .points()
            .iter()
            .filter_map(|point| {
                let distance = distance(point.position, position);
                (distance <= 14.0 / self.viewport.pixels_per_unit).then_some((point.id, distance))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|value| value.0);
        let label = format!("Point {}", self.session.document().points().len() + 1);
        if let Some(DocumentCommandEffect::CreatedPoint(created)) =
            self.apply_edit(DocumentEdit::CreatePoint { label, position })
        {
            self.selection = vec![SelectionItem::Point(created)];
            if let Some(existing) = nearby {
                self.inference = Some(InferenceProposal {
                    base_revision: self.session.revision(),
                    label: "Coincident with nearby point".into(),
                    edit: DocumentEdit::CreateConstraint {
                        label: "inferred coincidence".into(),
                        definition: DocumentConstraintDefinition::Coincident {
                            first: existing,
                            second: created,
                        },
                    },
                });
            }
        }
    }

    pub(crate) fn finish_draft(&mut self) {
        let Tool::Draw(tool) = self.tool else {
            return;
        };
        let points = self.draft.clone();
        let arc_sweep = self.arc_sweep;
        let minimum = tool.required_points().unwrap_or(2);
        if points.len() < minimum {
            self.rejected_change(format!("{} needs at least {minimum} points.", tool.label()));
            return;
        }
        let revision = self.session.revision();
        let transaction = self.session.transact(
            revision,
            format!("draw {}", tool.label()),
            move |document| create_geometry(document, tool, &points, arc_sweep),
        );
        match transaction {
            Ok(transaction) if transaction.accepted() => {
                let created = transaction.value.expect("accepted transaction value");
                self.draft.clear();
                self.draft_cursor = None;
                self.selection = created.selection;
                self.accepted_change(format!("{} accepted as one history step.", tool.label()));
                if let Some(edit) = created.inference {
                    self.inference = Some(InferenceProposal {
                        base_revision: self.session.revision(),
                        label: edit.0,
                        edit: edit.1,
                    });
                }
            }
            Ok(transaction) => {
                let message = format!(
                    "{} rejected; no partial objects were created: {:?}",
                    tool.label(),
                    transaction.outcome.result.solve().rejection
                );
                self.rejected_result(message, transaction.outcome.result);
            }
            Err(error) => self.rejected_change(format!(
                "{} failed; no partial objects were created: {error}",
                tool.label()
            )),
        }
    }

    pub(crate) fn select_at(&mut self, svg: [f64; 2], additive: bool) -> bool {
        let hit = self.hit_test(svg, HIT_RADIUS_PX);
        if !additive {
            self.selection.clear();
        }
        if let Some(item) = hit {
            if let Some(index) = self
                .selection
                .iter()
                .position(|selected| selected.same_object(item))
            {
                if additive {
                    self.selection.remove(index);
                } else {
                    self.selection[index] = item;
                }
            } else {
                self.selection.push(item);
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selection.clear();
    }

    fn hit_test(&self, svg: [f64; 2], hit_radius: f64) -> Option<SelectionItem> {
        let mut point_hit = self
            .document()
            .points()
            .iter()
            .filter_map(|point| {
                let screen = self.viewport.model_to_svg(point.position);
                let distance = distance(screen, svg);
                (distance <= hit_radius).then_some((point.id, distance))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1));
        if let Some((point, _)) = point_hit.take() {
            return Some(SelectionItem::Point(point));
        }
        sampled_curves(self.document())
            .into_iter()
            .flat_map(|(span, samples)| {
                samples
                    .windows(2)
                    .filter_map(move |pair| {
                        let first = self.viewport.model_to_svg(pair[0].1);
                        let second = self.viewport.model_to_svg(pair[1].1);
                        let (distance, fraction) = point_segment_distance(svg, first, second);
                        (distance <= hit_radius).then_some((
                            SelectionItem::Curve {
                                span,
                                parameter: pair[0].0 + fraction * (pair[1].0 - pair[0].0),
                            },
                            distance,
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|value| value.0)
    }

    pub(crate) fn begin_point_drag(
        &mut self,
        pointer_id: i32,
        point: DesignPointId,
        start_svg: [f64; 2],
    ) {
        self.drag_preview = Some(DragPreview {
            session: self.session.clone(),
        });
        self.gesture = Some(PointerGesture::DragPoint {
            pointer_id,
            point,
            start_svg,
            moved: false,
        });
    }

    pub(crate) fn begin_pan(&mut self, pointer_id: i32, svg: [f64; 2]) {
        self.gesture = Some(PointerGesture::Pan {
            pointer_id,
            last_svg: svg,
        });
    }

    pub(crate) fn begin_draft_placement(&mut self, pointer_id: i32, svg: [f64; 2]) {
        self.draft_cursor = Some(self.viewport.svg_to_model(svg));
        self.gesture = Some(PointerGesture::PlaceDraft {
            pointer_id,
            current_svg: svg,
        });
    }

    pub(crate) fn begin_box_select(&mut self, pointer_id: i32, svg: [f64; 2], additive: bool) {
        self.gesture = Some(PointerGesture::BoxSelect {
            pointer_id,
            start_svg: svg,
            current_svg: svg,
            additive,
        });
    }

    pub(crate) fn gesture_pointer(&self) -> Option<i32> {
        self.gesture.as_ref().map(|gesture| match gesture {
            PointerGesture::DragPoint { pointer_id, .. }
            | PointerGesture::Pan { pointer_id, .. }
            | PointerGesture::BoxSelect { pointer_id, .. }
            | PointerGesture::PlaceDraft { pointer_id, .. } => *pointer_id,
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn update_gesture(&mut self, pointer_id: i32, svg: [f64; 2]) -> bool {
        let model = self.viewport.svg_to_model(svg);
        match &mut self.gesture {
            Some(PointerGesture::DragPoint {
                pointer_id: active,
                point,
                start_svg,
                moved,
            }) if *active == pointer_id => {
                *moved |= distance(*start_svg, svg) >= 3.0;
                if !*moved {
                    return false;
                }
                let duplicate_target = self
                    .drag_preview
                    .as_ref()
                    .map_or_else(
                        || self.session.request(),
                        |preview| preview.session.request(),
                    )
                    .drag
                    .is_some_and(|drag| {
                        drag.point == *point && same_point_bits(drag.target, model)
                    });
                if duplicate_target {
                    return false;
                }
                let mut candidate = self
                    .drag_preview
                    .as_ref()
                    .map_or_else(|| self.session.clone(), |preview| preview.session.clone());
                {
                    let previous = candidate
                        .document()
                        .point(*point)
                        .map(|point| point.position);
                    let mut request = candidate
                        .request()
                        .without_previous_state_preferences()
                        .with_drag(*point, model);
                    if let Some(stable) = drag_stability_point(candidate.document(), *point)
                        && let Some(target) = candidate
                            .document()
                            .point(stable)
                            .map(|point| point.position)
                    {
                        request = request.with_stability_target(stable, target);
                    }
                    match candidate.rebuild_request(candidate.revision(), request) {
                        Ok(result) if result.accepted() => {
                            let projected = candidate
                                .document()
                                .point(*point)
                                .map(|point| point.position);
                            self.drag_preview = Some(DragPreview { session: candidate });
                            self.last_attempt = if previous == projected {
                                "Drag target has no solver-permitted motion; edit or suppress its driving constraints."
                                    .into()
                            } else {
                                "Projected drag accepted.".into()
                            };
                            self.last_attempt_result = None;
                        }
                        Ok(result) => {
                            self.last_attempt = format!(
                                "Drag target rejected; last projected position retained: {:?}",
                                result.solve().rejection
                            );
                            self.last_attempt_result = Some(result);
                        }
                        Err(error) => {
                            self.last_attempt = format!("Drag target failed: {error}");
                            self.last_attempt_result = None;
                        }
                    }
                }
                true
            }
            Some(PointerGesture::Pan {
                pointer_id: active,
                last_svg,
            }) if *active == pointer_id => {
                if same_point_bits(*last_svg, svg) {
                    return false;
                }
                self.viewport.center[0] -= (svg[0] - last_svg[0]) / self.viewport.pixels_per_unit;
                self.viewport.center[1] += (svg[1] - last_svg[1]) / self.viewport.pixels_per_unit;
                *last_svg = svg;
                true
            }
            Some(PointerGesture::BoxSelect {
                pointer_id: active,
                current_svg,
                ..
            }) if *active == pointer_id => {
                if same_point_bits(*current_svg, svg) {
                    return false;
                }
                *current_svg = svg;
                true
            }
            Some(PointerGesture::PlaceDraft {
                pointer_id: active,
                current_svg,
            }) if *active == pointer_id => {
                *current_svg = svg;
                self.draft_cursor = Some(model);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn end_gesture(&mut self, pointer_id: i32, commit: bool) {
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        match gesture {
            PointerGesture::DragPoint {
                pointer_id: active,
                point,
                moved,
                ..
            } if active == pointer_id => {
                let preview = self.drag_preview.take();
                let position = preview
                    .as_ref()
                    .and_then(|preview| preview.session.document().point(point))
                    .map(|point| point.position);
                if commit
                    && moved
                    && let Some(position) = position
                {
                    let changed = self
                        .session
                        .document()
                        .point(point)
                        .is_some_and(|accepted| {
                            accepted.position.map(f64::to_bits) != position.map(f64::to_bits)
                        });
                    if changed {
                        let document = preview
                            .expect("drag position came from preview")
                            .session
                            .document()
                            .clone();
                        let transaction = self.session.transact(
                            self.session.revision(),
                            "projected point drag",
                            move |candidate| {
                                *candidate = document;
                                Ok(())
                            },
                        );
                        match transaction {
                            Ok(transaction) if transaction.accepted() => self
                                .accepted_change("Projected drag committed as one history step."),
                            Ok(transaction) => {
                                let message = format!(
                                    "Drag release rejected; accepted document retained: {:?}",
                                    transaction.outcome.result.solve().rejection
                                );
                                self.rejected_result(message, transaction.outcome.result);
                            }
                            Err(error) => self.rejected_change(format!(
                                "Drag release failed; accepted document retained: {error}"
                            )),
                        }
                    }
                }
            }
            PointerGesture::BoxSelect {
                pointer_id: active,
                start_svg,
                current_svg,
                additive,
            } if active == pointer_id && commit => {
                self.select_box(start_svg, current_svg, additive);
            }
            PointerGesture::PlaceDraft {
                pointer_id: active,
                current_svg,
            } if active == pointer_id => {
                self.draft_cursor = None;
                if commit {
                    self.draw_click(self.viewport.svg_to_model(current_svg));
                } else {
                    self.last_attempt =
                        "Canceled pointer placement; staged drawing points were retained.".into();
                    self.last_attempt_result = None;
                }
            }
            PointerGesture::Pan { .. }
            | PointerGesture::BoxSelect { .. }
            | PointerGesture::DragPoint { .. }
            | PointerGesture::PlaceDraft { .. } => {
                self.drag_preview = None;
            }
        }
    }

    fn select_box(&mut self, first: [f64; 2], second: [f64; 2], additive: bool) {
        let min = [first[0].min(second[0]), first[1].min(second[1])];
        let max = [first[0].max(second[0]), first[1].max(second[1])];
        if !additive {
            self.selection.clear();
        }
        let points: Vec<_> = self
            .document()
            .points()
            .iter()
            .filter_map(|point| {
                let svg = self.viewport.model_to_svg(point.position);
                point_in_rect(svg, min, max).then_some(point.id)
            })
            .collect();
        for point in points {
            let item = SelectionItem::Point(point);
            if !self
                .selection
                .iter()
                .any(|selected| selected.same_object(item))
            {
                self.selection.push(item);
            }
        }
        let curves: Vec<_> = sampled_curves(self.document())
            .into_iter()
            .filter_map(|(span, samples)| {
                samples.iter().find_map(|(parameter, point)| {
                    point_in_rect(self.viewport.model_to_svg(*point), min, max).then_some(
                        SelectionItem::Curve {
                            span,
                            parameter: *parameter,
                        },
                    )
                })
            })
            .collect();
        for curve in curves {
            if !self
                .selection
                .iter()
                .any(|selected| selected.same_object(curve))
            {
                self.selection.push(curve);
            }
        }
    }

    pub(crate) fn cancel_interaction(&mut self) {
        self.gesture = None;
        self.drag_preview = None;
        self.draft.clear();
        self.draft_cursor = None;
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn apply_constraint(&mut self, kind: usize) {
        let points = self.selected_points();
        let curves = self.selected_curves();
        let neighborhood = self.contact_neighborhood;
        let tangent_orientation = self.tangent_orientation;
        let winding = self.contact_winding;
        let second_neighborhood = self.second_contact_neighborhood;
        let second_tangent_orientation = self.second_tangent_orientation;
        let second_winding = self.second_contact_winding;
        let revision = self.session.revision();
        let transaction = self
            .session
            .transact(revision, "apply constraint", |document| {
                let mut created = Vec::new();
                match kind {
                    0 if !points.is_empty() => {
                        for point in &points {
                            let target = document
                                .point(*point)
                                .ok_or_else(|| unknown_point(*point))?
                                .position;
                            created.push(document.add_constraint(
                                "fixed point",
                                DocumentConstraintDefinition::FixedPoint {
                                    point: *point,
                                    target,
                                },
                            )?);
                        }
                    }
                    1 if points.len() == 2 => created.push(document.add_constraint(
                        "coincident",
                        DocumentConstraintDefinition::Coincident {
                            first: points[0],
                            second: points[1],
                        },
                    )?),
                    2 | 3 if !curves.is_empty() => {
                        for (span, _) in &curves {
                            document.reselect_curve_branch(*span)?;
                            let definition = if kind == 2 {
                                DocumentConstraintDefinition::Horizontal { line: *span }
                            } else {
                                DocumentConstraintDefinition::Vertical { line: *span }
                            };
                            created.push(document.add_constraint(
                                if kind == 2 { "horizontal" } else { "vertical" },
                                definition,
                            )?);
                        }
                    }
                    4..=6 if curves.len() == 2 => {
                        let definition = match kind {
                            4 => DocumentConstraintDefinition::Parallel {
                                first: curves[0].0,
                                second: curves[1].0,
                            },
                            5 => DocumentConstraintDefinition::Perpendicular {
                                first: curves[0].0,
                                second: curves[1].0,
                            },
                            _ => DocumentConstraintDefinition::EqualLength {
                                first: curves[0].0,
                                second: curves[1].0,
                            },
                        };
                        created.push(document.add_constraint("line relation", definition)?);
                    }
                    7 if curves.len() == 2 => created.push(document.add_constraint(
                        "equal radius",
                        DocumentConstraintDefinition::EqualRadius {
                            first: curves[0].0.curve,
                            second: curves[1].0.curve,
                        },
                    )?),
                    8 if points.len() == 1 && curves.len() == 1 => {
                        created.push(document.add_constraint(
                            "midpoint",
                            DocumentConstraintDefinition::Midpoint {
                                point: points[0],
                                line: curves[0].0,
                            },
                        )?);
                    }
                    9 if points.len() == 2 && curves.len() == 1 => {
                        created.push(document.add_constraint(
                            "symmetric about line",
                            DocumentConstraintDefinition::SymmetricAboutLine {
                                first: points[0],
                                second: points[1],
                                line: curves[0].0,
                            },
                        )?);
                    }
                    10 if points.len() == 1 && curves.len() == 1 => {
                        let contact = add_contact(
                            document,
                            curves[0],
                            false,
                            "point contact",
                            neighborhood,
                            tangent_orientation,
                            winding,
                        )?;
                        created.push(document.add_constraint(
                            "point on curve",
                            DocumentConstraintDefinition::PointOnCurve {
                                point: points[0],
                                contact,
                            },
                        )?);
                    }
                    11 | 12 if curves.len() == 2 => {
                        let tangent = kind == 12;
                        let first = add_contact(
                            document,
                            curves[0],
                            tangent,
                            "first contact",
                            neighborhood,
                            tangent_orientation,
                            winding,
                        )?;
                        let second = add_contact(
                            document,
                            curves[1],
                            tangent,
                            "second contact",
                            second_neighborhood,
                            second_tangent_orientation,
                            second_winding,
                        )?;
                        let definition = if tangent {
                            DocumentConstraintDefinition::CurveCurveTangency {
                                first_contact: first,
                                second_contact: second,
                            }
                        } else {
                            DocumentConstraintDefinition::CurveCurveContact {
                                first_contact: first,
                                second_contact: second,
                            }
                        };
                        created.push(document.add_constraint(
                            if tangent {
                                "curve tangency"
                            } else {
                                "curve contact"
                            },
                            definition,
                        )?);
                    }
                    _ => {
                        return Err(geosolve_sketch::DocumentError::InvalidField {
                            field: "constraint selection",
                            message: "the selected objects are incompatible with this constraint"
                                .into(),
                        });
                    }
                }
                Ok(created)
            });
        match transaction {
            Ok(transaction) if transaction.accepted() => {
                self.selection = transaction
                    .value
                    .expect("accepted transaction value")
                    .into_iter()
                    .map(SelectionItem::Constraint)
                    .collect();
                self.accepted_change("Constraint transaction accepted.");
            }
            Ok(transaction) => {
                let message = format!(
                    "Constraint rejected; accepted document retained: {:?}",
                    transaction.outcome.result.solve().rejection
                );
                self.rejected_result(message, transaction.outcome.result);
            }
            Err(error) => self.rejected_change(format!("Constraint not applied: {error}")),
        }
    }

    pub(crate) fn apply_dimension(&mut self, kind: usize, mode: DocumentDimensionMode, value: f64) {
        self.apply_dimension_labeled(kind, mode, value, "dimension");
    }

    fn apply_dimension_labeled(
        &mut self,
        kind: usize,
        mode: DocumentDimensionMode,
        value: f64,
        label: &str,
    ) {
        let points = self.selected_points();
        let curves = self.selected_curves();
        let selected_dimension = self.selection.iter().find_map(|item| match item {
            SelectionItem::Dimension(id) => Some(*id),
            _ => None,
        });
        let angle_orientation = self.angle_orientation;
        let label = label.to_owned();
        let revision = self.session.revision();
        let transaction = self
            .session
            .transact(revision, "dimension edit", |document| {
                if let Some(id) = selected_dimension {
                    let dimension = document
                        .dimension(id)
                        .ok_or(geosolve_sketch::DocumentError::UnknownId {
                            kind: "dimension",
                            id: id.0,
                        })?
                        .clone();
                    let target = dimension_target(&dimension.definition);
                    document.set_scalar_value(target, value)?;
                    if mode == DocumentDimensionMode::Driving
                        && let DocumentDimensionDefinition::CurveLength { curve, .. } =
                            &dimension.definition
                    {
                        document.reselect_curve_branch(*curve)?;
                    }
                    document.set_dimension_mode(id, mode)?;
                    return Ok(id);
                }
                let (unit, domain) = if kind == 4 {
                    (ScalarUnit::Angle, ScalarDomain::Positive)
                } else {
                    (ScalarUnit::Length, ScalarDomain::Positive)
                };
                let target = document.add_scalar(format!("{label} target"), value, unit, domain)?;
                let definition = match kind {
                    0 if points.len() == 2 => DocumentDimensionDefinition::PointDistance {
                        first: points[0],
                        second: points[1],
                        target,
                    },
                    1 if curves.len() == 1 => DocumentDimensionDefinition::CurveLength {
                        curve: curves[0].0,
                        target,
                    },
                    2 if curves.len() == 1 => DocumentDimensionDefinition::Radius {
                        curve: curves[0].0.curve,
                        target,
                    },
                    3 if curves.len() == 1 => DocumentDimensionDefinition::Diameter {
                        curve: curves[0].0.curve,
                        target,
                    },
                    4 if curves.len() == 2 => DocumentDimensionDefinition::OrientedAngle {
                        first: curves[0].0,
                        second: curves[1].0,
                        target,
                        orientation: angle_orientation,
                    },
                    _ => {
                        return Err(geosolve_sketch::DocumentError::InvalidField {
                            field: "dimension selection",
                            message: "the selected objects are incompatible with this dimension"
                                .into(),
                        });
                    }
                };
                if mode == DocumentDimensionMode::Driving
                    && let DocumentDimensionDefinition::CurveLength { curve, .. } = &definition
                {
                    document.reselect_curve_branch(*curve)?;
                }
                document.add_dimension(label, definition, mode)
            });
        match transaction {
            Ok(transaction) if transaction.accepted() => {
                let dimension = transaction.value.expect("accepted transaction value");
                self.selection = vec![SelectionItem::Dimension(dimension)];
                self.accepted_change("Dimension transaction accepted.");
            }
            Ok(transaction) => {
                let message = format!(
                    "Dimension rejected; accepted document retained: {:?}",
                    transaction.outcome.result.solve().rejection
                );
                self.rejected_result(message, transaction.outcome.result);
            }
            Err(error) => self.rejected_change(format!("Dimension not applied: {error}")),
        }
    }

    pub(crate) fn delete_selection(&mut self) {
        let mut objects: Vec<_> = self.selection.iter().map(|item| item.object_id()).collect();
        objects.sort_by_key(|object| match object {
            DocumentObjectId::Constraint(_) | DocumentObjectId::Dimension(_) => 0,
            DocumentObjectId::Contact(_) => 1,
            DocumentObjectId::Curve(_) => 2,
            DocumentObjectId::Point(_) | DocumentObjectId::Scalar(_) => 3,
        });
        objects.dedup();
        if objects.is_empty() {
            self.rejected_change("Select an object to delete.");
            return;
        }
        let transaction = self.session.transact(
            self.session.revision(),
            "delete selection",
            move |document| document.remove_many_with_dependents(&objects),
        );
        match transaction {
            Ok(transaction) if transaction.accepted() => {
                self.selection.clear();
                self.accepted_change("Selection deleted as one history step.");
            }
            Ok(transaction) => {
                let message = format!(
                    "Deletion rejected; accepted document retained: {:?}",
                    transaction.outcome.result.solve().rejection
                );
                self.rejected_result(message, transaction.outcome.result);
            }
            Err(error) => self.rejected_change(format!("Deletion not applied: {error}.")),
        }
    }

    fn delete_object(&mut self, object: DocumentObjectId) {
        self.apply_edit(DocumentEdit::Delete { object });
    }

    pub(crate) fn toggle_selected_sources(&mut self) {
        let sources: Vec<_> = self
            .selection
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Constraint(id) => self
                    .session
                    .document()
                    .constraint(*id)
                    .map(|source| (source.source_id, !source.suppressed)),
                SelectionItem::Dimension(id) => self
                    .session
                    .document()
                    .dimension(*id)
                    .map(|source| (source.source_id, !source.suppressed)),
                _ => None,
            })
            .collect();
        if sources.is_empty() {
            self.rejected_change("Select constraints or dimensions to suppress or restore.");
            return;
        }
        let transaction = self.session.transact(
            self.session.revision(),
            "toggle source suppression",
            move |document| {
                for (source, suppressed) in &sources {
                    document.set_source_suppressed(*source, *suppressed)?;
                }
                Ok(())
            },
        );
        match transaction {
            Ok(transaction) if transaction.accepted() => {
                self.accepted_change("Source suppression updated.");
            }
            Ok(transaction) => {
                let message = format!(
                    "Suppression rejected: {:?}",
                    transaction.outcome.result.solve().rejection
                );
                self.rejected_result(message, transaction.outcome.result);
            }
            Err(error) => self.rejected_change(format!("Suppression not changed: {error}")),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn apply_branch_state(&mut self) {
        self.apply_branch_state_values(None, None);
    }

    #[allow(clippy::too_many_lines)]
    fn apply_branch_state_values(
        &mut self,
        first_parameter: Option<f64>,
        second_parameter: Option<f64>,
    ) {
        let arcs: Vec<_> = self
            .selection
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Curve { span, .. } => self
                    .session
                    .document()
                    .curve(span.curve)
                    .filter(|curve| matches!(curve.definition, CurveDefinition::CircularArc { .. }))
                    .map(|curve| curve.id),
                _ => None,
            })
            .collect();
        let selected_contacts: Vec<_> = self
            .selection
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Contact(id) => Some(*id),
                _ => None,
            })
            .collect();
        let contacts = if selected_contacts.is_empty() {
            Vec::new()
        } else {
            match self
                .session
                .document()
                .ordered_source_contacts(&selected_contacts)
            {
                Ok(contacts) => contacts,
                Err(error) => {
                    self.rejected_change(format!("Branch state not changed: {error}"));
                    return;
                }
            }
        };
        if arcs.is_empty() && contacts.is_empty() {
            self.rejected_change("Select arcs or complete contact source state to edit branches.");
            return;
        }
        let sweep = self.arc_sweep;
        let neighborhood = self.contact_neighborhood;
        let tangent_orientation = self.tangent_orientation;
        let winding = self.contact_winding;
        let second_neighborhood = self.second_contact_neighborhood;
        let second_tangent_orientation = self.second_tangent_orientation;
        let second_winding = self.second_contact_winding;
        let transaction = self.session.transact(
            self.session.revision(),
            "edit explicit branch state",
            move |document| {
                for arc in &arcs {
                    document.set_arc_sweep(*arc, sweep)?;
                }
                if !contacts.is_empty() {
                    let mut edits = Vec::with_capacity(contacts.len());
                    for (index, id) in contacts.iter().enumerate() {
                        let (neighborhood, tangent_orientation, winding) = if index == 0 {
                            (neighborhood, tangent_orientation, winding)
                        } else {
                            (
                                second_neighborhood,
                                second_tangent_orientation,
                                second_winding,
                            )
                        };
                        let contact = document
                            .contact(*id)
                            .ok_or(geosolve_sketch::DocumentError::UnknownId {
                                kind: "contact",
                                id: id.0,
                            })?
                            .clone();
                        let retained = document
                            .scalar(contact.parameter)
                            .ok_or(geosolve_sketch::DocumentError::UnknownId {
                                kind: "contact parameter",
                                id: contact.parameter.0,
                            })?
                            .value;
                        let requested = if index == 0 {
                            first_parameter
                        } else {
                            second_parameter
                        };
                        let (value, explicit_neighborhood) = match neighborhood {
                            NeighborhoodChoice::Picked => (
                                requested.unwrap_or(retained),
                                document.picked_contact_neighborhood(
                                    contact.curve,
                                    requested.unwrap_or(retained),
                                )?,
                            ),
                            NeighborhoodChoice::Interior => {
                                (requested.unwrap_or(retained), ContactNeighborhood::Interior)
                            }
                            NeighborhoodChoice::Start => (0.0, ContactNeighborhood::Start),
                            NeighborhoodChoice::End => (1.0, ContactNeighborhood::End),
                        };
                        edits.push(ContactStateEdit {
                            contact: *id,
                            value,
                            winding,
                            neighborhood: explicit_neighborhood,
                            tangent_orientation: contact
                                .tangent_orientation
                                .map(|_| tangent_orientation),
                        });
                    }
                    document.set_contact_states(&edits)?;
                }
                Ok(())
            },
        );
        match transaction {
            Ok(transaction) if transaction.accepted() => {
                self.accepted_change("Explicit branch state updated.");
            }
            Ok(transaction) => {
                let message = format!(
                    "Branch edit rejected; accepted state retained: {:?}",
                    transaction.outcome.result.solve().rejection
                );
                self.rejected_result(message, transaction.outcome.result);
            }
            Err(error) => self.rejected_change(format!("Branch state not changed: {error}")),
        }
    }

    pub(crate) fn confirm_inference(&mut self) {
        let Some(proposal) = self.inference.take() else {
            return;
        };
        if proposal.base_revision != self.session.revision() {
            self.rejected_change("Inference expired because the document changed.");
            return;
        }
        self.apply_edit(proposal.edit);
    }

    pub(crate) fn cancel_inference(&mut self) {
        self.inference = None;
    }

    pub(crate) fn undo(&mut self) {
        self.cancel_interaction();
        match self.session.undo(self.session.revision()) {
            Ok(outcome) if outcome.accepted() => self.accepted_change("Undo accepted."),
            Ok(outcome) => {
                let message = format!("Undo rejected: {:?}", outcome.result.solve().rejection);
                self.rejected_result(message, outcome.result);
            }
            Err(error) => self.rejected_change(error.to_string()),
        }
    }

    pub(crate) fn redo(&mut self) {
        self.cancel_interaction();
        match self.session.redo(self.session.revision()) {
            Ok(outcome) if outcome.accepted() => self.accepted_change("Redo accepted."),
            Ok(outcome) => {
                let message = format!("Redo rejected: {:?}", outcome.result.solve().rejection);
                self.rejected_result(message, outcome.result);
            }
            Err(error) => self.rejected_change(error.to_string()),
        }
    }

    pub(crate) fn import_json(&mut self, json: &str) {
        self.cancel_interaction();
        match self.session.import_json(self.session.revision(), json) {
            Ok(outcome) if outcome.accepted() => {
                self.selection.clear();
                self.fit_view();
                self.accepted_change("JSON import accepted and autosaved.");
            }
            Ok(outcome) => {
                let message = format!(
                    "Import rejected; accepted document retained: {:?}",
                    outcome.result.solve().rejection
                );
                self.rejected_result(message, outcome.result);
            }
            Err(error) => self.rejected_change(format!(
                "Import failed atomically; accepted document retained: {error}"
            )),
        }
    }

    pub(crate) fn export_json(&self) -> Result<String, String> {
        self.session
            .export_json()
            .map_err(|error| error.to_string())
    }

    pub(crate) fn storage_json(&mut self) -> Option<String> {
        if !self.storage_dirty {
            return None;
        }
        match self.export_json() {
            Ok(json) => Some(json),
            Err(error) => {
                self.last_attempt = format!("Autosave serialization failed: {error}");
                None
            }
        }
    }

    pub(crate) fn mark_storage_saved(&mut self) {
        self.storage_dirty = false;
    }

    pub(crate) fn zoom(&mut self, svg: [f64; 2], factor: f64) {
        self.viewport.zoom_at(svg, factor);
    }

    pub(crate) fn fit_view(&mut self) {
        let mut positions: Vec<_> = self
            .session
            .document()
            .points()
            .iter()
            .map(|point| point.position)
            .collect();
        positions.extend(
            sampled_curves(self.session.document())
                .into_iter()
                .flat_map(|(_, samples)| samples.into_iter().map(|(_, point)| point)),
        );
        if positions.is_empty() {
            self.viewport = Viewport::default();
            return;
        }
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for point in positions {
            for axis in 0..2 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        }
        self.viewport.center = [min[0] * 0.5 + max[0] * 0.5, min[1] * 0.5 + max[1] * 0.5];
        let width = finite_span(min[0], max[0], self.session.document().model_scale());
        let height = finite_span(min[1], max[1], self.session.document().model_scale());
        self.viewport.pixels_per_unit = (800.0 / width).min(520.0 / height).clamp(1.0e-12, 1.0e12);
    }

    pub(crate) fn set_object_selection(&mut self, item: SelectionItem, additive: bool) {
        if !additive {
            self.selection.clear();
        }
        if !self
            .selection
            .iter()
            .any(|selected| selected.same_object(item))
        {
            self.selection.push(item);
        }
    }

    pub(crate) fn toggle_contact_selection(&mut self, contact: ContactId) {
        if !self
            .selection
            .iter()
            .any(|item| matches!(item, SelectionItem::Contact(_)))
        {
            self.selection.clear();
        }
        let item = SelectionItem::Contact(contact);
        if let Some(index) = self
            .selection
            .iter()
            .position(|selected| selected.same_object(item))
        {
            self.selection.remove(index);
        } else {
            self.selection.push(item);
        }
    }

    fn selected_points(&self) -> Vec<DesignPointId> {
        self.selection
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Point(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    fn selected_curves(&self) -> Vec<(CurveSpan, f64)> {
        self.selection
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Curve { span, parameter } => Some((*span, *parameter)),
                _ => None,
            })
            .collect()
    }

    fn prune_selection(&mut self) {
        let document = self.session.document();
        self.selection.retain(|item| match item {
            SelectionItem::Point(id) => document.point(*id).is_some(),
            SelectionItem::Curve { span, .. } => document.curve(span.curve).is_some(),
            SelectionItem::Contact(id) => document.contact(*id).is_some(),
            SelectionItem::Constraint(id) => document.constraint(*id).is_some(),
            SelectionItem::Dimension(id) => document.dimension(*id).is_some(),
        });
    }

    fn selected(&self, item: SelectionItem) -> bool {
        self.selection
            .iter()
            .any(|selected| selected.same_object(item))
    }

    pub(crate) fn selection_summary(&self) -> String {
        if self.selection.is_empty() {
            return "Nothing selected".into();
        }
        let mut parts = Vec::new();
        for item in &self.selection {
            parts.push(match item {
                SelectionItem::Point(id) => self.session.document().point(*id).map_or_else(
                    || "missing point".into(),
                    |point| format!("point {}", point.label),
                ),
                SelectionItem::Curve { span, .. } => {
                    self.session.document().curve(span.curve).map_or_else(
                        || "missing curve".into(),
                        |curve| format!("curve {}", curve.label),
                    )
                }
                SelectionItem::Contact(id) => self.session.document().contact(*id).map_or_else(
                    || "missing contact".into(),
                    |contact| format!("contact {}", contact.label),
                ),
                SelectionItem::Constraint(id) => {
                    self.session.document().constraint(*id).map_or_else(
                        || "missing constraint".into(),
                        |source| format!("constraint {}", source.label),
                    )
                }
                SelectionItem::Dimension(id) => self.session.document().dimension(*id).map_or_else(
                    || "missing dimension".into(),
                    |source| format!("dimension {}", source.label),
                ),
            });
        }
        parts.join(", ")
    }

    pub(crate) fn interaction_help(&self) -> String {
        match self.tool {
            Tool::Select if self.gesture.is_some() => "Release to commit this projected interaction.".into(),
            Tool::Select => "Tap geometry to select or drag a control point. Drag empty space for box selection; Shift extends selection.".into(),
            Tool::Pan => "Drag to pan. Wheel or the zoom controls scale around the pointer.".into(),
            Tool::Draw(tool) => format!(
                "{} {} Pointer release stages each point; Undo point, Cancel, Escape and Backspace are available.",
                tool.label(),
                tool.stage_prompt(self.draft.len())
            ),
        }
    }

    pub(crate) fn draft_status(&self) -> String {
        match self.tool {
            Tool::Draw(tool) => {
                format!("{}: {}", tool.label(), tool.stage_prompt(self.draft.len()))
            }
            Tool::Select | Tool::Pan => "Choose a draw tool to begin.".into(),
        }
    }

    pub(crate) fn inference_label(&self) -> Option<&str> {
        self.inference
            .as_ref()
            .map(|proposal| proposal.label.as_str())
    }

    pub(crate) fn object_list_markup(&self) -> String {
        let result = self.display_session().accepted_result();
        self.object_list_markup_with_result(&result)
    }

    fn object_list_markup_with_result(&self, result: &DocumentSolveResult) -> String {
        let document = self.display_session().document();
        let mut markup = String::new();
        for point in document.points() {
            object_row(&mut markup, "point", point.id.0, &point.label, "");
        }
        for curve in document.curves() {
            object_row(&mut markup, "curve", curve.id.0, &curve.label, "");
        }
        for contact in document.contacts() {
            object_row(
                &mut markup,
                "contact",
                contact.id.0,
                &contact.label,
                &format!(
                    "w{} / {:?} / {:?}",
                    contact.winding, contact.neighborhood, contact.tangent_orientation
                ),
            );
        }
        for constraint in document.constraints() {
            object_row(
                &mut markup,
                "constraint",
                constraint.id.0,
                &constraint.label,
                if constraint.suppressed { "off" } else { "" },
            );
        }
        for dimension in document.dimensions() {
            let state = if dimension.suppressed {
                "off".into()
            } else if dimension.mode == DocumentDimensionMode::Reference {
                result
                    .accepted_reference_value(document, dimension.id)
                    .map_or_else(|| "reference".into(), |value| format!("ref {value:.6}"))
            } else {
                "driving".into()
            };
            object_row(
                &mut markup,
                "dimension",
                dimension.id.0,
                &dimension.label,
                &state,
            );
        }
        if markup.is_empty() {
            markup.push_str("<p class=\"selection-summary\">No persistent objects yet.</p>");
        }
        markup
    }

    pub(crate) fn solve_status_markup(&self) -> String {
        let result = self.display_session().accepted_result();
        Self::solve_status_markup_with_result(&result)
    }

    fn solve_status_markup_with_result(result: &DocumentSolveResult) -> String {
        let report = &result.accepted_view().core_report;
        let (rank, left_nullity, equality_dof, bounded_dof) = if report.rank_is_valid {
            (
                report.rank.to_string(),
                report.left_nullity.to_string(),
                report.right_nullity.to_string(),
                report.bidirectional_degrees_of_freedom.to_string(),
            )
        } else {
            (
                "unavailable".into(),
                "unavailable".into(),
                "unavailable".into(),
                "unavailable".into(),
            )
        };
        let structural = &report.structural;
        let structural_nullity = format!(
            "L{} / R{}",
            structural.structural_left_nullity, structural.structural_right_nullity
        );
        let backend = report.sparse_fallback_reason.map_or_else(
            || {
                format!(
                    "{:?} -> {:?}",
                    report.requested_backend, report.actual_backend
                )
            },
            |reason| {
                format!(
                    "{:?} -> {:?} ({reason:?})",
                    report.requested_backend, report.actual_backend
                )
            },
        );
        format!(
            "<div class=\"status-grid\"><div><span>hard validity</span><strong>{:?}</strong></div><div><span>normalized max</span><strong>{}</strong></div><div><span>numerical rank</span><strong>{rank}</strong></div><div><span>numerical left nullity</span><strong>{left_nullity}</strong></div><div><span>equality DOF</span><strong>{equality_dof}</strong></div><div><span>bounded DOF</span><strong>{bounded_dof}</strong></div><div><span>one-sided motion</span><strong>{:?}</strong></div><div><span>structural class</span><strong>{:?}</strong></div><div><span>structural rank</span><strong>{}</strong></div><div><span>structural nullity</span><strong>{structural_nullity}</strong></div><div><span>hard components</span><strong>{}</strong></div><div><span>linear backend</span><strong>{backend}</strong></div></div>",
            report.hard_validity,
            crate::format_metric(report.hard_residual_max),
            report.one_sided_mobility,
            structural.structural_classification,
            structural.structural_rank,
            structural.components,
        )
    }

    pub(crate) fn audit_markup(&self) -> String {
        let result = self.display_session().accepted_result();
        Self::audit_markup_with_result(&result)
    }

    fn audit_markup_with_result(result: &DocumentSolveResult) -> String {
        crate::audit_markup(&result.accepted_view().display_audit, &[])
    }

    pub(crate) fn last_attempt_markup(&self) -> String {
        let mut markup = format!(
            "<strong>Last action</strong><br>{}",
            crate::escape_html(&self.last_attempt)
        );
        if let Some(result) = &self.last_attempt_result {
            let report = &result.solve().core_report;
            let _ = write!(
                markup,
                "<br><span>conflict diagnostic: {:?}</span>",
                report.conflict_diagnostics.status
            );
            if !report.conflicting_sources.is_empty() {
                markup.push_str("<ul>");
                for source in &report.conflicting_sources {
                    let persistent = result.persistent_core_source(*source);
                    let label = persistent.and_then(|id| {
                        result
                            .attempted_mappings()
                            .source_mappings()
                            .iter()
                            .find_map(|mapping| {
                                (mapping.source_id == id).then_some(mapping.label.as_str())
                            })
                    });
                    let _ = write!(
                        markup,
                        "<li>{}</li>",
                        label.map_or_else(
                            || persistent
                                .map_or_else(|| "unmapped source".into(), |id| id.to_string()),
                            crate::escape_html,
                        )
                    );
                }
                markup.push_str("</ul>");
            }
        }
        markup
    }

    pub(crate) fn document_status(&self) -> String {
        let session = self.display_session();
        let document = session.document();
        format!(
            "{} points / {} curves / {} sources / revision {}",
            document.points().len(),
            document.curves().len(),
            document.constraints().len() + document.dimensions().len(),
            session.revision()
        )
    }

    pub(crate) fn accepted_is_valid(&self) -> bool {
        Self::result_is_valid(&self.display_session().accepted_result())
    }

    fn result_is_valid(result: &DocumentSolveResult) -> bool {
        result.accepted_view().core_report.hard_validity == HardValidity::Valid
    }

    pub(crate) fn render_svg(&self) -> String {
        let mut markup = String::new();
        render_grid(&mut markup, self.viewport);
        for (span, samples) in sampled_curves(self.document()) {
            if samples.len() < 2 {
                continue;
            }
            let selected = self.selected(SelectionItem::Curve {
                span,
                parameter: 0.0,
            });
            let mut path = String::new();
            for (index, (_, point)) in samples.iter().enumerate() {
                let svg = self.viewport.model_to_svg(*point);
                let _ = write!(
                    path,
                    "{} {:.3} {:.3}",
                    if index == 0 { 'M' } else { 'L' },
                    svg[0],
                    svg[1]
                );
            }
            let _ = write!(
                markup,
                "<path class=\"playground-curve{}\" data-curve-id=\"{}\" data-segment=\"{}\" d=\"{}\"><title>{}</title></path>",
                if selected { " selected" } else { "" },
                span.curve,
                span.segment,
                path,
                crate::escape_html(
                    self.document()
                        .curve(span.curve)
                        .map_or("curve", |curve| curve.label.as_str())
                )
            );
        }
        for point in self.document().points() {
            let svg = self.viewport.model_to_svg(point.position);
            let selected = self.selected(SelectionItem::Point(point.id));
            let _ = write!(
                markup,
                "<circle class=\"playground-point{}\" data-point-id=\"{}\" data-label=\"{}\" data-model-x=\"{:.17}\" data-model-y=\"{:.17}\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"7\"><title>{}</title></circle>",
                if selected { " selected" } else { "" },
                point.id,
                crate::escape_html(&point.label),
                point.position[0],
                point.position[1],
                svg[0],
                svg[1],
                crate::escape_html(&point.label)
            );
        }
        for contact in self.document().contacts() {
            if let Ok(jet) = self.document().evaluate_contact_jet(contact.id) {
                let position = [jet.position.x, jet.position.y];
                let svg = self.viewport.model_to_svg(position);
                let _ = write!(
                    markup,
                    "<circle class=\"playground-contact\" data-contact-id=\"{}\" data-label=\"{}\" data-model-x=\"{:.17}\" data-model-y=\"{:.17}\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"4\"><title>{}</title></circle>",
                    contact.id,
                    crate::escape_html(&contact.label),
                    position[0],
                    position[1],
                    svg[0],
                    svg[1],
                    crate::escape_html(&contact.label),
                );
            }
        }
        self.render_draft(&mut markup);
        if let Some(PointerGesture::BoxSelect {
            start_svg,
            current_svg,
            ..
        }) = self.gesture
        {
            let x = start_svg[0].min(current_svg[0]);
            let y = start_svg[1].min(current_svg[1]);
            let _ = write!(
                markup,
                "<rect class=\"selection-box\" x=\"{x:.3}\" y=\"{y:.3}\" width=\"{:.3}\" height=\"{:.3}\" />",
                (start_svg[0] - current_svg[0]).abs(),
                (start_svg[1] - current_svg[1]).abs()
            );
        }
        markup
    }

    #[allow(clippy::too_many_lines)]
    fn render_draft(&self, markup: &mut String) {
        let Tool::Draw(tool) = self.tool else {
            return;
        };
        let mut points = self.draft.clone();
        if let Some(cursor) = self.draft_cursor {
            points.push(cursor);
        }
        if points.is_empty() {
            return;
        }
        let svg_points = points
            .iter()
            .map(|point| self.viewport.model_to_svg(*point))
            .collect::<Vec<_>>();
        let mut polygon = String::new();
        for (index, svg) in svg_points.iter().enumerate() {
            let _ = write!(
                polygon,
                "{} {:.3} {:.3}",
                if index == 0 { 'M' } else { 'L' },
                svg[0],
                svg[1]
            );
        }
        if svg_points.len() >= 2 {
            let _ = write!(
                markup,
                "<path class=\"draft-geometry draft-control-polygon\" d=\"{polygon}\" />"
            );
        }
        match tool {
            DrawTool::Point => {
                let point = svg_points[0];
                let _ = write!(
                    markup,
                    "<circle class=\"draft-geometry draft-preview\" data-draft-kind=\"point\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"7\" />",
                    point[0], point[1]
                );
            }
            DrawTool::Line | DrawTool::Polyline if svg_points.len() >= 2 => {
                let kind = if tool == DrawTool::Line {
                    "line"
                } else {
                    "polyline"
                };
                let _ = write!(
                    markup,
                    "<path class=\"draft-geometry draft-preview\" data-draft-kind=\"{kind}\" d=\"{polygon}\" />"
                );
            }
            DrawTool::Rectangle if svg_points.len() >= 2 => {
                let first = svg_points[0];
                let second = *svg_points.last().unwrap();
                let _ = write!(
                    markup,
                    "<rect class=\"draft-geometry draft-preview\" data-draft-kind=\"rectangle\" x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" />",
                    first[0].min(second[0]),
                    first[1].min(second[1]),
                    (second[0] - first[0]).abs(),
                    (second[1] - first[1]).abs()
                );
            }
            DrawTool::Circle if svg_points.len() >= 2 => {
                let center = svg_points[0];
                let edge = *svg_points.last().unwrap();
                let _ = write!(
                    markup,
                    "<circle class=\"draft-geometry draft-preview\" data-draft-kind=\"circle\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\" />",
                    center[0],
                    center[1],
                    distance(center, edge)
                );
            }
            DrawTool::Arc if points.len() >= 3 => {
                let center = points[0];
                let start = points[1];
                let radius = distance(center, start);
                let start_angle = angle(center, start);
                let end_angle = angle(center, *points.last().unwrap());
                let signed_sweep = match self.arc_sweep {
                    DocumentArcSweep::CounterClockwise => (end_angle - start_angle).rem_euclid(TAU),
                    DocumentArcSweep::Clockwise => -(start_angle - end_angle).rem_euclid(TAU),
                };
                let end = [
                    center[0] + radius * end_angle.cos(),
                    center[1] + radius * end_angle.sin(),
                ];
                let center_svg = self.viewport.model_to_svg(center);
                let start_svg = self.viewport.model_to_svg(start);
                let end_svg = self.viewport.model_to_svg(end);
                let _ = write!(
                    markup,
                    "<path class=\"draft-geometry draft-preview\" data-draft-kind=\"arc\" d=\"M {:.3} {:.3} A {:.3} {:.3} 0 {} {} {:.3} {:.3}\" /><line class=\"draft-geometry\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" />",
                    start_svg[0],
                    start_svg[1],
                    radius * self.viewport.pixels_per_unit,
                    radius * self.viewport.pixels_per_unit,
                    u8::from(signed_sweep.abs() > std::f64::consts::PI),
                    u8::from(signed_sweep < 0.0),
                    end_svg[0],
                    end_svg[1],
                    center_svg[0],
                    center_svg[1],
                    end_svg[0],
                    end_svg[1]
                );
            }
            DrawTool::Quadratic if svg_points.len() >= 3 => {
                let [start, control, end] = [svg_points[0], svg_points[1], svg_points[2]];
                let _ = write!(
                    markup,
                    "<path class=\"draft-geometry draft-preview\" data-draft-kind=\"quadratic-bezier\" d=\"M {:.3} {:.3} Q {:.3} {:.3} {:.3} {:.3}\" />",
                    start[0], start[1], control[0], control[1], end[0], end[1]
                );
            }
            DrawTool::Cubic if svg_points.len() >= 4 => {
                let [start, first, second, end] =
                    [svg_points[0], svg_points[1], svg_points[2], svg_points[3]];
                let _ = write!(
                    markup,
                    "<path class=\"draft-geometry draft-preview\" data-draft-kind=\"cubic-bezier\" d=\"M {:.3} {:.3} C {:.3} {:.3} {:.3} {:.3} {:.3} {:.3}\" />",
                    start[0], start[1], first[0], first[1], second[0], second[1], end[0], end[1]
                );
            }
            _ => {}
        }
        for (index, svg) in svg_points.iter().enumerate() {
            let _ = write!(
                markup,
                "<circle class=\"draft-control\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"5\" /><text class=\"draft-label\" x=\"{:.3}\" y=\"{:.3}\">P{index}</text>",
                svg[0],
                svg[1],
                svg[0] + 8.0,
                svg[1] - 8.0
            );
        }
    }
}

#[derive(Debug)]
struct CreatedGeometry {
    selection: Vec<SelectionItem>,
    inference: Option<(String, DocumentEdit)>,
}

#[allow(clippy::too_many_lines)]
fn create_geometry(
    document: &mut SketchDocument,
    tool: DrawTool,
    positions: &[[f64; 2]],
    arc_sweep: DocumentArcSweep,
) -> Result<CreatedGeometry, geosolve_sketch::DocumentError> {
    let next = document.curves().len() + 1;
    let mut inference = None;
    let selection = match tool {
        DrawTool::Point => unreachable!("points use a direct command"),
        DrawTool::Rectangle => {
            let min = [
                positions[0][0].min(positions[1][0]),
                positions[0][1].min(positions[1][1]),
            ];
            let width = (positions[1][0] - positions[0][0]).abs();
            let height = (positions[1][1] - positions[0][1]).abs();
            let ids = document.add_rectangle(&format!("Rectangle {next}"), min, width, height)?;
            document.remove_with_owned_state(DocumentObjectId::Constraint(ids.anchor))?;
            for dimension in ids.dimensions {
                document.remove_with_owned_state(DocumentObjectId::Dimension(dimension))?;
            }
            ids.points
                .into_iter()
                .map(SelectionItem::Point)
                .chain(ids.curves.into_iter().map(|curve| SelectionItem::Curve {
                    span: CurveSpan::line(curve),
                    parameter: 0.5,
                }))
                .collect()
        }
        DrawTool::Line => {
            let points = add_points(document, positions, "Line control")?;
            let direction = normalized_direction(positions[0], positions[1])?;
            let curve = document.add_curve(
                format!("Line {next}"),
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: direction,
                },
            )?;
            let dx = direction[0].abs();
            let dy = direction[1].abs();
            if dy <= 0.08 {
                inference = Some((
                    "Horizontal line".into(),
                    DocumentEdit::CreateConstraint {
                        label: "inferred horizontal".into(),
                        definition: DocumentConstraintDefinition::Horizontal {
                            line: CurveSpan::line(curve),
                        },
                    },
                ));
            } else if dx <= 0.08 {
                inference = Some((
                    "Vertical line".into(),
                    DocumentEdit::CreateConstraint {
                        label: "inferred vertical".into(),
                        definition: DocumentConstraintDefinition::Vertical {
                            line: CurveSpan::line(curve),
                        },
                    },
                ));
            }
            created_curve_selection(&points, curve, 0.5)
        }
        DrawTool::Polyline => {
            let points = add_points(document, positions, "Polyline control")?;
            let directions = positions
                .windows(2)
                .map(|pair| normalized_direction(pair[0], pair[1]))
                .collect::<Result<Vec<_>, _>>()?;
            let curve = document.add_curve(
                format!("Polyline {next}"),
                CurveDefinition::Polyline {
                    points: points.clone(),
                    closed: false,
                    branch_directions: directions,
                },
            )?;
            created_curve_selection(&points, curve, 0.5)
        }
        DrawTool::Circle => {
            let center = document.add_point(format!("Circle {next} center"), positions[0])?;
            let radius_value = distance(positions[0], positions[1]);
            let radius = document.add_scalar(
                format!("Circle {next} radius"),
                radius_value,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )?;
            let curve = document.add_curve(
                format!("Circle {next}"),
                CurveDefinition::Circle { center, radius },
            )?;
            created_curve_selection(&[center], curve, 0.0)
        }
        DrawTool::Arc => {
            let center = document.add_point(format!("Arc {next} center"), positions[0])?;
            let radius = document.add_scalar(
                format!("Arc {next} radius"),
                distance(positions[0], positions[1]),
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )?;
            let start = angle(positions[0], positions[1]);
            let end = angle(positions[0], positions[2]);
            let start_angle = document.add_scalar(
                format!("Arc {next} start"),
                start,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )?;
            let end_angle = document.add_scalar(
                format!("Arc {next} end"),
                end,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )?;
            let curve = document.add_curve(
                format!("Arc {next}"),
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    sweep: arc_sweep,
                },
            )?;
            created_curve_selection(&[center], curve, 0.5)
        }
        DrawTool::Quadratic => {
            let points = add_points(document, positions, "Quadratic control")?;
            let controls = [points[0], points[1], points[2]];
            let curve = document.add_curve(
                format!("Quadratic {next}"),
                CurveDefinition::QuadraticBezier { controls },
            )?;
            created_curve_selection(&points, curve, 0.5)
        }
        DrawTool::Cubic => {
            let points = add_points(document, positions, "Cubic control")?;
            let controls = [points[0], points[1], points[2], points[3]];
            let curve = document.add_curve(
                format!("Cubic {next}"),
                CurveDefinition::CubicBezier { controls },
            )?;
            created_curve_selection(&points, curve, 0.5)
        }
    };
    Ok(CreatedGeometry {
        selection,
        inference,
    })
}

fn created_curve_selection(
    points: &[DesignPointId],
    curve: geosolve_sketch::CurveId,
    parameter: f64,
) -> Vec<SelectionItem> {
    points
        .iter()
        .copied()
        .map(SelectionItem::Point)
        .chain(std::iter::once(SelectionItem::Curve {
            span: CurveSpan::line(curve),
            parameter,
        }))
        .collect()
}

fn drag_stability_point(
    document: &SketchDocument,
    dragged: DesignPointId,
) -> Option<DesignPointId> {
    for constraint in document
        .constraints()
        .iter()
        .filter(|constraint| !constraint.suppressed)
    {
        let curve_contact = match constraint.definition {
            DocumentConstraintDefinition::LineCurveTangency {
                line,
                curve_contact,
                ..
            } if line_contains_point(document, line, dragged) => document.contact(curve_contact),
            DocumentConstraintDefinition::CurveCurveTangency {
                first_contact,
                second_contact,
            } => {
                let first = document.contact(first_contact)?;
                let second = document.contact(second_contact)?;
                if line_contains_point(document, first.curve, dragged) {
                    Some(second)
                } else if line_contains_point(document, second.curve, dragged) {
                    Some(first)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(contact) = curve_contact
            && let Some(control) = opposite_bezier_handle(document, contact)
        {
            return Some(control);
        }
    }
    let dragged_circle = document
        .curves()
        .iter()
        .find_map(|curve| match &curve.definition {
            CurveDefinition::Circle { center, .. } if *center == dragged => Some(curve.id),
            _ => None,
        })?;
    document
        .constraints()
        .iter()
        .filter(|constraint| !constraint.suppressed)
        .find_map(|constraint| {
            let DocumentConstraintDefinition::EqualRadius { first, second } =
                &constraint.definition
            else {
                return None;
            };
            let other = if *first == dragged_circle {
                *second
            } else if *second == dragged_circle {
                *first
            } else {
                return None;
            };
            let CurveDefinition::Circle { center, .. } = &document.curve(other)?.definition else {
                return None;
            };
            Some(*center)
        })
}

fn line_contains_point(document: &SketchDocument, span: CurveSpan, point: DesignPointId) -> bool {
    let Some(curve) = document.curve(span.curve) else {
        return false;
    };
    match &curve.definition {
        CurveDefinition::Line { start, end, .. } if span.segment == 0 => {
            *start == point || *end == point
        }
        CurveDefinition::Polyline { points, .. } => points
            .get(span.segment as usize..span.segment as usize + 2)
            .is_some_and(|segment| segment.contains(&point)),
        _ => false,
    }
}

fn opposite_bezier_handle(
    document: &SketchDocument,
    contact: &geosolve_sketch::ContactSlot,
) -> Option<DesignPointId> {
    let curve = document.curve(contact.curve.curve)?;
    match (&curve.definition, contact.neighborhood) {
        (CurveDefinition::CubicBezier { controls }, ContactNeighborhood::Start) => {
            Some(controls[2])
        }
        (CurveDefinition::CubicBezier { controls }, ContactNeighborhood::End) => Some(controls[1]),
        _ => None,
    }
}

fn add_points(
    document: &mut SketchDocument,
    positions: &[[f64; 2]],
    prefix: &str,
) -> Result<Vec<DesignPointId>, geosolve_sketch::DocumentError> {
    positions
        .iter()
        .enumerate()
        .map(|(index, position)| document.add_point(format!("{prefix} {}", index + 1), *position))
        .collect()
}

fn normalized_direction(
    first: [f64; 2],
    second: [f64; 2],
) -> Result<[f64; 2], geosolve_sketch::DocumentError> {
    let delta = [second[0] - first[0], second[1] - first[1]];
    let norm = delta[0].hypot(delta[1]);
    if !norm.is_finite() || norm <= 0.0 {
        return Err(geosolve_sketch::DocumentError::InvalidField {
            field: "drawn direction",
            message: "control points must be distinct".into(),
        });
    }
    Ok([delta[0] / norm, delta[1] / norm])
}

fn add_contact(
    document: &mut SketchDocument,
    selection: (CurveSpan, f64),
    tangent: bool,
    label: &str,
    neighborhood_choice: NeighborhoodChoice,
    tangent_orientation: TangentOrientation,
    winding: i32,
) -> Result<geosolve_sketch::ContactId, geosolve_sketch::DocumentError> {
    let parameter = match neighborhood_choice {
        NeighborhoodChoice::Start => 0.0,
        NeighborhoodChoice::End => 1.0,
        NeighborhoodChoice::Picked | NeighborhoodChoice::Interior => selection.1,
    };
    let neighborhood = match neighborhood_choice {
        NeighborhoodChoice::Picked => {
            document.picked_contact_neighborhood(selection.0, parameter)?
        }
        NeighborhoodChoice::Interior => ContactNeighborhood::Interior,
        NeighborhoodChoice::Start => ContactNeighborhood::Start,
        NeighborhoodChoice::End => ContactNeighborhood::End,
    };
    document.add_curve_contact(
        label,
        selection.0,
        parameter,
        winding,
        neighborhood,
        tangent.then_some(tangent_orientation),
    )
}

fn dimension_target(definition: &DocumentDimensionDefinition) -> geosolve_sketch::DesignScalarId {
    match definition {
        DocumentDimensionDefinition::PointDistance { target, .. }
        | DocumentDimensionDefinition::CurveLength { target, .. }
        | DocumentDimensionDefinition::Radius { target, .. }
        | DocumentDimensionDefinition::Diameter { target, .. }
        | DocumentDimensionDefinition::OrientedAngle { target, .. } => *target,
    }
}

type CurveSamples = (CurveSpan, Vec<(f64, [f64; 2])>);

fn sampled_curves(document: &SketchDocument) -> Vec<CurveSamples> {
    let mut output = Vec::new();
    for curve in document.curves() {
        let segment_count = match &curve.definition {
            CurveDefinition::Polyline { points, closed, .. } => {
                points.len().saturating_sub(usize::from(!*closed))
            }
            _ => 1,
        };
        for segment in 0..segment_count {
            let Ok(segment) = u32::try_from(segment) else {
                continue;
            };
            let span = CurveSpan {
                curve: curve.id,
                segment,
            };
            let circle = matches!(curve.definition, CurveDefinition::Circle { .. });
            let sample_count = if matches!(
                curve.definition,
                CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. }
            ) {
                1
            } else {
                CURVE_SAMPLES
            };
            let mut samples = Vec::new();
            for index in 0..=sample_count {
                let fraction = f64::from(index) / f64::from(sample_count);
                let parameter = if circle { fraction * TAU } else { fraction };
                if let Ok(jet) = document.evaluate_curve_jet(span, parameter) {
                    samples.push((parameter, [jet.position.x, jet.position.y]));
                }
            }
            output.push((span, samples));
        }
    }
    output
}

fn render_grid(markup: &mut String, viewport: Viewport) {
    let model_min = viewport.svg_to_model([0.0, CANVAS_HEIGHT]);
    let model_max = viewport.svg_to_model([CANVAS_WIDTH, 0.0]);
    let raw_step = 80.0 / viewport.pixels_per_unit;
    let exponent = raw_step.log10().floor();
    let base = 10.0_f64.powf(exponent);
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|factor| factor * base)
        .find(|step| *step >= raw_step)
        .unwrap_or(10.0 * base);
    let mut model_x = (model_min[0] / step).floor() * step;
    let end_x = (model_max[0] / step).ceil() * step;
    for _ in 0..2048 {
        if model_x > end_x {
            break;
        }
        let x = viewport.model_to_svg([model_x, 0.0])[0];
        let _ = write!(
            markup,
            "<line x1=\"{x:.3}\" y1=\"0\" x2=\"{x:.3}\" y2=\"700\" stroke=\"#1d2b30\" stroke-width=\"1\" />"
        );
        let next = model_x + step;
        if next.to_bits() == model_x.to_bits() || !next.is_finite() {
            break;
        }
        model_x = next;
    }
    let mut model_y = (model_min[1] / step).floor() * step;
    let end_y = (model_max[1] / step).ceil() * step;
    for _ in 0..2048 {
        if model_y > end_y {
            break;
        }
        let y = viewport.model_to_svg([0.0, model_y])[1];
        let _ = write!(
            markup,
            "<line x1=\"0\" y1=\"{y:.3}\" x2=\"1000\" y2=\"{y:.3}\" stroke=\"#1d2b30\" stroke-width=\"1\" />"
        );
        let next = model_y + step;
        if next.to_bits() == model_y.to_bits() || !next.is_finite() {
            break;
        }
        model_y = next;
    }
}

fn object_row(markup: &mut String, kind: &str, id: PersistentId, label: &str, state: &str) {
    let _ = write!(
        markup,
        "<div class=\"object-entry\"><button type=\"button\" class=\"object-row\" data-action=\"select-object\" data-kind=\"{kind}\" data-id=\"{id}\"><span class=\"kind\">{kind}</span><span>{}</span><span class=\"state\">{}</span></button>",
        crate::escape_html(label),
        crate::escape_html(state)
    );
    if matches!(kind, "constraint" | "dimension") {
        let _ = write!(
            markup,
            "<button type=\"button\" class=\"object-delete\" data-action=\"delete-object\" data-kind=\"{kind}\" data-id=\"{id}\" aria-label=\"Delete {kind} {}\">Delete</button>",
            crate::escape_html(label)
        );
    }
    markup.push_str("</div>");
}

fn unknown_point(id: DesignPointId) -> geosolve_sketch::DocumentError {
    geosolve_sketch::DocumentError::UnknownId {
        kind: "point",
        id: id.0,
    }
}

fn angle(center: [f64; 2], point: [f64; 2]) -> f64 {
    (point[1] - center[1]).atan2(point[0] - center[0])
}

fn distance(first: [f64; 2], second: [f64; 2]) -> f64 {
    (first[0] - second[0]).hypot(first[1] - second[1])
}

fn same_point_bits(first: [f64; 2], second: [f64; 2]) -> bool {
    first[0].to_bits() == second[0].to_bits() && first[1].to_bits() == second[1].to_bits()
}

fn finite_span(min: f64, max: f64, fallback: f64) -> f64 {
    let span = max - min;
    if span.is_finite() && span > 0.0 {
        span
    } else {
        fallback
    }
}

fn finite_screen_offset(value: f64, center: f64, pixels_per_unit: f64) -> f64 {
    let offset = (value - center) * pixels_per_unit;
    if offset.is_finite() {
        offset.clamp(-1.0e9, 1.0e9)
    } else if value >= center {
        1.0e9
    } else {
        -1.0e9
    }
}

fn point_in_rect(point: [f64; 2], min: [f64; 2], max: [f64; 2]) -> bool {
    point[0] >= min[0] && point[0] <= max[0] && point[1] >= min[1] && point[1] <= max[1]
}

fn point_segment_distance(point: [f64; 2], first: [f64; 2], second: [f64; 2]) -> (f64, f64) {
    let delta = [second[0] - first[0], second[1] - first[1]];
    let denominator = delta[0].mul_add(delta[0], delta[1] * delta[1]);
    if denominator <= f64::EPSILON {
        return (distance(point, first), 0.0);
    }
    let fraction = (((point[0] - first[0]) * delta[0] + (point[1] - first[1]) * delta[1])
        / denominator)
        .clamp(0.0, 1.0);
    let projection = [
        first[0] + fraction * delta[0],
        first[1] + fraction * delta[1],
    ];
    (distance(point, projection), fraction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(state: &mut PlaygroundState, tool: DrawTool, points: &[[f64; 2]]) {
        state.set_tool(Tool::Draw(tool));
        for point in points {
            state.draw_click(*point);
        }
        if tool == DrawTool::Polyline {
            state.finish_draft();
        }
    }

    #[test]
    fn viewport_transform_zoom_and_hit_geometry_round_trip() {
        let mut viewport = Viewport::default();
        let point = [2.5, -1.75];
        let svg = viewport.model_to_svg(point);
        assert!(distance(viewport.svg_to_model(svg), point) <= 1.0e-12);
        viewport.zoom_at(svg, 2.0);
        assert!(distance(viewport.svg_to_model(svg), point) <= 1.0e-12);
        let (distance, parameter) = point_segment_distance([5.0, 3.0], [0.0, 0.0], [10.0, 0.0]);
        assert!((distance - 3.0).abs() <= 1.0e-12);
        assert!((parameter - 0.5).abs() <= 1.0e-12);

        let extreme = Viewport {
            center: [0.0, 0.0],
            pixels_per_unit: 4.0,
        };
        assert!(
            extreme
                .model_to_svg([f64::MAX, -f64::MAX])
                .into_iter()
                .all(f64::is_finite)
        );
        let mut grid = String::new();
        render_grid(
            &mut grid,
            Viewport {
                center: [f64::MAX, f64::MAX],
                pixels_per_unit: 4.0,
            },
        );
        assert!(!grid.contains("inf"));
    }

    #[test]
    fn alpha_scale_extremes_fit_inside_the_editable_canvas() {
        for scale in [1.0e-6, 1.0, 1.0e6] {
            let state = PlaygroundState::example(AlphaScenarioKind::A1, scale).unwrap();
            for point in state.document().points() {
                let [x, y] = state.viewport().model_to_svg(point.position);
                assert!((0.0..=CANVAS_WIDTH).contains(&x), "scale={scale:e}: x={x}");
                assert!((0.0..=CANVAS_HEIGHT).contains(&y), "scale={scale:e}: y={y}");
            }
            assert!(state.viewport().pixels_per_unit.is_finite());
            assert!(state.viewport().pixels_per_unit > 0.0);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn advanced_constraint_stress_examples_render_valid_public_documents() {
        for (kind, labels, equality_dof, bounded_dof) in [
            (
                AlphaScenarioKind::StressCompass,
                ["Compass symmetric tips", "Compass opening angle 60 deg"],
                1,
                1,
            ),
            (
                AlphaScenarioKind::StressBridge,
                ["Bridge C1 endpoint tangency", "Bridge equal seam handles"],
                3,
                1,
            ),
            (
                AlphaScenarioKind::MotionCam,
                ["Left roller tangent to cam", "Cam rollers equal radius"],
                2,
                2,
            ),
            (
                AlphaScenarioKind::MotionOrbit,
                ["Orbit external tangency", "Orbit center distance reference"],
                1,
                1,
            ),
            (
                AlphaScenarioKind::MotionTrammel,
                ["Trammel bar length 5", "Trammel T bisects AM"],
                1,
                1,
            ),
            (
                AlphaScenarioKind::MotionScotchYoke,
                ["Yoke slot remains vertical", "Yoke crank radius 5"],
                1,
                1,
            ),
            (
                AlphaScenarioKind::MotionRotatingSquare,
                [
                    "Rotating square adjacent edges perpendicular",
                    "Rotating square opposite edges AB CD parallel",
                ],
                1,
                1,
            ),
            (
                AlphaScenarioKind::MotionScissor,
                [
                    "Scissor upper arms equal",
                    "Scissor joints mirror across base",
                ],
                1,
                1,
            ),
            (
                AlphaScenarioKind::MotionScissorTower,
                [
                    "Tower master diagonal length 10",
                    "Tower diagonal 10 matches master",
                ],
                1,
                1,
            ),
            (
                AlphaScenarioKind::MotionPeaucellier,
                [
                    "Peaucellier long bars equal",
                    "Peaucellier input circle radius 4",
                ],
                1,
                1,
            ),
            (
                AlphaScenarioKind::DiagnosticRankDrop,
                ["Rank distance A-P = 2", "Rank distance B-P = 2"],
                1,
                1,
            ),
            (
                AlphaScenarioKind::DiagnosticEndpointBound,
                [
                    "Endpoint follower on bounded rail",
                    "Endpoint-fixed contact t=1",
                ],
                2,
                0,
            ),
            (
                AlphaScenarioKind::DiagnosticRedundancy,
                ["Primary arm length 4", "Duplicate arm length 4"],
                0,
                0,
            ),
        ] {
            let state = PlaygroundState::example(kind, 1.0).unwrap();
            assert!(state.accepted_is_valid());
            let result = state.session().accepted_result();
            let report = &result.accepted_view().core_report;
            assert_eq!(report.right_nullity, equality_dof);
            assert_eq!(report.bidirectional_degrees_of_freedom, bounded_dof);
            let objects = state.object_list_markup();
            for label in labels {
                assert!(objects.contains(label));
            }
            assert!(state.render_svg().contains("playground-curve"));
        }
    }

    #[test]
    fn straight_curves_use_only_their_exact_endpoints() {
        let state = PlaygroundState::example(AlphaScenarioKind::Corpus, 1.0).unwrap();
        for (span, samples) in sampled_curves(state.document()) {
            let curve = state.document().curve(span.curve).unwrap();
            match curve.definition {
                CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => {
                    assert_eq!(samples.len(), 2, "{}", curve.label);
                    assert_eq!(samples[0].0.to_bits(), 0.0f64.to_bits());
                    assert_eq!(samples[1].0.to_bits(), 1.0f64.to_bits());
                }
                CurveDefinition::Circle { .. }
                | CurveDefinition::CircularArc { .. }
                | CurveDefinition::QuadraticBezier { .. }
                | CurveDefinition::CubicBezier { .. } => {
                    assert_eq!(samples.len(), CURVE_SAMPLES as usize + 1, "{}", curve.label);
                }
            }
        }
    }

    #[test]
    fn every_alpha_draw_tool_creates_one_atomic_history_entry() {
        let mut state = PlaygroundState::empty().unwrap();
        let fixtures = [
            (DrawTool::Line, vec![[-4.0, 0.0], [-2.0, 0.0]]),
            (
                DrawTool::Polyline,
                vec![[-4.0, -2.0], [-3.0, -1.0], [-2.0, -2.0]],
            ),
            (DrawTool::Rectangle, vec![[0.0, 0.0], [2.0, 1.5]]),
            (DrawTool::Circle, vec![[4.0, 0.0], [5.0, 0.0]]),
            (DrawTool::Arc, vec![[4.0, -3.0], [5.0, -3.0], [4.0, -2.0]]),
            (
                DrawTool::Quadratic,
                vec![[-1.0, 3.0], [0.0, 4.0], [1.0, 3.0]],
            ),
            (
                DrawTool::Cubic,
                vec![[2.0, 3.0], [3.0, 4.0], [4.0, 2.0], [5.0, 3.0]],
            ),
        ];
        for (index, (tool, points)) in fixtures.into_iter().enumerate() {
            draw(&mut state, tool, &points);
            assert_eq!(state.session().history_len(), index + 1);
            assert!(state.accepted_is_valid());
        }
        state.set_tool(Tool::Draw(DrawTool::Point));
        state.draw_click([7.0, 2.0]);
        assert_eq!(state.session().history_len(), 8);
        assert_eq!(state.document().curves().len(), 10);
        assert!(
            state
                .document()
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::QuadraticBezier { .. }))
        );
        assert!(
            state
                .document()
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::CubicBezier { .. }))
        );
        let markup = state.render_svg();
        assert!(markup.contains("playground-curve"));
        assert!(markup.contains("data-point-id"));
    }

    #[test]
    fn every_draw_tool_has_a_staged_primitive_preview() {
        let fixtures = [
            (DrawTool::Point, vec![], [1.0, 1.0], "point"),
            (DrawTool::Line, vec![[0.0, 0.0]], [2.0, 1.0], "line"),
            (
                DrawTool::Polyline,
                vec![[0.0, 0.0], [1.0, 1.0]],
                [2.0, 0.0],
                "polyline",
            ),
            (
                DrawTool::Rectangle,
                vec![[0.0, 0.0]],
                [2.0, 1.0],
                "rectangle",
            ),
            (DrawTool::Circle, vec![[0.0, 0.0]], [2.0, 0.0], "circle"),
            (
                DrawTool::Arc,
                vec![[0.0, 0.0], [2.0, 0.0]],
                [0.0, 3.0],
                "arc",
            ),
            (
                DrawTool::Quadratic,
                vec![[0.0, 0.0], [1.0, 2.0]],
                [2.0, 0.0],
                "quadratic-bezier",
            ),
            (
                DrawTool::Cubic,
                vec![[0.0, 0.0], [1.0, 2.0], [2.0, -1.0]],
                [3.0, 0.0],
                "cubic-bezier",
            ),
        ];
        for (tool, staged, cursor, kind) in fixtures {
            let mut state = PlaygroundState::empty().unwrap();
            state.set_tool(Tool::Draw(tool));
            for point in staged {
                state.draw_click(point);
            }
            state.set_draft_cursor(cursor);
            let markup = state.render_svg();
            assert!(
                markup.contains(&format!("data-draft-kind=\"{kind}\"")),
                "missing {kind}: {markup}"
            );
            assert!(state.document().points().is_empty());
            assert_eq!(state.session().history_len(), 0);
        }
    }

    #[test]
    fn pointer_cancel_and_invalid_completion_retain_the_staged_draft() {
        let mut state = PlaygroundState::empty().unwrap();
        state.set_tool(Tool::Draw(DrawTool::Line));
        let first = state.viewport().model_to_svg([0.0, 0.0]);
        state.begin_draft_placement(21, first);
        state.end_gesture(21, false);
        assert!(state.draft.is_empty());
        assert_eq!(state.session().history_len(), 0);

        state.begin_draft_placement(22, first);
        state.end_gesture(22, true);
        assert_eq!(state.draft, vec![[0.0, 0.0]]);
        state.begin_draft_placement(23, first);
        state.end_gesture(23, true);
        assert_eq!(state.draft, vec![[0.0, 0.0], [0.0, 0.0]]);
        assert!(state.document().curves().is_empty());
        assert_eq!(state.session().history_len(), 0);

        state.undo_draft_point();
        let second = state.viewport().model_to_svg([2.0, 0.0]);
        state.begin_draft_placement(24, second);
        state.end_gesture(24, true);
        assert!(state.draft.is_empty());
        assert_eq!(state.document().curves().len(), 1);
        assert_eq!(state.session().history_len(), 1);
    }

    #[test]
    fn deleting_each_new_shape_removes_its_generated_controls() {
        let fixtures = [
            (DrawTool::Line, vec![[0.0, 0.0], [2.0, 0.0]]),
            (DrawTool::Polyline, vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]]),
            (DrawTool::Rectangle, vec![[0.0, 0.0], [2.0, 1.0]]),
            (DrawTool::Circle, vec![[0.0, 0.0], [1.0, 0.0]]),
            (DrawTool::Arc, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]),
            (
                DrawTool::Quadratic,
                vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]],
            ),
            (
                DrawTool::Cubic,
                vec![[0.0, 0.0], [1.0, 1.0], [2.0, -1.0], [3.0, 0.0]],
            ),
        ];
        for (tool, points) in fixtures {
            let mut state = PlaygroundState::empty().unwrap();
            draw(&mut state, tool, &points);
            state.delete_selection();
            assert!(state.document().points().is_empty(), "{tool:?}");
            assert!(state.document().scalars().is_empty(), "{tool:?}");
            assert!(state.document().curves().is_empty(), "{tool:?}");
            assert!(state.document().constraints().is_empty(), "{tool:?}");
            assert!(state.document().dimensions().is_empty(), "{tool:?}");
        }
    }

    #[test]
    fn selection_constraints_dimensions_drag_history_and_json_use_document_session() {
        let mut state = PlaygroundState::empty().unwrap();
        draw(&mut state, DrawTool::Line, &[[-2.0, 0.0], [2.0, 0.0]]);
        state.set_tool(Tool::Draw(DrawTool::Point));
        state.draw_click([0.0, 0.0]);
        let line = state.document().curves()[0].id;
        let point = state.document().points().last().unwrap().id;
        state.selection = vec![
            SelectionItem::Point(point),
            SelectionItem::Curve {
                span: CurveSpan::line(line),
                parameter: 0.5,
            },
        ];
        state.apply_constraint(10);
        assert_eq!(state.document().constraints().len(), 1);
        assert_eq!(state.document().contacts().len(), 1);

        state.selection = vec![SelectionItem::Curve {
            span: CurveSpan::line(line),
            parameter: 0.5,
        }];
        state.apply_dimension(1, DocumentDimensionMode::Reference, 4.0);
        assert_eq!(state.document().dimensions().len(), 1);
        state.toggle_selected_sources();
        assert!(state.document().dimensions()[0].suppressed);
        state.toggle_selected_sources();
        assert!(!state.document().dimensions()[0].suppressed);

        state.set_tool(Tool::Draw(DrawTool::Point));
        state.draw_click([3.0, 3.0]);
        let drag_point = state.document().points().last().unwrap().id;
        let start_svg = state.viewport().model_to_svg([3.0, 3.0]);
        state.begin_point_drag(7, drag_point, start_svg);
        let target_svg = state.viewport().model_to_svg([3.5, 3.0]);
        state.update_gesture(7, target_svg);
        state.update_gesture(7, state.viewport().model_to_svg([3.6, 3.0]));
        assert!(state.preview_active());
        assert_eq!(
            state.drag_preview.as_ref().unwrap().session.history_len(),
            state.session().history_len()
        );
        state.end_gesture(7, true);
        assert!(!state.preview_active());
        let dragged = state.document().point(drag_point).unwrap().position;
        assert!(
            (dragged[0] - 3.6).abs() <= 1.0e-8,
            "{dragged:?}; {}",
            state.last_attempt
        );

        let json = state.export_json().unwrap();
        let revision = state.session().revision();
        state.import_json("{not valid json");
        assert_eq!(state.session().revision(), revision);
        assert_eq!(state.export_json().unwrap(), json);
        state.undo();
        state.redo();
        assert!(state.accepted_is_valid());
        assert!(state.solve_status_markup().contains("equality DOF"));
        assert!(state.solve_status_markup().contains("structural class"));
        assert!(state.solve_status_markup().contains("linear backend"));
        assert!(state.audit_markup().contains("audit"));
    }

    #[test]
    fn free_line_drag_crosses_its_inactive_branch() {
        let mut free = PlaygroundState::empty().unwrap();
        draw(&mut free, DrawTool::Line, &[[0.0, 0.0], [2.0, 0.0]]);
        let end = free.document().points()[1].id;
        let start = free.viewport().model_to_svg([2.0, 0.0]);
        free.begin_point_drag(11, end, start);
        free.update_gesture(11, free.viewport().model_to_svg([-2.0, 0.0]));
        assert!(distance(free.document().point(end).unwrap().position, [-2.0, 0.0]) <= 1.0e-8);
        assert!(!free.last_attempt.contains("opposite branch"));
        assert_eq!(free.drag_preview.as_ref().unwrap().session.history_len(), 1);
        free.end_gesture(11, true);
        assert_eq!(free.session().history_len(), 2);
        assert!(distance(free.document().point(end).unwrap().position, [-2.0, 0.0]) <= 1.0e-8);
        let start_position = free.document().points()[0].position;
        let end_position = free.document().point(end).unwrap().position;
        assert!(
            end_position[0] < start_position[0],
            "start={start_position:?}, end={end_position:?}"
        );
        let CurveDefinition::Line {
            branch_direction, ..
        } = &free.document().curves()[0].definition
        else {
            panic!("line expected");
        };
        assert!(distance(*branch_direction, [1.0, 0.0]) <= f64::EPSILON);
        let line = free.document().curves()[0].id;
        free.selection = vec![SelectionItem::Curve {
            span: CurveSpan::line(line),
            parameter: 0.5,
        }];
        free.apply_constraint(2);
        assert!(
            free.last_attempt.contains("accepted"),
            "{}",
            free.last_attempt
        );
        let CurveDefinition::Line {
            branch_direction, ..
        } = &free.document().curve(line).unwrap().definition
        else {
            panic!("line expected");
        };
        assert!(branch_direction[0] < -0.999_999, "{branch_direction:?}");
        free.selection = vec![SelectionItem::Curve {
            span: CurveSpan::line(line),
            parameter: 0.5,
        }];
        free.apply_dimension(1, DocumentDimensionMode::Driving, 2.0);
        assert!(
            free.last_attempt.contains("accepted"),
            "{}",
            free.last_attempt
        );
        let CurveDefinition::Line {
            branch_direction, ..
        } = &free.document().curve(line).unwrap().definition
        else {
            panic!("line expected");
        };
        assert!(
            branch_direction[0] < -0.999_999 && branch_direction[1].abs() <= 1.0e-8,
            "{branch_direction:?}"
        );
    }

    #[test]
    fn a5_line_endpoint_drag_stabilizes_the_opposite_bezier_handle() {
        let mut a5 = PlaygroundState::example(AlphaScenarioKind::A5, 1.0).unwrap();
        let line_end = a5
            .document()
            .points()
            .iter()
            .find(|point| point.label == "A5 line B")
            .unwrap()
            .id;
        let stable_controls = ["A5 P2", "A5 P3"].map(|label| {
            let point = a5
                .document()
                .points()
                .iter()
                .find(|point| point.label == label)
                .unwrap();
            (point.id, point.position)
        });
        assert_eq!(
            drag_stability_point(a5.document(), line_end),
            Some(stable_controls[0].0)
        );
        let start = a5.viewport().model_to_svg([2.0, 0.0]);
        let target = [2.0f64.sqrt(), 2.0f64.sqrt()];
        a5.begin_point_drag(12, line_end, start);
        for step in 1..=8 {
            let fraction = f64::from(step) / 8.0;
            a5.update_gesture(
                12,
                a5.viewport()
                    .model_to_svg([2.0 + (target[0] - 2.0) * fraction, target[1] * fraction]),
            );
        }
        assert!(a5.preview_active());
        assert!(a5.last_attempt_result.is_none(), "{}", a5.last_attempt);
        let preview_request = a5.drag_preview.as_ref().unwrap().session.request();
        assert!(!preview_request.previous_state_preferences);
        assert_eq!(
            preview_request.stability_target,
            Some(geosolve_sketch::DocumentDragTarget {
                point: stable_controls[0].0,
                target: stable_controls[0].1,
            })
        );
        assert!(distance(a5.document().point(line_end).unwrap().position, target) <= 1.0e-8);
        for (point, before) in stable_controls {
            assert!(distance(a5.document().point(point).unwrap().position, before) <= 1.0e-8);
        }
        a5.end_gesture(12, true);
        assert!(a5.accepted_is_valid());
        assert_eq!(a5.session().history_len(), 1);
        assert!(distance(a5.document().point(line_end).unwrap().position, target) <= 1.0e-8);
        for (point, before) in stable_controls {
            assert!(distance(a5.document().point(point).unwrap().position, before) <= 1.0e-8);
        }
    }

    #[test]
    fn drawn_rectangle_has_free_size_and_full_geometry_delete_cascades() {
        let mut state = PlaygroundState::empty().unwrap();
        draw(&mut state, DrawTool::Rectangle, &[[0.0, 0.0], [4.0, 3.0]]);
        assert_eq!(state.document().constraints().len(), 4);
        assert!(state.document().dimensions().is_empty());
        assert!(state.document().scalars().is_empty());
        assert_eq!(state.selection.len(), 8);
        assert_eq!(
            state
                .session()
                .accepted_result()
                .accepted_view()
                .core_report
                .right_nullity,
            4
        );
        let points = state
            .document()
            .points()
            .iter()
            .map(|point| (point.id, point.position))
            .collect::<Vec<_>>();
        let dragged = points[0].0;
        state.begin_point_drag(13, dragged, state.viewport().model_to_svg(points[0].1));
        state.update_gesture(13, state.viewport().model_to_svg([1.0, 1.0]));
        state.end_gesture(13, true);
        let expected = [[1.0, 1.0], [4.0, 1.0], [4.0, 3.0], [1.0, 3.0]];
        for ((point, _), expected) in points.iter().zip(expected) {
            let after = state.document().point(*point).unwrap().position;
            assert!(distance(after, expected) <= 1.0e-8, "{after:?}");
        }
        state.selection = state
            .document()
            .points()
            .iter()
            .map(|point| SelectionItem::Point(point.id))
            .chain(
                state
                    .document()
                    .curves()
                    .iter()
                    .map(|curve| SelectionItem::Curve {
                        span: CurveSpan::line(curve.id),
                        parameter: 0.5,
                    }),
            )
            .collect();
        state.delete_selection();
        assert!(state.document().points().is_empty());
        assert!(state.document().scalars().is_empty());
        assert!(state.document().curves().is_empty());
        assert!(state.document().constraints().is_empty());
        assert!(state.document().dimensions().is_empty());

        let mut a1 = PlaygroundState::example(AlphaScenarioKind::A1, 1.0).unwrap();
        assert_eq!(
            a1.object_list_markup()
                .matches("data-action=\"delete-object\"")
                .count(),
            a1.document().constraints().len() + a1.document().dimensions().len()
        );
        a1.selection = a1
            .document()
            .points()
            .iter()
            .map(|point| SelectionItem::Point(point.id))
            .chain(
                a1.document()
                    .curves()
                    .iter()
                    .map(|curve| SelectionItem::Curve {
                        span: CurveSpan::line(curve.id),
                        parameter: 0.5,
                    }),
            )
            .collect();
        a1.delete_selection();
        assert!(a1.document().points().is_empty());
        assert!(a1.document().scalars().is_empty());
        assert!(a1.document().curves().is_empty());
        assert!(a1.document().constraints().is_empty());
        assert!(a1.document().dimensions().is_empty());
        assert_eq!(a1.session().history_len(), 1);
    }

    #[test]
    fn inference_is_provisional_until_confirmed() {
        let mut state = PlaygroundState::empty().unwrap();
        draw(&mut state, DrawTool::Line, &[[0.0, 0.0], [2.0, 0.01]]);
        assert_eq!(state.document().constraints().len(), 0);
        assert_eq!(state.inference_label(), Some("Horizontal line"));
        state.cancel_inference();
        assert_eq!(state.document().constraints().len(), 0);

        draw(&mut state, DrawTool::Line, &[[0.0, 1.0], [2.0, 1.0]]);
        state.confirm_inference();
        assert_eq!(state.document().constraints().len(), 1);
    }

    #[test]
    fn page_exposes_document_tools_mobile_input_and_accepted_diagnostics() {
        let page = include_str!("../index.html");
        for tool in [
            "select",
            "pan",
            "point",
            "line",
            "polyline",
            "rectangle",
            "circle",
            "arc",
            "quadratic",
            "cubic",
        ] {
            assert!(page.contains(&format!("data-tool=\"{tool}\"")));
        }
        assert!(page.contains("id=\"sketch-viewport\""));
        assert!(page.contains("data-action=\"undo\""));
        assert!(page.contains("data-action=\"import-json\""));
        assert!(page.contains("data-action=\"download-json\""));
        assert!(page.contains("id=\"document-file\""));
        assert!(page.contains("data-action=\"load-example\""));
        assert!(page.contains("value=\"stress-compass\""));
        assert!(page.contains("value=\"stress-bridge\""));
        assert!(page.contains("value=\"motion-cam\""));
        assert!(page.contains("value=\"motion-orbit\""));
        assert!(page.contains("value=\"motion-trammel\""));
        assert!(page.contains("value=\"motion-scotch-yoke\""));
        assert!(page.contains("value=\"motion-rotating-square\""));
        assert!(page.contains("value=\"motion-scissor\""));
        assert!(page.contains("value=\"motion-scissor-tower\""));
        assert!(page.contains("value=\"motion-peaucellier\""));
        assert!(page.contains("value=\"diagnostic-rank-drop\""));
        assert!(page.contains("value=\"diagnostic-endpoint-bound\""));
        assert!(page.contains("value=\"diagnostic-redundancy\""));
        assert!(page.contains("<optgroup label=\"Solver diagnostics\">"));
        assert!(page.contains("data-action=\"undo-draft\""));
        assert!(page.contains("data-action=\"cancel-draft\""));
        assert!(page.contains("data-action=\"confirm-inference\""));
        assert!(page.contains("Quadratic Bézier"));
        assert!(page.contains("Cubic Bézier"));
        assert!(page.contains("aria-live=\"polite\""));
        let styles = include_str!("../styles.css");
        assert!(styles.contains("#sketch-viewport"));
        assert!(styles.contains("touch-action: none"));
        assert!(styles.contains("@media (max-width: 760px)"));
    }

    #[test]
    fn click_without_motion_preserves_history_and_polyline_spans_multiselect() {
        let mut state = PlaygroundState::empty().unwrap();
        state.set_tool(Tool::Draw(DrawTool::Point));
        state.draw_click([0.0, 0.0]);
        let point = state.document().points()[0].id;
        let history = state.session().history_len();
        let svg = state.viewport().model_to_svg([0.0, 0.0]);
        state.begin_point_drag(5, point, svg);
        assert!(!state.update_gesture(5, svg));
        assert!(!state.update_gesture(5, [svg[0] + 2.0, svg[1]]));
        state.end_gesture(5, true);
        assert_eq!(state.session().history_len(), history);

        state.begin_point_drag(6, point, svg);
        let target = state.viewport().model_to_svg([1.0, 0.0]);
        assert!(state.update_gesture(6, target));
        assert!(!state.update_gesture(6, target));
        state.end_gesture(6, false);
        assert_eq!(state.session().history_len(), history);

        draw(
            &mut state,
            DrawTool::Polyline,
            &[[2.0, 0.0], [3.0, 1.0], [4.0, 0.0]],
        );
        let polyline = state.document().curves()[0].id;
        state.set_object_selection(
            SelectionItem::Curve {
                span: CurveSpan {
                    curve: polyline,
                    segment: 0,
                },
                parameter: 0.5,
            },
            false,
        );
        state.set_object_selection(
            SelectionItem::Curve {
                span: CurveSpan {
                    curve: polyline,
                    segment: 1,
                },
                parameter: 0.5,
            },
            true,
        );
        assert_eq!(state.selected_curves().len(), 2);
    }

    #[test]
    fn conflict_attempt_is_mapped_separately_from_retained_accepted_view() {
        let mut state = PlaygroundState::empty().unwrap();
        state.set_tool(Tool::Draw(DrawTool::Point));
        state.draw_click([0.0, 0.0]);
        state.draw_click([1.0, 0.0]);
        let points: Vec<_> = state
            .document()
            .points()
            .iter()
            .map(|point| point.id)
            .collect();
        state.selection = points.iter().copied().map(SelectionItem::Point).collect();
        state.apply_dimension(0, DocumentDimensionMode::Driving, 1.0);
        let accepted_json = state.export_json().unwrap();
        state.selection = points.iter().copied().map(SelectionItem::Point).collect();
        state.apply_dimension(0, DocumentDimensionMode::Driving, 2.0);
        assert_eq!(state.export_json().unwrap(), accepted_json);
        assert!(state.last_attempt_result.is_some());
        let markup = state.last_attempt_markup();
        assert!(markup.contains("conflict diagnostic"));
        assert!(markup.contains("dimension"));
    }

    #[test]
    fn explicit_arc_branch_reference_measurement_and_imported_labels_render_truthfully() {
        let mut state = PlaygroundState::empty().unwrap();
        state.set_branch_options(
            DocumentArcSweep::Clockwise,
            ContactBranchOptions {
                neighborhood: NeighborhoodChoice::Picked,
                tangent_orientation: TangentOrientation::Opposed,
                winding: -2,
            },
            ContactBranchOptions {
                neighborhood: NeighborhoodChoice::Picked,
                tangent_orientation: TangentOrientation::Aligned,
                winding: 0,
            },
            DocumentAngleOrientation::Clockwise,
        );
        draw(
            &mut state,
            DrawTool::Arc,
            &[[0.0, 0.0], [2.0, 0.0], [0.0, -2.0]],
        );
        assert!(matches!(
            state.document().curves()[0].definition,
            CurveDefinition::CircularArc {
                sweep: DocumentArcSweep::Clockwise,
                ..
            }
        ));
        let arc = state.document().curves()[0].id;
        state.selection = vec![SelectionItem::Curve {
            span: CurveSpan::line(arc),
            parameter: 0.5,
        }];
        state.apply_dimension(2, DocumentDimensionMode::Reference, 2.0);
        assert!(state.object_list_markup().contains("ref 2.000000"));

        state
            .session
            .transact(state.session.revision(), "hostile label", |document| {
                let first = document.add_point("safe A", [4.0, 0.0])?;
                let second = document.add_point("safe B", [5.0, 0.0])?;
                document.add_curve(
                    "</title><script>alert(1)</script>",
                    CurveDefinition::Line {
                        start: first,
                        end: second,
                        branch_direction: [1.0, 0.0],
                    },
                )
            })
            .unwrap();
        let svg = state.render_svg();
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;/title&gt;"));
    }

    #[test]
    fn deleting_a_contact_constraint_removes_its_owned_hidden_state() {
        let mut state = PlaygroundState::empty().unwrap();
        draw(&mut state, DrawTool::Line, &[[-1.0, 0.0], [1.0, 0.0]]);
        state.set_tool(Tool::Draw(DrawTool::Point));
        state.draw_click([0.0, 0.0]);
        let curve = state.document().curves()[0].id;
        let point = state.document().points().last().unwrap().id;
        state.selection = vec![
            SelectionItem::Point(point),
            SelectionItem::Curve {
                span: CurveSpan::line(curve),
                parameter: 0.5,
            },
        ];
        state.apply_constraint(10);
        assert_eq!(state.document().contacts().len(), 1);
        let constraint = state.document().constraints()[0].id;
        let markup = state.object_list_markup();
        assert_eq!(markup.matches("data-action=\"delete-object\"").count(), 1);
        assert!(markup.contains("aria-label=\"Delete constraint point on curve\""));
        state.delete_object(DocumentObjectId::Constraint(constraint));
        assert!(state.document().constraints().is_empty());
        assert!(state.document().contacts().is_empty());
        state.selection = vec![SelectionItem::Curve {
            span: CurveSpan::line(curve),
            parameter: 0.5,
        }];
        state.delete_selection();
        assert!(state.document().curves().is_empty());
    }

    #[test]
    fn endpoint_tangency_and_persisted_branch_edits_use_explicit_state() {
        let mut state = PlaygroundState::empty().unwrap();
        draw(&mut state, DrawTool::Line, &[[0.0, 0.0], [2.0, 0.0]]);
        draw(
            &mut state,
            DrawTool::Cubic,
            &[[0.0, 0.0], [1.0, 0.0], [2.0, 1.0], [3.0, 1.0]],
        );
        state.set_branch_options(
            DocumentArcSweep::CounterClockwise,
            ContactBranchOptions {
                neighborhood: NeighborhoodChoice::Start,
                tangent_orientation: TangentOrientation::Aligned,
                winding: 0,
            },
            ContactBranchOptions {
                neighborhood: NeighborhoodChoice::Start,
                tangent_orientation: TangentOrientation::Aligned,
                winding: 0,
            },
            DocumentAngleOrientation::CounterClockwise,
        );
        select_all_curves(&mut state);
        state.apply_constraint(12);
        assert_eq!(
            state.document().contacts().len(),
            2,
            "{}",
            state.last_attempt
        );
        for contact in state.document().contacts() {
            assert_eq!(contact.neighborhood, ContactNeighborhood::Start);
            assert!(
                state
                    .document()
                    .scalar(contact.parameter)
                    .unwrap()
                    .value
                    .abs()
                    <= f64::EPSILON
            );
        }

        let contacts: Vec<_> = state
            .document()
            .contacts()
            .iter()
            .map(|contact| SelectionItem::Contact(contact.id))
            .collect();
        state.selection = contacts;
        state.apply_branch_state();
        assert!(state.accepted_is_valid());

        let mut arc_state = PlaygroundState::empty().unwrap();
        draw(
            &mut arc_state,
            DrawTool::Arc,
            &[[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]],
        );
        let arc = arc_state.document().curves()[0].id;
        arc_state.set_branch_options(
            DocumentArcSweep::Clockwise,
            ContactBranchOptions {
                neighborhood: NeighborhoodChoice::Picked,
                tangent_orientation: TangentOrientation::Aligned,
                winding: 0,
            },
            ContactBranchOptions {
                neighborhood: NeighborhoodChoice::Picked,
                tangent_orientation: TangentOrientation::Aligned,
                winding: 0,
            },
            DocumentAngleOrientation::CounterClockwise,
        );
        arc_state.selection = vec![SelectionItem::Curve {
            span: CurveSpan::line(arc),
            parameter: 0.5,
        }];
        arc_state.apply_branch_state();
        assert!(matches!(
            arc_state.document().curve(arc).unwrap().definition,
            CurveDefinition::CircularArc {
                sweep: DocumentArcSweep::Clockwise,
                ..
            }
        ));
    }

    #[test]
    fn paired_contacts_keep_independent_neighborhoods_and_touch_selection() {
        let mut state = PlaygroundState::empty().unwrap();
        draw(&mut state, DrawTool::Line, &[[0.0, 0.0], [1.0, 0.0]]);
        draw(&mut state, DrawTool::Line, &[[-1.0, 0.0], [1.0, 0.0]]);
        state.set_branch_options(
            DocumentArcSweep::CounterClockwise,
            ContactBranchOptions {
                neighborhood: NeighborhoodChoice::Start,
                tangent_orientation: TangentOrientation::Aligned,
                winding: 0,
            },
            ContactBranchOptions {
                neighborhood: NeighborhoodChoice::Interior,
                tangent_orientation: TangentOrientation::Aligned,
                winding: 0,
            },
            DocumentAngleOrientation::CounterClockwise,
        );
        select_all_curves(&mut state);
        state.apply_constraint(11);
        assert_eq!(
            state.document().contacts().len(),
            2,
            "{}",
            state.last_attempt
        );
        let first = &state.document().contacts()[0];
        let second = &state.document().contacts()[1];
        assert_eq!(first.neighborhood, ContactNeighborhood::Start);
        assert_eq!(second.neighborhood, ContactNeighborhood::Interior);
        assert!(
            state
                .document()
                .scalar(first.parameter)
                .unwrap()
                .value
                .abs()
                <= f64::EPSILON
        );
        assert!(
            (state.document().scalar(second.parameter).unwrap().value - 0.5).abs() <= f64::EPSILON
        );

        let first_id = first.id;
        let second_id = second.id;
        state.selection = vec![
            SelectionItem::Contact(second_id),
            SelectionItem::Contact(first_id),
        ];
        state.apply_branch_state();
        assert_eq!(
            state.document().contact(first_id).unwrap().neighborhood,
            ContactNeighborhood::Start
        );
        assert_eq!(
            state.document().contact(second_id).unwrap().neighborhood,
            ContactNeighborhood::Interior
        );

        state.selection.clear();
        state.toggle_contact_selection(first_id);
        state.toggle_contact_selection(second_id);
        assert_eq!(
            state
                .selection
                .iter()
                .filter(|item| matches!(item, SelectionItem::Contact(_)))
                .count(),
            2
        );
        state.toggle_contact_selection(first_id);
        assert_eq!(state.selection, vec![SelectionItem::Contact(second_id)]);
    }

    #[test]
    fn autosave_payload_retries_until_browser_confirms_storage() {
        let mut state = PlaygroundState::empty().unwrap();
        let first = state.storage_json().unwrap();
        assert_eq!(state.storage_json().unwrap(), first);
        state.mark_storage_saved();
        assert!(state.storage_json().is_none());
    }

    #[test]
    fn all_constraint_buttons_create_their_public_document_definition() {
        for kind in 0..=12 {
            let mut state = PlaygroundState::empty().unwrap();
            match kind {
                0 => {
                    state.set_tool(Tool::Draw(DrawTool::Point));
                    state.draw_click([0.0, 0.0]);
                    state.selection = vec![SelectionItem::Point(state.document().points()[0].id)];
                }
                1 => {
                    state.set_tool(Tool::Draw(DrawTool::Point));
                    state.draw_click([0.0, 0.0]);
                    state.draw_click([0.0, 0.0]);
                    state.selection = state
                        .document()
                        .points()
                        .iter()
                        .map(|point| SelectionItem::Point(point.id))
                        .collect();
                }
                2 | 3 => {
                    let end = if kind == 2 { [2.0, 0.0] } else { [0.0, 2.0] };
                    draw(&mut state, DrawTool::Line, &[[0.0, 0.0], end]);
                    select_all_curves(&mut state);
                }
                4..=6 => {
                    draw(&mut state, DrawTool::Line, &[[0.0, 0.0], [2.0, 0.0]]);
                    let second = if kind == 5 {
                        [[0.0, 0.0], [0.0, 2.0]]
                    } else {
                        [[0.0, 1.0], [2.0, 1.0]]
                    };
                    draw(&mut state, DrawTool::Line, &second);
                    select_all_curves(&mut state);
                }
                7 => {
                    draw(&mut state, DrawTool::Circle, &[[0.0, 0.0], [1.0, 0.0]]);
                    draw(&mut state, DrawTool::Circle, &[[3.0, 0.0], [4.0, 0.0]]);
                    select_all_curves(&mut state);
                }
                8 | 10 => {
                    draw(&mut state, DrawTool::Line, &[[-1.0, 0.0], [1.0, 0.0]]);
                    state.set_tool(Tool::Draw(DrawTool::Point));
                    state.draw_click([0.0, 0.0]);
                    select_point_and_curves(&mut state, 1);
                }
                9 => {
                    draw(&mut state, DrawTool::Line, &[[-2.0, 0.0], [2.0, 0.0]]);
                    state.set_tool(Tool::Draw(DrawTool::Point));
                    state.draw_click([0.0, 1.0]);
                    state.draw_click([0.0, -1.0]);
                    select_point_and_curves(&mut state, 2);
                }
                11 | 12 => {
                    draw(&mut state, DrawTool::Line, &[[-1.0, 0.0], [1.0, 0.0]]);
                    draw(&mut state, DrawTool::Line, &[[-1.0, 0.0], [1.0, 0.0]]);
                    select_all_curves(&mut state);
                }
                _ => unreachable!(),
            }
            state.apply_constraint(kind);
            assert_eq!(
                state.document().constraints().len(),
                1,
                "kind={kind}: {}",
                state.last_attempt
            );
            assert!(state.accepted_is_valid(), "kind={kind}");
        }
    }

    #[test]
    fn every_dimension_kind_supports_reference_display_and_driving_edit() {
        for kind in 0..=4 {
            let mut state = PlaygroundState::empty().unwrap();
            let target = match kind {
                0 => {
                    state.set_tool(Tool::Draw(DrawTool::Point));
                    state.draw_click([0.0, 0.0]);
                    state.draw_click([2.0, 0.0]);
                    state.selection = state
                        .document()
                        .points()
                        .iter()
                        .map(|point| SelectionItem::Point(point.id))
                        .collect();
                    2.0
                }
                1 => {
                    draw(&mut state, DrawTool::Line, &[[0.0, 0.0], [2.0, 0.0]]);
                    select_all_curves(&mut state);
                    2.0
                }
                2 => {
                    draw(&mut state, DrawTool::Circle, &[[0.0, 0.0], [1.0, 0.0]]);
                    select_all_curves(&mut state);
                    1.0
                }
                3 => {
                    draw(&mut state, DrawTool::Circle, &[[0.0, 0.0], [1.0, 0.0]]);
                    select_all_curves(&mut state);
                    2.0
                }
                4 => {
                    draw(&mut state, DrawTool::Line, &[[0.0, 0.0], [2.0, 0.0]]);
                    draw(&mut state, DrawTool::Line, &[[0.0, 0.0], [0.0, 2.0]]);
                    select_all_curves(&mut state);
                    std::f64::consts::PI * 0.5
                }
                _ => unreachable!(),
            };
            state.apply_dimension(kind, DocumentDimensionMode::Reference, target);
            assert_eq!(
                state.document().dimensions().len(),
                1,
                "kind={kind}: {}",
                state.last_attempt
            );
            assert!(state.object_list_markup().contains("ref "), "kind={kind}");
            state.apply_dimension(kind, DocumentDimensionMode::Driving, target);
            assert_eq!(
                state.document().dimensions()[0].mode,
                DocumentDimensionMode::Driving,
                "kind={kind}: {}",
                state.last_attempt
            );
            assert!(state.accepted_is_valid(), "kind={kind}");
        }
    }

    #[test]
    fn box_selection_and_pan_gestures_are_web_only_and_deterministic() {
        let mut state = PlaygroundState::empty().unwrap();
        state.set_tool(Tool::Draw(DrawTool::Point));
        state.draw_click([-1.0, 0.0]);
        state.draw_click([1.0, 0.0]);
        let first = state.viewport().model_to_svg([-1.2, -0.2]);
        let second = state.viewport().model_to_svg([1.2, 0.2]);
        state.begin_box_select(11, first, false);
        state.update_gesture(11, second);
        state.end_gesture(11, true);
        assert_eq!(state.selected_points().len(), 2);

        state.set_tool(Tool::Pan);
        let before = state.viewport().center;
        state.begin_pan(12, [500.0, 350.0]);
        state.update_gesture(12, [570.0, 420.0]);
        state.end_gesture(12, true);
        assert!(state.viewport().center[0] < before[0]);
        assert!(state.viewport().center[1] > before[1]);
        assert_eq!(state.session().history_len(), 2);
    }

    fn select_all_curves(state: &mut PlaygroundState) {
        state.selection = state
            .document()
            .curves()
            .iter()
            .map(|curve| SelectionItem::Curve {
                span: CurveSpan::line(curve.id),
                parameter: if matches!(curve.definition, CurveDefinition::Circle { .. }) {
                    0.0
                } else {
                    0.5
                },
            })
            .collect();
    }

    fn select_point_and_curves(state: &mut PlaygroundState, point_count: usize) {
        let mut selection: Vec<_> = state
            .document()
            .points()
            .iter()
            .rev()
            .take(point_count)
            .map(|point| SelectionItem::Point(point.id))
            .collect();
        selection.extend(
            state
                .document()
                .curves()
                .iter()
                .map(|curve| SelectionItem::Curve {
                    span: CurveSpan::line(curve.id),
                    parameter: 0.5,
                }),
        );
        state.selection = selection;
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        str::FromStr,
    };

    use geosolve_sketch::{
        AlphaScenarioKind, ContactId, CurveDefinition, CurveSpan, DesignPointId,
        DocumentConstraintId, DocumentDimensionId, DocumentDimensionMode, DocumentObjectId,
        MAX_DOCUMENT_JSON_BYTES, PersistentId,
    };
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use web_sys::{
        Blob, Document, Element, Event, FileReader, HtmlAnchorElement, HtmlInputElement,
        HtmlSelectElement, HtmlTextAreaElement, KeyboardEvent, MouseEvent, PointerEvent, Url,
        WheelEvent,
    };

    use super::{
        CANVAS_HEIGHT, CANVAS_WIDTH, DrawTool, HIT_RADIUS_PX, PlaygroundState, SelectionItem, Tool,
    };

    const STORAGE_KEY: &str = "geosolve.sketch-playground.accepted.v1";
    const STORAGE_BACKUP_KEY: &str = "geosolve.sketch-playground.accepted.backup.v1";

    #[derive(Clone, Copy)]
    struct PendingPointerMove {
        pointer_id: i32,
        svg: [f64; 2],
    }

    #[derive(Default)]
    struct PointerMoveQueue {
        pending: Cell<Option<PendingPointerMove>>,
        scheduled: Cell<bool>,
    }

    pub(crate) fn install(document: &Document) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
        let storage = window.local_storage().ok().flatten();
        let stored = storage
            .as_ref()
            .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten());
        let backup = storage
            .as_ref()
            .and_then(|storage| storage.get_item(STORAGE_BACKUP_KEY).ok().flatten());
        let state = match stored.as_deref() {
            Some(json) => match PlaygroundState::from_json(json) {
                Ok(state) => state,
                Err(error) => match backup
                    .as_deref()
                    .and_then(|json| PlaygroundState::from_json(json).ok())
                {
                    Some(mut state) => {
                        state.set_startup_notice(format!(
                            "Stored document was invalid and was not overwritten; recovered the last valid backup: {error}"
                        ));
                        state
                    }
                    None => {
                        let mut state =
                            PlaygroundState::empty().map_err(|error| JsValue::from_str(&error))?;
                        state.set_startup_notice(format!(
                            "Stored document was invalid, no valid backup was available, and the stored input was not overwritten: {error}"
                        ));
                        state
                    }
                },
            },
            None => PlaygroundState::empty().map_err(|error| JsValue::from_str(&error))?,
        };
        let initial_json = state
            .export_json()
            .map_err(|error| JsValue::from_str(&error))?;
        required(document, "document-json")?
            .dyn_into::<HtmlTextAreaElement>()?
            .set_value(&initial_json);
        let app = Rc::new(RefCell::new(state));
        render_shared(document, &app);
        install_click_listener(document, &app)?;
        install_pointer_listeners(document, &app)?;
        install_wheel_listener(document, &app)?;
        install_keyboard_listener(document, &app)?;
        install_file_listener(document, &app)?;
        required(document, "playground-root")?.set_attribute("data-e2e-ready", "true")?;
        Ok(())
    }

    fn required(document: &Document, id: &str) -> Result<Element, JsValue> {
        document
            .get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str(&format!("missing #{id} element")))
    }

    fn set_disabled(element: &Element, disabled: bool) -> Result<(), JsValue> {
        if disabled {
            element.set_attribute("disabled", "")
        } else {
            element.remove_attribute("disabled")
        }
    }

    fn render_shared(document: &Document, app: &Rc<RefCell<PlaygroundState>>) {
        let result = render(document, &mut app.borrow_mut());
        if let Err(error) = result
            && let Some(status) = document.get_element_by_id("last-attempt")
        {
            status.set_text_content(Some(&format!("Rendering error: {error:?}")));
        }
    }

    fn render(document: &Document, state: &mut PlaygroundState) -> Result<(), JsValue> {
        let root = required(document, "playground-root")?;
        let sequence = root
            .get_attribute("data-render-sequence")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0)
            .saturating_add(1);
        root.set_attribute("data-render-sequence", &sequence.to_string())?;
        root.set_attribute(
            "data-revision",
            &state.display_session().revision().to_string(),
        )?;
        root.set_attribute(
            "data-authoritative-revision",
            &state.session().revision().to_string(),
        )?;
        root.set_attribute(
            "data-preview-active",
            if state.preview_active() {
                "true"
            } else {
                "false"
            },
        )?;
        root.set_attribute(
            "data-history-length",
            &state.session().history_len().to_string(),
        )?;
        root.set_attribute(
            "data-history-cursor",
            &state.session().history_cursor().to_string(),
        )?;
        root.set_attribute(
            "data-viewport-center-x",
            &state.viewport().center[0].to_string(),
        )?;
        root.set_attribute(
            "data-viewport-center-y",
            &state.viewport().center[1].to_string(),
        )?;
        root.set_attribute(
            "data-pixels-per-unit",
            &state.viewport().pixels_per_unit.to_string(),
        )?;
        let accepted = state.display_session().accepted_result();
        let report = &accepted.accepted_view().core_report;
        root.set_attribute("data-hard-validity", &format!("{:?}", report.hard_validity))?;
        root.set_attribute(
            "data-hard-residual-max",
            &report.hard_residual_max.to_string(),
        )?;
        if report.rank_is_valid {
            root.set_attribute("data-rank", &report.rank.to_string())?;
            root.set_attribute("data-left-nullity", &report.left_nullity.to_string())?;
            root.set_attribute("data-equality-dof", &report.right_nullity.to_string())?;
            root.set_attribute(
                "data-bounded-dof",
                &report.bidirectional_degrees_of_freedom.to_string(),
            )?;
        } else {
            root.remove_attribute("data-rank")?;
            root.remove_attribute("data-left-nullity")?;
            root.remove_attribute("data-equality-dof")?;
            root.remove_attribute("data-bounded-dof")?;
        }
        root.set_attribute(
            "data-structural-classification",
            &format!("{:?}", report.structural.structural_classification),
        )?;
        root.set_attribute(
            "data-structural-rank",
            &report.structural.structural_rank.to_string(),
        )?;
        root.set_attribute(
            "data-structural-left-nullity",
            &report.structural.structural_left_nullity.to_string(),
        )?;
        root.set_attribute(
            "data-structural-right-nullity",
            &report.structural.structural_right_nullity.to_string(),
        )?;
        root.set_attribute(
            "data-hard-components",
            &report.structural.components.to_string(),
        )?;
        root.set_attribute(
            "data-one-sided-motion",
            &format!("{:?}", report.one_sided_mobility),
        )?;
        root.set_attribute(
            "data-requested-backend",
            &format!("{:?}", report.requested_backend),
        )?;
        root.set_attribute(
            "data-actual-backend",
            &format!("{:?}", report.actual_backend),
        )?;
        if let Some(reason) = report.sparse_fallback_reason {
            root.set_attribute("data-sparse-fallback", &format!("{reason:?}"))?;
        } else {
            root.remove_attribute("data-sparse-fallback")?;
        }
        let viewport = required(document, "sketch-viewport")?;
        viewport.set_inner_html(&state.render_svg());
        viewport.set_attribute("data-tool", state.tool().key())?;
        if state.gesture_pointer().is_some() {
            viewport.set_attribute("data-active", "true")?;
        } else {
            viewport.remove_attribute("data-active")?;
        }
        required(document, "tool-status")?.set_text_content(Some(state.tool().label()));
        required(document, "draft-status")?.set_text_content(Some(&state.draft_status()));
        required(document, "document-status")?.set_text_content(Some(&state.document_status()));
        required(document, "interaction-help")?.set_text_content(Some(&state.interaction_help()));
        required(document, "selection-summary")?.set_text_content(Some(&state.selection_summary()));
        required(document, "playground-solve-status")?
            .set_inner_html(&PlaygroundState::solve_status_markup_with_result(&accepted));
        let object_list = required(document, "object-list")?;
        let audit = required(document, "playground-audit")?;
        if state.preview_active() {
            object_list.set_attribute("aria-busy", "true")?;
            if root.get_attribute("data-detail-refresh").as_deref() != Some("deferred") {
                root.set_attribute("data-detail-refresh", "deferred")?;
                audit.set_inner_html(
                    "<p class=\"selection-summary\">Detailed audit refreshes when the drag is released.</p>",
                );
            }
        } else {
            root.remove_attribute("data-detail-refresh")?;
            object_list.remove_attribute("aria-busy")?;
            object_list.set_inner_html(&state.object_list_markup_with_result(&accepted));
            audit.set_inner_html(&PlaygroundState::audit_markup_with_result(&accepted));
        }
        required(document, "last-attempt")?.set_inner_html(&state.last_attempt_markup());
        required(document, "solve-view-label")?.set_text_content(Some(if state.preview_active() {
            "Accepted drag preview (not saved)"
        } else {
            "Accepted solve"
        }));
        let badge = required(document, "solve-badge")?;
        let accepted_is_valid = PlaygroundState::result_is_valid(&accepted);
        badge.set_text_content(Some(if state.preview_active() && accepted_is_valid {
            "accepted preview"
        } else if accepted_is_valid {
            "accepted"
        } else {
            "not valid"
        }));
        badge.set_class_name(if accepted_is_valid {
            "live-badge"
        } else {
            "live-badge expected-conflict"
        });
        let inference = required(document, "inference-panel")?;
        if let Some(label) = state.inference_label() {
            inference.remove_attribute("hidden")?;
            required(document, "inference-summary")?.set_text_content(Some(label));
        } else {
            inference.set_attribute("hidden", "")?;
        }
        set_disabled(&required(document, "undo")?, !state.session().can_undo())?;
        set_disabled(&required(document, "redo")?, !state.session().can_redo())?;
        set_disabled(&required(document, "undo-draft")?, state.draft.is_empty())?;
        set_disabled(&required(document, "cancel-draft")?, state.draft.is_empty())?;
        set_disabled(
            &required(document, "finish-draft")?,
            state.tool() != Tool::Draw(DrawTool::Polyline) || state.draft.len() < 2,
        )?;
        for key in [
            "select",
            "pan",
            "point",
            "line",
            "polyline",
            "rectangle",
            "circle",
            "arc",
            "quadratic",
            "cubic",
        ] {
            if let Some(button) = document.query_selector(&format!("[data-tool=\"{key}\"]"))? {
                let active = key == state.tool().key();
                button.set_class_name(if active { "active" } else { "" });
                button.set_attribute("aria-pressed", if active { "true" } else { "false" })?;
            }
        }
        required(document, "playground-announcement")?.set_text_content(Some(&state.last_attempt));
        if let Some(json) = state.storage_json() {
            let status = required(document, "storage-status")?;
            match web_sys::window().and_then(|window| window.local_storage().ok().flatten()) {
                Some(storage) => match storage.set_item(STORAGE_KEY, &json) {
                    Ok(()) => {
                        if storage.set_item(STORAGE_BACKUP_KEY, &json).is_ok() {
                            state.mark_storage_saved();
                            status.set_text_content(Some(
                                "Accepted revision and recovery backup saved locally.",
                            ));
                        } else {
                            status.set_text_content(Some(
                                "Accepted revision saved, but browser storage rejected the recovery backup; the backup will retry.",
                            ));
                        }
                    }
                    Err(_) => status.set_text_content(Some(
                        "Accepted revision is valid, but browser storage rejected the save.",
                    )),
                },
                None => status.set_text_content(Some("Browser local storage is unavailable.")),
            }
        }
        Ok(())
    }

    fn install_click_listener(
        document: &Document,
        app: &Rc<RefCell<PlaygroundState>>,
    ) -> Result<(), JsValue> {
        let root = required(document, "playground-root")?;
        let callback_document = document.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            let Some(target) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            let Ok(control) = target.closest("[data-tool], [data-action]") else {
                return;
            };
            let Some(control) = control else {
                return;
            };
            if let Some(tool) = control
                .get_attribute("data-tool")
                .and_then(|value| tool(&value))
            {
                callback_app.borrow_mut().set_tool(tool);
                render_shared(&callback_document, &callback_app);
                return;
            }
            let Some(action) = control.get_attribute("data-action") else {
                return;
            };
            event.prevent_default();
            match action.as_str() {
                "undo" => callback_app.borrow_mut().undo(),
                "redo" => callback_app.borrow_mut().redo(),
                "new" => {
                    if let Ok(state) = PlaygroundState::empty() {
                        *callback_app.borrow_mut() = state;
                    }
                }
                "load-example" => {
                    let selected = select_value(&callback_document, "alpha-example");
                    let kind = selected.as_deref().and_then(|value| match value {
                        "a1" => Some(AlphaScenarioKind::A1),
                        "a2" => Some(AlphaScenarioKind::A2),
                        "a3" => Some(AlphaScenarioKind::A3),
                        "a4" => Some(AlphaScenarioKind::A4),
                        "a5" => Some(AlphaScenarioKind::A5),
                        "a8" => Some(AlphaScenarioKind::A8),
                        "corpus" => Some(AlphaScenarioKind::Corpus),
                        "stress-compass" => Some(AlphaScenarioKind::StressCompass),
                        "stress-bridge" => Some(AlphaScenarioKind::StressBridge),
                        "motion-cam" => Some(AlphaScenarioKind::MotionCam),
                        "motion-orbit" => Some(AlphaScenarioKind::MotionOrbit),
                        "motion-trammel" => Some(AlphaScenarioKind::MotionTrammel),
                        "motion-scotch-yoke" => Some(AlphaScenarioKind::MotionScotchYoke),
                        "motion-rotating-square" => Some(AlphaScenarioKind::MotionRotatingSquare),
                        "motion-scissor" => Some(AlphaScenarioKind::MotionScissor),
                        "motion-scissor-tower" => Some(AlphaScenarioKind::MotionScissorTower),
                        "motion-peaucellier" => Some(AlphaScenarioKind::MotionPeaucellier),
                        "diagnostic-rank-drop" => Some(AlphaScenarioKind::DiagnosticRankDrop),
                        "diagnostic-endpoint-bound" => {
                            Some(AlphaScenarioKind::DiagnosticEndpointBound)
                        }
                        "diagnostic-redundancy" => Some(AlphaScenarioKind::DiagnosticRedundancy),
                        _ => None,
                    });
                    let scale = select_value(&callback_document, "alpha-scale")
                        .and_then(|value| value.parse::<f64>().ok());
                    let example = if selected.as_deref() == Some("medium") {
                        PlaygroundState::medium_performance_example().ok()
                    } else {
                        kind.zip(scale)
                            .and_then(|(kind, scale)| PlaygroundState::example(kind, scale).ok())
                    };
                    match example {
                        Some(state) => *callback_app.borrow_mut() = state,
                        None => callback_app
                            .borrow_mut()
                            .rejected_change("Canonical example could not be loaded."),
                    }
                }
                "zoom-in" => callback_app
                    .borrow_mut()
                    .zoom([CANVAS_WIDTH * 0.5, CANVAS_HEIGHT * 0.5], 1.25),
                "zoom-out" => callback_app
                    .borrow_mut()
                    .zoom([CANVAS_WIDTH * 0.5, CANVAS_HEIGHT * 0.5], 0.8),
                "zoom-fit" => callback_app.borrow_mut().fit_view(),
                "finish-draft" => callback_app.borrow_mut().finish_draft(),
                "undo-draft" => callback_app.borrow_mut().undo_draft_point(),
                "cancel-draft" => callback_app.borrow_mut().cancel_draft(),
                "clear-selection" => callback_app.borrow_mut().clear_selection(),
                "delete" => callback_app.borrow_mut().delete_selection(),
                "toggle-suppressed" => callback_app.borrow_mut().toggle_selected_sources(),
                "apply-branch-state" => {
                    let mut state = callback_app.borrow_mut();
                    update_branch_options(&callback_document, &mut state);
                    state.apply_branch_state_values(
                        optional_input_number(&callback_document, "contact-parameter"),
                        optional_input_number(&callback_document, "second-contact-parameter"),
                    );
                }
                "confirm-inference" => callback_app.borrow_mut().confirm_inference(),
                "cancel-inference" => callback_app.borrow_mut().cancel_inference(),
                "apply-constraint" => {
                    let kind = select_index(&callback_document, "constraint-kind").unwrap_or(0);
                    let mut state = callback_app.borrow_mut();
                    update_branch_options(&callback_document, &mut state);
                    state.apply_constraint(kind);
                }
                "apply-dimension" => {
                    let kind = select_index(&callback_document, "dimension-kind").unwrap_or(0);
                    let mode = if select_index(&callback_document, "dimension-mode") == Some(1) {
                        DocumentDimensionMode::Reference
                    } else {
                        DocumentDimensionMode::Driving
                    };
                    let value = required(&callback_document, "dimension-value")
                        .ok()
                        .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
                        .map_or(f64::NAN, |input| input.value_as_number());
                    let mut state = callback_app.borrow_mut();
                    update_branch_options(&callback_document, &mut state);
                    let label = input_value(&callback_document, "dimension-label")
                        .unwrap_or_else(|| "dimension".into());
                    state.apply_dimension_labeled(kind, mode, value, &label);
                }
                "export-json" => {
                    if let Ok(json) = callback_app.borrow().export_json()
                        && let Some(textarea) = required(&callback_document, "document-json")
                            .ok()
                            .and_then(|element| element.dyn_into::<HtmlTextAreaElement>().ok())
                    {
                        textarea.set_value(&json);
                        textarea.select();
                    }
                }
                "import-json" => {
                    if let Some(textarea) = required(&callback_document, "document-json")
                        .ok()
                        .and_then(|element| element.dyn_into::<HtmlTextAreaElement>().ok())
                    {
                        callback_app.borrow_mut().import_json(&textarea.value());
                    }
                }
                "download-json" => match callback_app.borrow().export_json() {
                    Ok(json) => {
                        if download_json(&callback_document, &json).is_err() {
                            callback_app
                                .borrow_mut()
                                .rejected_change("Browser rejected the JSON download.");
                        }
                    }
                    Err(error) => callback_app
                        .borrow_mut()
                        .rejected_change(format!("JSON download failed: {error}")),
                },
                "select-object" => select_object(&control, event.shift_key(), &callback_app),
                "delete-object" => delete_object(&control, &callback_app),
                _ => {}
            }
            render_shared(&callback_document, &callback_app);
        });
        root.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_pointer_listeners(
        document: &Document,
        app: &Rc<RefCell<PlaygroundState>>,
    ) -> Result<(), JsValue> {
        let viewport = required(document, "sketch-viewport")?;
        let move_queue = Rc::new(PointerMoveQueue::default());
        install_pointer_down(document, &viewport, app)?;
        install_pointer_move(document, &viewport, app, &move_queue)?;
        install_pointer_end(document, &viewport, app, &move_queue, "pointerup", true)?;
        install_pointer_end(
            document,
            &viewport,
            app,
            &move_queue,
            "pointercancel",
            false,
        )?;
        Ok(())
    }

    fn install_pointer_down(
        document: &Document,
        viewport: &Element,
        app: &Rc<RefCell<PlaygroundState>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_viewport = viewport.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            if !event.is_primary() || event.button() != 0 || callback_app.borrow().gesture.is_some()
            {
                return;
            }
            let Some(svg) = pointer_svg(&event, &callback_viewport) else {
                return;
            };
            event.prevent_default();
            let mut state = callback_app.borrow_mut();
            update_branch_options(&callback_document, &mut state);
            match state.tool() {
                Tool::Draw(_) => {
                    let captured = callback_viewport
                        .set_pointer_capture(event.pointer_id())
                        .is_ok();
                    if captured || event.pointer_type() != "mouse" {
                        state.begin_draft_placement(event.pointer_id(), svg);
                    }
                }
                Tool::Pan => {
                    if callback_viewport
                        .set_pointer_capture(event.pointer_id())
                        .is_ok()
                    {
                        state.begin_pan(event.pointer_id(), svg);
                    }
                }
                Tool::Select => {
                    let hit_radius = if event.pointer_type() == "mouse" {
                        HIT_RADIUS_PX
                    } else {
                        64.0
                    };
                    let hit = state.hit_test(svg, hit_radius);
                    match hit {
                        Some(SelectionItem::Point(point)) => {
                            state.set_object_selection(
                                SelectionItem::Point(point),
                                event.shift_key(),
                            );
                            if !event.shift_key()
                                && callback_viewport
                                    .set_pointer_capture(event.pointer_id())
                                    .is_ok()
                            {
                                state.begin_point_drag(event.pointer_id(), point, svg);
                            }
                        }
                        Some(item) => state.set_object_selection(item, event.shift_key()),
                        None => {
                            if callback_viewport
                                .set_pointer_capture(event.pointer_id())
                                .is_ok()
                            {
                                state.begin_box_select(event.pointer_id(), svg, event.shift_key());
                            }
                        }
                    }
                }
            }
            drop(state);
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
        app: &Rc<RefCell<PlaygroundState>>,
        move_queue: &Rc<PointerMoveQueue>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_viewport = viewport.clone();
        let callback_app = Rc::clone(app);
        let callback_queue = Rc::clone(move_queue);
        let frame_document = document.clone();
        let frame_app = Rc::clone(app);
        let frame_queue = Rc::clone(move_queue);
        let frame_callback: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> =
            Rc::new(RefCell::new(None));
        *frame_callback.borrow_mut() = Some(Closure::new(move |_timestamp: f64| {
            frame_queue.scheduled.set(false);
            if apply_pending_pointer_move(&frame_queue, &frame_app) {
                render_shared(&frame_document, &frame_app);
            }
        }));
        let callback_frame = Rc::clone(&frame_callback);
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            let Some(svg) = pointer_svg(&event, &callback_viewport) else {
                return;
            };
            let state = callback_app.borrow();
            let active_gesture = state.gesture_pointer() == Some(event.pointer_id());
            let should_queue = active_gesture || matches!(state.tool(), Tool::Draw(_));
            drop(state);
            if !should_queue {
                return;
            }
            if active_gesture {
                event.prevent_default();
            }
            callback_queue.pending.set(Some(PendingPointerMove {
                pointer_id: event.pointer_id(),
                svg,
            }));
            if callback_queue.scheduled.replace(true) {
                return;
            }
            let scheduled = web_sys::window().is_some_and(|window| {
                callback_frame.borrow().as_ref().is_some_and(|callback| {
                    window
                        .request_animation_frame(callback.as_ref().unchecked_ref())
                        .is_ok()
                })
            });
            if !scheduled {
                callback_queue.scheduled.set(false);
            }
            if !scheduled && apply_pending_pointer_move(&callback_queue, &callback_app) {
                render_shared(&callback_document, &callback_app);
            }
        });
        viewport
            .add_event_listener_with_callback("pointermove", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_pointer_end(
        document: &Document,
        viewport: &Element,
        app: &Rc<RefCell<PlaygroundState>>,
        move_queue: &Rc<PointerMoveQueue>,
        event_name: &str,
        commit: bool,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_viewport = viewport.clone();
        let callback_app = Rc::clone(app);
        let callback_queue = Rc::clone(move_queue);
        let callback = Closure::<dyn FnMut(PointerEvent)>::new(move |event: PointerEvent| {
            if callback_app.borrow().gesture_pointer() != Some(event.pointer_id()) {
                return;
            }
            event.prevent_default();
            if commit {
                apply_pending_pointer_move(&callback_queue, &callback_app);
            } else {
                callback_queue.pending.set(None);
            }
            if commit && let Some(svg) = pointer_svg(&event, &callback_viewport) {
                callback_app
                    .borrow_mut()
                    .update_gesture(event.pointer_id(), svg);
            }
            let _ = callback_viewport.release_pointer_capture(event.pointer_id());
            callback_app
                .borrow_mut()
                .end_gesture(event.pointer_id(), commit);
            render_shared(&callback_document, &callback_app);
        });
        viewport.add_event_listener_with_callback(event_name, callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn apply_pending_pointer_move(
        move_queue: &PointerMoveQueue,
        app: &Rc<RefCell<PlaygroundState>>,
    ) -> bool {
        let Some(pending) = move_queue.pending.take() else {
            return false;
        };
        let mut state = app.borrow_mut();
        if state.gesture_pointer() == Some(pending.pointer_id) {
            state.update_gesture(pending.pointer_id, pending.svg)
        } else if matches!(state.tool(), Tool::Draw(_)) {
            let model = state.viewport().svg_to_model(pending.svg);
            if state.draft_cursor == Some(model) {
                false
            } else {
                state.set_draft_cursor(model);
                true
            }
        } else {
            false
        }
    }

    fn install_wheel_listener(
        document: &Document,
        app: &Rc<RefCell<PlaygroundState>>,
    ) -> Result<(), JsValue> {
        let viewport = required(document, "sketch-viewport")?;
        let callback_document = document.clone();
        let callback_viewport = viewport.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
            let bounds = callback_viewport.get_bounding_client_rect();
            if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
                return;
            }
            event.prevent_default();
            let svg = [
                (f64::from(event.client_x()) - bounds.left()) * CANVAS_WIDTH / bounds.width(),
                (f64::from(event.client_y()) - bounds.top()) * CANVAS_HEIGHT / bounds.height(),
            ];
            callback_app
                .borrow_mut()
                .zoom(svg, (-event.delta_y() * 0.001).exp());
            render_shared(&callback_document, &callback_app);
        });
        viewport.add_event_listener_with_callback("wheel", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_keyboard_listener(
        document: &Document,
        app: &Rc<RefCell<PlaygroundState>>,
    ) -> Result<(), JsValue> {
        let callback_document = document.clone();
        let callback_app = Rc::clone(app);
        let callback = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            let key = event.key().to_ascii_lowercase();
            let editing_control = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .is_some_and(|target| {
                    matches!(target.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
                });
            if editing_control && key != "escape" {
                return;
            }
            let handled = if (event.ctrl_key() || event.meta_key()) && key == "z" {
                if !event.shift_key() && !callback_app.borrow().draft.is_empty() {
                    callback_app.borrow_mut().undo_draft_point();
                } else if event.shift_key() {
                    callback_app.borrow_mut().redo();
                } else {
                    callback_app.borrow_mut().undo();
                }
                true
            } else if (event.ctrl_key() || event.meta_key()) && key == "y" {
                callback_app.borrow_mut().redo();
                true
            } else if key == "escape" {
                let mut state = callback_app.borrow_mut();
                state.cancel_draft();
                state.drag_preview = None;
                state.gesture = None;
                state.cancel_inference();
                true
            } else if key == "backspace" && !callback_app.borrow().draft.is_empty() {
                callback_app.borrow_mut().undo_draft_point();
                true
            } else if key == "enter"
                && callback_app.borrow().tool() == Tool::Draw(DrawTool::Polyline)
            {
                callback_app.borrow_mut().finish_draft();
                true
            } else {
                false
            };
            if handled {
                event.prevent_default();
                render_shared(&callback_document, &callback_app);
            }
        });
        document.add_event_listener_with_callback("keydown", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn install_file_listener(
        document: &Document,
        app: &Rc<RefCell<PlaygroundState>>,
    ) -> Result<(), JsValue> {
        let input = required(document, "document-file")?.dyn_into::<HtmlInputElement>()?;
        let callback_document = document.clone();
        let callback_app = Rc::clone(app);
        let callback_input = input.clone();
        let upload_generation = Rc::new(Cell::new(0_u64));
        let callback = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
            let generation = upload_generation.get().saturating_add(1);
            upload_generation.set(generation);
            let Some(file) = callback_input.files().and_then(|files| files.get(0)) else {
                return;
            };
            if file.size() > MAX_DOCUMENT_JSON_BYTES as f64 {
                callback_app.borrow_mut().rejected_change(format!(
                    "Uploaded JSON exceeds the {} byte document limit.",
                    MAX_DOCUMENT_JSON_BYTES
                ));
                callback_input.set_value("");
                render_shared(&callback_document, &callback_app);
                return;
            }
            let Ok(reader) = FileReader::new() else {
                callback_app
                    .borrow_mut()
                    .rejected_change("Browser file reader is unavailable.");
                render_shared(&callback_document, &callback_app);
                return;
            };
            let load_document = callback_document.clone();
            let load_app = Rc::clone(&callback_app);
            let load_reader = reader.clone();
            let load_input = callback_input.clone();
            let load_generation = Rc::clone(&upload_generation);
            let load = Closure::once_into_js(move |_event: Event| {
                load_reader.set_onerror(None);
                if load_generation.get() != generation {
                    return;
                }
                match load_reader
                    .result()
                    .ok()
                    .and_then(|value| value.as_string())
                {
                    Some(json) => {
                        if let Some(textarea) = required(&load_document, "document-json")
                            .ok()
                            .and_then(|element| element.dyn_into::<HtmlTextAreaElement>().ok())
                        {
                            textarea.set_value(&json);
                        }
                        load_app.borrow_mut().import_json(&json);
                    }
                    None => load_app
                        .borrow_mut()
                        .rejected_change("Uploaded file could not be read as text."),
                }
                load_input.set_value("");
                render_shared(&load_document, &load_app);
            });
            let error_reader = reader.clone();
            let error_document = callback_document.clone();
            let error_app = Rc::clone(&callback_app);
            let error_input = callback_input.clone();
            let error_generation = Rc::clone(&upload_generation);
            let error = Closure::once_into_js(move |_event: Event| {
                error_reader.set_onload(None);
                error_reader.set_onerror(None);
                if error_generation.get() == generation {
                    error_app
                        .borrow_mut()
                        .rejected_change("Uploaded file could not be read as text.");
                    error_input.set_value("");
                    render_shared(&error_document, &error_app);
                }
            });
            reader.set_onload(Some(load.unchecked_ref()));
            reader.set_onerror(Some(error.unchecked_ref()));
            if reader.read_as_text(&file).is_err() {
                reader.set_onload(None);
                reader.set_onerror(None);
                callback_app
                    .borrow_mut()
                    .rejected_change("Uploaded file could not be read as text.");
                render_shared(&callback_document, &callback_app);
            }
        });
        input.add_event_listener_with_callback("change", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }

    fn download_json(document: &Document, json: &str) -> Result<(), JsValue> {
        let parts = js_sys::Array::new();
        parts.push(&JsValue::from_str(json));
        let blob = Blob::new_with_str_sequence(&parts)?;
        let url = Url::create_object_url_with_blob(&blob)?;
        let anchor = document
            .create_element("a")?
            .dyn_into::<HtmlAnchorElement>()?;
        anchor.set_href(&url);
        anchor.set_download("geosolve-sketch.json");
        anchor.click();
        Url::revoke_object_url(&url)
    }

    fn pointer_svg(event: &PointerEvent, viewport: &Element) -> Option<[f64; 2]> {
        let bounds = viewport.get_bounding_client_rect();
        (bounds.width() > 0.0 && bounds.height() > 0.0).then_some([
            (f64::from(event.client_x()) - bounds.left()) * CANVAS_WIDTH / bounds.width(),
            (f64::from(event.client_y()) - bounds.top()) * CANVAS_HEIGHT / bounds.height(),
        ])
    }

    fn select_index(document: &Document, id: &str) -> Option<usize> {
        required(document, id)
            .ok()?
            .dyn_into::<HtmlSelectElement>()
            .ok()?
            .selected_index()
            .try_into()
            .ok()
    }

    fn select_value(document: &Document, id: &str) -> Option<String> {
        required(document, id)
            .ok()?
            .dyn_into::<HtmlSelectElement>()
            .ok()
            .map(|select| select.value())
    }

    fn optional_input_number(document: &Document, id: &str) -> Option<f64> {
        let input = required(document, id)
            .ok()?
            .dyn_into::<HtmlInputElement>()
            .ok()?;
        (!input.value().trim().is_empty())
            .then(|| input.value_as_number())
            .filter(|value| value.is_finite())
    }

    fn input_value(document: &Document, id: &str) -> Option<String> {
        let value = required(document, id)
            .ok()?
            .dyn_into::<HtmlInputElement>()
            .ok()?
            .value();
        (!value.trim().is_empty()).then_some(value)
    }

    fn update_branch_options(document: &Document, state: &mut PlaygroundState) {
        let arc_sweep = if select_index(document, "arc-sweep") == Some(1) {
            geosolve_sketch::DocumentArcSweep::Clockwise
        } else {
            geosolve_sketch::DocumentArcSweep::CounterClockwise
        };
        let neighborhood = match select_index(document, "contact-neighborhood") {
            Some(1) => super::NeighborhoodChoice::Interior,
            Some(2) => super::NeighborhoodChoice::Start,
            Some(3) => super::NeighborhoodChoice::End,
            _ => super::NeighborhoodChoice::Picked,
        };
        let tangent_orientation = if select_index(document, "tangent-orientation") == Some(1) {
            geosolve_sketch::TangentOrientation::Opposed
        } else {
            geosolve_sketch::TangentOrientation::Aligned
        };
        let second_neighborhood = match select_index(document, "second-contact-neighborhood") {
            Some(1) => super::NeighborhoodChoice::Interior,
            Some(2) => super::NeighborhoodChoice::Start,
            Some(3) => super::NeighborhoodChoice::End,
            _ => super::NeighborhoodChoice::Picked,
        };
        let second_tangent_orientation =
            if select_index(document, "second-tangent-orientation") == Some(1) {
                geosolve_sketch::TangentOrientation::Opposed
            } else {
                geosolve_sketch::TangentOrientation::Aligned
            };
        let angle_orientation = if select_index(document, "angle-orientation") == Some(1) {
            geosolve_sketch::DocumentAngleOrientation::Clockwise
        } else {
            geosolve_sketch::DocumentAngleOrientation::CounterClockwise
        };
        let winding = required(document, "contact-winding")
            .ok()
            .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
            .and_then(|input| input.value().parse::<i32>().ok())
            .unwrap_or(0);
        let second_winding = required(document, "second-contact-winding")
            .ok()
            .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
            .and_then(|input| input.value().parse::<i32>().ok())
            .unwrap_or(0);
        state.set_branch_options(
            arc_sweep,
            super::ContactBranchOptions {
                neighborhood,
                tangent_orientation,
                winding,
            },
            super::ContactBranchOptions {
                neighborhood: second_neighborhood,
                tangent_orientation: second_tangent_orientation,
                winding: second_winding,
            },
            angle_orientation,
        );
    }

    fn select_object(control: &Element, additive: bool, app: &Rc<RefCell<PlaygroundState>>) {
        let Some(kind) = control.get_attribute("data-kind") else {
            return;
        };
        let Some(id) = control
            .get_attribute("data-id")
            .and_then(|id| PersistentId::from_str(&id).ok())
        else {
            return;
        };
        let item = match kind.as_str() {
            "point" => SelectionItem::Point(DesignPointId(id)),
            "curve" => {
                let curve = geosolve_sketch::CurveId(id);
                let parameter =
                    app.borrow()
                        .session()
                        .document()
                        .curve(curve)
                        .map_or(0.5, |curve| {
                            if matches!(curve.definition, CurveDefinition::Circle { .. }) {
                                0.0
                            } else {
                                0.5
                            }
                        });
                SelectionItem::Curve {
                    span: CurveSpan::line(curve),
                    parameter,
                }
            }
            "constraint" => SelectionItem::Constraint(DocumentConstraintId(id)),
            "contact" => {
                app.borrow_mut().toggle_contact_selection(ContactId(id));
                return;
            }
            "dimension" => SelectionItem::Dimension(DocumentDimensionId(id)),
            _ => return,
        };
        app.borrow_mut().set_object_selection(item, additive);
    }

    fn delete_object(control: &Element, app: &Rc<RefCell<PlaygroundState>>) {
        let Some(kind) = control.get_attribute("data-kind") else {
            return;
        };
        let Some(id) = control
            .get_attribute("data-id")
            .and_then(|id| PersistentId::from_str(&id).ok())
        else {
            return;
        };
        let object = match kind.as_str() {
            "constraint" => DocumentObjectId::Constraint(DocumentConstraintId(id)),
            "dimension" => DocumentObjectId::Dimension(DocumentDimensionId(id)),
            _ => return,
        };
        app.borrow_mut().delete_object(object);
    }

    fn tool(value: &str) -> Option<Tool> {
        Some(match value {
            "select" => Tool::Select,
            "pan" => Tool::Pan,
            "point" => Tool::Draw(DrawTool::Point),
            "line" => Tool::Draw(DrawTool::Line),
            "polyline" => Tool::Draw(DrawTool::Polyline),
            "rectangle" => Tool::Draw(DrawTool::Rectangle),
            "circle" => Tool::Draw(DrawTool::Circle),
            "arc" => Tool::Draw(DrawTool::Arc),
            "quadratic" => Tool::Draw(DrawTool::Quadratic),
            "cubic" => Tool::Draw(DrawTool::Cubic),
            _ => return None,
        })
    }
}
