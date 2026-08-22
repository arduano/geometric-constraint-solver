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
  readonly #mutations: JsonValue[] = [];

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
    this.#mutations.push({ kind: "insert", before: null, step: value as unknown as JsonValue });
    return Object.fromEntries(
      outputs.map((output) => [
        output.key,
        { document: this.#expected.document, step, output: output.id, kind: output.kind },
      ]),
    );
  }

  patch(): JsonValue {
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

export interface RpcResponse<Result = JsonValue> {
  readonly protocol: typeof RPC_PROTOCOL;
  readonly request_id: string | null;
  readonly method: string | null;
  readonly session_id: LineageSessionId | null;
  readonly ok: boolean;
  readonly result?: Result;
  readonly error?: { readonly code: string; readonly message: string };
}

export const RPC_PROTOCOL = "geosolve.lineage.rpc.v0" as const;

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

/** Thin typed transport wrapper; all validation remains in Rust. */
export class LineageRpcClient {
  readonly #transport: RpcTransport;
  #request = 0;
  #session: LineageSessionId | null = null;

  constructor(transport: RpcTransport) {
    this.#transport = transport;
  }

  call<Result extends JsonValue>(method: RpcMethod, params: JsonValue): RpcResponse<Result> {
    const requestId = String(++this.#request);
    const startsSession = method === "create" || method === "load" || method === "import";
    const response = JSON.parse(
      this.#transport.request(
        JSON.stringify({
          protocol: RPC_PROTOCOL,
          request_id: requestId,
          session_id: startsSession ? null : this.#session,
          method,
          params,
        }),
      ),
    ) as RpcResponse<Result>;
    if (response.ok && response.session_id !== null) {
      this.#session = response.session_id;
    }
    return response;
  }
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
