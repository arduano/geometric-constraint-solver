// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

//! This crate is deliberately optional. The native solver, sketch domain,
//! headless editor and low-level intent graph do not depend on it.

mod artifact;
mod bootstrap;
mod composition;
mod declaration_catalog;
mod demos;
mod expansion;
mod managed_edit;
mod model;
mod parser;
mod project;
mod reconcile;
mod session;

pub use artifact::{
    ArtifactValidationError, CollectionRule, EditLens, PATCH_ARTIFACT_FORMAT, PatchModuleArtifact,
    PatchTemplateNode, TemplateBinding, ValidatedPatchModuleArtifact,
};
pub use bootstrap::{
    EditorBootstrapDeclaration, EditorBootstrapError, initialize_code_project_from_editor,
};
pub use composition::{
    CodeCompositionError, MaterializedCodeProject, MaterializedFilletOutput,
    materialize_code_project_cold, materialize_code_project_incremental,
    rehydrate_materialized_code_project,
};
pub use declaration_catalog::{
    CODE_DECLARATION_FAMILIES, CodeDeclarationFamilyDescriptor, CodeDeclarationResultDescriptor,
    CodeResultShape, DirectDeclarationLowering, NativeDeclarationContract,
    TemplateDeclarationLowering, code_declaration_family, declaration_result_catalog,
    typescript_declaration_result_catalog,
};
pub use demos::{
    CodeProjectDemo, CodeProjectDemoId, bundled_code_project_demos,
    rounded_polyline_member_addresses,
};
pub use expansion::{
    CodeExpansionError, CodeHostRequest, ExpandedCodeProject, ExpandedFeatureCorner, ExpandedPort,
    ExpandedSemanticOutput, ExpandedSemanticTarget, GeneratedIntentProvenance,
    KeyedFilletHostRequest, expand_code_project, required_generated_members,
};
pub use managed_edit::{
    ManagedEdit, ManagedEditError, ManagedEditPlan, apply_managed_edit, plan_managed_edit,
};
pub use model::{
    AuthoringDeclaration, AuthoringProgram, CodeProject, CodeProjectFile, FeatureKind, FeatureRef,
    ManagedDiagnostic, ManagedDiagnosticCode, ManagedDocument, ManagedImport, ManagedOrganization,
    ManagedOutput, ManagedOwnedSpan, ManagedOwnedSpanKind, ManagedPathSegment, ManagedSpan,
    ManagedValue, ManagedValueOwnedSpan, OutputRef, PatchInvocation, ProjectKey,
    SemanticOutputPath, SemanticSymbol, UnitLiteral,
};
pub use parser::{
    MANAGED_COLLECTION_ITEM_LIMIT, MANAGED_SOURCE_LIMIT, MANAGED_VALUE_DEPTH_LIMIT,
    MANAGED_VALUE_NODE_LIMIT, ManagedParseError, ManagedRewrite, parse_managed_source,
    rewrite_managed_source, rewrite_managed_value,
};
pub use project::CodeProjectError;
pub use reconcile::{
    GeneratedMemberAddress, GeneratedMemberIdentity, GeneratedOverride, KeyedReconcileError,
    KeyedReconcilePlan, KeyedReconcileState, ReconciledMember,
};
pub use session::{
    CodeSessionError, CodeSessionFailure, CodeSessionIdentity, CodeSessionReceipt,
    CodeSessionSnapshot, PreparedCodeEdit, SketchCodeSession,
};

/// SDK ABI written into every precompiled data-only patch artifact.
pub const SKETCH_CODE_SDK_ABI: &str = "geosolve-sketch-code-v1";

/// Maximum canonical size of one caller-built patch artifact.
pub const PATCH_ARTIFACT_LIMIT: usize = 16 * 1024 * 1024;

/// Maximum admitted size of a complete optional code project.
pub const CODE_PROJECT_LIMIT: usize = 64 * 1024 * 1024;
