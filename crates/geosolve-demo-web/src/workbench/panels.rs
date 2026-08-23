// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::{collections::BTreeSet, fmt::Write as _};

use geosolve_constraint_editor::{
    ComputedFeatureProblemMetadata, LifecycleStatus, SceneConstraintEntry, SelectionItem,
};
use geosolve_sketch::{GeometryRole, SketchDatum, SketchDocument};
use geosolve_sketch_features::{
    ComputedCornerRef, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureEvaluationState, ComputedFeatureSnapshot,
};
use geosolve_sketch_lineage::{
    ImportedBaselineEncoding, LineageActionDefinition, LineageActionKind, LineageDocument,
    LineageStepState,
};

use super::icons::TreeIconKind;

pub(super) const fn lifecycle_presentation(
    status: LifecycleStatus,
) -> (&'static str, &'static str) {
    match status {
        LifecycleStatus::Accepted => ("accepted", "Accepted"),
        LifecycleStatus::DesignUnsolved => ("design-unsolved", "Design unsolved"),
        LifecycleStatus::RejectedAttempt => ("rejected-attempt", "Rejected attempt"),
        LifecycleStatus::SolvedPreview => ("solved-preview", "Solved preview"),
        LifecycleStatus::Solving => ("solving", "Solving"),
    }
}

pub(crate) fn problem_markup(problem: &str) -> String {
    format!(
        "<span class=\"wb-problem\" aria-label=\"Current sketch or computed-feature problem\" role=\"status\">{}</span>",
        escape(problem)
    )
}

/// Compact current-program identity for the read-only Lineage panel header.
pub(crate) fn lineage_summary(document: &LineageDocument) -> String {
    format!(
        "{} {} · r{}",
        document.steps().len(),
        if document.steps().len() == 1 {
            "action"
        } else {
            "actions"
        },
        document.revision().raw(),
    )
}

/// Read-only cursor presentation for the one authoritative Undo/Redo history.
pub(crate) fn lineage_history_markup(
    history_cursor: usize,
    history_len: usize,
    can_undo: bool,
    can_redo: bool,
) -> String {
    let position = if history_len == 0 {
        0
    } else {
        history_cursor.saturating_add(1).min(history_len)
    };
    format!(
        concat!(
            "<div class=\"wb-lineage-history-position\" ",
            "aria-label=\"Undo history position {position} of {history_len}; ",
            "Undo {undo_availability}; Redo {redo_availability}\">",
            "<span class=\"wb-lineage-history-cursor\">",
            "History <strong>{position} / {history_len}</strong></span>",
            "<span data-available=\"{can_undo}\">Undo {undo_mark}</span>",
            "<span data-available=\"{can_redo}\">Redo {redo_mark}</span>",
            "</div>"
        ),
        position = position,
        history_len = history_len,
        undo_availability = if can_undo { "available" } else { "unavailable" },
        redo_availability = if can_redo { "available" } else { "unavailable" },
        can_undo = can_undo,
        can_redo = can_redo,
        undo_mark = if can_undo { "✓" } else { "—" },
        redo_mark = if can_redo { "✓" } else { "—" },
    )
}

