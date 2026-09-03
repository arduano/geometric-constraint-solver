// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Closed, reversible managed sketch source compiler.
 *
 * TypeScript owns lexical admission and instrumentation. Executing the
 * instrumented callback records semantic declarations and value-consumer
 * provenance; it never evaluates geometry or solver equations.
 */

import ts from "typescript";

import {
  AUTHORING_METHOD_CATALOG,
  DECLARATION_RESULT_CATALOG,
} from "./generated-declaration-results.js";
import type { FeatureKind } from "./authoring.js";
import type {
  ArtifactTemplateArgument,
  ArtifactTemplateBinding,
  ArtifactTemplateNode,
  PatchArtifactPlan,
} from "./compiler.js";

export const MANAGED_SKETCH_IR_FORMAT =
  "geosolve-managed-sketch-ir-v3" as const;
export const EXECUTED_SKETCH_ARTIFACT_FORMAT =
  "geosolve-executed-sketch-artifact-v3" as const;
export const MANAGED_SKETCH_SOURCE_LIMIT = 4 * 1024 * 1024;
export const MANAGED_CANVAS_ADDITIONS_GROUP = "Canvas additions" as const;
export const MANAGED_MUTATION_BATCH_LIMIT = 1_024;

const MANAGED_STATEMENT_LIMIT = 65_536;
const MANAGED_MUTATION_PATH_LIMIT = 128;
const MANAGED_MUTATION_VALUE_DEPTH_LIMIT = 64;
const MANAGED_MUTATION_VALUE_NODE_LIMIT = 16_384;
const MANAGED_MUTATION_COMMENT_LIMIT = 1_024;
const MANAGED_MUTATION_STRING_LIMIT = 16_384;
const GENERATED_UNIT_HELPERS = ["mm", "rad"] as const;
type GeneratedUnitHelper = typeof GENERATED_UNIT_HELPERS[number];
const RETIRED_MANAGED_TRANSPORT_PROPERTIES = new Set([
  "recipe",
  "inputs",
  "fields",
  "values",
  "results",
  "operationOutputs",
  "outputs",
  "editLens",
]);
/** One stable semantic result/property coordinate. */
export type SemanticPathSegment = string | number | { readonly member: string };

/** Exact Serde representation of a source-authorable unit literal. */
export interface ManagedWireUnitLiteral {
  readonly unit: string;
  readonly value: number;
}

/** Exact data-only Serde representation shared with Rust `ManagedValue`. */
export type ManagedValue =
  | { readonly kind: "null" }
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "number"; readonly value: number }
  | { readonly kind: "string"; readonly value: string }
  | { readonly kind: "unit"; readonly value: ManagedWireUnitLiteral }
  | { readonly kind: "array"; readonly value: readonly ManagedValue[] }
  | {
    readonly kind: "object";
    readonly value: Readonly<Record<string, ManagedValue>>;
  }
  | {
    readonly kind: "reference";
    readonly value: {
      readonly declaration: string;
      readonly path: readonly SemanticPathSegment[];
    };
  };

export interface ManagedSourceSpan {
  /** Inclusive UTF-8 byte offset. */
  readonly start: number;
  /** Exclusive UTF-8 byte offset. */
  readonly end: number;
}

export type ManagedSourceSiteKind =
  | "declaration"
  | "value"
  | "group"
  | "group_reference"
  | "suppression"
  | "suppression_reference";

export interface ManagedSourceSite {
  readonly id: string;
  readonly kind: ManagedSourceSiteKind;
  readonly span: ManagedSourceSpan;
  readonly source_digest: string;
}

export type ManagedExpression =
  | { readonly kind: "null"; readonly site: string }
  | { readonly kind: "boolean"; readonly value: boolean; readonly site: string }
  | { readonly kind: "number"; readonly value: number; readonly site: string }
  | { readonly kind: "string"; readonly value: string; readonly site: string }
  | {
    readonly kind: "array";
    readonly values: readonly ManagedExpression[];
    readonly site: string;
  }
  | {
    readonly kind: "object";
    readonly fields: readonly ManagedObjectField[];
    readonly site: string;
  }
  | {
    readonly kind: "reference";
    readonly declaration: string;
    readonly path: readonly SemanticPathSegment[];
    readonly site: string;
  }
  | {
    readonly kind: "call";
    readonly callee: string;
    readonly arguments: readonly ManagedExpression[];
    readonly site: string;
  };

export interface ManagedObjectField {
  readonly name: string;
  readonly value: ManagedExpression;
  readonly comments: readonly string[];
}

export interface ManagedBindingStatement {
  readonly statement: "binding";
  readonly variable: string;
  readonly value: ManagedExpression;
  readonly comments: readonly string[];
}

export interface ManagedDeclarationStatement {
  readonly statement: "declaration";
  readonly variable: string;
  readonly symbol: string;
  readonly builder_path: readonly string[];
  /** Imported patch binding for `$.use`, otherwise null. */
  readonly patch: string | null;
  readonly arguments: ManagedExpression;
  readonly site: string;
  readonly comments: readonly string[];
}

export interface ManagedGroupStatement {
  readonly statement: "group";
  readonly name: string;
  readonly declarations: readonly ManagedReference[];
  readonly site: string;
  readonly comments: readonly string[];
}

export interface ManagedSuppressionStatement {
  readonly statement: "suppression";
  readonly target: ManagedReference;
  readonly site: string;
  readonly comments: readonly string[];
}

export interface ManagedReference {
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
  readonly site: string;
}

export type ManagedStatement =
  | ManagedBindingStatement
  | ManagedDeclarationStatement
  | ManagedGroupStatement
  | ManagedSuppressionStatement;

export interface ManagedImport {
  readonly module: string;
  readonly bindings: readonly string[];
}

export interface ManagedSketchIr {
  readonly format: typeof MANAGED_SKETCH_IR_FORMAT;
  readonly imports: readonly ManagedImport[];
  readonly statements: readonly ManagedStatement[];
  /** Ordinary nested value returned by the executed sketch callback. */
  readonly output: ManagedExpression;
  readonly source_sites: readonly ManagedSourceSite[];
  readonly source_digest: string;
  readonly ir_digest: string;
}

export interface ExecutedResultLeaf {
  readonly kind: FeatureKind;
  readonly path: readonly SemanticPathSegment[];
}

export interface ExecutedDeclarationResult {
  readonly declaration: string;
  readonly family: string;
  readonly patch: string | null;
  readonly site: string;
  readonly result: readonly ExecutedResultLeaf[];
}

export interface ExecutedValueConsumer {
  readonly value_site: string;
  readonly target: ExecutedConsumerTarget;
  readonly property: readonly SemanticPathSegment[];
}

export type ExecutedConsumerTarget =
  | {
    readonly target: "declaration";
    readonly declaration: string;
    readonly family: string;
  }
  | {
    readonly target: "generated";
    readonly address: ExecutedGeneratedMemberAddress;
    readonly family: string;
  };

export interface ExecutedGeneratedMemberAddress {
  readonly invocation: string;
  readonly template: readonly string[];
  readonly member_key: readonly string[];
  readonly output: readonly string[];
}

export interface ExecutedGeneratedMember {
  readonly address: ExecutedGeneratedMemberAddress;
  readonly family: string;
  readonly kind: FeatureKind;
}

export interface ExecutedGroup {
  readonly name: string;
  readonly site: string;
  readonly declarations: readonly ManagedReference[];
}

export interface ExecutedSuppression {
  readonly site: string;
  readonly target: ManagedReference;
}

export interface ExecutedSketchArtifact {
  readonly format: typeof EXECUTED_SKETCH_ARTIFACT_FORMAT;
  readonly source_digest: string;
  readonly ir_digest: string;
  readonly declarations: readonly ExecutedDeclarationResult[];
  readonly generated_members: readonly ExecutedGeneratedMember[];
  readonly groups: readonly ExecutedGroup[];
  readonly suppressions: readonly ExecutedSuppression[];
  readonly value_consumers: readonly ExecutedValueConsumer[];
  /** Complete data-only semantic value returned by the callback. */
  readonly output: ManagedValue;
  readonly artifact_digest: string;
}

export interface CompiledManagedSource {
  /** Exact raw draft bytes supplied to this compiler invocation. */
  readonly inputSourceDigest: string;
  readonly normalizedSource: string;
  readonly ir: ManagedSketchIr;
  readonly artifact: ExecutedSketchArtifact;
  readonly canonicalIrJson: string;
  readonly canonicalArtifactJson: string;
}

/** Authenticated source lifecycle families projected from managed IR. */
export type ManagedSourceDeclarationClosureKind = "profile_offset";

/** One visible operation root and its source-visible private helpers. */
export interface ManagedSourceDeclarationClosure {
  readonly kind: ManagedSourceDeclarationClosureKind;
  /** Semantic declaration symbol, never its lexical variable coordinate. */
  readonly root: string;
  /** Semantic declaration symbols in their current lexical order. */
  readonly helpers: readonly string[];
}

/** Atomic proof of the exact normalized source accepted by one mutation. */
export interface ManagedMutationReceipt {
  readonly baseSourceDigest: string;
  readonly candidateSourceDigest: string;
  readonly compiled: CompiledManagedSource;
}

export interface ManagedCompileOptions {
  /** Pinned patch plans keyed by their local managed import binding. */
  readonly patches?: Readonly<Record<string, PatchArtifactPlan>>;
}

/** Site-free semantic declaration prepared by Rust or another trusted host. */
export interface ManagedDeclarationDraft {
  readonly variable: string;
  readonly symbol: string;
  readonly builder_path: readonly string[];
  readonly arguments: ManagedValue;
  readonly patch?: string | null;
  readonly group?: typeof MANAGED_CANVAS_ADDITIONS_GROUP;
  readonly suppressed?: boolean;
  readonly comments?: readonly string[];
}

export type ManagedMutationTarget =
  | { readonly target: "declaration"; readonly declaration: string }
  | {
    readonly target: "generated";
    readonly address: ExecutedGeneratedMemberAddress;
  };

export interface ManagedValueMutation {
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
  readonly expected: ManagedValue;
  readonly value: ManagedValue;
}

export type ManagedSketchMutation =
  | {
    readonly mutation: "insert_declarations";
    readonly declarations: readonly ManagedDeclarationDraft[];
  }
  | {
    readonly mutation: "reorder_declaration";
    /** Semantic declaration symbol, never a lexical or native ID. */
    readonly declaration: string;
    /** Insert before this semantic symbol; null means the definition-list end. */
    readonly before: string | null;
  }
  | {
    readonly mutation: "set_suppressed";
    readonly target: ManagedMutationTarget;
    readonly suppressed: boolean;
  }
  | {
    readonly mutation: "set_value";
    /** Scalar-binding variable or declaration semantic symbol. */
    readonly declaration: string;
    readonly path: readonly SemanticPathSegment[];
    readonly expected: ManagedValue;
    readonly value: ManagedValue;
  }
  | {
    readonly mutation: "set_values";
    readonly values: readonly ManagedValueMutation[];
  }
  | {
    readonly mutation: "delete";
    readonly target: ManagedMutationTarget;
  };

export type ManagedMutationErrorCode =
  | "invalid_current"
  | "resource_limit"
  | "invalid_draft"
  | "duplicate_declaration"
  | "unknown_declaration"
  | "unknown_generated_member"
  | "ambiguous_generated_member"
  | "owned_helper_mutation"
  | "dependency_invalid_move"
  | "missing_patch_plan";

export class ManagedMutationError extends TypeError {
  readonly code: ManagedMutationErrorCode;
  /** Canonical candidate text retained only when mutation reached printing. */
  readonly candidateSource: string | undefined;
  /** UTF-8 byte span in `candidateSource`, when the compiler supplied one. */
  readonly span: ManagedSourceSpan | undefined;

  constructor(
    code: ManagedMutationErrorCode,
    message: string,
    candidate?: {
      readonly source: string;
      readonly span: ManagedSourceSpan;
    },
  ) {
    super(message);
    this.name = "ManagedMutationError";
    this.code = code;
    this.candidateSource = candidate?.source;
    this.span = candidate?.span;
  }
}

export class ManagedCompileError extends TypeError {
  readonly span: ManagedSourceSpan;

  constructor(message: string, span: ManagedSourceSpan) {
    super(message);
    this.name = "ManagedCompileError";
    this.span = span;
  }
}

interface ParseContext {
  readonly source: string;
  readonly sourceFile: ts.SourceFile;
  readonly sourceDigest: string;
  readonly utf8Offsets: readonly number[];
  readonly sites: ManagedSourceSite[];
  readonly siteIds: Set<string>;
  readonly variables: Set<string>;
  readonly bindingInitializers: Map<string, ts.Expression>;
  readonly declarationVariables: Set<string>;
  readonly importedBindings: Set<string>;
  readonly symbols: Set<string>;
  readonly groups: Set<string>;
  statementOrdinal: number;
}

interface RuntimeReference {
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
}

interface RuntimeResult {
  readonly reference: RuntimeReference;
}

const runtimeReference = Symbol("geosolve.managed.reference");

/** Parse, normalize, instrument, and execute one closed managed sketch sketch. */
export function compileManagedSource(
  source: string,
  options: ManagedCompileOptions = {},
): CompiledManagedSource {
  const parsed = parseManagedSource(source);
  const normalizedSource = printManagedSource(parsed);
  const normalized = parseManagedSource(normalizedSource);
  const artifact = executeManagedSource(normalizedSource, normalized, options);
  return Object.freeze({
    inputSourceDigest: sha256(source),
    normalizedSource,
    ir: deepFreeze(normalized),
    artifact,
    canonicalIrJson: canonicalManagedSketchIr(normalized),
    canonicalArtifactJson: canonicalExecutedSketchArtifact(artifact),
  });
}

/**
 * Reconstruct source-declaration lifecycle closures from compiled IR.
 *
 * Ownership is semantic and fail-closed: only a named aggregate with one
 * distinct declaration consumer and one matching typed Profile Offset input
 * is private. Names, labels, grouping, and adjacency never participate, so
 * shared aggregates remain independent roots.
 */
export function sourceDeclarationClosures(
  compiled: CompiledManagedSource,
  options: ManagedCompileOptions = {},
): readonly ManagedSourceDeclarationClosure[] {
  const authenticated = authenticateMutationCurrent(compiled, options);
  return deepFreeze(projectSourceDeclarationClosures(authenticated.ir));
}


/**
 * Canonically print and compile one mutated IR.
 *
 * Callers may never publish a hand-edited IR directly. Reparse assigns exact
 * UTF-8 ranges, authenticates every new source site, and recorder execution
 * derives the complete artifact from the regenerated source.
 */
export function recompileManagedSketchIr(
  ir: ManagedSketchIr,
  options: ManagedCompileOptions = {},
): CompiledManagedSource {
  return compileManagedSource(printManagedSource(ir), options);
}

/**
 * Apply one bounded semantic mutation to an exact compiled managed sketch owner.
 *
 * The current source/IR/artifact envelope is independently recompiled first.
 * Every successful result is then printed, reparsed and executed again; a
 * refusal mutates no caller-owned object and returns no partial source.
 */
export function applyManagedSketchMutation(
  current: CompiledManagedSource,
  mutation: ManagedSketchMutation,
  options: ManagedCompileOptions = {},
): ManagedMutationReceipt {
  validateMutationRequest(mutation);
  const authenticated = authenticateMutationCurrent(current, options);
  let imports = authenticated.ir.imports;
  const statements = [...authenticated.ir.statements];
  let output = authenticated.ir.output;
  switch (mutation.mutation) {
    case "insert_declarations":
      insertManagedDeclarations(
        statements,
        authenticated,
        mutation.declarations,
        options,
      );
      imports = closeGeneratedHelperImports(
        imports,
        mutation.declarations,
      );
      break;
    case "reorder_declaration":
      reorderManagedDeclaration(
        statements,
        authenticated,
        mutation.declaration,
        mutation.before,
      );
      break;
    case "set_suppressed":
      setManagedSuppression(
        statements,
        authenticated,
        mutation.target,
        mutation.suppressed,
      );
      break;
    case "set_value":
      setManagedValue(
        statements,
        mutation.declaration,
        mutation.path,
        mutation.expected,
        mutation.value,
      );
      break;
    case "set_values":
      setManagedValues(statements, mutation.values);
      break;
    case "delete":
      if (mutation.target.target === "generated") {
        // Generated members have no independent lexical declaration. Deletion
        // is the established reversible source-owned suppression operation.
        setManagedSuppression(
          statements,
          authenticated,
          mutation.target,
          true,
        );
      } else {
        const removed = deleteManagedDeclarationClosure(
          statements,
          authenticated,
          mutation.target.declaration,
        );
        output = removeOutputReferences(output, removed);
      }
      break;
  }
  if (statements.length > MANAGED_STATEMENT_LIMIT) {
    mutationFail(
      "resource_limit",
      "managed sketch mutation exceeds the statement bound",
    );
  }
  const compiled = compileMutatedStatements(
    authenticated.ir,
    imports,
    statements,
    options,
    output,
  );
  return deepFreeze({
    baseSourceDigest: authenticated.ir.source_digest,
    candidateSourceDigest: compiled.ir.source_digest,
    compiled,
  });
}

/** Exact data bytes admitted by Rust's managed sketch IR validator. */
export function canonicalManagedSketchIr(ir: ManagedSketchIr): string {
  return canonicalJson(ir);
}

/** Exact data bytes admitted by Rust's executed-artifact validator. */
export function canonicalExecutedSketchArtifact(
  artifact: ExecutedSketchArtifact,
): string {
  return canonicalJson(artifact);
}

