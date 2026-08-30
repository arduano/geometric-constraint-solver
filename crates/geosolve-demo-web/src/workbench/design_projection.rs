// SPDX-License-Identifier: GPL-3.0-or-later

//! Browser markup for the editor-owned projectional Design Intent DTOs.
//!
//! The browser deliberately does not inspect an [`IntentSession`] or generate
//! a second source representation. Stable selection, Inspector coordinates,
//! recognized source tokens and read-only History all come from
//! `geosolve-constraint-editor`'s equation-free projection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use geosolve_constraint_editor::{
    IntentGraphNodeKind, IntentInspectorEditTarget, IntentInspectorField,
    IntentInspectorProjection, IntentOutlineDeclaration, IntentSourceToken,
    IntentSourceTokenTarget, IntentWorkbenchProjection,
};
use geosolve_sketch_intent::{
    IntentDefinitionFieldDescriptor, IntentFieldChoices, IntentFieldDefault, IntentFieldKey,
    IntentKey, IntentLiteral, IntentLiteralSchema, IntentOutputDescriptor,
    IntentPatchOperationKind, IntentPlanDisposition, IntentProjectionPath,
    IntentProjectionPathSegment, IntentSessionIdentity, IntentUnit, LeafField, LeafRef, NodeId,
    OperationKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DesignProjectionSelection {
    pub node: NodeId,
}

/// Explicit authority shown beside a code-owned Inspector parameter. These
/// rows are presentation metadata only: the mutation adapter independently
/// re-resolves the same target against a fresh managed-control manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InspectorParameterAuthority {
    ModifiableSource {
        source_path: String,
        source_text: String,
        consumer_count: usize,
        generated_consumer_count: usize,
    },
    ModifiableInstance,
    Encoded {
        reason: String,
    },
    Blocked {
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InspectorParameterPresentation {
    pub target: IntentInspectorEditTarget,
    pub authority: InspectorParameterAuthority,
}

#[derive(Clone, Copy)]
pub(crate) struct InspectorPresentation<'a> {
    pub parameters: &'a [InspectorParameterPresentation],
    pub name_authority: Option<&'a InspectorParameterAuthority>,
}

/// One bounded target-to-schema index shared by managed-parameter authority
/// derivation and Inspector markup. The central descriptor remains the source
/// of truth; this view only prevents repeated linear scans of it for every
/// projected field.
pub(crate) struct InspectorDescriptorIndex<'a> {
    definitions: BTreeMap<IntentFieldKey, &'a IntentDefinitionFieldDescriptor>,
    instances: BTreeMap<LeafRef, (&'a IntentOutputDescriptor, IntentProjectionPath)>,
}

impl<'a> InspectorDescriptorIndex<'a> {
    pub(crate) fn new(inspector: &'a IntentInspectorProjection) -> Self {
        let mut definitions = BTreeMap::new();
        for descriptor in &inspector.descriptor.fields {
            let replaced = definitions.insert(descriptor.schema.field.clone(), descriptor);
            assert!(
                replaced.is_none(),
                "central Inspector definition descriptors must be unique"
            );
        }

        let mut instances = BTreeMap::new();
        for output in &inspector.descriptor.outputs {
            for field in &output.writable {
                let leaf = LeafRef {
                    node: output.port.node,
                    port: output.port.port,
                    field: *field,
                };
                let path = output
                    .path_for_leaf(leaf)
                    .expect("descriptor output owns its declared writable leaf");
                let replaced = instances.insert(leaf, (output, path));
                assert!(
                    replaced.is_none(),
                    "central Inspector writable-leaf descriptors must be unique"
                );
            }
        }
        Self {
            definitions,
            instances,
        }
    }

    pub(crate) fn definition(
        &self,
        field: &IntentFieldKey,
    ) -> Option<&'a IntentDefinitionFieldDescriptor> {
        self.definitions.get(field).copied()
    }

    pub(crate) fn instance(
        &self,
        leaf: LeafRef,
    ) -> Option<(&'a IntentOutputDescriptor, &IntentProjectionPath)> {
        self.instances
            .get(&leaf)
            .map(|(output, path)| (*output, path))
    }
}

pub(crate) fn declaration_count(projection: &IntentWorkbenchProjection) -> usize {
    let hidden = grouped_outline_helper_nodes(projection);
    projection
        .outline
        .iter()
        .map(|cell| {
            cell.declarations
                .iter()
                .filter(|declaration| !hidden.contains(&declaration.node))
                .count()
        })
        .sum()
}

pub(crate) fn outline_markup(
    projection: &IntentWorkbenchProjection,
    selection: Option<DesignProjectionSelection>,
) -> String {
    let mut markup = String::new();
    if let Some(diagnostic) = &projection.latest_diagnostic {
        let _ = write!(
            markup,
            concat!(
                "<div class=\"wb-intent-diagnostic\" role=\"status\">",
                "<strong>Retained intent needs attention</strong><span>{}</span></div>"
            ),
            escape_html(diagnostic.as_str()),
        );
    }
    let hidden = grouped_outline_helper_nodes(projection);
    for (cell_index, cell) in projection.outline.iter().enumerate() {
        let declarations = cell
            .declarations
            .iter()
            .filter(|declaration| !hidden.contains(&declaration.node))
            .collect::<Vec<_>>();
        let _ = write!(
            markup,
            concat!(
                "<section class=\"wb-intent-cell\" data-intent-cell=\"{}\" data-intent-drop-cell=\"{}\">",
                "<header class=\"wb-intent-cell-header\" draggable=\"true\" ",
                "data-intent-cell-drag=\"{}\" data-intent-cell-drop-before=\"{}\">",
                "<div><h3>{}</h3><span>{} declaration{}</span></div>",
                "<span class=\"wb-intent-cell-actions\" aria-label=\"Reorder cell\">",
                "<button type=\"button\" data-intent-cell-move=\"up\" data-intent-cell=\"{}\"{} aria-label=\"Move cell up\">↑</button>",
                "<button type=\"button\" data-intent-cell-move=\"down\" data-intent-cell=\"{}\"{} aria-label=\"Move cell down\">↓</button>",
                "</span></header>"
            ),
            cell.cell,
            cell.cell,
            cell.cell,
            cell.cell,
            escape_html(cell.name.as_str()),
            declarations.len(),
            if declarations.len() == 1 { "" } else { "s" },
            cell.cell,
            if cell_index > 0 { "" } else { " disabled" },
            cell.cell,
            if cell_index + 1 < projection.outline.len() {
                ""
            } else {
                " disabled"
            },
        );
        for (index, declaration) in declarations.iter().enumerate() {
            push_declaration_button(
                &mut markup,
                declaration,
                cell.cell,
                index > 0,
                index + 1 < declarations.len(),
                selection,
            );
        }
        markup.push_str("</section>");
    }
    if projection.outline.len() > 1 {
        markup.push_str(
            "<div class=\"wb-intent-cell-drop-end\" data-intent-cell-drop-end=\"true\" aria-label=\"Move cell to end\"></div>",
        );
    }
    markup
}

