// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Equation-free TypeScript vocabulary for the Rust-owned GeoSolve Design
 * Intent Graph. This package builds and transports typed patches; it never
 * evaluates geometry, constraints, residuals, or expressions.
 *
 * The wire types in this module deliberately mirror `geosolve-sketch-intent`
 * serde JSON. Session brands and runtime ownership seals are non-enumerable,
 * so they cannot leak into the Rust request.
 */

declare const sessionBrand: unique symbol;
declare const nodeBrand: unique symbol;
declare const portBrand: unique symbol;
declare const cellBrand: unique symbol;
declare const leafBrand: unique symbol;
declare const operationBrand: unique symbol;

const runtimeSession = Symbol("geosolve.intent.session");
const runtimePortKind = Symbol("geosolve.intent.port-kind");

// These resource limits mirror the closed Rust intent model. Keep projection
// decoding large enough for every graph which Rust can validate; smaller UI
// convenience limits must never make a valid authoritative session unreadable
// to code-first clients.
const MAX_INTENT_GRAPH_NODES = 65_536;
const MAX_INTENT_PATCH_OPERATIONS = 16_384;
const MAX_INTENT_NODE_COMPONENTS = 4_096;
// An operation owns two fixed logical ports, at most one result plus one
// paired source per output slot, and at most u16::MAX curve-span ports across
// all slots. Other closed declaration families have smaller port catalogs.
const MAX_INTENT_DECLARATION_OUTPUTS = 2 + (2 * MAX_INTENT_NODE_COMPONENTS) + 0xffff;

export type GeometryRecipe =
  | "sketch_point"
  | "segment"
  | "polyline"
  | "midpoint_line"
  | "two_point_aligned_rectangle"
  | "three_point_corner_rectangle"
  | "center_rectangle"
  | "three_point_center_rectangle"
  | "center_radius_circle"
  | "two_point_diameter_circle"
  | "three_point_circle"
  | "center_arc"
  | "three_point_arc"
  | "tangent_arc"
  | "center_axes_ellipse"
  | "axis_endpoints_ellipse"
  | "center_axes_elliptical_arc"
  | "axis_endpoints_elliptical_arc"
  | "quadratic_bezier"
  | "cubic_bezier"
  | "rational_quadratic_conic"
  | "parabola"
  | "hyperbola"
  | "open_control_nurbs"
  | "periodic_control_nurbs";

export type ConstraintKind =
  | "fixed_point"
  | "fixed_coordinate"
  | "coincident_with_origin"
  | "point_on_datum_axis"
  | "coincident"
  | "external_point_coincident"
  | "horizontal"
  | "vertical"
  | "horizontal_points"
  | "vertical_points"
  | "horizontal_point_to_midpoint"
  | "vertical_point_to_midpoint"
  | "point_on_curve"
  | "parallel"
  | "perpendicular"
  | "external_line_collinear"
  | "collinear_with_datum_axis"
  | "concentric"
  | "collinear"
  | "equal_length"
  | "equal_radius"
  | "midpoint"
  | "symmetric_about_line"
  | "symmetric_about_datum_axis"
  | "line_circle_tangency"
  | "circle_circle_tangency"
  | "circle_arc_tangency"
  | "line_curve_tangency"
  | "curve_curve_contact"
  | "curve_curve_tangency"
  | "curve_direction"
  | "equal_curvature"
  | "endpoint_continuity"
  | "line_line_fillet"
  | "curve_curve_fillet";

export type DimensionKind =
  | "point_distance"
  | "curve_length"
  | "radius"
  | "diameter"
  | "oriented_angle"
  | "supporting_line_offset"
  | "exact_translated_segment_offset"
  | "profile_offset";

export type OperationKind =
  | "split"
  | "break"
  | "trim"
  | "extend"
  | "mirror"
  | "chamfer"
  | "associative_fillet"
  | "rectangle"
  | "regular_polygon"
  | "slot"
  | "linear_pattern"
  | "profile_offset";

export type IntentOperationOutputKind =
  | "point"
  | "scalar"
  | "curve"
  | "contact"
  | "constraint"
  | "dimension"
  | "parameter"
  | "external_binding";

export interface IntentOperationOutput {
  readonly kind: IntentOperationOutputKind;
  readonly curve_span_count: number;
}

export type IntentChildSchema =
  | "none"
  | "polyline_vertex"
  | "spline_control"
  | "fillet_corner"
  | "pattern_instance";

export type AggregateKind = "open_chain" | "closed_profile";

export type BootstrapNativeKind =
  | "document"
  | "point"
  | "scalar"
  | "curve"
  | "contact"
  | "constraint"
  | "dimension"
  | "parameter"
  | "external_binding"
  | "semantic_catalog"
  | "semantic_source"
  | "curve_trim_view"
  | "geometry_role"
  | "parameter_binding"
  | "parameter_output"
  | "computed_feature"
  | "annotation_placement";

export type PortKind =
  | "point"
  | "handle_point"
  | "scalar"
  | "curve"
  | "curve_span"
  | "contact"
  | "constraint"
  | "dimension"
  | "source"
  | "parameter"
  | "parameter_binding"
  | "parameter_output"
  | "external_binding"
  | "semantic_catalog"
  | "profile"
  | "chain"
  | "operation"
  | "feature"
  | "feature_corner"
  | "annotation"
  | "collection";

export type InputRole =
  | "point"
  | "contact"
  | "curve"
  | "span"
  | "scalar"
  | "constraint"
  | "dimension"
  | "feature"
  | "profile"
  | "chain"
  | "parameter"
  | "external"
  | "source"
  | "catalog"
  | "identity";

export type PortKindForRole<R extends InputRole> =
  R extends "point" ? "point" :
  R extends "contact" ? "contact" :
  R extends "curve" ? "curve" :
  R extends "span" ? "curve_span" :
  R extends "scalar" ? "scalar" :
  R extends "constraint" ? "constraint" :
  R extends "dimension" ? "dimension" :
  R extends "feature" ? "feature" :
  R extends "profile" ? "profile" :
  R extends "chain" ? "chain" :
  R extends "parameter" ? "parameter" :
  R extends "external" ? "external_binding" :
  R extends "source" ? "source" :
  R extends "catalog" ? "semantic_catalog" :
  PortKind;

export type PortRole =
  | "primary"
  | "start"
  | "end"
  | "center"
  | "midpoint"
  | "corner"
  | "control"
  | "major_axis_point"
  | "minor_axis_point"
  | "curve"
  | "span"
  | "contact"
  | "target"
  | "constraint"
  | "dimension"
  | "source"
  | "catalog"
  | "operation"
  | "feature"
  | "feature_corner"
  | "parameter"
  | "binding"
  | "output"
  | "external"
  | "annotation"
  | "collection"
  | "profile"
  | "chain"
  | "result";

export type LeafField = "x" | "y" | "value" | "angle" | "weight" | "parameter";
export type LeafFieldForKind<K extends PortKind> =
  K extends "point" | "handle_point" ? "x" | "y" :
  K extends "scalar" ? "value" | "angle" | "weight" | "parameter" :
  never;

export type InputSlot<R extends InputRole = InputRole> = `${R}:${string}`;
export type IntentPortSelector =
  | `node:${PortRole}:${string}`
  | `child:${string}:${PortRole}:${string}`;

export interface SessionRef<S extends string> {
  readonly id: S;
  readonly [sessionBrand]: S;
}

export interface NodeRef<S extends string> {
  readonly id: string;
  readonly [nodeBrand]: S;
}

export interface CellRef<S extends string> {
  readonly id: string;
  readonly [cellBrand]: S;
}

export interface StablePortRef<S extends string, K extends PortKind> {
  readonly source: "stable";
  readonly port: {
    readonly node: string;
    readonly port: string;
    readonly kind: K;
  };
  readonly [portBrand]: readonly [S, K];
}

export interface AliasPortRef<S extends string, K extends PortKind> {
  readonly source: "alias";
  readonly node: string;
  readonly selector: IntentPortSelector;
  readonly [portBrand]: readonly [S, K];
}

export type AnyPortRef<S extends string, K extends PortKind = PortKind> =
  | StablePortRef<S, K>
  | AliasPortRef<S, K>;

export interface IntentInputBinding<S extends string, R extends InputRole = InputRole> {
  readonly slot: InputSlot<R>;
  readonly source: AnyPortRef<S, PortKindForRole<R>>;
  readonly [sessionBrand]: S;
}

export type IntentUnit = "length" | "angle" | "dimensionless";

export type IntentLiteral =
  | { readonly kind: "boolean"; readonly value: boolean }
  | { readonly kind: "integer"; readonly value: number | bigint }
  | { readonly kind: "natural"; readonly value: number | bigint }
  | { readonly kind: "text"; readonly value: string }
  | { readonly kind: "enum"; readonly value: string }
  | { readonly kind: "point"; readonly value: readonly [number, number] }
  | {
      readonly kind: "quantity";
      readonly value: { readonly value: number; readonly unit: IntentUnit };
    };

export type QuantityLiteral = Extract<IntentLiteral, { readonly kind: "quantity" }>;

export type IntentNodeKind =
  | { readonly family: "geometry"; readonly recipe: GeometryRecipe }
  | { readonly family: "constraint"; readonly constraint: ConstraintKind }
  | { readonly family: "dimension"; readonly dimension: DimensionKind }
  | { readonly family: "operation"; readonly operation: OperationKind }
  | { readonly family: "computed_feature"; readonly feature: "fillet_set" }
  | { readonly family: "parameter"; readonly parameter: "parameter" | "binding" | "output" }
  | { readonly family: "external"; readonly external: "binding" | "snapshot_reference" }
  | { readonly family: "aggregate"; readonly aggregate: AggregateKind }
  | {
      readonly family: "bootstrap";
      readonly object: {
        readonly kind: BootstrapNativeKind;
        readonly codec: string;
        readonly payload: readonly number[];
      };
    }
  | { readonly family: "annotation" }
  | {
      readonly family: "identity";
      readonly transition: "alias" | "continue" | "retire";
      readonly port_kind: PortKind;
    };

export interface IntentNodeDraft<S extends string> {
  readonly kind: IntentNodeKind;
  readonly symbol: string;
  readonly name: string;
  readonly inputs: Readonly<Partial<Record<InputSlot, AnyPortRef<S>>>>;
  readonly fields: Readonly<Record<string, IntentLiteral>>;
  readonly initial_instance: Readonly<
    Partial<Record<IntentPortSelector, Readonly<Partial<Record<LeafField, QuantityLiteral>>>>>
  >;
  readonly operation_outputs: readonly IntentOperationOutput[];
  readonly dynamic_children: number;
  readonly suppressed: boolean;
  readonly [sessionBrand]: S;
}

export interface ComponentIdentity {
  readonly revision: string;
  readonly digest: string;
}

export interface ExternalInputsIdentity {
  readonly revision: string;
  readonly digest: string;
}

export interface IntentSessionIdentityWire {
  readonly session: string;
  readonly revision: string;
  readonly digest: string;
  readonly graph: ComponentIdentity;
  readonly instance: ComponentIdentity;
  readonly reservations: ComponentIdentity;
  readonly organization: ComponentIdentity;
  readonly external_inputs: ExternalInputsIdentity;
}

export interface IntentSessionIdentity<S extends string> extends IntentSessionIdentityWire {
  readonly session: S;
  readonly [sessionBrand]: S;
}

export type PatchPolicy = "require_accepted" | "retain_failed_intent";

export type DeletePolicy =
  | { readonly policy: "reject_dependents" }
  | { readonly policy: "cascade"; readonly exact_nodes: readonly string[] }
  | {
      readonly policy: "cascade_roots";
      readonly exact_roots: readonly string[];
      readonly exact_nodes: readonly string[];
    };

export type CellTarget<S extends string = string> =
  | { readonly target: "stable"; readonly cell: string; readonly [cellBrand]: S }
  | { readonly target: "alias"; readonly alias: string; readonly [cellBrand]: S };

export interface ExternalInputs {
  readonly revision: string;
  readonly parameter_batch: readonly number[];
  readonly external_snapshots: readonly number[];
}

export interface LeafRef<S extends string, K extends PortKind = PortKind> {
  readonly value: string;
  readonly field: LeafFieldForKind<K>;
  readonly [leafBrand]: readonly [S, K];
}

export type IntentOperation<S extends string> =
  | {
      readonly operation: "create_node";
      readonly alias: string;
      readonly draft: IntentNodeDraft<S>;
      readonly cell: CellTarget<S> | null;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "delete_node";
      readonly node: string;
      readonly policy: DeletePolicy;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "set_suppressed";
      readonly node: string;
      readonly suppressed: boolean;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "set_definition_field";
      readonly node: string;
      readonly field: string;
      readonly value: IntentLiteral;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "set_instance_leaf";
      readonly leaf: string;
      readonly value: QuantityLiteral;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "rebind_input";
      readonly node: string;
      readonly slot: InputSlot;
      readonly source: AnyPortRef<S>;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "eject_bootstrap_point";
      readonly node: string;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "rename_node";
      readonly node: string;
      readonly name: string;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "move_declaration";
      readonly node: string;
      readonly cell: CellTarget<S>;
      readonly before: string | null;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "create_cell";
      readonly alias: string;
      readonly name: string;
      readonly before: string | null;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "delete_cell";
      readonly cell: string;
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "reorder_cells";
      readonly exact_order: readonly string[];
      readonly [operationBrand]: S;
    }
  | {
      readonly operation: "replace_external_inputs";
      readonly inputs: ExternalInputs;
      readonly [operationBrand]: S;
    };

export interface IntentPatch<S extends string> {
  readonly expected: IntentSessionIdentity<S>;
  readonly policy: PatchPolicy;
  readonly operations: readonly IntentOperation<S>[];
  readonly [sessionBrand]: S;
}

export type IntentRpcRequest<S extends string = string> =
  | { readonly method: "snapshot" }
  | { readonly method: "apply_patch"; readonly patch: IntentPatch<S> }
  | { readonly method: "undo" }
  | { readonly method: "redo" }
  | { readonly method: "inspector"; readonly node: string }
  | {
      readonly method: "edit_source_token";
      readonly expected: IntentSessionIdentity<S>;
      readonly token: number;
      readonly replacement: string;
    };

/**
 * The exact single-string surface implemented by WASM `IntentRpcHandle.apply`.
 * A host binds one typed client to one explicitly owned handle:
 *
 * ```ts
 * const handle = new wasm.IntentRpcHandle(sessionId, documentId, 1);
 * const first = JSON.parse(handle.apply('{"method":"snapshot"}'));
 * const sessionId = first.value.snapshot.identity.session;
 * const client = new IntentClient(sessionId, {
 *   apply: (request) => handle.apply(request),
 * });
 * await client.undo();
 * ```
 */
export interface IntentRpcTransport {
  apply(canonicalRequestJson: string): string | Promise<string>;
}

/** Maximum response size accepted before JSON parsing. */
export const MAX_INTENT_RPC_RESPONSE_BYTES = 64 * 1024 * 1024;

/** Maximum response size for a mutating RPC receipt. */
export const MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES = 16 * 1024 * 1024;

/** An integer which can be represented exactly after parsing Rust JSON. */
export type IntentWireInteger = number | bigint;

/** Direct stable-port object used by Rust read-only projections. */
export interface IntentPortReference {
  readonly node: string;
  readonly port: string;
  readonly kind: PortKind;
}

/** One semantic object member or ordered collection index in a projection. */
export type IntentProjectionPathSegment = string | number;

/** Schema-derived path such as `["corners", 0, "parents", 1]`. */
export type IntentProjectionPath = readonly IntentProjectionPathSegment[];

/** Stable output reference used by source and Inspector presentation. */
export interface IntentProjectedPortReference {
  readonly declaration: string;
  readonly output: IntentProjectionPath;
  readonly kind: PortKind;
}

/** Recursive semantic object member used by generated Structured Source. */
export type IntentSourceValue<Leaf> =
  | Leaf
  | IntentSourceObject<Leaf>
  | IntentSourceArray<Leaf>;

