// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_intent::intent_content_digest;
use thiserror::Error;

use crate::{
    AuthoringDeclaration, AuthoringProgram, ManagedDiagnostic, ManagedDiagnosticCode,
    ManagedDocument, ManagedImport, ManagedOrganization, ManagedOutput, ManagedOwnedSpan,
    ManagedOwnedSpanKind, ManagedPathSegment, ManagedSpan, ManagedValue, ManagedValueOwnedSpan,
    PatchInvocation, SemanticOutputPath, SemanticSymbol, UnitLiteral,
};

/// Maximum admitted byte length of the GUI-managed TypeScript file.
pub const MANAGED_SOURCE_LIMIT: usize = 4 * 1024 * 1024;

/// Maximum recursive nesting admitted for managed arrays and objects.
pub const MANAGED_VALUE_DEPTH_LIMIT: usize = 64;

/// Maximum number of literal/reference/container values in one managed file.
pub const MANAGED_VALUE_NODE_LIMIT: usize = 16_384;

/// Maximum aggregate number of entries across all managed arrays and objects.
pub const MANAGED_COLLECTION_ITEM_LIMIT: usize = 8_192;

const DIRECTIVE: &str = "\"use geosolve managed-v1\"";

/// One rejected managed-subset parse or authenticated rewrite.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{diagnostic}", diagnostic = .diagnostic.message)]
pub struct ManagedParseError {
    pub diagnostic: ManagedDiagnostic,
}

/// Exact-CAS replacement of one parser-authenticated source span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedRewrite {
    pub expected_source_digest: String,
    pub span: ManagedSpan,
    pub expected_text: String,
    pub replacement: String,
}

impl ManagedRewrite {
    #[must_use]
    pub fn new(
        document: &ManagedDocument,
        span: ManagedSpan,
        replacement: impl Into<String>,
    ) -> Self {
        let expected_text = document
            .source
            .get(span.start..span.end)
            .unwrap_or_default()
            .to_owned();
        Self {
            expected_source_digest: document.source_digest.clone(),
            span,
            expected_text,
            replacement: replacement.into(),
        }
    }
}

/// Parses one bounded managed-v1 source file while retaining its exact bytes
/// and every GUI-owned source coordinate.
///
/// # Errors
///
/// Returns a precise spanned diagnostic for a resource violation or any token
/// outside the closed managed-v1 subset.
pub fn parse_managed_source(source: &str) -> Result<ManagedDocument, ManagedParseError> {
    if source.len() > MANAGED_SOURCE_LIMIT {
        return Err(diagnostic_error(
            source,
            ManagedDiagnosticCode::SourceTooLarge,
            format!(
                "managed source is {} bytes; the limit is {MANAGED_SOURCE_LIMIT}",
                source.len()
            ),
            ManagedSpan::new(0, source.len()),
        ));
    }
    Parser::new(source).parse()
}

/// Replaces exactly one authenticated owned span, reparses the whole bounded
/// candidate and returns a new lossless document. No surrounding trivia is
/// regenerated.
///
/// # Errors
///
/// Returns a precise diagnostic for stale authority, an invalid UTF-8 span,
/// an unowned span, a resource violation or an invalid rewritten candidate.
pub fn rewrite_managed_source(
    document: &ManagedDocument,
    rewrite: &ManagedRewrite,
) -> Result<ManagedDocument, ManagedParseError> {
    if rewrite.expected_source_digest != document.source_digest {
        return Err(diagnostic_error(
            &document.source,
            ManagedDiagnosticCode::RewriteStale,
            "managed rewrite was prepared against stale source",
            rewrite.span,
        ));
    }
    if rewrite.span.start > rewrite.span.end
        || rewrite.span.end > document.source.len()
        || !document.source.is_char_boundary(rewrite.span.start)
        || !document.source.is_char_boundary(rewrite.span.end)
    {
        return Err(diagnostic_error(
            &document.source,
            ManagedDiagnosticCode::InvalidUtf8Boundary,
            "managed rewrite span is not a valid UTF-8 source range",
            rewrite.span,
        ));
    }
    if !document
        .owned_spans
        .iter()
        .any(|owned| owned.span == rewrite.span && owned.source_digest == document.source_digest)
    {
        return Err(diagnostic_error(
            &document.source,
            ManagedDiagnosticCode::RewriteStale,
            "managed rewrite does not target an authenticated owned span",
            rewrite.span,
        ));
    }
    let current = &document.source[rewrite.span.start..rewrite.span.end];
    if current != rewrite.expected_text {
        return Err(diagnostic_error(
            &document.source,
            ManagedDiagnosticCode::RewriteStale,
            "managed rewrite expected bytes do not match canonical source",
            rewrite.span,
        ));
    }
    let candidate_len = document
        .source
        .len()
        .saturating_sub(rewrite.span.len())
        .saturating_add(rewrite.replacement.len());
    if candidate_len > MANAGED_SOURCE_LIMIT {
        return Err(diagnostic_error(
            &document.source,
            ManagedDiagnosticCode::SourceTooLarge,
            format!("rewritten managed source would be {candidate_len} bytes"),
            rewrite.span,
        ));
    }
    let mut candidate = String::with_capacity(candidate_len);
    candidate.push_str(&document.source[..rewrite.span.start]);
    candidate.push_str(&rewrite.replacement);
    candidate.push_str(&document.source[rewrite.span.end..]);
    parse_managed_source(&candidate)
}