function authenticateMutationCurrent(
  current: CompiledManagedSource,
  options: ManagedCompileOptions,
): CompiledManagedSource {
  if (
    typeof current !== "object" || current === null ||
    typeof current.normalizedSource !== "string"
  ) {
    return mutationFail(
      "invalid_current",
      "managed sketch mutation requires a compiled source owner",
    );
  }
  let recompiled: CompiledManagedSource;
  try {
    recompiled = compileManagedSource(current.normalizedSource, options);
  } catch (error) {
    if (
      error instanceof TypeError &&
      /no pinned artifact plan/u.test(error.message)
    ) {
      return mutationFail("missing_patch_plan", error.message);
    }
    return mutationFail(
      "invalid_current",
      `managed sketch current authority cannot be recompiled: ${
        errorMessage(error)
      }`,
    );
  }
  if (
    current.normalizedSource !== recompiled.normalizedSource ||
    current.canonicalIrJson !== recompiled.canonicalIrJson ||
    current.canonicalArtifactJson !== recompiled.canonicalArtifactJson
  ) {
    return mutationFail(
      "invalid_current",
      "managed sketch mutation input does not match its normalized source authority",
    );
  }
  return recompiled;
}

function compileMutatedStatements(
  owner: ManagedSketchIr,
  imports: readonly ManagedImport[],
  statements: readonly ManagedStatement[],
  options: ManagedCompileOptions,
  output: ManagedExpression = owner.output,
): CompiledManagedSource {
  const draft = { ...owner, imports, statements, output };
  let candidateSource: string | undefined;
  try {
    candidateSource = printManagedSource(draft);
    return compileManagedSource(candidateSource, options);
  } catch (error) {
    if (error instanceof ManagedMutationError) throw error;
    if (
      error instanceof TypeError &&
      /no pinned artifact plan/u.test(error.message)
    ) {
      return mutationFail("missing_patch_plan", error.message);
    }
    return mutationFail(
      "invalid_draft",
      `managed sketch mutation is invalid: ${errorMessage(error)}`,
      typeof candidateSource === "string"
        ? {
          source: candidateSource,
          span: error instanceof ManagedCompileError
            ? error.span
            : { start: 0, end: 0 },
        }
        : undefined,
    );
  }
}

function closeGeneratedHelperImports(
  imports: readonly ManagedImport[],
  drafts: readonly ManagedDeclarationDraft[],
): readonly ManagedImport[] {
  const closed = imports.map((entry) => ({
    module: entry.module,
    bindings: [...entry.bindings],
  }));
  const imported = new Set(imports.flatMap((entry) => entry.bindings));
  const missing = requiredGeneratedUnitHelpers(drafts).filter((helper) =>
    !imported.has(helper)
  );
  if (missing.length === 0) return closed;

  const sdkIndex = closed.findIndex((entry) =>
    entry.module === "@geosolve/sketch-code" &&
    entry.bindings.includes("sketch")
  );
  const sdkImport = closed[sdkIndex];
  if (sdkIndex < 0 || sdkImport === undefined) {
    return mutationFail(
      "invalid_current",
      "managed sketch mutation cannot locate its sketch SDK import",
    );
  }
  closed[sdkIndex] = {
    ...sdkImport,
    bindings: [...sdkImport.bindings, ...missing],
  };
  return closed;
}

function requiredGeneratedUnitHelpers(
  drafts: readonly ManagedDeclarationDraft[],
): readonly GeneratedUnitHelper[] {
  const required = new Set<GeneratedUnitHelper>();
  const visit = (value: ManagedValue): void => {
    switch (value.kind) {
      case "unit":
        required.add(generatedUnitHelper(value.value.unit));
        return;
      case "array":
        value.value.forEach(visit);
        return;
      case "object":
        Object.values(value.value).forEach(visit);
        return;
      case "null":
      case "bool":
      case "number":
      case "string":
      case "reference":
        return;
    }
  };
  drafts.forEach((draft) => visit(draft.arguments));
  return GENERATED_UNIT_HELPERS.filter((helper) => required.has(helper));
}

function generatedUnitHelper(unit: string): GeneratedUnitHelper {
  switch (unit) {
    case "mm":
    case "rad":
      return unit;
    default:
      return mutationFail(
        "invalid_draft",
        `unsupported managed sketch unit ${unit}`,
      );
  }
}

function insertManagedDeclarations(
  statements: ManagedStatement[],
  current: CompiledManagedSource,
  drafts: readonly ManagedDeclarationDraft[],
  options: ManagedCompileOptions,
): void {
  if (drafts.length === 0) {
    return mutationFail(
      "invalid_draft",
      "managed sketch declaration insertion is empty",
    );
  }
  if (drafts.length > MANAGED_MUTATION_BATCH_LIMIT) {
    return mutationFail(
      "resource_limit",
      `managed sketch insertion has ${drafts.length} declarations; the limit is ${MANAGED_MUTATION_BATCH_LIMIT}`,
    );
  }
  const occupiedVariables = new Set<string>();
  const symbols = new Set<string>();
  for (const statement of statements) {
    if (
      statement.statement === "binding" || statement.statement === "declaration"
    ) {
      occupiedVariables.add(statement.variable);
    }
    if (statement.statement === "declaration") symbols.add(statement.symbol);
  }
  const existingGroupIndex = statements.findIndex((statement) =>
    statement.statement === "group" &&
    statement.name === MANAGED_CANVAS_ADDITIONS_GROUP
  );
  const insertionIndex = existingGroupIndex >= 0
    ? existingGroupIndex
    : statements.reduce(
      (last, statement, index) =>
        statement.statement === "binding" ||
          statement.statement === "declaration"
          ? index + 1
          : last,
      0,
    );
  const availableVariables = new Set(
    statements.slice(0, insertionIndex).flatMap((statement) =>
      statement.statement === "binding" || statement.statement === "declaration"
        ? [statement.variable]
        : []
    ),
  );
  const insertedStatements: ManagedDeclarationStatement[] = [];
  const insertedSuppressionVariables: string[] = [];
  for (const draft of drafts) {
    validateDeclarationDraft(
      draft,
      current,
      occupiedVariables,
      availableVariables,
      symbols,
      options,
    );
    occupiedVariables.add(draft.variable);
    availableVariables.add(draft.variable);
    symbols.add(draft.symbol);
    if (draft.suppressed === true) {
      insertedSuppressionVariables.push(draft.variable);
    }
    insertedStatements.push({
      statement: "declaration",
      variable: draft.variable,
      symbol: draft.symbol,
      builder_path: [...draft.builder_path],
      patch: draft.patch ?? null,
      arguments: managedValueToExpression(draft.arguments),
      site: mutationPlaceholderSite("declaration"),
      comments: [...(draft.comments ?? [])],
    });
  }
  statements.splice(insertionIndex, 0, ...insertedStatements);

  const grouped = drafts.filter((draft) =>
    (draft.group ?? MANAGED_CANVAS_ADDITIONS_GROUP) ===
      MANAGED_CANVAS_ADDITIONS_GROUP
  ).map((draft) => draft.variable);
  if (grouped.length !== drafts.length) {
    return mutationFail(
      "invalid_draft",
      `managed sketch canvas insertions must use the ${
        JSON.stringify(MANAGED_CANVAS_ADDITIONS_GROUP)
      } group`,
    );
  }
  const groupIndex = existingGroupIndex < 0
    ? -1
    : existingGroupIndex + insertedStatements.length;
  if (groupIndex < 0) {
    statements.push({
      statement: "group",
      name: MANAGED_CANVAS_ADDITIONS_GROUP,
      declarations: grouped.map((declaration) => ({
        declaration,
        path: [],
        site: mutationPlaceholderSite("group_reference"),
      })),
      site: mutationPlaceholderSite("group"),
      comments: [],
    });
  } else {
    const group = statements[groupIndex];
    if (group?.statement !== "group") {
      throw new TypeError("managed group index changed");
    }
    statements[groupIndex] = {
      ...group,
      declarations: [
        ...group.declarations,
        ...grouped.map((declaration) => ({
          declaration,
          path: [],
          site: mutationPlaceholderSite("group_reference"),
        })),
      ],
    };
  }
  for (const declaration of insertedSuppressionVariables) {
    statements.push({
      statement: "suppression",
      target: {
        declaration,
        path: [],
        site: mutationPlaceholderSite("suppression_reference"),
      },
      site: mutationPlaceholderSite("suppression"),
      comments: [],
    });
  }
}

function validateDeclarationDraft(
  draft: ManagedDeclarationDraft,
  current: CompiledManagedSource,
  occupiedVariables: ReadonlySet<string>,
  availableVariables: ReadonlySet<string>,
  symbols: ReadonlySet<string>,
  options: ManagedCompileOptions,
): void {
  if (!isRecord(draft)) {
    return mutationFail(
      "invalid_draft",
      "managed sketch declaration draft must be an object",
    );
  }
  requireMutationIdentifier(draft.variable, "declaration variable");
  requireMutationText(draft.symbol, "declaration symbol");
  if (occupiedVariables.has(draft.variable)) {
    return mutationFail(
      "duplicate_declaration",
      `managed binding ${draft.variable} already exists`,
    );
  }
  if (symbols.has(draft.symbol)) {
    return mutationFail(
      "duplicate_declaration",
      `managed declaration ${draft.symbol} already exists`,
    );
  }
  if (
    !Array.isArray(draft.builder_path) ||
    draft.builder_path.length === 0 ||
    draft.builder_path.length > MANAGED_MUTATION_PATH_LIMIT ||
    draft.builder_path.some((segment) =>
      typeof segment !== "string" || !validIdentifier(segment)
    )
  ) {
    return mutationFail(
      "invalid_draft",
      `managed declaration ${draft.symbol} has an invalid builder path`,
    );
  }
  if (
    draft.group !== undefined &&
    draft.group !== MANAGED_CANVAS_ADDITIONS_GROUP
  ) {
    return mutationFail(
      "invalid_draft",
      `managed sketch canvas insertions must use the ${
        JSON.stringify(MANAGED_CANVAS_ADDITIONS_GROUP)
      } group`,
    );
  }
  if (draft.suppressed !== undefined && typeof draft.suppressed !== "boolean") {
    return mutationFail(
      "invalid_draft",
      "managed sketch declaration suppression must be boolean",
    );
  }
  if (draft.comments !== undefined && !Array.isArray(draft.comments)) {
    return mutationFail(
      "invalid_draft",
      "managed sketch declaration comments must be an array",
    );
  }
  const patch = draft.patch ?? null;
  if (patch === null) {
    const family = draft.builder_path.join(".");
    if (!(family in DECLARATION_RESULT_CATALOG)) {
      return mutationFail(
        "invalid_draft",
        `unsupported managed sketch declaration family ${family}`,
      );
    }
  } else {
    requireMutationIdentifier(patch, "patch binding");
    if (!current.ir.imports.some((entry) => entry.bindings.includes(patch))) {
      return mutationFail(
        "invalid_draft",
        `patch binding ${patch} is not statically imported`,
      );
    }
    if (options.patches?.[patch] === undefined) {
      return mutationFail(
        "missing_patch_plan",
        `managed sketch patch ${patch} has no pinned artifact plan`,
      );
    }
  }
  validateMutationComments(draft.comments ?? []);
  validateManagedDraftValue(draft.arguments, availableVariables, 0, {
    count: 0,
  });
  if (draft.arguments.kind !== "object") {
    return mutationFail(
      "invalid_draft",
      `managed declaration ${draft.symbol} arguments must be an object`,
    );
  }
}

function reorderManagedDeclaration(
  statements: ManagedStatement[],
  current: CompiledManagedSource,
  declaration: string,
  before: string | null,
): void {
  requireMutationText(declaration, "declaration symbol");
  if (before !== null) {
    requireMutationText(before, "destination declaration symbol");
  }
  const closures = projectSourceDeclarationClosures(current.ir);
  const sourceOwner = closureRootForHelper(closures, declaration);
  if (sourceOwner !== undefined) {
    return ownedHelperMutationFail(
      declaration,
      sourceOwner,
      "moved",
      "move",
    );
  }
  if (before !== null) {
    const destinationOwner = closureRootForHelper(closures, before);
    if (destinationOwner !== undefined) {
      return ownedHelperMutationFail(
        before,
        destinationOwner,
        "used as a reorder destination",
        "target",
      );
    }
  }
  const declarationIndexes = topLevelDeclarationIndexes(statements);
  const source = declarationIndexes.find((entry) =>
    entry.statement.symbol === declaration
  );
  if (source === undefined) {
    return mutationFail(
      "unknown_declaration",
      `managed declaration ${declaration} does not exist`,
    );
  }
  if (before === declaration) return;
  if (
    before !== null &&
    !declarationIndexes.some((entry) => entry.statement.symbol === before)
  ) {
    return mutationFail(
      "unknown_declaration",
      `managed declaration ${before} does not exist`,
    );
  }
  const sourceClosure = closureForRoot(closures, declaration);
  const sourceSymbols = sourceClosure === undefined
    ? new Set([declaration])
    : closureSymbols(sourceClosure);
  const moved = declarationIndexes
    .map((entry) => entry.statement)
    .filter((statement) => sourceSymbols.has(statement.symbol));
  const reordered = declarationIndexes
    .map((entry) => entry.statement)
    .filter((statement) => !sourceSymbols.has(statement.symbol));
  const destinationClosure = before === null
    ? undefined
    : closureForRoot(closures, before);
  const destinationSymbols = destinationClosure === undefined
    ? undefined
    : closureSymbols(destinationClosure);
  const destination = before === null
    ? reordered.length
    : reordered.findIndex((statement) =>
      destinationSymbols === undefined
        ? statement.symbol === before
        : destinationSymbols.has(statement.symbol)
    );
  if (destination < 0) {
    return mutationFail(
      "unknown_declaration",
      `managed declaration ${before} does not exist`,
    );
  }
  reordered.splice(destination, 0, ...moved);
  const slots = declarationIndexes.map((entry) => entry.index);
  slots.forEach((slot, index) => {
    statements[slot] = reordered[index]!;
  });
  validateLexicalStatementOrder(statements);
}

function validateLexicalStatementOrder(
  statements: readonly ManagedStatement[],
): void {
  const seen = new Set<string>();
  const declarations = new Map(
    statements.flatMap((statement) =>
      statement.statement === "declaration"
        ? [[statement.variable, statement.symbol] as const]
        : []
    ),
  );
  for (const statement of statements) {
    let references: readonly string[];
    let consumer: string;
    switch (statement.statement) {
      case "binding":
        references = expressionReferences(statement.value);
        consumer = `binding ${statement.variable}`;
        break;
      case "declaration":
        references = expressionReferences(statement.arguments);
        consumer = `declaration ${statement.symbol}`;
        break;
      case "group":
        references = statement.declarations.map((reference) =>
          reference.declaration
        );
        consumer = `group ${JSON.stringify(statement.name)}`;
        break;
      case "suppression":
        references = [statement.target.declaration];
        consumer = "suppression";
        break;
    }
    for (const dependency of references) {
      if (seen.has(dependency)) continue;
      const producer = declarations.get(dependency) ?? `binding ${dependency}`;
      return mutationFail(
        "dependency_invalid_move",
        `managed ${consumer} must remain after ${producer}`,
      );
    }
    if (
      statement.statement === "binding" || statement.statement === "declaration"
    ) {
      seen.add(statement.variable);
    }
  }
}

function setManagedSuppression(
  statements: ManagedStatement[],
  current: CompiledManagedSource,
  target: ManagedMutationTarget,
  suppressed: boolean,
): void {
  refuseOwnedHelperMutation(current, target, "suppressed", "suppress");
  const reference = mutationTargetReference(current, target);
  const matching = (statement: ManagedStatement): boolean =>
    statement.statement === "suppression" &&
    sameReference(statement.target, reference);
  const existing = statements.reduce<number[]>((indexes, statement, index) => {
    if (matching(statement)) indexes.push(index);
    return indexes;
  }, []);
  if (existing.length > 1) {
    return mutationFail(
      "invalid_current",
      "managed sketch source repeats an exact suppression target",
    );
  }
  if (suppressed) {
    if (existing.length === 0) {
      statements.push({
        statement: "suppression",
        target: {
          ...reference,
          site: mutationPlaceholderSite("suppression_reference"),
        },
        site: mutationPlaceholderSite("suppression"),
        comments: [],
      });
    }
  } else if (existing[0] !== undefined) {
    statements.splice(existing[0], 1);
  }
}

function setManagedValue(
  statements: ManagedStatement[],
  declaration: string,
  path: readonly SemanticPathSegment[],
  expected: ManagedValue,
  replacement: ManagedValue,
): void {
  requireMutationText(declaration, "value owner declaration");
  validateMutationPath(path);
  const variables = new Set(
    statements.flatMap((statement) =>
      statement.statement === "binding" || statement.statement === "declaration"
        ? [statement.variable]
        : []
    ),
  );
  validateManagedDraftValue(expected, variables, 0, { count: 0 });
  validateManagedDraftValue(replacement, variables, 0, { count: 0 });
  const indexes = statements.flatMap((statement, index) => {
    if (statement.statement === "binding" && statement.variable === declaration) {
      return [index];
    }
    if (
      statement.statement === "declaration" &&
      statement.symbol === declaration
    ) return [index];
    return [];
  });
  if (indexes.length !== 1) {
    return mutationFail(
      "unknown_declaration",
      "managed sketch value owner is absent or ambiguous",
    );
  }
  const index = indexes[0]!;
  const statement = statements[index]!;
  const root = statement.statement === "binding"
    ? statement.value
    : statement.statement === "declaration"
    ? statement.arguments
    : mutationFail("unknown_declaration", "managed sketch value owner disappeared");
  const current = expressionAtMutationPath(root, path);
  if (
    canonicalJson(expressionToManagedValue(current)) !==
      canonicalJson(expected)
  ) {
    return mutationFail(
      "invalid_current",
      "managed sketch value mutation expected value is stale",
    );
  }
  const next = replaceExpressionAtMutationPath(
    root,
    path,
    managedValueToExpression(replacement),
  );
  if (statement.statement === "binding") {
    statements[index] = { ...statement, value: next };
  } else if (statement.statement === "declaration") {
    statements[index] = { ...statement, arguments: next };
  } else {
    return mutationFail(
      "unknown_declaration",
      "managed sketch value owner disappeared",
    );
  }
}