/** Semantic object used by generated Structured Source. */
export interface IntentSourceObject<Leaf> {
  readonly [field: string]: IntentSourceValue<Leaf>;
  /** Storage selectors such as `point:0000` are not semantic object fields. */
  readonly [storageSelector: `${string}:${string}`]: never;
  /** Ordered members belong in arrays rather than zero-padded object keys. */
  readonly [stringEncodedIndex: `${number}`]: never;
}

/** Semantic ordered collection used by generated Structured Source. */
export interface IntentSourceArray<Leaf>
  extends ReadonlyArray<IntentSourceValue<Leaf> | null> {}

/** Compact metadata substituted for opaque bootstrap payload bytes. */
export interface IntentBootstrapMetadata {
  readonly kind: BootstrapNativeKind;
  readonly codec: string;
  readonly payload_bytes: IntentWireInteger;
  readonly payload_sha256: string;
}

/** Read-only node kind used by graph and generated-source projections. */
export type IntentGraphNodeKind =
  | Exclude<IntentNodeKind, { readonly family: "bootstrap" }>
  | {
      readonly family: "bootstrap";
      readonly object: IntentBootstrapMetadata;
    };

/** One declaration in Rust-generated, TypeScript-shaped Structured Source. */
export interface IntentSourceDeclaration {
  readonly symbol: string;
  readonly name: string;
  readonly kind: IntentGraphNodeKind;
  readonly suppressed: boolean;
  readonly inputs: IntentSourceObject<IntentProjectedPortReference>;
  readonly definition: IntentSourceObject<IntentLiteral>;
  readonly instance: IntentSourceObject<IntentLiteral>;
}

/** One organization cell in Rust-generated Structured Source. */
export interface IntentSourceCell {
  readonly cell: string;
  readonly name: string;
  readonly declarations: readonly IntentSourceDeclaration[];
}

/**
 * Data-only shape emitted by Rust Structured Source.
 *
 * It intentionally has no `design(...)` callback, executable expression
 * graph, solver behavior, session authority, or opaque bootstrap bytes.
 */
export interface IntentSourceSnapshot {
  readonly cells: readonly IntentSourceCell[];
}

export type IntentIdentityFlow =
  | { readonly state: "owned_logical" }
  | { readonly state: "created"; readonly reservation: string }
  | { readonly state: "aliased"; readonly source: IntentPortReference }
  | {
      readonly state: "continued" | "retired";
      readonly source: IntentPortReference;
      readonly generation: IntentWireInteger;
    };

export type IntentNativeReservationKind =
  | "point"
  | "scalar"
  | "curve"
  | "contact"
  | "constraint"
  | "constraint_source"
  | "dimension"
  | "dimension_source"
  | "parameter"
  | "external_binding"
  | "semantic_catalog"
  | "semantic_source";

export type IntentEditClassification =
  | "definition"
  | "instance"
  | "input_binding"
  | "organization"
  | "read_only";

export type IntentLiteralSchema =
  | { readonly kind: "boolean" | "integer" | "natural" | "text" | "enum" | "point" }
  | { readonly kind: "quantity"; readonly unit: IntentUnit };

export interface IntentInputCardinality {
  readonly role: InputRole;
  readonly minimum: number;
  readonly maximum: number;
}

export interface IntentInputChoiceSchema {
  readonly alternatives: readonly InputSlot[];
  readonly minimum: number;
  readonly maximum: number;
}

export interface IntentDefinitionFieldSchema {
  readonly field: string;
  readonly literal: IntentLiteralSchema;
  readonly required: boolean;
}

export interface IntentNodeSchema {
  readonly inputs: readonly IntentInputCardinality[];
  readonly input_choices: readonly IntentInputChoiceSchema[];
  readonly fields: readonly IntentDefinitionFieldSchema[];
  readonly minimum_children: number;
  readonly maximum_children: number;
}

export type IntentFieldDefault =
  | { readonly default: "required" | "conditional" | "contextual" }
  | { readonly default: "literal"; readonly value: IntentLiteral };

export type IntentFieldChoices =
  | { readonly choices: "not_applicable" | "contextual" }
  | { readonly choices: "closed"; readonly values: readonly string[] };

export interface IntentDefinitionFieldDescriptor {
  readonly schema: IntentDefinitionFieldSchema;
  readonly path: IntentProjectionPath;
  readonly default: IntentFieldDefault;
  readonly choices: IntentFieldChoices;
  readonly edit: IntentEditClassification;
}

export interface IntentInputDescriptor {
  readonly slot: InputSlot;
  readonly path: IntentProjectionPath;
}

export interface IntentOutputDescriptor {
  readonly port: IntentPortReference;
  readonly selector: IntentPortSelector;
  readonly path: IntentProjectionPath;
  readonly kind: PortKind;
  readonly writable: readonly LeafField[];
  readonly flow: IntentIdentityFlow;
  readonly native: IntentNativeReservationKind | null;
  readonly edit: IntentEditClassification;
}

export interface IntentDeclarationDescriptor {
  readonly schema: IntentNodeSchema;
  readonly inputs: readonly IntentInputDescriptor[];
  readonly fields: readonly IntentDefinitionFieldDescriptor[];
  readonly outputs: readonly IntentOutputDescriptor[];
  readonly suppression_edit: IntentEditClassification;
  readonly input_edit: IntentEditClassification;
  readonly name_edit: IntentEditClassification;
}

export interface IntentGraphInput {
  readonly slot: InputSlot;
  readonly source: IntentPortReference;
}

export interface IntentGraphDefinitionField {
  readonly field: string;
  readonly value: IntentLiteral;
}

export interface IntentGraphInstanceLeaf {
  readonly leaf: string;
  readonly value: IntentLiteral;
}

export interface IntentGraphChild {
  readonly child: string;
  readonly schema: IntentChildSchema;
  readonly ports: readonly IntentPortReference[];
}

export interface IntentGraphDeclaration {
  readonly node: string;
  readonly symbol: string;
  readonly name: string;
  readonly kind: IntentGraphNodeKind;
  readonly bootstrap_origin: IntentBootstrapMetadata | null;
  readonly suppressed: boolean;
  readonly inputs: readonly IntentGraphInput[];
  readonly definition_fields: readonly IntentGraphDefinitionField[];
  readonly instance_leaves: readonly IntentGraphInstanceLeaf[];
  readonly operation_outputs: readonly IntentOperationOutput[];
  readonly children: readonly IntentGraphChild[];
  readonly descriptor: IntentDeclarationDescriptor;
  readonly dependencies: readonly string[];
}

export interface IntentGraphCell {
  readonly cell: string;
  readonly name: string;
  readonly declarations: readonly IntentGraphDeclaration[];
}

export interface IntentGraphSnapshot<S extends string> {
  readonly identity: IntentSessionIdentity<S>;
  readonly cells: readonly IntentGraphCell[];
  readonly [sessionBrand]: S;
}

export interface IntentOutlineDeclaration {
  readonly node: string;
  readonly symbol: string;
  readonly name: string;
  readonly kind: IntentGraphNodeKind;
  readonly suppressed: boolean;
  readonly retained_failure: boolean;
  readonly dependencies: readonly string[];
}

export interface IntentOutlineCell {
  readonly cell: string;
  readonly name: string;
  readonly declarations: readonly IntentOutlineDeclaration[];
}

export type IntentSourceTokenTarget =
  | { readonly target: "node_name"; readonly node: string }
  | { readonly target: "suppressed"; readonly node: string }
  | { readonly target: "definition"; readonly node: string; readonly field: string }
  | { readonly target: "instance"; readonly leaf: string };

export interface IntentSourceToken {
  readonly id: number;
  readonly start: number;
  readonly end: number;
  readonly target: IntentSourceTokenTarget;
}

export interface IntentStructuredSource<S extends string> {
  readonly identity: IntentSessionIdentity<S>;
  readonly text: string;
  readonly tokens: readonly IntentSourceToken[];
  readonly [sessionBrand]: S;
}

export type IntentPlanDisposition = "accepted" | "retained_failed" | "organization_only";
export type IntentAttemptDisposition = IntentPlanDisposition;

export type IntentPatchOperationKind =
  | "create_node"
  | "delete_node"
  | "set_suppressed"
  | "set_definition_field"
  | "set_instance_leaf"
  | "rebind_input"
  | "eject_bootstrap_point"
  | "rename_node"
  | "move_declaration"
  | "create_cell"
  | "delete_cell"
  | "reorder_cells"
  | "replace_external_inputs";

export interface IntentSemanticDiff {
  readonly created_nodes: readonly string[];
  readonly deleted_nodes: readonly string[];
  readonly definition_nodes: readonly string[];
  readonly instance_nodes: readonly string[];
  readonly organization_nodes: readonly string[];
  readonly graph_changed: boolean;
  readonly instance_changed: boolean;
  readonly organization_changed: boolean;
  readonly external_inputs_changed: boolean;
}

export interface IntentTransactionDescriptor {
  readonly target_revision: string;
  readonly disposition: IntentPlanDisposition;
  readonly operation_kinds: readonly IntentPatchOperationKind[];
  readonly affected_nodes: readonly string[];
  readonly diff: IntentSemanticDiff;
}

export interface IntentHistoryProjection {
  readonly applied: readonly IntentTransactionDescriptor[];
  readonly redoable: readonly IntentTransactionDescriptor[];
}

export interface IntentWorkbenchProjection<S extends string> {
  readonly identity: IntentSessionIdentity<S>;
  readonly outline: readonly IntentOutlineCell[];
  readonly structured_source: IntentStructuredSource<S>;
  readonly history: IntentHistoryProjection;
  readonly latest_disposition: IntentAttemptDisposition | null;
  readonly latest_diagnostic: string | null;
  readonly [sessionBrand]: S;
}

export type IntentInspectorField =
  | {
      readonly field: "definition";
      readonly definition: string;
      readonly value: IntentLiteral | null;
    }
  | {
      readonly field: "instance";
      readonly leaf: string;
      readonly value: IntentLiteral | null;
    };

export interface IntentInspectorInput {
  readonly path: IntentProjectionPath;
  readonly source: IntentProjectedPortReference;
}

export interface IntentInspectorProjection<S extends string> {
  readonly identity: IntentSessionIdentity<S>;
  readonly node: string;
  readonly symbol: string;
  readonly name: string;
  readonly kind: IntentGraphNodeKind;
  readonly suppressed: boolean;
  readonly retained_failure: boolean;
  readonly inputs: readonly IntentInspectorInput[];
  readonly descriptor: IntentDeclarationDescriptor;
  readonly fields: readonly IntentInspectorField[];
  readonly [sessionBrand]: S;
}

export interface IntentSemanticIdentity {
  readonly graph: ComponentIdentity;
  readonly instance: ComponentIdentity;
  readonly reservations: ComponentIdentity;
  readonly external_inputs: ExternalInputsIdentity;
}

export interface IntentValidationEvidence {
  readonly semantic: IntentSemanticIdentity;
  readonly document: string;
  readonly point_count: number;
  readonly curve_count: number;
  readonly constraint_count: number;
  readonly hard_residuals_validated: boolean;
  readonly maximum_normalized_hard_residual: number | null;
  readonly feature_document: string;
  readonly feature_revision: IntentWireInteger;
  readonly feature_digest: string;
  readonly feature_count: number;
  readonly computed_edge_count: number;
  readonly all_active_features_current: boolean;
}

export interface IntentRpcSnapshot<S extends string> {
  readonly identity: IntentSessionIdentity<S>;
  readonly graph: IntentGraphSnapshot<S>;
  readonly projection: IntentWorkbenchProjection<S>;
  readonly accepted_validation: IntentValidationEvidence | null;
  readonly [sessionBrand]: S;
}

export interface IntentAliasMap<S extends string> {
  readonly nodes: Readonly<Record<string, string>>;
  readonly ports: Readonly<Record<string, Readonly<Record<IntentPortSelector, IntentPortReference>>>>;
  readonly cells: Readonly<Record<string, string>>;
  readonly [sessionBrand]: S;
}

export interface IntentRpcSnapshotSuccess<S extends string> {
  readonly result: "snapshot";
  readonly snapshot: IntentRpcSnapshot<S>;
}

export interface IntentRpcPatchReceipt<S extends string> {
  readonly identity: IntentSessionIdentity<S>;
  readonly disposition: IntentPlanDisposition;
  readonly aliases: IntentAliasMap<S>;
  readonly [sessionBrand]: S;
}

export interface IntentRpcPatchSuccess<S extends string> {
  readonly result: "patch";
  readonly receipt: IntentRpcPatchReceipt<S>;
}

export interface IntentRpcHistoryReceipt<S extends string> {
  readonly identity: IntentSessionIdentity<S>;
  readonly moved: boolean;
  readonly [sessionBrand]: S;
}

export interface IntentRpcHistorySuccess<S extends string> {
  readonly result: "history";
  readonly receipt: IntentRpcHistoryReceipt<S>;
}

export interface IntentRpcInspectorSuccess<S extends string> {
  readonly result: "inspector";
  readonly identity: IntentSessionIdentity<S>;
  readonly inspector: IntentInspectorProjection<S> | null;
}

export type IntentRpcFailureCode =
  | "invalid_request"
  | "request_too_large"
  | "receipt_too_large"
  | "response_too_large"
  | "patch_rejected"
  | "session_rejected"
  | "materialization_rejected"
  | "native_rejected"
  | "document_rejected"
  | "coordinator_rejected"
  | "source_edit_rejected"
  | "editor_rejected"
  | "workbench_unavailable"
  | "workbench_busy"
  | "workbench_surface_unavailable"
  | "code_authority_required";

export interface IntentRpcFailure<S extends string> {
  readonly code: IntentRpcFailureCode;
  readonly message: string;
  readonly identity: IntentSessionIdentity<S> | null;
  readonly [sessionBrand]: S;
}

export interface IntentRpcSuccessOutcome<S extends string, V> {
  readonly outcome: "success";
  readonly value: V;
  readonly [sessionBrand]: S;
}

export interface IntentRpcFailureOutcome<S extends string> {
  readonly outcome: "failure";
  readonly failure: IntentRpcFailure<S>;
  readonly [sessionBrand]: S;
}

export type IntentRpcResponse<S extends string, V> =
  | IntentRpcSuccessOutcome<S, V>
  | IntentRpcFailureOutcome<S>;

export type IntentRpcSnapshotResponse<S extends string> =
  IntentRpcResponse<S, IntentRpcSnapshotSuccess<S>>;
export type IntentRpcPatchResponse<S extends string> =
  IntentRpcResponse<S, IntentRpcPatchSuccess<S>>;
export type IntentRpcHistoryResponse<S extends string> =
  IntentRpcResponse<S, IntentRpcHistorySuccess<S>>;
export type IntentRpcInspectorResponse<S extends string> =
  IntentRpcResponse<S, IntentRpcInspectorSuccess<S>>;

/** Malformed, method-confused, excessive, or cross-session transport data. */
export class IntentRpcProtocolError extends TypeError {
  constructor(message: string) {
    super(message);
    this.name = "IntentRpcProtocolError";
  }
}

export interface DraftOptions<S extends string> {
  readonly name?: string;
  readonly inputs?: readonly IntentInputBinding<S>[];
  readonly fields?: Readonly<Record<string, IntentLiteral>>;
  readonly initialInstance?: Readonly<
    Partial<Record<IntentPortSelector, Readonly<Partial<Record<LeafField, QuantityLiteral>>>>>
  >;
  readonly operationOutputs?: readonly IntentOperationOutput[];
  readonly dynamicChildren?: number;
  readonly suppressed?: boolean;
}

export function session<const S extends string>(id: S): SessionRef<S> {
  requireHex(id, 32, "intent session ID");
  return own(id, { id }) as SessionRef<S>;
}

