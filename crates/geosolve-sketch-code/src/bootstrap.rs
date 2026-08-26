// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit conversion of accepted GUI declarations into managed source.
//!
//! Conversion is intentionally a snapshot operation, not an attempt to infer
//! fictional recipe history from arbitrary native geometry. Only declaration
//! families with a truthful managed equivalent are admitted.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use geosolve_constraint_editor::{
    ComputedFeatureDefinition, IntentNativeBinding, NativeCurveSpanSource, NewComputedFilletCorner,
    ProjectionalEditorSession,
};
use geosolve_sketch::{
    ContactNeighborhood, DocumentArcSweep, DocumentCurveNormalSide, DocumentFilletEndpointOrder,
    DocumentFilletTrimEndpoint, DocumentTrimParameter,
};
use geosolve_sketch_intent::{
    ComputedFeatureKind, GeometryRecipeKind, InputRole, InputSlot, IntentNode, IntentNodeKind,
    IntentPortKind, IntentPortRole, IntentPortSelector, NodeId,
};
use thiserror::Error;

use crate::{CodeProject, ProjectKey, SemanticSymbol, parse_managed_source};

/// One accepted GUI declaration selected for explicit managed initialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorBootstrapDeclaration {
    pub node: NodeId,
    pub symbol: SemanticSymbol,
}

impl EditorBootstrapDeclaration {
    #[must_use]
    pub const fn new(node: NodeId, symbol: SemanticSymbol) -> Self {
        Self { node, symbol }
    }
}

/// Why an accepted GUI checkpoint could not be represented honestly in the
/// bounded managed-v1 surface.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum EditorBootstrapError {
    #[error("the GUI checkpoint has no independently accepted native authority")]
    MissingAcceptedAuthority,
    #[error("the accepted GUI checkpoint failed independent native validation")]
    InvalidAcceptedAuthority,
    #[error("bootstrap selection is empty")]
    EmptySelection,
    #[error("bootstrap selection repeats declaration node {0}")]
    DuplicateNode(NodeId),
    #[error("bootstrap selection repeats managed symbol `{0}`")]
    DuplicateSymbol(String),
    #[error("`{0}` is not a valid managed declaration identifier")]
    InvalidSymbol(String),
    #[error("selected GUI declaration node {0} does not exist")]
    MissingNode(NodeId),
    #[error("GUI declaration `{symbol}` uses unsupported recipe `{recipe}`")]
    UnsupportedRecipe { symbol: String, recipe: String },
    #[error("GUI declaration `{symbol}` depends on unselected declaration node {dependency}")]
    MissingDependency { symbol: String, dependency: NodeId },
    #[error("GUI declaration `{symbol}` has an unsupported `{input}` dependency")]
    UnsupportedReference { symbol: String, input: &'static str },
    #[error("GUI declaration `{symbol}` has no accepted native `{output}` point")]
    MissingNativePoint {
        symbol: String,
        output: &'static str,
    },
    #[error("GUI declaration `{symbol}` has no exact accepted computed feature")]
    MissingComputedFeature { symbol: String },
    #[error("GUI declaration `{symbol}` has invalid accepted computed-feature state")]
    InvalidComputedFeatureState { symbol: String },
    #[error("GUI declaration `{symbol}` does not own its accepted Fillet span `{input}`")]
    FilletSpanOwnershipMismatch { symbol: String, input: String },
    #[error("GUI declaration `{symbol}` contains non-finite accepted geometry")]
    NonFiniteGeometry { symbol: String },
    #[error("generated managed bootstrap is invalid: {0}")]
    InvalidManagedSource(String),
}

