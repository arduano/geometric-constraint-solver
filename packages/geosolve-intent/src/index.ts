// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Equation-free TypeScript vocabulary for the Rust-owned GeoSolve Design
 * Intent Graph. This package builds and transports typed patches; it never
 * evaluates geometry, constraints, residuals, or expressions.
 */

declare const sessionBrand: unique symbol;
declare const nodeBrand: unique symbol;
declare const portBrand: unique symbol;
declare const operationBrand: unique symbol;

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

export interface SessionRef<S extends string> {
  readonly id: S;
  readonly [sessionBrand]: S;
}

export interface NodeRef<S extends string> {
  readonly session: S;
  readonly id: string;
  readonly [nodeBrand]: S;
}

export interface PortRef<S extends string, K extends PortKind> {
  readonly session: S;
  readonly node: string;
  readonly port: string;
  readonly kind: K;
  readonly [portBrand]: readonly [S, K];
}

export interface AliasPortRef<S extends string, K extends PortKind> {
  readonly session: S;
  readonly alias: string;
  readonly selector: string;
  readonly kind: K;
  readonly [portBrand]: readonly [S, K];
}

export type AnyPortRef<S extends string, K extends PortKind = PortKind> =
  | PortRef<S, K>
  | AliasPortRef<S, K>;

export interface IntentInput<S extends string, R extends InputRole = InputRole> {
  readonly role: R;
  readonly index: number;
  readonly source: AnyPortRef<S, PortKindForRole<R>>;
}

export type IntentLiteral =
  | { readonly kind: "boolean"; readonly value: boolean }
  | { readonly kind: "integer"; readonly value: number }
  | { readonly kind: "natural"; readonly value: number }
  | { readonly kind: "text"; readonly value: string }
  | { readonly kind: "enum"; readonly value: string }
  | { readonly kind: "point"; readonly value: readonly [number, number] }
  | {
      readonly kind: "quantity";
      readonly value: number;
      readonly unit: "length" | "angle" | "dimensionless";
    };

export type DeclarationKind =
  | { readonly family: "geometry"; readonly recipe: GeometryRecipe }
  | { readonly family: "constraint"; readonly constraint: ConstraintKind }
  | { readonly family: "dimension"; readonly dimension: DimensionKind }
  | { readonly family: "operation"; readonly operation: OperationKind }
  | { readonly family: "computed_feature"; readonly feature: "fillet_set" }
  | {
      readonly family: "parameter";
      readonly parameter: "parameter" | "binding" | "output";
    }
  | {
      readonly family: "external";
      readonly external: "binding" | "snapshot_reference";
    }
  | { readonly family: "annotation" };

export interface NodeDraft<S extends string> {
  readonly session: S;
  readonly symbol: string;
  readonly displayName?: string;
  readonly declaration: DeclarationKind;
  readonly inputs?: readonly IntentInput<S>[];
  readonly fields?: Readonly<Record<string, IntentLiteral>>;
  readonly dynamicChildren?: number;
  readonly suppressed?: boolean;
}

export type PatchPolicy = "require_accepted" | "retain_failed_intent";

export type IntentOperation<S extends string> =
  | {
      readonly op: "create_node";
      readonly alias: string;
      readonly draft: NodeDraft<S>;
      readonly [operationBrand]: S;
    }
  | {
      readonly op: "delete_node";
      readonly node: NodeRef<S>;
      readonly exactCascade?: readonly NodeRef<S>[];
      readonly [operationBrand]: S;
    }
  | {
      readonly op: "set_suppressed";
      readonly node: NodeRef<S>;
      readonly suppressed: boolean;
      readonly [operationBrand]: S;
    }
  | {
      readonly op: "set_instance_leaf";
      readonly port: AnyPortRef<S, "point" | "scalar">;
      readonly field: "x" | "y" | "value" | "angle" | "weight" | "parameter";
      readonly value: number;
      readonly [operationBrand]: S;
    }
  | {
      readonly op: "rename_node";
      readonly node: NodeRef<S>;
      readonly displayName: string;
      readonly [operationBrand]: S;
    };

export interface IntentPatch<S extends string> {
  readonly expected: string;
  readonly policy: PatchPolicy;
  readonly operations: readonly IntentOperation<S>[];
  readonly session: S;
}

