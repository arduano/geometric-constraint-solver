// SPDX-License-Identifier: GPL-3.0-or-later

//! Browser markup for the editor-owned projectional Design Intent DTOs.
//!
//! The browser deliberately does not inspect an [`IntentSession`] or generate
//! a second source representation. Stable selection, Inspector coordinates,
//! recognized source tokens and read-only History all come from
//! `geosolve-constraint-editor`'s equation-free projection.

use std::fmt::Write;

use geosolve_constraint_editor::{
    IntentInspectorField, IntentInspectorProjection, IntentOutlineDeclaration, IntentSourceToken,
    IntentSourceTokenTarget, IntentWorkbenchProjection,
};
use geosolve_sketch_intent::{
    IntentLiteral, IntentLiteralSchema, IntentNodeKind, IntentPatchOperationKind,
    IntentPlanDisposition, LeafField, NodeId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DesignProjectionSelection {
    pub node: NodeId,
}

pub(crate) fn declaration_count(projection: &IntentWorkbenchProjection) -> usize {
    projection
        .outline
        .iter()
        .map(|cell| cell.declarations.len())
        .sum()
}

pub(crate) fn outline_markup(
    projection: &IntentWorkbenchProjection,
    selection: Option<DesignProjectionSelection>,
) -> String {
    let mut markup = String::new();
    for cell in &projection.outline {
        let _ = write!(
            markup,
            concat!(
                "<section class=\"wb-intent-cell\" data-intent-cell=\"{}\">",
                "<header><h3>{}</h3><span>{} declaration{}</span></header>"
            ),
            cell.cell,
            escape_html(cell.name.as_str()),
            cell.declarations.len(),
            if cell.declarations.len() == 1 {
                ""
            } else {
                "s"
            },
        );
        for declaration in &cell.declarations {
            push_declaration_button(&mut markup, declaration, selection);
        }
        markup.push_str("</section>");
    }
    markup
}

fn push_declaration_button(
    markup: &mut String,
    declaration: &IntentOutlineDeclaration,
    selection: Option<DesignProjectionSelection>,
) {
    let selected = selection.is_some_and(|selection| selection.node == declaration.node);
    let state = if declaration.suppressed {
        "suppressed"
    } else if declaration.retained_failure {
        "invalid"
    } else {
        "current"
    };
    let _ = write!(
        markup,
        concat!(
            "<button type=\"button\" class=\"wb-intent-row{}\" ",
            "id=\"wb-intent-node-{}\" data-intent-node=\"{}\" ",
            "data-intent-state=\"{}\" draggable=\"true\" role=\"treeitem\" ",
            "aria-selected=\"{}\"><span class=\"wb-intent-kind\">{}</span>",
            "<span class=\"wb-intent-name\">{}</span><small>{}</small></button>"
        ),
        if selected { " selected" } else { "" },
        declaration.node,
        declaration.node,
        state,
        selected,
        escape_html(node_family_label(&declaration.kind)),
        escape_html(declaration.name.as_str()),
        escape_html(&humanize_debug(&declaration.kind)),
    );
}

/// Renders the exact editor-owned structured source, wrapping only recognized
/// token ranges with stable typed edit coordinates. The source is displayed as
/// code and is never evaluated by the browser.
pub(crate) fn structured_source_markup(
    projection: &IntentWorkbenchProjection,
    selection: Option<DesignProjectionSelection>,
) -> String {
    let source = &projection.structured_source;
    let mut markup = String::new();
    let mut line_start = 0;
    for (line_number, inclusive) in source.text.split_inclusive('\n').enumerate() {
        let line = inclusive.strip_suffix('\n').unwrap_or(inclusive);
        let line_end = line_start + line.len();
        let line_tokens = source
            .tokens
            .iter()
            .filter(|token| token.start >= line_start && token.end <= line_end)
            .collect::<Vec<_>>();
        let selected = line_tokens.iter().any(|token| {
            let node = token_node(&token.target);
            selection.is_some_and(|value| value.node == node)
        });
        let node = line_tokens.first().map(|token| token_node(&token.target));
        let _ = write!(
            markup,
            "<div class=\"wb-intent-source-line{}\"{}><span>{}</span><code>",
            if selected { " selected" } else { "" },
            node.map_or_else(String::new, |node| format!(" data-intent-node=\"{node}\"")),
            line_number + 1,
        );
        push_source_line(&mut markup, line, line_start, &line_tokens);
        markup.push_str("</code></div>");
        line_start += inclusive.len();
    }
    markup
}

fn push_source_line(
    markup: &mut String,
    line: &str,
    line_start: usize,
    tokens: &[&IntentSourceToken],
) {
    let mut cursor = line_start;
    for token in tokens {
        if token.start < cursor {
            continue;
        }
        let prefix_start = cursor - line_start;
        let prefix_end = token.start - line_start;
        markup.push_str(&escape_html(&line[prefix_start..prefix_end]));
        let token_start = token.start - line_start;
        let token_end = token.end - line_start;
        let node = token_node(&token.target);
        let _ = write!(
            markup,
            concat!(
                "<span class=\"wb-intent-source-token\" ",
                "data-intent-source-token=\"{}\"{} contenteditable=\"plaintext-only\" ",
                "spellcheck=\"false\">{}</span>"
            ),
            token.id.0,
            format!(" data-intent-node=\"{node}\""),
            escape_html(&line[token_start..token_end]),
        );
        cursor = token.end;
    }
    markup.push_str(&escape_html(&line[cursor - line_start..]));
}

const fn token_node(target: &IntentSourceTokenTarget) -> NodeId {
    match target {
        IntentSourceTokenTarget::NodeName { node }
        | IntentSourceTokenTarget::Suppressed { node }
        | IntentSourceTokenTarget::Definition { node, .. } => *node,
        IntentSourceTokenTarget::Instance { leaf } => leaf.node,
    }
}

pub(crate) fn history_markup(projection: &IntentWorkbenchProjection) -> String {
    let mut markup = String::new();
    if !projection.history.applied.is_empty() {
        markup.push_str("<h3>Applied</h3><ol class=\"wb-intent-history-list\">");
        for entry in &projection.history.applied {
            push_history_row(&mut markup, entry, false);
        }
        markup.push_str("</ol>");
    }
    if !projection.history.redoable.is_empty() {
        markup.push_str("<h3>Redoable</h3><ol class=\"wb-intent-history-list redoable\">");
        for entry in &projection.history.redoable {
            push_history_row(&mut markup, entry, true);
        }
        markup.push_str("</ol>");
    }
    markup
}

fn push_history_row(
    markup: &mut String,
    entry: &geosolve_sketch_intent::IntentTransactionDescriptor,
    redoable: bool,
) {
    let operations = entry
        .operation_kinds
        .iter()
        .map(|kind| operation_label(*kind))
        .collect::<Vec<_>>()
        .join(" + ");
    let _ = write!(
        markup,
        concat!(
            "<li data-intent-history-revision=\"{}\" data-intent-history-state=\"{}\">",
            "<strong>{}</strong><span>{}</span><small>{} declaration{}</small></li>"
        ),
        entry.target_revision,
        if redoable { "redoable" } else { "applied" },
        escape_html(disposition_label(entry.disposition)),
        escape_html(&operations),
        entry.affected_nodes.len(),
        if entry.affected_nodes.len() == 1 {
            ""
        } else {
            "s"
        },
    );
}

pub(crate) fn inspector_markup(inspector: Option<&IntentInspectorProjection>) -> String {
    let Some(inspector) = inspector else {
        return String::new();
    };
    let mut markup = format!(
        concat!(
            "<section class=\"wb-intent-inspector\" data-intent-inspector-node=\"{}\" ",
            "data-intent-state=\"{}\"><h3>{}</h3><p>{}</p>",
            "<label>Display name<input type=\"text\" data-intent-edit=\"name\" ",
            "data-intent-node=\"{}\" value=\"{}\"></label>",
            "<label class=\"wb-option-check\"><input type=\"checkbox\" ",
            "data-intent-edit=\"suppressed\" data-intent-node=\"{}\"{}> Suppressed</label>"
        ),
        inspector.node,
        if inspector.retained_failure {
            "invalid"
        } else {
            "current"
        },
        escape_html(inspector.name.as_str()),
        escape_html(&humanize_debug(&inspector.kind)),
        inspector.node,
        escape_attribute(inspector.name.as_str()),
        inspector.node,
        if inspector.suppressed { " checked" } else { "" },
    );
    for field in &inspector.fields {
        match field {
            IntentInspectorField::Definition { schema, value } => {
                let identity = format!(
                    "data-intent-node=\"{}\" data-intent-field=\"{}\"",
                    inspector.node,
                    escape_attribute(schema.field.0.as_str()),
                );
                push_optional_literal_editor(
                    &mut markup,
                    schema.field.0.as_str(),
                    "definition",
                    &identity,
                    schema.literal,
                    value.as_ref(),
                );
            }
            IntentInspectorField::Instance {
                leaf,
                port_kind,
                value,
            } => {
                let identity = format!(
                    concat!(
                        "data-intent-node=\"{}\" data-intent-port=\"{}\" ",
                        "data-intent-leaf=\"{}\" data-intent-port-kind=\"{:?}\""
                    ),
                    leaf.node,
                    leaf.port,
                    leaf_field_label(leaf.field),
                    port_kind,
                );
                push_optional_literal_editor(
                    &mut markup,
                    leaf_field_label(leaf.field),
                    "instance",
                    &identity,
                    literal_schema_for_leaf(leaf.field),
                    value.as_ref(),
                );
            }
        }
    }
    markup.push_str("</section>");
    markup
}

fn push_optional_literal_editor(
    markup: &mut String,
    label: &str,
    owner: &str,
    identity: &str,
    schema: IntentLiteralSchema,
    literal: Option<&IntentLiteral>,
) {
    let Some(literal) = literal else {
        let _ = write!(
            markup,
            "<label>{}<input type=\"text\" data-intent-edit=\"{}\" {} data-intent-literal=\"{:?}\" value=\"\" placeholder=\"Not set\"></label>",
            escape_html(label),
            owner,
            identity,
            schema,
        );
        return;
    };
    match literal {
        IntentLiteral::Quantity { value, unit } => {
            let _ = write!(
                markup,
                "<label>{}<input type=\"number\" step=\"any\" data-intent-edit=\"{}\" {} data-intent-unit=\"{:?}\" value=\"{}\"></label>",
                escape_html(label),
                owner,
                identity,
                unit,
                value,
            );
        }
        IntentLiteral::Boolean(value) => {
            let _ = write!(
                markup,
                "<label class=\"wb-option-check\"><input type=\"checkbox\" data-intent-edit=\"{}\" {}{}> {}</label>",
                owner,
                identity,
                if *value { " checked" } else { "" },
                escape_html(label),
            );
        }
        IntentLiteral::Enum(value) | IntentLiteral::Text(value) => {
            let _ = write!(
                markup,
                "<label>{}<input type=\"text\" data-intent-edit=\"{}\" {} value=\"{}\"></label>",
                escape_html(label),
                owner,
                identity,
                escape_attribute(value.as_str()),
            );
        }
        IntentLiteral::Integer(value) => {
            push_integer_editor(markup, owner, identity, label, *value);
        }
        IntentLiteral::Natural(value) => {
            push_integer_editor(markup, owner, identity, label, *value);
        }
        IntentLiteral::Point([x, y]) => {
            let _ = write!(
                markup,
                concat!(
                    "<fieldset><legend>{}</legend><input type=\"number\" step=\"any\" ",
                    "data-intent-edit=\"{}\" data-intent-component=\"x\" {} value=\"{}\">",
                    "<input type=\"number\" step=\"any\" data-intent-edit=\"{}\" ",
                    "data-intent-component=\"y\" {} value=\"{}\"></fieldset>"
                ),
                escape_html(label),
                owner,
                identity,
                x,
                owner,
                identity,
                y,
            );
        }
    }
}

fn push_integer_editor<T: std::fmt::Display>(
    markup: &mut String,
    owner: &str,
    identity: &str,
    label: &str,
    value: T,
) {
    let _ = write!(
        markup,
        "<label>{}<input type=\"number\" step=\"1\" data-intent-edit=\"{}\" {} value=\"{}\"></label>",
        escape_html(label),
        owner,
        identity,
        value,
    );
}

const fn literal_schema_for_leaf(field: LeafField) -> IntentLiteralSchema {
    use geosolve_sketch_intent::IntentUnit;
    match field {
        LeafField::X | LeafField::Y => IntentLiteralSchema::Quantity(IntentUnit::Length),
        LeafField::Value => IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
        LeafField::Angle => IntentLiteralSchema::Quantity(IntentUnit::Angle),
        LeafField::Weight | LeafField::Parameter => {
            IntentLiteralSchema::Quantity(IntentUnit::Dimensionless)
        }
    }
}

fn node_family_label(kind: &IntentNodeKind) -> &'static str {
    match kind {
        IntentNodeKind::Geometry { .. } => "Geometry",
        IntentNodeKind::Constraint { .. } => "Relation",
        IntentNodeKind::Dimension { .. } => "Dimension",
        IntentNodeKind::Operation { .. } => "Operation",
        IntentNodeKind::ComputedFeature { .. } => "Computed",
        IntentNodeKind::Aggregate { .. } => "Aggregate",
        IntentNodeKind::Parameter { .. } => "Parameter",
        IntentNodeKind::External { .. } => "External",
        IntentNodeKind::Bootstrap { .. } => "Imported",
        IntentNodeKind::Annotation => "Annotation",
        IntentNodeKind::Identity { .. } => "Identity",
    }
}