/// Profile Offset authoring creates equation-free Profile/OpenChain operands
/// in the same atomic patch as the user-facing operation. A helper with one
/// exact Profile Offset consumer is presentation-owned by that operation and
/// is grouped out of Outline. Reusable/shared aggregates remain visible.
pub(crate) fn grouped_outline_helper_nodes(
    projection: &IntentWorkbenchProjection,
) -> BTreeSet<NodeId> {
    let declarations = projection
        .outline
        .iter()
        .flat_map(|cell| &cell.declarations)
        .map(|declaration| (declaration.node, declaration))
        .collect::<BTreeMap<_, _>>();
    let consumer_count = declarations
        .values()
        .flat_map(|declaration| declaration.dependencies.iter().copied())
        .fold(
            BTreeMap::<NodeId, usize>::new(),
            |mut counts, dependency| {
                *counts.entry(dependency).or_default() += 1;
                counts
            },
        );
    declarations
        .values()
        .filter(|declaration| {
            matches!(
                &declaration.kind,
                IntentGraphNodeKind::Operation {
                    operation: OperationKind::ProfileOffset
                }
            )
        })
        .flat_map(|operation| operation.dependencies.iter().copied())
        .filter(|dependency| {
            consumer_count.get(dependency) == Some(&1)
                && declarations.get(dependency).is_some_and(|declaration| {
                    matches!(&declaration.kind, IntentGraphNodeKind::Aggregate { .. })
                })
        })
        .collect()
}

fn push_declaration_button(
    markup: &mut String,
    declaration: &IntentOutlineDeclaration,
    cell: geosolve_sketch_intent::CellId,
    can_move_up: bool,
    can_move_down: bool,
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
            "<div class=\"wb-intent-row-wrap\" data-intent-node=\"{}\" data-intent-drop-before=\"{}\" data-intent-cell=\"{}\">",
            "<button type=\"button\" class=\"wb-intent-row{}\" ",
            "id=\"wb-intent-node-{}\" data-intent-node=\"{}\" ",
            "data-intent-cell=\"{}\" ",
            "data-intent-state=\"{}\" draggable=\"true\" role=\"treeitem\" ",
            "aria-selected=\"{}\"><span class=\"wb-intent-kind\">{}</span>",
            "<span class=\"wb-intent-name\">{}</span><small>{}</small></button>",
            "<span class=\"wb-intent-row-actions\" aria-label=\"Reorder declaration\">",
            "<button type=\"button\" data-intent-move=\"up\" data-intent-node=\"{}\"{} aria-label=\"Move declaration up\">↑</button>",
            "<button type=\"button\" data-intent-move=\"down\" data-intent-node=\"{}\"{} aria-label=\"Move declaration down\">↓</button>",
            "</span></div>"
        ),
        declaration.node,
        declaration.node,
        cell,
        if selected { " selected" } else { "" },
        declaration.node,
        declaration.node,
        cell,
        state,
        selected,
        escape_html(node_family_label(&declaration.kind)),
        escape_html(declaration.name.as_str()),
        escape_html(&humanize_debug(&declaration.kind)),
        declaration.node,
        if can_move_up { "" } else { " disabled" },
        declaration.node,
        if can_move_down { "" } else { " disabled" },
    );
}

/// Renders the exact editor-owned structured source, wrapping only recognized
/// token ranges with stable typed edit coordinates. The source is displayed as
/// code and is never evaluated by the browser. Grouped presentation-owned
/// helper declarations retain recognized typed token edits without exposing a
/// second selection or reorder target.
pub(crate) fn structured_source_markup(
    projection: &IntentWorkbenchProjection,
    selection: Option<DesignProjectionSelection>,
) -> String {
    let source = &projection.structured_source;
    let grouped_helpers = grouped_outline_helper_nodes(projection);
    let mut node_cells = BTreeMap::new();
    for cell in &projection.outline {
        for declaration in &cell.declarations {
            node_cells.entry(declaration.node).or_insert(cell.cell);
        }
    }
    debug_assert!(
        source
            .tokens
            .windows(2)
            .all(|tokens| tokens[0].end <= tokens[1].start),
        "editor-owned structured source tokens must remain ordered and non-overlapping"
    );
    let mut markup = String::new();
    let mut line_start = 0;
    let mut token_cursor = 0;
    for (line_number, inclusive) in source.text.split_inclusive('\n').enumerate() {
        let line = inclusive.strip_suffix('\n').unwrap_or(inclusive);
        let line_end = line_start + line.len();
        let line_token_start = token_cursor;
        while source
            .tokens
            .get(token_cursor)
            .is_some_and(|token| token.start <= line_end)
        {
            token_cursor += 1;
        }
        let line_tokens = &source.tokens[line_token_start..token_cursor];
        debug_assert!(
            line_tokens
                .iter()
                .all(|token| token.start >= line_start && token.end <= line_end)
        );
        let selected = line_tokens.iter().any(|token| {
            let node = token_node(&token.target);
            !grouped_helpers.contains(&node) && selection.is_some_and(|value| value.node == node)
        });
        let node = line_tokens
            .iter()
            .map(|token| token_node(&token.target))
            .find(|node| !grouped_helpers.contains(node));
        let organization =
            node.and_then(|node| node_cells.get(&node).copied().map(|cell| (node, cell)));
        let _ = write!(
            markup,
            "<div class=\"wb-intent-source-line{}\"{}><span>{}</span><code>",
            if selected { " selected" } else { "" },
            organization.map_or_else(String::new, |(node, cell)| {
                format!(
                    concat!(
                        " data-intent-node=\"{}\" data-intent-cell=\"{}\" ",
                        "data-intent-drop-before=\"{}\" draggable=\"true\""
                    ),
                    node, cell, node,
                )
            }),
            line_number + 1,
        );
        push_source_line(
            &mut markup,
            line,
            line_start,
            line_tokens,
            &grouped_helpers,
            source.identity,
        );
        markup.push_str("</code></div>");
        line_start += inclusive.len();
    }
    debug_assert_eq!(token_cursor, source.tokens.len());
    markup
}

fn push_source_line(
    markup: &mut String,
    line: &str,
    line_start: usize,
    tokens: &[IntentSourceToken],
    non_interactive_nodes: &BTreeSet<NodeId>,
    identity: IntentSessionIdentity,
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
                "data-intent-source-token=\"{}\" data-intent-session=\"{}\" ",
                "data-intent-revision=\"{}\" data-intent-digest=\"{}\"{} ",
                "contenteditable=\"plaintext-only\" ",
                "spellcheck=\"false\">{}</span>"
            ),
            token.id.0,
            identity.session,
            identity.revision,
            identity.digest,
            if non_interactive_nodes.contains(&node) {
                String::new()
            } else {
                format!(" data-intent-node=\"{node}\"")
            },
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
    inspector_markup_with_presentation(
        inspector,
        InspectorPresentation {
            parameters: &[],
            name_authority: None,
        },
    )
}

pub(crate) fn inspector_markup_with_parameters(
    inspector: Option<&IntentInspectorProjection>,
    parameters: &[InspectorParameterPresentation],
) -> String {
    let name_authority = inspector_name_authority(parameters);
    inspector_markup_with_presentation(
        inspector,
        InspectorPresentation {
            parameters,
            name_authority: name_authority.as_ref(),
        },
    )
}

pub(crate) fn inspector_name_authority(
    parameters: &[InspectorParameterPresentation],
) -> Option<InspectorParameterAuthority> {
    if parameters.is_empty() {
        return None;
    }
    if let Some(reason) = parameters.iter().find_map(|parameter| {
        if let InspectorParameterAuthority::Blocked { reason } = &parameter.authority {
            Some(reason.clone())
        } else {
            None
        }
    }) {
        return Some(InspectorParameterAuthority::Blocked { reason });
    }
    Some(InspectorParameterAuthority::Encoded {
        reason: "Code-owned identity · not declared as an editable sketch.ts value".into(),
    })
}

