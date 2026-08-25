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
    GeometryRecipeKind, IntentNodeKind, IntentPortRole, IntentPortSelector, NodeId,
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
    let mut body = String::new();
    for declaration in declarations {
        let node = intent
            .graph()
            .node(declaration.node)
            .ok_or(EditorBootstrapError::MissingNode(declaration.node))?;
        match node.kind {
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
            } => {
                let lower_left = accepted_point(
                    editor,
                    declaration,
                    IntentPortSelector::Node {
                        role: IntentPortRole::Corner,
                        index: 0,
                    },
                    "lower-left corner",
                )?;
                let upper_right = accepted_point(
                    editor,
                    declaration,
                    IntentPortSelector::Node {
                        role: IntentPortRole::Corner,
                        index: 2,
                    },
                    "upper-right corner",
                )?;
                if lower_left
                    .into_iter()
                    .chain(upper_right)
                    .any(|coordinate| !coordinate.is_finite())
                {
                    return Err(EditorBootstrapError::NonFiniteGeometry {
                        symbol: declaration.symbol.0.clone(),
                    });
                }
                let lower_x = managed_number(lower_left[0]);
                let lower_y = managed_number(lower_left[1]);
                let upper_x = managed_number(upper_right[0]);
                let upper_y = managed_number(upper_right[1]);
                write!(
                    body,
                    "  const {symbol} = $.geometry.rectangle(\"{symbol}\", {{\n    lowerLeft: [{lower_x}, {lower_y}],\n    upperRight: [{upper_x}, {upper_y}],\n  }});\n",
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
