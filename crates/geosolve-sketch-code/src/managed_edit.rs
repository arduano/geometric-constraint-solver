// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use thiserror::Error;

use crate::{
    ManagedDocument, ManagedPathSegment, ManagedRewrite, ManagedSpan, ManagedValue, SemanticSymbol,
    rewrite_managed_source, rewrite_managed_value,
};

/// Closed GUI-owned edits which can be projected back into managed-v1 source
/// from syntax and semantic symbols alone.
#[derive(Clone, Debug, PartialEq)]
pub enum ManagedEdit {
    /// Replaces the complete owned argument value of one direct declaration.
    SetDeclarationArguments {
        declaration: SemanticSymbol,
        arguments: ManagedValue,
    },
    /// Replaces one object field inside a direct or custom-patch invocation.
    /// Only the parser-authenticated leaf span is rewritten, so sibling
    /// comments and formatting inside the invocation remain byte exact.
    SetInvocationArgument {
        declaration: SemanticSymbol,
        path: Vec<String>,
        value: ManagedValue,
    },
    /// Replaces one exact organization call while retaining all other source
    /// bytes and declaration order.
    SetOrganization {
        name: String,
        declarations: Vec<SemanticSymbol>,
    },
    /// Removes one declaration and all managed organization references to it.
    /// Output references are removed from the returned source as well.
    DeleteDeclaration { declaration: SemanticSymbol },
}

/// Exact-CAS source rewrite prepared from one canonical managed document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedEditPlan {
    pub target: String,
    rewrite: ManagedRewrite,
    value_target: Option<(SemanticSymbol, crate::SemanticOutputPath)>,
}

impl ManagedEditPlan {
    #[must_use]
    pub const fn rewrite(&self) -> &ManagedRewrite {
        &self.rewrite
    }
}

/// Rejected semantic reverse edit. Parse diagnostics remain owned by the
/// existing managed parser and are preserved as their typed source error.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ManagedEditError {
    #[error("managed declaration `{0}` is unavailable")]
    UnknownDeclaration(String),
    #[error("managed organization `{0}` is unavailable")]
    UnknownOrganization(String),
    #[error("managed edit path `{0}` is empty or does not traverse objects")]
    InvalidPath(String),
    #[error("managed edit references unavailable declaration `{0}`")]
    UnknownReference(String),
    #[error("managed edit cannot encode reference member paths")]
    UnsupportedReferenceMember,
    #[error("managed edit contains a non-finite number")]
    NonFiniteNumber,
    #[error("managed edit has an invalid identifier `{0}`")]
    InvalidIdentifier(String),
    #[error(transparent)]
    Parse(#[from] crate::ManagedParseError),
}