function setManagedValues(
  statements: ManagedStatement[],
  values: readonly ManagedValueMutation[],
): void {
  if (values.length === 0) {
    return mutationFail("invalid_draft", "managed sketch value batch is empty");
  }
  if (values.length > MANAGED_MUTATION_BATCH_LIMIT) {
    return mutationFail(
      "resource_limit",
      `managed sketch value batch has ${values.length} edits; the limit is ${MANAGED_MUTATION_BATCH_LIMIT}`,
    );
  }
  const coordinates = new Set<string>();
  for (const value of values) {
    requireMutationText(value.declaration, "value owner declaration");
    validateMutationPath(value.path);
    const coordinate = canonicalJson([value.declaration, value.path]);
    if (coordinates.has(coordinate)) {
      return mutationFail(
        "invalid_draft",
        "managed sketch value batch repeats an owner/path coordinate",
      );
    }
    coordinates.add(coordinate);
  }
  for (let left = 0; left < values.length; left += 1) {
    for (let right = left + 1; right < values.length; right += 1) {
      const a = values[left]!;
      const b = values[right]!;
      if (a.declaration !== b.declaration) continue;
      const common = Math.min(a.path.length, b.path.length);
      if (
        canonicalJson(a.path.slice(0, common)) ===
          canonicalJson(b.path.slice(0, common))
      ) {
        return mutationFail(
          "invalid_draft",
          "managed sketch value batch contains overlapping paths",
        );
      }
    }
  }
  for (const value of values) {
    setManagedValue(
      statements,
      value.declaration,
      value.path,
      value.expected,
      value.value,
    );
  }
}

function expressionAtMutationPath(
  expression: ManagedExpression,
  path: readonly SemanticPathSegment[],
): ManagedExpression {
  let current = expression;
  for (const segment of path) {
    if (typeof segment === "string" && current.kind === "object") {
      const matches = current.fields.filter((field) => field.name === segment);
      if (matches.length !== 1) {
        return mutationFail("invalid_draft", "managed sketch value field is unavailable");
      }
      current = matches[0]!.value;
    } else if (
      typeof segment === "number" && current.kind === "array" &&
      segment < current.values.length
    ) {
      current = current.values[segment]!;
    } else {
      return mutationFail(
        "invalid_draft",
        "managed sketch value path does not address a lexical object field or array item",
      );
    }
  }
  return current;
}

function replaceExpressionAtMutationPath(
  expression: ManagedExpression,
  path: readonly SemanticPathSegment[],
  replacement: ManagedExpression,
): ManagedExpression {
  if (path.length === 0) return replacement;
  const [segment, ...rest] = path;
  if (typeof segment === "string" && expression.kind === "object") {
    let changed = 0;
    const fields = expression.fields.map((field) => {
      if (field.name !== segment) return field;
      changed += 1;
      return {
        ...field,
        value: replaceExpressionAtMutationPath(
          field.value,
          rest,
          replacement,
        ),
      };
    });
    if (changed !== 1) {
      return mutationFail("invalid_draft", "managed sketch value field is unavailable");
    }
    return { ...expression, fields };
  }
  if (
    typeof segment === "number" && expression.kind === "array" &&
    segment < expression.values.length
  ) {
    const values = [...expression.values];
    values[segment] = replaceExpressionAtMutationPath(
      values[segment]!,
      rest,
      replacement,
    );
    return { ...expression, values };
  }
  return mutationFail(
    "invalid_draft",
    "managed sketch value path does not address a lexical object field or array item",
  );
}

function expressionToManagedValue(
  expression: ManagedExpression,
): ManagedValue {
  switch (expression.kind) {
    case "null":
      return { kind: "null" };
    case "boolean":
      return { kind: "bool", value: expression.value };
    case "number":
      return { kind: "number", value: expression.value };
    case "string":
      return { kind: "string", value: expression.value };
    case "array":
      return {
        kind: "array",
        value: expression.values.map(expressionToManagedValue),
      };
    case "object":
      return {
        kind: "object",
        value: Object.fromEntries(expression.fields.map((field) => [
          field.name,
          expressionToManagedValue(field.value),
        ])),
      };
    case "reference":
      return {
        kind: "reference",
        value: {
          declaration: expression.declaration,
          path: expression.path,
        },
      };
    case "call":
      if (
        ["mm", "cm", "m", "inch", "deg", "rad"].includes(expression.callee) &&
        expression.arguments.length === 1 &&
        expression.arguments[0]?.kind === "number"
      ) {
        return {
          kind: "unit",
          value: {
            unit: expression.callee,
            value: expression.arguments[0].value,
          },
        };
      }
      return mutationFail(
        "invalid_current",
        "managed sketch value mutation encountered an unsupported lexical call",
      );
  }
}

function mutationTargetReference(
  current: CompiledManagedSource,
  target: ManagedMutationTarget,
): Pick<ManagedReference, "declaration" | "path"> {
  if (target.target === "declaration") {
    const statement = current.ir.statements.find((candidate) =>
      candidate.statement === "declaration" &&
      candidate.symbol === target.declaration
    );
    if (statement?.statement !== "declaration") {
      return mutationFail(
        "unknown_declaration",
        `managed declaration ${target.declaration} does not exist`,
      );
    }
    return { declaration: statement.variable, path: [] };
  }
  validateGeneratedAddressShape(target.address);
  const matches = current.artifact.generated_members.filter((member) =>
    sameGeneratedAddress(member.address, target.address)
  );
  if (matches.length === 0) {
    return mutationFail(
      "unknown_generated_member",
      "managed generated member does not exist",
    );
  }
  if (matches.length > 1) {
    return mutationFail(
      "ambiguous_generated_member",
      "managed generated member is ambiguous",
    );
  }
  const owner = current.ir.statements.find((candidate) =>
    candidate.statement === "declaration" &&
    candidate.symbol === target.address.invocation
  );
  if (owner?.statement !== "declaration" || owner.patch === null) {
    return mutationFail(
      "invalid_current",
      "managed generated member has no lexical patch owner",
    );
  }
  const path = generatedMemberReferencePath(current, owner, target.address);
  return { declaration: owner.variable, path };
}

function generatedMemberReferencePath(
  current: CompiledManagedSource,
  owner: ManagedDeclarationStatement,
  address: ExecutedGeneratedMemberAddress,
): SemanticPathSegment[] {
  const resultPaths =
    current.artifact.declarations.find((candidate) =>
      candidate.declaration === owner.symbol
    )?.result.map(({ path }) => path) ?? [];
  const candidates: SemanticPathSegment[][] = [];
  for (const root of resultPaths) {
    if (address.member_key.length > 0) {
      candidates.push([
        ...root,
        ...address.member_key.map((member) => ({ member })),
      ]);
    } else {
      candidates.push([...root]);
    }
  }
  if (address.member_key.length === 0) candidates.push([...address.template]);
  const exact = candidates.find((path) =>
    current.artifact.suppressions.some((suppression) =>
      suppression.target.declaration === owner.variable &&
      sameSemanticPath(suppression.target.path, path)
    )
  );
  if (exact !== undefined) return exact;
  if (resultPaths.length === 1) {
    return address.member_key.length > 0
      ? [
        ...resultPaths[0]!,
        ...address.member_key.map((member) => ({ member })),
      ]
      : [...resultPaths[0]!];
  }
  const template = resultPaths.find((path) =>
    sameStringPath(
      path.filter((segment): segment is string => typeof segment === "string"),
      address.template,
    )
  );
  if (template !== undefined) return [...template];
  return mutationFail(
    "ambiguous_generated_member",
    "managed generated member cannot be projected to one lexical result path",
  );
}

function deleteManagedDeclarationClosure(
  statements: ManagedStatement[],
  current: CompiledManagedSource,
  declaration: string,
): Set<string> {
  const closures = projectSourceDeclarationClosures(current.ir);
  const helperOwner = closureRootForHelper(closures, declaration);
  if (helperOwner !== undefined) {
    return ownedHelperMutationFail(
      declaration,
      helperOwner,
      "deleted",
      "delete",
    );
  }
  const owner = statements.find((statement) =>
    statement.statement === "declaration" && statement.symbol === declaration
  );
  if (owner?.statement !== "declaration") {
    return mutationFail(
      "unknown_declaration",
      `managed declaration ${declaration} does not exist`,
    );
  }
  const removed = new Set<string>([owner.variable]);
  const closure = closureForRoot(closures, declaration);
  if (closure !== undefined) {
    const variablesBySymbol = new Map(
      statements.flatMap((statement) =>
        statement.statement === "declaration"
          ? [[statement.symbol, statement.variable] as const]
          : []
      ),
    );
    for (const helper of closure.helpers) {
      const variable = variablesBySymbol.get(helper);
      if (variable === undefined) {
        return mutationFail(
          "invalid_current",
          "authenticated Profile Offset closure lost a helper declaration",
        );
      }
      removed.add(variable);
    }
  }
  let changed = true;
  while (changed) {
    changed = false;
    for (const statement of statements) {
      if (
        statement.statement !== "declaration" &&
        statement.statement !== "binding"
      ) continue;
      if (removed.has(statement.variable)) continue;
      const value = statement.statement === "declaration"
        ? statement.arguments
        : statement.value;
      if (
        expressionReferences(value).some((reference) =>
          removed.has(reference)
        )
      ) {
        removed.add(statement.variable);
        changed = true;
      }
    }
  }
  const bindings = new Map(
    statements.flatMap((statement) =>
      statement.statement === "binding"
        ? [[statement.variable, statement] as const]
        : []
    ),
  );
  const byVariable = new Map(
    statements.flatMap((statement) =>
      statement.statement === "binding" || statement.statement === "declaration"
        ? [[statement.variable, statement] as const]
        : []
    ),
  );
  const prunableBindings = new Set<string>();
  const visitUpstreamBinding = (variable: string): void => {
    const binding = bindings.get(variable);
    if (binding === undefined || prunableBindings.has(variable)) return;
    prunableBindings.add(variable);
    expressionReferences(binding.value).forEach(visitUpstreamBinding);
  };
  for (const variable of removed) {
    const statement = byVariable.get(variable);
    if (statement === undefined) continue;
    const value = statement.statement === "declaration"
      ? statement.arguments
      : statement.value;
    expressionReferences(value).forEach(visitUpstreamBinding);
  }
  const retained: ManagedStatement[] = [];
  for (const statement of statements) {
    if (
      (statement.statement === "declaration" ||
        statement.statement === "binding") &&
      removed.has(statement.variable)
    ) continue;
    if (
      statement.statement === "suppression" &&
      removed.has(statement.target.declaration)
    ) continue;
    if (statement.statement === "group") {
      const declarations = statement.declarations.filter((reference) =>
        !removed.has(reference.declaration)
      );
      if (declarations.length === 0) continue;
      retained.push({ ...statement, declarations });
      continue;
    }
    retained.push(statement);
  }
  statements.splice(0, statements.length, ...retained);
  pruneUnusedBindings(statements, prunableBindings);
  return removed;
}

function removeOutputReferences(
  expression: ManagedExpression,
  removed: ReadonlySet<string>,
): ManagedExpression {
  const retained = retainOutputExpression(expression, removed);
  return retained ?? {
    kind: "object",
    fields: [],
    site: expression.site,
  };
}

function retainOutputExpression(
  expression: ManagedExpression,
  removed: ReadonlySet<string>,
): ManagedExpression | undefined {
  switch (expression.kind) {
    case "reference":
      return removed.has(expression.declaration) ? undefined : expression;
    case "array":
      return {
        ...expression,
        values: expression.values.flatMap((value) => {
          const retained = retainOutputExpression(value, removed);
          return retained === undefined ? [] : [retained];
        }),
      };
    case "object":
      return {
        ...expression,
        fields: expression.fields.flatMap((field) => {
          const retained = retainOutputExpression(field.value, removed);
          return retained === undefined ? [] : [{ ...field, value: retained }];
        }),
      };
    case "call": {
      const arguments_ = expression.arguments.map((argument) =>
        retainOutputExpression(argument, removed)
      );
      return arguments_.some((argument) => argument === undefined)
        ? undefined
        : { ...expression, arguments: arguments_ as ManagedExpression[] };
    }
    case "null":
    case "boolean":
    case "number":
    case "string":
      return expression;
  }
}

function pruneUnusedBindings(
  statements: ManagedStatement[],
  candidates: ReadonlySet<string>,
): void {
  let changed = true;
  while (changed) {
    changed = false;
    const used = new Set<string>();
    for (const statement of statements) {
      if (statement.statement === "binding") {
        expressionReferences(statement.value).forEach((reference) =>
          used.add(reference)
        );
      } else if (statement.statement === "declaration") {
        expressionReferences(statement.arguments).forEach((reference) =>
          used.add(reference)
        );
      } else if (statement.statement === "group") {
        statement.declarations.forEach((reference) =>
          used.add(reference.declaration)
        );
      } else {
        used.add(statement.target.declaration);
      }
    }
    const index = statements.findIndex((statement) =>
      statement.statement === "binding" &&
      candidates.has(statement.variable) &&
      !used.has(statement.variable)
    );
    if (index >= 0) {
      statements.splice(index, 1);
      changed = true;
    }
  }
}

function topLevelDeclarationIndexes(
  statements: readonly ManagedStatement[],
): {
  readonly index: number;
  readonly statement: ManagedDeclarationStatement;
}[] {
  return statements.flatMap((statement, index) =>
    statement.statement === "declaration" ? [{ index, statement }] : []
  );
}

function validateMutationRequest(mutation: ManagedSketchMutation): void {
  if (!isRecord(mutation)) {
    return mutationFail(
      "invalid_draft",
      "managed sketch mutation request must be an object",
    );
  }
  const request = mutation as unknown as Readonly<Record<string, unknown>>;
  switch (request.mutation) {
    case "insert_declarations": {
      if (!Array.isArray(request.declarations)) {
        return mutationFail(
          "invalid_draft",
          "managed sketch insertion declarations must be an array",
        );
      }
      if (request.declarations.length === 0) {
        return mutationFail(
          "invalid_draft",
          "managed sketch declaration insertion is empty",
        );
      }
      if (request.declarations.length > MANAGED_MUTATION_BATCH_LIMIT) {
        return mutationFail(
          "resource_limit",
          `managed sketch insertion has ${request.declarations.length} declarations; the limit is ${MANAGED_MUTATION_BATCH_LIMIT}`,
        );
      }
      return;
    }
    case "reorder_declaration":
      requireMutationText(request.declaration, "declaration symbol");
      if (request.before !== null) {
        requireMutationText(request.before, "destination declaration symbol");
      }
      return;
    case "set_suppressed":
      validateMutationTargetShape(request.target);
      if (typeof request.suppressed !== "boolean") {
        return mutationFail(
          "invalid_draft",
          "managed sketch suppression state must be boolean",
        );
      }
      return;
    case "set_value":
      requireMutationText(request.declaration, "value owner declaration");
      validateMutationPath(request.path as readonly SemanticPathSegment[]);
      if (!isRecord(request.expected) || !isRecord(request.value)) {
        return mutationFail(
          "invalid_draft",
          "managed sketch value mutation requires expected and replacement values",
        );
      }
      validateManagedDraftValue(
        request.expected as ManagedValue,
        null,
        0,
        { count: 0 },
      );
      validateManagedDraftValue(
        request.value as ManagedValue,
        null,
        0,
        { count: 0 },
      );
      return;
    case "set_values":
      if (!Array.isArray(request.values) || request.values.length === 0) {
        return mutationFail(
          "invalid_draft",
          "managed sketch value batch must be a non-empty array",
        );
      }
      if (request.values.length > MANAGED_MUTATION_BATCH_LIMIT) {
        return mutationFail(
          "resource_limit",
          `managed sketch value batch has ${request.values.length} edits; the limit is ${MANAGED_MUTATION_BATCH_LIMIT}`,
        );
      }
      for (const value of request.values) {
        if (!isRecord(value)) {
          return mutationFail(
            "invalid_draft",
            "managed sketch value batch entry must be an object",
          );
        }
        requireMutationText(value.declaration, "value owner declaration");
        validateMutationPath(value.path as readonly SemanticPathSegment[]);
        if (!isRecord(value.expected) || !isRecord(value.value)) {
          return mutationFail(
            "invalid_draft",
            "managed sketch value batch requires expected and replacement values",
          );
        }
        validateManagedDraftValue(
          value.expected as ManagedValue,
          null,
          0,
          { count: 0 },
        );
        validateManagedDraftValue(
          value.value as ManagedValue,
          null,
          0,
          { count: 0 },
        );
      }
      return;
    case "delete":
      validateMutationTargetShape(request.target);
      return;
    default:
      return mutationFail(
        "invalid_draft",
        "managed sketch mutation operation is unsupported",
      );
  }
}

function validateMutationTargetShape(target: unknown): void {
  if (!isRecord(target)) {
    return mutationFail(
      "invalid_draft",
      "managed sketch mutation target must be an object",
    );
  }
  if (target.target === "declaration") {
    requireMutationText(target.declaration, "declaration symbol");
    return;
  }
  if (target.target === "generated") {
    validateGeneratedAddressShape(target.address);
    return;
  }
  return mutationFail(
    "invalid_draft",
    "managed sketch mutation target kind is unsupported",
  );
}