/// Initializes a complete artifact-free code project from selected accepted
/// GUI declarations.
///
/// This operation does not inspect native geometry and guess how it was made.
/// It reads the selected intent declaration kind, authenticates its exact
/// accepted native outputs through the central ownership map, and admits it
/// only when that declaration has a truthful managed-v1 equivalent. The
/// resulting project begins with no custom files or artifacts; callers may
/// subsequently add pinned reusable patches through ordinary managed edits.
///
/// # Errors
///
/// Returns a typed error when accepted authority is absent/invalid, a
/// selection is ambiguous, or a selected declaration cannot be represented
/// without inventing history.
pub fn initialize_code_project_from_editor(
    editor: &ProjectionalEditorSession,
    project: ProjectKey,
    declarations: &[EditorBootstrapDeclaration],
) -> Result<CodeProject, EditorBootstrapError> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(EditorBootstrapError::MissingAcceptedAuthority)?;
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err(EditorBootstrapError::InvalidAcceptedAuthority);
    }
    validate_selection(declarations)?;

    let intent = editor.coordinator().intent();
    let declarations = dependency_order(editor, declarations)?;
    let selected = declarations
        .iter()
        .map(|declaration| (declaration.node, *declaration))
        .collect::<BTreeMap<_, _>>();
    let mut body = String::new();
    for declaration in &declarations {
        let node = intent
            .graph()
            .node(declaration.node)
            .ok_or(EditorBootstrapError::MissingNode(declaration.node))?;
        write_managed_declaration(&mut body, editor, declaration, node, &selected)?;
    }

    let output = declarations
        .iter()
        .map(|declaration| declaration.symbol.0.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "\"use geosolve managed-v1\";\nimport {{ sketch }} from \"@geosolve/sketch-code\";\n\nexport default sketch(($) => {{\n{body}  return $.outputs({{{output}}});\n}});\n"
    );
    let managed = parse_managed_source(&source)
        .map_err(|error| EditorBootstrapError::InvalidManagedSource(error.to_string()))?;
    let code_project = CodeProject {
        project,
        managed,
        custom_files: BTreeMap::new(),
        artifacts: BTreeMap::new(),
        lock: serde_json::json!({
            "format": "geosolve-lock-v1",
            "modules": {},
        }),
    };
    code_project
        .validate()
        .map_err(|error| EditorBootstrapError::InvalidManagedSource(error.to_string()))?;
    Ok(code_project)
}

