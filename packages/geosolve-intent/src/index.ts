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
  | { readonly method: "edit_source_token"; readonly token: number; readonly replacement: string };

/** The exact single-string surface implemented by WASM `IntentRpcHandle.apply`. */
export interface IntentRpcTransport {
  apply(canonicalRequestJson: string): string | Promise<string>;
}

export interface DraftOptions<S extends string> {
  readonly name?: string;
  readonly inputs?: readonly IntentInputBinding<S>[];
  readonly fields?: Readonly<Record<string, IntentLiteral>>;
  readonly initialInstance?: Readonly<
    Partial<Record<IntentPortSelector, Readonly<Partial<Record<LeafField, QuantityLiteral>>>>>
  >;
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
  const dynamicChildren = options.dynamicChildren ?? 0;
  requireU16(dynamicChildren, "dynamic child count");
  return own(owner.id, {
    kind,
    symbol,
    name,
    inputs,
    fields,
    initial_instance: initialInstance,
    dynamic_children: dynamicChildren,
    suppressed: options.suppressed ?? false,
  }) as IntentNodeDraft<S>;
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

  async apply(value: IntentPatch<S>): Promise<string> {
    requireOwner(this.owner, value);
    return this.transport.apply(encodeIntentRpcRequest({ method: "apply_patch", patch: value }));
  }

  async snapshot(): Promise<string> {
    return this.transport.apply(encodeIntentRpcRequest({ method: "snapshot" }));
  }

  async undo(): Promise<string> {
    return this.transport.apply(encodeIntentRpcRequest({ method: "undo" }));
  }

  async redo(): Promise<string> {
    return this.transport.apply(encodeIntentRpcRequest({ method: "redo" }));
  }

  async inspector(node: NodeRef<S>): Promise<string> {
    requireOwner(this.owner, node);
    return this.transport.apply(encodeIntentRpcRequest({ method: "inspector", node: node.id }));
  }

  async editSourceToken(token: number, replacement: string): Promise<string> {
    if (!Number.isInteger(token) || token < 0 || token > 0xffff_ffff) {
      throw new RangeError("source token must be an unsigned 32-bit integer");
    }
    return this.transport.apply(encodeIntentRpcRequest({
      method: "edit_source_token",
      token,
      replacement,
    }));
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
      return `{"method":"edit_source_token","token":${value.token},"replacement":${quote(value.replacement)}}`;
  }
}

/** Deterministic generic JSON helper for non-wire presentation data. */
export function canonicalStringify(value: unknown): string {
  return JSON.stringify(canonicalValue(value));
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
  return `{"kind":${encodeNodeKind(value.kind)},"symbol":${quote(value.symbol)},"name":${quote(value.name)},"inputs":{${inputs}},"fields":{${fields}},"initial_instance":{${initial}},"dynamic_children":${value.dynamic_children},"suppressed":${String(value.suppressed)}}`;
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