export function sessionIdentity<S extends string>(
  owner: SessionRef<S>,
  value: IntentSessionIdentityWire,
): IntentSessionIdentity<S> {
  assertExactKeys(value, [
    "session",
    "revision",
    "digest",
    "graph",
    "instance",
    "reservations",
    "organization",
    "external_inputs",
  ], "intent session identity");
  if (value.session !== owner.id) {
    throw new TypeError(`cross-session intent identity: expected ${owner.id}`);
  }
  requireHex(value.session, 32, "intent session ID");
  requireHex(value.revision, 16, "intent revision");
  requireHex(value.digest, 64, "intent digest");
  const result = {
    session: owner.id,
    revision: value.revision,
    digest: value.digest,
    graph: checkedComponent(value.graph, "graph identity"),
    instance: checkedComponent(value.instance, "instance identity"),
    reservations: checkedComponent(value.reservations, "reservation identity"),
    organization: checkedComponent(value.organization, "organization identity"),
    external_inputs: checkedComponent(value.external_inputs, "external-input identity"),
  };
  return own(owner.id, result) as IntentSessionIdentity<S>;
}

export function stableNode<S extends string>(owner: SessionRef<S>, id: string): NodeRef<S> {
  requireHex(id, 16, "intent node ID");
  return own(owner.id, { id }) as NodeRef<S>;
}

export function stableCell<S extends string>(owner: SessionRef<S>, id: string): CellRef<S> {
  requireHex(id, 16, "intent cell ID");
  return own(owner.id, { id }) as CellRef<S>;
}

export function stablePort<S extends string, K extends PortKind>(
  owner: SessionRef<S>,
  node: NodeRef<NoInfer<S>>,
  port: string,
  kind: K,
): StablePortRef<S, K> {
  requireOwner(owner, node);
  requireHex(port, 16, "intent port ID");
  const result = { source: "stable" as const, port: { node: node.id, port, kind } };
  return ownPort(owner.id, kind, result) as StablePortRef<S, K>;
}

export function aliasPort<S extends string, K extends PortKind>(
  owner: SessionRef<S>,
  alias: string,
  selector: IntentPortSelector,
  kind: K,
): AliasPortRef<S, K> {
  requireKey(alias, "transaction alias");
  validatePortSelector(selector);
  const result = { source: "alias" as const, node: alias, selector };
  return ownPort(owner.id, kind, result) as AliasPortRef<S, K>;
}

export function inputSlot<R extends InputRole>(role: R, index: number): InputSlot<R> {
  return `${role}:${u16Hex(index, "input index")}` as InputSlot<R>;
}

export function nodePort(role: PortRole, index = 0): IntentPortSelector {
  return `node:${role}:${u16Hex(index, "port index")}`;
}

export function childPort(ordinal: number, role: PortRole, index = 0): IntentPortSelector {
  return `child:${u16Hex(ordinal, "child ordinal")}:${role}:${u16Hex(index, "port index")}`;
}

export function input<S extends string, R extends InputRole>(
  owner: SessionRef<S>,
  role: R,
  index: number,
  source: AnyPortRef<NoInfer<S>, PortKindForRole<R>>,
): IntentInputBinding<S, R> {
  requireOwner(owner, source);
  const expected = portKindForRole(role);
  const actual = getPortKind(source);
  if (expected !== undefined && actual !== expected) {
    throw new TypeError(`input role ${role} requires ${expected}, received ${actual}`);
  }
  return own(owner.id, { slot: inputSlot(role, index), source }) as IntentInputBinding<S, R>;
}

export function draft<S extends string>(
  owner: SessionRef<S>,
  symbol: string,
  kind: IntentNodeKind,
  options: DraftOptions<NoInfer<S>> = {},
): IntentNodeDraft<S> {
  requireKey(symbol, "developer symbol");
  const name = options.name ?? symbol;
  requireKey(name, "display name");
  validateNodeKind(kind);
  const inputs: Partial<Record<InputSlot, AnyPortRef<S>>> = {};
  for (const binding of options.inputs ?? []) {
    requireOwner(owner, binding);
    if (Object.hasOwn(inputs, binding.slot)) {
      throw new TypeError(`duplicate intent input slot ${binding.slot}`);
    }
    inputs[binding.slot] = binding.source as AnyPortRef<S>;
  }
  const fields: Record<string, IntentLiteral> = {};
  for (const [key, value] of sortedStringEntries(options.fields ?? {})) {
    requireKey(key, "definition field");
    validateLiteral(value);
    fields[key] = value;
  }
  const initialInstance = normalizeInitialInstance(options.initialInstance ?? {});
  const operationOutputs = (options.operationOutputs ?? []).map((output) => {
    validateOperationOutput(output);
    return { ...output };
  });
  const dynamicChildren = options.dynamicChildren ?? 0;
  requireU16(dynamicChildren, "dynamic child count");
  return own(owner.id, {
    kind,
    symbol,
    name,
    inputs,
    fields,
    initial_instance: initialInstance,
    operation_outputs: operationOutputs,
    dynamic_children: dynamicChildren,
    suppressed: options.suppressed ?? false,
  }) as unknown as IntentNodeDraft<S>;
}

export function stableCellTarget<S extends string>(
  owner: SessionRef<S>,
  cell: CellRef<NoInfer<S>>,
): CellTarget<S> {
  requireOwner(owner, cell);
  return own(owner.id, { target: "stable" as const, cell: cell.id }) as CellTarget<S>;
}

export function aliasCellTarget<S extends string>(
  owner: SessionRef<S>,
  alias: string,
): CellTarget<S> {
  requireKey(alias, "cell alias");
  return own(owner.id, { target: "alias" as const, alias }) as CellTarget<S>;
}

export function leaf<S extends string, K extends "point" | "handle_point" | "scalar">(
  port: StablePortRef<S, K>,
  field: LeafFieldForKind<K>,
): LeafRef<S, K> {
  const owner = ownerOf(port);
  if (owner === undefined) {
    throw new TypeError("unowned intent port reference");
  }
  const allowed = port.port.kind === "scalar"
    ? ["value", "angle", "weight", "parameter"]
    : ["x", "y"];
  if (!allowed.includes(field)) {
    throw new TypeError(`leaf field ${field} is not valid for ${port.port.kind}`);
  }
  return own(owner, {
    value: `${port.port.node}:${port.port.port}:${field}`,
    field,
  }) as LeafRef<S, K>;
}

export function rejectDependents(): DeletePolicy {
  return { policy: "reject_dependents" };
}

export function cascade<S extends string>(
  owner: SessionRef<S>,
  nodes: readonly NodeRef<NoInfer<S>>[],
): DeletePolicy {
  const values = new Set<string>();
  for (const node of nodes) {
    requireOwner(owner, node);
    values.add(node.id);
  }
  return { policy: "cascade", exact_nodes: [...values].sort(compareStrings) };
}

export function cascadeRoots<S extends string>(
  owner: SessionRef<S>,
  roots: readonly NodeRef<NoInfer<S>>[],
  nodes: readonly NodeRef<NoInfer<S>>[],
): DeletePolicy {
  const exactRoots = new Set<string>();
  const exactNodes = new Set<string>();
  for (const node of roots) {
    requireOwner(owner, node);
    exactRoots.add(node.id);
  }
  for (const node of nodes) {
    requireOwner(owner, node);
    exactNodes.add(node.id);
  }
  return {
    policy: "cascade_roots",
    exact_roots: [...exactRoots].sort(compareStrings),
    exact_nodes: [...exactNodes].sort(compareStrings),
  };
}

export function createNode<S extends string>(
  owner: SessionRef<S>,
  alias: string,
  node: IntentNodeDraft<NoInfer<S>>,
  cell: CellTarget<NoInfer<S>> | null = null,
): IntentOperation<S> {
  requireKey(alias, "transaction alias");
  requireOwner(owner, node);
  if (cell !== null) requireOwner(owner, cell);
  return operation(owner, { operation: "create_node", alias, draft: node, cell });
}

export function deleteNode<S extends string>(
  owner: SessionRef<S>,
  node: NodeRef<NoInfer<S>>,
  policy: DeletePolicy,
): IntentOperation<S> {
  requireOwner(owner, node);
  return operation(owner, { operation: "delete_node", node: node.id, policy });
}

export function setSuppressed<S extends string>(
  owner: SessionRef<S>,
  node: NodeRef<NoInfer<S>>,
  suppressed: boolean,
): IntentOperation<S> {
  requireOwner(owner, node);
  return operation(owner, { operation: "set_suppressed", node: node.id, suppressed });
}

export function setDefinitionField<S extends string>(
  owner: SessionRef<S>,
  node: NodeRef<NoInfer<S>>,
  field: string,
  value: IntentLiteral,
): IntentOperation<S> {
  requireOwner(owner, node);
  requireKey(field, "definition field");
  validateLiteral(value);
  return operation(owner, { operation: "set_definition_field", node: node.id, field, value });
}

export function setInstanceLeaf<S extends string, K extends "point" | "handle_point" | "scalar">(
  owner: SessionRef<S>,
  target: LeafRef<NoInfer<S>, K>,
  value: QuantityLiteral,
): IntentOperation<S> {
  requireOwner(owner, target);
  validateLiteral(value);
  return operation(owner, { operation: "set_instance_leaf", leaf: target.value, value });
}

export function rebindInput<S extends string>(
  owner: SessionRef<S>,
  node: NodeRef<NoInfer<S>>,
  slot: InputSlot,
  source: AnyPortRef<NoInfer<S>>,
): IntentOperation<S> {
  requireOwner(owner, node);
  requireOwner(owner, source);
  validateInputSlot(slot);
  return operation(owner, { operation: "rebind_input", node: node.id, slot, source });
}

/** Promotes one exact legacy native Point into an editable typed Sketch Point in place. */
export function ejectBootstrapPoint<S extends string>(
  owner: SessionRef<S>,
  node: NodeRef<NoInfer<S>>,
): IntentOperation<S> {
  requireOwner(owner, node);
  return operation(owner, { operation: "eject_bootstrap_point", node: node.id });
}

export function renameNode<S extends string>(
  owner: SessionRef<S>,
  node: NodeRef<NoInfer<S>>,
  name: string,
): IntentOperation<S> {
  requireOwner(owner, node);
  requireKey(name, "display name");
  return operation(owner, { operation: "rename_node", node: node.id, name });
}

export function moveDeclaration<S extends string>(
  owner: SessionRef<S>,
  node: NodeRef<NoInfer<S>>,
  cell: CellTarget<NoInfer<S>>,
  before: NodeRef<NoInfer<S>> | null = null,
): IntentOperation<S> {
  requireOwner(owner, node);
  requireOwner(owner, cell);
  if (before !== null) requireOwner(owner, before);
  return operation(owner, {
    operation: "move_declaration",
    node: node.id,
    cell,
    before: before?.id ?? null,
  });
}

export function createCell<S extends string>(
  owner: SessionRef<S>,
  alias: string,
  name: string,
  before: CellRef<NoInfer<S>> | null = null,
): IntentOperation<S> {
  requireKey(alias, "cell alias");
  requireKey(name, "cell name");
  if (before !== null) requireOwner(owner, before);
  return operation(owner, {
    operation: "create_cell",
    alias,
    name,
    before: before?.id ?? null,
  });
}

export function deleteCell<S extends string>(
  owner: SessionRef<S>,
  cell: CellRef<NoInfer<S>>,
): IntentOperation<S> {
  requireOwner(owner, cell);
  return operation(owner, { operation: "delete_cell", cell: cell.id });
}

export function reorderCells<S extends string>(
  owner: SessionRef<S>,
  cells: readonly CellRef<NoInfer<S>>[],
): IntentOperation<S> {
  const exactOrder = cells.map((cell) => {
    requireOwner(owner, cell);
    return cell.id;
  });
  return operation(owner, { operation: "reorder_cells", exact_order: exactOrder });
}

export function externalInputs(
  revision: string,
  parameterBatch: readonly number[],
  externalSnapshots: readonly number[],
): ExternalInputs {
  requireHex(revision, 16, "external input revision");
  return {
    revision,
    parameter_batch: validateBytes(parameterBatch, "parameter batch"),
    external_snapshots: validateBytes(externalSnapshots, "external snapshots"),
  };
}

export function replaceExternalInputs<S extends string>(
  owner: SessionRef<S>,
  inputs: ExternalInputs,
): IntentOperation<S> {
  const checked = externalInputs(
    inputs.revision,
    inputs.parameter_batch,
    inputs.external_snapshots,
  );
  return operation(owner, { operation: "replace_external_inputs", inputs: checked });
}

export function patch<S extends string>(
  owner: SessionRef<S>,
  expected: IntentSessionIdentity<NoInfer<S>>,
  policy: PatchPolicy,
  operations: readonly IntentOperation<NoInfer<S>>[],
): IntentPatch<S> {
  requireOwner(owner, expected);
  if (operations.length === 0 || operations.length > 16_384) {
    throw new RangeError("an intent patch must contain between 1 and 16384 operations");
  }
  for (const value of operations) requireOwner(owner, value);
  const sorted = [...operations].sort((left, right) =>
    compareStrings(encodeOperation(left), encodeOperation(right))
  );
  return own(owner.id, { expected, policy, operations: sorted }) as unknown as IntentPatch<S>;
}

export class IntentClient<const S extends string> {
  readonly owner: SessionRef<S>;

  constructor(
    sessionId: S,
    private readonly transport: IntentRpcTransport,
  ) {
    this.owner = session(sessionId);
  }

  async apply(value: IntentPatch<S>): Promise<IntentRpcPatchResponse<S>> {
    requireOwner(this.owner, value);
    const response = await this.transport.apply(
      encodeIntentRpcRequest({ method: "apply_patch", patch: value }),
    );
    return decodeIntentRpcResponse(this.owner, response, "patch");
  }

  async snapshot(): Promise<IntentRpcSnapshotResponse<S>> {
    const response = await this.transport.apply(encodeIntentRpcRequest({ method: "snapshot" }));
    return decodeIntentRpcResponse(this.owner, response, "snapshot");
  }

  async undo(): Promise<IntentRpcHistoryResponse<S>> {
    const response = await this.transport.apply(encodeIntentRpcRequest({ method: "undo" }));
    return decodeIntentRpcResponse(this.owner, response, "history");
  }

  async redo(): Promise<IntentRpcHistoryResponse<S>> {
    const response = await this.transport.apply(encodeIntentRpcRequest({ method: "redo" }));
    return decodeIntentRpcResponse(this.owner, response, "history");
  }

  async inspector(node: NodeRef<S>): Promise<IntentRpcInspectorResponse<S>> {
    requireOwner(this.owner, node);
    const response = await this.transport.apply(
      encodeIntentRpcRequest({ method: "inspector", node: node.id }),
    );
    const decoded = decodeIntentRpcResponse(this.owner, response, "inspector");
    if (
      decoded.outcome === "success"
      && decoded.value.inspector !== null
      && decoded.value.inspector.node !== node.id
    ) {
      throw new IntentRpcProtocolError("Inspector projection does not match the requested node");
    }
    return decoded;
  }

  async editSourceToken(
    expected: IntentSessionIdentity<NoInfer<S>>,
    token: number,
    replacement: string,
  ): Promise<IntentRpcPatchResponse<S>> {
    requireOwner(this.owner, expected);
    if (!Number.isInteger(token) || token < 0 || token > 0xffff_ffff) {
      throw new RangeError("source token must be an unsigned 32-bit integer");
    }
    const response = await this.transport.apply(encodeIntentRpcRequest({
      method: "edit_source_token",
      expected,
      token,
      replacement,
    }));
    return decodeIntentRpcResponse(this.owner, response, "patch");
  }
}

/** Encodes the exact Rust `IntentPatch` object shape and canonical operation order. */
export function encodeIntentPatch<S extends string>(value: IntentPatch<S>): string {
  const operations = value.operations.map(encodeOperation).sort(compareStrings);
  return `{"expected":${encodeSessionIdentity(value.expected)},"policy":${quote(value.policy)},"operations":[${operations.join(",")}]}`;
}