fn write_managed_declaration(
    body: &mut String,
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, &EditorBootstrapDeclaration>,
) -> Result<(), EditorBootstrapError> {
    match node.kind {
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
        } => {
            let bounds = accepted_rectangle_bounds(editor, declaration)?;
            let lower_x = managed_number(bounds.lower_left[0]);
            let lower_y = managed_number(bounds.lower_left[1]);
            let upper_x = managed_number(bounds.upper_right[0]);
            let upper_y = managed_number(bounds.upper_right[1]);
            write!(
                body,
                "  const {symbol} = $.geometry.rectangle(\"{symbol}\", {{\n    lowerLeft: [{lower_x}, {lower_y}],\n    upperRight: [{upper_x}, {upper_y}],\n  }});\n",
                symbol = declaration.symbol.0,
            )
            .expect("writing managed source to a String cannot fail");
        }
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        } => {
            let start = segment_endpoint_expression(
                editor,
                declaration,
                node,
                selected,
                0,
                IntentPortRole::Start,
                "start",
            )?;
            let end = segment_endpoint_expression(
                editor,
                declaration,
                node,
                selected,
                1,
                IntentPortRole::End,
                "end",
            )?;
            write!(
                body,
                "  const {symbol} = $.geometry.line(\"{symbol}\", {{\n    start: {start},\n    end: {end},\n  }});\n",
                symbol = declaration.symbol.0,
            )
            .expect("writing managed source to a String cannot fail");
        }
        IntentNodeKind::ComputedFeature {
            feature: ComputedFeatureKind::FilletSet,
        } => write_managed_fillet_set(body, editor, declaration, node, selected)?,
        ref kind => {
            return Err(EditorBootstrapError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: intent_kind_name(kind),
            });
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "the bounded Fillet serializer keeps exact accepted branch state visibly contiguous"
)]
fn write_managed_fillet_set(
    body: &mut String,
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, &EditorBootstrapDeclaration>,
) -> Result<(), EditorBootstrapError> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(EditorBootstrapError::MissingAcceptedAuthority)?;
    let feature_port = node
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Feature,
            index: 0,
        })
        .ok_or_else(|| EditorBootstrapError::MissingComputedFeature {
            symbol: declaration.symbol.0.clone(),
        })?;
    let Some(IntentNativeBinding::ComputedFeature(feature_id)) =
        accepted.ownership.port(feature_port.as_ref(node.id))
    else {
        return Err(EditorBootstrapError::MissingComputedFeature {
            symbol: declaration.symbol.0.clone(),
        });
    };
    let feature = accepted.features.feature(feature_id).ok_or_else(|| {
        EditorBootstrapError::MissingComputedFeature {
            symbol: declaration.symbol.0.clone(),
        }
    })?;
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
    if !fillet.radius.is_finite()
        || fillet.radius <= 0.0
        || feature.suppressed != node.suppressed
        || fillet.corners.is_empty()
        || fillet.corners.len() != node.child_order.len()
    {
        return Err(EditorBootstrapError::InvalidComputedFeatureState {
            symbol: declaration.symbol.0.clone(),
        });
    }

    writeln!(
        body,
        "  const {symbol} = $.computed.filletSet(\"{symbol}\", {{",
        symbol = declaration.symbol.0,
    )
    .expect("writing managed source to a String cannot fail");
    writeln!(body, "    radius: {},", managed_number(fillet.radius))
        .expect("writing managed source to a String cannot fail");
    body.push_str("    corners: [\n");
    for (ordinal, corner) in fillet.corners.iter().enumerate() {
        let ordinal_u16 = u16::try_from(ordinal).map_err(|_| {
            EditorBootstrapError::InvalidComputedFeatureState {
                symbol: declaration.symbol.0.clone(),
            }
        })?;
        let corner_port = node
            .port_by_selector(IntentPortSelector::InitialChild {
                ordinal: ordinal_u16,
                role: IntentPortRole::FeatureCorner,
                index: 0,
            })
            .ok_or_else(|| EditorBootstrapError::InvalidComputedFeatureState {
                symbol: declaration.symbol.0.clone(),
            })?;
        if accepted.ownership.port(corner_port.as_ref(node.id))
            != Some(IntentNativeBinding::ComputedFeatureCorner(corner.id))
        {
            return Err(EditorBootstrapError::InvalidComputedFeatureState {
                symbol: declaration.symbol.0.clone(),
            });
        }
        let corner = corner.without_id();
        if !fillet_corner_is_finite(corner) {
            return Err(EditorBootstrapError::InvalidComputedFeatureState {
                symbol: declaration.symbol.0.clone(),
            });
        }
        let first_index = u16::try_from(ordinal.saturating_mul(2)).map_err(|_| {
            EditorBootstrapError::InvalidComputedFeatureState {
                symbol: declaration.symbol.0.clone(),
            }
        })?;
        let second_index = first_index.checked_add(1).ok_or_else(|| {
            EditorBootstrapError::InvalidComputedFeatureState {
                symbol: declaration.symbol.0.clone(),
            }
        })?;
        body.push_str("      {\n");
        body.push_str("        parents: [\n");
        write_managed_fillet_parent(
            body,
            editor,
            declaration,
            node,
            selected,
            first_index,
            corner,
        )?;
        write_managed_fillet_parent(
            body,
            editor,
            declaration,
            node,
            selected,
            second_index,
            corner,
        )?;
        body.push_str("        ],\n");
        writeln!(
            body,
            "        endpointOrder: \"{}\",\n        sweep: \"{}\",\n      }},",
            match corner.endpoint_order {
                DocumentFilletEndpointOrder::FirstThenSecond => "firstThenSecond",
                DocumentFilletEndpointOrder::SecondThenFirst => "secondThenFirst",
            },
            match corner.sweep {
                DocumentArcSweep::CounterClockwise => "counterClockwise",
                DocumentArcSweep::Clockwise => "clockwise",
            },
        )
        .expect("writing managed source to a String cannot fail");
    }
    writeln!(
        body,
        "    ],\n    suppressed: {},\n  }});",
        feature.suppressed,
    )
    .expect("writing managed source to a String cannot fail");
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the explicit parent coordinate keeps accepted owner, input slot, and branch together"
)]
fn write_managed_fillet_parent(
    body: &mut String,
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, &EditorBootstrapDeclaration>,
    input_index: u16,
    corner: NewComputedFilletCorner,
) -> Result<(), EditorBootstrapError> {
    let parent = if input_index.is_multiple_of(2) {
        corner.first
    } else {
        corner.second
    };
    let span = fillet_span_expression(
        editor,
        declaration,
        node,
        selected,
        input_index,
        parent.source,
    )?;
    let neighborhood = match parent.neighborhood {
        ContactNeighborhood::Interior => "{ kind: \"interior\" }".to_owned(),
        ContactNeighborhood::Start => "{ kind: \"start\" }".to_owned(),
        ContactNeighborhood::End => "{ kind: \"end\" }".to_owned(),
        ContactNeighborhood::Local { lower, upper } => format!(
            "{{ kind: \"local\", lower: {}, upper: {} }}",
            managed_number(lower),
            managed_number(upper),
        ),
    };
    let periodic_anchor = parent.periodic_anchor.map_or_else(
        || "null".to_owned(),
        |DocumentTrimParameter { parameter, winding }| {
            format!(
                "{{ parameter: {}, winding: {winding} }}",
                managed_number(parameter),
            )
        },
    );
    writeln!(
        body,
        concat!(
            "          {{\n",
            "            span: {span},\n",
            "            parameter: {parameter},\n",
            "            winding: {winding},\n",
            "            neighborhood: {neighborhood},\n",
            "            normalSide: \"{normal_side}\",\n",
            "            retainedEndpoint: \"{retained_endpoint}\",\n",
            "            periodicAnchor: {periodic_anchor},\n",
            "          }},"
        ),
        span = span,
        parameter = managed_number(parent.picked_parameter),
        winding = parent.winding,
        neighborhood = neighborhood,
        normal_side = match parent.normal_side {
            DocumentCurveNormalSide::Left => "left",
            DocumentCurveNormalSide::Right => "right",
        },
        retained_endpoint = match parent.retained_endpoint {
            DocumentFilletTrimEndpoint::Start => "start",
            DocumentFilletTrimEndpoint::End => "end",
        },
        periodic_anchor = periodic_anchor,
    )
    .expect("writing managed source to a String cannot fail");
    Ok(())
}