function managedValueToExpression(value: ManagedValue): ManagedExpression {
  const site = mutationPlaceholderSite("value");
  switch (value.kind) {
    case "null":
      return { kind: "null", site };
    case "bool":
      return { kind: "boolean", value: value.value, site };
    case "number":
      return { kind: "number", value: value.value, site };
    case "string":
      return { kind: "string", value: value.value, site };
    case "unit": {
      const helper = generatedUnitHelper(value.value.unit);
      return {
        kind: "call",
        callee: helper,
        arguments: [{ kind: "number", value: value.value.value, site }],
        site,
      };
    }
    case "array":
      return {
        kind: "array",
        values: value.value.map(managedValueToExpression),
        site,
      };
    case "object":
      return {
        kind: "object",
        fields: Object.entries(value.value).map(([name, child]) => ({
          name,
          value: managedValueToExpression(child),
          comments: [],
        })),
        site,
      };
    case "reference":
      return {
        kind: "reference",
        declaration: value.value.declaration,
        path: [...value.value.path],
        site,
      };
  }
}

function validateManagedDraftValue(
  value: ManagedValue,
  // `null` is used only by the pre-authentication resource/shape pass. The
  // exact current lexical variable set is supplied by insertion/value
  // mutation once the compiled owner has been authenticated.
  variables: ReadonlySet<string> | null,
  depth: number,
  budget: { count: number },
): void {
  budget.count += 1;
  if (
    budget.count > MANAGED_MUTATION_VALUE_NODE_LIMIT ||
    depth > MANAGED_MUTATION_VALUE_DEPTH_LIMIT
  ) {
    return mutationFail(
      "resource_limit",
      "managed sketch declaration value exceeds its bound",
    );
  }
  if (!isRecord(value)) {
    return mutationFail(
      "invalid_draft",
      "managed sketch declaration value must be data-only",
    );
  }
  switch (value.kind) {
    case "null":
      return;
    case "bool":
      if (typeof value.value !== "boolean") {
        return mutationFail(
          "invalid_draft",
          "managed sketch declaration boolean is invalid",
        );
      }
      return;
    case "number":
      if (typeof value.value !== "number" || !Number.isFinite(value.value)) {
        return mutationFail(
          "invalid_draft",
          "managed sketch declaration contains a non-finite number",
        );
      }
      return;
    case "string":
      requireMutationText(value.value, "managed string");
      return;
    case "unit":
      if (!isRecord(value.value)) {
        return mutationFail(
          "invalid_draft",
          "managed sketch unit must be an object",
        );
      }
      requireMutationText(value.value.unit, "managed unit");
      if (
        typeof value.value.value !== "number" ||
        !Number.isFinite(value.value.value)
      ) {
        return mutationFail(
          "invalid_draft",
          "managed sketch unit contains a non-finite number",
        );
      }
      return;
    case "reference":
      if (!isRecord(value.value)) {
        return mutationFail(
          "invalid_draft",
          "managed sketch reference must be an object",
        );
      }
      requireMutationIdentifier(
        value.value.declaration,
        "reference declaration",
      );
      if (variables !== null && !variables.has(value.value.declaration)) {
        return mutationFail(
          "invalid_draft",
          `managed declaration references unknown or forward binding ${value.value.declaration}`,
        );
      }
      validateMutationPath(value.value.path);
      return;
    case "array":
      if (!Array.isArray(value.value)) {
        return mutationFail(
          "invalid_draft",
          "managed sketch declaration array is invalid",
        );
      }
      value.value.forEach((child) =>
        validateManagedDraftValue(child, variables, depth + 1, budget)
      );
      return;
    case "object":
      if (!isRecord(value.value)) {
        return mutationFail(
          "invalid_draft",
          "managed sketch declaration object is invalid",
        );
      }
      for (const [name, child] of Object.entries(value.value)) {
        requireMutationText(name, "managed object field");
        validateManagedDraftValue(child, variables, depth + 1, budget);
      }
      return;
    default:
      return mutationFail(
        "invalid_draft",
        "managed sketch declaration value kind is unsupported",
      );
  }
}

function expressionReferences(value: ManagedExpression): string[] {
  switch (value.kind) {
    case "reference":
      return [value.declaration];
    case "array":
      return value.values.flatMap(expressionReferences);
    case "object":
      return value.fields.flatMap((field) =>
        expressionReferences(field.value)
      );
    case "call":
      return value.arguments.flatMap(expressionReferences);
    default:
      return [];
  }
}

interface SourceClosureDeclaration {
  readonly variable: string;
  readonly symbol: string;
  readonly builderPath: readonly string[];
  readonly patch: string | null;
  readonly arguments: ManagedExpression;
}

interface ProfileOffsetHelperKind {
  readonly inputKind: "chain" | "profile";
}

const PROFILE_OFFSET_HELPER_KINDS_: readonly ProfileOffsetHelperKind[] = [
  {
    inputKind: "chain",
  },
  {
    inputKind: "profile",
  },
];

function projectSourceDeclarationClosures(
  ir: ManagedSketchIr,
): ManagedSourceDeclarationClosure[] {
  const declarations = ir.statements.flatMap((statement) =>
    statement.statement === "declaration"
      ? [{
        variable: statement.variable,
        symbol: statement.symbol,
        builderPath: statement.builder_path,
        patch: statement.patch,
        arguments: statement.arguments,
      }]
      : []
  );
  const helpersByRoot = new Map<string, string[]>();
  for (const helper of declarations) {
    const helperKind = profileOffsetHelperKind(helper);
    if (helperKind === undefined) continue;
    const consumers = ir.statements.filter((consumer) => {
      const expression = consumer.statement === "binding"
        ? consumer.value
        : consumer.statement === "declaration"
        ? consumer.arguments
        : undefined;
      return expression !== undefined &&
        expressionReferencesVariable(expression, helper.variable);
    });
    if (consumers.length !== 1) continue;
    const [consumer] = consumers;
    if (consumer?.statement !== "declaration") continue;
    const root = declarations.find((declaration) =>
      declaration.variable === consumer.variable
    );
    if (root === undefined) continue;
    if (
      !isProfileOffsetRoot(root) ||
      !hasExactProfileOffsetInput(root.arguments, helper, helperKind)
    ) continue;
    const helpers = helpersByRoot.get(root.variable) ?? [];
    helpers.push(helper.symbol);
    helpersByRoot.set(root.variable, helpers);
  }
  return declarations.flatMap((root) => {
    const helpers = helpersByRoot.get(root.variable);
    return helpers === undefined
      ? []
      : [{
        kind: "profile_offset" as const,
        root: root.symbol,
        helpers,
      }];
  });
}

function closureForRoot(
  closures: readonly ManagedSourceDeclarationClosure[],
  root: string,
): ManagedSourceDeclarationClosure | undefined {
  return closures.find((closure) => closure.root === root);
}

function closureRootForHelper(
  closures: readonly ManagedSourceDeclarationClosure[],
  helper: string,
): string | undefined {
  return closures.find((closure) => closure.helpers.includes(helper))?.root;
}

function closureSymbols(
  closure: ManagedSourceDeclarationClosure,
): Set<string> {
  return new Set([...closure.helpers, closure.root]);
}

function refuseOwnedHelperMutation(
  current: CompiledManagedSource,
  target: ManagedMutationTarget,
  operation: "suppressed" | "deleted",
  rootOperation: "suppress" | "delete",
): void {
  if (target.target !== "declaration") return;
  const root = closureRootForHelper(
    projectSourceDeclarationClosures(current.ir),
    target.declaration,
  );
  if (root !== undefined) {
    return ownedHelperMutationFail(
      target.declaration,
      root,
      operation,
      rootOperation,
    );
  }
}

function ownedHelperMutationFail(
  helper: string,
  root: string,
  operation: string,
  rootOperation: string,
): never {
  return mutationFail(
    "owned_helper_mutation",
    `Profile Offset helper ${helper} cannot be ${operation} independently; ${rootOperation} its Profile Offset root ${root}`,
  );
}

function profileOffsetHelperKind(
  declaration: SourceClosureDeclaration,
): ProfileOffsetHelperKind | undefined {
  if (declaration.patch !== null) return undefined;
  const named = PROFILE_OFFSET_HELPER_KINDS_.find((kind) =>
    sameStringPath(
      declaration.builderPath,
      [
        "aggregate",
        kind.inputKind === "chain" ? "openChain" : "closedProfile",
      ],
    )
  );
  return named;
}

function isProfileOffsetRoot(
  declaration: SourceClosureDeclaration,
): boolean {
  return declaration.patch === null &&
    sameStringPath(declaration.builderPath, ["operation", "profileOffset"]);
}

function hasExactProfileOffsetInput(
  arguments_: ManagedExpression,
  helper: SourceClosureDeclaration,
  helperKind: ProfileOffsetHelperKind,
): boolean {
  const sources = objectExpressionField(arguments_, "sources");
  if (sources?.kind === "array" && sources.values.length === 1) {
    const [source] = sources.values;
    if (
      source?.kind === "reference" &&
      source.declaration === helper.variable &&
      sameSemanticPath(source.path, [helperKind.inputKind])
    ) return true;
  }
  return false;
}

function objectExpressionField(
  expression: ManagedExpression,
  name: string,
): ManagedExpression | undefined {
  if (expression.kind !== "object") return undefined;
  return expression.fields.find((field) => field.name === name)?.value;
}

function expressionReferencesVariable(
  expression: ManagedExpression,
  variable: string,
): boolean {
  switch (expression.kind) {
    case "reference":
      return expression.declaration === variable;
    case "array":
      return expression.values.some((value) =>
        expressionReferencesVariable(value, variable)
      );
    case "object":
      return expression.fields.some((field) =>
        expressionReferencesVariable(field.value, variable)
      );
    case "call":
      return expression.arguments.some((argument) =>
        expressionReferencesVariable(argument, variable)
      );
    default:
      return false;
  }
}

function sameReference(
  left: Pick<ManagedReference, "declaration" | "path">,
  right: Pick<ManagedReference, "declaration" | "path">,
): boolean {
  return left.declaration === right.declaration &&
    sameSemanticPath(left.path, right.path);
}

function sameSemanticPath(
  left: readonly SemanticPathSegment[],
  right: readonly SemanticPathSegment[],
): boolean {
  return left.length === right.length && left.every((segment, index) => {
    const candidate = right[index];
    if (typeof segment === "object" || typeof candidate === "object") {
      return typeof segment === "object" && typeof candidate === "object" &&
        segment.member === candidate.member;
    }
    return segment === candidate;
  });
}

function sameGeneratedAddress(
  left: ExecutedGeneratedMemberAddress,
  right: ExecutedGeneratedMemberAddress,
): boolean {
  return left.invocation === right.invocation &&
    sameStringPath(left.template, right.template) &&
    sameStringPath(left.member_key, right.member_key) &&
    sameStringPath(left.output, right.output);
}

function validateGeneratedAddressShape(address: unknown): void {
  if (!isRecord(address)) {
    return mutationFail(
      "invalid_draft",
      "managed generated address must be an object",
    );
  }
  requireMutationText(address.invocation, "generated invocation");
  for (
    const [label, path] of [
      ["template", address.template],
      ["member key", address.member_key],
      ["output", address.output],
    ] as const
  ) {
    if (!Array.isArray(path)) {
      return mutationFail(
        "invalid_draft",
        `managed generated ${label} path must be an array`,
      );
    }
    if (path.length > MANAGED_MUTATION_PATH_LIMIT) {
      return mutationFail(
        "resource_limit",
        `managed generated ${label} path exceeds its bound`,
      );
    }
    path.forEach((value) => requireMutationText(value, `generated ${label}`));
  }
}

function validateMutationPath(path: readonly SemanticPathSegment[]): void {
  if (!Array.isArray(path)) {
    return mutationFail(
      "invalid_draft",
      "managed sketch semantic path must be an array",
    );
  }
  if (path.length > MANAGED_MUTATION_PATH_LIMIT) {
    return mutationFail(
      "resource_limit",
      "managed sketch semantic path exceeds its bound",
    );
  }
  for (const segment of path) {
    if (typeof segment === "number") {
      if (!Number.isSafeInteger(segment) || segment < 0) {
        return mutationFail(
          "invalid_draft",
          "managed sketch semantic path has an invalid index",
        );
      }
    } else if (isRecord(segment) && "member" in segment) {
      requireMutationText(segment.member, "managed member path");
    } else if (typeof segment === "string") {
      requireMutationText(segment, "managed field path");
    } else {
      return mutationFail(
        "invalid_draft",
        "managed sketch semantic path segment is invalid",
      );
    }
  }
}

function validateMutationComments(comments: readonly string[]): void {
  if (!Array.isArray(comments)) {
    return mutationFail(
      "invalid_draft",
      "managed sketch declaration comments must be an array",
    );
  }
  if (comments.length > MANAGED_MUTATION_COMMENT_LIMIT) {
    return mutationFail(
      "resource_limit",
      "managed sketch declaration has too many comments",
    );
  }
  for (const comment of comments) {
    requireMutationText(comment, "managed comment");
    if (comment.includes("\n") || comment.includes("\r")) {
      return mutationFail(
        "invalid_draft",
        "managed sketch comments must occupy one line",
      );
    }
  }
}

function requireMutationIdentifier(value: string, label: string): void {
  requireMutationText(value, label);
  if (!validIdentifier(value)) {
    return mutationFail(
      "invalid_draft",
      `${label} ${JSON.stringify(value)} is not an identifier`,
    );
  }
}

function requireMutationText(
  value: unknown,
  label: string,
): asserts value is string {
  if (typeof value !== "string") {
    return mutationFail("invalid_draft", `${label} must be a string`);
  }
  const bytes = new TextEncoder().encode(value).byteLength;
  if (bytes === 0) return mutationFail("invalid_draft", `${label} is empty`);
  if (bytes > MANAGED_MUTATION_STRING_LIMIT) {
    return mutationFail(
      "resource_limit",
      `${label} exceeds ${MANAGED_MUTATION_STRING_LIMIT} bytes`,
    );
  }
}

function isRecord(
  value: unknown,
): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function mutationPlaceholderSite(kind: ManagedSourceSiteKind): string {
  return `mutation:${kind}`;
}