/** Encodes one literal with the same tagged/content shape and finite-f64 spelling as Rust. */
export function encodeIntentLiteral(value: IntentLiteral): string {
  return encodeLiteral(value);
}

/** Encodes the closed DOM-free Rust `IntentRpcRequest` vocabulary. */
export function encodeIntentRpcRequest<S extends string>(value: IntentRpcRequest<S>): string {
  switch (value.method) {
    case "snapshot":
      return "{\"method\":\"snapshot\"}";
    case "apply_patch":
      return `{"method":"apply_patch","patch":${encodeIntentPatch(value.patch)}}`;
    case "undo":
      return "{\"method\":\"undo\"}";
    case "redo":
      return "{\"method\":\"redo\"}";
    case "inspector":
      requireHex(value.node, 16, "intent node ID");
      return `{"method":"inspector","node":${quote(value.node)}}`;
    case "edit_source_token":
      if (!Number.isInteger(value.token) || value.token < 0 || value.token > 0xffff_ffff) {
        throw new RangeError("source token must be an unsigned 32-bit integer");
      }
      return `{"method":"edit_source_token","expected":${encodeSessionIdentity(value.expected)},"token":${value.token},"replacement":${quote(value.replacement)}}`;
  }
}

/** Deterministic generic JSON helper for non-wire presentation data. */
export function canonicalStringify(value: unknown): string {
  return JSON.stringify(canonicalValue(value));
}

function decodeIntentRpcResponse<S extends string>(
  owner: SessionRef<S>,
  response: string,
  expected: "snapshot",
): IntentRpcSnapshotResponse<S>;
function decodeIntentRpcResponse<S extends string>(
  owner: SessionRef<S>,
  response: string,
  expected: "patch",
): IntentRpcPatchResponse<S>;
function decodeIntentRpcResponse<S extends string>(
  owner: SessionRef<S>,
  response: string,
  expected: "history",
): IntentRpcHistoryResponse<S>;
function decodeIntentRpcResponse<S extends string>(
  owner: SessionRef<S>,
  response: string,
  expected: "inspector",
): IntentRpcInspectorResponse<S>;
function decodeIntentRpcResponse<S extends string>(
  owner: SessionRef<S>,
  response: string,
  expected: "snapshot" | "patch" | "history" | "inspector",
): IntentRpcResponse<S, unknown> {
  try {
    if (typeof response !== "string") {
      throw new IntentRpcProtocolError("intent RPC transport returned a non-string response");
    }
    const maximumResponseBytes = expected === "patch" || expected === "history"
      ? MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES
      : MAX_INTENT_RPC_RESPONSE_BYTES;
    if (
      response.length > maximumResponseBytes ||
      new TextEncoder().encode(response).length > maximumResponseBytes
    ) {
      throw new IntentRpcProtocolError(
        `intent RPC response exceeds ${maximumResponseBytes} bytes`,
      );
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(response) as unknown;
    } catch {
      throw new IntentRpcProtocolError("intent RPC response is not valid JSON");
    }
    const envelope = rpcRecord(parsed, "intent RPC response");
    const outcome = rpcString(envelope.outcome, "intent RPC outcome");
    if (outcome === "failure") {
      rpcExactKeys(envelope, ["outcome", "failure"], "intent RPC failure response");
      const failure = parseRpcFailure(owner, envelope.failure);
      return own(owner.id, { outcome: "failure" as const, failure }) as IntentRpcFailureOutcome<S>;
    }
    if (outcome !== "success") {
      throw new IntentRpcProtocolError(`unknown intent RPC outcome ${JSON.stringify(outcome)}`);
    }
    rpcExactKeys(envelope, ["outcome", "value"], "intent RPC success response");
    const value = rpcRecord(envelope.value, "intent RPC success value");
    if (value.result !== expected) {
      throw new IntentRpcProtocolError(
        `intent RPC method expected result ${expected}, received ${String(value.result)}`,
      );
    }
    const checked = parseRpcSuccess(owner, expected, value);
    return own(owner.id, { outcome: "success" as const, value: checked }) as unknown as
      IntentRpcSuccessOutcome<S, unknown>;
  } catch (error) {
    if (error instanceof IntentRpcProtocolError) throw error;
    const message = error instanceof Error ? error.message : String(error);
    throw new IntentRpcProtocolError(`invalid intent RPC response: ${message}`);
  }
}

function parseRpcFailure<S extends string>(
  owner: SessionRef<S>,
  value: unknown,
): IntentRpcFailure<S> {
  const failure = rpcRecord(value, "intent RPC failure");
  rpcExactKeys(failure, ["code", "message", "identity"], "intent RPC failure");
  const code = rpcEnum(failure.code, RPC_FAILURE_CODES, "intent RPC failure code");
  const message = rpcBoundedString(failure.message, 1024 * 1024, "intent RPC failure message");
  const identity = failure.identity === null
    ? null
    : parseRpcIdentity(owner, failure.identity, "intent RPC failure identity");
  return own(owner.id, { code, message, identity }) as IntentRpcFailure<S>;
}

function parseRpcSuccess<S extends string>(
  owner: SessionRef<S>,
  expected: "snapshot" | "patch" | "history" | "inspector",
  value: Record<string, unknown>,
): unknown {
  switch (expected) {
    case "snapshot": {
      rpcExactKeys(value, ["result", "snapshot"], "snapshot RPC value");
      return {
        result: "snapshot" as const,
        snapshot: parseRpcSnapshot(owner, value.snapshot),
      } satisfies IntentRpcSnapshotSuccess<S>;
    }
    case "patch": {
      rpcExactKeys(value, ["result", "receipt"], "patch RPC value");
      const receipt = rpcRecord(value.receipt, "patch RPC receipt");
      rpcExactKeys(receipt, ["identity", "disposition", "aliases"], "patch RPC receipt");
      return {
        result: "patch" as const,
        receipt: own(owner.id, {
          identity: parseRpcIdentity(owner, receipt.identity, "patch receipt identity"),
          disposition: rpcEnum(
            receipt.disposition,
            PLAN_DISPOSITIONS,
            "intent plan disposition",
          ),
          aliases: parseAliasMap(owner, receipt.aliases),
        }) as IntentRpcPatchReceipt<S>,
      } satisfies IntentRpcPatchSuccess<S>;
    }
    case "history": {
      rpcExactKeys(value, ["result", "receipt"], "history RPC value");
      const receipt = rpcRecord(value.receipt, "history RPC receipt");
      rpcExactKeys(receipt, ["identity", "moved"], "history RPC receipt");
      return {
        result: "history" as const,
        receipt: own(owner.id, {
          identity: parseRpcIdentity(owner, receipt.identity, "history receipt identity"),
          moved: rpcBoolean(receipt.moved, "history moved flag"),
        }) as IntentRpcHistoryReceipt<S>,
      } satisfies IntentRpcHistorySuccess<S>;
    }
    case "inspector": {
      rpcExactKeys(value, ["result", "identity", "inspector"], "Inspector RPC value");
      const identity = parseRpcIdentity(owner, value.identity, "Inspector identity");
      const inspector = value.inspector === null
        ? null
        : parseInspector(owner, value.inspector);
      if (inspector !== null && !rpcIdentityEquals(identity, inspector.identity)) {
        throw new IntentRpcProtocolError("Inspector projection identity does not match receipt");
      }
      return {
        result: "inspector" as const,
        identity,
        inspector,
      } satisfies IntentRpcInspectorSuccess<S>;
    }
  }
}

function parseRpcSnapshot<S extends string>(
  owner: SessionRef<S>,
  value: unknown,
): IntentRpcSnapshot<S> {
  const snapshot = rpcRecord(value, "intent RPC snapshot");
  rpcExactKeys(
    snapshot,
    ["identity", "graph", "projection", "accepted_validation"],
    "intent RPC snapshot",
  );
  const identity = parseRpcIdentity(owner, snapshot.identity, "snapshot identity");
  const graph = parseGraphSnapshot(owner, snapshot.graph);
  const projection = parseWorkbenchProjection(owner, snapshot.projection);
  if (!rpcIdentityEquals(identity, graph.identity)) {
    throw new IntentRpcProtocolError("graph identity does not match snapshot identity");
  }
  if (!rpcIdentityEquals(identity, projection.identity)) {
    throw new IntentRpcProtocolError("projection identity does not match snapshot identity");
  }
  if (!rpcIdentityEquals(identity, projection.structured_source.identity)) {
    throw new IntentRpcProtocolError("Structured Source identity does not match snapshot identity");
  }
  const acceptedValidation = snapshot.accepted_validation === null
    ? null
    : parseValidationEvidence(snapshot.accepted_validation);
  return own(owner.id, {
    identity,
    graph,
    projection,
    accepted_validation: acceptedValidation,
  }) as IntentRpcSnapshot<S>;
}

function parseRpcIdentity<S extends string>(
  owner: SessionRef<S>,
  value: unknown,
  label: string,
): IntentSessionIdentity<S> {
  const identity = rpcRecord(value, label);
  rpcExactKeys(identity, [
    "session",
    "revision",
    "digest",
    "graph",
    "instance",
    "reservations",
    "organization",
    "external_inputs",
  ], label);
  const wire: IntentSessionIdentityWire = {
    session: rpcHex(identity.session, 32, `${label} session`),
    revision: rpcHex(identity.revision, 16, `${label} revision`),
    digest: rpcHex(identity.digest, 64, `${label} digest`),
    graph: parseComponentIdentity(identity.graph, `${label} graph`),
    instance: parseComponentIdentity(identity.instance, `${label} instance`),
    reservations: parseComponentIdentity(identity.reservations, `${label} reservations`),
    organization: parseComponentIdentity(identity.organization, `${label} organization`),
    external_inputs: parseComponentIdentity(identity.external_inputs, `${label} external inputs`),
  };
  return sessionIdentity(owner, wire);
}

function parseComponentIdentity(value: unknown, label: string): ComponentIdentity {
  const component = rpcRecord(value, label);
  rpcExactKeys(component, ["revision", "digest"], label);
  return {
    revision: rpcHex(component.revision, 16, `${label} revision`),
    digest: rpcHex(component.digest, 64, `${label} digest`),
  };
}

function rpcIdentityEquals(
  left: IntentSessionIdentityWire,
  right: IntentSessionIdentityWire,
): boolean {
  return left.session === right.session &&
    left.revision === right.revision &&
    left.digest === right.digest &&
    componentEquals(left.graph, right.graph) &&
    componentEquals(left.instance, right.instance) &&
    componentEquals(left.reservations, right.reservations) &&
    componentEquals(left.organization, right.organization) &&
    componentEquals(left.external_inputs, right.external_inputs);
}

function componentEquals(left: ComponentIdentity, right: ComponentIdentity): boolean {
  return left.revision === right.revision && left.digest === right.digest;
}

function parseGraphSnapshot<S extends string>(
  owner: SessionRef<S>,
  value: unknown,
): IntentGraphSnapshot<S> {
  const graph = rpcRecord(value, "intent graph snapshot");
  rpcExactKeys(graph, ["identity", "cells"], "intent graph snapshot");
  const identity = parseRpcIdentity(owner, graph.identity, "graph snapshot identity");
  const cells = rpcArray(graph.cells, MAX_INTENT_GRAPH_NODES, "intent graph cells")
    .map(parseGraphCell);
  return own(owner.id, { identity, cells }) as unknown as IntentGraphSnapshot<S>;
}

function parseGraphCell(value: unknown): IntentGraphCell {
  const cell = rpcRecord(value, "intent graph cell");
  rpcExactKeys(cell, ["cell", "name", "declarations"], "intent graph cell");
  return {
    cell: rpcHex(cell.cell, 16, "intent cell ID"),
    name: rpcKey(cell.name, "intent cell name"),
    declarations: rpcArray(
      cell.declarations,
      MAX_INTENT_GRAPH_NODES,
      "intent graph declarations",
    )
      .map(parseGraphDeclaration),
  };
}

function parseGraphDeclaration(value: unknown): IntentGraphDeclaration {
  const declaration = rpcRecord(value, "intent graph declaration");
  rpcExactKeys(declaration, [
    "node",
    "symbol",
    "name",
    "kind",
    "bootstrap_origin",
    "suppressed",
    "inputs",
    "definition_fields",
    "instance_leaves",
    "operation_outputs",
    "children",
    "descriptor",
    "dependencies",
  ], "intent graph declaration");
  return {
    node: rpcHex(declaration.node, 16, "intent node ID"),
    symbol: rpcKey(declaration.symbol, "intent node symbol"),
    name: rpcKey(declaration.name, "intent node name"),
    kind: parseGraphNodeKind(declaration.kind),
    bootstrap_origin: declaration.bootstrap_origin === null
      ? null
      : parseBootstrapMetadata(declaration.bootstrap_origin),
    suppressed: rpcBoolean(declaration.suppressed, "intent suppression flag"),
    inputs: rpcArray(
      declaration.inputs,
      MAX_INTENT_NODE_COMPONENTS,
      "intent graph inputs",
    ).map(parseGraphInput),
    definition_fields: rpcArray(
      declaration.definition_fields,
      MAX_INTENT_NODE_COMPONENTS,
      "intent graph definition fields",
    ).map(parseGraphDefinitionField),
    instance_leaves: rpcArray(
      declaration.instance_leaves,
      16_384,
      "intent graph instance leaves",
    ).map(parseGraphInstanceLeaf),
    operation_outputs: rpcArray(
      declaration.operation_outputs,
      MAX_INTENT_NODE_COMPONENTS,
      "intent graph operation outputs",
    ).map(parseOperationOutput),
    children: rpcArray(
      declaration.children,
      MAX_INTENT_NODE_COMPONENTS,
      "intent graph children",
    ).map(parseGraphChild),
    descriptor: parseDeclarationDescriptor(declaration.descriptor),
    dependencies: parseNodeIds(declaration.dependencies, "intent graph dependencies"),
  };
}

function parseGraphInput(value: unknown): IntentGraphInput {
  const inputValue = rpcRecord(value, "intent graph input");
  rpcExactKeys(inputValue, ["slot", "source"], "intent graph input");
  return {
    slot: rpcInputSlot(inputValue.slot),
    source: parsePortReference(inputValue.source),
  };
}

function parseGraphDefinitionField(value: unknown): IntentGraphDefinitionField {
  const field = rpcRecord(value, "intent graph definition field");
  rpcExactKeys(field, ["field", "value"], "intent graph definition field");
  return {
    field: rpcKey(field.field, "intent definition field"),
    value: parseRpcLiteral(field.value),
  };
}

function parseGraphInstanceLeaf(value: unknown): IntentGraphInstanceLeaf {
  const leafValue = rpcRecord(value, "intent graph instance leaf");
  rpcExactKeys(leafValue, ["leaf", "value"], "intent graph instance leaf");
  return {
    leaf: rpcLeaf(leafValue.leaf),
    value: parseRpcLiteral(leafValue.value),
  };
}

function parseOperationOutput(value: unknown): IntentOperationOutput {
  const output = rpcRecord(value, "intent operation output");
  rpcExactKeys(output, ["kind", "curve_span_count"], "intent operation output");
  const parsed = {
    kind: rpcEnum(output.kind, OPERATION_OUTPUT_KINDS, "intent operation output kind"),
    curve_span_count: rpcSafeUint(
      output.curve_span_count,
      0xffff,
      "intent operation curve span count",
    ),
  };
  validateOperationOutput(parsed);
  return parsed;
}

function parseGraphChild(value: unknown): IntentGraphChild {
  const child = rpcRecord(value, "intent graph child");
  rpcExactKeys(child, ["child", "schema", "ports"], "intent graph child");
  return {
    child: rpcHex(child.child, 16, "intent child ID"),
    schema: rpcEnum(child.schema, CHILD_SCHEMAS, "intent child schema"),
    ports: rpcArray(
      child.ports,
      MAX_INTENT_NODE_COMPONENTS,
      "intent child ports",
    ).map(parsePortReference),
  };
}