/// Rewrites one parser-authenticated declaration-relative value leaf.
///
/// This is the bounded edit-lens entry point: a caller identifies the managed
/// invocation and argument path semantically, while the parser remains sole
/// authority for its exact current byte range. Surrounding comments and
/// formatting are never regenerated.
///
/// # Errors
///
/// Returns a stale-coordinate diagnostic when the declaration/path is absent
/// or ambiguous, plus the ordinary exact rewrite/reparse diagnostics.
pub fn rewrite_managed_value(
    document: &ManagedDocument,
    declaration: &SemanticSymbol,
    path: &SemanticOutputPath,
    replacement: impl Into<String>,
) -> Result<ManagedDocument, ManagedParseError> {
    let mut matches = document.value_owned_spans.iter().filter(|owned| {
        &owned.declaration == declaration
            && &owned.path == path
            && owned.source_digest == document.source_digest
    });
    let Some(owned) = matches.next() else {
        return Err(diagnostic_error(
            &document.source,
            ManagedDiagnosticCode::RewriteStale,
            format!(
                "managed declaration `{}` has no authenticated value at the requested path",
                declaration.0
            ),
            ManagedSpan::new(0, 0),
        ));
    };
    if matches.next().is_some() {
        return Err(diagnostic_error(
            &document.source,
            ManagedDiagnosticCode::RewriteStale,
            format!(
                "managed declaration `{}` has an ambiguous value path",
                declaration.0
            ),
            owned.span,
        ));
    }
    let mut authenticated = document.clone();
    if !authenticated
        .owned_spans
        .iter()
        .any(|candidate| candidate.span == owned.span)
    {
        authenticated.owned_spans.push(ManagedOwnedSpan {
            kind: owned.kind,
            span: owned.span,
            source_digest: owned.source_digest.clone(),
        });
    }
    rewrite_managed_source(
        &authenticated,
        &ManagedRewrite::new(document, owned.span, replacement),
    )
}

