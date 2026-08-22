// SPDX-License-Identifier: GPL-3.0-or-later

/** Data-only TypeScript bindings for `geosolve.lineage.rpc.v0`. */

declare const brand: unique symbol;
export type Branded<T, Name extends string> = T & { readonly [brand]: Name };

export type LineageDocumentId = Branded<string, "LineageDocumentId">;
export type LineageRevision = Branded<string, "LineageRevision">;
export type LineageDigest = Branded<string, "LineageDigest">;
export type LineageStepId = Branded<string, "LineageStepId">;
export type LineageOutputId = Branded<string, "LineageOutputId">;
export type LineageReservationId = Branded<string, "LineageReservationId">;
export type LineageSessionId = Branded<string, "LineageSessionId">;
export type DeveloperKey = Branded<string, "DeveloperKey">;
export type SemanticKey = Branded<string, "SemanticKey">;
export type PersistentId = Branded<string, "PersistentId">;

export type OutputKind =
  | "point"
  | "scalar"
  | "curve"
  | "curve_span"
  | "trim_view"
  | "contact"
  | "constraint"
  | "dimension"
  | "source"
  | "parameter"
  | "parameter_binding"
  | "parameter_output"
  | "external_binding"
  | "geometry_role"
  | "activation"
  | "profile"
  | "chain"
  | "operation"
  | "feature"
  | "feature_corner"
  | "annotation"
  | "collection";

export type ReservationKind =
  | "point"
  | "scalar"
  | "curve"
  | "curve_span"
  | "trim_view"
  | "contact"
  | "constraint"
  | "dimension"
  | "source"
  | "parameter"
  | "parameter_binding"
  | "parameter_output"
  | "external_binding"
  | "profile"
  | "chain"
  | "operation"
  | "feature"
  | "feature_corner"
  | "annotation"
  | "hidden_support";

export interface LineageIdentity {
  readonly document: LineageDocumentId;
  readonly revision: LineageRevision;
  readonly digest: LineageDigest;
}

export interface OutputRef<Kind extends OutputKind = OutputKind> {
  readonly document: LineageDocumentId;
  readonly step: LineageStepId;
  readonly output: LineageOutputId;
  readonly kind: Kind;
}

export interface InputBinding<Kind extends OutputKind = OutputKind> {
  readonly key: SemanticKey;
  readonly kind: Kind;
  readonly source: OutputRef<Kind>;
}

export interface ActionPayload {
  readonly schema: SemanticKey;
  readonly version: number;
  readonly inputs: readonly InputBinding[];
  readonly parameters: Readonly<Record<string, JsonValue>>;
}

export type ActionDefinition =
  | { readonly kind: "imported_baseline"; readonly baseline: ImportedBaselineAction }
  | { readonly kind: "geometry_recipe"; readonly action: ActionPayload }
  | { readonly kind: "constraint"; readonly action: ActionPayload }
  | { readonly kind: "dimension"; readonly action: ActionPayload }
  | { readonly kind: "trim"; readonly action: ActionPayload }
  | { readonly kind: "parameter"; readonly action: ActionPayload }
  | { readonly kind: "binding"; readonly action: ActionPayload }
  | { readonly kind: "external"; readonly action: ActionPayload }
  | { readonly kind: "operation"; readonly action: ActionPayload }
  | { readonly kind: "computed_feature"; readonly action: ActionPayload }
  | { readonly kind: "annotation"; readonly action: ActionPayload };

export type ImportedBaselineEncoding =
  | { readonly kind: "canonical"; readonly schema: SemanticKey; readonly version: number }
  | { readonly kind: "opaque"; readonly media_type: SemanticKey; readonly version: number };

export interface ImportedBaselineAction {
  readonly encoding: ImportedBaselineEncoding;
  readonly payload: string;
}

export interface OutputDeclaration<Kind extends OutputKind = OutputKind> {
  readonly id: LineageOutputId;
  readonly key: SemanticKey;
  readonly kind: Kind;
  readonly reservation: LineageReservationId | null;
}

export interface ReservationDeclaration {
  readonly id: LineageReservationId;
  readonly key: SemanticKey;
  readonly kind: ReservationKind;
  readonly persistent_id: PersistentId;
}

export type OutputIdentityFlow =
  | { readonly kind: "owned_logical" }
  | { readonly kind: "created"; readonly reservation: LineageReservationId }
  | { readonly kind: "aliased"; readonly source: OutputRef }
  | { readonly kind: "continued"; readonly source: OutputRef }
  | { readonly kind: "retired"; readonly source: OutputRef };

export interface OutputIdentityDeclaration {
  readonly output: LineageOutputId;
  readonly flow: OutputIdentityFlow;
}

export interface WritableLeafDeclaration {
  readonly output: LineageOutputId;
  readonly key: SemanticKey;
}

export interface LineageStep {
  readonly id: LineageStepId;
  readonly key: DeveloperKey;
  readonly label: string;
  readonly state: "live" | "suppressed" | "tombstoned";
  readonly action: ActionDefinition;
  readonly outputs: readonly OutputDeclaration[];
  readonly output_identities: readonly OutputIdentityDeclaration[];
  readonly writable_leaves: readonly WritableLeafDeclaration[];
  readonly reservations: readonly ReservationDeclaration[];
}

export interface LineageStepRewrite {
  readonly label: string;
  readonly action: ActionDefinition;
}

/** Closed structural mutation vocabulary accepted by Rust's `LineagePatch`. */
export type LineageMutation =
  | { readonly kind: "insert"; readonly before: LineageStepId | null; readonly step: LineageStep }
  | {
      readonly kind: "rewrite";
      readonly step: LineageStepId;
      readonly replacement: LineageStepRewrite;
    }
  | { readonly kind: "tombstone"; readonly step: LineageStepId }
  | { readonly kind: "delete_subtree"; readonly root: LineageStepId }
  | { readonly kind: "set_suppressed"; readonly step: LineageStepId; readonly suppressed: boolean }
  | { readonly kind: "reorder"; readonly step: LineageStepId; readonly before: LineageStepId | null }
  | {
      readonly kind: "rebind";
      readonly step: LineageStepId;
      readonly input: SemanticKey;
      readonly target: OutputRef;
    }
  | { readonly kind: "set_evaluation_policy"; readonly policy: LineageEvaluationPolicyKey };

export interface LineagePatch {
  readonly expected: LineageIdentity;
  readonly mutations: readonly LineageMutation[];
}

export type LineageRewriteMutation = Extract<LineageMutation, { readonly kind: "rewrite" }>;

/** Exact owner-rewrite batch accepted by the dedicated `rewrite_owners` RPC method. */
export interface LineageRewritePatch {
  readonly expected: LineageIdentity;
  readonly mutations: readonly LineageRewriteMutation[];
}

export type JsonPrimitive = null | boolean | number | string;
export type JsonValue = JsonPrimitive | readonly JsonValue[] | { readonly [key: string]: JsonValue };

export const actionKindKeys = [
  "imported-baseline",
  "geometry-recipe",
  "constraint",
  "dimension",
  "trim",
  "parameter",
  "binding",
  "external",
  "operation",
  "computed-feature",
  "annotation",
] as const;

export const outputKindKeys = [
  "point",
  "scalar",
  "curve",
  "curve-span",
  "trim-view",
  "contact",
  "constraint",
  "dimension",
  "source",
  "parameter",
  "parameter-binding",
  "parameter-output",
  "external-binding",
  "geometry-role",
  "activation",
  "profile",
  "chain",
  "operation",
  "feature",
  "feature-corner",
  "annotation",
  "collection",
] as const;

export const reservationKindKeys = [
  "point",
  "scalar",
  "curve",
  "curve-span",
  "trim-view",
  "contact",
  "constraint",
  "dimension",
  "source",
  "parameter",
  "parameter-binding",
  "parameter-output",
  "external-binding",
  "profile",
  "chain",
  "operation",
  "feature",
  "feature-corner",
  "annotation",
  "hidden-support",
] as const;

export const geometryRecipeKeys = [
  "sketch-point",
  "segment",
  "polyline",
  "midpoint-line",
  "two-point-aligned-rectangle",
  "three-point-corner-rectangle",
  "center-rectangle",
  "three-point-center-rectangle",
  "center-radius-circle",
  "two-point-diameter-circle",
  "three-point-circle",
  "center-arc",
  "three-point-arc",
  "tangent-arc",
  "center-axes-ellipse",
  "axis-endpoints-ellipse",
  "center-axes-elliptical-arc",
  "axis-endpoints-elliptical-arc",
  "quadratic-bezier",
  "cubic-bezier",
  "rational-quadratic-conic",
  "parabola",
  "hyperbola",
  "open-control-nurbs",
  "periodic-control-nurbs",
] as const;
export type GeometryRecipeKey = (typeof geometryRecipeKeys)[number];

export const constraintKeys = [
  "fixed-point",
  "fixed-coordinate",
  "coincident-with-origin",
  "point-on-datum-axis",
  "coincident",
  "external-point-coincident",
  "horizontal",
  "vertical",
  "horizontal-points",
  "vertical-points",
  "horizontal-point-to-midpoint",
  "vertical-point-to-midpoint",
  "point-on-curve",
  "parallel",
  "perpendicular",
  "external-line-collinear",
  "collinear-with-datum-axis",
  "concentric",
  "collinear",
  "equal-length",
  "equal-radius",
  "midpoint",
  "symmetric-about-line",
  "symmetric-about-datum-axis",
  "line-circle-tangency",
  "circle-circle-tangency",
  "circle-arc-tangency",
  "line-curve-tangency",
  "curve-curve-contact",
  "curve-curve-tangency",
  "curve-direction",
  "equal-curvature",
  "endpoint-continuity",
  "line-line-fillet",
  "curve-curve-fillet",
] as const;
export type ConstraintKey = (typeof constraintKeys)[number];

export const dimensionKeys = [
  "point-distance",
  "curve-length",
  "radius",
  "diameter",
  "oriented-angle",
  "supporting-line-offset",
  "exact-translated-segment-offset",
  "profile-offset",
] as const;
export type DimensionKey = (typeof dimensionKeys)[number];

export const operationKeys = [
  "split",
  "break",
  "trim",
  "extend",
  "mirror",
  "chamfer",
  "associative-fillet",
  "rectangle",
  "regular-polygon",
  "slot",
  "linear-pattern",
  "profile-offset",
] as const;
export type OperationKey = (typeof operationKeys)[number];

export const nativePublicationKeys = ["native-line-fillet"] as const;
export type NativePublicationKey = (typeof nativePublicationKeys)[number];

export const computedFeatureKeys = ["fillet-set"] as const;
export type ComputedFeatureKey = (typeof computedFeatureKeys)[number];

export const trimKeys = ["curve-trim-view"] as const;
export type TrimKey = (typeof trimKeys)[number];

export const parameterKeys = ["host-parameter"] as const;
export type ParameterKey = (typeof parameterKeys)[number];

export const bindingKeys = [
  "parameter-binding",
  "parameter-output",
  "host-activation",
  "geometry-role",
] as const;
export type BindingKey = (typeof bindingKeys)[number];

export const externalKeys = ["external-binding", "external-snapshot"] as const;
export type ExternalKey = (typeof externalKeys)[number];

export const annotationKeys = ["annotation-layout"] as const;
export type AnnotationKey = (typeof annotationKeys)[number];