function mutationFail(
  code: ManagedMutationErrorCode,
  message: string,
  candidate?: {
    readonly source: string;
    readonly span: ManagedSourceSpan;
  },
): never {
  throw new ManagedMutationError(code, message, candidate);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Parse source into the closed reversible IR without executing semantics. */
export function parseManagedSource(source: string): ManagedSketchIr {
  const bytes = new TextEncoder().encode(source);
  if (bytes.byteLength > MANAGED_SKETCH_SOURCE_LIMIT) {
    throw new ManagedCompileError(
      `managed sketch source is ${bytes.byteLength} bytes; the limit is ${MANAGED_SKETCH_SOURCE_LIMIT}`,
      { start: 0, end: bytes.byteLength },
    );
  }
  const sourceFile = ts.createSourceFile(
    "sketch.ts",
    source,
    ts.ScriptTarget.ES2022,
    true,
    ts.ScriptKind.TS,
  );
  const syntaxError = (sourceFile as ts.SourceFile & {
    readonly parseDiagnostics: readonly ts.DiagnosticWithLocation[];
  }).parseDiagnostics[0];
  if (syntaxError !== undefined) {
    const start = syntaxError.start ?? 0;
    const end = start + (syntaxError.length ?? 0);
    throw compileError(
      source,
      start,
      end,
      ts.flattenDiagnosticMessageText(syntaxError.messageText, "\n"),
    );
  }
  const directive = sourceFile.statements[0];
  if (
    directive === undefined || !ts.isExpressionStatement(directive) ||
    !ts.isStringLiteral(directive.expression) ||
    directive.expression.text !== "use geosolve sketch"
  ) {
    const start = directive?.getStart(sourceFile) ?? 0;
    const end = directive?.getEnd() ?? 0;
    throw compileError(
      source,
      start,
      end,
      'first statement must be exactly "use geosolve sketch"',
    );
  }
  const sourceDigest = sha256(source);
  const context: ParseContext = {
    source,
    sourceFile,
    sourceDigest,
    utf8Offsets: utf8OffsetTable(source),
    sites: [],
    siteIds: new Set(),
    variables: new Set(),
    bindingInitializers: new Map(),
    declarationVariables: new Set(),
    importedBindings: new Set(),
    symbols: new Set(),
    groups: new Set(),
    statementOrdinal: 0,
  };
  const imports: ManagedImport[] = [];
  let callbackBody: ts.Block | undefined;
  for (const [index, statement] of sourceFile.statements.entries()) {
    if (
      index === 0 && ts.isExpressionStatement(statement) &&
      ts.isStringLiteral(statement.expression) &&
      statement.expression.text === "use geosolve sketch"
    ) {
      continue;
    }
    if (ts.isImportDeclaration(statement)) {
      const imported = parseImport(statement, context);
      for (const binding of imported.bindings) {
        if (context.importedBindings.has(binding)) {
          fail(context, statement, `duplicate imported binding ${binding}`);
        }
        context.importedBindings.add(binding);
      }
      imports.push(imported);
      continue;
    }
    if (ts.isExportAssignment(statement) && !statement.isExportEquals) {
      if (callbackBody !== undefined) {
        fail(
          context,
          statement,
          "managed sketch source has multiple default sketches",
        );
      }
      callbackBody = parseSketchEnvelope(statement.expression, context);
      continue;
    }
    fail(
      context,
      statement,
      "managed sketch permits only its directive, static imports, and one default sketch",
    );
  }
  const first = sourceFile.statements[0];
  if (
    first === undefined || !ts.isExpressionStatement(first) ||
    !ts.isStringLiteral(first.expression) ||
    first.expression.text !== "use geosolve sketch"
  ) {
    throw new ManagedCompileError("missing exact managed sketch directive", {
      start: 0,
      end: 0,
    });
  }
  if (callbackBody === undefined) {
    throw new ManagedCompileError(
      "missing default sketch(($) => { ... }) envelope",
      {
        start: bytes.byteLength,
        end: bytes.byteLength,
      },
    );
  }
  if (!context.importedBindings.has("sketch")) {
    throw new ManagedCompileError(
      "managed sketch must statically import sketch",
      { start: 0, end: 0 },
    );
  }
  const terminal = callbackBody.statements.at(-1);
  if (
    terminal === undefined || !ts.isReturnStatement(terminal) ||
    terminal.expression === undefined
  ) {
    throw new ManagedCompileError(
      "managed sketch sketch callback must end with an ordinary return value",
      {
        start: context.utf8Offsets[callbackBody.end - 1]!,
        end: context.utf8Offsets[callbackBody.end]!,
      },
    );
  }
  const statements = callbackBody.statements.slice(0, -1).map((statement) =>
    parseStatement(statement, context)
  );
  const output = parseExpression(
    terminal.expression,
    context,
    "sketch:output",
  );
  const provisional = {
    format: MANAGED_SKETCH_IR_FORMAT,
    imports,
    statements,
    output,
    source_sites: context.sites,
    source_digest: sourceDigest,
  };
  const irDigest = sha256(canonicalJson(provisional));
  return deepFreeze({ ...provisional, ir_digest: irDigest });
}

/** Print one canonical, stable managed sketch source file. */
export function printManagedSource(ir: ManagedSketchIr): string {
  if (ir.format !== MANAGED_SKETCH_IR_FORMAT) {
    throw new TypeError("unsupported managed sketch IR format");
  }
  const lines = ['"use geosolve sketch";'];
  for (const declaration of ir.imports) {
    lines.push(
      `import { ${declaration.bindings.join(", ")} } from ${
        quote(declaration.module)
      };`,
    );
  }
  if (ir.imports.length > 0) lines.push("");
  lines.push("export default sketch(($) => {");
  for (const statement of ir.statements) {
    appendComments(lines, statement.comments, "  ");
    switch (statement.statement) {
      case "binding":
        lines.push(
          `  const ${statement.variable} = ${
            printExpression(statement.value, 1)
          };`,
        );
        break;
      case "declaration":
        if (statement.patch !== null) {
          lines.push(
            `  const ${statement.variable} = $.use(${
              quote(statement.symbol)
            }, ${statement.patch}, ${
              printExpression(statement.arguments, 1)
            });`,
          );
          break;
        }
        lines.push(
          `  const ${statement.variable} = $.${
            statement.builder_path.join(".")
          }(${quote(statement.symbol)}, ${
            printExpression(statement.arguments, 1)
          });`,
        );
        break;
      case "group":
        lines.push(
          `  $.group(${quote(statement.name)}, [${
            statement.declarations.map(printReference).join(", ")
          }]);`,
        );
        break;
      case "suppression":
        lines.push(`  $.suppress(${printReference(statement.target)});`);
        break;
    }
  }
  lines.push(`  return ${printExpression(ir.output, 1)};`);
  lines.push("});", "");
  return lines.join("\n");
}

/** Execute the already-closed IR through the deterministic semantic recorder. */
export function executeManagedSketch(
  ir: ManagedSketchIr,
  options: ManagedCompileOptions = {},
): ExecutedSketchArtifact {
  if (ir.format !== MANAGED_SKETCH_IR_FORMAT) {
    throw new TypeError("unsupported managed sketch IR format");
  }
  const declarations: ExecutedDeclarationResult[] = [];
  const generatedMembers: ExecutedGeneratedMember[] = [];
  const groups: ExecutedGroup[] = [];
  const suppressions: ExecutedSuppression[] = [];
  const consumers: ExecutedValueConsumer[] = [];
  const environment = new Map<string, unknown>();
  const bindingOrigins = new Map<string, readonly string[]>();
  for (const statement of ir.statements) {
    try {
      switch (statement.statement) {
      case "binding":
        environment.set(
          statement.variable,
          evaluateExpression(statement.value, environment),
        );
        bindingOrigins.set(
          statement.variable,
          expressionOrigins(statement.value, bindingOrigins),
        );
        break;
      case "declaration": {
        const family = statement.builder_path.join(".");
        const argumentsValue = evaluateExpression(
          statement.arguments,
          environment,
        );
        const patch = statement.patch === null
          ? undefined
          : options.patches?.[statement.patch];
        if (statement.patch !== null && patch === undefined) {
          throw new TypeError(
            `managed sketch patch ${statement.patch} has no pinned artifact plan`,
          );
        }
        const result = patch === undefined
          ? createDeclarationResult(
            statement.symbol,
            family,
            statement.site,
            statement.patch,
            argumentsValue,
          )
          : createPatchInvocationResult(
            statement.symbol,
            statement.patch!,
            statement.site,
            patch,
            generatedMembers,
            consumers,
            statement.arguments,
            argumentsValue,
            bindingOrigins,
          );
        if (statement.patch === null) {
          visitConsumerSites(
            statement.arguments,
            { target: "declaration", declaration: statement.symbol, family },
            [],
            consumers,
            bindingOrigins,
          );
        }
        declarations.push(result.declaration);
        environment.set(statement.variable, result.runtime);
        // The value is evaluated to prove all references resolve at execution time.
        void argumentsValue;
        break;
      }
      case "group":
        statement.declarations.forEach((reference) =>
          resolveRuntimeReference(reference, environment)
        );
        groups.push({
          name: statement.name,
          site: statement.site,
          declarations: statement.declarations,
        });
        break;
      case "suppression":
        resolveRuntimeReference(statement.target, environment);
        suppressions.push({ site: statement.site, target: statement.target });
        break;
      }
    } catch (error) {
      if (error instanceof ManagedCompileError) throw error;
      const siteId = statement.statement === "binding"
        ? statement.value.site
        : statement.site;
      const span = ir.source_sites.find(({ id }) => id === siteId)?.span ?? {
        start: 0,
        end: 0,
      };
      throw new ManagedCompileError(errorMessage(error), span);
    }
  }
  const output = runtimeValueToManagedValue(
    evaluateExpression(ir.output, environment),
  );
  const provisional = {
    format: EXECUTED_SKETCH_ARTIFACT_FORMAT,
    source_digest: ir.source_digest,
    ir_digest: ir.ir_digest,
    declarations,
    generated_members: generatedMembers,
    groups,
    suppressions,
    value_consumers: consumers,
    output,
  };
  return deepFreeze({
    ...provisional,
    artifact_digest: sha256(canonicalJson(provisional)),
  });
}

/**
 * Execute the exact admitted callback through an equation-free recorder.
 *
 * Parsing supplies lexical names, spans and a closed syntax proof. The
 * callback itself supplies declaration order, result-reference flow and its
 * ordinary returned value. Every runtime event is cross-checked against the
 * lexical IR before the artifact can be emitted.
 */
export function executeManagedSource(
  source: string,
  ir: ManagedSketchIr,
  options: ManagedCompileOptions = {},
): ExecutedSketchArtifact {
  if (ir.format !== MANAGED_SKETCH_IR_FORMAT) {
    throw new TypeError("unsupported managed sketch IR format");
  }
  if (sha256(source) !== ir.source_digest) {
    throw new TypeError("managed source execution does not match its lexical IR digest");
  }

  const declarations: ExecutedDeclarationResult[] = [];
  const generatedMembers: ExecutedGeneratedMember[] = [];
  const groups: ExecutedGroup[] = [];
  const suppressions: ExecutedSuppression[] = [];
  const consumers: ExecutedValueConsumer[] = [];
  const environment = new Map<string, unknown>();
  const bindingOrigins = new Map<string, readonly string[]>();
  let statementCursor = 0;

  const consumeBindings = () => {
    while (ir.statements[statementCursor]?.statement === "binding") {
      const statement = ir.statements[statementCursor]!;
      if (statement.statement !== "binding") break;
      environment.set(
        statement.variable,
        evaluateExpression(statement.value, environment),
      );
      bindingOrigins.set(
        statement.variable,
        expressionOrigins(statement.value, bindingOrigins),
      );
      statementCursor += 1;
    }
  };

  const nextStatement = <Kind extends ManagedStatement["statement"]>(
    kind: Kind,
  ): Extract<ManagedStatement, { readonly statement: Kind }> => {
    consumeBindings();
    const statement = ir.statements[statementCursor];
    if (statement?.statement !== kind) {
      throw runtimeCompileError(
        ir,
        statement,
        `executed managed callback produced ${kind} out of lexical order`,
      );
    }
    statementCursor += 1;
    return statement as Extract<ManagedStatement, { readonly statement: Kind }>;
  };

  const invokeDeclaration = (
    builderPath: readonly string[],
    symbolValue: unknown,
    argumentsValue: unknown,
    patchBinding: string | null,
  ): unknown => {
    const statement = nextStatement("declaration");
    const family = builderPath.join(".");
    if (
      typeof symbolValue !== "string" || symbolValue !== statement.symbol ||
      family !== statement.builder_path.join(".") ||
      patchBinding !== statement.patch
    ) {
      throw runtimeCompileError(
        ir,
        statement,
        "executed managed declaration differs from its lexical declaration identity",
      );
    }
    const lexicalArguments = evaluateExpression(statement.arguments, environment);
    if (!sameRuntimeManagedValue(argumentsValue, lexicalArguments)) {
      throw runtimeCompileError(
        ir,
        statement,
        `executed managed declaration ${statement.symbol} arguments differ from lexical IR`,
      );
    }
    const patch = patchBinding === null ? undefined : options.patches?.[patchBinding];
    if (patchBinding !== null && patch === undefined) {
      throw runtimeCompileError(
        ir,
        statement,
        `managed sketch patch ${patchBinding} has no pinned artifact plan`,
      );
    }
    const result = patch === undefined
      ? createDeclarationResult(
        statement.symbol,
        family,
        statement.site,
        statement.patch,
        argumentsValue,
      )
      : createPatchInvocationResult(
        statement.symbol,
        patchBinding!,
        statement.site,
        patch,
        generatedMembers,
        consumers,
        statement.arguments,
        argumentsValue,
        bindingOrigins,
      );
    if (patch === undefined) {
      visitConsumerSites(
        statement.arguments,
        {
          target: "declaration",
          declaration: statement.symbol,
          family,
        },
        [],
        consumers,
        bindingOrigins,
      );
    }
    declarations.push(result.declaration);
    environment.set(statement.variable, result.runtime);
    return result.runtime;
  };

  const patchBindings = new Map<string, object>();
  const builder = recorderBuilder({
    declaration: (path, symbol, values) =>
      invokeDeclaration(path, symbol, values, null),
    use: (symbol, binding, values) => {
      const patch = [...patchBindings.entries()].find(([, value]) => value === binding)?.[0];
      if (patch === undefined) {
        throw new TypeError("$.use received an unpinned patch binding");
      }
      return invokeDeclaration(["use"], symbol, values, patch);
    },
    group: (name, values) => {
      const statement = nextStatement("group");
      if (name !== statement.name || !Array.isArray(values)) {
        throw runtimeCompileError(
          ir,
          statement,
          "executed managed group differs from lexical IR",
        );
      }
      const actual = values.map((value) => runtimeReferenceOf(value));
      const expected = statement.declarations.map((reference) =>
        resolveRuntimeReference(reference, environment)
      );
      if (
        actual.some((value) => value === undefined) ||
        canonicalJson(actual) !== canonicalJson(expected)
      ) {
        throw runtimeCompileError(
          ir,
          statement,
          "executed managed group references differ from lexical IR",
        );
      }
      groups.push({
        name: statement.name,
        site: statement.site,
        declarations: statement.declarations,
      });
    },
    suppress: (value) => {
      const statement = nextStatement("suppression");
      const actual = runtimeReferenceOf(value);
      const expected = resolveRuntimeReference(statement.target, environment);
      if (actual === undefined || canonicalJson(actual) !== canonicalJson(expected)) {
        throw runtimeCompileError(
          ir,
          statement,
          "executed managed suppression target differs from lexical IR",
        );
      }
      suppressions.push({ site: statement.site, target: statement.target });
    },
  });

  const sourceFile = ts.createSourceFile(
    "sketch.ts",
    source,
    ts.ScriptTarget.ES2022,
    true,
    ts.ScriptKind.TS,
  );
  const exported = sourceFile.statements.find((statement): statement is ts.ExportAssignment =>
    ts.isExportAssignment(statement) && !statement.isExportEquals
  );
  if (exported === undefined) {
    throw new TypeError("managed source execution lost its default sketch expression");
  }

  const importNames: string[] = [];
  const importValues: unknown[] = [];
  let callbackExecutions = 0;
  let callbackOutput: unknown;
  const sketchEntry = (callback: unknown): unknown => {
    if (typeof callback !== "function" || callbackExecutions !== 0) {
      throw new TypeError("managed sketch must execute exactly one synchronous callback");
    }
    callbackExecutions += 1;
    callbackOutput = (callback as (value: unknown) => unknown)(builder);
    return callbackOutput;
  };
  const units = new Set(["mm", "cm", "m", "inch", "deg", "rad"]);
  for (const declaration of ir.imports) {
    for (const binding of declaration.bindings) {
      importNames.push(binding);
      if (binding === "sketch") {
        if (declaration.module !== "@geosolve/sketch-code") {
          throw new TypeError("managed sketch must import sketch from @geosolve/sketch-code");
        }
        importValues.push(sketchEntry);
      } else if (units.has(binding)) {
        if (declaration.module !== "@geosolve/sketch-code") {
          throw new TypeError(`managed unit ${binding} must come from @geosolve/sketch-code`);
        }
        importValues.push((value: unknown) => {
          if (typeof value !== "number" || !Number.isFinite(value)) {
            throw new TypeError(`managed unit ${binding} requires one finite number`);
          }
          return Object.freeze({ unit: binding, value });
        });
      } else {
        if (options.patches?.[binding] === undefined) {
          throw new TypeError(`managed import ${binding} has no closed runtime binding`);
        }
        const sentinel = Object.freeze({ binding });
        patchBindings.set(binding, sentinel);
        importValues.push(sentinel);
      }
    }
  }

  try {
    const evaluateModule = Function(
      ...importNames,
      `"use strict"; return (${exported.expression.getText(sourceFile)});`,
    ) as (...values: readonly unknown[]) => unknown;
    const moduleValue = evaluateModule(...importValues);
    if (callbackExecutions !== 1 || moduleValue !== callbackOutput) {
      throw new TypeError("managed sketch default export did not return its callback value");
    }
    consumeBindings();
    if (statementCursor !== ir.statements.length) {
      throw runtimeCompileError(
        ir,
        ir.statements[statementCursor],
        "managed callback did not execute every lexical statement",
      );
    }
    const actualOutput = runtimeValueToManagedValue(callbackOutput);
    const lexicalOutput = runtimeValueToManagedValue(
      evaluateExpression(ir.output, environment),
    );
    if (canonicalJson(actualOutput) !== canonicalJson(lexicalOutput)) {
      throw new TypeError(
        `executed managed callback output differs from lexical IR: runtime=${
          canonicalJson(actualOutput)
        } lexical=${canonicalJson(lexicalOutput)}`,
      );
    }
    const provisional = {
      format: EXECUTED_SKETCH_ARTIFACT_FORMAT,
      source_digest: ir.source_digest,
      ir_digest: ir.ir_digest,
      declarations,
      generated_members: generatedMembers,
      groups,
      suppressions,
      value_consumers: consumers,
      output: actualOutput,
    };
    return deepFreeze({
      ...provisional,
      artifact_digest: sha256(canonicalJson(provisional)),
    });
  } catch (error) {
    if (error instanceof ManagedCompileError) throw error;
    throw new ManagedCompileError(errorMessage(error), { start: 0, end: 0 });
  }
}

interface RecorderBuilderCallbacks {
  readonly declaration: (
    path: readonly string[],
    symbol: unknown,
    values: unknown,
  ) => unknown;
  readonly use: (symbol: unknown, binding: unknown, values: unknown) => unknown;
  readonly group: (name: unknown, values: unknown) => void;
  readonly suppress: (value: unknown) => void;
}

function recorderBuilder(callbacks: RecorderBuilderCallbacks): unknown {
  const namespace = (path: readonly string[]): unknown =>
    new Proxy(() => undefined, {
      get: (_target, property) => {
        if (typeof property !== "string") return undefined;
        return namespace([...path, property]);
      },
      apply: (_target, _receiver, values: unknown[]) => {
        if (values.length !== 2) {
          throw new TypeError(`managed declaration ${path.join(".")} requires ID and values`);
        }
        return callbacks.declaration(path, values[0], values[1]);
      },
    });
  return new Proxy(Object.create(null) as Record<string, unknown>, {
    get: (_target, property) => {
      if (property === "use") {
        return (symbol: unknown, binding: unknown, values: unknown) =>
          callbacks.use(symbol, binding, values);
      }
      if (property === "group") return callbacks.group;
      if (property === "suppress") return callbacks.suppress;
      if (typeof property !== "string") return undefined;
      return namespace([property]);
    },
  });
}

function sameRuntimeManagedValue(left: unknown, right: unknown): boolean {
  return canonicalJson(runtimeValueToManagedValue(left)) ===
    canonicalJson(runtimeValueToManagedValue(right));
}

function runtimeCompileError(
  ir: ManagedSketchIr,
  statement: ManagedStatement | undefined,
  message: string,
): ManagedCompileError {
  const site = statement === undefined
    ? undefined
    : statement.statement === "binding"
    ? statement.value.site
    : statement.site;
  const span = ir.source_sites.find((candidate) => candidate.id === site)?.span ?? {
    start: 0,
    end: 0,
  };
  return new ManagedCompileError(message, span);
}

function parseImport(
  node: ts.ImportDeclaration,
  context: ParseContext,
): ManagedImport {
  if (
    !ts.isStringLiteral(node.moduleSpecifier) ||
    node.importClause?.isTypeOnly === true ||
    node.importClause?.name !== undefined ||
    node.importClause?.namedBindings === undefined ||
    !ts.isNamedImports(node.importClause.namedBindings)
  ) {
    fail(context, node, "managed sketch imports must use static named bindings");
  }
  const bindings = node.importClause.namedBindings.elements.map((element) => {
    if (element.isTypeOnly || element.propertyName !== undefined) {
      fail(
        context,
        element,
        "managed sketch imports cannot alias or import type-only bindings",
      );
    }
    return element.name.text;
  });
  if (bindings.length === 0 || new Set(bindings).size !== bindings.length) {
    fail(context, node, "managed sketch imports require unique named bindings");
  }
  return { module: node.moduleSpecifier.text, bindings };
}

function parseSketchEnvelope(
  node: ts.Expression,
  context: ParseContext,
): ts.Block {
  if (
    !ts.isCallExpression(node) || !ts.isIdentifier(node.expression) ||
    node.expression.text !== "sketch" || node.arguments.length !== 1
  ) {
    fail(context, node, "default export must be sketch(($) => { ... })");
  }
  const callback = node.arguments[0];
  if (
    callback === undefined || !ts.isArrowFunction(callback) ||
    callback.modifiers !== undefined ||
    callback.typeParameters !== undefined || callback.parameters.length !== 1 ||
    !ts.isBlock(callback.body)
  ) {
    fail(
      context,
      node,
      "managed sketch sketch callback must be the exact synchronous ($) => { ... } form",
    );
  }
  const parameter = callback.parameters[0];
  if (
    parameter === undefined || !ts.isIdentifier(parameter.name) ||
    parameter.name.text !== "$"
  ) {
    fail(
      context,
      node,
      "managed sketch sketch callback must use one `$` builder parameter",
    );
  }
  return callback.body;
}

function parseStatement(
  node: ts.Statement,
  context: ParseContext,
): ManagedStatement {
  const ordinal = context.statementOrdinal;
  context.statementOrdinal += 1;
  const comments = attachedComments(node, context);
  if (ts.isVariableStatement(node)) {
    if (
      (node.declarationList.flags & ts.NodeFlags.Const) === 0 ||
      node.declarationList.declarations.length !== 1
    ) {
      fail(
        context,
        node,
        "managed sketch bindings must be one-name const declarations",
      );
    }
    const declaration = node.declarationList.declarations[0];
    if (
      declaration === undefined || !ts.isIdentifier(declaration.name) ||
      declaration.type !== undefined ||
      declaration.exclamationToken !== undefined ||
      declaration.initializer === undefined
    ) {
      fail(
        context,
        node,
        "managed sketch bindings must have one inferred identifier and initializer",
      );
    }
    const variable = declaration.name.text;
    if (context.variables.has(variable)) {
      fail(context, declaration.name, `duplicate binding ${variable}`);
    }
    const call = declaration.initializer;
    const builderPath = managedBuilderPath(call);
    if (builderPath !== undefined) {
      if (!ts.isCallExpression(call) || call.arguments.length !== 2) {
        fail(
          context,
          call,
          "managed declaration calls require symbol string and argument object",
        );
      }
      const symbolNode = call.arguments[0];
      const argumentsNode = call.arguments[1];
      if (
        symbolNode === undefined || !ts.isStringLiteral(symbolNode) ||
        argumentsNode === undefined ||
        !ts.isObjectLiteralExpression(argumentsNode)
      ) {
        fail(
          context,
          call,
          "managed declaration arguments must be an object literal",
        );
      }
      const symbol = symbolNode.text;
      if (context.symbols.has(symbol)) {
        fail(context, symbolNode, `duplicate semantic symbol ${symbol}`);
      }
      context.symbols.add(symbol);
      const siteBase = `declaration:${variable}:${symbol}`;
      const site = addSite(context, "declaration", call, `${siteBase}:call`);
      const args = parseExpression(
        argumentsNode,
        context,
        `${siteBase}:arguments`,
      );
      const family = builderPath.join(".");
      const availability = (
        AUTHORING_METHOD_CATALOG as Readonly<Record<string, string>>
      )[family];
      if (availability === "requires_host_snapshot") {
        fail(
          context,
          call.expression,
          `managed sketch declaration family ${family} requires immutable host-snapshot authority`,
        );
      }
      if (availability !== "public") {
        fail(
          context,
          call.expression,
          `unsupported managed sketch declaration family ${family}`,
        );
      }
      rejectRetiredManagedTransportProperties(argumentsNode, context);
      context.variables.add(variable);
      context.declarationVariables.add(variable);
      return {
        statement: "declaration",
        variable,
        symbol,
        builder_path: builderPath,
        patch: null,
        arguments: args,
        site,
        comments,
      };
    }
    if (
      ts.isCallExpression(call) && isBuilderMethod(call.expression, ["use"])
    ) {
      if (call.arguments.length !== 3) {
        fail(
          context,
          call,
          "$.use requires symbol, imported patch binding, and argument object",
        );
      }
      const symbolNode = call.arguments[0];
      const patchNode = call.arguments[1];
      const argumentsNode = call.arguments[2];
      if (
        symbolNode === undefined || !ts.isStringLiteral(symbolNode) ||
        patchNode === undefined || !ts.isIdentifier(patchNode) ||
        argumentsNode === undefined ||
        !ts.isObjectLiteralExpression(argumentsNode)
      ) {
        fail(
          context,
          call,
          "$.use requires symbol, imported patch binding, and argument object",
        );
      }
      const symbol = symbolNode.text;
      if (context.symbols.has(symbol)) {
        fail(context, symbolNode, `duplicate semantic symbol ${symbol}`);
      }
      context.symbols.add(symbol);
      const siteBase = `declaration:${variable}:${symbol}`;
      if (!context.importedBindings.has(patchNode.text)) {
        fail(
          context,
          patchNode,
          `patch binding ${patchNode.text} is not statically imported`,
        );
      }
      const invocationArguments = parseExpression(
        argumentsNode,
        context,
        `${siteBase}:arguments`,
      );
      context.variables.add(variable);
      context.declarationVariables.add(variable);
      return {
        statement: "declaration",
        variable,
        symbol,
        builder_path: ["use"],
        patch: patchNode.text,
        arguments: invocationArguments,
        site: addSite(context, "declaration", call, `${siteBase}:call`),
        comments,
      };
    }
    const value = parseExpression(
      declaration.initializer,
      context,
      `binding:${variable}:value`,
    );
    context.variables.add(variable);
    context.bindingInitializers.set(variable, declaration.initializer);
    return {
      statement: "binding",
      variable,
      value,
      comments,
    };
  }
  if (ts.isExpressionStatement(node) && ts.isCallExpression(node.expression)) {
    const call = node.expression;
    if (isBuilderMethod(call.expression, ["group"])) {
      const name = call.arguments[0];
      const declarations = call.arguments[1];
      if (
        call.arguments.length !== 2 || name === undefined ||
        !ts.isStringLiteral(name) ||
        declarations === undefined || !ts.isArrayLiteralExpression(declarations)
      ) {
        fail(context, call, "$.group requires a name and a reference array");
      }
      if (context.groups.has(name.text)) {
        fail(context, name, `duplicate managed group ${name.text}`);
      }
      context.groups.add(name.text);
      const siteBase = `group:${name.text}`;
      return {
        statement: "group",
        name: name.text,
        declarations: declarations.elements.map((element, index) =>
          parseReference(
            element,
            context,
            "group_reference",
            `${siteBase}:reference:${index}`,
          )
        ),
        site: addSite(context, "group", call, `${siteBase}:call`),
        comments,
      };
    }
    if (isBuilderMethod(call.expression, ["suppress"])) {
      if (call.arguments.length !== 1) {
        fail(context, call, "$.suppress requires one semantic reference");
      }
      const siteBase = `suppression:${ordinal}`;
      return {
        statement: "suppression",
        target: parseReference(
          call.arguments[0]!,
          context,
          "suppression_reference",
          `${siteBase}:reference`,
        ),
        site: addSite(context, "suppression", call, `${siteBase}:call`),
        comments,
      };
    }
  }
  fail(
    context,
    node,
    "unsupported managed sketch statement; loops, helpers, mutation, returns, and arbitrary expressions are forbidden",
  );
}

function rejectRetiredManagedTransportProperties(
  node: ts.Expression,
  context: ParseContext,
  visitedBindings = new Set<string>(),
): void {
  if (ts.isObjectLiteralExpression(node)) {
    for (const property of node.properties) {
      const name = ts.isShorthandPropertyAssignment(property)
        ? property.name.text
        : ts.isPropertyAssignment(property) &&
            (ts.isIdentifier(property.name) || ts.isStringLiteral(property.name))
        ? property.name.text
        : undefined;
      if (name !== undefined && RETIRED_MANAGED_TRANSPORT_PROPERTIES.has(name)) {
        fail(
          context,
          property,
          `managed named declaration arguments cannot contain retired transport property \`${name}\``,
        );
      }
      if (ts.isShorthandPropertyAssignment(property)) {
        rejectRetiredManagedTransportProperties(
          property.name,
          context,
          visitedBindings,
        );
      } else if (ts.isPropertyAssignment(property)) {
        rejectRetiredManagedTransportProperties(
          property.initializer,
          context,
          visitedBindings,
        );
      }
    }
    return;
  }
  if (ts.isArrayLiteralExpression(node)) {
    for (const element of node.elements) {
      if (!ts.isSpreadElement(element) && !ts.isOmittedExpression(element)) {
        rejectRetiredManagedTransportProperties(element, context, visitedBindings);
      }
    }
    return;
  }
  if (ts.isCallExpression(node)) {
    for (const argument of node.arguments) {
      rejectRetiredManagedTransportProperties(argument, context, visitedBindings);
    }
    return;
  }
  if (ts.isPrefixUnaryExpression(node)) {
    rejectRetiredManagedTransportProperties(node.operand, context, visitedBindings);
    return;
  }
  const reference = referencePath(node);
  const binding = reference === undefined
    ? undefined
    : context.bindingInitializers.get(reference.declaration);
  if (binding !== undefined && visitedBindings.add(reference!.declaration)) {
    rejectRetiredManagedTransportProperties(binding, context, visitedBindings);
  }
}

function parseExpression(
  node: ts.Expression,
  context: ParseContext,
  siteKey: string,
): ManagedExpression {
  if (node.kind === ts.SyntaxKind.NullKeyword) {
    return { kind: "null", site: addSite(context, "value", node, siteKey) };
  }
  if (
    node.kind === ts.SyntaxKind.TrueKeyword ||
    node.kind === ts.SyntaxKind.FalseKeyword
  ) {
    return {
      kind: "boolean",
      value: node.kind === ts.SyntaxKind.TrueKeyword,
      site: addSite(context, "value", node, siteKey),
    };
  }
  if (ts.isNumericLiteral(node)) {
    const value = Number(node.text);
    if (!Number.isFinite(value)) {
      fail(context, node, "managed sketch numeric literals must be finite");
    }
    return {
      kind: "number",
      value,
      site: addSite(context, "value", node, siteKey),
    };
  }
  if (
    ts.isPrefixUnaryExpression(node) &&
    (node.operator === ts.SyntaxKind.MinusToken ||
      node.operator === ts.SyntaxKind.PlusToken) &&
    ts.isNumericLiteral(node.operand)
  ) {
    const value = Number(node.getText(context.sourceFile));
    if (!Number.isFinite(value)) {
      fail(context, node, "managed sketch numeric literals must be finite");
    }
    return {
      kind: "number",
      value,
      site: addSite(context, "value", node, siteKey),
    };
  }
  if (ts.isStringLiteral(node)) {
    return {
      kind: "string",
      value: node.text,
      site: addSite(context, "value", node, siteKey),
    };
  }
  if (ts.isArrayLiteralExpression(node)) {
    if (node.elements.some(ts.isSpreadElement)) {
      fail(context, node, "managed sketch arrays cannot contain spreads");
    }
    return {
      kind: "array",
      values: node.elements.map((element, index) =>
        parseExpression(element, context, `${siteKey}:index:${index}`)
      ),
      site: addSite(context, "value", node, siteKey),
    };
  }
  if (ts.isObjectLiteralExpression(node)) {
    const fields: ManagedObjectField[] = [];
    const names = new Set<string>();
    for (const property of node.properties) {
      if (ts.isShorthandPropertyAssignment(property)) {
        const name = property.name.text;
        if (property.objectAssignmentInitializer !== undefined) {
          fail(
            context,
            property,
            "managed sketch object shorthand cannot contain an assignment initializer",
          );
        }
        if (names.has(name)) {
          fail(context, property.name, `duplicate object field ${name}`);
        }
        names.add(name);
        fields.push({
          name,
          value: parseExpression(
            property.name,
            context,
            `${siteKey}:field:${name}`,
          ),
          comments: attachedComments(property, context),
        });
        continue;
      }
      if (
        !ts.isPropertyAssignment(property) ||
        (!ts.isIdentifier(property.name) && !ts.isStringLiteral(property.name))
      ) {
        fail(
          context,
          property,
          "managed sketch objects require explicit identifier or string properties",
        );
      }
      const name = property.name.text;
      if (names.has(name)) {
        fail(context, property.name, `duplicate object field ${name}`);
      }
      names.add(name);
      fields.push({
        name,
        value: parseExpression(
          property.initializer,
          context,
          `${siteKey}:field:${name}`,
        ),
        comments: attachedComments(property, context),
      });
    }
    return {
      kind: "object",
      fields,
      site: addSite(context, "value", node, siteKey),
    };
  }
  const reference = referencePath(node);
  if (reference !== undefined) {
    if (!context.variables.has(reference.declaration)) {
      fail(
        context,
        node,
        `reference to unknown or forward binding ${reference.declaration}`,
      );
    }
    return {
      kind: "reference",
      ...reference,
      site: addSite(context, "value", node, siteKey),
    };
  }
  if (
    ts.isCallExpression(node) && ts.isIdentifier(node.expression) &&
    ["mm", "cm", "m", "inch", "deg", "rad"].includes(node.expression.text) &&
    context.importedBindings.has(node.expression.text) &&
    node.typeArguments === undefined && node.arguments.length === 1
  ) {
    return {
      kind: "call",
      callee: node.expression.text,
      arguments: node.arguments.map((argument, index) =>
        parseExpression(argument, context, `${siteKey}:argument:${index}`)
      ),
      site: addSite(context, "value", node, siteKey),
    };
  }
  fail(context, node, "unsupported managed sketch value expression");
}

function parseReference(
  node: ts.Expression,
  context: ParseContext,
  kind: "group_reference" | "suppression_reference",
  siteKey: string,
): ManagedReference {
  const reference = referencePath(node);
  if (
    reference === undefined ||
    !context.declarationVariables.has(reference.declaration)
  ) {
    fail(
      context,
      node,
      "managed sketch organization and suppression targets must be prior semantic references",
    );
  }
  return { ...reference, site: addSite(context, kind, node, siteKey) };
}

function managedBuilderPath(node: ts.Expression): string[] | undefined {
  if (!ts.isCallExpression(node)) return undefined;
  const path = propertyPath(node.expression);
  return path !== undefined && path.length >= 3 && path[0] === "$" &&
      !["group", "suppress", "outputs", "organize"].includes(path[1] ?? "")
    ? path.slice(1)
    : undefined;
}

function isBuilderMethod(
  node: ts.Expression,
  tail: readonly string[],
): boolean {
  const path = propertyPath(node);
  return path !== undefined && path.length === tail.length + 1 &&
    path[0] === "$" &&
    tail.every((segment, index) => path[index + 1] === segment);
}

function propertyPath(node: ts.Expression): string[] | undefined {
  if (ts.isIdentifier(node)) return [node.text];
  if (!ts.isPropertyAccessExpression(node)) return undefined;
  const parent = propertyPath(node.expression);
  return parent === undefined ? undefined : [...parent, node.name.text];
}

function referencePath(node: ts.Expression): RuntimeReference | undefined {
  const path: SemanticPathSegment[] = [];
  let current = node;
  while (
    ts.isPropertyAccessExpression(current) ||
    ts.isElementAccessExpression(current)
  ) {
    if (ts.isPropertyAccessExpression(current)) {
      path.unshift(current.name.text);
      current = current.expression;
      continue;
    }
    const argument = current.argumentExpression;
    if (argument === undefined) return undefined;
    if (ts.isStringLiteral(argument)) path.unshift({ member: argument.text });
    else if (ts.isNumericLiteral(argument)) path.unshift(Number(argument.text));
    else return undefined;
    current = current.expression;
  }
  return ts.isIdentifier(current)
    ? { declaration: current.text, path }
    : undefined;
}

function addSite(
  context: ParseContext,
  kind: ManagedSourceSiteKind,
  node: ts.Node,
  stableKey: string,
): string {
  const start16 = node.getStart(context.sourceFile);
  const end16 = node.end;
  const span = {
    start: context.utf8Offsets[start16]!,
    end: context.utf8Offsets[end16]!,
  };
  const basis = `${kind}\0${stableKey}`;
  const id = sha256(basis);
  if (context.siteIds.has(id)) {
    fail(context, node, "duplicate managed source-site identity");
  }
  context.siteIds.add(id);
  context.sites.push({ id, kind, span, source_digest: context.sourceDigest });
  return id;
}

function attachedComments(node: ts.Node, context: ParseContext): string[] {
  const ranges = ts.getLeadingCommentRanges(context.source, node.pos) ?? [];
  return ranges.map((range) => {
    const raw = context.source.slice(range.pos, range.end);
    if (raw.startsWith("//")) return raw.slice(2).trim();
    return raw.slice(2, -2).trim();
  });
}

function evaluateExpression(
  node: ManagedExpression,
  environment: ReadonlyMap<string, unknown>,
): unknown {
  switch (node.kind) {
    case "null":
      return null;
    case "boolean":
    case "number":
    case "string":
      return node.value;
    case "array":
      return node.values.map((value) => evaluateExpression(value, environment));
    case "object":
      return Object.fromEntries(
        node.fields.map((
          field,
        ) => [field.name, evaluateExpression(field.value, environment)]),
      );
    case "reference":
      return runtimeReferenceValue(node, environment);
    case "call":
      if (
        ["mm", "cm", "m", "inch", "deg", "rad"].includes(node.callee) &&
        node.arguments.length === 1
      ) {
        const value = evaluateExpression(node.arguments[0]!, environment);
        if (typeof value !== "number" || !Number.isFinite(value)) {
          throw new TypeError(`managed unit ${node.callee} requires one finite number`);
        }
        return Object.freeze({ unit: node.callee, value });
      }
      throw new TypeError(`unsupported managed call ${node.callee}`);
  }
}

function resolveRuntimeReference(
  reference: RuntimeReference,
  environment: ReadonlyMap<string, unknown>,
): RuntimeReference {
  let current = environment.get(reference.declaration);
  if (current === undefined) {
    throw new TypeError(
      `executed managed sketch reference ${reference.declaration} is unavailable`,
    );
  }
  let runtime: RuntimeReference | undefined = runtimeReferenceOf(current);
  for (const segment of reference.path) {
    const key = typeof segment === "object" ? segment.member : segment;
    if (typeof current !== "object" || current === null || !(key in current)) {
      throw new TypeError(
        `executed managed sketch result ${reference.declaration}.${
          String(key)
        } is unavailable`,
      );
    }
    current = (current as Record<PropertyKey, unknown>)[key];
    runtime = runtimeReferenceOf(current) ?? runtime;
  }
  return runtime ?? reference;
}

function runtimeReferenceValue(
  reference: RuntimeReference,
  environment: ReadonlyMap<string, unknown>,
): unknown {
  let current = environment.get(reference.declaration);
  if (current === undefined) {
    throw new TypeError(
      `executed managed sketch reference ${reference.declaration} is unavailable`,
    );
  }
  for (const segment of reference.path) {
    const key = typeof segment === "object" ? segment.member : segment;
    if (typeof current !== "object" || current === null || !(key in current)) {
      throw new TypeError(
        `executed managed sketch result ${reference.declaration}.${String(key)} is unavailable`,
      );
    }
    current = (current as Record<PropertyKey, unknown>)[key];
  }
  return current;
}

function runtimeReferenceOf(value: unknown): RuntimeReference | undefined {
  return typeof value === "object" && value !== null &&
      runtimeReference in value
    ? (value as { readonly [runtimeReference]: RuntimeReference })[
      runtimeReference
    ]
    : undefined;
}

function runtimeValueToManagedValue(
  value: unknown,
  depth = 0,
  budget: { remaining: number } = { remaining: MANAGED_MUTATION_VALUE_NODE_LIMIT },
): ManagedValue {
  if (depth > MANAGED_MUTATION_VALUE_DEPTH_LIMIT) {
    throw new TypeError("managed callback output exceeds the depth bound");
  }
  budget.remaining -= 1;
  if (budget.remaining < 0) {
    throw new TypeError("managed callback output exceeds the node bound");
  }
  if (value === null) return { kind: "null" };
  if (typeof value === "boolean") return { kind: "bool", value };
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError("managed callback output contains a non-finite number");
    }
    return { kind: "number", value };
  }
  if (typeof value === "string") {
    if (new TextEncoder().encode(value).byteLength > MANAGED_MUTATION_STRING_LIMIT) {
      throw new TypeError("managed callback output string exceeds its bound");
    }
    return { kind: "string", value };
  }
  const reference = runtimeReferenceOf(value);
  if (reference !== undefined) {
    return {
      kind: "reference",
      value: {
        declaration: reference.declaration,
        path: reference.path,
      },
    };
  }
  if (Array.isArray(value)) {
    return {
      kind: "array",
      value: value.map((child) =>
        runtimeValueToManagedValue(child, depth + 1, budget)
      ),
    };
  }
  if (typeof value === "object" && value !== null) {
    const fields = value as Readonly<Record<string, unknown>>;
    const keys = Object.keys(fields);
    if (
      keys.length === 2 && keys.includes("unit") && keys.includes("value") &&
      typeof fields.unit === "string" &&
      ["mm", "cm", "m", "inch", "deg", "rad"].includes(fields.unit) &&
      typeof fields.value === "number" && Number.isFinite(fields.value)
    ) {
      return {
        kind: "unit",
        value: { unit: fields.unit, value: fields.value },
      };
    }
    return {
      kind: "object",
      // Rust authenticates this map through `BTreeMap`, so executed callback
      // insertion order must not affect canonical artifact bytes or digests.
      value: Object.fromEntries(keys.sort().map((key) => [
        key,
        runtimeValueToManagedValue(fields[key], depth + 1, budget),
      ])),
    };
  }
  throw new TypeError("managed callback output must be finite data or typed references");
}