struct Parser<'a> {
    source: &'a str,
    position: usize,
    imports: Vec<ManagedImport>,
    imported_bindings: BTreeSet<String>,
    declarations: Vec<AuthoringDeclaration>,
    declared: BTreeMap<String, SemanticSymbol>,
    symbols: BTreeSet<SemanticSymbol>,
    organizations: Vec<ManagedOrganization>,
    outputs: Vec<ManagedOutput>,
    owned: Vec<(ManagedOwnedSpanKind, ManagedSpan)>,
    value_owned: Vec<(
        SemanticSymbol,
        SemanticOutputPath,
        ManagedOwnedSpanKind,
        ManagedSpan,
    )>,
    value_nodes: usize,
    collection_items: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            imports: Vec::new(),
            imported_bindings: BTreeSet::new(),
            declarations: Vec::new(),
            declared: BTreeMap::new(),
            symbols: BTreeSet::new(),
            organizations: Vec::new(),
            outputs: Vec::new(),
            owned: Vec::new(),
            value_owned: Vec::new(),
            value_nodes: 0,
            collection_items: 0,
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed top-level grammar is clearer as one auditable state transition"
    )]
    fn parse(mut self) -> Result<ManagedDocument, ManagedParseError> {
        self.skip_trivia()?;
        if !self.remaining().starts_with(DIRECTIVE) {
            return self.fail(
                ManagedDiagnosticCode::MissingDirective,
                "managed source must begin with the exact directive \"use geosolve managed-v1\";",
                self.point_span(),
            );
        }
        let directive_start = self.position;
        self.position += DIRECTIVE.len();
        if !self.consume_punct(';')? {
            return self.fail(
                ManagedDiagnosticCode::InvalidDirective,
                "managed-v1 directive must end with a semicolon",
                ManagedSpan::new(directive_start, self.position),
            );
        }
        self.skip_trivia()?;
        while self.peek_keyword("import") {
            self.parse_import()?;
            self.skip_trivia()?;
        }

        let envelope_start = self.position;
        self.expect_keyword("export", ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_keyword("default", ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_keyword("sketch", ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_punct('(', ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_punct('(', ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_punct('$', ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_punct(')', ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_punct('=', ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_punct('>', ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_punct('{', ManagedDiagnosticCode::InvalidSketchEnvelope)?;

        let mut saw_outputs = false;
        loop {
            self.skip_trivia()?;
            if self.peek_keyword("const") {
                if saw_outputs {
                    return self
                        .unsupported("declarations after the output record are not allowed");
                }
                self.parse_declaration()?;
            } else if self.remaining().starts_with("$.organize") {
                if saw_outputs {
                    return self.unsupported("organization after the output record is not allowed");
                }
                self.parse_organization()?;
            } else if self.peek_keyword("return") {
                if saw_outputs {
                    return self.fail(
                        ManagedDiagnosticCode::MissingOutputs,
                        "managed source has more than one output record",
                        self.point_span(),
                    );
                }
                self.parse_outputs()?;
                saw_outputs = true;
            } else if self.remaining().starts_with('}') {
                break;
            } else {
                return self.unsupported(
                    "managed sketch body accepts only const declarations, $.organize and return $.outputs",
                );
            }
        }
        if !saw_outputs {
            return self.fail(
                ManagedDiagnosticCode::MissingOutputs,
                "managed sketch requires one final return $.outputs({...})",
                self.point_span(),
            );
        }
        self.expect_punct('}', ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_punct(')', ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        self.expect_punct(';', ManagedDiagnosticCode::InvalidSketchEnvelope)?;
        let envelope_end = self.position;
        self.skip_trivia()?;
        if self.position != self.source.len() {
            return self.fail(
                ManagedDiagnosticCode::TrailingSource,
                "unexpected source after managed sketch envelope",
                ManagedSpan::new(self.position, self.source.len()),
            );
        }

        let digest = intent_content_digest(self.source.as_bytes()).to_string();
        let owned_spans = self
            .owned
            .into_iter()
            .map(|(kind, span)| ManagedOwnedSpan {
                kind,
                span,
                source_digest: digest.clone(),
            })
            .collect();
        let value_owned_spans = self
            .value_owned
            .into_iter()
            .map(|(declaration, path, kind, span)| ManagedValueOwnedSpan {
                declaration,
                path,
                kind,
                span,
                source_digest: digest.clone(),
            })
            .collect();
        Ok(ManagedDocument {
            source: self.source.to_owned(),
            source_digest: digest,
            imports: self.imports,
            program: AuthoringProgram {
                declarations: self.declarations,
                organizations: self.organizations,
                outputs: self.outputs,
            },
            envelope_span: ManagedSpan::new(envelope_start, envelope_end),
            owned_spans,
            value_owned_spans,
        })
    }

    fn parse_import(&mut self) -> Result<(), ManagedParseError> {
        let start = self.position;
        self.expect_keyword("import", ManagedDiagnosticCode::InvalidImport)?;
        let mut bindings = Vec::new();
        if self.consume_punct('{')? {
            loop {
                self.skip_trivia()?;
                if self.consume_punct('}')? {
                    break;
                }
                let imported = self.parse_identifier(ManagedDiagnosticCode::InvalidImport)?;
                let local = if self.consume_keyword("as")? {
                    self.parse_identifier(ManagedDiagnosticCode::InvalidImport)?
                } else {
                    imported
                };
                if !self.imported_bindings.insert(local.clone()) {
                    return self.fail(
                        ManagedDiagnosticCode::InvalidImport,
                        format!("duplicate imported binding `{local}`"),
                        ManagedSpan::new(start, self.position),
                    );
                }
                bindings.push(local);
                if self.consume_punct(',')? {
                    continue;
                }
                self.expect_punct('}', ManagedDiagnosticCode::InvalidImport)?;
                break;
            }
        } else {
            let binding = self.parse_identifier(ManagedDiagnosticCode::InvalidImport)?;
            self.imported_bindings.insert(binding.clone());
            bindings.push(binding);
        }
        self.expect_keyword("from", ManagedDiagnosticCode::InvalidImport)?;
        let (module, _) = self.parse_string(ManagedDiagnosticCode::InvalidImport)?;
        self.expect_punct(';', ManagedDiagnosticCode::InvalidImport)?;
        self.imports.push(ManagedImport {
            module,
            bindings,
            span: ManagedSpan::new(start, self.position),
        });
        Ok(())
    }

    fn parse_declaration(&mut self) -> Result<(), ManagedParseError> {
        let statement_start = self.position;
        self.expect_keyword("const", ManagedDiagnosticCode::UnexpectedToken)?;
        let variable = self.parse_identifier(ManagedDiagnosticCode::UnexpectedToken)?;
        if self.declared.contains_key(&variable) {
            return self.fail(
                ManagedDiagnosticCode::DuplicateSymbol,
                format!("duplicate declaration variable `{variable}`"),
                ManagedSpan::new(statement_start, self.position),
            );
        }
        self.expect_punct('=', ManagedDiagnosticCode::UnexpectedToken)?;
        self.expect_punct('$', ManagedDiagnosticCode::UnexpectedToken)?;
        self.expect_punct('.', ManagedDiagnosticCode::UnexpectedToken)?;
        let mut builder_path = vec![self.parse_identifier(ManagedDiagnosticCode::UnexpectedToken)?];
        while self.consume_punct('.')? {
            builder_path.push(self.parse_identifier(ManagedDiagnosticCode::UnexpectedToken)?);
        }
        self.expect_punct('(', ManagedDiagnosticCode::UnexpectedToken)?;
        let (symbol_text, symbol_span) =
            self.parse_string(ManagedDiagnosticCode::UnexpectedToken)?;
        let symbol = SemanticSymbol(symbol_text);
        if !self.symbols.insert(symbol.clone()) {
            return self.fail(
                ManagedDiagnosticCode::DuplicateSymbol,
                format!("duplicate stable semantic symbol `{}`", symbol.0),
                symbol_span,
            );
        }
        self.expect_punct(',', ManagedDiagnosticCode::UnexpectedToken)?;
        let mut patch = None;
        let (arguments, arguments_span) = if builder_path == ["use"] {
            let module_binding = self.parse_identifier(ManagedDiagnosticCode::UnexpectedToken)?;
            if !self.imported_bindings.contains(&module_binding) {
                return self.fail(
                    ManagedDiagnosticCode::ForwardReference,
                    format!("patch binding `{module_binding}` is not statically imported"),
                    self.point_span(),
                );
            }
            self.expect_punct(',', ManagedDiagnosticCode::UnexpectedToken)?;
            let parsed = self.parse_value(&symbol)?;
            patch = Some(PatchInvocation {
                module_binding,
                arguments: parsed.0.clone(),
            });
            parsed
        } else {
            self.parse_value(&symbol)?
        };
        self.expect_punct(')', ManagedDiagnosticCode::UnexpectedToken)?;
        self.expect_punct(';', ManagedDiagnosticCode::UnexpectedToken)?;
        let statement_span = ManagedSpan::new(statement_start, self.position);
        self.owned.extend([
            (ManagedOwnedSpanKind::Declaration, statement_span),
            (ManagedOwnedSpanKind::Symbol, symbol_span),
            (ManagedOwnedSpanKind::Arguments, arguments_span),
        ]);
        self.declared.insert(variable.clone(), symbol.clone());
        self.declarations.push(AuthoringDeclaration {
            variable,
            symbol,
            builder_path,
            arguments,
            patch,
            statement_span,
            symbol_span,
            arguments_span,
        });
        Ok(())
    }

    fn parse_organization(&mut self) -> Result<(), ManagedParseError> {
        let start = self.position;
        self.expect_exact("$.organize", ManagedDiagnosticCode::UnexpectedToken)?;
        self.expect_punct('(', ManagedDiagnosticCode::UnexpectedToken)?;
        let (name, _) = self.parse_string(ManagedDiagnosticCode::UnexpectedToken)?;
        self.expect_punct(',', ManagedDiagnosticCode::UnexpectedToken)?;
        self.expect_punct('[', ManagedDiagnosticCode::UnexpectedToken)?;
        let mut declarations = Vec::new();
        loop {
            self.skip_trivia()?;
            if self.consume_punct(']')? {
                break;
            }
            let variable = self.parse_identifier(ManagedDiagnosticCode::UnexpectedToken)?;
            let Some(symbol) = self.declared.get(&variable) else {
                return self.fail(
                    ManagedDiagnosticCode::ForwardReference,
                    format!("organization references unknown declaration `{variable}`"),
                    self.point_span(),
                );
            };
            declarations.push(symbol.clone());
            if self.consume_punct(',')? {
                continue;
            }
            self.expect_punct(']', ManagedDiagnosticCode::UnexpectedToken)?;
            break;
        }
        self.expect_punct(')', ManagedDiagnosticCode::UnexpectedToken)?;
        self.expect_punct(';', ManagedDiagnosticCode::UnexpectedToken)?;
        let span = ManagedSpan::new(start, self.position);
        self.owned.push((ManagedOwnedSpanKind::Organization, span));
        self.organizations.push(ManagedOrganization {
            name,
            declarations,
            span,
        });
        Ok(())
    }

    fn parse_outputs(&mut self) -> Result<(), ManagedParseError> {
        let start = self.position;
        self.expect_keyword("return", ManagedDiagnosticCode::MissingOutputs)?;
        self.expect_exact("$.outputs", ManagedDiagnosticCode::MissingOutputs)?;
        self.expect_punct('(', ManagedDiagnosticCode::MissingOutputs)?;
        self.expect_punct('{', ManagedDiagnosticCode::MissingOutputs)?;
        let mut names = BTreeSet::new();
        loop {
            self.skip_trivia()?;
            if self.consume_punct('}')? {
                break;
            }
            let name = self.parse_property_key()?;
            if !names.insert(name.clone()) {
                return self.fail(
                    ManagedDiagnosticCode::DuplicateObjectKey,
                    format!("duplicate output key `{name}`"),
                    self.point_span(),
                );
            }
            let (declaration, path) = if self.consume_punct(':')? {
                self.parse_reference()?
            } else {
                let Some(symbol) = self.declared.get(&name) else {
                    return self.fail(
                        ManagedDiagnosticCode::ForwardReference,
                        format!("output shorthand references unknown declaration `{name}`"),
                        self.point_span(),
                    );
                };
                (symbol.clone(), SemanticOutputPath::default())
            };
            self.outputs.push(ManagedOutput {
                name,
                declaration,
                path,
            });
            if self.consume_punct(',')? {
                continue;
            }
            self.expect_punct('}', ManagedDiagnosticCode::MissingOutputs)?;
            break;
        }
        self.expect_punct(')', ManagedDiagnosticCode::MissingOutputs)?;
        self.expect_punct(';', ManagedDiagnosticCode::MissingOutputs)?;
        self.owned.push((
            ManagedOwnedSpanKind::Outputs,
            ManagedSpan::new(start, self.position),
        ));
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one recursive value recognizer keeps the deliberately closed syntax exhaustive"
    )]
    fn parse_value(
        &mut self,
        declaration: &SemanticSymbol,
    ) -> Result<(ManagedValue, ManagedSpan), ManagedParseError> {
        self.parse_value_at_depth(0, declaration, &SemanticOutputPath::default())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed recursive grammar and its pre-descent resource guards are audited together"
    )]
    fn parse_value_at_depth(
        &mut self,
        depth: usize,
        declaration: &SemanticSymbol,
        path: &SemanticOutputPath,
    ) -> Result<(ManagedValue, ManagedSpan), ManagedParseError> {
        self.skip_trivia()?;
        let start = self.position;
        if depth > MANAGED_VALUE_DEPTH_LIMIT {
            return self.fail(
                ManagedDiagnosticCode::ResourceLimit,
                format!(
                    "managed value nesting exceeds the depth limit of {MANAGED_VALUE_DEPTH_LIMIT}"
                ),
                self.point_span(),
            );
        }
        self.value_nodes = self.value_nodes.saturating_add(1);
        if self.value_nodes > MANAGED_VALUE_NODE_LIMIT {
            return self.fail(
                ManagedDiagnosticCode::ResourceLimit,
                format!(
                    "managed source exceeds the value-node limit of {MANAGED_VALUE_NODE_LIMIT}"
                ),
                self.point_span(),
            );
        }
        let value = match self.peek_byte() {
            Some(b'"' | b'\'') => {
                let (value, span) = self.parse_string(ManagedDiagnosticCode::UnexpectedToken)?;
                self.owned.push((ManagedOwnedSpanKind::Literal, span));
                ManagedValue::String(value)
            }
            Some(b'[') => {
                self.position += 1;
                let mut values = Vec::new();
                loop {
                    self.skip_trivia()?;
                    if self.consume_punct(']')? {
                        break;
                    }
                    self.consume_collection_item()?;
                    let mut child_path = path.clone();
                    child_path.0.push(ManagedPathSegment::Index(values.len()));
                    values.push(
                        self.parse_value_at_depth(depth + 1, declaration, &child_path)?
                            .0,
                    );
                    if self.consume_punct(',')? {
                        continue;
                    }
                    self.expect_punct(']', ManagedDiagnosticCode::UnexpectedToken)?;
                    break;
                }
                ManagedValue::Array(values)
            }
            Some(b'{') => {
                self.position += 1;
                let mut values = BTreeMap::new();
                loop {
                    self.skip_trivia()?;
                    if self.consume_punct('}')? {
                        break;
                    }
                    let key = self.parse_property_key()?;
                    if values.contains_key(&key) {
                        return self.fail(
                            ManagedDiagnosticCode::DuplicateObjectKey,
                            format!("duplicate object key `{key}`"),
                            self.point_span(),
                        );
                    }
                    self.consume_collection_item()?;
                    self.expect_punct(':', ManagedDiagnosticCode::UnexpectedToken)?;
                    let mut child_path = path.clone();
                    child_path.0.push(ManagedPathSegment::Field(key.clone()));
                    values.insert(
                        key,
                        self.parse_value_at_depth(depth + 1, declaration, &child_path)?
                            .0,
                    );
                    if self.consume_punct(',')? {
                        continue;
                    }
                    self.expect_punct('}', ManagedDiagnosticCode::UnexpectedToken)?;
                    break;
                }
                ManagedValue::Object(values)
            }
            Some(byte) if byte.is_ascii_digit() || byte == b'-' => {
                let value = self.parse_number()?;
                let span = ManagedSpan::new(start, self.position);
                self.owned.push((ManagedOwnedSpanKind::Literal, span));
                ManagedValue::Number(value)
            }
            Some(byte) if is_identifier_start(byte) => {
                let identifier = self.parse_identifier(ManagedDiagnosticCode::UnexpectedToken)?;
                match identifier.as_str() {
                    "true" => ManagedValue::Bool(true),
                    "false" => ManagedValue::Bool(false),
                    "null" => ManagedValue::Null,
                    unit if self.consume_punct('(')? => {
                        if !matches!(unit, "mm" | "cm" | "m" | "inch" | "deg" | "rad") {
                            return self.unsupported(
                                "only closed unit constructors are allowed in managed values",
                            );
                        }
                        let value = self.parse_number()?;
                        self.expect_punct(')', ManagedDiagnosticCode::UnexpectedToken)?;
                        ManagedValue::Unit(UnitLiteral {
                            unit: unit.to_owned(),
                            value,
                        })
                    }
                    _ => {
                        self.position = start;
                        let (declaration, path) = self.parse_reference()?;
                        self.owned.push((
                            ManagedOwnedSpanKind::Reference,
                            ManagedSpan::new(start, self.position),
                        ));
                        ManagedValue::Reference { declaration, path }
                    }
                }
            }
            _ => {
                return self.fail(
                    ManagedDiagnosticCode::UnexpectedToken,
                    "expected a finite literal, object, array or earlier declaration reference",
                    self.point_span(),
                );
            }
        };
        let span = ManagedSpan::new(start, self.position);
        if matches!(
            value,
            ManagedValue::Bool(_) | ManagedValue::Null | ManagedValue::Unit(_)
        ) {
            self.owned.push((ManagedOwnedSpanKind::Literal, span));
        }
        let kind = if matches!(value, ManagedValue::Reference { .. }) {
            ManagedOwnedSpanKind::Reference
        } else {
            ManagedOwnedSpanKind::Literal
        };
        // Every recursively parsed value owns one exact semantic path, not
        // only scalar leaves. Aggregate ownership lets a GUI rewrite an
        // authenticated point/record as one value while preserving every
        // sibling comment and surrounding byte.
        self.value_owned
            .push((declaration.clone(), path.clone(), kind, span));
        Ok((value, span))
    }

    fn consume_collection_item(&mut self) -> Result<(), ManagedParseError> {
        self.collection_items = self.collection_items.saturating_add(1);
        if self.collection_items > MANAGED_COLLECTION_ITEM_LIMIT {
            return self.fail(
                ManagedDiagnosticCode::ResourceLimit,
                format!(
                    "managed source exceeds the aggregate collection-item limit of {MANAGED_COLLECTION_ITEM_LIMIT}"
                ),
                self.point_span(),
            );
        }
        Ok(())
    }

    fn parse_reference(
        &mut self,
    ) -> Result<(SemanticSymbol, SemanticOutputPath), ManagedParseError> {
        let variable = self.parse_identifier(ManagedDiagnosticCode::UnexpectedToken)?;
        let Some(symbol) = self.declared.get(&variable).cloned() else {
            return self.fail(
                ManagedDiagnosticCode::ForwardReference,
                format!("reference `{variable}` does not name an earlier declaration"),
                self.point_span(),
            );
        };
        let mut path = Vec::new();
        loop {
            if self.consume_punct('.')? {
                path.push(ManagedPathSegment::Field(
                    self.parse_identifier(ManagedDiagnosticCode::UnexpectedToken)?,
                ));
            } else if self.consume_punct('[')? {
                self.skip_trivia()?;
                let index_start = self.position;
                while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
                    self.position += 1;
                }
                if index_start == self.position {
                    return self.unsupported("managed reference indices must be integer literals");
                }
                let index = self.source[index_start..self.position]
                    .parse::<usize>()
                    .map_err(|_| {
                        diagnostic_error(
                            self.source,
                            ManagedDiagnosticCode::UnexpectedToken,
                            "managed reference index is out of range",
                            ManagedSpan::new(index_start, self.position),
                        )
                    })?;
                self.expect_punct(']', ManagedDiagnosticCode::UnexpectedToken)?;
                path.push(ManagedPathSegment::Index(index));
            } else {
                break;
            }
        }
        Ok((symbol, SemanticOutputPath(path)))
    }

    fn parse_property_key(&mut self) -> Result<String, ManagedParseError> {
        self.skip_trivia()?;
        if matches!(self.peek_byte(), Some(b'"' | b'\'')) {
            return self
                .parse_string(ManagedDiagnosticCode::UnexpectedToken)
                .map(|value| value.0);
        }
        self.parse_identifier(ManagedDiagnosticCode::UnexpectedToken)
    }

    fn parse_number(&mut self) -> Result<f64, ManagedParseError> {
        self.skip_trivia()?;
        let start = self.position;
        if self.peek_byte() == Some(b'-') {
            self.position += 1;
        }
        let integer_start = self.position;
        while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        if integer_start == self.position {
            return self.fail(
                ManagedDiagnosticCode::UnexpectedToken,
                "expected decimal number",
                self.point_span(),
            );
        }
        if self.peek_byte() == Some(b'.') {
            self.position += 1;
            let fraction_start = self.position;
            while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
                self.position += 1;
            }
            if fraction_start == self.position {
                return self.fail(
                    ManagedDiagnosticCode::UnexpectedToken,
                    "decimal point must be followed by digits",
                    self.point_span(),
                );
            }
        }
        if matches!(self.peek_byte(), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.peek_byte(), Some(b'+' | b'-')) {
                self.position += 1;
            }
            let exponent_start = self.position;
            while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
                self.position += 1;
            }
            if exponent_start == self.position {
                return self.fail(
                    ManagedDiagnosticCode::UnexpectedToken,
                    "number exponent must contain digits",
                    self.point_span(),
                );
            }
        }
        let span = ManagedSpan::new(start, self.position);
        let parsed = self.source[start..self.position]
            .parse::<f64>()
            .map_err(|_| {
                diagnostic_error(
                    self.source,
                    ManagedDiagnosticCode::UnexpectedToken,
                    "invalid decimal number",
                    span,
                )
            })?;
        if !parsed.is_finite() {
            return self.fail(
                ManagedDiagnosticCode::NonFiniteLiteral,
                "managed numerical literals must be finite",
                span,
            );
        }
        Ok(parsed)
    }

    fn parse_string(
        &mut self,
        code: ManagedDiagnosticCode,
    ) -> Result<(String, ManagedSpan), ManagedParseError> {
        self.skip_trivia()?;
        let start = self.position;
        let Some(quote @ (b'"' | b'\'')) = self.peek_byte() else {
            return self.fail(code, "expected static string literal", self.point_span());
        };
        self.position += 1;
        let mut value = String::new();
        while let Some(byte) = self.peek_byte() {
            if byte == quote {
                self.position += 1;
                return Ok((value, ManagedSpan::new(start, self.position)));
            }
            if byte == b'\n' || byte == b'\r' {
                return self.fail(
                    code,
                    "managed strings cannot contain a raw newline",
                    ManagedSpan::new(start, self.position),
                );
            }
            if byte == b'\\' {
                self.position += 1;
                let Some(escaped) = self.peek_byte() else {
                    return self.fail(code, "unterminated string escape", self.point_span());
                };
                let character = match escaped {
                    b'\\' => '\\',
                    b'"' => '"',
                    b'\'' => '\'',
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    _ => {
                        return self.unsupported(
                            "managed strings support only quote, slash, n, r and t escapes",
                        );
                    }
                };
                value.push(character);
                self.position += 1;
                continue;
            }
            let remaining = &self.source[self.position..];
            let Some(character) = remaining.chars().next() else {
                break;
            };
            value.push(character);
            self.position += character.len_utf8();
        }
        self.fail(
            code,
            "unterminated static string literal",
            ManagedSpan::new(start, self.position),
        )
    }

    fn parse_identifier(
        &mut self,
        code: ManagedDiagnosticCode,
    ) -> Result<String, ManagedParseError> {
        self.skip_trivia()?;
        let start = self.position;
        if !self.peek_byte().is_some_and(is_identifier_start) {
            return self.fail(code, "expected static identifier", self.point_span());
        }
        self.position += 1;
        while self.peek_byte().is_some_and(is_identifier_continue) {
            self.position += 1;
        }
        Ok(self.source[start..self.position].to_owned())
    }

    fn expect_keyword(
        &mut self,
        keyword: &str,
        code: ManagedDiagnosticCode,
    ) -> Result<(), ManagedParseError> {
        self.skip_trivia()?;
        if self.peek_keyword(keyword) {
            self.position += keyword.len();
            Ok(())
        } else {
            self.fail(code, format!("expected `{keyword}`"), self.point_span())
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> Result<bool, ManagedParseError> {
        self.skip_trivia()?;
        if self.peek_keyword(keyword) {
            self.position += keyword.len();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn peek_keyword(&self, keyword: &str) -> bool {
        self.remaining().starts_with(keyword)
            && self
                .source
                .as_bytes()
                .get(self.position + keyword.len())
                .is_none_or(|byte| !is_identifier_continue(*byte))
    }

    fn expect_exact(
        &mut self,
        expected: &str,
        code: ManagedDiagnosticCode,
    ) -> Result<(), ManagedParseError> {
        self.skip_trivia()?;
        if self.remaining().starts_with(expected) {
            self.position += expected.len();
            Ok(())
        } else {
            self.fail(code, format!("expected `{expected}`"), self.point_span())
        }
    }

    fn expect_punct(
        &mut self,
        punct: char,
        code: ManagedDiagnosticCode,
    ) -> Result<(), ManagedParseError> {
        if self.consume_punct(punct)? {
            Ok(())
        } else {
            self.fail(code, format!("expected `{punct}`"), self.point_span())
        }
    }

    fn consume_punct(&mut self, punct: char) -> Result<bool, ManagedParseError> {
        self.skip_trivia()?;
        if self.remaining().starts_with(punct) {
            self.position += punct.len_utf8();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn skip_trivia(&mut self) -> Result<(), ManagedParseError> {
        loop {
            let start = self.position;
            while self
                .peek_byte()
                .is_some_and(|byte| byte.is_ascii_whitespace())
            {
                self.position += 1;
            }
            if self.remaining().starts_with("//") {
                self.position += 2;
                while self.peek_byte().is_some_and(|byte| byte != b'\n') {
                    self.position += 1;
                }
            } else if self.remaining().starts_with("/*") {
                let comment_start = self.position;
                self.position += 2;
                let Some(end) = self.remaining().find("*/") else {
                    return self.fail(
                        ManagedDiagnosticCode::UnexpectedToken,
                        "unterminated block comment",
                        ManagedSpan::new(comment_start, self.source.len()),
                    );
                };
                self.position += end + 2;
            }
            if self.position == start {
                return Ok(());
            }
        }
    }

    fn unsupported<T>(&self, message: impl Into<String>) -> Result<T, ManagedParseError> {
        self.fail(
            ManagedDiagnosticCode::UnsupportedSyntax,
            message,
            self.point_span(),
        )
    }

    fn fail<T>(
        &self,
        code: ManagedDiagnosticCode,
        message: impl Into<String>,
        span: ManagedSpan,
    ) -> Result<T, ManagedParseError> {
        Err(diagnostic_error(self.source, code, message, span))
    }

    fn remaining(&self) -> &'a str {
        &self.source[self.position..]
    }

    fn peek_byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.position).copied()
    }

    fn point_span(&self) -> ManagedSpan {
        ManagedSpan::new(
            self.position,
            self.position.saturating_add(1).min(self.source.len()),
        )
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$')
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

fn diagnostic_error(
    source: &str,
    code: ManagedDiagnosticCode,
    message: impl Into<String>,
    span: ManagedSpan,
) -> ManagedParseError {
    let mut offset = span.start.min(source.len());
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.chars().count(), |(_, tail)| tail.chars().count())
        + 1;
    ManagedParseError {
        diagnostic: ManagedDiagnostic {
            code,
            message: message.into(),
            span,
            line,
            column,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#""use geosolve managed-v1";

import { sketch, mm } from "@geosolve/sketch-code";
import { roundEveryCorner } from "./patches/round.patch.ts";

export default sketch(($) => {
  // User formatting is retained.
  const path = $.geometry.polyline("path", {
    vertices: [
      { key: "a", position: [0, 0] },
      { key: "b", position: [20, 0] },
      { key: "c", position: [20, 10] },
    ],
    closed: false,
  });
  const rounded = $.use("rounded", roundEveryCorner, {
    polyline: path,
    radius: mm(4),
  });
  $.organize("Profile", [path, rounded]);
  return $.outputs({ path, first: rounded.fillets[0] });
});
"#;

    #[test]
    fn parses_closed_subset_and_semantic_references() {
        let document = parse_managed_source(SOURCE).unwrap();
        assert_eq!(document.source, SOURCE);
        assert_eq!(document.program.declarations.len(), 2);
        assert_eq!(document.program.declarations[1].symbol.0, "rounded");
        assert!(document.program.declarations[1].patch.is_some());
        assert_eq!(document.program.outputs[1].declaration.0, "rounded");
        assert_eq!(
            document.program.outputs[1].path.0,
            vec![
                ManagedPathSegment::Field("fillets".into()),
                ManagedPathSegment::Index(0)
            ]
        );
        assert!(document.owned_spans.len() > 12);
    }

    #[test]
    fn exact_span_rewrite_preserves_all_unowned_bytes() {
        let document = parse_managed_source(SOURCE).unwrap();
        let radius = document
            .value_owned_spans
            .iter()
            .find(|owned| {
                owned.declaration.0 == "rounded"
                    && owned.path.0 == [ManagedPathSegment::Field("radius".into())]
            })
            .unwrap();
        assert_eq!(
            document.source.get(radius.span.start..radius.span.end),
            Some("mm(4)")
        );
        let rewritten = rewrite_managed_source(
            &document,
            &ManagedRewrite::new(&document, radius.span, "mm(2.5)"),
        )
        .unwrap();
        assert_eq!(rewritten.source, SOURCE.replacen("mm(4)", "mm(2.5)", 1));
        assert!(rewritten.source.contains("// User formatting is retained."));

        let semantic_rewrite = rewrite_managed_value(
            &document,
            &SemanticSymbol("rounded".into()),
            &SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())]),
            "mm(3.25)",
        )
        .unwrap();
        assert_eq!(
            semantic_rewrite.source,
            SOURCE.replacen("mm(4)", "mm(3.25)", 1)
        );
    }

    #[test]
    fn aggregate_value_paths_are_authenticated_for_gui_point_rewrites() {
        let source = r#""use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const path = $.geometry.rectangle("path", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  $.organize("Profile", [path]);
  return $.outputs({ path });
});
"#;
        let document = parse_managed_source(source).unwrap();
        let upper_right = document
            .value_owned_spans
            .iter()
            .find(|owned| {
                owned.declaration == SemanticSymbol("path".into())
                    && owned.path.0 == [ManagedPathSegment::Field("upperRight".into())]
            })
            .expect("aggregate point span");
        assert_eq!(
            document
                .source
                .get(upper_right.span.start..upper_right.span.end),
            Some("[60, 35]"),
        );
        let rewritten = rewrite_managed_value(
            &document,
            &SemanticSymbol("path".into()),
            &SemanticOutputPath(vec![ManagedPathSegment::Field("upperRight".into())]),
            "[64, 38]",
        )
        .unwrap();
        assert!(rewritten.source.contains("upperRight: [64, 38]"));
        assert!(rewritten.source.contains("lowerLeft: [0, 0]"));
    }

    #[test]
    fn rejects_stale_unowned_and_utf8_split_rewrites() {
        let document = parse_managed_source(SOURCE).unwrap();
        let mut stale = ManagedRewrite::new(
            &document,
            document.program.declarations[0].symbol_span,
            "\"other\"",
        );
        stale.expected_source_digest = "0".repeat(64);
        assert_eq!(
            rewrite_managed_source(&document, &stale)
                .unwrap_err()
                .diagnostic
                .code,
            ManagedDiagnosticCode::RewriteStale
        );
        let unowned = ManagedRewrite::new(&document, ManagedSpan::new(0, 1), "x");
        assert_eq!(
            rewrite_managed_source(&document, &unowned)
                .unwrap_err()
                .diagnostic
                .code,
            ManagedDiagnosticCode::RewriteStale
        );

        let unicode_source = SOURCE.replacen("User", "Usér", 1);
        let unicode = parse_managed_source(&unicode_source).unwrap();
        let split = unicode_source.find('é').unwrap() + 1;
        let rewrite = ManagedRewrite {
            expected_source_digest: unicode.source_digest.clone(),
            span: ManagedSpan::new(split, split + 1),
            expected_text: String::new(),
            replacement: String::new(),
        };
        assert_eq!(
            rewrite_managed_source(&unicode, &rewrite)
                .unwrap_err()
                .diagnostic
                .code,
            ManagedDiagnosticCode::InvalidUtf8Boundary
        );
    }

    #[test]
    fn rejects_non_subset_constructs_and_reference_errors() {
        let loop_source =
            SOURCE.replacen("  const path", "  for (const x of []) {}\n  const path", 1);
        assert_eq!(
            parse_managed_source(&loop_source)
                .unwrap_err()
                .diagnostic
                .code,
            ManagedDiagnosticCode::UnsupportedSyntax
        );
        let forward = SOURCE.replacen("polyline: path", "polyline: future", 1);
        assert_eq!(
            parse_managed_source(&forward).unwrap_err().diagnostic.code,
            ManagedDiagnosticCode::ForwardReference
        );
        let duplicate = SOURCE.replacen("closed: false", "closed: false, closed: true", 1);
        assert_eq!(
            parse_managed_source(&duplicate)
                .unwrap_err()
                .diagnostic
                .code,
            ManagedDiagnosticCode::DuplicateObjectKey
        );
        let non_finite = SOURCE.replacen("mm(4)", "mm(1e999)", 1);
        assert_eq!(
            parse_managed_source(&non_finite)
                .unwrap_err()
                .diagnostic
                .code,
            ManagedDiagnosticCode::NonFiniteLiteral
        );
    }

    #[test]
    fn enforces_directive_and_source_bound_before_parsing() {
        let wrong = SOURCE.replacen("managed-v1", "managed-v2", 1);
        assert_eq!(
            parse_managed_source(&wrong).unwrap_err().diagnostic.code,
            ManagedDiagnosticCode::MissingDirective
        );
        let oversized = " ".repeat(MANAGED_SOURCE_LIMIT + 1);
        assert_eq!(
            parse_managed_source(&oversized)
                .unwrap_err()
                .diagnostic
                .code,
            ManagedDiagnosticCode::SourceTooLarge
        );
    }

    #[test]
    fn rejects_hostile_value_depth_before_unbounded_recursion() {
        let nested = format!(
            "{}0{}",
            "[".repeat(MANAGED_VALUE_DEPTH_LIMIT + 2),
            "]".repeat(MANAGED_VALUE_DEPTH_LIMIT + 2)
        );
        let source = format!(
            "\"use geosolve managed-v1\";\nexport default sketch(($) => {{\n  const value = $.geometry.point(\"value\", {nested});\n  return $.outputs({{ value }});\n}});\n"
        );
        let diagnostic = parse_managed_source(&source).unwrap_err().diagnostic;
        assert_eq!(diagnostic.code, ManagedDiagnosticCode::ResourceLimit);
        assert!(diagnostic.message.contains("depth limit"));
    }

    #[test]
    fn rejects_hostile_aggregate_collection_item_count() {
        let values = std::iter::repeat_n("0", MANAGED_COLLECTION_ITEM_LIMIT + 1)
            .collect::<Vec<_>>()
            .join(",");
        let source = format!(
            "\"use geosolve managed-v1\";\nexport default sketch(($) => {{\n  const value = $.geometry.point(\"value\", [{values}]);\n  return $.outputs({{ value }});\n}});\n"
        );
        let diagnostic = parse_managed_source(&source).unwrap_err().diagnostic;
        assert_eq!(diagnostic.code, ManagedDiagnosticCode::ResourceLimit);
        assert!(diagnostic.message.contains("collection-item limit"));
    }

    #[test]
    fn rejects_hostile_total_value_node_count_across_declarations() {
        let mut declarations = String::new();
        for index in 0..=MANAGED_VALUE_NODE_LIMIT {
            use std::fmt::Write as _;
            writeln!(
                declarations,
                "  const value{index} = $.geometry.point(\"value{index}\", 0);"
            )
            .unwrap();
        }
        let source = format!(
            "\"use geosolve managed-v1\";\nexport default sketch(($) => {{\n{declarations}  return $.outputs({{ value: value0 }});\n}});\n"
        );
        assert!(source.len() < MANAGED_SOURCE_LIMIT);
        let diagnostic = parse_managed_source(&source).unwrap_err().diagnostic;
        assert_eq!(diagnostic.code, ManagedDiagnosticCode::ResourceLimit);
        assert!(diagnostic.message.contains("value-node limit"));
    }
}