fn operation_label(kind: IntentPatchOperationKind) -> &'static str {
    match kind {
        IntentPatchOperationKind::CreateNode => "Create",
        IntentPatchOperationKind::DeleteNode => "Delete",
        IntentPatchOperationKind::SetSuppressed => "Suppress",
        IntentPatchOperationKind::SetDefinitionField => "Edit definition",
        IntentPatchOperationKind::SetInstanceLeaf => "Move / edit value",
        IntentPatchOperationKind::RebindInput => "Reconnect",
        IntentPatchOperationKind::RenameNode => "Rename",
        IntentPatchOperationKind::MoveDeclaration => "Organize",
        IntentPatchOperationKind::CreateCell => "Create cell",
        IntentPatchOperationKind::DeleteCell => "Delete cell",
        IntentPatchOperationKind::ReorderCells => "Reorder cells",
        IntentPatchOperationKind::ReplaceExternalInputs => "Update host inputs",
    }
}

const fn disposition_label(disposition: IntentPlanDisposition) -> &'static str {
    match disposition {
        IntentPlanDisposition::Accepted => "Accepted",
        IntentPlanDisposition::RetainedFailed => "Retained invalid intent",
        IntentPlanDisposition::OrganizationOnly => "Organization only",
    }
}

const fn leaf_field_label(field: LeafField) -> &'static str {
    match field {
        LeafField::X => "x",
        LeafField::Y => "y",
        LeafField::Value => "value",
        LeafField::Angle => "angle",
        LeafField::Weight => "weight",
        LeafField::Parameter => "parameter",
    }
}