pub(crate) fn inspector_markup_with_presentation(
    inspector: Option<&IntentInspectorProjection>,
    presentation: InspectorPresentation<'_>,
) -> String {
    let Some(inspector) = inspector else {
        return String::new();
    };
    let descriptors = InspectorDescriptorIndex::new(inspector);
    inspector_markup_with_presentation_and_descriptors(inspector, presentation, &descriptors)
}

pub(crate) fn inspector_markup_with_presentation_and_descriptors(
    inspector: &IntentInspectorProjection,
    presentation: InspectorPresentation<'_>,
    descriptors: &InspectorDescriptorIndex<'_>,
) -> String {
    let parameter_index = InspectorParameterIndex::new(presentation.parameters);
    let identity = inspector.identity;
    let (name_interaction, name_metadata) = match presentation.name_authority {
        Some(InspectorParameterAuthority::Encoded { reason }) => (
            "data-intent-encoded=\"true\" disabled aria-readonly=\"true\"".to_owned(),
            format!(
                "<small class=\"wb-intent-parameter-authority\" data-intent-name-authority=\"encoded\"><strong>Encoded</strong><span>{}</span></small>",
                escape_html(reason),
            ),
        ),
        Some(InspectorParameterAuthority::Blocked { reason }) => (
            "data-intent-blocked=\"true\" disabled aria-readonly=\"true\"".to_owned(),
            format!(
                "<small class=\"wb-intent-parameter-authority\" data-intent-name-authority=\"blocked\"><strong>Blocked</strong><span>{}</span></small>",
                escape_html(reason),
            ),
        ),
        Some(
            InspectorParameterAuthority::ModifiableSource { .. }
            | InspectorParameterAuthority::ModifiableInstance,
        )
        | None => ("data-intent-edit=\"name\"".to_owned(), String::new()),
    };
    let mut markup = format!(
        concat!(
            "<section class=\"wb-intent-inspector\" data-intent-inspector-node=\"{}\" ",
            "data-intent-session=\"{}\" data-intent-revision=\"{}\" ",
            "data-intent-digest=\"{}\" ",
            "data-intent-state=\"{}\"><h3>{}</h3><p>{}</p>",
            "<label>Display name<input type=\"text\" {} ",
            "data-intent-node=\"{}\" value=\"{}\">{}</label>",
            "{}"
        ),
        inspector.node,
        identity.session,
        identity.revision,
        identity.digest,
        if inspector.retained_failure {
            "invalid"
        } else {
            "current"
        },
        escape_html(inspector.name.as_str()),
        escape_html(&humanize_debug(&inspector.kind)),
        name_interaction,
        inspector.node,
        escape_attribute(inspector.name.as_str()),
        name_metadata,
        suppression_control(
            inspector,
            parameter_index.get(&IntentInspectorEditTarget::Suppressed)
        ),
    );
    push_inspector_inputs(&mut markup, inspector);
    push_inspector_fields(&mut markup, inspector, &parameter_index, descriptors);
    markup.push_str("</section>");
    markup
}

struct InspectorParameterIndex<'a> {
    suppressed: Option<&'a InspectorParameterPresentation>,
    definitions: BTreeMap<&'a IntentFieldKey, &'a InspectorParameterPresentation>,
    instances: BTreeMap<LeafRef, &'a InspectorParameterPresentation>,
}

impl<'a> InspectorParameterIndex<'a> {
    fn new(parameters: &'a [InspectorParameterPresentation]) -> Self {
        let mut index = Self {
            suppressed: None,
            definitions: BTreeMap::new(),
            instances: BTreeMap::new(),
        };
        for parameter in parameters {
            let replaced = match &parameter.target {
                IntentInspectorEditTarget::Suppressed => index.suppressed.replace(parameter),
                IntentInspectorEditTarget::Definition { field } => {
                    index.definitions.insert(field, parameter)
                }
                IntentInspectorEditTarget::Instance { leaf } => {
                    index.instances.insert(*leaf, parameter)
                }
            };
            assert!(
                replaced.is_none(),
                "Inspector parameter targets must be unique"
            );
        }
        index
    }

    fn get(
        &self,
        target: &IntentInspectorEditTarget,
    ) -> Option<&'a InspectorParameterPresentation> {
        match target {
            IntentInspectorEditTarget::Suppressed => self.suppressed,
            IntentInspectorEditTarget::Definition { field } => self.definitions.get(field).copied(),
            IntentInspectorEditTarget::Instance { leaf } => self.instances.get(leaf).copied(),
        }
    }
}

fn suppression_control(
    inspector: &IntentInspectorProjection,
    parameter: Option<&InspectorParameterPresentation>,
) -> String {
    let mut markup = String::new();
    if parameter.is_some() {
        markup.push_str("<div class=\"wb-intent-parameter\">");
    }
    let edit = match parameter.map(|parameter| &parameter.authority) {
        Some(InspectorParameterAuthority::Encoded { .. }) => {
            "data-intent-encoded=\"true\" disabled aria-readonly=\"true\""
        }
        Some(InspectorParameterAuthority::Blocked { .. }) => {
            "data-intent-blocked=\"true\" disabled aria-readonly=\"true\""
        }
        Some(
            InspectorParameterAuthority::ModifiableSource { .. }
            | InspectorParameterAuthority::ModifiableInstance,
        )
        | None => "data-intent-edit=\"suppressed\"",
    };
    let _ = write!(
        markup,
        "<label class=\"wb-option-check\"><input type=\"checkbox\" {edit} data-intent-node=\"{}\" data-intent-schema=\"boolean\"{}> Suppressed</label>",
        inspector.node,
        if inspector.suppressed { " checked" } else { "" },
    );
    if let Some(parameter) = parameter {
        push_parameter_authority(&mut markup, &parameter.authority);
        markup.push_str("</div>");
    }
    markup
}

fn push_inspector_inputs(markup: &mut String, inspector: &IntentInspectorProjection) {
    if inspector.inputs.is_empty() {
        return;
    }
    let mut tree = InspectorMarkupTree::object();
    for input in &inspector.inputs {
        let input_path = projection_path_code(&input.path);
        let output_path = projection_path_code(&input.source.output);
        let label = projection_path_terminal_label(&input.path);
        let mut chip = String::new();
        let _ = write!(
            chip,
            concat!(
                "<div class=\"wb-intent-input\" data-intent-input-path=\"{}\" ",
                "data-intent-source-declaration=\"{}\" data-intent-source-output=\"{}\" ",
                "data-intent-source-kind=\"{:?}\"><span>{}</span>",
                "<code>{} → {} · {:?}</code></div>"
            ),
            escape_attribute(&input_path),
            escape_attribute(input.source.declaration.as_str()),
            escape_attribute(&output_path),
            input.source.kind,
            escape_html(&label),
            escape_html(input.source.declaration.as_str()),
            escape_html(&output_path),
            input.source.kind,
        );
        tree.insert(&input.path, chip);
    }
    markup.push_str("<fieldset class=\"wb-intent-inputs\"><legend>Inputs</legend>");
    push_inspector_tree(markup, &tree, &mut Vec::new(), None);
    markup.push_str("</fieldset>");
}