fn fillet_span_expression(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, &EditorBootstrapDeclaration>,
    input_index: u16,
    native_source: NativeCurveSpanSource,
) -> Result<String, EditorBootstrapError> {
    let input_name = format!(
        "corners[{}].parents[{}].span",
        input_index / 2,
        input_index % 2,
    );
    let source = node
        .inputs
        .get(&InputSlot::new(InputRole::Span, input_index))
        .copied()
        .ok_or_else(|| EditorBootstrapError::FilletSpanOwnershipMismatch {
            symbol: declaration.symbol.0.clone(),
            input: input_name.clone(),
        })?;
    let source_declaration =
        selected
            .get(&source.node)
            .ok_or(EditorBootstrapError::MissingDependency {
                symbol: declaration.symbol.0.clone(),
                dependency: source.node,
            })?;
    let source_node = editor
        .coordinator()
        .intent()
        .graph()
        .node(source.node)
        .ok_or(EditorBootstrapError::MissingNode(source.node))?;
    let source_port = source_node.port(source.port).ok_or_else(|| {
        EditorBootstrapError::FilletSpanOwnershipMismatch {
            symbol: declaration.symbol.0.clone(),
            input: input_name.clone(),
        }
    })?;
    if source.kind != IntentPortKind::CurveSpan
        || source_port.kind != IntentPortKind::CurveSpan
        || editor
            .coordinator()
            .accepted_materialization()
            .and_then(|accepted| accepted.ownership.port(source))
            != Some(IntentNativeBinding::CurveSpan(native_source.span))
    {
        return Err(EditorBootstrapError::FilletSpanOwnershipMismatch {
            symbol: declaration.symbol.0.clone(),
            input: input_name,
        });
    }
    let IntentPortSelector::Node {
        role: IntentPortRole::Span,
        index,
    } = source_port.selector
    else {
        return Err(EditorBootstrapError::FilletSpanOwnershipMismatch {
            symbol: declaration.symbol.0.clone(),
            input: input_name,
        });
    };
    match source_node.kind {
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        } if index == 0 => Ok(format!("{}.span", source_declaration.symbol.0)),
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
        } if index < 4 => Ok(format!(
            "{}.edges.{}",
            source_declaration.symbol.0,
            ["bottom", "right", "top", "left"][usize::from(index)],
        )),
        _ => Err(EditorBootstrapError::UnsupportedReference {
            symbol: declaration.symbol.0.clone(),
            input: "Fillet parent span",
        }),
    }
}

fn fillet_corner_is_finite(corner: NewComputedFilletCorner) -> bool {
    [corner.first, corner.second].into_iter().all(|parent| {
        parent.picked_parameter.is_finite()
            && match parent.neighborhood {
                ContactNeighborhood::Local { lower, upper } => {
                    lower.is_finite() && upper.is_finite()
                }
                ContactNeighborhood::Interior
                | ContactNeighborhood::Start
                | ContactNeighborhood::End => true,
            }
            && parent
                .periodic_anchor
                .is_none_or(|anchor| anchor.parameter.is_finite())
    })
}