/// Chronological selectable rows derived freshly from the retained Rust action program.
pub(crate) fn lineage_markup(
    document: &LineageDocument,
    selected: Option<geosolve_sketch_lineage::LineageStepId>,
) -> String {
    let mut output = String::new();
    for (index, step) in document.steps().iter().enumerate() {
        let (kind_key, kind_label) = lineage_kind_presentation(step.action.kind());
        let (schema, version) = lineage_action_schema(&step.action);
        let action_name = lineage_action_name(&step.action, schema);
        let (state_key, state_label) = match step.state {
            LineageStepState::Live => ("live", "Live"),
            LineageStepState::Suppressed => ("suppressed", "Suppressed"),
            LineageStepState::Tombstoned => ("tombstoned", "Deleted"),
        };
        let is_selected = selected == Some(step.id);
        let is_pinned = matches!(
            &step.action,
            LineageActionDefinition::ImportedBaseline { .. }
        ) || step.state == LineageStepState::Tombstoned;
        let input_count = step.action.inputs().len();
        let output_count = step.outputs.len();
        let escaped_schema = escape(schema);
        let escaped_developer_key = escape(step.key.as_str());
        let escaped_label = escape(&step.label);
        let escaped_action_name = escape(&action_name);
        let display_step_id = format!("s{:x}", step.id.raw());
        let option_id = format!("wb-lineage-step-{:016x}", step.id.raw());
        let title = escape(&format!(
            "{} · Stable step {} · Developer key {} · {} v{}",
            step.label, step.id, step.key, schema, version
        ));
        let _ = write!(
            output,
            concat!(
                "<div id=\"{option_id}\" class=\"wb-lineage-row{selected_class}\" ",
                "role=\"option\" aria-selected=\"{is_selected}\" tabindex=\"-1\" ",
                "aria-posinset=\"{ordinal}\" aria-setsize=\"{step_count}\" ",
                "data-lineage-step-id=\"{step_id}\" data-lineage-action-kind=\"{kind_key}\" ",
                "data-lineage-ordinal=\"{ordinal}\" ",
                "data-lineage-step-state=\"{state_key}\" data-lineage-schema=\"{escaped_schema}\" ",
                "data-lineage-schema-version=\"{version}\" ",
                "data-lineage-developer-key=\"{escaped_developer_key}\" ",
                "data-lineage-draggable=\"{draggable}\" draggable=\"{draggable}\" ",
                "title=\"{title}\">",
                "<span class=\"wb-lineage-drag-grip\" aria-hidden=\"true\"></span>",
                "<span class=\"wb-lineage-order\" aria-hidden=\"true\">{ordinal}</span>",
                "<span class=\"wb-lineage-copy\">",
                "<span class=\"wb-lineage-heading\"><strong>{escaped_action_name}</strong>",
                "<code aria-label=\"Stable step {step_id}\">{display_step_id}</code></span>",
                "<span class=\"wb-lineage-owner-label\">{escaped_label}</span>",
                "<span class=\"wb-lineage-meta\">{kind_label} · {inputs} · {outputs}</span>",
                "<span class=\"wb-lineage-key\">Key {escaped_developer_key}</span>",
                "<span class=\"wb-lineage-schema\">{escaped_schema} · v{version}</span>",
                "</span>",
                "<span class=\"wb-lineage-badges\">",
                "<span class=\"wb-lineage-state\">{state_label}</span>",
                "{movement_badge}",
                "</span>",
                "</div>"
            ),
            option_id = option_id,
            selected_class = if is_selected { " selected" } else { "" },
            is_selected = is_selected,
            step_id = step.id,
            ordinal = index + 1,
            step_count = document.steps().len(),
            kind_key = kind_key,
            state_key = state_key,
            escaped_schema = escaped_schema,
            escaped_developer_key = escaped_developer_key,
            title = title,
            escaped_action_name = escaped_action_name,
            display_step_id = display_step_id,
            escaped_label = escaped_label,
            kind_label = kind_label,
            inputs = count_label(input_count, "input", "inputs"),
            outputs = count_label(output_count, "output", "outputs"),
            version = version,
            state_label = state_label,
            draggable = !is_pinned,
            movement_badge = if is_pinned {
                "<span class=\"wb-lineage-movement\">Pinned</span>"
            } else {
                ""
            },
        );
    }
    if output.is_empty() {
        output.push_str(
            "<p class=\"wb-empty wb-lineage-empty\" role=\"status\">No retained actions</p>",
        );
    }
    output
}

const fn lineage_kind_presentation(kind: LineageActionKind) -> (&'static str, &'static str) {
    match kind {
        LineageActionKind::ImportedBaseline => ("imported-baseline", "Baseline"),
        LineageActionKind::GeometryRecipe => ("geometry-recipe", "Geometry"),
        LineageActionKind::Constraint => ("constraint", "Constraint"),
        LineageActionKind::Dimension => ("dimension", "Dimension"),
        LineageActionKind::Trim => ("trim", "Trim"),
        LineageActionKind::Parameter => ("parameter", "Parameter"),
        LineageActionKind::Binding => ("binding", "Binding"),
        LineageActionKind::External => ("external", "External"),
        LineageActionKind::Operation => ("operation", "Operation"),
        LineageActionKind::ComputedFeature => ("computed-feature", "Computed feature"),
        LineageActionKind::Annotation => ("annotation", "Annotation"),
    }
}

fn lineage_action_schema(action: &LineageActionDefinition) -> (&str, u32) {
    match action {
        LineageActionDefinition::ImportedBaseline { baseline } => match &baseline.encoding {
            ImportedBaselineEncoding::Canonical { schema, version } => (schema.as_str(), *version),
            ImportedBaselineEncoding::Opaque {
                media_type,
                version,
            } => (media_type.as_str(), *version),
        },
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => {
            (action.schema.as_str(), action.version)
        }
    }
}

fn lineage_action_name(action: &LineageActionDefinition, schema: &str) -> String {
    if matches!(action, LineageActionDefinition::ImportedBaseline { .. }) {
        return "Imported baseline".into();
    }
    if let Some(key) = schema.strip_prefix("geosolve.geometry.v1.")
        && let Some(variant) = super::geometry_palette::variant_from_key(key)
    {
        return super::geometry_palette::variant_label(variant).into();
    }
    humanize_schema_suffix(schema)
}

fn humanize_schema_suffix(schema: &str) -> String {
    let suffix = schema.rsplit('.').next().unwrap_or(schema);
    let words = suffix
        .split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(humanize_schema_word)
        .collect::<Vec<_>>();
    if words.is_empty() {
        schema.to_owned()
    } else {
        words.join(" ")
    }
}