fn push_inspector_fields(
    markup: &mut String,
    inspector: &IntentInspectorProjection,
    parameters: &InspectorParameterIndex<'_>,
    descriptors: &InspectorDescriptorIndex<'_>,
) {
    let mut definitions = InspectorMarkupTree::object();
    let mut instances = InspectorMarkupTree::object();
    for field in &inspector.fields {
        match field {
            IntentInspectorField::Definition { definition, value } => {
                let descriptor = descriptors
                    .definition(definition)
                    .expect("projected Inspector field has central descriptor");
                let schema = &descriptor.schema;
                let identity = format!(
                    "data-intent-node=\"{}\" data-intent-field=\"{}\"",
                    inspector.node,
                    escape_attribute(schema.field.0.as_str()),
                );
                let mut control = String::new();
                push_optional_literal_editor(
                    &mut control,
                    &projection_path_leaf_label(&descriptor.path),
                    "definition",
                    &identity,
                    schema.literal,
                    Some(&descriptor.choices),
                    Some(&descriptor.default),
                    value.as_ref(),
                    parameters.get(&IntentInspectorEditTarget::Definition {
                        field: definition.clone(),
                    }),
                );
                definitions.insert(&descriptor.path, control);
            }
            IntentInspectorField::Instance { leaf, value } => {
                let (output, path) = descriptors
                    .instance(*leaf)
                    .expect("projected Inspector leaf has central output descriptor");
                let port_kind = output.kind;
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
                let mut control = String::new();
                push_optional_literal_editor(
                    &mut control,
                    &projection_path_leaf_label(path),
                    "instance",
                    &identity,
                    literal_schema_for_instance(leaf.field, value.as_ref()),
                    None,
                    None,
                    value.as_ref(),
                    parameters.get(&IntentInspectorEditTarget::Instance { leaf: *leaf }),
                );
                instances.insert(path, control);
            }
        }
    }
    push_inspector_tree_section(markup, "Definition", "definition", &definitions);
    push_inspector_tree_section(markup, "Instance", "instance", &instances);
}

enum InspectorMarkupTree {
    Object(InspectorMarkupObject),
    Array(BTreeMap<u16, Self>),
    Leaf(String),
}

struct InspectorMarkupObject {
    values: Vec<(IntentKey, InspectorMarkupTree)>,
    positions: BTreeMap<IntentKey, usize>,
}

impl InspectorMarkupTree {
    fn object() -> Self {
        Self::Object(InspectorMarkupObject {
            values: Vec::new(),
            positions: BTreeMap::new(),
        })
    }

    fn is_empty(&self) -> bool {
        matches!(self, Self::Object(object) if object.values.is_empty())
    }

    fn insert(&mut self, path: &IntentProjectionPath, markup: String) {
        self.insert_segments(path.segments(), markup);
    }

    fn insert_segments(&mut self, segments: &[IntentProjectionPathSegment], markup: String) {
        let (head, tail) = segments
            .split_first()
            .expect("validated Inspector path is non-empty");
        match (self, head) {
            (Self::Object(object), IntentProjectionPathSegment::Field(field)) => {
                let position = object.positions.get(field).copied();
                if tail.is_empty() {
                    assert!(position.is_none(), "Inspector paths must be unique");
                    let position = object.values.len();
                    object.values.push((field.clone(), Self::Leaf(markup)));
                    let replaced = object.positions.insert(field.clone(), position);
                    debug_assert!(replaced.is_none());
                } else if let Some(position) = position {
                    object.values[position].1.insert_segments(tail, markup);
                } else {
                    let mut next = match tail[0] {
                        IntentProjectionPathSegment::Field(_) => Self::object(),
                        IntentProjectionPathSegment::Index(_) => Self::Array(BTreeMap::new()),
                    };
                    next.insert_segments(tail, markup);
                    let position = object.values.len();
                    object.values.push((field.clone(), next));
                    let replaced = object.positions.insert(field.clone(), position);
                    debug_assert!(replaced.is_none());
                }
            }
            (Self::Array(values), IntentProjectionPathSegment::Index(index)) => {
                if tail.is_empty() {
                    let replaced = values.insert(*index, Self::Leaf(markup));
                    assert!(replaced.is_none(), "Inspector paths must be unique");
                } else {
                    let next = values.entry(*index).or_insert_with(|| match tail[0] {
                        IntentProjectionPathSegment::Field(_) => Self::object(),
                        IntentProjectionPathSegment::Index(_) => Self::Array(BTreeMap::new()),
                    });
                    next.insert_segments(tail, markup);
                }
            }
            _ => panic!("Inspector path changes container kind"),
        }
    }
}

fn push_inspector_tree_section(
    markup: &mut String,
    title: &str,
    owner: &str,
    tree: &InspectorMarkupTree,
) {
    if tree.is_empty() {
        return;
    }
    let _ = write!(
        markup,
        "<fieldset class=\"wb-intent-fields wb-intent-{}\"><legend>{}</legend>",
        owner,
        escape_html(title),
    );
    push_inspector_tree(markup, tree, &mut Vec::new(), None);
    markup.push_str("</fieldset>");
}