function createDeclarationResult(
  symbol: string,
  family: string,
  site: string,
  patch: string | null,
  argumentsValue: unknown,
): {
  readonly declaration: ExecutedDeclarationResult;
  readonly runtime: unknown;
} {
  const descriptor = (DECLARATION_RESULT_CATALOG as Readonly<
    Record<string, {
      readonly feature_kind: FeatureKind;
      readonly outputs: unknown;
    }>
  >)[family];
  const leaves: ExecutedResultLeaf[] = [];
  const rootReference = {
    declaration: symbol,
    path: [] as SemanticPathSegment[],
  };
  if (descriptor === undefined) {
    throw new TypeError(
      `managed sketch declaration family ${family} has no result descriptor`,
    );
  }
  const runtime = materializeResultShape(
    descriptor.outputs,
    rootReference,
    leaves,
    declarationKeys(family, argumentsValue),
  );
  return {
    declaration: { declaration: symbol, family, patch, site, result: leaves },
    runtime,
  };
}

function validSemanticPathSegment(value: unknown): value is SemanticPathSegment {
  return typeof value === "string" && value.length > 0 ||
    Number.isSafeInteger(value) && (value as number) >= 0 ||
    typeof value === "object" && value !== null &&
      Object.keys(value).length === 1 &&
      typeof (value as { readonly member?: unknown }).member === "string" &&
      (value as { readonly member: string }).member.length > 0;
}