fn humanize_schema_word(word: &str) -> String {
    match word {
        "nurbs" => "NURBS".into(),
        "2d" => "2D".into(),
        "id" => "ID".into(),
        "g0" => "G0".into(),
        "g1" => "G1".into(),
        "g2" => "G2".into(),
        "x" => "X".into(),
        "y" => "Y".into(),
        _ => {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        }
    }
}

fn count_label(count: usize, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

#[cfg(test)]
pub(crate) fn tree_markup(document: &SketchDocument, selection: &[SelectionItem]) -> String {
    tree_markup_with_pending(document, &[], selection, &[])
}

#[cfg(test)]
pub(crate) fn tree_markup_with_pending(
    document: &SketchDocument,
    constraint_entries: &[SceneConstraintEntry],
    selection: &[SelectionItem],
    pending: &[SelectionItem],
) -> String {
    tree_markup_with_pending_and_implicit(
        document,
        constraint_entries,
        selection,
        pending,
        &BTreeSet::new(),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one ordered tree pass keeps Profile/Construction grouping and persistent rows auditable"
)]
fn tree_markup_with_pending_and_implicit(
    document: &SketchDocument,
    constraint_entries: &[SceneConstraintEntry],
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    implicit_spans: &BTreeSet<geosolve_sketch::CurveSpan>,
) -> String {
    let mut output = String::new();
    group_label(&mut output, "References", 3);
    for (datum, label, detail, icon) in [
        (
            SketchDatum::Origin,
            "Origin",
            "Fixed model zero · protected",
            TreeIconKind::DatumOrigin,
        ),
        (
            SketchDatum::XAxis,
            "X axis",
            "Infinite horizontal datum · protected",
            TreeIconKind::DatumAxis,
        ),
        (
            SketchDatum::YAxis,
            "Y axis",
            "Infinite vertical datum · protected",
            TreeIconKind::DatumAxis,
        ),
    ] {
        datum_row(
            &mut output,
            datum,
            label,
            detail,
            icon,
            selection.contains(&SelectionItem::Datum(datum)),
            pending.contains(&SelectionItem::Datum(datum)),
        );
    }
    if !document.points().is_empty() {
        group_label(&mut output, "Points", document.points().len());
    }
    for point in document.points() {
        row(
            &mut output,
            "point",
            &point.id.to_string(),
            None,
            &point.label,
            TreeIconKind::Point,
            selection.contains(&SelectionItem::Point(point.id)),
            pending.contains(&SelectionItem::Point(point.id)),
            "",
        );
    }
    for role in [GeometryRole::Profile, GeometryRole::Construction] {
        let curves = document
            .curves()
            .iter()
            .filter(|curve| document.geometry_role(curve.id) == Some(role))
            .collect::<Vec<_>>();
        if curves.is_empty() {
            continue;
        }
        group_label(
            &mut output,
            match role {
                GeometryRole::Profile => "Profile geometry",
                GeometryRole::Construction => "Construction geometry",
            },
            curves.len(),
        );
        for curve in curves {
            if let Ok(spans) = document.curve_spans(curve.id) {
                for span in spans {
                    let mut role_attributes = match role {
                        GeometryRole::Construction => " data-role=\"construction\"".to_owned(),
                        GeometryRole::Profile => " data-role=\"profile\"".to_owned(),
                    };
                    if implicit_spans.contains(&span) {
                        role_attributes.push_str(concat!(
                            " data-has-implicit-construction=\"true\"",
                            " title=\"Fillet-hidden construction occurrence available\"",
                        ));
                    }
                    row(
                        &mut output,
                        "curve",
                        &span.curve.to_string(),
                        Some(span.segment),
                        &curve.label,
                        TreeIconKind::Curve,
                        selection.contains(&SelectionItem::Curve(span)),
                        pending.contains(&SelectionItem::Curve(span)),
                        &role_attributes,
                    );
                }
            }
        }
    }
    if !constraint_entries.is_empty() {
        group_label(&mut output, "Constraints", constraint_entries.len());
    }
    for constraint in constraint_entries {
        row(
            &mut output,
            "constraint",
            &constraint.id.to_string(),
            None,
            &constraint.label,
            TreeIconKind::Constraint,
            selection.contains(&SelectionItem::Constraint(constraint.id)),
            pending.contains(&SelectionItem::Constraint(constraint.id)),
            "",
        );
    }
    if !document.dimensions().is_empty() {
        group_label(&mut output, "Dimensions", document.dimensions().len());
    }
    for dimension in document.dimensions() {
        row(
            &mut output,
            "dimension",
            &dimension.id.to_string(),
            None,
            &dimension.label,
            TreeIconKind::Dimension,
            selection.contains(&SelectionItem::Dimension(dimension.id)),
            pending.contains(&SelectionItem::Dimension(dimension.id)),
            match dimension.mode {
                geosolve_sketch::DocumentDimensionMode::Driving => {
                    " data-dimension-mode=\"driving\""
                }
                geosolve_sketch::DocumentDimensionMode::Reference => {
                    " data-dimension-mode=\"reference\""
                }
            },
        );
    }
    if !document.external_bindings().is_empty() {
        group_label(
            &mut output,
            "External references",
            document.external_bindings().len(),
        );
    }
    for binding in document.external_bindings() {
        let topology = binding
            .expected_topology
            .map_or_else(|| "none".to_owned(), |value| short_digest(value.bytes()));
        let _ = write!(
            output,
            "<div class=\"wb-tree-row wb-tree-external\" role=\"treeitem\" data-external-binding=\"{}\" data-external-kind=\"{:?}\" data-external-topology=\"{}\"><span class=\"wb-tree-icon\">{}</span>{}</div>",
            binding.id,
            binding.expected_kind,
            topology,
            super::icons::tree_icon_markup(TreeIconKind::External),
            escape(&binding.label),
        );
    }
    if output.is_empty() {
        output.push_str("<p class=\"wb-empty\">No sketch objects</p>");
    }
    output
}

pub(crate) fn tree_markup_with_features(
    document: &SketchDocument,
    constraint_entries: &[SceneConstraintEntry],
    features: &ComputedFeatureDocument,
    snapshot: Option<&ComputedFeatureSnapshot>,
    problems: &[ComputedFeatureProblemMetadata],
    selection: &[SelectionItem],
    pending: &[SelectionItem],
) -> String {
    let implicit_spans = snapshot
        .into_iter()
        .flat_map(ComputedFeatureSnapshot::construction_fragments)
        .map(|fragment| fragment.source.span)
        .collect::<BTreeSet<_>>();
    let mut output = tree_markup_with_pending_and_implicit(
        document,
        constraint_entries,
        selection,
        pending,
        &implicit_spans,
    );
    let _ = write!(
        output,
        "<div class=\"wb-tree-group-label\"><span>Features</span><span>{}</span></div>",
        features.features().len()
    );
    if features.features().is_empty() {
        output.push_str("<p class=\"wb-empty\">No computed features</p>");
        return output;
    }
    for feature in features.features() {
        let evaluation = snapshot.and_then(|snapshot| {
            snapshot
                .feature_evaluations()
                .iter()
                .find(|value| value.feature == feature.id)
        });
        let (state, detail) = match evaluation.map(|value| &value.state) {
            Some(ComputedFeatureEvaluationState::Current { .. }) => ("current", String::new()),
            Some(ComputedFeatureEvaluationState::Failed { failure }) => (
                "failed",
                format!(" title=\"{}\"", escape(&failure.to_string())),
            ),
            Some(ComputedFeatureEvaluationState::Suppressed) | None if feature.suppressed => {
                ("suppressed", String::new())
            }
            None => ("unavailable", String::new()),
            Some(ComputedFeatureEvaluationState::Suppressed) => ("suppressed", String::new()),
        };
        let selected = selection.contains(&SelectionItem::Feature(feature.id));
        let has_problem = problems
            .iter()
            .any(|problem| problem.feature == Some(feature.id));
        let _ = write!(
            output,
            "<button class=\"wb-tree-row wb-tree-feature{}{}\" role=\"treeitem\" aria-selected=\"{}\" data-editor-item=\"feature\" data-feature-id=\"{}\" data-feature-state=\"{state}\"{}{}><span class=\"wb-tree-icon\">{}</span>{}</button>",
            if selected { " selected" } else { "" },
            if has_problem { " has-problem" } else { "" },
            selected,
            feature.id,
            if has_problem {
                " data-feature-problem=\"true\""
            } else {
                ""
            },
            detail,
            super::icons::tree_icon_markup(TreeIconKind::Feature),
            escape(&feature.label),
        );
        let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
        for (index, corner) in fillet.corners.iter().enumerate() {
            let item = SelectionItem::FeatureCorner(ComputedCornerRef {
                feature: feature.id,
                corner: corner.id,
            });
            let selected = selection.contains(&item);
            let has_problem = problems.iter().any(|problem| {
                problem.feature == Some(feature.id) && problem.corners.contains(&corner.id)
            });
            let _ = write!(
                output,
                "<button class=\"wb-tree-row wb-tree-feature-corner{}{}\" role=\"treeitem\" aria-selected=\"{}\" data-editor-item=\"feature-corner\" data-feature-id=\"{}\" data-feature-corner-id=\"{}\"{}><span class=\"wb-tree-icon\">{}</span>Corner {}</button>",
                if selected { " selected" } else { "" },
                if has_problem { " has-problem" } else { "" },
                selected,
                feature.id,
                corner.id,
                if has_problem {
                    " data-feature-problem=\"true\""
                } else {
                    ""
                },
                super::icons::tree_icon_markup(TreeIconKind::FeatureCorner),
                index + 1,
            );
        }
    }
    output
}

fn group_label(output: &mut String, label: &str, count: usize) {
    let _ = write!(
        output,
        "<div class=\"wb-tree-group-label\"><span>{}</span><span>{count}</span></div>",
        escape(label),
    );
}

fn datum_row(
    output: &mut String,
    datum: SketchDatum,
    label: &str,
    detail: &str,
    icon: TreeIconKind,
    selected: bool,
    pending: bool,
) {
    let key = match datum {
        SketchDatum::Origin => "origin",
        SketchDatum::XAxis => "x-axis",
        SketchDatum::YAxis => "y-axis",
    };
    let _ = write!(
        output,
        concat!(
            "<button class=\"wb-tree-row wb-tree-datum{}{}\" role=\"treeitem\" ",
            "aria-selected=\"{}\" aria-label=\"{} · {}\" data-editor-item=\"datum\" ",
            "data-datum=\"{}\" data-protected=\"true\" title=\"{}\">",
            "<span class=\"wb-tree-icon\">{}</span>{}<span class=\"wb-tree-protected\">fixed</span></button>"
        ),
        if selected { " selected" } else { "" },
        if pending { " authoring-pending" } else { "" },
        selected,
        escape(label),
        escape(detail),
        key,
        escape(detail),
        super::icons::tree_icon_markup(icon),
        escape(label),
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "tree rows keep typed selection and authoring-pending presentation explicit"
)]
fn row(
    output: &mut String,
    kind: &str,
    id: &str,
    segment: Option<u32>,
    label: &str,
    icon_kind: TreeIconKind,
    selected: bool,
    pending: bool,
    extra: &str,
) {
    let label = escape(label);
    let segment = segment.map_or_else(String::new, |value| {
        format!(" data-editor-segment=\"{value}\"")
    });
    let _ = write!(
        output,
        "<button class=\"wb-tree-row{}{}\" role=\"treeitem\" aria-selected=\"{}\" data-editor-item=\"{kind}\" data-persistent-id=\"{id}\"{segment}{extra}><span class=\"wb-tree-icon\">{}</span>{label}</button>",
        if selected { " selected" } else { "" },
        if pending { " authoring-pending" } else { "" },
        if selected { "true" } else { "false" },
        super::icons::tree_icon_markup(icon_kind),
    );
}

fn short_digest(bytes: [u8; 32]) -> String {
    let mut output = String::new();
    for byte in &bytes[..6] {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

pub(crate) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::{
        lifecycle_presentation, lineage_history_markup, lineage_markup, lineage_summary,
        problem_markup, tree_markup, tree_markup_with_pending,
    };
    use geosolve_constraint_editor::{LifecycleStatus, RetainedEditorCoordinator, SelectionItem};
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        DocumentEdit, DocumentSolveRequest, RetainedSketchDocumentSession, SketchDatum,
        SketchDocument,
    };
    use geosolve_sketch_lineage::{
        ImportedBaselineAction, ImportedBaselineEncoding, LineageActionDefinition,
        LineageDeveloperKey, LineageDocument, LineageDocumentId, LineageInputBinding,
        LineageMutation, LineageOutput, LineageOutputId, LineageOutputKind, LineagePatch,
        LineageSemanticKey, LineageStep, LineageStepId, LineageStepRewrite, VersionedActionPayload,
    };

    fn semantic_key(value: &str) -> LineageSemanticKey {
        LineageSemanticKey::new(value).expect("semantic key")
    }

    fn developer_key(value: &str) -> LineageDeveloperKey {
        LineageDeveloperKey::new(value).expect("developer key")
    }

    fn insert(document: &mut LineageDocument, step: LineageStep) {
        document
            .apply_patch(LineagePatch::new(
                document.identity(),
                vec![LineageMutation::Insert {
                    before: None,
                    step: Box::new(step),
                }],
            ))
            .expect("insert lineage step");
    }

    fn lineage_fixture() -> LineageDocument {
        let document_id = LineageDocumentId::from_raw(0x83);
        let mut document = LineageDocument::with_id(document_id);
        insert(
            &mut document,
            LineageStep::new(
                LineageStepId::from_raw(1),
                developer_key("imported-baseline"),
                "Imported baseline",
                LineageActionDefinition::ImportedBaseline {
                    baseline: ImportedBaselineAction {
                        encoding: ImportedBaselineEncoding::Opaque {
                            media_type: semantic_key("application/vnd.geosolve.test+json"),
                            version: 1,
                        },
                        payload: "{}".into(),
                    },
                },
                vec![LineageOutput {
                    id: LineageOutputId::from_raw(1),
                    key: semantic_key("scene"),
                    kind: LineageOutputKind::Collection,
                    reservation: None,
                }],
                Vec::new(),
            ),
        );
        let baseline_scene = document
            .output_ref(LineageStepId::from_raw(1), LineageOutputId::from_raw(1))
            .expect("baseline scene output");
        insert(
            &mut document,
            LineageStep::new(
                LineageStepId::from_raw(2),
                developer_key("geometry-\"recipe"),
                "Point <script> & guide",
                LineageActionDefinition::GeometryRecipe {
                    action: VersionedActionPayload {
                        schema: semantic_key("geosolve.geometry.v1.sketch-point\"<unsafe>"),
                        version: 1,
                        inputs: vec![LineageInputBinding {
                            key: semantic_key("scene"),
                            kind: LineageOutputKind::Collection,
                            source: baseline_scene,
                        }],
                        parameters: std::collections::BTreeMap::new(),
                    },
                },
                Vec::new(),
                Vec::new(),
            ),
        );
        insert(
            &mut document,
            LineageStep::new(
                LineageStepId::from_raw(3),
                developer_key("delete-operation"),
                "Delete",
                LineageActionDefinition::Operation {
                    action: VersionedActionPayload::empty(
                        semantic_key("geosolve.operation.v1.delete"),
                        1,
                    ),
                },
                Vec::new(),
                Vec::new(),
            ),
        );
        document
            .apply_patch(LineagePatch::new(
                document.identity(),
                vec![
                    LineageMutation::SetSuppressed {
                        step: LineageStepId::from_raw(2),
                        suppressed: true,
                    },
                    LineageMutation::Tombstone {
                        step: LineageStepId::from_raw(3),
                    },
                ],
            ))
            .expect("set retained lifecycle states");
        document
    }

    #[test]
    fn tree_problem_and_lifecycle_markup_preserve_typed_semantics() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let point = document.add_point("A < origin", [0.0, 0.0]).unwrap();
        let markup = tree_markup(&document, &[SelectionItem::Point(point)]);
        assert!(markup.contains("role=\"treeitem\""));
        assert!(markup.contains("aria-selected=\"true\""));
        assert!(markup.contains(&format!("data-persistent-id=\"{point}\"")));
        assert!(markup.contains("A &lt; origin"));
        assert!(markup.contains("class=\"wb-tree-symbol\""));
        assert!(markup.contains("data-tree-icon=\"point\""));
        assert!(markup.contains("<span>References</span><span>3</span>"));
        for key in ["origin", "x-axis", "y-axis"] {
            assert!(markup.contains(&format!("data-datum=\"{key}\"")));
        }
        assert_eq!(markup.matches("data-protected=\"true\"").count(), 3);
        let datum_selection = tree_markup(&document, &[SelectionItem::Datum(SketchDatum::XAxis)]);
        let selected_axis = datum_selection
            .split_once("data-datum=\"x-axis\"")
            .map(|(prefix, _)| &prefix[prefix.rfind("<button").expect("datum row")..])
            .expect("x axis row");
        assert!(selected_axis.contains("selected"));
        assert!(selected_axis.contains("aria-selected=\"true\""));
        assert!(!markup.contains("<span class=\"wb-tree-icon\"></span>"));
        let pending = tree_markup_with_pending(&document, &[], &[], &[SelectionItem::Point(point)]);
        assert!(pending.contains("wb-tree-row authoring-pending"));
        assert!(pending.contains("aria-selected=\"false\""));
        assert_eq!(
            lifecycle_presentation(LifecycleStatus::RejectedAttempt),
            ("rejected-attempt", "Rejected attempt")
        );
        let problem = problem_markup("bad < geometry");
        assert!(problem.contains("aria-label=\"Current sketch or computed-feature problem\""));
        assert!(problem.contains("role=\"status\""));
        assert!(problem.contains("bad &lt; geometry"));
    }

    #[test]
    fn constraint_rows_consume_headless_entries_even_for_rejected_design_intent() {
        use geosolve_constraint_editor::constraint_entries;
        use geosolve_sketch::DocumentConstraintDefinition;

        let mut document = SketchDocument::new(8.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let second = document.add_point("second", [1.0, 2.0]).expect("point");
        let constraint = document
            .add_constraint(
                "Design-only horizontal < relation",
                DocumentConstraintDefinition::HorizontalPoints { first, second },
            )
            .expect("constraint");
        let entries = constraint_entries(&document);
        let markup = tree_markup_with_pending(
            &document,
            &entries,
            &[SelectionItem::Constraint(constraint)],
            &[],
        );
        assert!(markup.contains("Design-only horizontal &lt; relation"));
        assert!(markup.contains(&format!("data-persistent-id=\"{constraint}\"")));
        assert!(markup.contains("aria-selected=\"true\""));

        let without_entries = tree_markup_with_pending(
            &document,
            &[],
            &[SelectionItem::Constraint(constraint)],
            &[],
        );
        assert!(
            !without_entries.contains("Design-only horizontal"),
            "the workbench must not silently fall back to interpreting document constraints"
        );
    }

    #[test]
    fn lineage_markup_preserves_authoritative_order_identity_and_lifecycle() {
        let document = lineage_fixture();
        let markup = lineage_markup(&document, Some(LineageStepId::from_raw(2)));
        let history = lineage_history_markup(1, 4, true, true);

        assert_eq!(lineage_summary(&document), "3 actions · r4");
        assert_eq!(markup.matches("role=\"option\"").count(), 3);
        let baseline = markup
            .find("data-lineage-step-id=\"0000000000000001\"")
            .expect("baseline row");
        let geometry = markup
            .find("data-lineage-step-id=\"0000000000000002\"")
            .expect("geometry row");
        let deleted = markup
            .find("data-lineage-step-id=\"0000000000000003\"")
            .expect("deleted row");
        assert!(baseline < geometry && geometry < deleted);
        assert!(markup.contains("data-lineage-action-kind=\"imported-baseline\""));
        assert!(markup.contains("data-lineage-action-kind=\"geometry-recipe\""));
        assert!(markup.contains("data-lineage-action-kind=\"operation\""));
        assert!(markup.contains("data-lineage-step-state=\"live\""));
        assert!(markup.contains("data-lineage-step-state=\"suppressed\""));
        assert!(markup.contains("data-lineage-step-state=\"tombstoned\""));
        assert!(markup.contains(">Live<"));
        assert!(markup.contains(">Suppressed<"));
        assert!(markup.contains(">Deleted<"));
        assert!(markup.contains("0 inputs · 1 output"));
        assert!(markup.contains("1 input · 0 outputs"));
        assert!(history.contains("History <strong>2 / 4</strong>"));
        assert!(history.contains("data-available=\"true\">Undo"));
        assert!(history.contains("data-available=\"true\">Redo"));
        let bounded_history = lineage_history_markup(1_024, 2_049, true, true);
        assert!(bounded_history.contains("History <strong>1025 / 2049</strong>"));
        assert!(bounded_history.contains("class=\"wb-lineage-history-cursor\""));
        assert!(markup.contains("Point &lt;script&gt; &amp; guide"));
        assert!(markup.contains(
            "data-lineage-schema=\"geosolve.geometry.v1.sketch-point&quot;&lt;unsafe&gt;\""
        ));
        assert!(markup.contains("data-lineage-developer-key=\"geometry-&quot;recipe\""));
        assert!(markup.contains("Key geometry-&quot;recipe"));
        assert!(markup.contains("data-lineage-schema-version=\"1\""));
        assert!(markup.contains(concat!(
            "id=\"wb-lineage-step-0000000000000002\" ",
            "class=\"wb-lineage-row selected\" role=\"option\" ",
            "aria-selected=\"true\" tabindex=\"-1\""
        )));
        assert_eq!(markup.matches("aria-selected=\"true\"").count(), 1);
        assert_eq!(markup.matches("aria-setsize=\"3\"").count(), 3);
        assert!(markup.contains("aria-posinset=\"2\""));
        assert!(markup.contains("data-lineage-ordinal=\"2\""));
        assert_eq!(markup.matches("data-lineage-draggable=\"true\"").count(), 1);
        assert_eq!(
            markup.matches("data-lineage-draggable=\"false\"").count(),
            2
        );
        assert_eq!(markup.matches(">Pinned<").count(), 2);
        for forbidden in ["<button", "data-editor-item"] {
            assert!(
                !markup.contains(forbidden),
                "lineage rows must not impersonate sketch-tree controls: {forbidden}"
            );
        }
    }

    #[test]
    fn lineage_markup_reflects_rewrite_without_inventing_an_event_row() {
        let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x8302));
        let step_id = LineageStepId::from_raw(1);
        insert(
            &mut document,
            LineageStep::new(
                step_id,
                developer_key("editable-owner"),
                "Original action",
                LineageActionDefinition::GeometryRecipe {
                    action: VersionedActionPayload::empty(
                        semantic_key("geosolve.geometry.v1.segment"),
                        1,
                    ),
                },
                Vec::new(),
                Vec::new(),
            ),
        );
        let before = lineage_markup(&document, Some(step_id));
        let before_summary = lineage_summary(&document);

        document
            .apply_patch(LineagePatch::new(
                document.identity(),
                vec![LineageMutation::Rewrite {
                    step: step_id,
                    replacement: Box::new(LineageStepRewrite {
                        label: "Rewritten action".into(),
                        action: LineageActionDefinition::GeometryRecipe {
                            action: VersionedActionPayload::empty(
                                semantic_key("geosolve.geometry.v1.midpoint-line"),
                                1,
                            ),
                        },
                    }),
                }],
            ))
            .expect("rewrite retained owner");
        let after = lineage_markup(&document, Some(step_id));
        let history = lineage_history_markup(0, 1, false, false);

        assert_eq!(before.matches("role=\"option\"").count(), 1);
        assert_eq!(after.matches("role=\"option\"").count(), 1);
        assert!(after.contains("data-lineage-step-id=\"0000000000000001\""));
        assert!(before.contains("Original action"));
        assert!(after.contains("Rewritten action"));
        assert!(!after.contains("Original action"));
        assert_ne!(before_summary, lineage_summary(&document));
        assert_eq!(lineage_summary(&document), "1 action · r2");
        assert!(history.contains("data-available=\"false\">Undo"));
        assert!(history.contains("data-available=\"false\">Redo"));
    }

    #[test]
    fn lineage_panel_tracks_the_real_coordinator_program_and_history_cursor() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(1.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

        assert_eq!(coordinator.lineage_document().steps().len(), 1);
        assert_eq!(
            (coordinator.history_cursor(), coordinator.history_len()),
            (0, 1)
        );
        assert!(!coordinator.can_undo());
        assert!(!coordinator.can_redo());
        let initial = lineage_markup(coordinator.lineage_document(), None);
        assert!(initial.contains("Imported baseline"));

        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "Lineage panel point".into(),
                    position: [2.0, 3.0],
                },
            )
            .expect("create retained point");
        assert_eq!(coordinator.lineage_document().steps().len(), 2);
        assert_eq!(
            (coordinator.history_cursor(), coordinator.history_len()),
            (1, 2)
        );
        assert!(coordinator.can_undo());
        assert!(!coordinator.can_redo());
        let created = lineage_markup(coordinator.lineage_document(), None);
        assert_eq!(created.matches("role=\"option\"").count(), 2);
        assert!(created.contains("geosolve.document-edit.v1.create-point"));

        coordinator.undo().expect("undo point action");
        assert_eq!(coordinator.lineage_document().steps().len(), 1);
        assert_eq!(
            (coordinator.history_cursor(), coordinator.history_len()),
            (0, 2)
        );
        assert!(!coordinator.can_undo());
        assert!(coordinator.can_redo());
        assert_eq!(
            lineage_markup(coordinator.lineage_document(), None),
            initial
        );

        coordinator.redo().expect("redo point action");
        assert_eq!(coordinator.lineage_document().steps().len(), 2);
        assert_eq!(
            (coordinator.history_cursor(), coordinator.history_len()),
            (1, 2)
        );
        assert!(coordinator.can_undo());
        assert!(!coordinator.can_redo());
        assert_eq!(
            lineage_markup(coordinator.lineage_document(), None),
            created
        );
    }

    #[test]
    fn lineage_panel_has_truthful_empty_program_presentation() {
        let document = LineageDocument::with_id(LineageDocumentId::from_raw(0x8303));
        assert_eq!(lineage_summary(&document), "0 actions · r0");
        let markup = lineage_markup(&document, None);
        assert!(markup.contains("No retained actions"));
        assert!(!markup.contains("data-lineage-step-id"));
    }

    #[test]
    fn lineage_selection_and_inspector_markup_are_accessible_and_bounded() {
        let html = include_str!("../../index.html");
        assert!(html.contains(concat!(
            "id=\"wb-lineage\" class=\"wb-lineage\" role=\"listbox\" ",
            "aria-label=\"Retained sketch actions\" aria-multiselectable=\"false\" ",
            "tabindex=\"0\""
        )));
        for id in [
            "wb-lineage-inspector",
            "wb-lineage-inspector-label",
            "wb-lineage-inspector-state",
            "wb-lineage-inspector-meta",
            "wb-lineage-inspector-graph",
            "wb-lineage-reorder",
            "wb-lineage-move-earlier",
            "wb-lineage-move-later",
            "wb-lineage-position",
            "wb-lineage-reorder-note",
            "wb-lineage-debug-editor",
            "wb-lineage-debug-json",
            "wb-lineage-debug-apply",
            "wb-lineage-debug-reset",
            "wb-lineage-debug-status",
        ] {
            assert_eq!(
                html.matches(&format!("id=\"{id}\"")).count(),
                1,
                "#{id} must have one presentation owner"
            );
        }
        assert!(html.contains("<details id=\"wb-lineage-debug-editor\""));
        assert!(!html.contains("<details id=\"wb-lineage-debug-editor\" open"));
        assert!(html.contains("aria-live=\"polite\""));

        let css = include_str!("../../styles.css");
        for contract in [
            ".wb-lineage-row[aria-selected=\"true\"]",
            ".wb-lineage-row[data-lineage-draggable=\"true\"]",
            ".wb-lineage-row[data-lineage-drop=\"before\"]",
            ".wb-lineage-row[data-lineage-drop=\"after\"]",
            ".wb-lineage-inspector-meta dd",
            "overflow-wrap: anywhere;",
            ".wb-lineage-debug-editor textarea",
            "resize: vertical;",
        ] {
            assert!(
                css.contains(contract),
                "missing lineage CSS contract: {contract}"
            );
        }
    }
}