export const curveControlKeys = [
  "center",
  "start-point",
  "end-point",
  "control-point",
  "radius",
  "trim-start",
  "trim-end",
  "major-axis-point",
  "minor-axis",
  "rational-middle",
  "vertex",
  "focus",
  "transverse-axis-point",
  "conjugate-axis",
] as const;

export const curvePropertyKeys = [
  "radius",
  "minor-axis-ratio",
  "trim-start",
  "trim-end",
  "semi-conjugate",
  "rational-weight",
  "nurbs-weight",
] as const;

export const geometryRoleKeys = ["profile", "construction"] as const;

export const explicitBranchFamilyKeys = [
  "line-direction",
  "polyline-direction",
  "arc-sweep",
  "elliptical-arc-sweep",
  "hyperbola-branch",
  "contact-parameter",
  "contact-winding",
  "contact-neighborhood",
  "normal-side",
  "retained-endpoint",
  "periodic-anchor",
  "tangency-mode",
  "tangency-direction",
  "fillet-sides",
  "fillet-trim-endpoints",
  "fillet-endpoint-order",
  "fillet-sweep",
  "angle-orientation",
  "profile-offset-direction",
  "profile-offset-traversal",
  "profile-offset-junction",
  "profile-offset-terminal-policy",
  "spline-form",
  "spline-span-transition",
  "nurbs-gauge",
  "source-suppression",
  "element-activation",
] as const;

export interface ActionArguments {
  readonly inputs?: readonly InputBinding[];
  readonly parameters?: Readonly<Record<string, JsonValue>>;
}

type KeyedBuilders<Key extends string> = {
  readonly [K in Key]: (arguments_: ActionArguments) => ActionPayload;
};

function action(schema: string, arguments_: ActionArguments): ActionPayload {
  return {
    schema: semanticKey(schema),
    version: 1,
    inputs: [...(arguments_.inputs ?? [])].sort((left, right) =>
      left.key.localeCompare(right.key),
    ),
    parameters: Object.fromEntries(
      Object.entries(arguments_.parameters ?? {}).sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    ),
  };
}

function keyedBuilders<Key extends string>(
  prefix: string,
  keys: readonly Key[],
): KeyedBuilders<Key> {
  return Object.fromEntries(
    keys.map((key) => [key, (arguments_: ActionArguments) => action(`${prefix}.${key}`, arguments_)]),
  ) as KeyedBuilders<Key>;
}

/** Exhaustive builders for every M78 geometry recipe. */
export const geometry = keyedBuilders("geosolve.geometry.v1", geometryRecipeKeys);
/** Exhaustive builders for every persistent constraint definition. */
export const constraints = keyedBuilders("geosolve.constraint.v1", constraintKeys);
/** Exhaustive builders for every persistent dimension definition. */
export const dimensions = keyedBuilders("geosolve.dimension.v1", dimensionKeys);
/** Exhaustive builders for the twelve public sketch-operation kinds. */
export const operations = keyedBuilders("geosolve.operation.v1", operationKeys);
/** Native publication paths that do not add another `SketchOperationKind`. */
export const nativePublications = keyedBuilders(
  "geosolve.operation.v1",
  nativePublicationKeys,
);
/** Exhaustive builders for current computed-feature intent. */
export const computedFeatures = keyedBuilders("geosolve.feature.v1", computedFeatureKeys);
/** Persistent equation-free trim-view declarations. */
export const trims = keyedBuilders("geosolve.trim.v1", trimKeys);
/** Host-owned parameter declarations. */
export const parameters = keyedBuilders("geosolve.parameter.v1", parameterKeys);
/** Persistent parameter/output/role/activation bindings. */
export const bindings = keyedBuilders("geosolve.binding.v1", bindingKeys);
/** External binding declarations and immutable input snapshots. */
export const externals = keyedBuilders("geosolve.external.v1", externalKeys);
/** Stable semantic annotation-layout records. */
export const annotations = keyedBuilders("geosolve.annotation.v1", annotationKeys);

export interface StepArguments {
  readonly key: DeveloperKey;
  readonly label: string;
  readonly action: ActionDefinition;
  readonly outputs?: readonly (Omit<OutputDeclaration, "id"> & {
    readonly identity?: OutputIdentityFlow;
    readonly writable?: readonly SemanticKey[];
  })[];
  readonly reservations?: readonly Omit<ReservationDeclaration, "id">[];
}

export interface AllocatorHighWater {
  readonly next_step_id: LineageStepId;
  readonly next_output_id: LineageOutputId;
  readonly next_reservation_id: LineageReservationId;
}

/**
 * Allocates only lineage-local IDs. Persistent native reservations remain
 * explicit host inputs; no geometry or native identity is invented here.
 */
export class LineagePatchBuilder {
  readonly #expected: LineageIdentity;
  #step: bigint;
  #output: bigint;
  #reservation: bigint;
  readonly #mutations: LineageMutation[] = [];

  constructor(expected: LineageIdentity, highWater: AllocatorHighWater) {
    this.#expected = expected;
    this.#step = parseHex(highWater.next_step_id);
    this.#output = parseHex(highWater.next_output_id);
    this.#reservation = parseHex(highWater.next_reservation_id);
  }