function isFeatureKind(value: unknown): value is FeatureKind {
  return typeof value === "string" && [
    "point",
    "curve",
    "curve_span",
    "scalar",
    "contact",
    "constraint",
    "dimension",
    "profile",
    "chain",
    "operation",
    "feature",
    "feature_corner",
    "collection",
  ].includes(value);
}

function createPatchInvocationResult(
  symbol: string,
  patchName: string,
  site: string,
  plan: PatchArtifactPlan,
  generatedMembers: ExecutedGeneratedMember[],
  consumers: ExecutedValueConsumer[],
  argumentsValue: ManagedExpression,
  runtimeArguments: unknown,
  bindingOrigins: ReadonlyMap<string, readonly string[]>,
): {
  readonly declaration: ExecutedDeclarationResult;
  readonly runtime: unknown;
} {
  if (argumentsValue.kind !== "object") {
    throw new TypeError(
      `executed patch invocation ${symbol} has non-object arguments`,
    );
  }
  const leaves = Object.entries(plan.outputs).map(([name, kind]) => ({
    kind: managedFeatureKind(kind),
    path: [name] as SemanticPathSegment[],
  }));
  const runtime = Object.create(null) as Record<PropertyKey, unknown>;
  Object.defineProperty(runtime, runtimeReference, {
    value: { declaration: symbol, path: [] },
  });
  for (const [name] of Object.entries(plan.outputs)) {
    const output = Object.create(null) as Record<PropertyKey, unknown>;
    Object.defineProperty(output, runtimeReference, {
      value: { declaration: symbol, path: [name] },
      configurable: true,
    });
    runtime[name] = output;
  }

  const dynamicTemplates = new Set<number>();
  plan.collections.forEach((collection) => {
    const keys = patchCollectionKeys(runtimeArguments, collection.input);
    const outputPath = collection.path;
    for (const key of keys) {
      const memberPath: SemanticPathSegment[] = [...outputPath, {
        member: key,
      }];
      setRuntimePath(
        runtime,
        memberPath,
        makeRuntimeLeaf({ declaration: symbol, path: memberPath }),
      );
      for (const templatePath of collection.templates) {
        const templateIndex = plan.templates.findIndex((template) =>
          sameStringPath(template.path, templatePath)
        );
        if (templateIndex < 0) {
          throw new TypeError(
            `patch ${patchName} references an unknown template`,
          );
        }
        dynamicTemplates.add(templateIndex);
        const template = plan.templates[templateIndex]!;
        recordGeneratedTemplate(
          symbol,
          template,
          [key],
          argumentsValue,
          generatedMembers,
          consumers,
          bindingOrigins,
        );
        publishTemplateRuntimeResult(
          runtime,
          symbol,
          template,
          [
            ...outputPath,
            { member: key },
            ...(template.result_path ?? []),
          ],
        );
      }
    }
  });
  plan.templates.forEach((template, index) => {
    if (dynamicTemplates.has(index)) return;
    recordGeneratedTemplate(
      symbol,
      template,
      [],
      argumentsValue,
      generatedMembers,
      consumers,
      bindingOrigins,
    );
    if (template.result_path !== null) {
      publishTemplateRuntimeResult(
        runtime,
        symbol,
        template,
        template.result_path,
      );
    }
  });
  return {
    declaration: {
      declaration: symbol,
      family: "use",
      patch: patchName,
      site,
      result: leaves,
    },
    runtime: deepFreeze(runtime),
  };
}

function recordGeneratedTemplate(
  invocation: string,
  template: ArtifactTemplateNode,
  memberKey: readonly string[],
  argumentsValue: ManagedExpression,
  generatedMembers: ExecutedGeneratedMember[],
  consumers: ExecutedValueConsumer[],
  bindingOrigins: ReadonlyMap<string, readonly string[]>,
): void {
  const exposedOutputs = template.result_output === null
    ? template.outputs
    : template.outputs.filter((output) =>
      sameSemanticPath(template.result_output!, output.path)
    );
  for (const output of exposedOutputs) {
    const target: ExecutedConsumerTarget = {
      target: "generated",
      address: {
        invocation,
        template: template.path,
        member_key: memberKey,
        output: generatedOutputAddress(output.path),
      },
      family: template.declaration_family,
    };
    generatedMembers.push({
      address: target.address,
      family: target.family,
      kind: managedFeatureKind(output.kind),
    });
    visitTemplateBindings(template.arguments, [], (property, binding) => {
      if (binding.source !== "input") return;
      // `binding.path` selects a semantic sub-result of one typed patch
      // input. The reversible lexical consumer is still the invocation
      // argument itself, not a guessed property access inside that reference.
      const value = expressionAt(argumentsValue, [binding.name]);
      if (value !== undefined) {
        visitConsumerSites(
          value,
          target,
          property,
          consumers,
          bindingOrigins,
        );
      }
    });
  }
}

function managedFeatureKind(value: unknown): FeatureKind {
  if (!isFeatureKind(value)) {
    throw new TypeError(`patch output has unsupported feature kind ${String(value)}`);
  }
  return value;
}

function visitTemplateBindings(
  argument: ArtifactTemplateArgument,
  path: readonly SemanticPathSegment[],
  visit: (
    path: readonly SemanticPathSegment[],
    binding: ArtifactTemplateBinding,
  ) => void,
): void {
  switch (argument.argument) {
    case "binding":
      visit(path, argument.value);
      return;
    case "array":
      argument.value.forEach((child, index) =>
        visitTemplateBindings(child, [...path, index], visit)
      );
      return;
    case "object":
      Object.entries(argument.value).forEach(([name, child]) =>
        visitTemplateBindings(child, [...path, name], visit)
      );
      return;
    case "literal":
      return;
  }
}

function generatedOutputAddress(
  path: readonly SemanticPathSegment[],
): string[] {
  return path.map((segment) =>
    typeof segment === "string"
      ? `field:${segment}`
      : typeof segment === "number"
      ? `index:${segment}`
      : `member:${segment.member}`
  );
}

function publishTemplateRuntimeResult(
  runtime: Record<PropertyKey, unknown>,
  declaration: string,
  template: ArtifactTemplateNode,
  resultPath: readonly SemanticPathSegment[],
): void {
  if (template.result_output !== null) {
    setRuntimePath(
      runtime,
      resultPath,
      makeRuntimeLeaf({ declaration, path: resultPath }),
    );
    return;
  }
  for (const output of template.outputs) {
    const path = [...resultPath, ...output.path];
    setRuntimePath(
      runtime,
      path,
      makeRuntimeLeaf({ declaration, path }),
    );
  }
  // A patch callback can return one declaration's complete result object,
  // rather than one selected output leaf. Preserve that declaration identity
  // on the populated container so a later managed callback return or input
  // observes one semantic reference instead of an incidental object assembled
  // from its child references.
  setRuntimePath(
    runtime,
    resultPath,
    makeRuntimeLeaf({ declaration, path: resultPath }),
  );
}

function patchCollectionKeys(
  runtimeArguments: unknown,
  input: string,
): string[] {
  if (typeof runtimeArguments !== "object" || runtimeArguments === null) {
    return [];
  }
  const value = (runtimeArguments as Record<string, unknown>)[input];
  if (typeof value !== "object" || value === null) return [];
  const keys = (value as { readonly keys?: unknown }).keys;
  if (Array.isArray(keys) && keys.every((key) => typeof key === "string")) {
    return [...keys];
  }
  return Object.keys(value).filter((key) => key !== "keys" && key !== "byKey");
}

function expressionAt(
  expression: ManagedExpression,
  path: readonly SemanticPathSegment[],
): ManagedExpression | undefined {
  let current: ManagedExpression | undefined = expression;
  for (const segment of path) {
    if (current === undefined) return undefined;
    if (current.kind === "object" && typeof segment === "string") {
      current = current.fields.find((field) => field.name === segment)?.value;
    } else if (current.kind === "array" && typeof segment === "number") {
      current = current.values[segment];
    } else {
      return undefined;
    }
  }
  return current;
}

function setRuntimePath(
  root: Record<PropertyKey, unknown>,
  path: readonly SemanticPathSegment[],
  value: unknown,
): void {
  let current = root;
  path.forEach((segment, index) => {
    const key = typeof segment === "object" ? segment.member : segment;
    if (index === path.length - 1) {
      const existing = current[key];
      const reference = runtimeReferenceOf(value);
      if (
        typeof existing === "object" && existing !== null &&
        !Object.isFrozen(existing) && reference !== undefined
      ) {
        const existingReference = runtimeReferenceOf(existing);
        if (
          existingReference !== undefined &&
          sameReference(existingReference, reference)
        ) {
          return;
        }
        Object.defineProperty(existing, runtimeReference, { value: reference });
        return;
      }
      current[key] = value;
      return;
    }
    const existing = current[key];
    if (
      typeof existing === "object" && existing !== null &&
      !Object.isFrozen(existing)
    ) {
      current = existing as Record<PropertyKey, unknown>;
      return;
    }
    const next = path[index + 1];
    const child = typeof next === "number"
      ? [] as unknown as Record<PropertyKey, unknown>
      : Object.create(null) as Record<PropertyKey, unknown>;
    const existingReference = runtimeReferenceOf(existing);
    if (existingReference !== undefined) {
      Object.defineProperty(child, runtimeReference, {
        value: existingReference,
      });
    }
    current[key] = child;
    current = child;
  });
}

function sameStringPath(
  left: readonly string[],
  right: readonly string[],
): boolean {
  return left.length === right.length &&
    left.every((segment, index) => segment === right[index]);
}