function parseGraphNodeKind(value: unknown): IntentGraphNodeKind {
  const result = parseNodeKind(value);
  return result;
}

function parseNodeKind(value: unknown): IntentGraphNodeKind {
  const kind = rpcRecord(value, "intent node kind");
  const family = rpcEnum(kind.family, NODE_FAMILIES, "intent node family");
  switch (family) {
    case "geometry":
      rpcExactKeys(kind, ["family", "recipe"], "geometry node kind");
      return { family, recipe: rpcEnum(kind.recipe, GEOMETRY_RECIPES, "geometry recipe") };
    case "constraint":
      rpcExactKeys(kind, ["family", "constraint"], "constraint node kind");
      return { family, constraint: rpcEnum(kind.constraint, CONSTRAINT_KINDS, "constraint kind") };
    case "dimension":
      rpcExactKeys(kind, ["family", "dimension"], "dimension node kind");
      return { family, dimension: rpcEnum(kind.dimension, DIMENSION_KINDS, "dimension kind") };
    case "operation":
      rpcExactKeys(kind, ["family", "operation"], "operation node kind");
      return { family, operation: rpcEnum(kind.operation, OPERATION_KINDS, "operation kind") };
    case "computed_feature":
      rpcExactKeys(kind, ["family", "feature"], "computed-feature node kind");
      return { family, feature: rpcEnum(kind.feature, ["fillet_set"] as const, "feature kind") };
    case "aggregate":
      rpcExactKeys(kind, ["family", "aggregate"], "aggregate node kind");
      return { family, aggregate: rpcEnum(kind.aggregate, AGGREGATE_KINDS, "aggregate kind") };
    case "parameter":
      rpcExactKeys(kind, ["family", "parameter"], "parameter node kind");
      return { family, parameter: rpcEnum(kind.parameter, PARAMETER_KINDS, "parameter kind") };
    case "external":
      rpcExactKeys(kind, ["family", "external"], "external node kind");
      return { family, external: rpcEnum(kind.external, EXTERNAL_KINDS, "external kind") };
    case "bootstrap": {
      rpcExactKeys(kind, ["family", "object"], "bootstrap node kind");
      return { family, object: parseBootstrapMetadata(kind.object) };
    }
    case "annotation":
      rpcExactKeys(kind, ["family"], "annotation node kind");
      return { family };
    case "identity":
      rpcExactKeys(kind, ["family", "transition", "port_kind"], "identity node kind");
      return {
        family,
        transition: rpcEnum(kind.transition, IDENTITY_TRANSITIONS, "identity transition"),
        port_kind: rpcEnum(kind.port_kind, PORT_KINDS, "identity port kind"),
      };
  }
}

function parseBootstrapMetadata(value: unknown): IntentBootstrapMetadata {
  const metadata = rpcRecord(value, "bootstrap metadata");
  rpcExactKeys(
    metadata,
    ["kind", "codec", "payload_bytes", "payload_sha256"],
    "bootstrap metadata",
  );
  return {
    kind: rpcEnum(metadata.kind, BOOTSTRAP_NATIVE_KINDS, "bootstrap native kind"),
    codec: rpcKey(metadata.codec, "bootstrap codec"),
    payload_bytes: rpcWireUint(metadata.payload_bytes, "bootstrap payload byte count"),
    payload_sha256: rpcHex(metadata.payload_sha256, 64, "bootstrap payload SHA-256"),
  };
}

function parseDeclarationDescriptor(value: unknown): IntentDeclarationDescriptor {
  const descriptor = rpcRecord(value, "intent declaration descriptor");
  rpcExactKeys(descriptor, [
    "schema",
    "inputs",
    "fields",
    "outputs",
    "suppression_edit",
    "input_edit",
    "name_edit",
  ], "intent declaration descriptor");
  const schema = parseNodeSchema(descriptor.schema);
  const inputs = rpcArray(
    descriptor.inputs,
    MAX_INTENT_NODE_COMPONENTS,
    "input descriptors",
  ).map(parseInputDescriptor);
  const fields = rpcArray(
    descriptor.fields,
    MAX_INTENT_NODE_COMPONENTS,
    "definition descriptors",
  ).map(parseDefinitionDescriptor);
  const outputs = rpcArray(
    descriptor.outputs,
    MAX_INTENT_DECLARATION_OUTPUTS,
    "output descriptors",
  ).map(parseOutputDescriptor);

  assertUniqueStrings(inputs.map((input) => input.slot), "input descriptor slots");
  assertUnambiguousProjectionPaths(
    inputs.map((input) => input.path),
    "input descriptor paths",
  );
  assertUniqueStrings(
    fields.map((field) => field.schema.field),
    "definition descriptor fields",
  );
  assertUnambiguousProjectionPaths(
    fields.map((field) => field.path),
    "definition descriptor paths",
  );
  assertDescriptorFieldsMatchSchema(schema, fields);
  assertUniqueStrings(
    outputs.map((output) => `${output.port.node}:${output.port.port}`),
    "output descriptor ports",
  );
  assertUniqueStrings(
    outputs.map((output) => output.selector),
    "output descriptor selectors",
  );
  assertUniqueProjectionPaths(
    outputs.map((output) => output.path),
    "output descriptor paths",
  );
  assertUnambiguousProjectionPaths(
    outputs.flatMap((output) => output.writable.map((leaf) =>
      output.writable.length === 1 ? output.path : [...output.path, leaf]
    )),
    "writable instance paths",
  );

  return {
    schema,
    inputs,
    fields,
    outputs,
    suppression_edit: rpcEnum(
      descriptor.suppression_edit,
      EDIT_CLASSIFICATIONS,
      "suppression edit classification",
    ),
    input_edit: rpcEnum(
      descriptor.input_edit,
      EDIT_CLASSIFICATIONS,
      "input edit classification",
    ),
    name_edit: rpcEnum(
      descriptor.name_edit,
      EDIT_CLASSIFICATIONS,
      "name edit classification",
    ),
  };
}

function parseProjectionPath(value: unknown, label: string): IntentProjectionPath {
  const segments = rpcArray(value, 32, label);
  if (segments.length === 0) {
    throw new IntentRpcProtocolError(`${label} must not be empty`);
  }
  if (typeof segments[0] !== "string") {
    throw new IntentRpcProtocolError(`${label} must begin with an object field`);
  }
  return segments.map((segment, index) => {
    if (typeof segment === "string") {
      return rpcKey(segment, `${label} field`);
    }
    if (typeof segment === "number") {
      return rpcSafeUint(segment, 0xffff, `${label} index`);
    }
    throw new IntentRpcProtocolError(`${label} segment ${index} is not a field or index`);
  });
}

function projectionPathSegmentKey(segment: IntentProjectionPathSegment): string {
  return typeof segment === "string" ? `field:${segment}` : `index:${segment}`;
}

function projectionPathKey(path: IntentProjectionPath): string {
  return path.map(projectionPathSegmentKey).join("\u0000");
}

function assertUniqueStrings(values: readonly string[], label: string): void {
  const seen = new Set<string>();
  for (const value of values) {
    if (seen.has(value)) {
      throw new IntentRpcProtocolError(`${label} contain duplicate coordinate ${JSON.stringify(value)}`);
    }
    seen.add(value);
  }
}

function assertUniqueProjectionPaths(
  paths: readonly IntentProjectionPath[],
  label: string,
): void {
  assertUniqueStrings(paths.map(projectionPathKey), label);
}

interface ProjectionPathTrie {
  terminal: boolean;
  readonly children: Map<string, ProjectionPathTrie>;
}

function assertUnambiguousProjectionPaths(
  paths: readonly IntentProjectionPath[],
  label: string,
): void {
  const root: ProjectionPathTrie = { terminal: false, children: new Map() };
  for (const path of paths) {
    let node = root;
    for (const segment of path) {
      if (node.terminal) {
        throw new IntentRpcProtocolError(`${label} contain a prefix collision`);
      }
      const key = projectionPathSegmentKey(segment);
      let child = node.children.get(key);
      if (child === undefined) {
        child = { terminal: false, children: new Map() };
        node.children.set(key, child);
      }
      node = child;
    }
    if (node.terminal) {
      throw new IntentRpcProtocolError(`${label} contain a duplicate path`);
    }
    if (node.children.size !== 0) {
      throw new IntentRpcProtocolError(`${label} contain a prefix collision`);
    }
    node.terminal = true;
  }
}

function literalSchemasEqual(
  left: IntentLiteralSchema,
  right: IntentLiteralSchema,
): boolean {
  return left.kind === right.kind
    && (left.kind !== "quantity"
      || (right.kind === "quantity" && left.unit === right.unit));
}

function assertDescriptorFieldsMatchSchema(
  schema: IntentNodeSchema,
  fields: readonly IntentDefinitionFieldDescriptor[],
): void {
  assertUniqueStrings(schema.fields.map((field) => field.field), "definition schema fields");
  if (schema.fields.length !== fields.length) {
    throw new IntentRpcProtocolError("definition descriptors do not match the declaration schema");
  }
  const descriptors = new Map(fields.map((field) => [field.schema.field, field.schema]));
  for (const expected of schema.fields) {
    const actual = descriptors.get(expected.field);
    if (
      actual === undefined
      || actual.required !== expected.required
      || !literalSchemasEqual(actual.literal, expected.literal)
    ) {
      throw new IntentRpcProtocolError("definition descriptors do not match the declaration schema");
    }
  }
}

function parseProjectedPortReference(value: unknown): IntentProjectedPortReference {
  const reference = rpcRecord(value, "projected port reference");
  rpcExactKeys(
    reference,
    ["declaration", "output", "kind"],
    "projected port reference",
  );
  return {
    declaration: rpcKey(reference.declaration, "projected declaration symbol"),
    output: parseProjectionPath(reference.output, "projected output path"),
    kind: rpcEnum(reference.kind, PORT_KINDS, "projected port kind"),
  };
}

function parseInputDescriptor(value: unknown): IntentInputDescriptor {
  const descriptor = rpcRecord(value, "input descriptor");
  rpcExactKeys(descriptor, ["slot", "path"], "input descriptor");
  return {
    slot: rpcInputSlot(descriptor.slot),
    path: parseProjectionPath(descriptor.path, "input descriptor path"),
  };
}

function parseNodeSchema(value: unknown): IntentNodeSchema {
  const schema = rpcRecord(value, "intent node schema");
  rpcExactKeys(schema, [
    "inputs",
    "input_choices",
    "fields",
    "minimum_children",
    "maximum_children",
  ], "intent node schema");
  return {
    inputs: rpcArray(
      schema.inputs,
      MAX_INTENT_NODE_COMPONENTS,
      "input cardinalities",
    ).map((candidate) => {
      const inputValue = rpcRecord(candidate, "input cardinality");
      rpcExactKeys(inputValue, ["role", "minimum", "maximum"], "input cardinality");
      return {
        role: rpcEnum(inputValue.role, INPUT_ROLES, "input role"),
        minimum: rpcSafeUint(inputValue.minimum, 0xffff, "minimum input count"),
        maximum: rpcSafeUint(inputValue.maximum, 0xffff, "maximum input count"),
      };
    }),
    input_choices: rpcArray(
      schema.input_choices,
      MAX_INTENT_NODE_COMPONENTS,
      "input choice schemas",
    )
      .map((candidate) => {
        const choice = rpcRecord(candidate, "input choice schema");
        rpcExactKeys(choice, ["alternatives", "minimum", "maximum"], "input choice schema");
        return {
          alternatives: rpcArray(
            choice.alternatives,
            MAX_INTENT_NODE_COMPONENTS,
            "input choice alternatives",
          )
            .map(rpcInputSlot),
          minimum: rpcSafeUint(choice.minimum, 0xffff, "minimum input choice count"),
          maximum: rpcSafeUint(choice.maximum, 0xffff, "maximum input choice count"),
        };
      }),
    fields: rpcArray(
      schema.fields,
      MAX_INTENT_NODE_COMPONENTS,
      "definition field schemas",
    )
      .map(parseDefinitionFieldSchema),
    minimum_children: rpcSafeUint(schema.minimum_children, 0xffff, "minimum child count"),
    maximum_children: rpcSafeUint(schema.maximum_children, 0xffff, "maximum child count"),
  };
}

function parseDefinitionFieldSchema(value: unknown): IntentDefinitionFieldSchema {
  const schema = rpcRecord(value, "definition field schema");
  rpcExactKeys(schema, ["field", "literal", "required"], "definition field schema");
  return {
    field: rpcKey(schema.field, "definition field"),
    literal: parseLiteralSchema(schema.literal),
    required: rpcBoolean(schema.required, "definition required flag"),
  };
}

function parseLiteralSchema(value: unknown): IntentLiteralSchema {
  const schema = rpcRecord(value, "intent literal schema");
  const kind = rpcEnum(schema.kind, LITERAL_KINDS, "intent literal schema kind");
  if (kind === "quantity") {
    rpcExactKeys(schema, ["kind", "unit"], "quantity literal schema");
    return { kind, unit: rpcEnum(schema.unit, INTENT_UNITS, "intent unit") };
  }
  rpcExactKeys(schema, ["kind"], "intent literal schema");
  return { kind };
}

function parseDefinitionDescriptor(value: unknown): IntentDefinitionFieldDescriptor {
  const descriptor = rpcRecord(value, "definition field descriptor");
  rpcExactKeys(
    descriptor,
    ["schema", "path", "default", "choices", "edit"],
    "definition field descriptor",
  );
  return {
    schema: parseDefinitionFieldSchema(descriptor.schema),
    path: parseProjectionPath(descriptor.path, "definition descriptor path"),
    default: parseFieldDefault(descriptor.default),
    choices: parseFieldChoices(descriptor.choices),
    edit: rpcEnum(descriptor.edit, EDIT_CLASSIFICATIONS, "definition edit classification"),
  };
}

function parseFieldDefault(value: unknown): IntentFieldDefault {
  const defaultValue = rpcRecord(value, "intent field default");
  const tag = rpcEnum(defaultValue.default, FIELD_DEFAULTS, "intent field default tag");
  if (tag === "literal") {
    rpcExactKeys(defaultValue, ["default", "value"], "literal field default");
    return { default: tag, value: parseRpcLiteral(defaultValue.value) };
  }
  rpcExactKeys(defaultValue, ["default"], "intent field default");
  return { default: tag };
}

function parseFieldChoices(value: unknown): IntentFieldChoices {
  const choices = rpcRecord(value, "intent field choices");
  const tag = rpcEnum(choices.choices, FIELD_CHOICES, "intent field choices tag");
  if (tag === "closed") {
    rpcExactKeys(choices, ["choices", "values"], "closed field choices");
    return {
      choices: tag,
      values: rpcArray(
        choices.values,
        MAX_INTENT_NODE_COMPONENTS,
        "closed field choice values",
      )
        .map((candidate) => rpcKey(candidate, "field choice")),
    };
  }
  rpcExactKeys(choices, ["choices"], "intent field choices");
  return { choices: tag };
}

function parseOutputDescriptor(value: unknown): IntentOutputDescriptor {
  const output = rpcRecord(value, "intent output descriptor");
  rpcExactKeys(
    output,
    ["port", "selector", "path", "kind", "writable", "flow", "native", "edit"],
    "intent output descriptor",
  );
  const kind = rpcEnum(output.kind, PORT_KINDS, "output port kind");
  const port = parsePortReference(output.port);
  if (port.kind !== kind) {
    throw new IntentRpcProtocolError("output descriptor port kind does not match its kind field");
  }
  return {
    port,
    selector: rpcPortSelector(output.selector),
    path: parseProjectionPath(output.path, "output descriptor path"),
    kind,
    writable: rpcArray(output.writable, LEAF_FIELDS.length, "writable output fields")
      .map((field) => rpcEnum(field, LEAF_FIELDS, "writable leaf field")),
    flow: parseIdentityFlow(output.flow),
    native: output.native === null
      ? null
      : rpcEnum(output.native, NATIVE_RESERVATION_KINDS, "native reservation kind"),
    edit: rpcEnum(output.edit, EDIT_CLASSIFICATIONS, "output edit classification"),
  };
}

