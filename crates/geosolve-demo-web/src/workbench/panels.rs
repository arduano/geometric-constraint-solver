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
    use super::{lifecycle_presentation, problem_markup, tree_markup, tree_markup_with_pending};
    use geosolve_constraint_editor::{LifecycleStatus, SelectionItem};
    use geosolve_sketch::{SketchDatum, SketchDocument};

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
}