fn dependency_order<'a>(
    editor: &ProjectionalEditorSession,
    declarations: &'a [EditorBootstrapDeclaration],
) -> Result<Vec<&'a EditorBootstrapDeclaration>, EditorBootstrapError> {
    let selected = declarations
        .iter()
        .map(|declaration| (declaration.node, declaration))
        .collect::<BTreeMap<_, _>>();
    let mut emitted = BTreeSet::new();
    let mut ordered = Vec::with_capacity(declarations.len());
    while ordered.len() < declarations.len() {
        let mut progress = false;
        for declaration in declarations {
            if emitted.contains(&declaration.node) {
                continue;
            }
            let node = editor
                .coordinator()
                .intent()
                .graph()
                .node(declaration.node)
                .ok_or(EditorBootstrapError::MissingNode(declaration.node))?;
            for dependency in node.dependencies() {
                if !selected.contains_key(&dependency) {
                    return Err(EditorBootstrapError::MissingDependency {
                        symbol: declaration.symbol.0.clone(),
                        dependency,
                    });
                }
            }
            if node
                .dependencies()
                .iter()
                .all(|dependency| emitted.contains(dependency))
            {
                emitted.insert(declaration.node);
                ordered.push(declaration);
                progress = true;
            }
        }
        if !progress {
            return Err(EditorBootstrapError::InvalidManagedSource(
                "selected GUI declarations contain a dependency cycle".into(),
            ));
        }
    }
    Ok(ordered)
}

#[derive(Clone, Copy)]
struct RectangleBounds {
    lower_left: [f64; 2],
    upper_right: [f64; 2],
}

#[allow(
    clippy::float_cmp,
    reason = "equal finite bounds are the exact zero-extent invalid rectangle boundary"
)]
fn accepted_rectangle_bounds(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
) -> Result<RectangleBounds, EditorBootstrapError> {
    let mut corners = [[0.0; 2]; 4];
    for (index, corner) in corners.iter_mut().enumerate() {
        *corner = accepted_point(
            editor,
            declaration,
            IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: u16::try_from(index).expect("four rectangle corners fit u16"),
            },
            "rectangle corner",
        )?;
    }
    if corners
        .iter()
        .flatten()
        .any(|coordinate| !coordinate.is_finite())
    {
        return Err(EditorBootstrapError::NonFiniteGeometry {
            symbol: declaration.symbol.0.clone(),
        });
    }
    let min_x = corners
        .iter()
        .map(|corner| corner[0])
        .fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|corner| corner[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = corners
        .iter()
        .map(|corner| corner[1])
        .fold(f64::INFINITY, f64::min);
    let max_y = corners
        .iter()
        .map(|corner| corner[1])
        .fold(f64::NEG_INFINITY, f64::max);
    if min_x == max_x || min_y == max_y {
        return Err(EditorBootstrapError::NonFiniteGeometry {
            symbol: declaration.symbol.0.clone(),
        });
    }
    Ok(RectangleBounds {
        lower_left: [min_x, min_y],
        upper_right: [max_x, max_y],
    })
}