function parseIdentityFlow(value: unknown): IntentIdentityFlow {
  const flow = rpcRecord(value, "intent identity flow");
  const state = rpcEnum(flow.state, IDENTITY_FLOW_STATES, "identity flow state");
  switch (state) {
    case "owned_logical":
      rpcExactKeys(flow, ["state"], "owned logical identity flow");
      return { state };
    case "created":
      rpcExactKeys(flow, ["state", "reservation"], "created identity flow");
      return { state, reservation: rpcHex(flow.reservation, 16, "reservation ID") };
    case "aliased":
      rpcExactKeys(flow, ["state", "source"], "aliased identity flow");
      return { state, source: parsePortReference(flow.source) };
    case "continued":
    case "retired":
      rpcExactKeys(flow, ["state", "source", "generation"], `${state} identity flow`);
      return {
        state,
        source: parsePortReference(flow.source),
        generation: rpcWireUint(flow.generation, "identity generation"),
      };
  }
}

function parseRpcLiteral(value: unknown): IntentLiteral {
  const literal = rpcRecord(value, "intent literal");
  const kind = rpcEnum(literal.kind, LITERAL_KINDS, "intent literal kind");
  rpcExactKeys(literal, ["kind", "value"], "intent literal");
  switch (kind) {
    case "boolean":
      return { kind, value: rpcBoolean(literal.value, "boolean intent literal") };
    case "integer":
      return { kind, value: rpcWireInteger(literal.value, I64_MIN, I64_MAX, "integer literal") };
    case "natural":
      return { kind, value: rpcWireInteger(literal.value, 0n, U64_MAX, "natural literal") };
    case "text":
    case "enum":
      return { kind, value: rpcKey(literal.value, `${kind} literal`) };
    case "point": {
      const pointValue = rpcArray(literal.value, 2, "point literal");
      if (pointValue.length !== 2) {
        throw new IntentRpcProtocolError("point intent literal must have exactly two coordinates");
      }
      return {
        kind,
        value: [
          rpcFinite(pointValue[0], "point x coordinate"),
          rpcFinite(pointValue[1], "point y coordinate"),
        ],
      };
    }
    case "quantity": {
      const quantity = rpcRecord(literal.value, "quantity intent literal");
      rpcExactKeys(quantity, ["value", "unit"], "quantity intent literal");
      return {
        kind,
        value: {
          value: rpcFinite(quantity.value, "quantity value"),
          unit: rpcEnum(quantity.unit, INTENT_UNITS, "intent unit"),
        },
      };
    }
  }
}

function parseWorkbenchProjection<S extends string>(
  owner: SessionRef<S>,
  value: unknown,
): IntentWorkbenchProjection<S> {
  const projection = rpcRecord(value, "intent workbench projection");
  rpcExactKeys(projection, [
    "identity",
    "outline",
    "structured_source",
    "history",
    "latest_disposition",
    "latest_diagnostic",
  ], "intent workbench projection");
  const identity = parseRpcIdentity(owner, projection.identity, "workbench projection identity");
  const structuredSource = parseStructuredSource(owner, projection.structured_source);
  return own(owner.id, {
    identity,
    outline: rpcArray(
      projection.outline,
      MAX_INTENT_GRAPH_NODES,
      "intent outline cells",
    ).map(parseOutlineCell),
    structured_source: structuredSource,
    history: parseHistory(projection.history),
    latest_disposition: projection.latest_disposition === null
      ? null
      : rpcEnum(
        projection.latest_disposition,
        PLAN_DISPOSITIONS,
        "latest intent disposition",
      ),
    latest_diagnostic: projection.latest_diagnostic === null
      ? null
      : rpcKey(projection.latest_diagnostic, "latest intent diagnostic"),
  }) as unknown as IntentWorkbenchProjection<S>;
}

function parseOutlineCell(value: unknown): IntentOutlineCell {
  const cell = rpcRecord(value, "intent outline cell");
  rpcExactKeys(cell, ["cell", "name", "declarations"], "intent outline cell");
  return {
    cell: rpcHex(cell.cell, 16, "intent cell ID"),
    name: rpcKey(cell.name, "intent cell name"),
    declarations: rpcArray(
      cell.declarations,
      MAX_INTENT_GRAPH_NODES,
      "intent outline declarations",
    )
      .map(parseOutlineDeclaration),
  };
}

function parseOutlineDeclaration(value: unknown): IntentOutlineDeclaration {
  const declaration = rpcRecord(value, "intent outline declaration");
  rpcExactKeys(declaration, [
    "node",
    "symbol",
    "name",
    "kind",
    "suppressed",
    "retained_failure",
    "dependencies",
  ], "intent outline declaration");
  return {
    node: rpcHex(declaration.node, 16, "intent node ID"),
    symbol: rpcKey(declaration.symbol, "intent node symbol"),
    name: rpcKey(declaration.name, "intent node name"),
    kind: parseGraphNodeKind(declaration.kind),
    suppressed: rpcBoolean(declaration.suppressed, "intent suppression flag"),
    retained_failure: rpcBoolean(declaration.retained_failure, "retained failure flag"),
    dependencies: parseNodeIds(declaration.dependencies, "intent outline dependencies"),
  };
}

function parseStructuredSource<S extends string>(
  owner: SessionRef<S>,
  value: unknown,
): IntentStructuredSource<S> {
  const source = rpcRecord(value, "intent Structured Source");
  rpcExactKeys(source, ["identity", "text", "tokens"], "intent Structured Source");
  const identity = parseRpcIdentity(owner, source.identity, "Structured Source identity");
  const text = rpcBoundedString(source.text, MAX_INTENT_RPC_RESPONSE_BYTES, "Structured Source text");
  const byteLength = new TextEncoder().encode(text).length;
  const tokens = rpcArray(source.tokens, 1_000_000, "Structured Source tokens")
    .map((token) => parseSourceToken(token, byteLength));
  return own(owner.id, { identity, text, tokens }) as unknown as IntentStructuredSource<S>;
}

function parseSourceToken(value: unknown, sourceBytes: number): IntentSourceToken {
  const token = rpcRecord(value, "Structured Source token");
  rpcExactKeys(token, ["id", "start", "end", "target"], "Structured Source token");
  const start = rpcSafeUint(token.start, sourceBytes, "source token start");
  const end = rpcSafeUint(token.end, sourceBytes, "source token end");
  if (end < start) {
    throw new IntentRpcProtocolError("Structured Source token end precedes its start");
  }
  return {
    id: rpcSafeUint(token.id, 0xffff_ffff, "source token ID"),
    start,
    end,
    target: parseSourceTokenTarget(token.target),
  };
}

function parseSourceTokenTarget(value: unknown): IntentSourceTokenTarget {
  const target = rpcRecord(value, "Structured Source token target");
  const tag = rpcEnum(
    target.target,
    ["node_name", "suppressed", "definition", "instance"] as const,
    "source token target",
  );
  switch (tag) {
    case "node_name":
    case "suppressed":
      rpcExactKeys(target, ["target", "node"], "node source token target");
      return { target: tag, node: rpcHex(target.node, 16, "source token node ID") };
    case "definition":
      rpcExactKeys(target, ["target", "node", "field"], "definition source token target");
      return {
        target: tag,
        node: rpcHex(target.node, 16, "source token node ID"),
        field: rpcKey(target.field, "source token definition field"),
      };
    case "instance":
      rpcExactKeys(target, ["target", "leaf"], "instance source token target");
      return { target: tag, leaf: rpcLeaf(target.leaf) };
  }
}

function parseHistory(value: unknown): IntentHistoryProjection {
  const history = rpcRecord(value, "intent history projection");
  rpcExactKeys(history, ["applied", "redoable"], "intent history projection");
  return {
    applied: rpcArray(history.applied, 1_024, "applied intent history")
      .map(parseTransactionDescriptor),
    redoable: rpcArray(history.redoable, 1_024, "redoable intent history")
      .map(parseTransactionDescriptor),
  };
}

function parseTransactionDescriptor(value: unknown): IntentTransactionDescriptor {
  const transaction = rpcRecord(value, "intent transaction descriptor");
  rpcExactKeys(transaction, [
    "target_revision",
    "disposition",
    "operation_kinds",
    "affected_nodes",
    "diff",
  ], "intent transaction descriptor");
  return {
    target_revision: rpcHex(transaction.target_revision, 16, "transaction target revision"),
    disposition: rpcEnum(transaction.disposition, PLAN_DISPOSITIONS, "transaction disposition"),
    operation_kinds: rpcArray(
      transaction.operation_kinds,
      MAX_INTENT_PATCH_OPERATIONS,
      "transaction operation kinds",
    ).map((kind) => rpcEnum(kind, PATCH_OPERATION_KINDS, "patch operation kind")),
    affected_nodes: parseNodeIds(transaction.affected_nodes, "transaction affected nodes"),
    diff: parseSemanticDiff(transaction.diff),
  };
}

function parseSemanticDiff(value: unknown): IntentSemanticDiff {
  const diff = rpcRecord(value, "intent semantic diff");
  rpcExactKeys(diff, [
    "created_nodes",
    "deleted_nodes",
    "definition_nodes",
    "instance_nodes",
    "organization_nodes",
    "graph_changed",
    "instance_changed",
    "organization_changed",
    "external_inputs_changed",
  ], "intent semantic diff");
  return {
    created_nodes: parseNodeIds(diff.created_nodes, "created nodes"),
    deleted_nodes: parseNodeIds(diff.deleted_nodes, "deleted nodes"),
    definition_nodes: parseNodeIds(diff.definition_nodes, "definition nodes"),
    instance_nodes: parseNodeIds(diff.instance_nodes, "instance nodes"),
    organization_nodes: parseNodeIds(diff.organization_nodes, "organization nodes"),
    graph_changed: rpcBoolean(diff.graph_changed, "graph changed flag"),
    instance_changed: rpcBoolean(diff.instance_changed, "instance changed flag"),
    organization_changed: rpcBoolean(diff.organization_changed, "organization changed flag"),
    external_inputs_changed: rpcBoolean(
      diff.external_inputs_changed,
      "external inputs changed flag",
    ),
  };
}

function parseInspector<S extends string>(
  owner: SessionRef<S>,
  value: unknown,
): IntentInspectorProjection<S> {
  const inspector = rpcRecord(value, "intent Inspector projection");
  rpcExactKeys(inspector, [
    "identity",
    "node",
    "symbol",
    "name",
    "kind",
    "suppressed",
    "retained_failure",
    "inputs",
    "descriptor",
    "fields",
  ], "intent Inspector projection");
  const inputs = rpcArray(
    inspector.inputs,
    MAX_INTENT_NODE_COMPONENTS,
    "Inspector inputs",
  ).map((candidate) => {
    const input = rpcRecord(candidate, "Inspector input");
    rpcExactKeys(input, ["path", "source"], "Inspector input");
    return {
      path: parseProjectionPath(input.path, "Inspector input path"),
      source: parseProjectedPortReference(input.source),
    };
  });
  assertUnambiguousProjectionPaths(
    inputs.map((input) => input.path),
    "Inspector input paths",
  );
  const descriptor = parseDeclarationDescriptor(inspector.descriptor);
  assertInspectorInputsMatchDescriptor(inputs, descriptor.inputs);
  return own(owner.id, {
    identity: parseRpcIdentity(owner, inspector.identity, "Inspector projection identity"),
    node: rpcHex(inspector.node, 16, "Inspector node ID"),
    symbol: rpcKey(inspector.symbol, "Inspector symbol"),
    name: rpcKey(inspector.name, "Inspector name"),
    kind: parseGraphNodeKind(inspector.kind),
    suppressed: rpcBoolean(inspector.suppressed, "Inspector suppression flag"),
    retained_failure: rpcBoolean(inspector.retained_failure, "Inspector retained failure flag"),
    inputs,
    descriptor,
    fields: rpcArray(inspector.fields, 16_384, "Inspector fields").map(parseInspectorField),
  }) as unknown as IntentInspectorProjection<S>;
}

function assertInspectorInputsMatchDescriptor(
  inputs: readonly IntentInspectorInput[],
  descriptors: readonly IntentInputDescriptor[],
): void {
  if (inputs.length !== descriptors.length) {
    throw new IntentRpcProtocolError("Inspector inputs do not match descriptor inputs");
  }
  const descriptorPaths = new Set(descriptors.map((descriptor) =>
    projectionPathKey(descriptor.path)));
  if (inputs.some((input) => !descriptorPaths.has(projectionPathKey(input.path)))) {
    throw new IntentRpcProtocolError("Inspector inputs do not match descriptor inputs");
  }
}

function parseInspectorField(value: unknown): IntentInspectorField {
  const field = rpcRecord(value, "intent Inspector field");
  const tag = rpcEnum(field.field, ["definition", "instance"] as const, "Inspector field tag");
  if (tag === "definition") {
    rpcExactKeys(field, ["field", "definition", "value"], "Inspector definition field");
    return {
      field: tag,
      definition: rpcKey(field.definition, "Inspector definition field key"),
      value: field.value === null ? null : parseRpcLiteral(field.value),
    };
  }
  rpcExactKeys(field, ["field", "leaf", "value"], "Inspector instance field");
  return {
    field: tag,
    leaf: rpcLeaf(field.leaf),
    value: field.value === null ? null : parseRpcLiteral(field.value),
  };
}

function parseValidationEvidence(value: unknown): IntentValidationEvidence {
  const evidence = rpcRecord(value, "intent validation evidence");
  rpcExactKeys(evidence, [
    "semantic",
    "document",
    "point_count",
    "curve_count",
    "constraint_count",
    "hard_residuals_validated",
    "maximum_normalized_hard_residual",
    "feature_document",
    "feature_revision",
    "feature_digest",
    "feature_count",
    "computed_edge_count",
    "all_active_features_current",
  ], "intent validation evidence");
  const maximumResidual = evidence.maximum_normalized_hard_residual === null
    ? null
    : rpcFinite(
      evidence.maximum_normalized_hard_residual,
      "maximum normalized hard residual",
    );
  return {
    semantic: parseSemanticIdentity(evidence.semantic),
    document: rpcHex(evidence.document, 32, "accepted document ID"),
    point_count: rpcCount(evidence.point_count, "accepted point count"),
    curve_count: rpcCount(evidence.curve_count, "accepted curve count"),
    constraint_count: rpcCount(evidence.constraint_count, "accepted constraint count"),
    hard_residuals_validated: rpcBoolean(
      evidence.hard_residuals_validated,
      "hard residual validation flag",
    ),
    maximum_normalized_hard_residual: maximumResidual,
    feature_document: rpcHex(evidence.feature_document, 32, "computed feature document ID"),
    feature_revision: rpcWireUint(evidence.feature_revision, "computed feature revision"),
    feature_digest: rpcHex(evidence.feature_digest, 64, "computed feature digest"),
    feature_count: rpcCount(evidence.feature_count, "computed feature count"),
    computed_edge_count: rpcCount(evidence.computed_edge_count, "computed edge count"),
    all_active_features_current: rpcBoolean(
      evidence.all_active_features_current,
      "active feature current flag",
    ),
  };
}

function parseSemanticIdentity(value: unknown): IntentSemanticIdentity {
  const semantic = rpcRecord(value, "intent semantic identity");
  rpcExactKeys(
    semantic,
    ["graph", "instance", "reservations", "external_inputs"],
    "intent semantic identity",
  );
  return {
    graph: parseComponentIdentity(semantic.graph, "semantic graph identity"),
    instance: parseComponentIdentity(semantic.instance, "semantic instance identity"),
    reservations: parseComponentIdentity(
      semantic.reservations,
      "semantic reservations identity",
    ),
    external_inputs: parseComponentIdentity(
      semantic.external_inputs,
      "semantic external-input identity",
    ),
  };
}

