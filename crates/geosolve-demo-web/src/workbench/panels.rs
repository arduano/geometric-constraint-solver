// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::fmt::Write as _;

use geosolve_constraint_editor::{ComputedFeatureProblemMetadata, LifecycleStatus, SelectionItem};
use geosolve_sketch::{GeometryRole, SketchDocument};
use geosolve_sketch_features::{
    ComputedCornerRef, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureEvaluationState, ComputedFeatureSnapshot,
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

#[cfg(test)]
pub(crate) fn tree_markup(document: &SketchDocument, selection: &[SelectionItem]) -> String {
    tree_markup_with_pending(document, selection, &[])
}

pub(crate) fn tree_markup_with_pending(
    document: &SketchDocument,
    selection: &[SelectionItem],
    pending: &[SelectionItem],
) -> String {
    let mut output = String::new();
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
    for curve in document.curves() {
        if let Ok(spans) = document.curve_spans(curve.id) {
            for span in spans {
                row(
                    &mut output,
                    "curve",
                    &span.curve.to_string(),
                    Some(span.segment),
                    &curve.label,
                    TreeIconKind::Curve,
                    selection.contains(&SelectionItem::Curve(span)),
                    pending.contains(&SelectionItem::Curve(span)),
                    match document.geometry_role(curve.id) {
                        Some(GeometryRole::Construction) => " data-role=\"construction\"",
                        _ => " data-role=\"profile\"",
                    },
                );
            }
        }
    }
    for constraint in document.constraints() {
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
    features: &ComputedFeatureDocument,
    snapshot: Option<&ComputedFeatureSnapshot>,
    problems: &[ComputedFeatureProblemMetadata],
    selection: &[SelectionItem],
    pending: &[SelectionItem],
) -> String {
    let mut output = String::from("<div class=\"wb-tree-group-label\"><span>Sketch</span></div>");
    output.push_str(&tree_markup_with_pending(document, selection, pending));
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
    use super::{lifecycle_presentation, problem_markup, tree_markup, tree_markup_with_pending};
    use geosolve_constraint_editor::{LifecycleStatus, SelectionItem};
    use geosolve_sketch::SketchDocument;

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
        assert!(!markup.contains("<span class=\"wb-tree-icon\"></span>"));
        let pending = tree_markup_with_pending(&document, &[], &[SelectionItem::Point(point)]);
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
}