export interface IntentRpcTransport {
  request(method: "intent.apply", canonicalJson: string): Promise<string>;
}

export function session<const S extends string>(id: S): SessionRef<S> {
  requireKey(id, "session id");
  return { id } as SessionRef<S>;
}

export function stablePort<S extends string, K extends PortKind>(
  owner: SessionRef<S>,
  node: string,
  port: string,
  kind: K,
): PortRef<S, K> {
  requireKey(node, "node id");
  requireKey(port, "port id");
  return { session: owner.id, node, port, kind } as PortRef<S, K>;
}

export function aliasPort<S extends string, K extends PortKind>(
  owner: SessionRef<S>,
  alias: string,
  selector: string,
  kind: K,
): AliasPortRef<S, K> {
  requireKey(alias, "transaction alias");
  requireKey(selector, "port selector");
  return { session: owner.id, alias, selector, kind } as AliasPortRef<S, K>;
}

export function input<S extends string, R extends InputRole>(
  owner: SessionRef<S>,
  role: R,
  index: number,
  source: AnyPortRef<NoInfer<S>, PortKindForRole<R>>,
): IntentInput<S, R> {
  requireSession(owner, source);
  if (!Number.isInteger(index) || index < 0 || index > 0xffff) {
    throw new RangeError("input index must be an unsigned 16-bit integer");
  }
  const expected = portKindForRole(role);
  if (expected !== undefined && source.kind !== expected) {
    throw new TypeError(`input role ${role} requires ${expected}, received ${source.kind}`);
  }
  return { role, index, source };
}

export function draft<S extends string>(
  owner: SessionRef<S>,
  symbol: string,
  declaration: DeclarationKind,
  options: Omit<NodeDraft<NoInfer<S>>, "session" | "symbol" | "declaration"> = {},
): NodeDraft<S> {
  requireKey(symbol, "developer symbol");
  for (const binding of options.inputs ?? []) {
    requireSession(owner, binding.source);
  }
  return { session: owner.id, symbol, declaration, ...options };
}

export function createNode<S extends string>(
  owner: SessionRef<S>,
  alias: string,
  node: NodeDraft<NoInfer<S>>,
): IntentOperation<S> {
  requireKey(alias, "transaction alias");
  requireSession(owner, node);
  return { op: "create_node", alias, draft: node } as IntentOperation<S>;
}

export function patch<S extends string>(
  owner: SessionRef<S>,
  expected: string,
  policy: PatchPolicy,
  operations: readonly IntentOperation<NoInfer<S>>[],
): IntentPatch<S> {
  requireKey(expected, "expected session identity");
  if (operations.length === 0) {
    throw new RangeError("an intent patch must contain at least one operation");
  }
  for (const operation of operations) {
    requireOperationSession(owner, operation);
  }
  return { expected, policy, operations: [...operations], session: owner.id };
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
    requireSession(this.owner, value);
    return this.transport.request("intent.apply", canonicalStringify(value));
  }
}

export function canonicalStringify(value: unknown): string {
  return JSON.stringify(canonicalValue(value));
}

function canonicalValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(canonicalValue);
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, child]) => [key, canonicalValue(child)]),
    );
  }
  if (typeof value === "number" && !Number.isFinite(value)) {
    throw new TypeError("intent numbers must be finite");
  }
  return value;
}

function requireSession<S extends string>(
  owner: SessionRef<S>,
  value: { readonly session: string },
): void {
  if (value.session !== owner.id) {
    throw new TypeError(`cross-session intent reference: expected ${owner.id}`);
  }
}

function requireOperationSession<S extends string>(
  owner: SessionRef<S>,
  operation: IntentOperation<S>,
): void {
  switch (operation.op) {
    case "create_node":
      requireSession(owner, operation.draft);
      return;
    case "delete_node":
    case "set_suppressed":
    case "rename_node":
      requireSession(owner, operation.node);
      return;
    case "set_instance_leaf":
      requireSession(owner, operation.port);
  }
}

function requireKey(value: string, label: string): void {
  if (value.length === 0 || value.length > 256 || /[\u0000-\u001f]/u.test(value)) {
    throw new TypeError(`${label} is empty, excessive, or contains a control character`);
  }
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