fn segment_endpoint_expression(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, &EditorBootstrapDeclaration>,
    input_index: u16,
    role: IntentPortRole,
    input: &'static str,
) -> Result<String, EditorBootstrapError> {
    if let Some(source) = node
        .inputs
        .get(&InputSlot::new(InputRole::Point, input_index))
    {
        let source_declaration =
            selected
                .get(&source.node)
                .ok_or(EditorBootstrapError::MissingDependency {
                    symbol: declaration.symbol.0.clone(),
                    dependency: source.node,
                })?;
        let source_node = editor
            .coordinator()
            .intent()
            .graph()
            .node(source.node)
            .ok_or(EditorBootstrapError::MissingNode(source.node))?;
        let port =
            source_node
                .port(source.port)
                .ok_or(EditorBootstrapError::UnsupportedReference {
                    symbol: declaration.symbol.0.clone(),
                    input,
                })?;
        return match (&source_node.kind, port.selector) {
            (
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
                },
                IntentPortSelector::Node {
                    role: IntentPortRole::Corner,
                    index,
                },
            ) => {
                let position = accepted_point(
                    editor,
                    source_declaration,
                    IntentPortSelector::Node {
                        role: IntentPortRole::Corner,
                        index,
                    },
                    "referenced rectangle corner",
                )?;
                let bounds = accepted_rectangle_bounds(editor, source_declaration)?;
                let member = rectangle_corner_member(bounds, position).ok_or(
                    EditorBootstrapError::UnsupportedReference {
                        symbol: declaration.symbol.0.clone(),
                        input,
                    },
                )?;
                Ok(format!("{}.corners.{member}", source_declaration.symbol.0))
            }
            (
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Segment,
                },
                IntentPortSelector::Node {
                    role: IntentPortRole::Start,
                    index: 0,
                },
            ) => Ok(format!("{}.start", source_declaration.symbol.0)),
            (
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Segment,
                },
                IntentPortSelector::Node {
                    role: IntentPortRole::End,
                    index: 0,
                },
            ) => Ok(format!("{}.end", source_declaration.symbol.0)),
            _ => Err(EditorBootstrapError::UnsupportedReference {
                symbol: declaration.symbol.0.clone(),
                input,
            }),
        };
    }

    let position = accepted_point(
        editor,
        declaration,
        IntentPortSelector::Node { role, index: 0 },
        input,
    )?;
    if position
        .into_iter()
        .any(|coordinate| !coordinate.is_finite())
    {
        return Err(EditorBootstrapError::NonFiniteGeometry {
            symbol: declaration.symbol.0.clone(),
        });
    }
    Ok(format!(
        "[{}, {}]",
        managed_number(position[0]),
        managed_number(position[1]),
    ))
}

fn rectangle_corner_member(bounds: RectangleBounds, position: [f64; 2]) -> Option<&'static str> {
    if position
        .into_iter()
        .any(|coordinate| !coordinate.is_finite())
    {
        return None;
    }
    let middle = [
        bounds.lower_left[0].mul_add(0.5, bounds.upper_right[0] * 0.5),
        bounds.lower_left[1].mul_add(0.5, bounds.upper_right[1] * 0.5),
    ];
    match (position[0] <= middle[0], position[1] <= middle[1]) {
        (true, true) => Some("lowerLeft"),
        (false, true) => Some("lowerRight"),
        (false, false) => Some("upperRight"),
        (true, false) => Some("upperLeft"),
    }
}

fn validate_selection(
    declarations: &[EditorBootstrapDeclaration],
) -> Result<(), EditorBootstrapError> {
    if declarations.is_empty() {
        return Err(EditorBootstrapError::EmptySelection);
    }
    let mut nodes = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    for declaration in declarations {
        if !nodes.insert(declaration.node) {
            return Err(EditorBootstrapError::DuplicateNode(declaration.node));
        }
        if !valid_managed_identifier(&declaration.symbol.0) {
            return Err(EditorBootstrapError::InvalidSymbol(
                declaration.symbol.0.clone(),
            ));
        }
        if !symbols.insert(declaration.symbol.0.clone()) {
            return Err(EditorBootstrapError::DuplicateSymbol(
                declaration.symbol.0.clone(),
            ));
        }
    }
    Ok(())
}

fn accepted_point(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    selector: IntentPortSelector,
    output: &'static str,
) -> Result<[f64; 2], EditorBootstrapError> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(EditorBootstrapError::MissingAcceptedAuthority)?;
    let node = editor
        .coordinator()
        .intent()
        .graph()
        .node(declaration.node)
        .ok_or(EditorBootstrapError::MissingNode(declaration.node))?;
    let port = node.port_by_selector(selector).ok_or_else(|| {
        EditorBootstrapError::MissingNativePoint {
            symbol: declaration.symbol.0.clone(),
            output,
        }
    })?;
    let Some(IntentNativeBinding::Point(point)) =
        accepted.ownership.port(port.as_ref(declaration.node))
    else {
        return Err(EditorBootstrapError::MissingNativePoint {
            symbol: declaration.symbol.0.clone(),
            output,
        });
    };
    accepted
        .session
        .design_document()
        .point(point)
        .map(|point| point.position)
        .ok_or_else(|| EditorBootstrapError::MissingNativePoint {
            symbol: declaration.symbol.0.clone(),
            output,
        })
}

fn valid_managed_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && characters.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
}

fn managed_number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        serde_json::to_string(&value).expect("finite f64 has a JSON representation")
    }
}

fn intent_kind_name(kind: &IntentNodeKind) -> String {
    match kind {
        IntentNodeKind::Geometry { recipe } => format!("geometry.{recipe:?}"),
        _ => format!("{kind:?}"),
    }
}
