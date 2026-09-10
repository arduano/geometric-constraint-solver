// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Public compiler handoff for the closed managed sketch language.
 *
 * Source mutation helpers deliberately remain package-internal. A host may
 * compile source into an authenticated envelope here, while structured
 * rewrites must arrive through Rust-prepared two-phase mutation authority.
 */
export {
  EXECUTED_SKETCH_ARTIFACT_FORMAT,
  MANAGED_SKETCH_IR_FORMAT,
  MANAGED_SKETCH_SOURCE_LIMIT,
  ManagedCompileError,
  compileManagedSource,
  applyManagedSourcePatch,
} from "./managed.js";

export type {
  CompiledManagedSource,
  ExecutedConsumerTarget,
  ExecutedDeclarationResult,
  ExecutedGeneratedMember,
  ExecutedGeneratedMemberAddress,
  ExecutedGroup,
  ExecutedParameter,
  ExecutedPresentation,
  ExecutedResultLeaf,
  ExecutedSketchArtifact,
  ExecutedSuppression,
  ExecutedValueConsumer,
  ManagedBindingStatement,
  ManagedCompileOptions,
  ManagedDeclarationStatement,
  ManagedExpression,
  ManagedGroupStatement,
  ManagedImport,
  ManagedObjectField,
  ManagedParameterAnnotation,
  ManagedReference,
  ManagedSketchIr,
  ManagedSourceSite,
  ManagedSourceSiteKind,
  ManagedSourceSpan,
  ManagedSourceEdit,
  ManagedSourcePatch,
  ManagedSourceMutationReceipt,
  ManagedStatement,
  ManagedSuppressionStatement,
  ManagedValue,
  ManagedWireUnitLiteral,
  SemanticPathSegment,
} from "./managed.js";
