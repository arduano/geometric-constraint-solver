// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

//! This crate is deliberately optional. The native solver, sketch domain,
//! headless editor and low-level intent graph do not depend on it.

#[cfg(test)]
extern crate self as geosolve_sketch_code;
#[cfg(test)]
#[path = "../tests/support/managed_regression_projects.rs"]
mod managed_regression_projects;

mod artifact;
mod bootstrap;
mod bundled_samples;
mod composition;
mod declaration_catalog;
mod document_export;
mod editor_insertion;
mod editor_publication;
/// Shared native/source publication proof; does not grant publication authority.
pub mod editor_terminal;
mod expansion;
mod generated;
mod managed;
mod managed_control;
mod model;
mod overlay;
mod point_terminal;
mod prepared_mutation;
mod project;
mod reconcile;
mod session;
mod work_receipt;

pub use point_terminal::transport_code_point_terminal_branches;

pub use artifact::{
    ArtifactValidationError, CollectionRule, PATCH_ARTIFACT_FORMAT, PatchModuleArtifact,
    PatchTemplateNode, PatchTemplateOutput, TemplateArgument, TemplateBinding,
    ValidatedPatchModuleArtifact,
};
pub use bundled_samples::{
    BundledSampleSpec, SampleCategory, SampleExpected, SampleProvenance,
    SampleProvenanceRelationship, bundled_sample, bundled_sample_catalog,
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
pub use document_export::{
    ManagedSketchExportError, export_sketch_document_to_managed_source,
    export_sketch_document_with_features_to_managed_source,
};
pub use editor_publication::{
    PreparedEditorSourceInsertion, prepare_editor_source_insertion, retain_editor_default_labels,
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
pub use generated::{
    GENERATED_SKETCH_ARTIFACT_FORMAT, GENERATED_SKETCH_ARTIFACT_LIMIT, GeneratedApplication,
    GeneratedDeclaration, GeneratedGroup, GeneratedParameter, GeneratedReference,
    GeneratedSketchArtifact, GeneratedValidationError, GeneratedValue, ValidatedGeneratedSketch,
    materialize_generated_sketch_cold,
};
pub use managed::{
    CompiledManagedSource, EXECUTED_SKETCH_ARTIFACT_FORMAT, ExecutedConsumerTarget,
    ExecutedDeclarationResult, ExecutedGeneratedMember, ExecutedGeneratedMemberAddress,
    ExecutedGroup, ExecutedParameter, ExecutedPresentation, ExecutedResultLeaf,
    ExecutedSketchArtifact, ExecutedSuppression, ExecutedValueConsumer, MANAGED_SKETCH_IR_FORMAT,
    MANAGED_SOURCE_LIMIT, MANAGED_WIRE_LIMIT, ManagedAuthoredMetadata, ManagedDimensionDefaults,
    ManagedDocumentPresentation, ManagedExpression, ManagedIrImport, ManagedObjectField,
    ManagedParameterDeclaration, ManagedPresentation, ManagedReference, ManagedSketchIr,
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
    MANAGED_MUTATION_BATCH_LIMIT, ManagedDeclarationDraft, ManagedMetadataTarget,
    ManagedMutationAuthority, ManagedMutationReceipt, ManagedMutationTarget, ManagedSketchMutation,
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

mod navigation;
pub use navigation::{
    ManagedNavigationEntry, ManagedNavigationIndex, generated_panel_row_id,
    managed_navigation_index, managed_panel_row_id,
};

mod browsing;
pub use browsing::{
    ManagedControlSubmission, managed_browsing_navigation_index, managed_control_source_mutation,
};
pub use browsing::{
    ManagedDeclarationClosureRole, ManagedDeclarationPanelProjection, ManagedDeclarationPanelRow,
    ManagedGeneratedPanelRow, managed_declaration_panel_projection,
};

mod editor_checkpoint;
pub use editor_checkpoint::{
    ValidatedSourceHistory, encode_editor_checkpoint, restore_editor_checkpoint,
    validate_editor_checkpoint,
};

mod source_properties;
pub use source_properties::{
    ManagedPropertyTarget, ManagedSourceProperties, managed_control_label,
    managed_parameter_consumer_labels, parameter_can_extract,
};

mod inspector;
pub use inspector::{
    InspectorDescriptorIndex, InspectorParameterAuthority, InspectorParameterPresentation,
    ManagedSourceInspector, managed_path_text, managed_source_path,
};

/// Shared persisted source workspace and native history admission.
pub mod authoring_persistence;

/// Accepted source/native interaction correspondence shared by hosts.
pub mod interaction;

mod compiler_context;
pub use compiler_context::managed_compiler_patches;