fn push_inspector_tree(
    markup: &mut String,
    tree: &InspectorMarkupTree,
    path: &mut Vec<IntentProjectionPathSegment>,
    array_name: Option<&str>,
) {
    match tree {
        InspectorMarkupTree::Leaf(control) => markup.push_str(control),
        InspectorMarkupTree::Object(object) => {
            for (field, value) in &object.values {
                path.push(IntentProjectionPathSegment::Field(field.clone()));
                if matches!(value, InspectorMarkupTree::Leaf(_)) {
                    push_inspector_tree(markup, value, path, None);
                } else {
                    let code = projection_path_segments_code(path);
                    let _ = write!(
                        markup,
                        concat!(
                            "<fieldset class=\"wb-intent-path-group\" ",
                            "data-intent-group-path=\"{}\"><legend>{}</legend>"
                        ),
                        escape_attribute(&code),
                        escape_html(&humanize_identifier(field.as_str())),
                    );
                    push_inspector_tree(markup, value, path, Some(field.as_str()));
                    markup.push_str("</fieldset>");
                }
                path.pop();
            }
        }
        InspectorMarkupTree::Array(values) => {
            let item = singular_projection_label(array_name.unwrap_or("item"));
            for (index, value) in values {
                path.push(IntentProjectionPathSegment::Index(*index));
                let code = projection_path_segments_code(path);
                let visible = u32::from(*index) + 1;
                let _ = write!(
                    markup,
                    concat!(
                        "<fieldset class=\"wb-intent-array-item\" data-intent-array-index=\"{}\" ",
                        "data-intent-group-path=\"{}\"><legend>{} {}</legend>"
                    ),
                    index,
                    escape_attribute(&code),
                    escape_html(&item),
                    visible,
                );
                push_inspector_tree(markup, value, path, None);
                markup.push_str("</fieldset>");
                path.pop();
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one closed literal-schema renderer keeps optional state and typed controls consistent across every Inspector literal family"
)]
fn push_optional_literal_editor(
    markup: &mut String,
    label: &str,
    owner: &str,
    identity: &str,
    schema: IntentLiteralSchema,
    choices: Option<&IntentFieldChoices>,
    default: Option<&IntentFieldDefault>,
    literal: Option<&IntentLiteral>,
    parameter: Option<&InspectorParameterPresentation>,
) {
    if parameter.is_some() {
        markup.push_str("<div class=\"wb-intent-parameter\">");
    }
    let interaction = match parameter.map(|parameter| &parameter.authority) {
        Some(InspectorParameterAuthority::Encoded { .. }) => {
            "data-intent-encoded=\"true\" disabled aria-readonly=\"true\"".to_owned()
        }
        Some(InspectorParameterAuthority::Blocked { .. }) => {
            "data-intent-blocked=\"true\" disabled aria-readonly=\"true\"".to_owned()
        }
        Some(
            InspectorParameterAuthority::ModifiableSource { .. }
            | InspectorParameterAuthority::ModifiableInstance,
        )
        | None => format!("data-intent-edit=\"{}\"", escape_attribute(owner)),
    };
    let schema_key = literal_schema_key(schema);
    match schema {
        IntentLiteralSchema::Quantity(unit) => {
            let value = match literal {
                Some(IntentLiteral::Quantity { value, .. }) => value.to_string(),
                _ => String::new(),
            };
            let _ = write!(
                markup,
                "<label>{}<input type=\"number\" step=\"any\" {} {} data-intent-schema=\"{}\" data-intent-unit=\"{}\" value=\"{}\" placeholder=\"Not set\"></label>",
                escape_html(label),
                interaction,
                identity,
                schema_key,
                intent_unit_key(unit),
                value,
            );
        }
        IntentLiteralSchema::Boolean => {
            let checked = matches!(literal, Some(IntentLiteral::Boolean(true)));
            let _ = write!(
                markup,
                "<label class=\"wb-option-check\"><input type=\"checkbox\" {} {} data-intent-schema=\"{}\"{}> {}</label>",
                interaction,
                identity,
                schema_key,
                if checked { " checked" } else { "" },
                escape_html(label),
            );
        }
        IntentLiteralSchema::Enum => {
            let value = match literal {
                Some(IntentLiteral::Enum(value) | IntentLiteral::Text(value)) => value.as_str(),
                _ => "",
            };
            if let Some(IntentFieldChoices::Closed(choices)) = choices {
                let placeholder = match default {
                    Some(IntentFieldDefault::Literal(IntentLiteral::Enum(value))) => {
                        format!("Default ({})", humanize_identifier(value.as_str()))
                    }
                    Some(IntentFieldDefault::Contextual) => "Automatic".to_owned(),
                    Some(IntentFieldDefault::Conditional) => "Not applicable".to_owned(),
                    Some(IntentFieldDefault::Required | IntentFieldDefault::Literal(_)) => {
                        "Choose…".to_owned()
                    }
                    None => "Not set".to_owned(),
                };
                let _ = write!(
                    markup,
                    "<label>{}<select {} {} data-intent-schema=\"{}\">",
                    escape_html(label),
                    interaction,
                    identity,
                    schema_key,
                );
                let _ = write!(
                    markup,
                    "<option value=\"\" disabled{}>{}</option>",
                    if value.is_empty() { " selected" } else { "" },
                    escape_html(&placeholder),
                );
                for choice in choices {
                    let _ = write!(
                        markup,
                        "<option value=\"{}\"{}>{}</option>",
                        escape_attribute(choice.as_str()),
                        if choice.as_str() == value {
                            " selected"
                        } else {
                            ""
                        },
                        escape_html(&humanize_identifier(choice.as_str())),
                    );
                }
                markup.push_str("</select></label>");
            } else {
                let _ = write!(
                    markup,
                    "<label>{}<input type=\"text\" {} {} data-intent-schema=\"{}\" value=\"{}\" placeholder=\"Not set\"></label>",
                    escape_html(label),
                    interaction,
                    identity,
                    schema_key,
                    escape_attribute(value),
                );
            }
        }
        IntentLiteralSchema::Text => {
            let value = match literal {
                Some(IntentLiteral::Text(value)) => value.as_str(),
                _ => "",
            };
            let _ = write!(
                markup,
                "<label>{}<input type=\"text\" {} {} data-intent-schema=\"{}\" value=\"{}\" placeholder=\"Not set\"></label>",
                escape_html(label),
                interaction,
                identity,
                schema_key,
                escape_attribute(value),
            );
        }
        IntentLiteralSchema::Integer => {
            let value = match literal {
                Some(IntentLiteral::Integer(value)) => value.to_string(),
                _ => String::new(),
            };
            push_integer_editor(markup, &interaction, identity, label, &schema_key, &value);
        }
        IntentLiteralSchema::Natural => {
            let value = match literal {
                Some(IntentLiteral::Natural(value)) => value.to_string(),
                _ => String::new(),
            };
            push_integer_editor(markup, &interaction, identity, label, &schema_key, &value);
        }
        IntentLiteralSchema::Point => {
            let [x, y] = match literal {
                Some(IntentLiteral::Point(point)) => point.map(|value| value.to_string()),
                _ => [String::new(), String::new()],
            };
            let _ = write!(
                markup,
                concat!(
                    "<fieldset><legend>{}</legend><input type=\"number\" step=\"any\" ",
                    "{} data-intent-component=\"x\" {} ",
                    "data-intent-schema=\"{}\" value=\"{}\" placeholder=\"x\">",
                    "<input type=\"number\" step=\"any\" {} ",
                    "data-intent-component=\"y\" {} data-intent-schema=\"{}\" ",
                    "value=\"{}\" placeholder=\"y\"></fieldset>"
                ),
                escape_html(label),
                interaction,
                identity,
                schema_key,
                x,
                interaction,
                identity,
                schema_key,
                y,
            );
        }
    }
    if let Some(parameter) = parameter {
        push_parameter_authority(markup, &parameter.authority);
        markup.push_str("</div>");
    }
}

fn push_integer_editor(
    markup: &mut String,
    interaction: &str,
    identity: &str,
    label: &str,
    schema_key: &str,
    value: &str,
) {
    let _ = write!(
        markup,
        "<label>{}<input type=\"number\" step=\"1\" {} {} data-intent-schema=\"{}\" value=\"{}\" placeholder=\"Not set\"></label>",
        escape_html(label),
        interaction,
        identity,
        schema_key,
        value,
    );
}

fn push_parameter_authority(markup: &mut String, authority: &InspectorParameterAuthority) {
    match authority {
        InspectorParameterAuthority::ModifiableSource {
            source_path,
            source_text,
            consumer_count,
            generated_consumer_count,
        } => {
            let direct_consumer_count = consumer_count.saturating_sub(*generated_consumer_count);
            let fan_out = match (*generated_consumer_count, direct_consumer_count) {
                (generated, 0) => format!(
                    "{} by {generated} generated consumer{}",
                    if generated > 1 { "shared" } else { "used" },
                    if generated == 1 { "" } else { "s" },
                ),
                (0, direct) => format!(
                    "{} by {direct} direct declaration{}",
                    if direct > 1 { "shared" } else { "used" },
                    if direct == 1 { "" } else { "s" },
                ),
                (generated, direct) => format!(
                    "shared by {consumer_count} consumers · {generated} generated · {direct} direct"
                ),
            };
            let _ = write!(
                markup,
                concat!(
                    "<aside class=\"wb-intent-parameter-authority\" ",
                    "data-intent-parameter-authority=\"modifiable-source\" ",
                    "data-intent-source-path=\"{}\" data-intent-source-consumers=\"{}\" ",
                    "data-intent-source-generated-consumers=\"{}\">",
                    "<strong>Modifiable in sketch.ts</strong>",
                    "<small><code>{}</code> · <code>{}</code> · {}</small>",
                    "</aside>"
                ),
                escape_attribute(source_path),
                consumer_count,
                generated_consumer_count,
                escape_html(source_path),
                escape_html(source_text),
                escape_html(&fan_out),
            );
        }
        InspectorParameterAuthority::ModifiableInstance => markup.push_str(concat!(
            "<aside class=\"wb-intent-parameter-authority\" ",
            "data-intent-parameter-authority=\"modifiable-instance\">",
            "<strong>Modifiable instance</strong>",
            "<small>Solver draft/overlay · does not rewrite sketch.ts</small></aside>"
        )),
        InspectorParameterAuthority::Encoded { reason } => {
            let _ = write!(
                markup,
                concat!(
                    "<aside class=\"wb-intent-parameter-authority\" ",
                    "data-intent-parameter-authority=\"encoded\">",
                    "<strong>Encoded</strong><small>{}</small></aside>"
                ),
                escape_html(reason),
            );
        }
        InspectorParameterAuthority::Blocked { reason } => {
            let _ = write!(
                markup,
                concat!(
                    "<aside class=\"wb-intent-parameter-authority\" ",
                    "data-intent-parameter-authority=\"blocked\">",
                    "<strong>Blocked</strong><small>{}</small></aside>"
                ),
                escape_html(reason),
            );
        }
    }
}

pub(crate) const fn intent_unit_key(unit: IntentUnit) -> &'static str {
    match unit {
        IntentUnit::Length => "length",
        IntentUnit::Angle => "angle",
        IntentUnit::Dimensionless => "dimensionless",
    }
}

pub(crate) fn literal_schema_key(schema: IntentLiteralSchema) -> String {
    match schema {
        IntentLiteralSchema::Boolean => "boolean".to_owned(),
        IntentLiteralSchema::Integer => "integer".to_owned(),
        IntentLiteralSchema::Natural => "natural".to_owned(),
        IntentLiteralSchema::Text => "text".to_owned(),
        IntentLiteralSchema::Enum => "enum".to_owned(),
        IntentLiteralSchema::Point => "point".to_owned(),
        IntentLiteralSchema::Quantity(unit) => format!("quantity:{}", intent_unit_key(unit)),
    }
}

pub(crate) const fn literal_schema_for_leaf(field: LeafField) -> IntentLiteralSchema {
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

pub(crate) fn literal_schema_for_instance(
    field: LeafField,
    value: Option<&IntentLiteral>,
) -> IntentLiteralSchema {
    value.map_or_else(
        || literal_schema_for_leaf(field),
        |value| match value {
            IntentLiteral::Quantity { unit, .. } => IntentLiteralSchema::Quantity(*unit),
            _ => literal_schema_for_leaf(field),
        },
    )
}

fn node_family_label(kind: &IntentGraphNodeKind) -> &'static str {
    match kind {
        IntentGraphNodeKind::Geometry { .. } => "Geometry",
        IntentGraphNodeKind::Constraint { .. } => "Relation",
        IntentGraphNodeKind::Dimension { .. } => "Dimension",
        IntentGraphNodeKind::Operation { .. } => "Operation",
        IntentGraphNodeKind::ComputedFeature { .. } => "Computed",
        IntentGraphNodeKind::Aggregate { .. } => "Aggregate",
        IntentGraphNodeKind::Parameter { .. } => "Parameter",
        IntentGraphNodeKind::External { .. } => "External",
        IntentGraphNodeKind::Bootstrap { .. } => "Imported",
        IntentGraphNodeKind::Annotation => "Annotation",
        IntentGraphNodeKind::Identity { .. } => "Identity",
    }
}

fn operation_label(kind: IntentPatchOperationKind) -> &'static str {
    match kind {
        IntentPatchOperationKind::CreateNode => "Create",
        IntentPatchOperationKind::DeleteNode => "Delete",
        IntentPatchOperationKind::SetSuppressed => "Suppress",
        IntentPatchOperationKind::SetDefinitionField => "Edit definition",
        IntentPatchOperationKind::SetInstanceLeaf => "Move / edit value",
        IntentPatchOperationKind::EjectBootstrapPoint => "Eject bootstrap Point",
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

fn projection_path_code(path: &IntentProjectionPath) -> String {
    projection_path_segments_code(path.segments())
}

fn projection_path_segments_code(segments: &[IntentProjectionPathSegment]) -> String {
    let mut output = String::new();
    for segment in segments {
        match segment {
            IntentProjectionPathSegment::Field(field) => {
                if !output.is_empty() {
                    output.push('.');
                }
                output.push_str(field.as_str());
            }
            IntentProjectionPathSegment::Index(index) => {
                let _ = write!(output, "[{index}]");
            }
        }
    }
    output
}

fn projection_path_leaf_label(path: &IntentProjectionPath) -> String {
    path.segments()
        .iter()
        .rev()
        .find_map(|segment| match segment {
            IntentProjectionPathSegment::Field(field) => Some(humanize_identifier(field.as_str())),
            IntentProjectionPathSegment::Index(_) => None,
        })
        .expect("validated projection path begins with a field")
}

fn projection_path_terminal_label(path: &IntentProjectionPath) -> String {
    match path.segments().last() {
        Some(IntentProjectionPathSegment::Field(field)) => humanize_identifier(field.as_str()),
        Some(IntentProjectionPathSegment::Index(_)) => "Reference".to_owned(),
        None => unreachable!("validated projection path is non-empty"),
    }
}

fn singular_projection_label(value: &str) -> String {
    let singular = match value {
        "vertices" => "vertex",
        "indices" => "index",
        candidate => candidate.strip_suffix('s').unwrap_or(candidate),
    };
    humanize_identifier(singular)
}

fn humanize_identifier(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 4);
    let mut previous_lower = false;
    for character in value.chars() {
        if character == '_' || character == '-' {
            output.push(' ');
            previous_lower = false;
            continue;
        }
        if character.is_ascii_uppercase() && previous_lower {
            output.push(' ');
        }
        output.push(character.to_ascii_lowercase());
        previous_lower = character.is_ascii_lowercase() || character.is_ascii_digit();
    }
    if let Some(first) = output.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    output
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
    use geosolve_constraint_editor::{
        IntentGraphNodeKind, IntentInspectorEditTarget, IntentInspectorField, IntentInspectorInput,
        IntentOutlineDeclaration, IntentProjectedPortReference, IntentSourceToken,
        IntentSourceTokenId, IntentSourceTokenTarget, IntentWorkbenchProjection,
    };
    use geosolve_sketch_intent::{
        AggregateKind, ComputedFeatureKind, GeometryRecipeKind, InputRole, InputSlot,
        IntentDeclarationDescriptor, IntentEditClassification, IntentEvaluation,
        IntentInputDescriptor, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind,
        IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortKind, IntentPortRole,
        IntentPortSelector, IntentProjectionPath, IntentSession, IntentSessionId, IntentUnit,
        LeafField, MaterializationEvidence, NodeId, OperationKind,
    };

    use super::{
        DesignProjectionSelection, InspectorParameterAuthority, InspectorParameterPresentation,
        declaration_count, history_markup, inspector_markup, inspector_markup_with_parameters,
        outline_markup, structured_source_markup,
    };

    fn key(value: &str) -> IntentKey {
        IntentKey::new(value).unwrap()
    }

    fn display_name_control(markup: &str) -> &str {
        markup
            .split_once("Display name")
            .expect("display-name control")
            .1
            .split_once("</label>")
            .expect("bounded display-name label")
            .0
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
        assert!(
            source.contains(
                "import type { IntentSourceSnapshot } from &quot;@geosolve/intent&quot;;"
            )
        );
        assert!(source.contains("satisfies IntentSourceSnapshot;"));
        assert!(source.contains("data-intent-source-token=\"0\""));
        assert!(source.contains(&format!(
            "data-intent-digest=\"{}\"",
            projection.identity.digest
        )));
        assert!(source.contains("class=\"wb-intent-source-line selected\""));
        assert!(history.contains("Accepted"));
        assert!(history.contains("Create"));
        assert!(!history.contains("data-wb-action"));
        assert!(inspector.contains("data-intent-edit=\"name\""));
        assert!(inspector.contains(&format!(
            "data-intent-digest=\"{}\"",
            projection.identity.digest
        )));
        assert_eq!(
            inspector.matches("data-intent-edit=\"instance\"").count(),
            2
        );
        assert!(inspector.contains("data-intent-group-path=\"point\"><legend>Point</legend>"));
        assert!(inspector.contains(">X<input"));
        assert!(inspector.contains(">Y<input"));
        assert!(inspector.contains("<select data-intent-edit=\"definition\""));
        assert!(
            inspector.contains("<option value=\"\" disabled selected>Default (Profile)</option>")
        );
        assert!(inspector.contains("<option value=\"profile\">Profile</option>"));
        assert!(inspector.contains("<option value=\"construction\">Construction</option>"));
        assert!(!inspector.contains("onclick="));
    }

    #[test]
    fn parameter_markup_applies_code_owned_name_authority_without_affecting_gui_names() {
        let (session, projection, node) = fixture();
        let inspector = projection
            .inspector(&session, node)
            .expect("fixture Inspector");

        let gui_markup = inspector_markup(Some(&inspector));
        let gui_name = display_name_control(&gui_markup);
        assert!(gui_name.contains("data-intent-edit=\"name\""));
        assert!(!gui_name.contains("data-intent-name-authority="));
        assert!(!gui_name.contains(" disabled"));

        let source_backed = [InspectorParameterPresentation {
            target: IntentInspectorEditTarget::Suppressed,
            authority: InspectorParameterAuthority::ModifiableSource {
                source_path: "point.enabled".into(),
                source_text: "true".into(),
                consumer_count: 1,
                generated_consumer_count: 0,
            },
        }];
        let source_markup = inspector_markup_with_parameters(Some(&inspector), &source_backed);
        let source_name = display_name_control(&source_markup);
        assert!(source_name.contains("data-intent-name-authority=\"encoded\""));
        assert!(source_name.contains("data-intent-encoded=\"true\""));
        assert!(source_name.contains(" disabled"));
        assert!(!source_name.contains("data-intent-edit=\"name\""));

        let blocked = [InspectorParameterPresentation {
            target: IntentInspectorEditTarget::Suppressed,
            authority: InspectorParameterAuthority::Blocked {
                reason: "Apply or Revert the source draft".into(),
            },
        }];
        let blocked_markup = inspector_markup_with_parameters(Some(&inspector), &blocked);
        let blocked_name = display_name_control(&blocked_markup);
        assert!(blocked_name.contains("data-intent-name-authority=\"blocked\""));
        assert!(blocked_name.contains("data-intent-blocked=\"true\""));
        assert!(blocked_name.contains("Apply or Revert the source draft"));
        assert!(blocked_name.contains(" disabled"));
        assert!(!blocked_name.contains("data-intent-edit=\"name\""));
    }

    #[test]
    fn inspector_renders_stable_input_bindings_as_read_only_references() {
        let (session, _, _) = fixture();
        let node = NodeId::from_raw(0x8305_0011);
        let slot = InputSlot::new(InputRole::Point, 0);
        let input_path = IntentProjectionPath::field(key("start"));
        let source = IntentProjectedPortReference {
            declaration: key("source.point"),
            output: IntentProjectionPath::field(key("point")),
            kind: IntentPortKind::Point,
        };
        let kind = IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        };
        let inspector = geosolve_constraint_editor::IntentInspectorProjection {
            identity: session.identity(),
            node,
            symbol: key("bound_segment"),
            name: key("Bound segment"),
            kind: IntentGraphNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            suppressed: false,
            retained_failure: false,
            inputs: vec![IntentInspectorInput {
                path: input_path.clone(),
                source: source.clone(),
            }],
            descriptor: IntentDeclarationDescriptor {
                schema: kind.schema(0),
                inputs: vec![IntentInputDescriptor {
                    slot,
                    path: input_path,
                }],
                fields: kind.field_descriptors(0),
                outputs: Vec::new(),
                suppression_edit: IntentEditClassification::Definition,
                input_edit: IntentEditClassification::InputBinding,
                name_edit: IntentEditClassification::Organization,
            },
            fields: Vec::new(),
        };

        let markup = inspector_markup(Some(&inspector));
        let inputs = markup
            .split_once("<fieldset class=\"wb-intent-inputs\">")
            .unwrap()
            .1
            .split_once("</fieldset>")
            .unwrap()
            .0;

        assert!(inputs.contains("<legend>Inputs</legend>"));
        assert!(inputs.contains("data-intent-input-path=\"start\""));
        assert!(inputs.contains("data-intent-source-declaration=\"source.point\""));
        assert!(inputs.contains("data-intent-source-output=\"point\""));
        assert!(inputs.contains("data-intent-source-kind=\"Point\""));
        assert!(inputs.contains("<span>Start</span>"));
        assert!(inputs.contains("<code>source.point → point · Point</code>"));
        assert!(!inputs.contains("point:0000"));
        assert!(!inputs.contains("data-intent-source-node"));
        assert!(!inputs.contains("data-intent-source-port"));
        assert!(!inputs.contains("<input"));
        assert!(!inputs.contains("<button"));
        assert!(!inputs.contains("contenteditable"));
        assert!(!inputs.contains("data-intent-edit"));
    }

    #[test]
    fn inspector_groups_repeated_fillet_fields_as_nested_objects_and_arrays() {
        let (session, _, _) = fixture();
        let node = NodeId::from_raw(0x8305_0020);
        let kind = IntentNodeKind::ComputedFeature {
            feature: ComputedFeatureKind::FilletSet,
        };
        let descriptors = kind.field_descriptors(2);
        let fields = descriptors
            .iter()
            .map(|descriptor| IntentInspectorField::Definition {
                definition: descriptor.schema.field.clone(),
                value: None,
            })
            .collect();
        let inspector = geosolve_constraint_editor::IntentInspectorProjection {
            identity: session.identity(),
            node,
            symbol: key("fillet.main"),
            name: key("Two corner fillet"),
            kind: IntentGraphNodeKind::ComputedFeature {
                feature: ComputedFeatureKind::FilletSet,
            },
            suppressed: false,
            retained_failure: false,
            inputs: Vec::new(),
            descriptor: IntentDeclarationDescriptor {
                schema: kind.schema(2),
                inputs: Vec::new(),
                fields: descriptors,
                outputs: Vec::new(),
                suppression_edit: IntentEditClassification::Definition,
                input_edit: IntentEditClassification::InputBinding,
                name_edit: IntentEditClassification::Organization,
            },
            fields,
        };

        let markup = inspector_markup(Some(&inspector));
        assert!(markup.contains("data-intent-group-path=\"corners\"><legend>Corners</legend>"));
        assert!(markup.contains(
            "data-intent-array-index=\"0\" data-intent-group-path=\"corners[0]\"><legend>Corner 1</legend>"
        ));
        assert!(markup.contains(
            "data-intent-array-index=\"1\" data-intent-group-path=\"corners[1]\"><legend>Corner 2</legend>"
        ));
        assert!(
            markup
                .contains("data-intent-group-path=\"corners[0].parents\"><legend>Parents</legend>")
        );
        assert!(markup.contains(
            "data-intent-group-path=\"corners[0].parents[1]\"><legend>Parent 2</legend>"
        ));
        assert!(markup.contains(">Parameter<input"));
        assert!(!markup.contains(">Corner 0000"));
        assert!(!markup.contains(">Parent 0001"));
    }

    #[test]
    fn source_markup_is_deterministic_and_never_executable_input() {
        let (_, projection, _) = fixture();
        let first = structured_source_markup(&projection, None);
        assert_eq!(first, structured_source_markup(&projection, None));
        assert!(first.contains("export const sketch = {"));
        assert!(first.contains("satisfies IntentSourceSnapshot;"));
        assert!(first.contains("contenteditable=\"plaintext-only\""));
        assert!(first.contains("data-intent-drop-before="));
        assert!(first.contains("draggable=\"true\""));
        assert!(!first.contains("eval("));
        assert!(!first.contains("new Function"));
    }

    #[test]
    fn source_markup_preserves_ordered_tokens_and_cell_bindings() {
        let (_, projection, node) = fixture();
        let cell = projection.outline[0].cell;
        let markup =
            structured_source_markup(&projection, Some(DesignProjectionSelection { node }));

        let mut previous_position = None;
        for token in &projection.structured_source.tokens {
            let coordinate = format!("data-intent-source-token=\"{}\"", token.id.0);
            assert_eq!(markup.matches(&coordinate).count(), 1);
            let position = markup
                .find(&coordinate)
                .expect("every ordered source token is rendered");
            if let Some(previous_position) = previous_position {
                assert!(previous_position < position);
            }
            previous_position = Some(position);
        }
        assert_eq!(
            markup.matches("class=\"wb-intent-source-token\"").count(),
            projection.structured_source.tokens.len()
        );

        let selected_rows = markup
            .split("</div>")
            .filter(|row| row.contains("class=\"wb-intent-source-line selected\""))
            .collect::<Vec<_>>();
        assert!(!selected_rows.is_empty());
        for row in selected_rows {
            assert!(row.contains(&format!("data-intent-node=\"{node}\"")));
            assert!(row.contains(&format!("data-intent-cell=\"{cell}\"")));
            assert!(row.contains(&format!("data-intent-drop-before=\"{node}\"")));
            assert!(row.contains("draggable=\"true\""));
        }
    }

    #[test]
    fn outline_groups_private_offset_operands_and_renders_retained_diagnostic() {
        let (_, mut projection, _) = fixture();
        let aggregate = NodeId::from_raw(0x8305_1001);
        let offset = NodeId::from_raw(0x8305_1002);
        let declaration = |node, symbol: &str, kind, dependencies| IntentOutlineDeclaration {
            node,
            symbol: key(symbol),
            name: key(symbol),
            kind,
            suppressed: false,
            retained_failure: false,
            dependencies,
        };
        projection.outline[0].declarations.extend([
            declaration(
                aggregate,
                "offset_operand_helper",
                IntentGraphNodeKind::Aggregate {
                    aggregate: AggregateKind::OpenChain,
                },
                Vec::new(),
            ),
            declaration(
                offset,
                "Offset 1",
                IntentGraphNodeKind::Operation {
                    operation: OperationKind::ProfileOffset,
                },
                vec![aggregate],
            ),
        ]);
        projection.latest_diagnostic = Some(key("offset-distance-invalid"));

        let markup = outline_markup(&projection, None);
        assert_eq!(declaration_count(&projection), 2);
        assert!(markup.contains("Offset 1"));
        assert!(!markup.contains("offset_operand_helper"));
        assert!(markup.contains("Retained intent needs attention"));
        assert!(markup.contains("offset-distance-invalid"));

        projection.outline[0].declarations.push(declaration(
            NodeId::from_raw(0x8305_1003),
            "Offset 2",
            IntentGraphNodeKind::Operation {
                operation: OperationKind::ProfileOffset,
            },
            vec![aggregate],
        ));
        let shared_markup = outline_markup(&projection, None);
        assert!(shared_markup.contains("offset_operand_helper"));
        assert_eq!(declaration_count(&projection), 4);
    }

    #[test]
    fn structured_source_does_not_expose_private_offset_helpers_as_declarations() {
        let (_, mut projection, helper) = fixture();
        let offset = NodeId::from_raw(0x8305_2002);
        let declaration = |node, symbol: &str, kind, dependencies| IntentOutlineDeclaration {
            node,
            symbol: key(symbol),
            name: key(symbol),
            kind,
            suppressed: false,
            retained_failure: false,
            dependencies,
        };
        let helper_declaration = projection.outline[0]
            .declarations
            .iter_mut()
            .find(|declaration| declaration.node == helper)
            .expect("fixture declaration exists");
        helper_declaration.kind = IntentGraphNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        };
        projection.outline[0].declarations.push(declaration(
            offset,
            "Offset 1",
            IntentGraphNodeKind::Operation {
                operation: OperationKind::ProfileOffset,
            },
            vec![helper],
        ));

        let offset_line = "    declare(\"offset\", \"Offset 1\");\n";
        let offset_name_start = projection.structured_source.text.len()
            + offset_line
                .find("\"Offset 1\"")
                .expect("synthetic declaration has a name token");
        projection.structured_source.text.push_str(offset_line);
        projection.structured_source.tokens.push(IntentSourceToken {
            id: IntentSourceTokenId(
                u32::try_from(projection.structured_source.tokens.len())
                    .expect("bounded source token count"),
            ),
            start: offset_name_start,
            end: offset_name_start + "\"Offset 1\"".len(),
            target: IntentSourceTokenTarget::NodeName { node: offset },
        });

        let markup = structured_source_markup(
            &projection,
            Some(DesignProjectionSelection { node: helper }),
        );
        assert!(!markup.contains(&format!("data-intent-node=\"{helper}\"")));
        assert!(!markup.contains(&format!("data-intent-drop-before=\"{helper}\"")));
        let helper_row = markup
            .split("</div>")
            .find(|row| row.contains("&quot;Point 1&quot;"))
            .expect("private helper source row is rendered as code");
        assert!(!helper_row.contains(" selected"));
        assert!(!helper_row.contains("data-intent-node="));
        assert!(!helper_row.contains("data-intent-cell="));
        assert!(!helper_row.contains("data-intent-drop-before="));
        assert!(!helper_row.contains("draggable="));
        assert!(helper_row.contains("data-intent-source-token=\"0\""));
        assert!(helper_row.contains("contenteditable=\"plaintext-only\""));

        let offset_row = markup
            .split("</div>")
            .find(|row| row.contains("&quot;Offset 1&quot;"))
            .expect("visible Offset source row is rendered");
        assert!(offset_row.contains(&format!("data-intent-node=\"{offset}\"")));
        assert!(offset_row.contains("data-intent-cell="));
        assert!(offset_row.contains(&format!("data-intent-drop-before=\"{offset}\"")));
        assert!(offset_row.contains("draggable=\"true\""));
        assert!(offset_row.contains("data-intent-source-token="));
        assert!(offset_row.contains("contenteditable=\"plaintext-only\""));
    }
}