  insert(arguments_: StepArguments): Readonly<Record<string, OutputRef>> {
    const step = hexId<LineageStepId>(this.#step++);
    const reservations = (arguments_.reservations ?? []).map((reservation) => ({
      ...reservation,
      id: hexId<LineageReservationId>(this.#reservation++),
    }));
    const declaredOutputs = arguments_.outputs ?? [];
    const outputs = declaredOutputs.map(
      ({ identity: _identity, writable: _writable, ...output }) => ({
        ...output,
        id: hexId<LineageOutputId>(this.#output++),
      }),
    );
    const outputIdentities = declaredOutputs.map((declared, index) => {
      const output = outputs[index];
      if (output === undefined) {
        throw new Error("lineage output allocation was not reproducible");
      }
      const flow =
        declared.identity ??
        (output.reservation === null
          ? { kind: "owned_logical" as const }
          : { kind: "created" as const, reservation: output.reservation });
      if (
        (flow.kind === "created" && output.reservation !== flow.reservation) ||
        (flow.kind !== "created" && output.reservation !== null)
      ) {
        throw new Error("lineage output identity does not match its reservation");
      }
      return { output: output.id, flow };
    });
    const writableLeaves = declaredOutputs.flatMap((declared, index) => {
      const output = outputs[index];
      if (output === undefined) {
        throw new Error("lineage writable-leaf allocation was not reproducible");
      }
      return (declared.writable ?? []).map((key) => ({ output: output.id, key }));
    });
    const value: LineageStep = {
      id: step,
      key: arguments_.key,
      label: arguments_.label,
      state: "live",
      action: arguments_.action,
      outputs,
      output_identities: outputIdentities,
      writable_leaves: writableLeaves,
      reservations,
    };
    this.#mutations.push({ kind: "insert", before: null, step: value });
    return Object.fromEntries(
      outputs.map((output) => [
        output.key,
        { document: this.#expected.document, step, output: output.id, kind: output.kind },
      ]),
    );
  }

  patch(): LineagePatch {
    return {
      expected: {
        document: this.#expected.document,
        revision: this.#expected.revision,
        digest: this.#expected.digest,
      },
      mutations: [...this.#mutations],
    };
  }
}

export interface RpcTransport {
  request(request: string): string;
}

export const RPC_PROTOCOL = "geosolve.lineage.rpc.v0" as const;
/** Must remain byte-for-byte equal to Rust's bounded RPC request limit. */
export const RPC_MAX_REQUEST_BYTES = 16 * 1024 * 1024;
/** Rust's maximum retained step/result-row cardinality. */
export const RPC_MAX_RESULT_ITEMS = 100_000;
const RPC_MAX_HISTORY_ITEMS = 1_024;

export type RpcMethod =
  | "create"
  | "load"
  | "import"
  | "inspect"
  | "reconcile"
  | "mutate"
  | "rewrite_owners"
  | "set_policy"
  | "undo"
  | "redo"
  | "evaluate"
  | "export"
  | "export_lineage";

export type LineageEvaluationPolicyKey = "strict_chronological" | "dependency_local";
export type LineageEvaluationDispositionKey =
  | "pending"
  | "accepted"
  | "failed"
  | "cancelled"
  | "exhausted"
  | "stale";

export interface LineageEvaluationAttempt {
  readonly target: LineageIdentity;
  readonly policy: LineageEvaluationPolicyKey;
  readonly disposition: LineageEvaluationDispositionKey;
  readonly external_inputs: string | null;
  readonly materialization_digest: LineageDigest | null;
  readonly failed_steps: readonly LineageStepId[];
  readonly diagnostic: string | null;
}

export interface LineageAcceptedAuthority {
  readonly lineage: LineageIdentity;
  readonly external_inputs: string | null;
  readonly materialization_digest: LineageDigest;
}

export interface LineageLifecycleSnapshot {
  readonly revision: LineageRevision;
  readonly next_step_id: LineageStepId;
  readonly next_output_id: LineageOutputId;
  readonly next_reservation_id: LineageReservationId;
}

export interface LineageSnapshot {
  readonly session_id: LineageSessionId;
  readonly identity: LineageIdentity;
  readonly evaluation_policy: LineageEvaluationPolicyKey;
  readonly can_undo: boolean;
  readonly can_redo: boolean;
  readonly undo_length: string;
  readonly redo_length: string;
  readonly lifecycle: LineageLifecycleSnapshot;
  readonly latest_attempt: LineageEvaluationAttempt | null;
  readonly last_accepted: LineageAcceptedAuthority | null;
  readonly last_accepted_document: Readonly<Record<string, JsonValue>> | null;
  readonly document: Readonly<Record<string, JsonValue>>;
}

export interface LineageMutationResult {
  readonly identity: LineageIdentity;
  readonly changed: boolean;
  readonly inserted_steps: readonly LineageStepId[];
  readonly tombstoned_steps: readonly LineageStepId[];
}

export interface LineageIdentityResult {
  readonly identity: LineageIdentity;
}

export type LineageEvaluationStepState =
  | "ready"
  | "suppressed"
  | "tombstoned"
  | "failed"
  | "blocked"
  | "unevaluated";

export interface LineageEvaluationStep {
  readonly step: LineageStepId;
  readonly state: LineageEvaluationStepState;
  readonly dependencies: readonly LineageStepId[];
}

export const evaluationFailureCodes = [
  "dependency_blocked",
  "dependency_local_mismatch",
  "feature_document_invalid",
  "feature_evaluation_error",
  "feature_evidence_error",
  "feature_namespace_mismatch",
  "host_input_identity_mismatch",
  "host_input_invalid",
  "lineage_session_invalid",
  "materialization_evidence_error",
  "materialization_map_error",
  "sketch_document_invalid",
  "sketch_evaluation_error",
  "sketch_evaluation_rejected",
  "sketch_evidence_error",
  "workbench_materialization_unsupported",
] as const;
export type LineageEvaluationFailureCode = (typeof evaluationFailureCodes)[number];

export interface LineageAcceptedEvaluationEvidence {
  readonly evaluator: "geosolve.constraint-editor.lineage-cold.v1";
  readonly external_inputs: string;
  readonly materialization_digest: LineageDigest;
  readonly sketch_digest: LineageDigest;
  readonly feature_digest: LineageDigest;
  readonly computed_feature_count: number;
  readonly validated_prefix_count: number;
  readonly strict_prefix_digest: LineageDigest;
}

export interface LineageFailedEvaluationEvidence {
  readonly evaluator: "geosolve.constraint-editor.lineage-cold.v1";
  readonly code: LineageEvaluationFailureCode;
  readonly message: string;
  readonly failed_step?: LineageStepId | null;
  readonly failed_features: readonly string[];
}

interface LineageEvaluationResultBase {
  readonly identity: LineageIdentity;
  readonly steps: readonly LineageEvaluationStep[];
  readonly attempt: LineageEvaluationAttempt;
  readonly last_accepted: LineageAcceptedAuthority | null;
}

export type LineageEvaluationResult =
  | (LineageEvaluationResultBase & {
      readonly disposition: "accepted";
      readonly evidence: LineageAcceptedEvaluationEvidence;
      readonly last_accepted: LineageAcceptedAuthority;
    })
  | (LineageEvaluationResultBase & {
      readonly disposition: "failed";
      readonly evidence: LineageFailedEvaluationEvidence;
    });

export interface LineageSessionExportResult {
  readonly identity: LineageIdentity;
  readonly session_json: string;
}

export interface LineageDocumentExportResult {
  readonly identity: LineageIdentity;
  readonly lineage_json: string;
}

export interface RpcResultByMethod {
  readonly create: LineageSnapshot;
  readonly load: LineageSnapshot;
  readonly import: LineageSnapshot;
  readonly inspect: LineageSnapshot;
  readonly reconcile: LineageIdentityResult;
  readonly mutate: LineageMutationResult;
  readonly rewrite_owners: LineageMutationResult;
  readonly set_policy: LineageMutationResult;
  readonly undo: LineageSnapshot;
  readonly redo: LineageSnapshot;
  readonly evaluate: LineageEvaluationResult;
  readonly export: LineageSessionExportResult;
  readonly export_lineage: LineageDocumentExportResult;
}

/** Closed JSON object used by RPC methods that accept no parameters. */
export type EmptyRpcParams = Readonly<Record<string, never>>;

export interface LineageRpcHostInputs {
  readonly version: 1;
  readonly parameter_batch_json: string;
  readonly external_snapshot_set_json: string;
}

export interface RpcParamsByMethod {
  readonly create: { readonly document_id?: LineageDocumentId };
  readonly load: { readonly session_json: string };
  readonly import: { readonly lineage_json: string };
  readonly inspect: EmptyRpcParams;
  readonly reconcile: { readonly expected: LineageIdentity; readonly lineage_json: string };
  readonly mutate: { readonly patch: LineagePatch };
  readonly rewrite_owners: { readonly patch: LineageRewritePatch };
  readonly set_policy: {
    readonly expected: LineageIdentity;
    readonly policy: LineageEvaluationPolicyKey;
  };
  readonly undo: { readonly expected: LineageIdentity };
  readonly redo: { readonly expected: LineageIdentity };
  readonly evaluate: {
    readonly expected: LineageIdentity;
    readonly host_inputs: LineageRpcHostInputs;
  };
  readonly export: EmptyRpcParams;
  readonly export_lineage: EmptyRpcParams;
}

export const rpcErrorCodes = [
  "action_kind_mismatch",
  "create_failed",
  "dependency_local_mismatch",
  "feature_document_invalid",
  "feature_evaluation_error",
  "feature_evidence_error",
  "feature_namespace_mismatch",
  "host_input_identity_mismatch",
  "host_input_invalid",
  "id_exhausted",
  "internal_serialization",
  "invalid_document_id",
  "invalid_editor_authority",
  "invalid_external_snapshot_set",
  "invalid_failed_step",
  "invalid_owner_rewrite",
  "invalid_parameter_batch",
  "invalid_params",
  "invalid_request",
  "invalid_state_transition",
  "lineage_rejected",
  "lineage_session_invalid",
  "materialization_evidence_error",
  "materialization_map_error",
  "missing_session",
  "not_initialized",
  "nothing_to_redo",
  "nothing_to_undo",
  "request_too_large",
  "resource_limit",
  "revision_exhausted",
  "sketch_document_invalid",
  "sketch_evaluation_error",
  "sketch_evaluation_rejected",
  "sketch_evidence_error",
  "stale_revision",
  "unexpected_session",
  "unknown_method",
  "unsupported_action_schema",
  "unsupported_action_version",
  "unsupported_host_input_version",
  "unsupported_protocol",
  "workbench_materialization_unsupported",
  "wrong_document",
  "wrong_port_kind",
  "wrong_session",
] as const;
export type RpcErrorCode = (typeof rpcErrorCodes)[number];

export type RpcSuccessResponse<Method extends RpcMethod = RpcMethod> =
  Method extends RpcMethod
    ? {
        readonly protocol: typeof RPC_PROTOCOL;
        readonly request_id: string;
        readonly method: Method;
        readonly session_id: LineageSessionId;
        readonly ok: true;
        readonly result: RpcResultByMethod[Method];
      }
    : never;

export type RpcErrorResponse<Method extends RpcMethod = RpcMethod> =
  Method extends RpcMethod
    ? {
        readonly protocol: typeof RPC_PROTOCOL;
        readonly request_id: string;
        readonly method: Method;
        readonly session_id: LineageSessionId | null;
        readonly ok: false;
        readonly error: { readonly code: RpcErrorCode; readonly message: string };
      }
    : never;

export type RpcResponse<Method extends RpcMethod = RpcMethod> = Method extends RpcMethod
  ? RpcSuccessResponse<Method> | RpcErrorResponse<Method>
  : never;

export type RpcResponseViolation =
  | "invalid_json"
  | "invalid_envelope"
  | "invalid_result"
  | "unknown_error_code"
  | "protocol_mismatch"
  | "request_id_mismatch"
  | "method_mismatch"
  | "session_id_mismatch";

/** A transport response was not a correlated `geosolve.lineage.rpc.v0` envelope. */
export class LineageRpcResponseError extends Error {
  readonly violation: RpcResponseViolation;

  constructor(violation: RpcResponseViolation, message: string) {
    super(message);
    this.name = "LineageRpcResponseError";
    this.violation = violation;
  }
}

/** A request could not enter the bounded Rust RPC transport contract. */
export class LineageRpcRequestError extends Error {
  readonly code = "request_too_large" as const;
  readonly byte_length: number;
  readonly byte_limit = RPC_MAX_REQUEST_BYTES;

  constructor(byteLength: number) {
    super(
      `lineage RPC request is ${byteLength} UTF-8 bytes; the limit is ${RPC_MAX_REQUEST_BYTES}`,
    );
    this.name = "LineageRpcRequestError";
    this.byte_length = byteLength;
  }
}

interface LineageClientCorrelationState {
  /** Protocol authority needed to correlate traversal; never flattened geometry. */
  readonly policy: LineageEvaluationPolicyKey;
  readonly last_accepted: LineageAcceptedAuthority | null;
}

interface LineageClientHistoryState {
  readonly undo: readonly LineageClientCorrelationState[];
  readonly redo: readonly LineageClientCorrelationState[];
}

/** Thin typed transport wrapper; Rust remains the sole lineage/geometry validator. */
export class LineageRpcClient {
  readonly #transport: RpcTransport;
  #request = 0n;
  #session: LineageSessionId | null = null;
  #identity: LineageIdentity | null = null;
  #policy: LineageEvaluationPolicyKey | null = null;
  #lastAccepted: LineageAcceptedAuthority | null = null;
  #undo: LineageClientCorrelationState[] = [];
  #redo: LineageClientCorrelationState[] = [];

  constructor(transport: RpcTransport) {
    this.#transport = transport;
  }

  call<Method extends RpcMethod>(
    method: Method,
    params: RpcParamsByMethod[Method],
  ): RpcResponse<Method> {
    const requestId = String(++this.#request);
    const startsSession = method === "create" || method === "load" || method === "import";
    const requestSession = startsSession ? null : this.#session;
    const requestIdentity = startsSession || this.#identity === null
      ? null
      : { ...this.#identity };
    const requestPolicy = startsSession ? null : this.#policy;
    const requestLastAccepted = startsSession
      ? null
      : cloneNullableAuthority(this.#lastAccepted);
    const requestHistory = startsSession
      ? { undo: [], redo: [] }
      : cloneClientHistory({ undo: this.#undo, redo: this.#redo });
    const request = JSON.stringify({
      protocol: RPC_PROTOCOL,
      request_id: requestId,
      session_id: requestSession,
      method,
      params,
    });
    const requestBytes = new TextEncoder().encode(request).byteLength;
    if (requestBytes > RPC_MAX_REQUEST_BYTES) {
      // Rust deliberately cannot correlate a request rejected before bounded
      // envelope decoding. Preflight keeps the typed client inside the
      // correlated protocol; a null-correlated transport reply still fails as
      // an invalid envelope below rather than being assigned forged metadata.
      throw new LineageRpcRequestError(requestBytes);
    }
    const correlatedParams = (
      JSON.parse(request) as { readonly params: RpcParamsByMethod[Method] }
    ).params;
    const response = parseRpcResponse(
      this.#transport.request(request),
      requestId,
      method,
      correlatedParams,
      requestSession,
      requestIdentity,
      requestPolicy,
      requestLastAccepted,
      requestHistory,
      startsSession,
    );
    if (response.ok) {
      const retainedPolicy = nextRetainedPolicy(
        method,
        correlatedParams,
        response.result,
        requestPolicy,
      );
      const retainedHistory = nextRetainedHistory(
        method,
        correlatedParams,
        response.result,
        requestPolicy,
        requestLastAccepted,
        requestHistory,
      );
      this.#session = response.session_id;
      this.#identity = { ...response.result.identity };
      this.#policy = retainedPolicy;
      this.#lastAccepted = nextRetainedLastAccepted(
        method,
        response.result,
        requestLastAccepted,
      );
      this.#undo = retainedHistory.undo;
      this.#redo = retainedHistory.redo;
    }
    return response;
  }
}

function nextRetainedHistory(
  method: RpcMethod,
  params: RpcParamsByMethod[RpcMethod],
  result: RpcResultByMethod[RpcMethod],
  previousPolicy: LineageEvaluationPolicyKey | null,
  previousAccepted: LineageAcceptedAuthority | null,
  previousHistory: LineageClientHistoryState,
): { undo: LineageClientCorrelationState[]; redo: LineageClientCorrelationState[] } {
  if (method === "create" || method === "import") return { undo: [], redo: [] };
  if (method === "load") {
    const loaded = decodeLineageSessionJsonHeader(
      (params as RpcParamsByMethod["load"]).session_json,
      "load session_json",
    );
    return {
      undo: loaded.undo.map((state) => cloneClientState(state)),
      redo: loaded.redo.map((state) => cloneClientState(state)),
    };
  }
  const next = cloneClientHistory(previousHistory);
  const current = clientCorrelationState(previousPolicy, previousAccepted);
  if (
    method === "reconcile" ||
    ((method === "mutate" || method === "rewrite_owners" || method === "set_policy") &&
      (result as LineageMutationResult).changed)
  ) {
    pushBoundedClientHistory(next.undo, current);
    next.redo = [];
    return next;
  }
  if (method === "undo") {
    next.undo.pop();
    pushBoundedClientHistory(next.redo, current);
  } else if (method === "redo") {
    next.redo.pop();
    pushBoundedClientHistory(next.undo, current);
  }
  return next;
}

function clientCorrelationState(
  policy: LineageEvaluationPolicyKey | null,
  lastAccepted: LineageAcceptedAuthority | null,
): LineageClientCorrelationState {
  if (policy === null) invalidResult("lineage RPC client has no retained evaluation policy");
  return { policy, last_accepted: cloneNullableAuthority(lastAccepted) };
}

function cloneClientState(
  state: LineageClientCorrelationState,
): LineageClientCorrelationState {
  return {
    policy: state.policy,
    last_accepted: cloneNullableAuthority(state.last_accepted),
  };
}

function cloneClientHistory(
  history: LineageClientHistoryState,
): { undo: LineageClientCorrelationState[]; redo: LineageClientCorrelationState[] } {
  return {
    undo: history.undo.map((state) => cloneClientState(state)),
    redo: history.redo.map((state) => cloneClientState(state)),
  };
}

function pushBoundedClientHistory(
  history: LineageClientCorrelationState[],
  state: LineageClientCorrelationState,
): void {
  if (history.length === RPC_MAX_HISTORY_ITEMS) history.shift();
  history.push(cloneClientState(state));
}

function nextRetainedPolicy(
  method: RpcMethod,
  params: RpcParamsByMethod[RpcMethod],
  result: RpcResultByMethod[RpcMethod],
  previous: LineageEvaluationPolicyKey | null,
): LineageEvaluationPolicyKey | null {
  if (method === "create" || method === "load" || method === "import" || method === "inspect") {
    return (result as LineageSnapshot).evaluation_policy;
  }
  if (method === "set_policy") {
    return (params as RpcParamsByMethod["set_policy"]).policy;
  }
  if (method === "reconcile") {
    return decodeLineageDocumentJsonHeader(
      (params as RpcParamsByMethod["reconcile"]).lineage_json,
      "reconcile lineage_json",
    ).evaluation_policy;
  }
  if (method === "mutate") {
    return requestedMutationPolicy(
      (params as RpcParamsByMethod["mutate"]).patch,
      previous,
    );
  }
  if (method === "undo" || method === "redo") {
    return (result as LineageSnapshot).evaluation_policy;
  }
  return previous;
}

function requestedMutationPolicy(
  patch: LineagePatch,
  previous: LineageEvaluationPolicyKey | null,
): LineageEvaluationPolicyKey | null {
  let policy = previous;
  for (const mutation of patch.mutations) {
    if (mutation.kind === "set_evaluation_policy") policy = mutation.policy;
  }
  return policy;
}

function nextRetainedLastAccepted(
  method: RpcMethod,
  result: RpcResultByMethod[RpcMethod],
  previous: LineageAcceptedAuthority | null,
): LineageAcceptedAuthority | null {
  if (
    method === "create" ||
    method === "load" ||
    method === "import" ||
    method === "inspect" ||
    method === "undo" ||
    method === "redo"
  ) {
    return cloneNullableAuthority((result as LineageSnapshot).last_accepted);
  }
  if (method === "evaluate") {
    return cloneNullableAuthority((result as LineageEvaluationResult).last_accepted);
  }
  return cloneNullableAuthority(previous);
}

function parseRpcResponse<Method extends RpcMethod>(
  raw: string,
  requestId: string,
  method: Method,
  params: RpcParamsByMethod[Method],
  requestSession: LineageSessionId | null,
  requestIdentity: LineageIdentity | null,
  requestPolicy: LineageEvaluationPolicyKey | null,
  requestLastAccepted: LineageAcceptedAuthority | null,
  requestHistory: LineageClientHistoryState,
  startsSession: boolean,
): RpcResponse<Method> {
  let value: unknown;
  try {
    value = JSON.parse(raw) as unknown;
  } catch {
    throw responseError("invalid_json", "lineage RPC response is not valid JSON");
  }
  if (!isJsonObject(value)) {
    throw responseError("invalid_envelope", "lineage RPC response must be a JSON object");
  }
  if (typeof value.protocol !== "string") {
    throw responseError("invalid_envelope", "lineage RPC response protocol must be a string");
  }
  if (value.protocol !== RPC_PROTOCOL) {
    throw responseError(
      "protocol_mismatch",
      `lineage RPC response protocol \`${value.protocol}\` does not match \`${RPC_PROTOCOL}\``,
    );
  }
  if (typeof value.request_id !== "string") {
    throw responseError("invalid_envelope", "lineage RPC response request_id must be a string");
  }
  if (value.request_id !== requestId) {
    throw responseError(
      "request_id_mismatch",
      `lineage RPC response request_id \`${value.request_id}\` does not match \`${requestId}\``,
    );
  }
  if (typeof value.method !== "string") {
    throw responseError("invalid_envelope", "lineage RPC response method must be a string");
  }
  if (value.method !== method) {
    throw responseError(
      "method_mismatch",
      `lineage RPC response method \`${value.method}\` does not match \`${method}\``,
    );
  }
  if (value.session_id !== null && typeof value.session_id !== "string") {
    throw responseError(
      "invalid_envelope",
      "lineage RPC response session_id must be a string or null",
    );
  }
  if (typeof value.ok !== "boolean") {
    throw responseError("invalid_envelope", "lineage RPC response ok must be a boolean");
  }

  const responseSession = value.session_id as string | null;
  if (startsSession) {
    const validStartingSession = value.ok
      ? typeof responseSession === "string" && responseSession.length > 0
      : responseSession === null;
    if (!validStartingSession) {
      throw responseError(
        "session_id_mismatch",
        "lineage RPC session-start response has an invalid correlated session_id",
      );
    }
  } else if (responseSession !== requestSession || (value.ok && responseSession === null)) {
    throw responseError(
      "session_id_mismatch",
      "lineage RPC response session_id does not match the targeted session",
    );
  }

  const requiredKeys = value.ok
    ? ["method", "ok", "protocol", "request_id", "result", "session_id"]
    : ["error", "method", "ok", "protocol", "request_id", "session_id"];
  if (!hasExactKeys(value, requiredKeys)) {
    throw responseError(
      "invalid_envelope",
      value.ok
        ? "successful lineage RPC response must contain exactly one result"
        : "failed lineage RPC response must contain exactly one error",
    );
  }
  if (value.ok) {
    const result = decodeRpcResult(
      method,
      params,
      value.result,
      responseSession,
      requestIdentity,
      requestPolicy,
      requestLastAccepted,
      requestHistory,
    );
    return {
      protocol: RPC_PROTOCOL,
      request_id: requestId,
      method,
      session_id: responseSession as LineageSessionId,
      ok: true,
      result,
    } as unknown as RpcResponse<Method>;
  }
  if (
    !isJsonObject(value.error) ||
    !hasExactKeys(value.error, ["code", "message"]) ||
    typeof value.error.code !== "string" ||
    !/^[a-z][a-z0-9_]*$/u.test(value.error.code) ||
    typeof value.error.message !== "string" ||
    value.error.message.length === 0
  ) {
    throw responseError(
      "invalid_envelope",
      "lineage RPC error must contain only a stable code and non-empty message",
    );
  }
  if (!rpcErrorCodeSet.has(value.error.code)) {
    throw responseError(
      "unknown_error_code",
      `lineage RPC returned unknown error code \`${value.error.code}\``,
    );
  }
  return {
    protocol: RPC_PROTOCOL,
    request_id: requestId,
    method,
    session_id: responseSession as LineageSessionId | null,
    ok: false,
    error: {
      code: value.error.code as RpcErrorCode,
      message: value.error.message,
    },
  } as unknown as RpcResponse<Method>;
}

const rpcErrorCodeSet: ReadonlySet<string> = new Set(rpcErrorCodes);
const evaluationFailureCodeSet: ReadonlySet<string> = new Set(evaluationFailureCodes);
const identityKeys = ["digest", "document", "revision"] as const;
const attemptKeys = [
  "diagnostic",
  "disposition",
  "external_inputs",
  "failed_steps",
  "materialization_digest",
  "policy",
  "target",
] as const;
const authorityKeys = ["external_inputs", "lineage", "materialization_digest"] as const;

function decodeRpcResult<Method extends RpcMethod>(
  method: Method,
  params: RpcParamsByMethod[Method],
  value: unknown,
  responseSession: string | null,
  requestIdentity: LineageIdentity | null,
  requestPolicy: LineageEvaluationPolicyKey | null,
  requestLastAccepted: LineageAcceptedAuthority | null,
  requestHistory: LineageClientHistoryState,
): RpcResultByMethod[Method] {
  let result: RpcResultByMethod[RpcMethod];
  switch (method) {
    case "create":
    case "load":
    case "import":
    case "inspect":
      result = decodeSnapshot(
        value,
        responseSession,
        method,
        params,
        requestIdentity,
        requestPolicy,
        requestLastAccepted,
        requestHistory,
      );
      break;
    case "mutate":
    case "rewrite_owners":
    case "set_policy":
      result = decodeMutationResult(
        value,
        responseSession,
        method,
        params,
        requestIdentity,
        requestPolicy,
      );
      break;
    case "reconcile":
      result = decodeIdentityResult(
        value,
        responseSession,
        params as RpcParamsByMethod["reconcile"],
        requestIdentity,
      );
      break;
    case "undo":
    case "redo":
      result = decodeSnapshot(
        value,
        responseSession,
        method,
        params,
        requestIdentity,
        requestPolicy,
        requestLastAccepted,
        requestHistory,
      );
      break;
    case "evaluate":
      result = decodeEvaluationResult(
        value,
        responseSession,
        (params as RpcParamsByMethod["evaluate"]).expected,
        requestIdentity,
        requestPolicy,
        requestLastAccepted,
      );
      break;
    case "export":
      result = decodeSessionExport(value, responseSession, requestIdentity);
      break;
    case "export_lineage":
      result = decodeDocumentExport(value, responseSession, requestIdentity);
      break;
  }
  return result as RpcResultByMethod[Method];
}

function decodeSnapshot(
  value: unknown,
  responseSession: string | null,
  method: RpcMethod,
  params: RpcParamsByMethod[RpcMethod],
  requestIdentity: LineageIdentity | null,
  requestPolicy: LineageEvaluationPolicyKey | null,
  requestLastAccepted: LineageAcceptedAuthority | null,
  requestHistory: LineageClientHistoryState,
): LineageSnapshot {
  const object = exactResultObject(value, [
    "can_redo",
    "can_undo",
    "document",
    "evaluation_policy",
    "identity",
    "last_accepted",
    "last_accepted_document",
    "latest_attempt",
    "lifecycle",
    "redo_length",
    "session_id",
    "undo_length",
  ]);
  const identity = decodeIdentity(object.identity);
  const sessionId = requiredString(object.session_id, "snapshot session_id");
  if (sessionId !== responseSession || sessionId !== `lineage:${identity.document}`) {
    invalidResult("lineage RPC snapshot session_id does not match its envelope and document");
  }
  const policy = evaluationPolicy(object.evaluation_policy);
  const lifecycle = exactNestedObject(object.lifecycle, [
    "next_output_id",
    "next_reservation_id",
    "next_step_id",
    "revision",
  ], "snapshot lifecycle");
  const document = requiredJsonObject(object.document, "snapshot document");
  const decodedLifecycle: LineageLifecycleSnapshot = {
    revision: revisionId(lifecycle.revision, "snapshot lifecycle revision"),
    next_step_id: stepId(lifecycle.next_step_id, "snapshot next_step_id"),
    next_output_id: outputId(lifecycle.next_output_id, "snapshot next_output_id"),
    next_reservation_id: reservationId(
      lifecycle.next_reservation_id,
      "snapshot next_reservation_id",
    ),
  };
  validateLineageDocumentHeader(document, identity, policy, decodedLifecycle, "snapshot document");
  const lastAcceptedDocument = object.last_accepted_document === null
    ? null
    : requiredJsonObject(object.last_accepted_document, "snapshot last_accepted_document");
  const latestAttempt = nullableAttempt(object.latest_attempt);
  const lastAccepted = nullableAuthority(object.last_accepted);
  if ((lastAcceptedDocument === null) !== (lastAccepted === null)) {
    invalidResult(
      "lineage RPC snapshot accepted document and authority must be present together",
    );
  }
  if (latestAttempt !== null && latestAttempt.target.document !== identity.document) {
    invalidResult("lineage RPC snapshot attempt belongs to a different document");
  }
  if (latestAttempt !== null && latestAttempt.policy !== policy) {
    invalidResult("lineage RPC snapshot attempt policy does not match its retained document");
  }
  if (lastAccepted !== null && lastAccepted.lineage.document !== identity.document) {
    invalidResult("lineage RPC snapshot accepted authority belongs to a different document");
  }
  if (lastAccepted !== null && lastAcceptedDocument !== null) {
    validateLineageDocumentHeader(
      lastAcceptedDocument,
      lastAccepted.lineage,
      null,
      decodedLifecycle,
      "snapshot last_accepted_document",
    );
  }
  if (
    latestAttempt !== null &&
    (latestAttempt.disposition === "pending" ||
      latestAttempt.disposition === "accepted" ||
      latestAttempt.disposition === "failed") &&
    !sameIdentity(latestAttempt.target, identity)
  ) {
    invalidResult("lineage RPC snapshot current attempt does not target its retained identity");
  }
  if (latestAttempt?.disposition === "accepted") {
    if (
      lastAccepted === null ||
      !sameIdentity(lastAccepted.lineage, identity) ||
      latestAttempt.external_inputs !== lastAccepted.external_inputs ||
      latestAttempt.materialization_digest !== lastAccepted.materialization_digest
    ) {
      invalidResult("lineage RPC snapshot accepted attempt does not match accepted authority");
    }
  }
  const canUndo = requiredBoolean(object.can_undo, "snapshot can_undo");
  const canRedo = requiredBoolean(object.can_redo, "snapshot can_redo");
  const undoLength = decimalString(object.undo_length, "snapshot undo_length");
  const redoLength = decimalString(object.redo_length, "snapshot redo_length");
  if (canUndo !== (undoLength !== "0") || canRedo !== (redoLength !== "0")) {
    invalidResult("lineage RPC snapshot history booleans disagree with their lengths");
  }
  if (
    BigInt(undoLength) > BigInt(RPC_MAX_HISTORY_ITEMS) ||
    BigInt(redoLength) > BigInt(RPC_MAX_HISTORY_ITEMS)
  ) {
    invalidResult("lineage RPC snapshot history length exceeds the Rust bound");
  }
  const snapshot: LineageSnapshot = {
    session_id: sessionId as LineageSessionId,
    identity,
    evaluation_policy: policy,
    can_undo: canUndo,
    can_redo: canRedo,
    undo_length: undoLength,
    redo_length: redoLength,
    lifecycle: decodedLifecycle,
    latest_attempt: latestAttempt,
    last_accepted: lastAccepted,
    last_accepted_document: lastAcceptedDocument,
    document,
  };
  validateSnapshotRequestCorrelation(
    method,
    params,
    snapshot,
    requestIdentity,
    requestPolicy,
    requestLastAccepted,
    requestHistory,
  );
  return snapshot;
}

function decodeMutationResult(
  value: unknown,
  responseSession: string | null,
  method: RpcMethod,
  params: RpcParamsByMethod[RpcMethod],
  requestIdentity: LineageIdentity | null,
  requestPolicy: LineageEvaluationPolicyKey | null,
): LineageMutationResult {
  const object = exactResultObject(value, [
    "changed",
    "identity",
    "inserted_steps",
    "tombstoned_steps",
  ]);
  const identity = decodeIdentity(object.identity);
  requireIdentitySession(identity, responseSession, "mutation identity");
  const changed = requiredBoolean(object.changed, "mutation changed");
  const insertedSteps = stepIdArray(object.inserted_steps, "mutation inserted_steps");
  const tombstonedSteps = stepIdArray(object.tombstoned_steps, "mutation tombstoned_steps");
  requireUniqueStrings(insertedSteps, "mutation inserted_steps");
  requireUniqueStrings(tombstonedSteps, "mutation tombstoned_steps");
  const expected = method === "set_policy"
    ? (params as RpcParamsByMethod["set_policy"]).expected
    : (params as RpcParamsByMethod["mutate"]).patch.expected;
  requireCurrentRequestIdentity(expected, requestIdentity, `${method} expected identity`);
  requireMutationIdentityTransition(identity, expected, changed);
  if (!changed && (insertedSteps.length !== 0 || tombstonedSteps.length !== 0)) {
    invalidResult("unchanged lineage RPC mutation reports changed step evidence");
  }
  if (
    (method === "rewrite_owners" || method === "set_policy") &&
    (insertedSteps.length !== 0 || tombstonedSteps.length !== 0)
  ) {
    invalidResult(`${method} lineage RPC result reports impossible lifecycle changes`);
  }
  if (method === "mutate") {
    const patch = (params as RpcParamsByMethod["mutate"]).patch;
    const requestedPolicy = requestedMutationPolicy(patch, requestPolicy);
    if (!changed && requestedPolicy !== requestPolicy) {
      invalidResult(
        "unchanged lineage RPC mutation policy evidence disagrees with the retained policy",
      );
    }
    const expectedInserted = patch.mutations.flatMap((mutation) =>
      mutation.kind === "insert" ? [mutation.step.id] : []
    );
    if (!sameStringSet(insertedSteps, expectedInserted)) {
      invalidResult("lineage RPC mutation inserted-step evidence does not match its request");
    }
    const directTombstones = new Set(
      patch.mutations.flatMap((mutation) =>
        mutation.kind === "tombstone" ? [mutation.step] : []
      ),
    );
    const hasSubtreeDeletion = patch.mutations.some(
      (mutation) => mutation.kind === "delete_subtree",
    );
    if (
      !hasSubtreeDeletion &&
      tombstonedSteps.some((step) => !directTombstones.has(step))
    ) {
      invalidResult("lineage RPC mutation tombstone evidence does not match its request");
    }
    const deletionOnly = patch.mutations.every(
      (mutation) => mutation.kind === "tombstone" || mutation.kind === "delete_subtree",
    );
    if (changed && deletionOnly && tombstonedSteps.length === 0) {
      invalidResult("changed deletion-only mutation reports no tombstoned step");
    }
    const inserted = new Set(expectedInserted);
    const definitelyTombstoned = patch.mutations.flatMap((mutation) => {
      if (mutation.kind === "tombstone" && inserted.has(mutation.step)) {
        return [mutation.step];
      }
      if (mutation.kind === "delete_subtree" && inserted.has(mutation.root)) {
        return [mutation.root];
      }
      return [];
    });
    if (definitelyTombstoned.some((step) => !tombstonedSteps.includes(step))) {
      invalidResult("newly inserted deletion target is missing from mutation tombstone evidence");
    }
  }
  if (method === "set_policy" && requestPolicy !== null) {
    const requested = (params as RpcParamsByMethod["set_policy"]).policy;
    if (changed !== (requested !== requestPolicy)) {
      invalidResult("lineage RPC set_policy change evidence disagrees with the retained policy");
    }
  }
  return {
    identity,
    changed,
    inserted_steps: insertedSteps,
    tombstoned_steps: tombstonedSteps,
  };
}

function decodeIdentityResult(
  value: unknown,
  responseSession: string | null,
  params: RpcParamsByMethod["reconcile"],
  requestIdentity: LineageIdentity | null,
): LineageIdentityResult {
  const object = exactResultObject(value, ["identity"]);
  const identity = decodeIdentity(object.identity);
  requireIdentitySession(identity, responseSession, "result identity");
  const { expected } = params;
  requireCurrentRequestIdentity(expected, requestIdentity, "reconcile expected identity");
  if (identity.document !== expected.document || !hexIncremented(identity.revision, expected.revision)) {
    invalidResult("reconcile lineage RPC result does not advance exactly one document revision");
  }
  const replacement = decodeLineageDocumentJsonHeader(
    params.lineage_json,
    "reconcile lineage_json",
  );
  if (!sameIdentity(identity, replacement.identity)) {
    invalidResult("reconcile lineage RPC result does not match its replacement document");
  }
  return { identity };
}

function decodeEvaluationResult(
  value: unknown,
  responseSession: string | null,
  expected: LineageIdentity,
  requestIdentity: LineageIdentity | null,
  requestPolicy: LineageEvaluationPolicyKey | null,
  requestLastAccepted: LineageAcceptedAuthority | null,
): LineageEvaluationResult {
  const object = exactResultObject(value, [
    "attempt",
    "disposition",
    "evidence",
    "identity",
    "last_accepted",
    "steps",
  ]);
  const identity = decodeIdentity(object.identity);
  requireIdentitySession(identity, responseSession, "evaluation identity");
  requireCurrentRequestIdentity(expected, requestIdentity, "evaluation expected identity");
  if (!sameIdentity(identity, expected)) {
    invalidResult("lineage RPC evaluation identity does not match the requested exact identity");
  }
  const attempt = decodeAttempt(object.attempt);
  if (!sameIdentity(attempt.target, identity)) {
    invalidResult("lineage RPC evaluation attempt does not target the returned identity");
  }
  if (requestPolicy !== null && attempt.policy !== requestPolicy) {
    invalidResult("lineage RPC evaluation attempt does not use the retained evaluation policy");
  }
  const steps = requiredBoundedArray(object.steps, "evaluation steps").map((entry) => {
    const step = exactNestedObject(entry, ["dependencies", "state", "step"], "evaluation step");
    const state = evaluationStepState(step.state);
    const dependencies = stepIdArray(step.dependencies, "evaluation step dependencies");
    if (state !== "blocked" && dependencies.length !== 0) {
      invalidResult("only a blocked lineage evaluation step may name dependencies");
    }
    if (state === "blocked" && dependencies.length === 0) {
      invalidResult("a blocked lineage evaluation step must name at least one dependency");
    }
    return {
      step: stepId(step.step, "evaluation step identity"),
      state,
      dependencies,
    };
  });
  const stepIds = new Set(steps.map((step) => step.step));
  if (stepIds.size !== steps.length) invalidResult("lineage RPC evaluation plan repeats a step");
  for (const step of steps) {
    if (new Set(step.dependencies).size !== step.dependencies.length) {
      invalidResult("lineage RPC evaluation plan contains duplicate blocked dependencies");
    }
    if (step.dependencies.some((dependency) => !stepIds.has(dependency))) {
      invalidResult("lineage RPC evaluation plan names an unknown blocked dependency");
    }
  }
  validateEvaluationPlan(steps, attempt.policy);
  if (attempt.failed_steps.some((step) => !stepIds.has(step))) {
    invalidResult("lineage RPC evaluation attempt names a step outside its returned plan");
  }
  const lastAccepted = nullableAuthority(object.last_accepted);
  if (lastAccepted !== null && lastAccepted.lineage.document !== identity.document) {
    invalidResult("lineage RPC evaluation accepted authority belongs to a different document");
  }
  if (object.disposition === "accepted") {
    if (attempt.disposition !== "accepted" || lastAccepted === null) {
      invalidResult("accepted lineage RPC evaluation lacks matching accepted authority");
    }
    if (!sameIdentity(lastAccepted.lineage, identity)) {
      invalidResult("accepted lineage RPC authority does not match the evaluated identity");
    }
    const evidence = decodeAcceptedEvidence(object.evidence);
    if (
      steps.some(
        (step) =>
          step.state === "failed" ||
          step.state === "blocked" ||
          step.state === "unevaluated",
      )
    ) {
      invalidResult("accepted lineage RPC evaluation contains a non-accepted plan state");
    }
    const readyStepCount = steps.filter((step) => step.state === "ready").length;
    if (readyStepCount === 0) {
      invalidResult("accepted lineage RPC evaluation contains no validated ready step");
    }
    if (evidence.validated_prefix_count !== readyStepCount) {
      invalidResult(
        "accepted lineage RPC evidence prefix count does not match its ready plan steps",
      );
    }
    if (
      attempt.external_inputs !== evidence.external_inputs ||
      attempt.materialization_digest !== evidence.materialization_digest ||
      lastAccepted.external_inputs !== evidence.external_inputs ||
      lastAccepted.materialization_digest !== evidence.materialization_digest
    ) {
      invalidResult(
        "accepted lineage RPC evaluation evidence, attempt, and authority do not match",
      );
    }
    return {
      identity,
      disposition: "accepted",
      evidence,
      steps,
      attempt,
      last_accepted: lastAccepted,
    };
  }
  if (object.disposition === "failed") {
    if (attempt.disposition !== "failed") {
      invalidResult("failed lineage RPC evaluation lacks a matching failed attempt");
    }
    if (attempt.external_inputs === null) {
      invalidResult("failed lineage RPC evaluation does not identify its exact host inputs");
    }
    if (!sameNullableAuthority(lastAccepted, requestLastAccepted)) {
      invalidResult("failed lineage RPC evaluation changed prior accepted authority");
    }
    const evidence = decodeFailedEvidence(object.evidence);
    if (evidence.code === "dependency_blocked") {
      const blockedSteps = steps
        .filter((step) => step.state === "blocked")
        .map((step) => step.step);
      if (!sameStringSet(blockedSteps, attempt.failed_steps)) {
        invalidResult(
          "dependency-blocked lineage RPC evidence does not match the blocked attempt steps",
        );
      }
    } else {
      if (steps.some((step) => step.state === "blocked" || step.state === "unevaluated")) {
        invalidResult("domain-failed lineage RPC evaluation contains a blocked plan state");
      }
      if (attempt.failed_steps.length !== 1) {
        invalidResult("domain-failed lineage RPC evaluation must identify one failed step");
      }
      const failedStep = attempt.failed_steps[0];
      if (failedStep === undefined) {
        invalidResult("domain-failed lineage RPC evaluation is missing its failed step");
      }
      if (
        evidence.failed_step !== null &&
        (evidence.failed_step !== failedStep || !stepIds.has(evidence.failed_step))
      ) {
        invalidResult("lineage RPC failed evidence does not match the failed attempt step");
      }
      if (!steps.some((step) => step.step === failedStep && step.state === "ready")) {
        invalidResult("domain-failed lineage RPC evidence does not identify a ready plan step");
      }
    }
    return {
      identity,
      disposition: "failed",
      evidence,
      steps,
      attempt,
      last_accepted: lastAccepted,
    };
  }
  invalidResult("lineage RPC evaluation disposition must be accepted or failed");
}

function decodeAcceptedEvidence(value: unknown): LineageAcceptedEvaluationEvidence {
  const object = exactNestedObject(value, [
    "computed_feature_count",
    "evaluator",
    "external_inputs",
    "feature_digest",
    "materialization_digest",
    "sketch_digest",
    "strict_prefix_digest",
    "validated_prefix_count",
  ], "accepted evaluation evidence");
  if (object.evaluator !== "geosolve.constraint-editor.lineage-cold.v1") {
    invalidResult("lineage RPC accepted evidence has an unknown evaluator");
  }
  return {
    evaluator: object.evaluator,
    external_inputs: requiredString(object.external_inputs, "accepted external_inputs"),
    materialization_digest: digestId(
      object.materialization_digest,
      "accepted materialization_digest",
    ),
    sketch_digest: digestId(object.sketch_digest, "accepted sketch_digest"),
    feature_digest: digestId(object.feature_digest, "accepted feature_digest"),
    computed_feature_count: nonnegativeInteger(
      object.computed_feature_count,
      "accepted computed_feature_count",
    ),
    validated_prefix_count: nonnegativeInteger(
      object.validated_prefix_count,
      "accepted validated_prefix_count",
    ),
    strict_prefix_digest: digestId(object.strict_prefix_digest, "accepted strict_prefix_digest"),
  };
}

function decodeFailedEvidence(value: unknown): LineageFailedEvaluationEvidence {
  if (!isJsonObject(value)) invalidResult("failed evaluation evidence must be an object");
  const withStep = Object.hasOwn(value, "failed_step");
  const expected = withStep
    ? ["code", "evaluator", "failed_features", "failed_step", "message"]
    : ["code", "evaluator", "failed_features", "message"];
  if (!hasExactKeys(value, expected)) {
    invalidResult("failed evaluation evidence contains an unknown or missing field");
  }
  if (value.evaluator !== "geosolve.constraint-editor.lineage-cold.v1") {
    invalidResult("lineage RPC failed evidence has an unknown evaluator");
  }
  const code = requiredString(value.code, "failed evaluation code");
  if (!evaluationFailureCodeSet.has(code)) {
    invalidResult(`lineage RPC returned unknown evaluation failure code \`${code}\``);
  }
  if ((code === "dependency_blocked") === withStep) {
    invalidResult(
      code === "dependency_blocked"
        ? "dependency-blocked evaluation evidence must not contain a failed_step"
        : "domain evaluation evidence must contain its optional failed_step field",
    );
  }
  const base: Omit<LineageFailedEvaluationEvidence, "failed_step"> = {
    evaluator: "geosolve.constraint-editor.lineage-cold.v1",
    code: code as LineageEvaluationFailureCode,
    message: requiredString(value.message, "failed evaluation message"),
    failed_features: stringArray(value.failed_features, "failed evaluation features"),
  };
  if (!withStep) return base;
  return {
    ...base,
    failed_step: value.failed_step === null
      ? null
      : stepId(value.failed_step, "failed evaluation step"),
  };
}

function decodeSessionExport(
  value: unknown,
  responseSession: string | null,
  requestIdentity: LineageIdentity | null,
): LineageSessionExportResult {
  const object = exactResultObject(value, ["identity", "session_json"]);
  const identity = decodeIdentity(object.identity);
  requireIdentitySession(identity, responseSession, "session export identity");
  requireCurrentRequestIdentity(identity, requestIdentity, "session export identity");
  const sessionJson = requiredString(object.session_json, "session_json");
  const exportedIdentity = decodeLineageSessionJsonCurrentIdentity(sessionJson, "session_json");
  if (!sameIdentity(identity, exportedIdentity)) {
    invalidResult("lineage RPC session export payload does not match its result identity");
  }
  return {
    identity,
    session_json: sessionJson,
  };
}

function decodeDocumentExport(
  value: unknown,
  responseSession: string | null,
  requestIdentity: LineageIdentity | null,
): LineageDocumentExportResult {
  const object = exactResultObject(value, ["identity", "lineage_json"]);
  const identity = decodeIdentity(object.identity);
  requireIdentitySession(identity, responseSession, "lineage export identity");
  requireCurrentRequestIdentity(identity, requestIdentity, "lineage export identity");
  const lineageJson = requiredString(object.lineage_json, "lineage_json");
  const exported = decodeLineageDocumentJsonHeader(lineageJson, "lineage_json");
  if (!sameIdentity(identity, exported.identity)) {
    invalidResult("lineage RPC document export payload does not match its result identity");
  }
  return {
    identity,
    lineage_json: lineageJson,
  };
}

function decodeIdentity(value: unknown): LineageIdentity {
  const object = exactNestedObject(value, identityKeys, "lineage identity");
  return {
    document: documentId(object.document, "lineage document identity"),
    revision: revisionId(object.revision, "lineage revision"),
    digest: digestId(object.digest, "lineage digest"),
  };
}

interface DecodedLineageDocumentHeader {
  readonly identity: LineageIdentity;
  readonly evaluation_policy: LineageEvaluationPolicyKey;
  readonly next_step_id: LineageStepId;
  readonly next_output_id: LineageOutputId;
  readonly next_reservation_id: LineageReservationId;
}

interface DecodedLineageSessionHeader {
  readonly current: DecodedLineageDocumentHeader;
  readonly lifecycle: LineageLifecycleSnapshot;
  readonly undo: readonly LineageClientCorrelationState[];
  readonly redo: readonly LineageClientCorrelationState[];
}

function decodeLineageDocumentHeader(
  value: unknown,
  label: string,
): DecodedLineageDocumentHeader {
  const object = exactNestedObject(value, [
    "digest",
    "document_id",
    "evaluation_policy",
    "next_output_id",
    "next_reservation_id",
    "next_step_id",
    "revision",
    "steps",
    "version",
  ], label);
  if (object.version !== 1) invalidResult(`${label} has an unsupported version`);
  const steps = requiredBoundedArray(object.steps, `${label} steps`);
  if (!isJsonValue(steps)) invalidResult(`${label} steps must contain finite JSON values`);
  return {
    identity: {
      document: documentId(object.document_id, `${label} document_id`),
      revision: revisionId(object.revision, `${label} revision`),
      digest: digestId(object.digest, `${label} digest`),
    },
    evaluation_policy: evaluationPolicy(object.evaluation_policy),
    next_step_id: stepId(object.next_step_id, `${label} next_step_id`),
    next_output_id: outputId(object.next_output_id, `${label} next_output_id`),
    next_reservation_id: reservationId(
      object.next_reservation_id,
      `${label} next_reservation_id`,
    ),
  };
}

function decodeLineageDocumentJsonHeader(
  value: unknown,
  label: string,
): DecodedLineageDocumentHeader {
  return decodeLineageDocumentHeader(parseJsonObjectString(value, label), label);
}

function decodeLineageSessionJsonHeader(
  value: unknown,
  label: string,
): DecodedLineageSessionHeader {
  const object = exactNestedObject(parseJsonObjectString(value, label), [
    "auxiliary_high_waters",
    "digest",
    "document",
    "last_accepted",
    "last_accepted_document",
    "latest_attempt",
    "lifecycle",
    "redo",
    "undo",
    "version",
  ], label);
  if (object.version !== 1) invalidResult(`${label} has an unsupported session version`);
  digestId(object.digest, `${label} digest`);
  requiredJsonObject(object.auxiliary_high_waters, `${label} auxiliary_high_waters`);
  const undo = requiredBoundedArray(object.undo, `${label} undo`, RPC_MAX_HISTORY_ITEMS);
  const redo = requiredBoundedArray(object.redo, `${label} redo`, RPC_MAX_HISTORY_ITEMS);
  const current = decodeLineageDocumentJsonHeader(object.document, `${label} document`);
  const lifecycle = exactNestedObject(
    object.lifecycle,
    ["allocator", "revision"],
    `${label} lifecycle`,
  );
  const allocator = exactNestedObject(
    lifecycle.allocator,
    ["next_output_id", "next_reservation_id", "next_step_id"],
    `${label} lifecycle allocator`,
  );
  const decodedLifecycle: LineageLifecycleSnapshot = {
    revision: revisionId(lifecycle.revision, `${label} lifecycle revision`),
    next_step_id: stepId(allocator.next_step_id, `${label} lifecycle next_step_id`),
    next_output_id: outputId(allocator.next_output_id, `${label} lifecycle next_output_id`),
    next_reservation_id: reservationId(
      allocator.next_reservation_id,
      `${label} lifecycle next_reservation_id`,
    ),
  };
  validateHeaderLifecycle(current, decodedLifecycle, `${label} document`);
  const decodedUndo = undo.map((checkpoint, index) =>
    decodeLoadedCheckpointState(
      checkpoint,
      current.identity.document,
      decodedLifecycle,
      `${label} undo[${index}]`,
    )
  );
  const decodedRedo = redo.map((checkpoint, index) =>
    decodeLoadedCheckpointState(
      checkpoint,
      current.identity.document,
      decodedLifecycle,
      `${label} redo[${index}]`,
    )
  );
  return {
    current,
    lifecycle: decodedLifecycle,
    undo: decodedUndo,
    redo: decodedRedo,
  };
}

function decodeLoadedCheckpointState(
  value: unknown,
  expectedDocument: LineageDocumentId,
  lifecycle: LineageLifecycleSnapshot,
  label: string,
): LineageClientCorrelationState {
  const checkpoint = exactNestedObject(value, [
    "document",
    "last_accepted",
    "last_accepted_document",
    "latest_attempt",
  ], label);
  const document = decodeLineageDocumentJsonHeader(
    checkpoint.document,
    `${label} document`,
  );
  if (document.identity.document !== expectedDocument) {
    invalidResult(`${label} belongs to another lineage document`);
  }
  validateHeaderLifecycle(document, lifecycle, `${label} document`);
  // Standalone RPC load deliberately strips all current and historical
  // evaluator authority. Only the declarative policy survives as a future
  // Undo/Redo correlation witness.
  return { policy: document.evaluation_policy, last_accepted: null };
}

function decodeLineageSessionJsonCurrentIdentity(value: unknown, label: string): LineageIdentity {
  return decodeLineageSessionJsonHeader(value, label).current.identity;
}

function validateLineageDocumentHeader(
  document: Readonly<Record<string, JsonValue>>,
  identity: LineageIdentity,
  policy: LineageEvaluationPolicyKey | null,
  lifecycle: LineageLifecycleSnapshot,
  label: string,
): void {
  const header = decodeLineageDocumentHeader(document, label);
  if (!sameIdentity(header.identity, identity)) {
    invalidResult(`${label} identity does not match its surrounding authority`);
  }
  if (policy !== null && header.evaluation_policy !== policy) {
    invalidResult(`${label} policy does not match its snapshot`);
  }
  validateHeaderLifecycle(header, lifecycle, label);
}

function validateHeaderLifecycle(
  header: DecodedLineageDocumentHeader,
  lifecycle: LineageLifecycleSnapshot,
  label: string,
): void {
  if (
    !hexAtLeast(lifecycle.revision, header.identity.revision) ||
    !hexAtLeast(lifecycle.next_step_id, header.next_step_id) ||
    !hexAtLeast(lifecycle.next_output_id, header.next_output_id) ||
    !hexAtLeast(lifecycle.next_reservation_id, header.next_reservation_id)
  ) {
    invalidResult(`${label} exceeds its session lifecycle high-water`);
  }
}

function validateSnapshotRequestCorrelation(
  method: RpcMethod,
  params: RpcParamsByMethod[RpcMethod],
  snapshot: LineageSnapshot,
  requestIdentity: LineageIdentity | null,
  requestPolicy: LineageEvaluationPolicyKey | null,
  requestLastAccepted: LineageAcceptedAuthority | null,
  requestHistory: LineageClientHistoryState,
): void {
  const { identity } = snapshot;
  if (method === "create") {
    const documentId = (params as RpcParamsByMethod["create"]).document_id;
    if (documentId !== undefined && identity.document !== documentId) {
      invalidResult("create snapshot does not use the requested document identity");
    }
    requireEmptySnapshotHistory(snapshot, "create");
    return;
  }
  if (method === "load") {
    const loaded = decodeLineageSessionJsonHeader(
      (params as RpcParamsByMethod["load"]).session_json,
      "load session_json",
    );
    if (!sameIdentity(identity, loaded.current.identity)) {
      invalidResult("load snapshot identity does not match the requested session payload");
    }
    if (
      snapshot.evaluation_policy !== loaded.current.evaluation_policy ||
      !sameLifecycle(snapshot.lifecycle, loaded.lifecycle) ||
      snapshot.undo_length !== String(loaded.undo.length) ||
      snapshot.redo_length !== String(loaded.redo.length)
    ) {
      invalidResult("load snapshot lifecycle does not match the requested session payload");
    }
    if (
      snapshot.latest_attempt === null ||
      snapshot.latest_attempt.disposition !== "pending" ||
      snapshot.latest_attempt.external_inputs !== null ||
      snapshot.last_accepted !== null ||
      snapshot.last_accepted_document !== null
    ) {
      invalidResult("load snapshot did not strip caller-supplied evaluation authority");
    }
    return;
  }
  if (method === "import") {
    const imported = decodeLineageDocumentJsonHeader(
      (params as RpcParamsByMethod["import"]).lineage_json,
      "import lineage_json",
    );
    if (!sameIdentity(identity, imported.identity)) {
      invalidResult("import snapshot identity does not match the requested lineage payload");
    }
    if (
      snapshot.evaluation_policy !== imported.evaluation_policy ||
      snapshot.lifecycle.revision !== imported.identity.revision ||
      snapshot.lifecycle.next_step_id !== imported.next_step_id ||
      snapshot.lifecycle.next_output_id !== imported.next_output_id ||
      snapshot.lifecycle.next_reservation_id !== imported.next_reservation_id ||
      snapshot.latest_attempt !== null ||
      snapshot.last_accepted !== null ||
      snapshot.last_accepted_document !== null
    ) {
      invalidResult("import snapshot contains state not present in the imported lineage document");
    }
    requireEmptySnapshotHistory(snapshot, "import");
    return;
  }
  if (method === "inspect") {
    requireCurrentRequestIdentity(identity, requestIdentity, "inspect snapshot identity");
    if (requestPolicy === null || snapshot.evaluation_policy !== requestPolicy) {
      invalidResult("inspect snapshot changed the retained evaluation policy");
    }
    if (!sameNullableAuthority(snapshot.last_accepted, requestLastAccepted)) {
      invalidResult("inspect snapshot changed retained accepted authority");
    }
    requireSnapshotHistoryLengths(snapshot, requestHistory.undo.length, requestHistory.redo.length);
    return;
  }
  if (method === "undo" || method === "redo") {
    const expected = (params as RpcParamsByMethod["undo"]).expected;
    requireCurrentRequestIdentity(expected, requestIdentity, `${method} expected identity`);
    if (
      identity.document !== expected.document ||
      !hexIncremented(identity.revision, expected.revision)
    ) {
      invalidResult(`${method} lineage RPC snapshot does not advance exactly one revision`);
    }
    if (
      snapshot.latest_attempt === null ||
      snapshot.latest_attempt.disposition !== "pending" ||
      snapshot.latest_attempt.external_inputs !== null ||
      !sameIdentity(snapshot.latest_attempt.target, identity)
    ) {
      invalidResult(`${method} lineage RPC snapshot is not pending at its restored identity`);
    }
    const restored = method === "undo"
      ? requestHistory.undo.at(-1)
      : requestHistory.redo.at(-1);
    if (restored === undefined) {
      invalidResult(`${method} lineage RPC snapshot has no matching retained history checkpoint`);
    }
    if (
      snapshot.evaluation_policy !== restored.policy ||
      !sameNullableAuthority(snapshot.last_accepted, restored.last_accepted)
    ) {
      invalidResult(`${method} lineage RPC snapshot does not match its retained history checkpoint`);
    }
    const undoLength = method === "undo"
      ? requestHistory.undo.length - 1
      : pushedHistoryLength(requestHistory.undo.length);
    const redoLength = method === "redo"
      ? requestHistory.redo.length - 1
      : pushedHistoryLength(requestHistory.redo.length);
    requireSnapshotHistoryLengths(snapshot, undoLength, redoLength);
    return;
  }
  invalidResult(`lineage RPC ${method} unexpectedly returned a snapshot`);
}

function pushedHistoryLength(length: number): number {
  return Math.min(RPC_MAX_HISTORY_ITEMS, length + 1);
}

function requireSnapshotHistoryLengths(
  snapshot: LineageSnapshot,
  undoLength: number,
  redoLength: number,
): void {
  if (
    snapshot.undo_length !== String(undoLength) ||
    snapshot.redo_length !== String(redoLength)
  ) {
    invalidResult("lineage RPC snapshot history does not match retained client history");
  }
}

function requireEmptySnapshotHistory(snapshot: LineageSnapshot, method: string): void {
  if (
    snapshot.can_undo ||
    snapshot.can_redo ||
    snapshot.undo_length !== "0" ||
    snapshot.redo_length !== "0"
  ) {
    invalidResult(`${method} snapshot unexpectedly carries lineage history`);
  }
}

function sameLifecycle(
  left: LineageLifecycleSnapshot,
  right: LineageLifecycleSnapshot,
): boolean {
  return left.revision === right.revision &&
    left.next_step_id === right.next_step_id &&
    left.next_output_id === right.next_output_id &&
    left.next_reservation_id === right.next_reservation_id;
}

function decodeAttempt(value: unknown): LineageEvaluationAttempt {
  const object = exactNestedObject(value, attemptKeys, "lineage evaluation attempt");
  const externalInputs = nullableString(object.external_inputs, "attempt external_inputs");
  const materializationDigest = object.materialization_digest === null
    ? null
    : digestId(object.materialization_digest, "attempt materialization_digest");
  const diagnostic = nullableString(object.diagnostic, "attempt diagnostic");
  const disposition = evaluationDisposition(object.disposition);
  const failedSteps = stepIdArray(object.failed_steps, "attempt failed_steps");
  if (new Set(failedSteps).size !== failedSteps.length) {
    invalidResult("lineage evaluation attempt contains duplicate failed steps");
  }
  switch (disposition) {
    case "pending":
      if (
        externalInputs !== null ||
        materializationDigest !== null ||
        failedSteps.length !== 0 ||
        diagnostic !== null
      ) {
        invalidResult("pending lineage evaluation attempt carries result evidence");
      }
      break;
    case "accepted":
      if (materializationDigest === null || failedSteps.length !== 0 || diagnostic !== null) {
        invalidResult("accepted lineage evaluation attempt carries inconsistent evidence");
      }
      break;
    case "failed":
      if (materializationDigest !== null || failedSteps.length === 0 || diagnostic === null) {
        invalidResult("failed lineage evaluation attempt carries inconsistent evidence");
      }
      break;
    case "cancelled":
    case "exhausted":
    case "stale":
      if (materializationDigest !== null || failedSteps.length !== 0 || diagnostic === null) {
        invalidResult("nonpublishing lineage evaluation attempt carries inconsistent evidence");
      }
      break;
  }
  return {
    target: decodeIdentity(object.target),
    policy: evaluationPolicy(object.policy),
    disposition,
    external_inputs: externalInputs,
    materialization_digest: materializationDigest,
    failed_steps: failedSteps,
    diagnostic,
  };
}

function nullableAttempt(value: unknown): LineageEvaluationAttempt | null {
  return value === null ? null : decodeAttempt(value);
}

function decodeAuthority(value: unknown): LineageAcceptedAuthority {
  const object = exactNestedObject(value, authorityKeys, "lineage accepted authority");
  return {
    lineage: decodeIdentity(object.lineage),
    external_inputs: nullableString(object.external_inputs, "accepted external_inputs"),
    materialization_digest: digestId(
      object.materialization_digest,
      "accepted materialization_digest",
    ),
  };
}

function nullableAuthority(value: unknown): LineageAcceptedAuthority | null {
  return value === null ? null : decodeAuthority(value);
}

function cloneNullableAuthority(
  authority: LineageAcceptedAuthority | null,
): LineageAcceptedAuthority | null {
  return authority === null
    ? null
    : {
        lineage: { ...authority.lineage },
        external_inputs: authority.external_inputs,
        materialization_digest: authority.materialization_digest,
      };
}

function exactResultObject(
  value: unknown,
  keys: readonly string[],
): Record<string, unknown> {
  return exactNestedObject(value, keys, "lineage RPC result");
}

function exactNestedObject(
  value: unknown,
  keys: readonly string[],
  label: string,
): Record<string, unknown> {
  if (!isJsonObject(value) || !hasExactKeys(value, keys)) {
    invalidResult(`${label} contains an unknown or missing field`);
  }
  return value;
}

function requiredJsonObject(
  value: unknown,
  label: string,
): Readonly<Record<string, JsonValue>> {
  if (!isJsonObject(value) || !isJsonValue(value)) {
    invalidResult(`${label} must be a finite JSON object`);
  }
  return value as Readonly<Record<string, JsonValue>>;
}

function requiredArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) invalidResult(`${label} must be an array`);
  return value;
}

function requiredBoundedArray(
  value: unknown,
  label: string,
  limit = RPC_MAX_RESULT_ITEMS,
): readonly unknown[] {
  const array = requiredArray(value, label);
  if (array.length > limit) invalidResult(`${label} exceeds the bounded result cardinality`);
  return array;
}

function requiredString(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0) {
    invalidResult(`${label} must be a non-empty string`);
  }
  return value;
}

function nullableString(value: unknown, label: string): string | null {
  return value === null ? null : requiredString(value, label);
}

function requiredBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") invalidResult(`${label} must be a boolean`);
  return value;
}

function decimalString(value: unknown, label: string): string {
  const string = requiredString(value, label);
  if (!/^(?:0|[1-9][0-9]*)$/u.test(string)) invalidResult(`${label} must be an unsigned decimal`);
  return string;
}

function fixedHex(value: unknown, digits: number, label: string): string {
  const string = requiredString(value, label);
  if (!new RegExp(`^[0-9a-f]{${digits}}$`, "u").test(string)) {
    invalidResult(`${label} must be ${digits} lowercase hexadecimal digits`);
  }
  return string;
}

function documentId(value: unknown, label: string): LineageDocumentId {
  return fixedHex(value, 32, label) as LineageDocumentId;
}

function revisionId(value: unknown, label: string): LineageRevision {
  return fixedHex(value, 16, label) as LineageRevision;
}

function stepId(value: unknown, label: string): LineageStepId {
  return fixedHex(value, 16, label) as LineageStepId;
}

function outputId(value: unknown, label: string): LineageOutputId {
  return fixedHex(value, 16, label) as LineageOutputId;
}

function reservationId(value: unknown, label: string): LineageReservationId {
  return fixedHex(value, 16, label) as LineageReservationId;
}

function digestId(value: unknown, label: string): LineageDigest {
  return fixedHex(value, 64, label) as LineageDigest;
}

function stepIdArray(value: unknown, label: string): readonly LineageStepId[] {
  return requiredBoundedArray(value, label).map((item, index) =>
    stepId(item, `${label}[${index}]`)
  );
}

function stringArray(value: unknown, label: string): readonly string[] {
  return requiredBoundedArray(value, label).map((item, index) =>
    requiredString(item, `${label}[${index}]`),
  );
}

function nonnegativeInteger(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    invalidResult(`${label} must be a non-negative safe integer`);
  }
  return value;
}

function evaluationPolicy(value: unknown): LineageEvaluationPolicyKey {
  if (value !== "strict_chronological" && value !== "dependency_local") {
    invalidResult("lineage evaluation policy is unknown");
  }
  return value;
}

function evaluationDisposition(value: unknown): LineageEvaluationDispositionKey {
  if (
    value !== "pending" &&
    value !== "accepted" &&
    value !== "failed" &&
    value !== "cancelled" &&
    value !== "exhausted" &&
    value !== "stale"
  ) {
    invalidResult("lineage evaluation disposition is unknown");
  }
  return value;
}

function evaluationStepState(value: unknown): LineageEvaluationStepState {
  if (
    value !== "ready" &&
    value !== "suppressed" &&
    value !== "tombstoned" &&
    value !== "failed" &&
    value !== "blocked" &&
    value !== "unevaluated"
  ) {
    invalidResult("lineage evaluation step state is unknown");
  }
  return value;
}

function validateEvaluationPlan(
  steps: readonly LineageEvaluationStep[],
  policy: LineageEvaluationPolicyKey,
): void {
  const positions = new Map(steps.map((step, index) => [step.step, index]));
  const states = new Map(steps.map((step) => [step.step, step.state]));
  let strictStopped = false;
  for (const [index, step] of steps.entries()) {
    if (step.state === "failed") {
      invalidResult("lineage RPC dependency_plan([]) cannot contain a failed step state");
    }
    if (step.state === "blocked") {
      if (policy === "strict_chronological" && strictStopped) {
        invalidResult("strict lineage RPC evaluation contains a second blocked live step");
      }
      for (const dependency of step.dependencies) {
        const dependencyPosition = positions.get(dependency);
        if (
          dependencyPosition === undefined ||
          dependencyPosition >= index ||
          states.get(dependency) === "ready"
        ) {
          invalidResult(
            "lineage RPC blocked step must name an earlier non-ready dependency",
          );
        }
      }
      if (policy === "strict_chronological") strictStopped = true;
      continue;
    }
    if (step.state === "unevaluated") {
      if (policy !== "strict_chronological" || !strictStopped) {
        invalidResult("lineage RPC unevaluated step is inconsistent with its evaluation policy");
      }
      continue;
    }
    if (policy === "strict_chronological" && strictStopped && step.state === "ready") {
      invalidResult("strict lineage RPC evaluation resumes after a blocked step");
    }
  }
}

function sameIdentity(left: LineageIdentity, right: LineageIdentity): boolean {
  return left.document === right.document && left.revision === right.revision && left.digest === right.digest;
}

function sameNullableAuthority(
  left: LineageAcceptedAuthority | null,
  right: LineageAcceptedAuthority | null,
): boolean {
  return left === null
    ? right === null
    : right !== null &&
      sameIdentity(left.lineage, right.lineage) &&
      left.external_inputs === right.external_inputs &&
      left.materialization_digest === right.materialization_digest;
}

function requireIdentitySession(
  identity: LineageIdentity,
  responseSession: string | null,
  label: string,
): void {
  if (responseSession !== `lineage:${identity.document}`) {
    invalidResult(`lineage RPC ${label} belongs to a different session document`);
  }
}

function requireCurrentRequestIdentity(
  identity: LineageIdentity,
  requestIdentity: LineageIdentity | null,
  label: string,
): void {
  if (requestIdentity === null || !sameIdentity(identity, requestIdentity)) {
    invalidResult(`lineage RPC ${label} does not match the retained client identity`);
  }
}

function requireMutationIdentityTransition(
  identity: LineageIdentity,
  expected: LineageIdentity,
  changed: boolean,
): void {
  if (identity.document !== expected.document) {
    invalidResult("lineage RPC mutation result belongs to another document");
  }
  if (changed) {
    if (!hexIncremented(identity.revision, expected.revision)) {
      invalidResult("changed lineage RPC mutation did not advance exactly one revision");
    }
  } else if (!sameIdentity(identity, expected)) {
    invalidResult("unchanged lineage RPC mutation changed its document identity");
  }
}

function hexAtLeast(left: string, right: string): boolean {
  return BigInt(`0x${left}`) >= BigInt(`0x${right}`);
}

function hexIncremented(next: string, previous: string): boolean {
  return BigInt(`0x${next}`) === BigInt(`0x${previous}`) + 1n;
}

function requireUniqueStrings(values: readonly string[], label: string): void {
  if (new Set(values).size !== values.length) {
    invalidResult(`${label} contains duplicate identities`);
  }
}

function sameStringSet(left: readonly string[], right: readonly string[]): boolean {
  if (left.length !== right.length) return false;
  const leftSet = new Set(left);
  const rightSet = new Set(right);
  return (
    leftSet.size === left.length &&
    rightSet.size === right.length &&
    leftSet.size === rightSet.size &&
    [...leftSet].every((value) => rightSet.has(value))
  );
}

function parseJsonObjectString(value: unknown, label: string): Record<string, unknown> {
  const string = requiredString(value, label);
  let parsed: unknown;
  try {
    parsed = JSON.parse(string) as unknown;
  } catch {
    invalidResult(`${label} must contain valid JSON`);
  }
  if (!isJsonObject(parsed) || !isJsonValue(parsed)) {
    invalidResult(`${label} must contain a finite JSON object`);
  }
  return parsed;
}

function invalidResult(message: string): never {
  throw responseError("invalid_result", message);
}

function responseError(
  violation: RpcResponseViolation,
  message: string,
): LineageRpcResponseError {
  return new LineageRpcResponseError(violation, message);
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isJsonValue(value: unknown): value is JsonValue {
  const pending: unknown[] = [value];
  while (pending.length > 0) {
    const item = pending.pop();
    if (
      item === null ||
      typeof item === "string" ||
      typeof item === "boolean" ||
      (typeof item === "number" && Number.isFinite(item))
    ) {
      continue;
    }
    if (Array.isArray(item)) {
      for (const child of item) pending.push(child);
      continue;
    }
    if (isJsonObject(item)) {
      for (const child of Object.values(item)) pending.push(child);
      continue;
    }
    return false;
  }
  return true;
}

export function developerKey(value: string): DeveloperKey {
  requireKey(value, "developer key");
  return value as DeveloperKey;
}

export function semanticKey(value: string): SemanticKey {
  requireKey(value, "semantic key");
  return value as SemanticKey;
}

export function persistentId(value: string): PersistentId {
  requireKey(value, "persistent ID");
  return value as PersistentId;
}

export function input<Kind extends OutputKind>(
  key: string,
  kind: Kind,
  source: OutputRef<Kind>,
): InputBinding<Kind> {
  return { key: semanticKey(key), kind, source };
}

function requireKey(value: string, label: string): void {
  if (value.length === 0 || value.trim() !== value || /[\u0000-\u001f\u007f]/u.test(value)) {
    throw new Error(`${label} is empty or contains disallowed characters`);
  }
}

function parseHex(value: string): bigint {
  if (!/^[0-9a-f]{16}$/u.test(value)) {
    throw new Error("lineage allocator ID must be sixteen lowercase hexadecimal digits");
  }
  return BigInt(`0x${value}`);
}

function hexId<Id extends string>(value: bigint): Id {
  if (value <= 0n || value > 0xffff_ffff_ffff_ffffn) {
    throw new Error("lineage identity allocator exhausted");
  }
  return value.toString(16).padStart(16, "0") as Id;
}