function materializeResultShape(
  shape: unknown,
  reference: RuntimeReference,
  leaves: ExecutedResultLeaf[],
  keys: DeclarationResultKeys,
): unknown {
  const value = shape as {
    readonly shape:
      | "leaf"
      | "native_span"
      | "object"
      | "tuple"
      | "keyed"
      | "native_span_keyed"
      | "dynamic_keyed";
    readonly kind?: FeatureKind;
    readonly fields?: Readonly<Record<string, unknown>>;
    readonly items?: readonly unknown[];
    readonly source?:
      | "polyline_vertices"
      | "polyline_segments"
      | "polyline_corners"
      | "spline_controls"
      | "spline_spans"
      | "fillet_corners";
    readonly member?: unknown;
  };
  switch (value.shape) {
    case "leaf":
      leaves.push({ kind: value.kind!, path: reference.path });
      return makeRuntimeLeaf(reference);
    case "native_span":
      leaves.push({ kind: "curve_span", path: reference.path });
      return makeRuntimeLeaf(reference);
    case "keyed":
    case "native_span_keyed": {
      leaves.push({ kind: "collection", path: reference.path });
      const collection = Object.create(null) as Record<PropertyKey, unknown>;
      Object.defineProperty(collection, runtimeReference, { value: reference });
      const byKey = Object.create(null) as Record<string, unknown>;
      const field = reference.path[reference.path.length - 1];
      const memberKeys = field === "segments"
        ? keys.segments
        : field === "filletableCorners"
        ? keys.corners
        : keys.vertices;
      for (const key of memberKeys) {
        const kind = value.shape === "native_span_keyed"
          ? "curve_span"
          : value.kind!;
        const memberReference = {
          declaration: reference.declaration,
          path: [...reference.path, { member: key }],
        };
        leaves.push({ kind, path: memberReference.path });
        byKey[key] = makeRuntimeLeaf(memberReference);
      }
      collection.keys = Object.freeze([...memberKeys]);
      collection.byKey = Object.freeze(byKey);
      return Object.freeze(collection);
    }
    case "object": {
      if (Object.keys(value.fields ?? {}).length === 0) {
        return lazyPlannedResult(reference, false);
      }
      const object = Object.create(null) as Record<PropertyKey, unknown>;
      Object.defineProperty(object, runtimeReference, { value: reference });
      for (const [name, child] of Object.entries(value.fields ?? {})) {
        object[name] = materializeResultShape(
          child,
          {
            declaration: reference.declaration,
            path: [...reference.path, name],
          },
          leaves,
          keys,
        );
      }
      return Object.freeze(object);
    }
    case "tuple": {
      if ((value.items ?? []).length === 0) {
        return lazyPlannedResult(reference, true);
      }
      return Object.freeze((value.items ?? []).map((child, index) =>
        materializeResultShape(
          child,
          {
            declaration: reference.declaration,
            path: [...reference.path, index],
          },
          leaves,
          keys,
        )
      ));
    }
    case "dynamic_keyed": {
      leaves.push({ kind: "collection", path: reference.path });
      const memberKeys = value.source === "polyline_vertices"
        ? keys.vertices
        : value.source === "polyline_segments"
        ? keys.segments
        : value.source === "polyline_corners"
        ? keys.corners
        : value.source === "spline_controls"
        ? keys.controls
        : value.source === "spline_spans"
        ? keys.spans
        : keys.fillets;
      const collection = Object.create(null) as Record<PropertyKey, unknown>;
      Object.defineProperty(collection, runtimeReference, { value: reference });
      const byKey = Object.create(null) as Record<string, unknown>;
      for (const key of memberKeys) {
        byKey[key] = materializeResultShape(
          value.member,
          {
            declaration: reference.declaration,
            path: [...reference.path, { member: key }],
          },
          leaves,
          keys,
        );
      }
      collection.keys = Object.freeze([...memberKeys]);
      collection.byKey = Object.freeze(byKey);
      return Object.freeze(collection);
    }
  }
}

/**
 * Topology-dependent operation members are invented only by Rust's native
 * preparation pass. Managed execution still needs to carry an explicitly
 * accessed path into the artifact so Rust can authenticate or reject it.
 * This proxy records that path without guessing an inventory or a kind.
 */
function lazyPlannedResult(
  reference: RuntimeReference,
  tuple: boolean,
): unknown {
  const target = (tuple ? [] : Object.create(null)) as Record<PropertyKey, unknown>;
  Object.defineProperty(target, runtimeReference, { value: reference });
  const child = (property: PropertyKey): unknown => {
    if (property === runtimeReference) return reference;
    if (typeof property !== "string") return Reflect.get(target, property);
    const segment = tuple && /^(?:0|[1-9][0-9]*)$/u.test(property)
      ? Number(property)
      : property;
    return lazyPlannedResult({
      declaration: reference.declaration,
      path: [...reference.path, segment],
    }, false);
  };
  return new Proxy(target, {
    get: (_value, property) => child(property),
    has: (_value, property) => property === runtimeReference ||
      typeof property === "string",
  });
}

function makeRuntimeLeaf(reference: RuntimeReference): RuntimeResult {
  const value = Object.create(null) as Record<PropertyKey, unknown>;
  Object.defineProperty(value, runtimeReference, { value: reference });
  return Object.freeze(value) as unknown as RuntimeResult;
}

function visitConsumerSites(
  value: ManagedExpression,
  target: ExecutedConsumerTarget,
  property: readonly SemanticPathSegment[],
  consumers: ExecutedValueConsumer[],
  bindingOrigins: ReadonlyMap<string, readonly string[]>,
): void {
  if (value.kind === "reference") {
    const origins = bindingOrigins.get(value.declaration) ?? [];
    // Scalar bindings retain their defining value as the shared fan-out
    // origin. A reference to an executed declaration result has no scalar
    // origin, so its own lexical site is the reversible consumer coordinate:
    // replacing that one reference with a local value detaches only this
    // consumer rather than rewriting its producer.
    for (const origin of origins.length === 0 ? [value.site] : origins) {
      consumers.push({ value_site: origin, target, property });
    }
    return;
  }
  if (value.kind === "call") {
    consumers.push({ value_site: value.site, target, property });
    return;
  }
  if (
    value.kind === "null" || value.kind === "boolean" ||
    value.kind === "number" ||
    value.kind === "string"
  ) {
    consumers.push({ value_site: value.site, target, property });
    return;
  }
  if (value.kind === "array") {
    value.values.forEach((child, index) =>
      visitConsumerSites(
        child,
        target,
        [...property, index],
        consumers,
        bindingOrigins,
      )
    );
  } else if (value.kind === "object") {
    value.fields.forEach((field) =>
      visitConsumerSites(
        field.value,
        target,
        [...property, field.name],
        consumers,
        bindingOrigins,
      )
    );
  }
}

function expressionOrigins(
  value: ManagedExpression,
  bindingOrigins: ReadonlyMap<string, readonly string[]>,
): readonly string[] {
  switch (value.kind) {
    case "null":
    case "boolean":
    case "number":
    case "string":
    case "call":
      return [value.site];
    case "reference":
      return bindingOrigins.get(value.declaration) ?? [];
    case "array":
      return value.values.flatMap((child) =>
        expressionOrigins(child, bindingOrigins)
      );
    case "object":
      return value.fields.flatMap((field) =>
        expressionOrigins(field.value, bindingOrigins)
      );
  }
}

interface DeclarationResultKeys {
  readonly vertices: readonly string[];
  readonly segments: readonly string[];
  readonly corners: readonly string[];
  readonly controls: readonly string[];
  readonly spans: readonly string[];
  readonly fillets: readonly string[];
}

function declarationKeys(family: string, argumentsValue: unknown): DeclarationResultKeys {
  const empty = {
    vertices: [], segments: [], corners: [], controls: [], spans: [], fillets: [],
  } as const;
  if (typeof argumentsValue !== "object" || argumentsValue === null) return empty;
  const keyedArgument = (name: string): readonly string[] | undefined => {
    const values = (argumentsValue as Readonly<Record<string, unknown>>)[name];
    if (!Array.isArray(values)) return undefined;
    const keys: string[] = [];
    for (const value of values) {
      if (
        typeof value !== "object" || value === null ||
        typeof (value as { readonly key?: unknown }).key !== "string"
      ) return undefined;
      keys.push((value as { readonly key: string }).key);
    }
    return keys;
  };
  if (family === "geometry.polyline") {
    const keys = keyedArgument("vertices");
    if (keys === undefined) return empty;
    const closed = (argumentsValue as { readonly closed?: unknown }).closed === true;
    return {
      ...empty,
      vertices: keys,
      segments: closed ? keys : keys.slice(0, -1),
      corners: closed ? keys : keys.slice(1, -1),
    };
  }
  if (
    family === "geometry.openControlBSpline" ||
    family === "geometry.periodicControlBSpline" ||
    family === "geometry.openControlNurbs" ||
    family === "geometry.periodicControlNurbs"
  ) {
    const controls = keyedArgument("controls");
    const degree = (argumentsValue as { readonly degree?: unknown }).degree;
    if (
      controls === undefined || typeof degree !== "number" ||
      !Number.isSafeInteger(degree) || degree < 1
    ) return empty;
    const spanCount = (
        family === "geometry.periodicControlBSpline" ||
        family === "geometry.periodicControlNurbs"
      )
      ? controls.length
      : Math.max(0, controls.length - degree);
    return { ...empty, controls, spans: controls.slice(0, spanCount) };
  }
  if (family === "computed.filletSet") {
    return { ...empty, fillets: keyedArgument("corners") ?? [] };
  }
  return empty;
}

function printExpression(value: ManagedExpression, depth: number): string {
  switch (value.kind) {
    case "null":
      return "null";
    case "boolean":
      return String(value.value);
    case "number":
      return Object.is(value.value, -0) ? "-0" : String(value.value);
    case "string":
      return quote(value.value);
    case "reference":
      return printReference(value);
    case "call":
      return `${value.callee}(${
        value.arguments.map((argument) => printExpression(argument, depth))
          .join(", ")
      })`;
    case "array":
      return `[${
        value.values.map((child) => printExpression(child, depth)).join(", ")
      }]`;
    case "object": {
      if (value.fields.length === 0) return "{}";
      const indent = "  ".repeat(depth + 1);
      const close = "  ".repeat(depth);
      const lines: string[] = ["{"];
      for (const field of value.fields) {
        appendComments(lines, field.comments, indent);
        lines.push(
          `${indent}${printProperty(field.name)}: ${
            printExpression(field.value, depth + 1)
          },`,
        );
      }
      lines.push(`${close}}`);
      return lines.join("\n");
    }
  }
}

function printReference(
  reference: Pick<ManagedReference, "declaration" | "path">,
): string {
  return reference.path.reduce<string>((text, segment) => {
    if (typeof segment === "number") return `${text}[${segment}]`;
    if (typeof segment === "object") return `${text}[${quote(segment.member)}]`;
    return validIdentifier(segment)
      ? `${text}.${segment}`
      : `${text}[${quote(segment)}]`;
  }, reference.declaration);
}

function appendComments(
  lines: string[],
  comments: readonly string[],
  indent: string,
): void {
  for (const comment of comments) lines.push(`${indent}// ${comment}`);
}

function printProperty(name: string): string {
  return validIdentifier(name) ? name : quote(name);
}

function validIdentifier(value: string): boolean {
  return /^[$_\p{ID_Start}][$_\u200C\u200D\p{ID_Continue}]*$/u.test(value);
}

function quote(value: string): string {
  return JSON.stringify(value);
}

function utf8OffsetTable(value: string): number[] {
  const offsets = new Array<number>(value.length + 1);
  let bytes = 0;
  let index = 0;
  while (index < value.length) {
    offsets[index] = bytes;
    const code = value.codePointAt(index)!;
    const width16 = code > 0xffff ? 2 : 1;
    const width8 = code <= 0x7f
      ? 1
      : code <= 0x7ff
      ? 2
      : code <= 0xffff
      ? 3
      : 4;
    for (let inner = 1; inner < width16; inner += 1) {
      offsets[index + inner] = bytes;
    }
    index += width16;
    bytes += width8;
  }
  offsets[value.length] = bytes;
  return offsets;
}

function compileError(
  source: string,
  start: number,
  end: number,
  message: string,
): ManagedCompileError {
  const offsets = utf8OffsetTable(source);
  return new ManagedCompileError(message, {
    start: offsets[start]!,
    end: offsets[end]!,
  });
}

function fail(context: ParseContext, node: ts.Node, message: string): never {
  throw compileError(
    context.source,
    node.getStart(context.sourceFile),
    node.end,
    message,
  );
}

/** Browser/Deno-safe SHA-256 used by every managed sketch source and wire digest. */
export function managedContentDigest(value: string | Uint8Array): string {
  const bytes = typeof value === "string"
    ? new TextEncoder().encode(value)
    : value;
  const paddedLength = Math.ceil((bytes.length + 9) / 64) * 64;
  const padded = new Uint8Array(paddedLength);
  padded.set(bytes);
  padded[bytes.length] = 0x80;
  const paddedView = new DataView(padded.buffer);
  const bitLength = bytes.length * 8;
  paddedView.setUint32(paddedLength - 8, Math.floor(bitLength / 0x1_0000_0000));
  paddedView.setUint32(paddedLength - 4, bitLength >>> 0);

  const hash = new Uint32Array([
    0x6a09e667,
    0xbb67ae85,
    0x3c6ef372,
    0xa54ff53a,
    0x510e527f,
    0x9b05688c,
    0x1f83d9ab,
    0x5be0cd19,
  ]);
  const words = new Uint32Array(64);
  for (let chunk = 0; chunk < paddedLength; chunk += 64) {
    for (let index = 0; index < 16; index += 1) {
      words[index] = paddedView.getUint32(chunk + index * 4);
    }
    for (let index = 16; index < 64; index += 1) {
      const left = words[index - 15]!;
      const right = words[index - 2]!;
      const sigma0 = rotateRight(left, 7) ^ rotateRight(left, 18) ^
        (left >>> 3);
      const sigma1 = rotateRight(right, 17) ^ rotateRight(right, 19) ^
        (right >>> 10);
      words[index] =
        (words[index - 16]! + sigma0 + words[index - 7]! + sigma1) >>> 0;
    }

    let a = hash[0]!;
    let b = hash[1]!;
    let c = hash[2]!;
    let d = hash[3]!;
    let e = hash[4]!;
    let f = hash[5]!;
    let g = hash[6]!;
    let h = hash[7]!;
    for (let index = 0; index < 64; index += 1) {
      const upperE = rotateRight(e, 6) ^ rotateRight(e, 11) ^
        rotateRight(e, 25);
      const choose = (e & f) ^ (~e & g);
      const first =
        (h + upperE + choose + SHA256_CONSTANTS[index]! + words[index]!) >>> 0;
      const upperA = rotateRight(a, 2) ^ rotateRight(a, 13) ^
        rotateRight(a, 22);
      const majority = (a & b) ^ (a & c) ^ (b & c);
      const second = (upperA + majority) >>> 0;
      h = g;
      g = f;
      f = e;
      e = (d + first) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (first + second) >>> 0;
    }
    hash[0] = (hash[0]! + a) >>> 0;
    hash[1] = (hash[1]! + b) >>> 0;
    hash[2] = (hash[2]! + c) >>> 0;
    hash[3] = (hash[3]! + d) >>> 0;
    hash[4] = (hash[4]! + e) >>> 0;
    hash[5] = (hash[5]! + f) >>> 0;
    hash[6] = (hash[6]! + g) >>> 0;
    hash[7] = (hash[7]! + h) >>> 0;
  }
  return [...hash].map((word) => word.toString(16).padStart(8, "0")).join("");
}

const sha256 = managedContentDigest;

function rotateRight(value: number, count: number): number {
  return (value >>> count) | (value << (32 - count));
}

const SHA256_CONSTANTS = new Uint32Array([
  0x428a2f98,
  0x71374491,
  0xb5c0fbcf,
  0xe9b5dba5,
  0x3956c25b,
  0x59f111f1,
  0x923f82a4,
  0xab1c5ed5,
  0xd807aa98,
  0x12835b01,
  0x243185be,
  0x550c7dc3,
  0x72be5d74,
  0x80deb1fe,
  0x9bdc06a7,
  0xc19bf174,
  0xe49b69c1,
  0xefbe4786,
  0x0fc19dc6,
  0x240ca1cc,
  0x2de92c6f,
  0x4a7484aa,
  0x5cb0a9dc,
  0x76f988da,
  0x983e5152,
  0xa831c66d,
  0xb00327c8,
  0xbf597fc7,
  0xc6e00bf3,
  0xd5a79147,
  0x06ca6351,
  0x14292967,
  0x27b70a85,
  0x2e1b2138,
  0x4d2c6dfc,
  0x53380d13,
  0x650a7354,
  0x766a0abb,
  0x81c2c92e,
  0x92722c85,
  0xa2bfe8a1,
  0xa81a664b,
  0xc24b8b70,
  0xc76c51a3,
  0xd192e819,
  0xd6990624,
  0xf40e3585,
  0x106aa070,
  0x19a4c116,
  0x1e376c08,
  0x2748774c,
  0x34b0bcb5,
  0x391c0cb3,
  0x4ed8aa4a,
  0x5b9cca4f,
  0x682e6ff3,
  0x748f82ee,
  0x78a5636f,
  0x84c87814,
  0x8cc70208,
  0x90befffa,
  0xa4506ceb,
  0xbef9a3f7,
  0xc67178f2,
]);

function canonicalJson(value: unknown, float = false): string {
  if (
    value === null || typeof value === "string" || typeof value === "boolean"
  ) return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError("canonical managed sketch data must be finite");
    }
    return float
      ? formatRustFloat(value)
      : Object.is(value, -0)
      ? "-0"
      : JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map((child) => canonicalJson(child)).join(",")}]`;
  }
  if (typeof value === "object" && value !== null) {
    const record = value as Readonly<Record<string, unknown>>;
    return `{${
      Object.entries(record).map(([key, child]) => {
        const childIsFloat = record.kind === "number" && key === "value";
        const encoded = record.kind === "object" && key === "value" &&
            typeof child === "object" && child !== null && !Array.isArray(child)
          ? canonicalManagedValueObject(child as Readonly<Record<string, unknown>>)
          : canonicalJson(child, childIsFloat);
        return `${JSON.stringify(key)}:${encoded}`;
      }).join(",")
    }}`;
  }
  throw new TypeError("canonical managed sketch data must be data-only");
}

/** Rust stores `ManagedValue::Object` in a `BTreeMap`; mirror that exact order. */
function canonicalManagedValueObject(
  value: Readonly<Record<string, unknown>>,
): string {
  const entries = Object.entries(value)
    .sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)
    .map(([key, child]) => `${JSON.stringify(key)}:${canonicalJson(child)}`);
  return `{${entries.join(",")}}`;
}

function formatRustFloat(value: number): string {
  if (!Number.isFinite(value)) {
    throw new TypeError("managed sketch numbers must be finite");
  }
  if (Object.is(value, -0)) return "-0.0";
  const magnitude = Math.abs(value);
  if (Number.isInteger(value) && magnitude < 1e16) return `${value}.0`;
  return magnitude !== 0 && (magnitude < 1e-5 || magnitude >= 1e16)
    ? value.toExponential()
    : String(value);
}

function deepFreeze<Value>(value: Value): Value {
  if (typeof value !== "object" || value === null || Object.isFrozen(value)) {
    return value;
  }
  for (const child of Object.values(value)) deepFreeze(child);
  return Object.freeze(value);
}
