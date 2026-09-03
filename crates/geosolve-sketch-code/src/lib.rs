// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

//! This crate is deliberately optional. The native solver, sketch domain,
//! headless editor and low-level intent graph do not depend on it.

mod artifact;
mod composition;
mod declaration_catalog;
mod demos;
mod editor_insertion;
mod expansion;
mod managed;
mod managed_control;
mod model;
mod overlay;
mod prepared_mutation;
mod project;
mod reconcile;
mod session;
mod work_receipt;

pub use artifact::{
    ArtifactValidationError, CollectionRule, PATCH_ARTIFACT_FORMAT, PatchModuleArtifact,
    PatchTemplateNode, PatchTemplateOutput, TemplateArgument, TemplateBinding,
    ValidatedPatchModuleArtifact,
};
pub use composition::{
    CodeCompositionError, MaterializedCodeProject, MaterializedFilletOutput,
    materialize_code_project_cold, materialize_code_project_cold_with_overlay,
    materialize_code_project_incremental, materialize_code_project_incremental_for_structural_edit,
    materialize_code_project_incremental_with_overlay,
    materialize_code_project_incremental_with_overlay_and_accepted_continuation_audited,
    materialize_code_project_incremental_with_overlay_audited, rehydrate_materialized_code_project,
};
pub use declaration_catalog::{
    CODE_AUTHORING_FAMILIES, CodeAuthoringArgumentKind, CodeAuthoringAvailability,
    CodeAuthoringCatalogError, CodeAuthoringCollectionMember, CodeAuthoringDeclarationDescriptor,
    CodeAuthoringDeclarationKind, CodeAuthoringDynamicChildren,
    CodeAuthoringDynamicChildrenDescriptor, CodeAuthoringFamilyDescriptor,
    CodeAuthoringInputDescriptor, CodeAuthoringResultPolicy, CodeAuthoringValueDescriptor,
    CodeDeclarationResultDescriptor, CodeResultKeySource, CodeResultShape, code_authoring_family,
    declaration_result_catalog, public_code_authoring_families, resolve_code_authoring_declaration,
    typescript_declaration_result_catalog,
};
pub use demos::{
    CodeProjectDemo, CodeProjectDemoId, bundled_code_project_demos,
    rounded_polyline_member_addresses,
};
pub use editor_insertion::{
    CANVAS_ADDITIONS_GROUP, EditorBootstrapDeclaration, EditorDeclarationInsertionError,
    EditorDeclarationInsertionPlan, EditorSourceDeclarationClosure,
    EditorSourceDeclarationClosureKind, EditorSourceDeclarationDraft, EditorSourceDeclarationRef,
    prepare_editor_declaration_insertions,
};
pub use expansion::{
    CodeExpansionError, CodeHostRequest, CodePointEdit, CodePointSeedSource, CodeRectangleCorner,
    ExpandedCodeProject, ExpandedFeatureCorner, ExpandedGeneratedChild, ExpandedPort,
    ExpandedSemanticOutput, ExpandedSemanticTarget, ExpandedWritablePoint,
    GeneratedIntentProvenance, KeyedFilletHostRequest, direct_declaration_intent_symbol,
    expand_code_project, expand_code_project_for_structural_edit, expand_code_project_with_overlay,
    required_generated_members, stage_point_drags,
};
pub use managed::{
    CompiledManagedSource, EXECUTED_SKETCH_ARTIFACT_FORMAT, ExecutedConsumerTarget,
    ExecutedDeclarationResult, ExecutedGeneratedMember, ExecutedGeneratedMemberAddress,
    ExecutedGroup, ExecutedResultLeaf, ExecutedSketchArtifact, ExecutedSuppression,
    ExecutedValueConsumer, MANAGED_SKETCH_IR_FORMAT, MANAGED_SOURCE_LIMIT, MANAGED_WIRE_LIMIT,
    ManagedExpression, ManagedIrImport, ManagedObjectField, ManagedReference, ManagedSketchIr,
    ManagedSourceDeclarationClosure, ManagedSourceDeclarationClosureKind, ManagedSourceSite,
    ManagedSourceSiteKind, ManagedSourceSpan, ManagedStatement, ManagedValidationError,
};
pub use managed_control::{
    MANAGED_CONTROL_CONSUMER_LIMIT, MANAGED_CONTROL_LIMIT, ManagedControl, ManagedControlAccess,
    ManagedControlAuthority, ManagedControlBound, ManagedControlConsumer,
    ManagedControlConsumerTarget, ManagedControlEdit, ManagedControlEditBatch, ManagedControlError,
    ManagedControlId, ManagedControlManifest, ManagedControlNavigation, ManagedControlNumberKind,
    ManagedControlReadOnlyReason, ManagedControlSchema, ManagedControlSource, ManagedControlToken,
    managed_control_authority, managed_control_manifest, prepare_managed_control_mutation,
};
pub use model::{
    AuthoringDeclaration, AuthoringProgram, CodeProject, CodeProjectFile, FeatureKind, FeatureRef,
    ManagedDiagnostic, ManagedDiagnosticCode, ManagedDocument, ManagedImport, ManagedOrganization,
    ManagedOutput, ManagedOwnedSpan, ManagedOwnedSpanKind, ManagedPathSegment,
    ManagedScalarBinding, ManagedSpan, ManagedValue, ManagedValueOwnedSpan, OutputRef,
    PatchInvocation, ProjectKey, SemanticOutputPath, SemanticSymbol, UnitLiteral,
};
pub use overlay::{
    CodeDraft, CodeDraftProvenance, CodeDraftValue, CodeGeneratedChildAddress,
    CodeInteractionOverlay, CodeOverlayError, CodeOwnerAddress, CodeOwnerIdentity,
    CodeWritableAddress, CodeWritableField,
};
pub use prepared_mutation::{
    MANAGED_MUTATION_BATCH_LIMIT, ManagedDeclarationDraft, ManagedMutationAuthority,
    ManagedMutationReceipt, ManagedMutationTarget, ManagedSketchMutation,
    ManagedSourceDeclarationHelperMutation, ManagedValueMutation, PREPARED_MANAGED_MUTATION_FORMAT,
    PREPARED_MANAGED_MUTATION_WIRE_LIMIT, PREPARED_MANAGED_SOURCE_FORMAT,
    PreparedManagedMutationError, PreparedManagedMutationReceipt, PreparedManagedMutationRequest,
    PreparedManagedMutationTicket, PreparedManagedSourceRequest, PreparedManagedSourceTicket,
    ValidatedManagedMutation, ValidatedManagedSource, derive_managed_value_mutation,
    managed_point_value_mutations, prepare_managed_mutation, prepare_managed_source,
    validate_prepared_managed_mutation, validate_prepared_managed_source,
};
pub use project::CodeProjectError;
pub use reconcile::{
    GeneratedMemberAddress, GeneratedMemberIdentity, GeneratedOverride, KeyedReconcileError,
    KeyedReconcilePlan, KeyedReconcileState, ReconciledMember,
};
pub use session::{
    CodeSessionError, CodeSessionFailure, CodeSessionIdentity, CodeSessionReceipt,
    CodeSessionSnapshot, MAX_CODE_SESSION_WIRE_INTEGER, PreparedCodeEdit, SketchCodeSession,
};
pub use work_receipt::{AuditedCodeWork, CodeWorkReceipt};

/// SDK ABI written into every precompiled data-only patch artifact.
pub const SKETCH_CODE_SDK_ABI: &str = "geosolve-sketch-code-v2";

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde skip_serializing_if predicates receive a shared reference"
)]
pub(crate) const fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

/// Maximum canonical size of one caller-built patch artifact.
pub const PATCH_ARTIFACT_LIMIT: usize = 16 * 1024 * 1024;

/// Maximum admitted size of a complete optional code project.
pub const CODE_PROJECT_LIMIT: usize = 64 * 1024 * 1024;