fn humanize_debug(value: &impl std::fmt::Debug) -> String {
    let debug = format!("{value:?}");
    let mut output = String::with_capacity(debug.len() + 4);
    let mut previous_lower = false;
    for character in debug.chars() {
        if character.is_ascii_uppercase() && previous_lower {
            output.push(' ');
        }
        output.push(character);
        previous_lower = character.is_ascii_lowercase() || character.is_ascii_digit();
    }
    output
}

fn escape_attribute(value: &str) -> String {
    escape_html(value).replace('`', "&#96;")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::IntentWorkbenchProjection;
    use geosolve_sketch_intent::{
        GeometryRecipeKind, IntentEvaluation, IntentKey, IntentLiteral, IntentNodeDraft,
        IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortRole,
        IntentPortSelector, IntentSession, IntentSessionId, IntentUnit, LeafField,
        MaterializationEvidence,
    };

    use super::{
        DesignProjectionSelection, declaration_count, history_markup, inspector_markup,
        outline_markup, structured_source_markup,
    };

    fn key(value: &str) -> IntentKey {
        IntentKey::new(value).unwrap()
    }

    fn fixture() -> (
        IntentSession,
        IntentWorkbenchProjection,
        geosolve_sketch_intent::NodeId,
    ) {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_5001)).unwrap();
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            key("point_1"),
        )
        .with_display_name(key("Point 1"))
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
            LeafField::X,
            IntentLiteral::Quantity {
                value: 1.25,
                unit: IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
            LeafField::Y,
            IntentLiteral::Quantity {
                value: -2.5,
                unit: IntentUnit::Length,
            },
        );
        let plan = session
            .plan_patch(
                IntentPatch::new(
                    session.identity(),
                    IntentPatchPolicy::RequireAccepted,
                    vec![IntentPatchOperation::CreateNode {
                        alias: key("point"),
                        draft: Box::new(draft),
                        cell: None,
                    }],
                ),
                |candidate| IntentEvaluation::Accepted {
                    evidence: MaterializationEvidence::new_host_artifacts(
                        candidate.external_inputs().identity(),
                        b"accepted".to_vec(),
                        b"ownership".to_vec(),
                        b"validation".to_vec(),
                    )
                    .unwrap(),
                },
            )
            .unwrap();
        let node = plan.aliases().node(&key("point")).unwrap();
        session.commit_plan(plan).unwrap();
        let projection = IntentWorkbenchProjection::from_session(&session);
        (session, projection, node)
    }

    #[test]
    fn all_projections_share_editor_owned_stable_coordinates() {
        let (session, projection, node) = fixture();
        let selection = Some(DesignProjectionSelection { node });
        let outline = outline_markup(&projection, selection);
        let source = structured_source_markup(&projection, selection);
        let history = history_markup(&projection);
        let inspector = projection.inspector(&session, node);
        let inspector = inspector_markup(inspector.as_ref());
        assert_eq!(declaration_count(&projection), 1);
        assert!(outline.contains(&format!("data-intent-node=\"{node}\"")));
        assert!(outline.contains("aria-selected=\"true\""));
        assert!(source.contains("import { design } from &quot;@geosolve/intent&quot;;"));
        assert!(source.contains("data-intent-source-token=\"0\""));
        assert!(source.contains("class=\"wb-intent-source-line selected\""));
        assert!(history.contains("Accepted"));
        assert!(history.contains("Create"));
        assert!(!history.contains("data-wb-action"));
        assert!(inspector.contains("data-intent-edit=\"name\""));
        assert_eq!(
            inspector.matches("data-intent-edit=\"instance\"").count(),
            2
        );
        assert!(!inspector.contains("onclick="));
    }

    #[test]
    fn source_markup_is_deterministic_and_never_executable_input() {
        let (_, projection, _) = fixture();
        let first = structured_source_markup(&projection, None);
        assert_eq!(first, structured_source_markup(&projection, None));
        assert!(first.contains("export const sketch = design"));
        assert!(first.contains("contenteditable=\"plaintext-only\""));
        assert!(!first.contains("eval("));
        assert!(!first.contains("new Function"));
    }
}