function parseAliasMap<S extends string>(owner: SessionRef<S>, value: unknown): IntentAliasMap<S> {
  const aliases = rpcRecord(value, "intent alias map");
  rpcExactKeys(aliases, ["nodes", "ports", "cells"], "intent alias map");
  const nodes = parseStringMap(
    aliases.nodes,
    MAX_INTENT_PATCH_OPERATIONS,
    "node aliases",
    (candidate) => rpcHex(candidate, 16, "aliased node ID"),
  );
  const ports = parseStringMap(
    aliases.ports,
    MAX_INTENT_PATCH_OPERATIONS,
    "port aliases",
    (candidate) => {
      const selectors = rpcRecord(candidate, "aliased port selectors");
      return parseStringMap(
        selectors,
        MAX_INTENT_DECLARATION_OUTPUTS,
        "aliased port selectors",
        (port, selector) => {
          rpcPortSelector(selector);
          return parsePortReference(port);
        },
      ) as Readonly<Record<IntentPortSelector, IntentPortReference>>;
    },
  );
  const cells = parseStringMap(
    aliases.cells,
    MAX_INTENT_PATCH_OPERATIONS,
    "cell aliases",
    (candidate) => rpcHex(candidate, 16, "aliased cell ID"),
  );
  return own(owner.id, { nodes, ports, cells }) as IntentAliasMap<S>;
}

function parsePortReference(value: unknown): IntentPortReference {
  const reference = rpcRecord(value, "intent port reference");
  rpcExactKeys(reference, ["node", "port", "kind"], "intent port reference");
  return {
    node: rpcHex(reference.node, 16, "port owner node ID"),
    port: rpcHex(reference.port, 16, "intent port ID"),
    kind: rpcEnum(reference.kind, PORT_KINDS, "intent port kind"),
  };
}

function parseNodeIds(value: unknown, label: string): readonly string[] {
  return rpcArray(value, MAX_INTENT_GRAPH_NODES, label).map((candidate) =>
    rpcHex(candidate, 16, "intent node ID"));
}

function parseStringMap<T>(
  value: unknown,
  maximum: number,
  label: string,
  parse: (candidate: unknown, key: string) => T,
): Readonly<Record<string, T>> {
  const map = rpcRecord(value, label);
  const entries = Object.entries(map);
  if (entries.length > maximum) {
    throw new IntentRpcProtocolError(`${label} exceeds ${maximum} entries`);
  }
  const result: Record<string, T> = {};
  for (const [key, candidate] of entries) {
    rpcKey(key, `${label} key`);
    Object.defineProperty(result, key, {
      value: parse(candidate, key),
      enumerable: true,
      configurable: true,
      writable: true,
    });
  }
  return result;
}

function rpcRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new IntentRpcProtocolError(`${label} must be an object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new IntentRpcProtocolError(`${label} must be a plain object`);
  }
  return value as Record<string, unknown>;
}

function rpcExactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
  label: string,
): void {
  const actual = Object.keys(value).sort(compareStrings);
  const wanted = [...expected].sort(compareStrings);
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    throw new IntentRpcProtocolError(`${label} has unknown or missing fields`);
  }
}

function rpcArray(value: unknown, maximum: number, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new IntentRpcProtocolError(`${label} must be an array`);
  }
  if (value.length > maximum) {
    throw new IntentRpcProtocolError(`${label} exceeds ${maximum} entries`);
  }
  return value;
}

function rpcString(value: unknown, label: string): string {
  if (typeof value !== "string" || hasUnpairedSurrogate(value)) {
    throw new IntentRpcProtocolError(`${label} must be a valid Unicode string`);
  }
  return value;
}

function rpcBoundedString(value: unknown, maximumBytes: number, label: string): string {
  const result = rpcString(value, label);
  if (new TextEncoder().encode(result).length > maximumBytes) {
    throw new IntentRpcProtocolError(`${label} exceeds ${maximumBytes} bytes`);
  }
  return result;
}

function rpcKey(value: unknown, label: string): string {
  const result = rpcString(value, label);
  try {
    requireKey(result, label);
  } catch {
    throw new IntentRpcProtocolError(`${label} is not a valid bounded intent key`);
  }
  return result;
}

function rpcHex(value: unknown, width: number, label: string): string {
  const result = rpcString(value, label);
  if (result.length !== width || !/^[0-9a-f]+$/u.test(result)) {
    throw new IntentRpcProtocolError(
      `${label} must be exactly ${width} lowercase hexadecimal characters`,
    );
  }
  return result;
}

function rpcBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") {
    throw new IntentRpcProtocolError(`${label} must be a boolean`);
  }
  return value;
}

function rpcFinite(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new IntentRpcProtocolError(`${label} must be a finite number`);
  }
  return value;
}

function rpcSafeUint(value: unknown, maximum: number, label: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > maximum) {
    throw new IntentRpcProtocolError(`${label} must be an exactly represented unsigned integer`);
  }
  return value as number;
}

function rpcCount(value: unknown, label: string): number {
  return rpcSafeUint(value, Number.MAX_SAFE_INTEGER, label);
}

function rpcWireUint(value: unknown, label: string): IntentWireInteger {
  return rpcWireInteger(value, 0n, U64_MAX, label);
}

function rpcWireInteger(
  value: unknown,
  minimum: bigint,
  maximum: bigint,
  label: string,
): IntentWireInteger {
  if (typeof value === "bigint") {
    if (value < minimum || value > maximum) {
      throw new IntentRpcProtocolError(`${label} is outside the Rust integer range`);
    }
    return value;
  }
  if (!Number.isSafeInteger(value)) {
    throw new IntentRpcProtocolError(
      `${label} cannot be represented exactly by the JavaScript JSON transport`,
    );
  }
  const integer = BigInt(value as number);
  if (integer < minimum || integer > maximum) {
    throw new IntentRpcProtocolError(`${label} is outside the Rust integer range`);
  }
  return value as number;
}

function rpcEnum<const T extends readonly string[]>(
  value: unknown,
  allowed: T,
  label: string,
): T[number] {
  const result = rpcString(value, label);
  if (!(allowed as readonly string[]).includes(result)) {
    throw new IntentRpcProtocolError(`${label} has unknown value ${JSON.stringify(result)}`);
  }
  return result as T[number];
}

function rpcInputSlot(value: unknown): InputSlot {
  const result = rpcString(value, "intent input slot");
  try {
    validateInputSlot(result);
  } catch {
    throw new IntentRpcProtocolError("invalid Rust intent input slot");
  }
  return result;
}

function rpcPortSelector(value: unknown): IntentPortSelector {
  const result = rpcString(value, "intent port selector");
  try {
    validatePortSelector(result);
  } catch {
    throw new IntentRpcProtocolError("invalid Rust intent port selector");
  }
  return result;
}

function rpcLeaf(value: unknown): string {
  const result = rpcString(value, "intent writable leaf");
  const match = /^([0-9a-f]{16}):([0-9a-f]{16}):(x|y|value|angle|weight|parameter)$/u.exec(result);
  if (match === null) {
    throw new IntentRpcProtocolError("invalid Rust intent writable leaf");
  }
  return result;
}

function operation<S extends string, T extends object>(
  owner: SessionRef<S>,
  value: T,
): IntentOperation<S> {
  return own(owner.id, value) as unknown as IntentOperation<S>;
}

function own<T extends object>(owner: string, value: T): T {
  Object.defineProperty(value, runtimeSession, { value: owner, enumerable: false });
  return value;
}

function ownPort<T extends object>(owner: string, kind: PortKind, value: T): T {
  own(owner, value);
  Object.defineProperty(value, runtimePortKind, { value: kind, enumerable: false });
  return value;
}

function ownerOf(value: unknown): string | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  return (value as { readonly [runtimeSession]?: string })[runtimeSession];
}

function requireOwner<S extends string>(owner: SessionRef<S>, value: unknown): void {
  if (ownerOf(value) !== owner.id) {
    throw new TypeError(`cross-session intent reference: expected ${owner.id}`);
  }
}

function getPortKind(value: AnyPortRef<string>): PortKind {
  const kind = (value as { readonly [runtimePortKind]?: PortKind })[runtimePortKind];
  if (kind === undefined) throw new TypeError("unowned intent port reference");
  return kind;
}

function checkedComponent(value: ComponentIdentity, label: string): ComponentIdentity {
  assertExactKeys(value, ["revision", "digest"], label);
  requireHex(value.revision, 16, `${label} revision`);
  requireHex(value.digest, 64, `${label} digest`);
  return { revision: value.revision, digest: value.digest };
}

function assertExactKeys(value: object, expected: readonly string[], label: string): void {
  const actual = Object.keys(value).sort(compareStrings);
  const wanted = [...expected].sort(compareStrings);
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    throw new TypeError(`${label} has unknown or missing fields`);
  }
}

function requireHex(value: string, width: number, label: string): void {
  if (value.length !== width || !/^[0-9a-f]+$/u.test(value)) {
    throw new TypeError(`${label} must be exactly ${width} lowercase hexadecimal characters`);
  }
}

function requireKey(value: string, label: string): void {
  if (
    value.length === 0 ||
    new TextEncoder().encode(value).length > 256 ||
    hasUnpairedSurrogate(value) ||
    value.trim() !== value ||
    /\p{Cc}/u.test(value)
  ) {
    throw new TypeError(
      `${label} is empty, excessive, padded, contains invalid Unicode, or contains a control character`,
    );
  }
}

function hasUnpairedSurrogate(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      if (index + 1 >= value.length) return true;
      const next = value.charCodeAt(index + 1);
      if (next < 0xdc00 || next > 0xdfff) return true;
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      return true;
    }
  }
  return false;
}

function requireU16(value: number, label: string): void {
  if (!Number.isInteger(value) || value < 0 || value > 0xffff) {
    throw new RangeError(`${label} must be an unsigned 16-bit integer`);
  }
}

function u16Hex(value: number, label: string): string {
  requireU16(value, label);
  return value.toString(16).padStart(4, "0");
}

function validateInputSlot(value: string): asserts value is InputSlot {
  const match = /^(point|contact|curve|span|scalar|constraint|dimension|feature|profile|chain|parameter|external|source|catalog|identity):([0-9a-f]{4})$/u.exec(value);
  if (match === null) throw new TypeError("invalid Rust intent input slot");
}

function validatePortSelector(value: string): asserts value is IntentPortSelector {
  const node = /^node:([a-z_]+):([0-9a-f]{4})$/u.exec(value);
  const child = /^child:([0-9a-f]{4}):([a-z_]+):([0-9a-f]{4})$/u.exec(value);
  const role = node?.[1] ?? child?.[2];
  if (role === undefined || !(PORT_ROLES as readonly string[]).includes(role)) {
    throw new TypeError("invalid Rust intent port selector");
  }
}

function validateNodeKind(value: IntentNodeKind): void {
  switch (value.family) {
    case "geometry":
    case "constraint":
    case "dimension":
    case "operation":
    case "computed_feature":
    case "parameter":
    case "external":
    case "aggregate":
    case "annotation":
      return;
    case "bootstrap":
      requireKey(value.object.codec, "bootstrap codec");
      validateBytes(value.object.payload, "bootstrap payload");
      return;
    case "identity":
      return;
  }
}

function validateLiteral(value: IntentLiteral): void {
  switch (value.kind) {
    case "boolean":
      return;
    case "integer":
      if (!isWireInteger(value.value, I64_MIN, I64_MAX)) {
        throw new RangeError("integer intent literals must fit Rust i64");
      }
      return;
    case "natural":
      if (!isWireInteger(value.value, 0n, U64_MAX)) {
        throw new RangeError("natural intent literals must fit Rust u64");
      }
      return;
    case "text":
    case "enum":
      requireKey(value.value, `${value.kind} literal`);
      return;
    case "point":
      requireFinite(value.value[0]);
      requireFinite(value.value[1]);
      return;
    case "quantity":
      requireFinite(value.value.value);
  }
}

function validateOperationOutput(value: IntentOperationOutput): void {
  if (!(OPERATION_OUTPUT_KINDS as readonly string[]).includes(value.kind)) {
    throw new TypeError("invalid intent operation output kind");
  }
  requireU16(value.curve_span_count, "operation curve span count");
  if ((value.kind === "curve") !== (value.curve_span_count > 0)) {
    throw new TypeError("only curve operation outputs may own a positive span count");
  }
}

function requireFinite(value: number): void {
  if (!Number.isFinite(value)) throw new TypeError("intent numbers must be finite");
}

function isWireInteger(value: number | bigint, minimum: bigint, maximum: bigint): boolean {
  if (typeof value === "number" && !Number.isSafeInteger(value)) return false;
  const integer = typeof value === "bigint" ? value : BigInt(value);
  return integer >= minimum && integer <= maximum;
}

function validateBytes(values: readonly number[], label: string): number[] {
  const result = [...values];
  if (result.length > 16 * 1024 * 1024) {
    throw new RangeError(`${label} exceeds the Rust opaque-component bound`);
  }
  if (result.some((value) => !Number.isInteger(value) || value < 0 || value > 255)) {
    throw new RangeError(`${label} must contain only bytes`);
  }
  return result;
}

function normalizeInitialInstance(
  value: DraftOptions<string>["initialInstance"],
): Record<string, Partial<Record<LeafField, QuantityLiteral>>> {
  const result: Record<string, Partial<Record<LeafField, QuantityLiteral>>> = {};
  for (const [selector, leaves] of Object.entries(value ?? {}).sort(([left], [right]) =>
    compareSelectors(left, right)
  )) {
    validatePortSelector(selector);
    const checked: Partial<Record<LeafField, QuantityLiteral>> = {};
    for (const field of LEAF_FIELDS) {
      const literal = leaves?.[field];
      if (literal !== undefined) {
        validateLiteral(literal);
        checked[field] = literal;
      }
    }
    result[selector] = checked;
  }
  return result;
}

function portKindForRole(role: InputRole): PortKind | undefined {
  const exact: Partial<Record<InputRole, PortKind>> = {
    point: "point",
    contact: "contact",
    curve: "curve",
    span: "curve_span",
    scalar: "scalar",
    constraint: "constraint",
    dimension: "dimension",
    feature: "feature",
    profile: "profile",
    chain: "chain",
    parameter: "parameter",
    external: "external_binding",
    source: "source",
    catalog: "semantic_catalog",
  };
  return exact[role];
}

function encodeSessionIdentity(value: IntentSessionIdentityWire): string {
  return `{"session":${quote(value.session)},"revision":${quote(value.revision)},"digest":${quote(value.digest)},"graph":${encodeComponent(value.graph)},"instance":${encodeComponent(value.instance)},"reservations":${encodeComponent(value.reservations)},"organization":${encodeComponent(value.organization)},"external_inputs":${encodeComponent(value.external_inputs)}}`;
}

function encodeComponent(value: ComponentIdentity): string {
  return `{"revision":${quote(value.revision)},"digest":${quote(value.digest)}}`;
}

function encodeNodeKind(value: IntentNodeKind): string {
  switch (value.family) {
    case "geometry":
      return `{"family":"geometry","recipe":${quote(value.recipe)}}`;
    case "constraint":
      return `{"family":"constraint","constraint":${quote(value.constraint)}}`;
    case "dimension":
      return `{"family":"dimension","dimension":${quote(value.dimension)}}`;
    case "operation":
      return `{"family":"operation","operation":${quote(value.operation)}}`;
    case "computed_feature":
      return `{"family":"computed_feature","feature":${quote(value.feature)}}`;
    case "parameter":
      return `{"family":"parameter","parameter":${quote(value.parameter)}}`;
    case "external":
      return `{"family":"external","external":${quote(value.external)}}`;
    case "aggregate":
      return `{"family":"aggregate","aggregate":${quote(value.aggregate)}}`;
    case "bootstrap":
      return `{"family":"bootstrap","object":{"kind":${quote(value.object.kind)},"codec":${quote(value.object.codec)},"payload":[${value.object.payload.join(",")}]}}`;
    case "annotation":
      return "{\"family\":\"annotation\"}";
    case "identity":
      return `{"family":"identity","transition":${quote(value.transition)},"port_kind":${quote(value.port_kind)}}`;
  }
}

function encodeLiteral(value: IntentLiteral): string {
  validateLiteral(value);
  switch (value.kind) {
    case "boolean":
      return `{"kind":"boolean","value":${String(value.value)}}`;
    case "integer":
      return `{"kind":"integer","value":${String(value.value)}}`;
    case "natural":
      return `{"kind":"natural","value":${String(value.value)}}`;
    case "text":
      return `{"kind":"text","value":${quote(value.value)}}`;
    case "enum":
      return `{"kind":"enum","value":${quote(value.value)}}`;
    case "point":
      return `{"kind":"point","value":[${rustFloat(value.value[0])},${rustFloat(value.value[1])}]}`;
    case "quantity":
      return `{"kind":"quantity","value":{"value":${rustFloat(value.value.value)},"unit":${quote(value.value.unit)}}}`;
  }
}

function encodePortRef(value: AnyPortRef<string>): string {
  if (value.source === "stable") {
    return `{"source":"stable","port":{"node":${quote(value.port.node)},"port":${quote(value.port.port)},"kind":${quote(value.port.kind)}}}`;
  }
  return `{"source":"alias","node":${quote(value.node)},"selector":${quote(value.selector)}}`;
}

function encodeCellTarget(value: CellTarget<string>): string {
  return value.target === "stable"
    ? `{"target":"stable","cell":${quote(value.cell)}}`
    : `{"target":"alias","alias":${quote(value.alias)}}`;
}

function encodeDraft(value: IntentNodeDraft<string>): string {
  const inputs = Object.entries(value.inputs)
    .sort(([left], [right]) => compareInputSlots(left, right))
    .map(([slot, source]) => `${quote(slot)}:${encodePortRef(source as AnyPortRef<string>)}`)
    .join(",");
  const fields = sortedStringEntries(value.fields)
    .map(([key, literal]) => `${quote(key)}:${encodeLiteral(literal)}`)
    .join(",");
  const initial = Object.entries(value.initial_instance)
    .sort(([left], [right]) => compareSelectors(left, right))
    .map(([selector, leaves]) => {
      const encodedLeaves = LEAF_FIELDS.flatMap((field) => {
        const literal = leaves?.[field];
        return literal === undefined ? [] : [`${quote(field)}:${encodeLiteral(literal)}`];
      }).join(",");
      return `${quote(selector)}:{${encodedLeaves}}`;
    })
    .join(",");
  const operationOutputs = value.operation_outputs.length === 0
    ? ""
    : `,"operation_outputs":[${value.operation_outputs.map((output) => `{"kind":${quote(output.kind)},"curve_span_count":${output.curve_span_count}}`).join(",")}]`;
  return `{"kind":${encodeNodeKind(value.kind)},"symbol":${quote(value.symbol)},"name":${quote(value.name)},"inputs":{${inputs}},"fields":{${fields}},"initial_instance":{${initial}}${operationOutputs},"dynamic_children":${value.dynamic_children},"suppressed":${String(value.suppressed)}}`;
}

function encodeDeletePolicy(value: DeletePolicy): string {
  switch (value.policy) {
    case "reject_dependents":
      return "{\"policy\":\"reject_dependents\"}";
    case "cascade":
      return `{"policy":"cascade","exact_nodes":[${[...value.exact_nodes].sort(compareStrings).map(quote).join(",")}]}`;
    case "cascade_roots":
      return `{"policy":"cascade_roots","exact_roots":[${[...value.exact_roots].sort(compareStrings).map(quote).join(",")}],"exact_nodes":[${[...value.exact_nodes].sort(compareStrings).map(quote).join(",")}]}`;
  }
}

function encodeExternalInputs(value: ExternalInputs): string {
  return `{"revision":${quote(value.revision)},"parameter_batch":[${value.parameter_batch.join(",")}],"external_snapshots":[${value.external_snapshots.join(",")}]} `
    .trimEnd();
}

function encodeOperation(value: IntentOperation<string>): string {
  switch (value.operation) {
    case "create_node":
      return `{"operation":"create_node","alias":${quote(value.alias)},"draft":${encodeDraft(value.draft)},"cell":${value.cell === null ? "null" : encodeCellTarget(value.cell)}}`;
    case "delete_node":
      return `{"operation":"delete_node","node":${quote(value.node)},"policy":${encodeDeletePolicy(value.policy)}}`;
    case "set_suppressed":
      return `{"operation":"set_suppressed","node":${quote(value.node)},"suppressed":${String(value.suppressed)}}`;
    case "set_definition_field":
      return `{"operation":"set_definition_field","node":${quote(value.node)},"field":${quote(value.field)},"value":${encodeLiteral(value.value)}}`;
    case "set_instance_leaf":
      return `{"operation":"set_instance_leaf","leaf":${quote(value.leaf)},"value":${encodeLiteral(value.value)}}`;
    case "rebind_input":
      return `{"operation":"rebind_input","node":${quote(value.node)},"slot":${quote(value.slot)},"source":${encodePortRef(value.source)}}`;
    case "eject_bootstrap_point":
      return `{"operation":"eject_bootstrap_point","node":${quote(value.node)}}`;
    case "rename_node":
      return `{"operation":"rename_node","node":${quote(value.node)},"name":${quote(value.name)}}`;
    case "move_declaration":
      return `{"operation":"move_declaration","node":${quote(value.node)},"cell":${encodeCellTarget(value.cell)},"before":${value.before === null ? "null" : quote(value.before)}}`;
    case "create_cell":
      return `{"operation":"create_cell","alias":${quote(value.alias)},"name":${quote(value.name)},"before":${value.before === null ? "null" : quote(value.before)}}`;
    case "delete_cell":
      return `{"operation":"delete_cell","cell":${quote(value.cell)}}`;
    case "reorder_cells":
      return `{"operation":"reorder_cells","exact_order":[${value.exact_order.map(quote).join(",")}]}`;
    case "replace_external_inputs":
      return `{"operation":"replace_external_inputs","inputs":${encodeExternalInputs(value.inputs)}}`;
  }
}

function rustFloat(value: number): string {
  requireFinite(value);
  if (Object.is(value, -0)) return "-0.0";
  const magnitude = Math.abs(value);
  if (magnitude !== 0 && (magnitude < 1e-5 || magnitude >= 1e16)) {
    return value.toExponential();
  }
  const text = String(value);
  return /^-?\d+$/u.test(text) ? `${text}.0` : text;
}

function quote(value: string): string {
  return JSON.stringify(value);
}

function canonicalValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalValue);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      sortedStringEntries(value as Record<string, unknown>)
        .map(([key, child]) => [key, canonicalValue(child)]),
    );
  }
  if (typeof value === "number" && !Number.isFinite(value)) {
    throw new TypeError("intent numbers must be finite");
  }
  return value;
}

function sortedStringEntries<T>(value: Readonly<Record<string, T>>): Array<[string, T]> {
  return Object.entries(value).sort(([left], [right]) => compareStrings(left, right));
}

function compareStrings(left: string, right: string): number {
  const leftBytes = new TextEncoder().encode(left);
  const rightBytes = new TextEncoder().encode(right);
  const length = Math.min(leftBytes.length, rightBytes.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftBytes[index] ?? 0) - (rightBytes[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return leftBytes.length - rightBytes.length;
}

function compareInputSlots(left: string, right: string): number {
  const [leftRole = "", leftIndex = ""] = left.split(":");
  const [rightRole = "", rightIndex = ""] = right.split(":");
  const role = INPUT_ROLES.indexOf(leftRole as InputRole) - INPUT_ROLES.indexOf(rightRole as InputRole);
  return role === 0 ? compareStrings(leftIndex, rightIndex) : role;
}

function compareSelectors(left: string, right: string): number {
  const leftParts = left.split(":");
  const rightParts = right.split(":");
  if (leftParts[0] !== rightParts[0]) return leftParts[0] === "node" ? -1 : 1;
  if (leftParts[0] === "node") {
    const role = PORT_ROLES.indexOf(leftParts[1] as PortRole) - PORT_ROLES.indexOf(rightParts[1] as PortRole);
    return role === 0 ? compareStrings(leftParts[2] ?? "", rightParts[2] ?? "") : role;
  }
  const ordinal = compareStrings(leftParts[1] ?? "", rightParts[1] ?? "");
  if (ordinal !== 0) return ordinal;
  const role = PORT_ROLES.indexOf(leftParts[2] as PortRole) - PORT_ROLES.indexOf(rightParts[2] as PortRole);
  return role === 0 ? compareStrings(leftParts[3] ?? "", rightParts[3] ?? "") : role;
}

const GEOMETRY_RECIPES = [
  "sketch_point",
  "segment",
  "polyline",
  "midpoint_line",
  "two_point_aligned_rectangle",
  "three_point_corner_rectangle",
  "center_rectangle",
  "three_point_center_rectangle",
  "center_radius_circle",
  "two_point_diameter_circle",
  "three_point_circle",
  "center_arc",
  "three_point_arc",
  "tangent_arc",
  "center_axes_ellipse",
  "axis_endpoints_ellipse",
  "center_axes_elliptical_arc",
  "axis_endpoints_elliptical_arc",
  "quadratic_bezier",
  "cubic_bezier",
  "rational_quadratic_conic",
  "parabola",
  "hyperbola",
  "open_control_nurbs",
  "periodic_control_nurbs",
] as const satisfies readonly GeometryRecipe[];

const CONSTRAINT_KINDS = [
  "fixed_point",
  "fixed_coordinate",
  "coincident_with_origin",
  "point_on_datum_axis",
  "coincident",
  "external_point_coincident",
  "horizontal",
  "vertical",
  "horizontal_points",
  "vertical_points",
  "horizontal_point_to_midpoint",
  "vertical_point_to_midpoint",
  "point_on_curve",
  "parallel",
  "perpendicular",
  "external_line_collinear",
  "collinear_with_datum_axis",
  "concentric",
  "collinear",
  "equal_length",
  "equal_radius",
  "midpoint",
  "symmetric_about_line",
  "symmetric_about_datum_axis",
  "line_circle_tangency",
  "circle_circle_tangency",
  "circle_arc_tangency",
  "line_curve_tangency",
  "curve_curve_contact",
  "curve_curve_tangency",
  "curve_direction",
  "equal_curvature",
  "endpoint_continuity",
  "line_line_fillet",
  "curve_curve_fillet",
] as const satisfies readonly ConstraintKind[];

const DIMENSION_KINDS = [
  "point_distance",
  "curve_length",
  "radius",
  "diameter",
  "oriented_angle",
  "supporting_line_offset",
  "exact_translated_segment_offset",
  "profile_offset",
] as const satisfies readonly DimensionKind[];

const OPERATION_KINDS = [
  "split",
  "break",
  "trim",
  "extend",
  "mirror",
  "chamfer",
  "associative_fillet",
  "rectangle",
  "regular_polygon",
  "slot",
  "linear_pattern",
  "profile_offset",
] as const satisfies readonly OperationKind[];

const OPERATION_OUTPUT_KINDS = [
  "point",
  "scalar",
  "curve",
  "contact",
  "constraint",
  "dimension",
  "parameter",
  "external_binding",
] as const satisfies readonly IntentOperationOutputKind[];

const CHILD_SCHEMAS = [
  "none",
  "polyline_vertex",
  "spline_control",
  "fillet_corner",
  "pattern_instance",
] as const satisfies readonly IntentChildSchema[];

const BOOTSTRAP_NATIVE_KINDS = [
  "document",
  "point",
  "scalar",
  "curve",
  "contact",
  "constraint",
  "dimension",
  "parameter",
  "external_binding",
  "semantic_catalog",
  "semantic_source",
  "curve_trim_view",
  "geometry_role",
  "parameter_binding",
  "parameter_output",
  "computed_feature",
  "annotation_placement",
] as const satisfies readonly BootstrapNativeKind[];

const PORT_KINDS = [
  "point",
  "handle_point",
  "scalar",
  "curve",
  "curve_span",
  "contact",
  "constraint",
  "dimension",
  "source",
  "parameter",
  "parameter_binding",
  "parameter_output",
  "external_binding",
  "semantic_catalog",
  "profile",
  "chain",
  "operation",
  "feature",
  "feature_corner",
  "annotation",
  "collection",
] as const satisfies readonly PortKind[];

const NATIVE_RESERVATION_KINDS = [
  "point",
  "scalar",
  "curve",
  "contact",
  "constraint",
  "constraint_source",
  "dimension",
  "dimension_source",
  "parameter",
  "external_binding",
  "semantic_catalog",
  "semantic_source",
] as const satisfies readonly IntentNativeReservationKind[];

const NODE_FAMILIES = [
  "geometry",
  "constraint",
  "dimension",
  "operation",
  "computed_feature",
  "aggregate",
  "parameter",
  "external",
  "bootstrap",
  "annotation",
  "identity",
] as const;

const AGGREGATE_KINDS = ["open_chain", "closed_profile"] as const;
const PARAMETER_KINDS = ["parameter", "binding", "output"] as const;
const EXTERNAL_KINDS = ["binding", "snapshot_reference"] as const;
const IDENTITY_TRANSITIONS = ["alias", "continue", "retire"] as const;
const IDENTITY_FLOW_STATES = [
  "owned_logical",
  "created",
  "aliased",
  "continued",
  "retired",
] as const;
const EDIT_CLASSIFICATIONS = [
  "definition",
  "instance",
  "input_binding",
  "organization",
  "read_only",
] as const satisfies readonly IntentEditClassification[];
const LITERAL_KINDS = [
  "boolean",
  "integer",
  "natural",
  "text",
  "enum",
  "point",
  "quantity",
] as const;
const INTENT_UNITS = ["length", "angle", "dimensionless"] as const satisfies readonly IntentUnit[];
const FIELD_DEFAULTS = ["required", "literal", "conditional", "contextual"] as const;
const FIELD_CHOICES = ["not_applicable", "closed", "contextual"] as const;
const PLAN_DISPOSITIONS = [
  "accepted",
  "retained_failed",
  "organization_only",
] as const satisfies readonly IntentPlanDisposition[];
const PATCH_OPERATION_KINDS = [
  "create_node",
  "delete_node",
  "set_suppressed",
  "set_definition_field",
  "set_instance_leaf",
  "rebind_input",
  "eject_bootstrap_point",
  "rename_node",
  "move_declaration",
  "create_cell",
  "delete_cell",
  "reorder_cells",
  "replace_external_inputs",
] as const satisfies readonly IntentPatchOperationKind[];
const RPC_FAILURE_CODES = [
  "invalid_request",
  "request_too_large",
  "receipt_too_large",
  "response_too_large",
  "patch_rejected",
  "session_rejected",
  "materialization_rejected",
  "native_rejected",
  "document_rejected",
  "coordinator_rejected",
  "source_edit_rejected",
  "editor_rejected",
  "workbench_unavailable",
  "workbench_busy",
  "workbench_surface_unavailable",
  "code_authority_required",
] as const satisfies readonly IntentRpcFailureCode[];

const INPUT_ROLES: readonly InputRole[] = [
  "point", "contact", "curve", "span", "scalar", "constraint", "dimension", "feature",
  "profile", "chain", "parameter", "external", "source", "catalog", "identity",
];

const PORT_ROLES: readonly PortRole[] = [
  "primary", "start", "end", "center", "midpoint", "corner", "control",
  "major_axis_point", "minor_axis_point", "curve", "span", "contact", "target",
  "constraint", "dimension", "source", "catalog", "operation", "feature",
  "feature_corner", "parameter", "binding", "output", "external", "annotation",
  "collection", "profile", "chain", "result",
];

const LEAF_FIELDS: readonly LeafField[] = [
  "x", "y", "value", "angle", "weight", "parameter",
];

const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;
const U64_MAX = (1n << 64n) - 1n;