/// Prepares one semantic reverse edit against exact authenticated CST spans.
/// No source or scene authority changes until [`apply_managed_edit`] accepts
/// the plan against the same canonical document.
///
/// # Errors
///
/// Returns a typed semantic or encoding error when the requested declaration,
/// organization, reference, path, identifier, or finite value is not owned by
/// the supplied canonical managed document.
pub fn plan_managed_edit(
    document: &ManagedDocument,
    edit: ManagedEdit,
) -> Result<ManagedEditPlan, ManagedEditError> {
    let variables = document
        .program
        .declarations
        .iter()
        .map(|declaration| (declaration.symbol.clone(), declaration.variable.clone()))
        .collect::<BTreeMap<_, _>>();
    match edit {
        ManagedEdit::DeleteDeclaration { declaration } => {
            let target = document
                .program
                .declarations
                .iter()
                .find(|candidate| candidate.symbol == declaration)
                .ok_or_else(|| ManagedEditError::UnknownDeclaration(declaration.0.clone()))?;
            let mut source = document.source.clone();
            let mut spans = vec![target.statement_span];
            for organization in &document.program.organizations {
                if organization.declarations.contains(&declaration) {
                    spans.push(organization.span);
                }
            }
            spans.sort_by_key(|span| span.start);
            spans.dedup_by_key(|span| (span.start, span.end));
            for span in spans.into_iter().rev() {
                source.replace_range(span.start..span.end, "");
            }
            // The managed output envelope is intentionally simple and
            // lossless; remove the deleted binding from output object lists.
            let variable = target.variable.as_str();
            source = source
                .replace(&format!(", {variable} }}"), " }")
                .replace(&format!("{{ {variable},"), "{");
            Ok(ManagedEditPlan {
                target: format!("delete.{}", declaration.0),
                rewrite: ManagedRewrite::new(
                    document,
                    ManagedSpan::new(0, document.source.len()),
                    source,
                ),
                value_target: None,
            })
        }
        ManagedEdit::SetDeclarationArguments {
            declaration,
            arguments,
        } => {
            let target = document
                .program
                .declarations
                .iter()
                .find(|candidate| candidate.symbol == declaration)
                .ok_or_else(|| ManagedEditError::UnknownDeclaration(declaration.0.clone()))?;
            let replacement = format_managed_value(&arguments, &variables)?;
            Ok(ManagedEditPlan {
                target: format!("{}.arguments", declaration.0),
                rewrite: ManagedRewrite::new(document, target.arguments_span, replacement),
                value_target: None,
            })
        }
        ManagedEdit::SetInvocationArgument {
            declaration,
            path,
            value,
        } => {
            document
                .program
                .declarations
                .iter()
                .find(|candidate| candidate.symbol == declaration)
                .ok_or_else(|| ManagedEditError::UnknownDeclaration(declaration.0.clone()))?;
            if path.is_empty() || path.iter().any(|segment| !is_identifier(segment)) {
                return Err(ManagedEditError::InvalidPath(path.join(".")));
            }
            let semantic_path = crate::SemanticOutputPath(
                path.iter()
                    .cloned()
                    .map(ManagedPathSegment::Field)
                    .collect(),
            );
            let owned = document
                .value_owned_spans
                .iter()
                .find(|owned| {
                    owned.declaration == declaration
                        && owned.path == semantic_path
                        && owned.source_digest == document.source_digest
                })
                .ok_or_else(|| ManagedEditError::InvalidPath(path.join(".")))?;
            let replacement = format_managed_value(&value, &variables)?;
            Ok(ManagedEditPlan {
                target: format!("{}.{}", declaration.0, path.join(".")),
                rewrite: ManagedRewrite::new(document, owned.span, replacement),
                value_target: Some((declaration, semantic_path)),
            })
        }
        ManagedEdit::SetOrganization { name, declarations } => {
            let organization = document
                .program
                .organizations
                .iter()
                .find(|candidate| candidate.name == name)
                .ok_or_else(|| ManagedEditError::UnknownOrganization(name.clone()))?;
            let declaration_variables = declarations
                .iter()
                .map(|symbol| {
                    variables
                        .get(symbol)
                        .cloned()
                        .ok_or_else(|| ManagedEditError::UnknownDeclaration(symbol.0.clone()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let replacement = format_organization(&name, &declaration_variables)?;
            Ok(ManagedEditPlan {
                target: format!("organization.{name}"),
                rewrite: ManagedRewrite::new(document, organization.span, replacement),
                value_target: None,
            })
        }
    }
}

/// Applies an exact previously prepared managed edit and reparses the complete
/// bounded candidate before returning it.
///
/// # Errors
///
/// Returns a managed parse/rewrite error when the plan was prepared against
/// stale source, no longer names its authenticated span, or produces source
/// outside the bounded managed-v1 subset.
pub fn apply_managed_edit(
    document: &ManagedDocument,
    plan: &ManagedEditPlan,
) -> Result<ManagedDocument, ManagedEditError> {
    if let Some((declaration, path)) = &plan.value_target {
        if plan.rewrite.expected_source_digest != document.source_digest {
            return rewrite_managed_source(document, &plan.rewrite).map_err(Into::into);
        }
        rewrite_managed_value(
            document,
            declaration,
            path,
            plan.rewrite.replacement.clone(),
        )
        .map_err(Into::into)
    } else {
        rewrite_managed_source(document, &plan.rewrite).map_err(Into::into)
    }
}

fn format_organization(name: &str, variables: &[String]) -> Result<String, ManagedEditError> {
    if variables.iter().any(|variable| !is_identifier(variable)) {
        return Err(ManagedEditError::InvalidIdentifier(
            variables
                .iter()
                .find(|variable| !is_identifier(variable))
                .cloned()
                .unwrap_or_default(),
        ));
    }
    Ok(format!(
        "$.organize({}, [{}]);",
        quote(name),
        variables.join(", ")
    ))
}

fn format_managed_value(
    value: &ManagedValue,
    variables: &BTreeMap<SemanticSymbol, String>,
) -> Result<String, ManagedEditError> {
    match value {
        ManagedValue::Null => Ok("null".into()),
        ManagedValue::Bool(value) => Ok(value.to_string()),
        ManagedValue::Number(value) => format_number(*value),
        ManagedValue::String(value) => Ok(quote(value)),
        ManagedValue::Unit(value) => {
            if !is_identifier(&value.unit) {
                return Err(ManagedEditError::InvalidIdentifier(value.unit.clone()));
            }
            Ok(format!("{}({})", value.unit, format_number(value.value)?))
        }
        ManagedValue::Array(values) => values
            .iter()
            .map(|value| format_managed_value(value, variables))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| format!("[{}]", values.join(", "))),
        ManagedValue::Object(values) => values
            .iter()
            .map(|(key, value)| {
                let key = if is_identifier(key) {
                    key.clone()
                } else {
                    quote(key)
                };
                Ok(format!(
                    "{key}: {}",
                    format_managed_value(value, variables)?
                ))
            })
            .collect::<Result<Vec<_>, ManagedEditError>>()
            .map(|values| format!("{{ {} }}", values.join(", "))),
        ManagedValue::Reference { declaration, path } => {
            let mut reference = variables
                .get(declaration)
                .cloned()
                .ok_or_else(|| ManagedEditError::UnknownReference(declaration.0.clone()))?;
            for segment in &path.0 {
                match segment {
                    ManagedPathSegment::Field(field) if is_identifier(field) => {
                        reference.push('.');
                        reference.push_str(field);
                    }
                    ManagedPathSegment::Index(index) => {
                        reference.push('[');
                        reference.push_str(&index.to_string());
                        reference.push(']');
                    }
                    ManagedPathSegment::Field(field) => {
                        return Err(ManagedEditError::InvalidIdentifier(field.clone()));
                    }
                    ManagedPathSegment::Member { .. } => {
                        return Err(ManagedEditError::UnsupportedReferenceMember);
                    }
                }
            }
            Ok(reference)
        }
    }
}

fn format_number(value: f64) -> Result<String, ManagedEditError> {
    if !value.is_finite() {
        return Err(ManagedEditError::NonFiniteNumber);
    }
    if value == 0.0 && value.is_sign_negative() {
        return Ok("-0".into());
    }
    Ok(value.to_string())
}

fn quote(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a Rust string cannot fail")
}

fn is_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManagedOrganization, UnitLiteral, parse_managed_source};

    const SOURCE: &str = r#"// retained header
"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { roundEveryCorner } from "./patches/round.patch.ts";

export default sketch(($) => {
  // direct authoring comment survives
  const path = $.geometry.polyline("path", {
    vertices: [{ key: "a", position: [0, 0] }, { key: "b", position: [4, 0] }],
    closed: false,
  });
  const rounded = $.use("rounded", roundEveryCorner, {
    corners: path.filletableCorners,
    // lens-adjacent formatting survives
    radius: mm(0.4),
  });
  $.organize("Profile", [path, rounded]);
  return $.outputs({ path, rounded });
});
"#;

    #[test]
    fn invocation_lens_rewrites_only_authenticated_arguments_and_is_stale_safe() {
        let document = parse_managed_source(SOURCE).unwrap();
        let plan = plan_managed_edit(
            &document,
            ManagedEdit::SetInvocationArgument {
                declaration: SemanticSymbol("rounded".into()),
                path: vec!["radius".into()],
                value: ManagedValue::Unit(UnitLiteral {
                    unit: "mm".into(),
                    value: 1.25,
                }),
            },
        )
        .unwrap();
        let rewritten = apply_managed_edit(&document, &plan).unwrap();
        assert!(rewritten.source.contains("radius: mm(1.25)"));
        assert!(
            rewritten
                .source
                .contains("corners: path.filletableCorners,")
        );
        assert!(
            rewritten
                .source
                .contains("// lens-adjacent formatting survives")
        );
        assert!(
            rewritten
                .source
                .contains("// direct authoring comment survives")
        );
        assert_eq!(
            rewritten.program.declarations[1].arguments,
            rewritten.program.declarations[1]
                .patch
                .as_ref()
                .unwrap()
                .arguments,
        );
        assert!(matches!(
            apply_managed_edit(&rewritten, &plan),
            Err(ManagedEditError::Parse(crate::ManagedParseError {
                diagnostic: crate::ManagedDiagnostic {
                    code: crate::ManagedDiagnosticCode::RewriteStale,
                    ..
                }
            }))
        ));
    }

    #[test]
    fn sequential_invocation_leaf_edits_reauthenticate_the_reparsed_source() {
        let source = SOURCE.replace(
            "corners: path.filletableCorners,",
            "corners: path.filletableCorners,\n    offset: [4, 8],",
        );
        let mut document = parse_managed_source(&source).unwrap();
        for (path, value) in [
            (
                "radius",
                ManagedValue::Unit(UnitLiteral {
                    unit: "mm".into(),
                    value: 0.75,
                }),
            ),
            (
                "offset",
                ManagedValue::Array(vec![ManagedValue::Number(6.0), ManagedValue::Number(9.0)]),
            ),
        ] {
            let plan = plan_managed_edit(
                &document,
                ManagedEdit::SetInvocationArgument {
                    declaration: SemanticSymbol("rounded".into()),
                    path: vec![path.into()],
                    value,
                },
            )
            .unwrap();
            document = apply_managed_edit(&document, &plan).unwrap();
        }
        assert!(document.source.contains("radius: mm(0.75)"));
        assert!(document.source.contains("offset: [6, 9]"));
        assert!(
            document
                .source
                .contains("// lens-adjacent formatting survives")
        );
    }

    #[test]
    fn organization_rewrite_uses_semantic_symbols_not_wire_ids() {
        let document = parse_managed_source(SOURCE).unwrap();
        let plan = plan_managed_edit(
            &document,
            ManagedEdit::SetOrganization {
                name: "Profile".into(),
                declarations: vec![
                    SemanticSymbol("rounded".into()),
                    SemanticSymbol("path".into()),
                ],
            },
        )
        .unwrap();
        let rewritten = apply_managed_edit(&document, &plan).unwrap();
        assert!(
            rewritten
                .source
                .contains("$.organize(\"Profile\", [rounded, path]);")
        );
        assert_eq!(
            rewritten.program.organizations,
            [ManagedOrganization {
                name: "Profile".into(),
                declarations: vec![
                    SemanticSymbol("rounded".into()),
                    SemanticSymbol("path".into()),
                ],
                span: rewritten.program.organizations[0].span,
            }]
        );
    }
}
