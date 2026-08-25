// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit conversion of accepted GUI declarations into managed source.
//!
//! Conversion is intentionally a snapshot operation, not an attempt to infer
//! fictional recipe history from arbitrary native geometry. Only declaration
//! families with a truthful managed equivalent are admitted.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use geosolve_constraint_editor::{IntentNativeBinding, ProjectionalEditorSession};
use geosolve_sketch_intent::{
    GeometryRecipeKind, InputRole, InputSlot, IntentNode, IntentNodeKind, IntentPortRole,
    IntentPortSelector, NodeId,
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
        ref kind => {
            return Err(EditorBootstrapError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: intent_kind_name(kind),
            });
        }
    }
    Ok(())
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
        if !matches!(
            source_node.kind,
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::TwoPointAlignedRectangle
            }
        ) {
            return Err(EditorBootstrapError::UnsupportedReference {
                symbol: declaration.symbol.0.clone(),
                input,
            });
        }
        let port =
            source_node
                .port(source.port)
                .ok_or(EditorBootstrapError::UnsupportedReference {
                    symbol: declaration.symbol.0.clone(),
                    input,
                })?;
        let IntentPortSelector::Node {
            role: IntentPortRole::Corner,
            index,
        } = port.selector
        else {
            return Err(EditorBootstrapError::UnsupportedReference {
                symbol: declaration.symbol.0.clone(),
                input,
            });
        };
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
        return Ok(format!("{}.corners.{member}", source_declaration.symbol.0));
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
