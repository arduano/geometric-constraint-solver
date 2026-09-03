// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Stable project brand carried by every public semantic reference.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectKey(pub String);

/// Stable declaration symbol authored as the first argument of a managed call.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticSymbol(pub String);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureKind {
    Point,
    Curve,
    CurveSpan,
    Scalar,
    Contact,
    Constraint,
    Dimension,
    Profile,
    Chain,
    Operation,
    Feature,
    FeatureCorner,
    Collection,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ManagedPathSegment {
    Field(String),
    Index(usize),
    Member { member: String },
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticOutputPath(pub Vec<ManagedPathSegment>);

/// User-facing feature reference. Raw intent node/port IDs are deliberately absent.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FeatureRef {
    pub project: ProjectKey,
    pub declaration: SemanticSymbol,
    pub output: SemanticOutputPath,
    pub expected_kind: FeatureKind,
    pub generation: u32,
}

/// Typed leaf reference used inside artifacts and managed values.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct OutputRef {
    pub project: ProjectKey,
    pub declaration: SemanticSymbol,
    pub output: SemanticOutputPath,
    pub expected_kind: FeatureKind,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedSpan {
    pub start: usize,
    pub end: usize,
}

/// Semantic ownership of one exactly rewritable source span.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedOwnedSpanKind {
    Declaration,
    Symbol,
    Arguments,
    Literal,
    Reference,
    Organization,
}

/// Authenticated source coordinate which the GUI may replace without taking
/// ownership of surrounding comments or formatting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedOwnedSpan {
    pub kind: ManagedOwnedSpanKind,
    pub span: ManagedSpan,
    pub source_digest: String,
}

/// Declaration-relative coordinate of one exactly rewritable managed value.
/// Runtime value-consumer provenance joins the semantic argument field to
/// the compiler-authenticated source bytes that own the leaf.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedValueOwnedSpan {
    pub declaration: SemanticSymbol,
    pub path: SemanticOutputPath,
    pub kind: ManagedOwnedSpanKind,
    pub span: ManagedSpan,
    pub source_digest: String,
}

impl ManagedSpan {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitLiteral {
    pub unit: String,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ManagedValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Unit(UnitLiteral),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
    Reference {
        declaration: SemanticSymbol,
        path: SemanticOutputPath,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedImport {
    pub module: String,
    pub bindings: Vec<String>,
    pub span: ManagedSpan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatchInvocation {
    pub module_binding: String,
    pub arguments: ManagedValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuthoringDeclaration {
    pub variable: String,
    pub symbol: SemanticSymbol,
    pub builder_path: Vec<String>,
    pub arguments: ManagedValue,
    pub patch: Option<PatchInvocation>,
    pub statement_span: ManagedSpan,
    pub symbol_span: ManagedSpan,
    pub arguments_span: ManagedSpan,
}

/// One lexical numeric binding projected from executed managed V3 authority.
/// It may be referenced only by later declaration arguments; it is not a
/// native declaration or sketch output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManagedScalarBinding {
    pub variable: String,
    pub symbol: SemanticSymbol,
    pub value: ManagedValue,
    pub statement_span: ManagedSpan,
    pub symbol_span: ManagedSpan,
    pub value_span: ManagedSpan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedOrganization {
    pub name: String,
    pub declarations: Vec<SemanticSymbol>,
    pub span: ManagedSpan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedOutput {
    pub name: String,
    pub declaration: SemanticSymbol,
    pub path: SemanticOutputPath,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuthoringProgram {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scalar_bindings: Vec<ManagedScalarBinding>,
    pub declarations: Vec<AuthoringDeclaration>,
    pub organizations: Vec<ManagedOrganization>,
    pub outputs: Vec<ManagedOutput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedDiagnosticCode {
    SourceTooLarge,
    ResourceLimit,
    InvalidUtf8Boundary,
    MissingDirective,
    InvalidDirective,
    InvalidImport,
    InvalidSketchEnvelope,
    UnsupportedSyntax,
    UnexpectedToken,
    DuplicateSymbol,
    DuplicateObjectKey,
    ForwardReference,
    NonFiniteLiteral,
    TrailingSource,
    RewriteStale,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedDiagnostic {
    pub code: ManagedDiagnosticCode,
    pub message: String,
    pub span: ManagedSpan,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManagedDocument {
    pub source: String,
    pub source_digest: String,
    pub imports: Vec<ManagedImport>,
    pub program: AuthoringProgram,
    pub envelope_span: ManagedSpan,
    pub owned_spans: Vec<ManagedOwnedSpan>,
    #[serde(default)]
    pub value_owned_spans: Vec<ManagedValueOwnedSpan>,
    /// Complete executed/reversible compiler authority for managed source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiled: Option<Box<crate::CompiledManagedSource>>,
    /// Monotonic source-declaration allocator authority. This metadata is not
    /// printed into `sketch.ts`; hosts retain its maximum across Undo/delete
    /// so a canvas-authored name is never silently reused.
    #[serde(default, skip_serializing_if = "crate::is_zero_u64")]
    pub declaration_name_high_water: u64,
}

impl ManagedDocument {
    #[must_use]
    pub const fn has_compiled_authority(&self) -> bool {
        self.compiled.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodeProjectFile {
    pub path: String,
    pub source_digest: String,
    pub contents: String,
    pub managed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeProject {
    pub project: ProjectKey,
    pub managed: ManagedDocument,
    pub custom_files: BTreeMap<String, CodeProjectFile>,
    pub artifacts: BTreeMap<String, serde_json::Value>,
    pub lock: serde_json::Value,
}

impl FeatureRef {
    /// Resolves a child output without exposing an intent node or port ID.
    #[must_use]
    pub fn output(&self, field: impl Into<String>, expected_kind: FeatureKind) -> OutputRef {
        let mut output = self.output.clone();
        output.0.push(ManagedPathSegment::Field(field.into()));
        OutputRef {
            project: self.project.clone(),
            declaration: self.declaration.clone(),
            output,
            expected_kind,
            generation: self.generation,
        }
    }
}

impl OutputRef {
    /// Narrows a semantic collection member without exposing wire identity.
    #[must_use]
    pub fn member(&self, key: impl Into<String>, expected_kind: FeatureKind) -> Self {
        let mut output = self.output.clone();
        output
            .0
            .push(ManagedPathSegment::Member { member: key.into() });
        Self {
            project: self.project.clone(),
            declaration: self.declaration.clone(),
            output,
            expected_kind,
            generation: self.generation,
        }
    }
}
