// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Public, equation-free GeoSolve sketch authoring vocabulary.
 *
 * Declaration IDs are caller-owned semantic identity. Lexical variable names
 * and presentation labels are deliberately separate from that identity.
 */

import {
  registerAuthoringSchema,
  registerPatchRuntime,
} from "./authoring-private.js";
import {
  AUTHORING_METHOD_CATALOG,
  DECLARATION_RESULT_CATALOG,
} from "./generated-declaration-results.js";

declare const projectBrand: unique symbol;
declare const referenceBrand: unique symbol;
declare const referenceKindBrand: unique symbol;
declare const featureOutputsBrand: unique symbol;
declare const nativeSpanBrand: unique symbol;
declare const collectionOwnerBrand: unique symbol;
declare const unitBrand: unique symbol;
declare const sketchBrand: unique symbol;
declare const patchBrand: unique symbol;

const referenceRuntime = Symbol("geosolve.authoring.reference");
const sketchRuntime = Symbol("geosolve.authoring.sketch");
const patchRuntime = Symbol("geosolve.authoring.patch");
const schemaRuntime = Symbol("geosolve.authoring.schema");

const SKETCH_PROJECT_NAME = "__geosolve_sketch_execution__" as const;
const PATCH_PROJECT_NAME = "__geosolve_patch_execution__" as const;

export type FeatureKind =
  | "point"
  | "curve"
  | "curve_span"
  | "scalar"
  | "contact"
  | "constraint"
  | "dimension"
  | "profile"
  | "chain"
  | "operation"
  | "feature"
  | "feature_corner"
  | "collection";

type SemanticMemberKey = string | number;
type SemanticPathSegment = SemanticMemberKey | { readonly member: SemanticMemberKey };

export interface SketchProject<Name extends string = string> {
  readonly name: Name;
  readonly [projectBrand]: (name: Name) => Name;
}

export interface OutputRef<Project, Kind extends FeatureKind> {
  readonly [referenceBrand]: (project: Project) => Project;
  readonly [referenceKindBrand]: Kind;
}

export type FeatureRef<Project, Kind extends FeatureKind, Outputs> =
  & OutputRef<Project, Kind>
  & Readonly<Outputs>
  & { readonly [featureOutputsBrand]: Outputs };

export type PointRef<Project> = OutputRef<Project, "point">;
export type PointLikeRef<Project> =
  | PointRef<Project>
  | FeatureCornerRef<Project>;
export type CurveRef<Project> = OutputRef<Project, "curve">;
export type CurveSpanRef<Project> = OutputRef<Project, "curve_span">;
export type NativeCurveSpanRef<Project> = CurveSpanRef<Project> & {
  readonly [nativeSpanBrand]: true;
};
export type ScalarRef<Project> = OutputRef<Project, "scalar">;
export type ContactRef<Project> = OutputRef<Project, "contact">;
export type ConstraintRef<Project> = OutputRef<Project, "constraint">;
export type DimensionRef<Project> = OutputRef<Project, "dimension">;
export type ProfileRef<Project> = OutputRef<Project, "profile">;
export type ChainRef<Project> = OutputRef<Project, "chain">;
export type OperationRef<Project> = OutputRef<Project, "operation">;
export type FeatureCornerRef<Project> = OutputRef<Project, "feature_corner">;

export type CurveLike<Project> =
  | CurveRef<Project>
  | { readonly curve: CurveRef<Project> };

export type SpanLike<Project> =
  | NativeCurveSpanRef<Project>
  | { readonly span: NativeCurveSpanRef<Project> };

export interface KeyedFeatureCollection<Key extends PropertyKey, Value> {
  readonly keys: readonly Key[];
  readonly byKey: Readonly<Record<Key, Value>>;
}

export type DerivedFeatureCollection<Owner, Key extends PropertyKey, Value> =
  & KeyedFeatureCollection<Key, Value>
  & { readonly [collectionOwnerBrand]: Owner };

export type FeatureRecord<RecordType extends Readonly<Record<PropertyKey, unknown>>> = {
  readonly [Key in keyof RecordType]: RecordType[Key];
};

export type LengthUnit = "mm" | "cm" | "m" | "inch";
export type AngleUnit = "deg" | "rad";
export type SupportedUnit = LengthUnit | AngleUnit;

export interface UnitLiteral<Unit extends SupportedUnit = SupportedUnit> {
  readonly unit: Unit;
  readonly value: number;
  readonly [unitBrand]: Unit;
}

export type Length = UnitLiteral<LengthUnit>;
export type Angle = UnitLiteral<AngleUnit>;
export type Point2 = readonly [x: number, y: number];

function unit<Unit extends SupportedUnit>(name: Unit, value: number): UnitLiteral<Unit> {
  requireFinite(value, `${name} value`);
  return Object.freeze({ unit: name, value }) as UnitLiteral<Unit>;
}

export const mm = (value: number): UnitLiteral<"mm"> => unit("mm", value);
export const cm = (value: number): UnitLiteral<"cm"> => unit("cm", value);
export const m = (value: number): UnitLiteral<"m"> => unit("m", value);
export const inch = (value: number): UnitLiteral<"inch"> => unit("inch", value);
export const deg = (value: number): UnitLiteral<"deg"> => unit("deg", value);
export const rad = (value: number): UnitLiteral<"rad"> => unit("rad", value);

export type GeometryRole = "profile" | "construction";
export type Axis = "x" | "y";
export type Direction = "forward" | "reverse";
export type Sweep = "counterClockwise" | "clockwise";
export type Side = "left" | "right";
export type Endpoint = "start" | "end";
export type DimensionMode = "driving" | "reference";

export interface PresentationOptions {
  /** Presentation only; never declaration identity. */
  readonly label?: string;
}

export interface SuppressibleOptions extends PresentationOptions {
  readonly suppressed?: boolean;
}

export type PointInput<Project> = PointLikeRef<NoInfer<Project>> | Point2;

export type ContactDomain =
  | { readonly kind: "supportingLine" }
  | { readonly kind: "bounded"; readonly lower: number; readonly upper: number }
  | { readonly kind: "periodic"; readonly period: number };

export type ContactNeighborhood =
  | { readonly kind: "interior" | "start" | "end" }
  | { readonly kind: "local"; readonly lower: number; readonly upper: number };

export type ContactOrientation = "none" | "unoriented" | "aligned" | "opposed";

/** Complete explicit curve-contact branch state. */
export interface ContactState {
  readonly parameter: number;
  readonly winding: number;
  readonly domain: ContactDomain;
  readonly neighborhood: ContactNeighborhood;
  readonly orientation: ContactOrientation;
}

export type PeriodicAnchor =
  | { readonly kind: "none" }
  | { readonly kind: "anchor"; readonly parameter: number; readonly winding: number };

export interface FilletParent<Project> {
  readonly span: SpanLike<NoInfer<Project>>;
  readonly parameter: number;
  readonly winding: number;
  readonly neighborhood: ContactNeighborhood;
  readonly normalSide: Side;
  readonly trimEndpoint: Endpoint;
  readonly periodicAnchor: PeriodicAnchor;
}

export interface FilletCorner<Project, Key extends PropertyKey = PropertyKey> {
  readonly key: Key;
  readonly parents: readonly [
    first: FilletParent<NoInfer<Project>>,
    second: FilletParent<NoInfer<Project>>,
  ];
  readonly endpointOrder: "firstThenSecond" | "secondThenFirst";
  readonly sweep: Sweep;
}

export interface CurveOutputs<Project> {
  readonly curve: CurveRef<Project>;
  readonly span: NativeCurveSpanRef<Project>;
}

export type SketchPointFeature<Project> = FeatureRef<Project, "feature", {
  readonly point: PointRef<Project>;
}>;

export type SegmentFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly start: PointRef<Project>;
  readonly end: PointRef<Project>;
}>;

export type MidpointLineFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly start: PointRef<Project>;
  readonly midpoint: PointRef<Project>;
  readonly end: PointRef<Project>;
  readonly constraint: ConstraintRef<Project>;
}>;

export type RectangleFeature<Project> = FeatureRef<Project, "feature", {
  readonly corners: readonly [
    PointRef<Project>,
    PointRef<Project>,
    PointRef<Project>,
    PointRef<Project>,
  ];
  readonly curves: readonly [
    CurveRef<Project>,
    CurveRef<Project>,
    CurveRef<Project>,
    CurveRef<Project>,
  ];
  readonly spans: readonly [
    NativeCurveSpanRef<Project>,
    NativeCurveSpanRef<Project>,
    NativeCurveSpanRef<Project>,
    NativeCurveSpanRef<Project>,
  ];
  readonly center?: PointRef<Project>;
}>;

export type CircleFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly center: PointRef<Project>;
  readonly radius: ScalarRef<Project>;
}>;

export type ArcFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly center: PointRef<Project>;
  readonly start: PointRef<Project>;
  readonly midpoint: PointRef<Project>;
  readonly end: PointRef<Project>;
  readonly radius: ScalarRef<Project>;
  readonly startAngle: ScalarRef<Project>;
  readonly endAngle: ScalarRef<Project>;
}>;

export type EllipseFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly center: PointRef<Project>;
  readonly majorAxisPoint: PointRef<Project>;
  readonly minorAxisPoint: PointRef<Project>;
  readonly minorAxisRatio: ScalarRef<Project>;
}>;

export type EllipticalArcFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly center: PointRef<Project>;
  readonly majorAxisPoint: PointRef<Project>;
  readonly minorAxisPoint: PointRef<Project>;
  readonly start: PointRef<Project>;
  readonly end: PointRef<Project>;
  readonly minorAxisRatio: ScalarRef<Project>;
  readonly startAngle: ScalarRef<Project>;
  readonly endAngle: ScalarRef<Project>;
}>;

export type QuadraticBezierFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly start: PointRef<Project>;
  readonly control: PointRef<Project>;
  readonly end: PointRef<Project>;
}>;

export type CubicBezierFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly start: PointRef<Project>;
  readonly controls: readonly [PointRef<Project>, PointRef<Project>];
  readonly end: PointRef<Project>;
}>;

export type ConicFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly start: PointRef<Project>;
  readonly weightedMiddle: PointRef<Project>;
  readonly end: PointRef<Project>;
  readonly middleWeight: ScalarRef<Project>;
}>;

export type ParabolaFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly vertex: PointRef<Project>;
  readonly focus: PointRef<Project>;
  readonly trimStart: ScalarRef<Project>;
  readonly trimEnd: ScalarRef<Project>;
}>;

export type HyperbolaFeature<Project> = FeatureRef<Project, "feature", CurveOutputs<Project> & {
  readonly center: PointRef<Project>;
  readonly transverseAxisPoint: PointRef<Project>;
  readonly semiConjugate: ScalarRef<Project>;
  readonly trimStart: ScalarRef<Project>;
  readonly trimEnd: ScalarRef<Project>;
}>;

export interface PolylineVertex<Project, Key extends PropertyKey> {
  readonly key: Key;
  readonly position: PointInput<Project>;
}

export type PolylineFeature<Project, Key extends PropertyKey> = FeatureRef<Project, "feature", {
  readonly curve: CurveRef<Project>;
  readonly vertices: KeyedFeatureCollection<Key, PointRef<Project>>;
  readonly segments: KeyedFeatureCollection<Key, NativeCurveSpanRef<Project>>;
  readonly filletableCorners: DerivedFeatureCollection<
    PolylineFeature<Project, Key>,
    Key,
    FeatureCornerRef<Project>
  >;
}>;

export interface NurbsControl<Project, Key extends PropertyKey> {
  readonly key: Key;
  readonly position: PointInput<Project>;
  /** Dimensionless and finite. */
  readonly weight: number;
}

export interface NurbsControlOutputs<Project> {
  readonly position: PointRef<Project>;
  readonly weight: ScalarRef<Project>;
}

export type NurbsFeature<Project, Key extends PropertyKey> = FeatureRef<Project, "feature", {
  readonly curve: CurveRef<Project>;
  readonly controls: KeyedFeatureCollection<Key, NurbsControlOutputs<Project>>;
  readonly spans: KeyedFeatureCollection<Key, NativeCurveSpanRef<Project>>;
}>;

export type ConstraintFeature<Project, Outputs extends object = object> = FeatureRef<
  Project,
  "constraint",
  { readonly constraint: ConstraintRef<Project> } & Outputs
>;

export interface ContactOutput<Project> {
  readonly parameter: ScalarRef<Project>;
  readonly contact: ContactRef<Project>;
}

export type SingleContactConstraintFeature<Project> = ConstraintFeature<Project, {
  readonly contact: ContactOutput<Project>;
}>;

export type PairedContactConstraintFeature<Project> = ConstraintFeature<Project, {
  readonly contacts: {
    readonly first: ContactOutput<Project>;
    readonly second: ContactOutput<Project>;
  };
}>;

export type DimensionFeature<Project> = FeatureRef<Project, "dimension", {
  readonly value: ScalarRef<Project>;
  readonly dimension: DimensionRef<Project>;
}>;

export type OpenChainFeature<Project> = FeatureRef<Project, "chain", {
  readonly chain: ChainRef<Project>;
}>;

export type ClosedProfileFeature<Project> = FeatureRef<Project, "profile", {
  readonly profile: ProfileRef<Project>;
}>;

export type FilletFeature<Project> = FeatureRef<Project, "feature", {
  readonly arc: CurveSpanRef<Project>;
}>;

/** Patch-only host-authored rounded profile with scalar-derived mount centres. */
export type RoundedRectangleProfileFeature<Project> = FeatureRef<Project, "feature", {
  readonly profile: ProfileRef<Project>;
  readonly mounts: {
    readonly nw: PointRef<Project>;
    readonly ne: PointRef<Project>;
    readonly se: PointRef<Project>;
    readonly sw: PointRef<Project>;
  };
}>;

export interface FilletSetMember<Project> {
  readonly corner: FeatureCornerRef<Project>;
  readonly arc: CurveSpanRef<Project>;
}

export type FilletSetFeature<Project, Key extends PropertyKey> = FeatureRef<Project, "feature", {
  readonly fillets: KeyedFeatureCollection<Key, FilletSetMember<Project>>;
}>;

/** Operation results are typed by stable semantic role, never allocation order. */
export type OperationFeature<Project, Outputs extends object = object> = FeatureRef<
  Project,
  "operation",
  { readonly operation: OperationRef<Project> } & Outputs
>;

export type SplitOperationFeature<Project> = OperationFeature<Project, {
  readonly before: NativeCurveSpanRef<Project>;
  readonly after: NativeCurveSpanRef<Project>;
}>;

export type BreakOperationFeature<Project> = OperationFeature<Project, {
  readonly before: NativeCurveSpanRef<Project>;
  readonly middle: NativeCurveSpanRef<Project>;
  readonly after: NativeCurveSpanRef<Project>;
}>;

export type TrimOperationFeature<Project> = OperationFeature<Project, {
  readonly retained: NativeCurveSpanRef<Project>;
}>;

export type ExtendOperationFeature<Project> = OperationFeature<Project, {
  readonly curve: CurveRef<Project>;
  readonly span: NativeCurveSpanRef<Project>;
}>;

export type MirrorOperationFeature<Project> = OperationFeature<Project, {
  readonly curve: CurveRef<Project>;
  readonly span: NativeCurveSpanRef<Project>;
  readonly controls: Readonly<Record<string, PointRef<Project>>>;
  readonly symmetryConstraints: Readonly<Record<string, ConstraintRef<Project>>>;
}>;

export type ChamferOperationFeature<Project> = OperationFeature<Project, {
  readonly endpoints: { readonly first: PointRef<Project>; readonly second: PointRef<Project> };
  readonly edge: CurveRef<Project>;
  readonly span: NativeCurveSpanRef<Project>;
  readonly parents: {
    readonly first: ContactOutput<Project> & { readonly constraint: ConstraintRef<Project> };
    readonly second: ContactOutput<Project> & { readonly constraint: ConstraintRef<Project> };
  };
  readonly distances: {
    readonly first: DimensionFeature<Project>;
    readonly second: DimensionFeature<Project>;
  };
}>;

export type AssociativeFilletOperationFeature<Project> = OperationFeature<Project, {
  readonly center: PointRef<Project>;
  readonly radius: ScalarRef<Project>;
  readonly startAngle: ScalarRef<Project>;
  readonly endAngle: ScalarRef<Project>;
  readonly arc: CurveRef<Project>;
  readonly span: NativeCurveSpanRef<Project>;
  readonly parents: {
    readonly first: ContactOutput<Project>;
    readonly second: ContactOutput<Project>;
  };
  readonly association: ConstraintRef<Project>;
  readonly radiusDimension: DimensionFeature<Project>;
}>;

export type RectangleOperationFeature<Project> = OperationFeature<Project, {
  readonly corners: {
    readonly bottomLeft: PointRef<Project>;
    readonly bottomRight: PointRef<Project>;
    readonly topRight: PointRef<Project>;
    readonly topLeft: PointRef<Project>;
  };
  readonly edges: {
    readonly bottom: CurveRef<Project>;
    readonly right: CurveRef<Project>;
    readonly top: CurveRef<Project>;
    readonly left: CurveRef<Project>;
  };
  readonly spans: {
    readonly bottom: NativeCurveSpanRef<Project>;
    readonly right: NativeCurveSpanRef<Project>;
    readonly top: NativeCurveSpanRef<Project>;
    readonly left: NativeCurveSpanRef<Project>;
  };
  readonly constraints: Readonly<Record<string, ConstraintRef<Project>>>;
  readonly dimensions: {
    readonly width: DimensionFeature<Project>;
    readonly height: DimensionFeature<Project>;
  };
}>;

export type RegularPolygonOperationFeature<Project> = OperationFeature<Project, {
  readonly vertices: readonly PointRef<Project>[];
  readonly edges: readonly CurveRef<Project>[];
  readonly spans: readonly NativeCurveSpanRef<Project>[];
}>;

export type SlotOperationFeature<Project> = OperationFeature<Project, {
  readonly centers: { readonly first: PointRef<Project>; readonly second: PointRef<Project> };
  readonly boundaryPoints: Readonly<Record<string, PointRef<Project>>>;
  readonly edges: { readonly top: CurveRef<Project>; readonly bottom: CurveRef<Project> };
  readonly spans: {
    readonly top: NativeCurveSpanRef<Project>;
    readonly bottom: NativeCurveSpanRef<Project>;
  };
  readonly arcs: {
    readonly right: CurveOutputs<Project> & {
      readonly radius: ScalarRef<Project>;
      readonly startAngle: ScalarRef<Project>;
      readonly endAngle: ScalarRef<Project>;
    };
    readonly left: CurveOutputs<Project> & {
      readonly radius: ScalarRef<Project>;
      readonly startAngle: ScalarRef<Project>;
      readonly endAngle: ScalarRef<Project>;
    };
  };
  readonly joins: Readonly<Record<string, ContactOutput<Project> & {
    readonly constraint: ConstraintRef<Project>;
  }>>;
  readonly fixedConstraints: Readonly<Record<string, ConstraintRef<Project>>>;
}>;

export type LinearPatternOperationFeature<Project> = OperationFeature<Project, {
  readonly instances: readonly {
    readonly sources: Readonly<Record<string, {
      readonly curve: CurveRef<Project>;
      readonly span: NativeCurveSpanRef<Project>;
      readonly controls: Readonly<Record<string, PointRef<Project>>>;
    }>>;
  }[];
}>;

export interface ProfileOffsetPathFeature<Project> {
  readonly boundaries: Readonly<Record<string, PointRef<Project>>>;
  readonly edges: Readonly<Record<string, CurveOutputs<Project> & {
    readonly center?: PointRef<Project>;
    readonly radius?: ScalarRef<Project>;
    readonly startAngle?: ScalarRef<Project>;
    readonly endAngle?: ScalarRef<Project>;
  }>>;
  readonly junctions: Readonly<Record<string, {
    readonly incoming: ContactOutput<Project>;
    readonly outgoing: ContactOutput<Project>;
    readonly constraint: ConstraintRef<Project>;
  }>>;
}

export type ProfileOffsetOperationFeature<Project> = OperationFeature<Project, {
  readonly operand: {
    readonly outer?: ProfileOffsetPathFeature<Project>;
    readonly holes?: Readonly<Record<string, ProfileOffsetPathFeature<Project>>>;
    readonly chain?: ProfileOffsetPathFeature<Project>;
  };
  readonly distance: DimensionFeature<Project>;
}>;

export interface SketchPointValues<Project> extends PresentationOptions {
  readonly point: PointInput<Project>;
  readonly role?: GeometryRole;
}

export interface SegmentValues<Project> extends PresentationOptions {
  readonly start: PointInput<Project>;
  readonly end: PointInput<Project>;
  readonly branchDirection?: Point2;
  readonly role?: GeometryRole;
}

export interface PolylineValues<Project, Key extends PropertyKey> extends PresentationOptions {
  readonly vertices: readonly PolylineVertex<Project, Key>[];
  readonly closed?: boolean;
  readonly role?: GeometryRole;
}

export interface NurbsValues<Project, Key extends PropertyKey> extends PresentationOptions {
  readonly controls: readonly NurbsControl<Project, Key>[];
  readonly degree: number;
  /** Stable child key, never an ordinal gauge index. */
  readonly gauge: Key;
  readonly role?: GeometryRole;
}

export interface GeometryBuilder<Project> {
  sketchPoint(id: string, values: SketchPointValues<Project>): SketchPointFeature<Project>;
  segment(id: string, values: SegmentValues<Project>): SegmentFeature<Project>;
  polyline<const Key extends string>(
    id: string,
    values: PolylineValues<Project, Key>,
  ): PolylineFeature<Project, Key>;
  midpointLine(id: string, values: PresentationOptions & {
    readonly midpoint: PointInput<Project>;
    readonly end: PointInput<Project>;
    readonly branchDirection?: Point2;
    readonly role?: GeometryRole;
  }): MidpointLineFeature<Project>;
  twoPointAlignedRectangle(id: string, values: PresentationOptions & {
    readonly firstCorner: PointInput<Project>;
    readonly oppositeCorner: PointInput<Project>;
    readonly regularized?: boolean;
    readonly role?: GeometryRole;
  }): RectangleFeature<Project>;
  threePointCornerRectangle(id: string, values: PresentationOptions & {
    readonly firstCorner: PointInput<Project>;
    readonly secondCorner: PointInput<Project>;
    readonly thirdCorner: PointInput<Project>;
    readonly regularized?: boolean;
    readonly role?: GeometryRole;
  }): RectangleFeature<Project>;
  centerRectangle(id: string, values: PresentationOptions & {
    readonly center: PointInput<Project>;
    readonly corner: PointInput<Project>;
    readonly regularized?: boolean;
    readonly role?: GeometryRole;
  }): RectangleFeature<Project>;
  threePointCenterRectangle(id: string, values: PresentationOptions & {
    readonly center: PointInput<Project>;
    readonly corner: PointInput<Project>;
    readonly sideMidpoint?: Point2;
    readonly regularized?: boolean;
    readonly role?: GeometryRole;
  }): RectangleFeature<Project>;
  centerRadiusCircle(id: string, values: PresentationOptions & {
    readonly center: PointInput<Project>;
    /** Ergonomic scalar radius; Rust owns the native radius-point lowering. */
    readonly radius: Length | ScalarRef<Project>;
    readonly role?: GeometryRole;
  }): CircleFeature<Project>;
  twoPointDiameterCircle(id: string, values: PresentationOptions & {
    readonly start: PointInput<Project>;
    readonly end: PointInput<Project>;
    readonly role?: GeometryRole;
  }): CircleFeature<Project>;
  threePointCircle(id: string, values: PresentationOptions & {
    readonly first: PointInput<Project>;
    readonly second: PointInput<Project>;
    readonly third: PointInput<Project>;
    readonly role?: GeometryRole;
  }): CircleFeature<Project>;
  centerArc(id: string, values: PresentationOptions & {
    readonly center: PointInput<Project>;
    readonly start: PointInput<Project>;
    readonly end: PointInput<Project>;
    readonly sweep?: Sweep;
    readonly role?: GeometryRole;
  }): ArcFeature<Project>;
  threePointArc(id: string, values: PresentationOptions & {
    readonly first: PointInput<Project>;
    readonly second: PointInput<Project>;
    readonly third: PointInput<Project>;
    readonly sweep?: Sweep;
    readonly role?: GeometryRole;
  }): ArcFeature<Project>;
  tangentArc(id: string, values: PresentationOptions & {
    /** Explicit native centre; source tangency alone cannot reconstruct it. */
    readonly center: Point2;
    readonly start: PointInput<Project>;
    readonly end: PointInput<Project>;
    readonly source: { readonly span: SpanLike<Project>; readonly contact?: ContactState };
    readonly sweep?: Sweep;
    readonly orientation?: "aligned" | "opposed";
    readonly role?: GeometryRole;
  }): ArcFeature<Project>;
  centerAxesEllipse(id: string, values: PresentationOptions & {
    readonly center: PointInput<Project>;
    readonly majorAxisPoint: PointInput<Project>;
    readonly minorAxisPoint: PointInput<Project>;
    readonly role?: GeometryRole;
  }): EllipseFeature<Project>;
  axisEndpointsEllipse(id: string, values: PresentationOptions & {
    readonly majorAxisStart: PointInput<Project>;
    readonly majorAxisEnd: PointInput<Project>;
    readonly minorAxisPoint: PointInput<Project>;
    readonly role?: GeometryRole;
  }): EllipseFeature<Project>;
  centerAxesEllipticalArc(id: string, values: PresentationOptions & {
    readonly center: PointInput<Project>;
    readonly majorAxisPoint: PointInput<Project>;
    readonly minorAxisPoint: PointInput<Project>;
    readonly start: PointInput<Project>;
    readonly end: PointInput<Project>;
    readonly sweep?: Sweep;
    readonly role?: GeometryRole;
  }): EllipticalArcFeature<Project>;
  axisEndpointsEllipticalArc(id: string, values: PresentationOptions & {
    readonly majorAxisStart: PointInput<Project>;
    readonly majorAxisEnd: PointInput<Project>;
    readonly minorAxisPoint: PointInput<Project>;
    readonly start: PointInput<Project>;
    readonly end: PointInput<Project>;
    readonly sweep?: Sweep;
    readonly role?: GeometryRole;
  }): EllipticalArcFeature<Project>;
  quadraticBezier(id: string, values: PresentationOptions & {
    readonly start: PointInput<Project>;
    readonly control: PointInput<Project>;
    readonly end: PointInput<Project>;
    readonly role?: GeometryRole;
  }): QuadraticBezierFeature<Project>;
  cubicBezier(id: string, values: PresentationOptions & {
    readonly start: PointInput<Project>;
    readonly firstControl: PointInput<Project>;
    readonly secondControl: PointInput<Project>;
    readonly end: PointInput<Project>;
    readonly role?: GeometryRole;
  }): CubicBezierFeature<Project>;
  rationalQuadraticConic(id: string, values: PresentationOptions & {
    readonly start: PointInput<Project>;
    readonly end: PointInput<Project>;
    /** Stored homogeneous control, not an aliasable point input. */
    readonly weightedMiddle: Point2;
    /** Dimensionless homogeneous middle weight. */
    readonly middleWeight: number;
    readonly role?: GeometryRole;
  }): ConicFeature<Project>;
  parabola(id: string, values: PresentationOptions & {
    readonly vertex: PointInput<Project>;
    readonly focus: PointInput<Project>;
    /** Dimensionless parameters on the admitted native span. */
    readonly trimStart: number;
    readonly trimEnd: number;
    readonly role?: GeometryRole;
  }): ParabolaFeature<Project>;
  hyperbola(id: string, values: PresentationOptions & {
    readonly center: PointInput<Project>;
    readonly transverseAxisPoint: PointInput<Project>;
    readonly semiConjugate: Length | ScalarRef<Project>;
    /** Dimensionless parameters on the selected branch span. */
    readonly trimStart: number;
    readonly trimEnd: number;
    readonly branch?: "positive" | "negative";
    readonly role?: GeometryRole;
  }): HyperbolaFeature<Project>;
  openControlNurbs<const Key extends string>(
    id: string,
    values: NurbsValues<Project, Key>,
  ): NurbsFeature<Project, Key>;
  periodicControlNurbs<const Key extends string>(
    id: string,
    values: NurbsValues<Project, Key>,
  ): NurbsFeature<Project, Key>;
}

type ConstraintValues = SuppressibleOptions;
type ContactValues = ConstraintValues & { readonly contact?: ContactState };
type PairedContactValues = ConstraintValues & {
  readonly contacts?: { readonly first: ContactState; readonly second: ContactState };
};

export interface ConstraintBuilder<Project> {
  fixedPoint(id: string, values: ConstraintValues & {
    readonly point: PointLikeRef<Project>;
    readonly target?: Point2;
  }): ConstraintFeature<Project>;
  fixedCoordinate(id: string, values: ConstraintValues & {
    readonly point: PointLikeRef<Project>;
    readonly axis?: Axis;
    readonly target?: Length;
  }): ConstraintFeature<Project>;
  coincidentWithOrigin(id: string, values: ConstraintValues & {
    readonly point: PointLikeRef<Project>;
  }): ConstraintFeature<Project>;
  pointOnDatumAxis(id: string, values: ConstraintValues & {
    readonly point: PointLikeRef<Project>;
    readonly axis?: Axis;
  }): ConstraintFeature<Project>;
  coincident(id: string, values: ConstraintValues & {
    readonly first: PointLikeRef<Project>;
    readonly second: PointLikeRef<Project>;
  }): ConstraintFeature<Project>;
  horizontal(id: string, values: ConstraintValues & {
    readonly span: SpanLike<Project>;
  }): ConstraintFeature<Project>;
  vertical(id: string, values: ConstraintValues & {
    readonly span: SpanLike<Project>;
  }): ConstraintFeature<Project>;
  horizontalPoints(id: string, values: ConstraintValues & {
    readonly first: PointLikeRef<Project>;
    readonly second: PointLikeRef<Project>;
  }): ConstraintFeature<Project>;
  verticalPoints(id: string, values: ConstraintValues & {
    readonly first: PointLikeRef<Project>;
    readonly second: PointLikeRef<Project>;
  }): ConstraintFeature<Project>;
  horizontalPointToMidpoint(id: string, values: ConstraintValues & {
    readonly point: PointLikeRef<Project>;
    readonly line: SpanLike<Project>;
  }): ConstraintFeature<Project>;
  verticalPointToMidpoint(id: string, values: ConstraintValues & {
    readonly point: PointLikeRef<Project>;
    readonly line: SpanLike<Project>;
  }): ConstraintFeature<Project>;
  pointOnCurve(id: string, values: ContactValues & {
    readonly point: PointLikeRef<Project>;
    readonly curve: SpanLike<Project>;
  }): SingleContactConstraintFeature<Project>;
  parallel(id: string, values: ConstraintValues & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
  }): ConstraintFeature<Project>;
  perpendicular(id: string, values: ConstraintValues & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
  }): ConstraintFeature<Project>;
  collinearWithDatumAxis(id: string, values: ConstraintValues & {
    readonly span: SpanLike<Project>;
    readonly axis?: Axis;
    readonly direction?: Direction;
  }): ConstraintFeature<Project>;
  concentric(id: string, values: ConstraintValues & {
    readonly first: CurveLike<Project>;
    readonly second: CurveLike<Project>;
  }): ConstraintFeature<Project>;
  collinear(id: string, values: ConstraintValues & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly firstDirection?: Direction;
    readonly secondDirection?: Direction;
  }): ConstraintFeature<Project>;
  equalLength(id: string, values: ConstraintValues & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
  }): ConstraintFeature<Project>;
  equalRadius(id: string, values: ConstraintValues & {
    readonly first: CurveLike<Project>;
    readonly second: CurveLike<Project>;
  }): ConstraintFeature<Project>;
  midpoint(id: string, values: ConstraintValues & {
    readonly point: PointLikeRef<Project>;
    readonly line: SpanLike<Project>;
  }): ConstraintFeature<Project>;
  symmetricAboutLine(id: string, values: ConstraintValues & {
    readonly first: PointLikeRef<Project>;
    readonly second: PointLikeRef<Project>;
    readonly axis: SpanLike<Project>;
  }): ConstraintFeature<Project>;
  symmetricAboutDatumAxis(id: string, values: ConstraintValues & {
    readonly first: PointLikeRef<Project>;
    readonly second: PointLikeRef<Project>;
    readonly axis?: Axis;
  }): ConstraintFeature<Project>;
  lineCircleTangency(id: string, values: PairedContactValues & {
    readonly line: SpanLike<Project>;
    readonly circle: CurveLike<Project>;
    readonly side?: Side;
  }): PairedContactConstraintFeature<Project>;
  circleCircleTangency(id: string, values: ConstraintValues & {
    readonly first: CurveLike<Project>;
    readonly second: CurveLike<Project>;
    readonly mode?: "external" | "firstContainsSecond" | "secondContainsFirst";
    readonly centerDirection?: Point2;
  }): ConstraintFeature<Project>;
  circleArcTangency(id: string, values: PairedContactValues & {
    readonly circle: CurveLike<Project>;
    readonly arc: CurveLike<Project>;
    readonly side?: "outsideArc" | "insideArc";
  }): PairedContactConstraintFeature<Project>;
  lineCurveTangency(id: string, values: ContactValues & {
    readonly line: SpanLike<Project>;
    readonly curve: SpanLike<Project>;
    readonly endpoint: Endpoint;
  }): SingleContactConstraintFeature<Project>;
  curveCurveContact(id: string, values: PairedContactValues & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
  }): PairedContactConstraintFeature<Project>;
  curveCurveTangency(id: string, values: PairedContactValues & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
  }): PairedContactConstraintFeature<Project>;
  curveDirection(id: string, values: ContactValues & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly relation: "tangent" | "normal";
    readonly orientation?: "aligned" | "opposed";
    readonly side?: Side;
  }): SingleContactConstraintFeature<Project>;
  equalCurvature(id: string, values: PairedContactValues & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly relation: "signed" | "magnitudeSameSign" | "magnitudeOppositeSign";
  }): PairedContactConstraintFeature<Project>;
  endpointContinuity(id: string, values: PairedContactValues & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly continuity: "g0" | "g1" | "g2" | "parametricC2";
    readonly parameterRatio?: number;
  }): PairedContactConstraintFeature<Project>;
  lineLineFillet(id: string, values: PairedContactValues & {
    readonly fillet: CurveLike<Project>;
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly firstSide: Side;
    readonly secondSide: Side;
    readonly endpointOrder: "firstThenSecond" | "secondThenFirst";
  }): PairedContactConstraintFeature<Project>;
  curveCurveFillet(id: string, values: PairedContactValues & {
    readonly fillet: CurveLike<Project>;
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly firstSide: Side;
    readonly secondSide: Side;
    readonly endpointOrder: "firstThenSecond" | "secondThenFirst";
    readonly firstTrimEndpoint: Endpoint;
    readonly secondTrimEndpoint: Endpoint;
  }): PairedContactConstraintFeature<Project>;
}

type DimensionValues<Value> = SuppressibleOptions & {
  readonly value: Value;
  readonly mode?: DimensionMode;
};

export interface DimensionBuilder<Project> {
  pointDistance(id: string, values: DimensionValues<Length> & {
    readonly first: PointLikeRef<Project>;
    readonly second: PointLikeRef<Project>;
  }): DimensionFeature<Project>;
  curveLength(id: string, values: DimensionValues<Length> & {
    readonly curve: SpanLike<Project>;
  }): DimensionFeature<Project>;
  radius(id: string, values: DimensionValues<Length> & {
    readonly curve: CurveLike<Project>;
  }): DimensionFeature<Project>;
  diameter(id: string, values: DimensionValues<Length> & {
    readonly curve: CurveLike<Project>;
  }): DimensionFeature<Project>;
  orientedAngle(id: string, values: DimensionValues<Angle> & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly orientation?: Sweep;
  }): DimensionFeature<Project>;
  supportingLineOffset(id: string, values: DimensionValues<Length> & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly side?: Side;
    readonly orientation?: "same" | "reversed";
  }): DimensionFeature<Project>;
  exactTranslatedSegmentOffset(id: string, values: DimensionValues<Length> & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly side?: Side;
    readonly orientation?: "same" | "reversed";
  }): DimensionFeature<Project>;
  profileOffset(id: string, values: Omit<DimensionValues<Length>, "mode"> & {
    readonly source: ProfileRef<Project> | ChainRef<Project>;
    readonly target?: SpanLike<Project>;
    readonly direction?: "outward" | "inward";
    readonly side?: Side;
    readonly sourceTraversal: Direction;
    readonly targetTraversal: Direction;
  }): DimensionFeature<Project>;
}

export interface OperationBuilder<Project> {
  split(id: string, values: PresentationOptions & {
    readonly source: SpanLike<Project>;
    readonly parameter: number;
    readonly retained: "before" | "after";
  }): SplitOperationFeature<Project>;
  break(id: string, values: PresentationOptions & {
    readonly source: SpanLike<Project>;
    readonly start: number;
    readonly end: number;
    readonly retained: "before" | "after";
  }): BreakOperationFeature<Project>;
  trim(id: string, values: PresentationOptions & {
    readonly source: SpanLike<Project>;
    readonly parameter: number;
    readonly retained: "before" | "after";
  }): TrimOperationFeature<Project>;
  extend(id: string, values: PresentationOptions & {
    readonly source: SpanLike<Project>;
    readonly target: SpanLike<Project>;
    readonly endpoint: Endpoint;
  }): ExtendOperationFeature<Project>;
  mirror(id: string, values: PresentationOptions & {
    readonly source: CurveLike<Project>;
    readonly axis: SpanLike<Project>;
  }): MirrorOperationFeature<Project>;
  chamfer(id: string, values: PresentationOptions & {
    readonly first: SpanLike<Project>;
    readonly second: SpanLike<Project>;
    readonly firstDistance: Length;
    readonly secondDistance: Length;
  }): ChamferOperationFeature<Project>;
  associativeFillet(id: string, values: PresentationOptions & {
    readonly radius: Length;
    readonly radiusMode: DimensionMode;
    readonly parents: readonly [FilletParent<Project>, FilletParent<Project>];
    readonly endpointOrder: "firstThenSecond" | "secondThenFirst";
    readonly sweep: Sweep;
  }): AssociativeFilletOperationFeature<Project>;
  rectangle(id: string, values: PresentationOptions & {
    readonly origin: Point2;
    readonly width: Length;
    readonly height: Length;
    readonly role: GeometryRole;
  }): RectangleOperationFeature<Project>;
  regularPolygon(id: string, values: PresentationOptions & {
    readonly center: Point2;
    readonly radius: Length;
    readonly sides: number;
    readonly rotation: Angle;
    readonly role: GeometryRole;
  }): RegularPolygonOperationFeature<Project>;
  slot(id: string, values: PresentationOptions & {
    readonly firstCenter: Point2;
    readonly secondCenter: Point2;
    readonly radius: Length;
    readonly role: GeometryRole;
  }): SlotOperationFeature<Project>;
  linearPattern(id: string, values: PresentationOptions & {
    readonly sources: readonly CurveLike<Project>[];
    readonly instances: number;
    readonly step: Point2;
  }): LinearPatternOperationFeature<Project>;
  profileOffset(id: string, values: PresentationOptions & {
    readonly sources: readonly (ProfileRef<Project> | ChainRef<Project>)[];
    readonly distance: Length;
    readonly direction?: "outward" | "inward";
    readonly side?: Side;
    readonly firstTraversal?: Direction;
  }): ProfileOffsetOperationFeature<Project>;
}

export interface AggregateBuilder<Project> {
  openChain(id: string, values: PresentationOptions & {
    readonly spans: readonly SpanLike<Project>[];
  }): OpenChainFeature<Project>;
  closedProfile(id: string, values: PresentationOptions & {
    readonly spans: readonly SpanLike<Project>[];
  }): ClosedProfileFeature<Project>;
}

export interface ComputedBuilder<Project> {
  filletSet<const Key extends string>(id: string, values: PresentationOptions & {
    readonly radius: Length;
    readonly corners: readonly FilletCorner<Project, Key>[];
  }): FilletSetFeature<Project, Key>;
}

/** Host-authored declarations admitted only inside trusted custom patches. */
export interface PatchComputedBuilder<Project> extends ComputedBuilder<Project> {
  fillet(id: string, values: PresentationOptions & {
    readonly corner: FeatureCornerRef<Project>;
    readonly radius: Length;
  }): FilletFeature<Project>;
  roundedRectangleProfile(id: string, values: PresentationOptions & {
    readonly width: Length;
    readonly height: Length;
    readonly cornerRadius: Length;
  }): RoundedRectangleProfileFeature<Project>;
}

type ValueSchemaKind =
  | "point"
  | "corner"
  | "curve"
  | "curveSpan"
  | "scalar"
  | "length"
  | "angle"
  | "feature"
  | "keyed"
  | "record";

interface ValueSchemaRuntime {
  readonly kind: ValueSchemaKind;
  readonly feature?: FeatureSchemaName;
  readonly element?: ValueSchema<unknown>;
}

export interface ValueSchema<Value> {
  readonly [schemaRuntime]: {
    readonly value: Value;
    readonly runtime: ValueSchemaRuntime;
  };
}

export type InputSchemas = Readonly<Record<string, ValueSchema<unknown>>>;
type SchemaValue<Schema> = Schema extends ValueSchema<infer Value> ? Value : never;

type Reproject<Value, Project> =
  Value extends { readonly [nativeSpanBrand]: true }
    ? NativeCurveSpanRef<Project>
    : Value extends {
      readonly [referenceKindBrand]: infer Kind extends FeatureKind;
      readonly [featureOutputsBrand]: infer Outputs;
    }
      ? FeatureRef<Project, Kind, Reproject<Outputs, Project>>
      : Value extends { readonly [referenceKindBrand]: infer Kind extends FeatureKind }
        ? OutputRef<Project, Kind>
      : Value extends KeyedFeatureCollection<infer Key, infer Element>
        ? KeyedFeatureCollection<Key, Reproject<Element, Project>>
        : Value extends object
          ? { readonly [Key in keyof Value]: Reproject<Value[Key], Project> }
          : Value;

export type PatchInputs<Schemas extends InputSchemas, Project> = {
  readonly [Key in keyof Schemas]: Reproject<SchemaValue<Schemas[Key]>, Project>;
};

function valueSchema<Value>(
  kind: ValueSchemaKind,
  feature?: FeatureSchemaName,
  element?: ValueSchema<unknown>,
): ValueSchema<Value> {
  const runtime = Object.freeze({
    kind,
    ...(feature === undefined ? {} : { feature }),
    ...(element === undefined ? {} : { element }),
  });
  const schema = Object.freeze({ [schemaRuntime]: Object.freeze({
    value: undefined as Value,
    runtime,
  }) });
  registerAuthoringSchema(schema, {
    kind,
    ...(feature === undefined ? {} : { feature }),
    ...(element === undefined ? {} : { element }),
  });
  return schema;
}

export type FeatureSchemaName =
  | "sketchPoint"
  | "segment"
  | "polyline"
  | "rectangle"
  | "circle"
  | "arc"
  | "ellipse"
  | "ellipticalArc"
  | "quadraticBezier"
  | "cubicBezier"
  | "conic"
  | "parabola"
  | "hyperbola"
  | "nurbs"
  | "fillet"
  | "filletSet";

type FeatureSchemaValue<Name extends FeatureSchemaName> =
  Name extends "sketchPoint" ? SketchPointFeature<unknown>
    : Name extends "segment" ? SegmentFeature<unknown>
    : Name extends "polyline" ? PolylineFeature<unknown, string>
    : Name extends "rectangle" ? RectangleFeature<unknown>
    : Name extends "circle" ? CircleFeature<unknown>
    : Name extends "arc" ? ArcFeature<unknown>
    : Name extends "ellipse" ? EllipseFeature<unknown>
    : Name extends "ellipticalArc" ? EllipticalArcFeature<unknown>
    : Name extends "quadraticBezier" ? QuadraticBezierFeature<unknown>
    : Name extends "cubicBezier" ? CubicBezierFeature<unknown>
    : Name extends "conic" ? ConicFeature<unknown>
    : Name extends "parabola" ? ParabolaFeature<unknown>
    : Name extends "hyperbola" ? HyperbolaFeature<unknown>
    : Name extends "nurbs" ? NurbsFeature<unknown, string>
    : Name extends "fillet" ? FilletFeature<unknown>
    : FilletSetFeature<unknown, string>;

export const t = Object.freeze({
  point: (): ValueSchema<PointRef<unknown>> => valueSchema("point"),
  corner: (): ValueSchema<FeatureCornerRef<unknown>> => valueSchema("corner"),
  curve: (): ValueSchema<CurveRef<unknown>> => valueSchema("curve"),
  curveSpan: (): ValueSchema<NativeCurveSpanRef<unknown>> => valueSchema("curveSpan"),
  scalar: (): ValueSchema<ScalarRef<unknown>> => valueSchema("scalar"),
  length: (): ValueSchema<Length> => valueSchema("length"),
  angle: (): ValueSchema<Angle> => valueSchema("angle"),
  feature: <const Name extends FeatureSchemaName>(name: Name): ValueSchema<FeatureSchemaValue<Name>> =>
    valueSchema("feature", name),
  keyed: <Value>(element: ValueSchema<Value>): ValueSchema<KeyedFeatureCollection<string, Value>> =>
    valueSchema("keyed", undefined, element),
  record: <Value>(element: ValueSchema<Value>): ValueSchema<Readonly<Record<string, Value>>> =>
    valueSchema("record", undefined, element),
});

export interface PatchProject extends SketchProject<typeof PATCH_PROJECT_NAME> {}

export interface PatchBuilder<Project> {
  readonly geometry: GeometryBuilder<Project>;
  readonly constraint: ConstraintBuilder<Project>;
  readonly dimension: DimensionBuilder<Project>;
  readonly operation: OperationBuilder<Project>;
  readonly aggregate: AggregateBuilder<Project>;
  readonly computed: PatchComputedBuilder<Project>;
  each<Key extends PropertyKey, Value, Result>(
    collection: KeyedFeatureCollection<Key, Value>,
    build: (value: Value & { readonly key: Key }) => Result,
  ): DerivedFeatureCollection<typeof collection, Key, Result>;
  mapRecord<RecordType extends Readonly<Record<PropertyKey, unknown>>, Result>(
    record: RecordType,
    build: (value: RecordType[keyof RecordType]) => Result,
  ): { readonly [Key in keyof RecordType]: Result };
}

export interface PatchDefinition<Schemas extends InputSchemas, Result> {
  readonly inputs: Schemas;
  readonly [patchBrand]: Result;
  readonly [patchRuntime]: {
    readonly build: (
      builder: PatchBuilder<PatchProject>,
      inputs: PatchInputs<Schemas, PatchProject>,
    ) => unknown;
  };
}

export function definePatch<Schemas extends InputSchemas, Result>(
  inputs: Schemas,
  build: (
    builder: PatchBuilder<PatchProject>,
    inputs: PatchInputs<Schemas, PatchProject>,
  ) => Result,
): PatchDefinition<Schemas, Result> {
  if (typeof build !== "function") throw new TypeError("patch requires a builder callback");
  const definition = Object.freeze({ inputs, [patchRuntime]: { build } }) as PatchDefinition<
    Schemas,
    Result
  >;
  registerPatchRuntime(definition, {
    inputs,
    build: build as (builder: unknown, inputs: unknown) => unknown,
  });
  return definition;
}

type PatchInvocationInputs<Schemas extends InputSchemas, Project> = {
  readonly [Key in keyof Schemas]: Reproject<SchemaValue<Schemas[Key]>, Project>;
};

export interface SketchBuilder<Project> {
  readonly geometry: GeometryBuilder<Project>;
  readonly constraint: ConstraintBuilder<Project>;
  readonly dimension: DimensionBuilder<Project>;
  readonly operation: OperationBuilder<Project>;
  readonly aggregate: AggregateBuilder<Project>;
  readonly computed: ComputedBuilder<Project>;
  use<Schemas extends InputSchemas, Result>(
    id: string,
    patch: PatchDefinition<Schemas, Result>,
    inputs: PatchInvocationInputs<Schemas, Project>,
  ): Reproject<Result, Project>;
  group(label: string, declarations: readonly OutputRef<Project, FeatureKind>[]): void;
  suppress(declaration: OutputRef<Project, FeatureKind>): void;
}

export interface SketchExecutionProject extends SketchProject<typeof SKETCH_PROJECT_NAME> {}

export interface Sketch<Result> {
  /** Ordinary nested value returned by the sketch callback. */
  readonly output: Result;
  readonly [sketchBrand]: Result;
}

/** Execute the equation-free callback and retain its ordinary nested result. */
export function sketch<Result>(
  build: (builder: SketchBuilder<SketchExecutionProject>) => Result,
): Sketch<Result> {
  if (typeof build !== "function") throw new TypeError("sketch requires a builder callback");
  const result = build(createSketchBuilder());
  return Object.freeze({ output: result, [sketchRuntime]: result }) as unknown as Sketch<Result>;
}

interface ReferenceRuntime {
  readonly project: string;
  /** Opaque declaration/application/local/member identity segments. */
  readonly identity: readonly SemanticMemberKey[];
  readonly declaration: string;
  readonly namespace: string;
  readonly method: string;
  readonly path: readonly SemanticPathSegment[];
  readonly kind: FeatureKind;
}

function makeReference<Project, Kind extends FeatureKind>(
  runtime: ReferenceRuntime,
): OutputRef<Project, Kind> {
  const value = Object.create(null) as Record<PropertyKey, unknown>;
  Object.defineProperty(value, referenceRuntime, { value: runtime });
  return Object.freeze(value) as unknown as OutputRef<Project, Kind>;
}

function lazyReference<Project>(
  runtime: ReferenceRuntime,
  members: Readonly<Record<PropertyKey, unknown>> = {},
): object {
  const target = Object.create(null) as Record<PropertyKey, unknown>;
  Object.defineProperty(target, referenceRuntime, { value: runtime });
  Object.assign(target, members);
  Object.freeze(target);
  return new Proxy(target, {
    get: (value, property, receiver) => {
      if (Reflect.has(value, property)) return Reflect.get(value, property, receiver);
      if (typeof property !== "string") return Reflect.get(value, property, receiver);
      const path = [...runtime.path, property];
      return lazyReference<Project>({
        ...runtime,
        path,
        kind: inferredOutputKind(property),
      });
    },
  });
}

function inferredOutputKind(property: string): FeatureKind {
  const lower = property.toLowerCase();
  if (lower.includes("constraint") || lower === "association") return "constraint";
  if (lower.includes("dimension")) return "dimension";
  if (lower === "parameter" || lower === "value" || lower === "radius"
    || lower.includes("angle") || lower.includes("weight") || lower === "target") return "scalar";
  if (lower === "contact") return "contact";
  if (lower === "span" || lower === "before" || lower === "after"
    || lower === "middle" || lower === "retained") return "curve_span";
  if (lower === "curve" || lower === "arc" || lower === "edge") return "curve";
  if (lower === "profile") return "profile";
  if (lower === "chain") return "chain";
  if (lower === "operation") return "operation";
  if (lower.includes("corner")) return "feature_corner";
  return "point";
}

type RuntimeShape =
  | FeatureKind
  | { readonly [key: string]: RuntimeShape }
  | readonly RuntimeShape[];

type CatalogRuntimeShape =
  | { readonly shape: "leaf"; readonly kind: FeatureKind }
  | { readonly shape: "native_span" }
  | {
    readonly shape: "object";
    readonly fields: Readonly<Record<string, CatalogRuntimeShape>>;
  }
  | { readonly shape: "tuple"; readonly items: readonly CatalogRuntimeShape[] }
  | {
    readonly shape: "dynamic_keyed";
    readonly member: CatalogRuntimeShape;
  };

function catalogRuntimeEntry(family: string): {
  readonly feature_kind: FeatureKind;
  readonly outputs: CatalogRuntimeShape;
} | undefined {
  if (family === "computed.roundedRectangleProfile") {
    return {
      feature_kind: "feature",
      outputs: {
        shape: "object",
        fields: {
          mounts: {
            shape: "object",
            fields: {
              ne: { shape: "leaf", kind: "point" },
              nw: { shape: "leaf", kind: "point" },
              se: { shape: "leaf", kind: "point" },
              sw: { shape: "leaf", kind: "point" },
            },
          },
          profile: { shape: "leaf", kind: "profile" },
        },
      },
    };
  }
  return (DECLARATION_RESULT_CATALOG as unknown as Readonly<Record<string, {
    readonly feature_kind: FeatureKind;
    readonly outputs: CatalogRuntimeShape;
  }>>)[family];
}

function runtimeShape(shape: CatalogRuntimeShape): RuntimeShape {
  switch (shape.shape) {
    case "leaf": return shape.kind;
    case "native_span": return "curve_span";
    case "object": return Object.fromEntries(
      Object.entries(shape.fields).map(([name, child]) => [name, runtimeShape(child)]),
    );
    case "tuple": return shape.items.map(runtimeShape);
    // Concrete keyed children are materialized by the dedicated Polyline,
    // NURBS and FilletSet paths. Other plan-dependent collections retain a
    // typed lazy namespace until native preparation supplies exact keys.
    case "dynamic_keyed": return {};
  }
}

function shapedReference<Project>(runtime: ReferenceRuntime, shape: RuntimeShape): unknown {
  if (typeof shape === "string") {
    return makeReference<Project, FeatureKind>({ ...runtime, kind: shape });
  }
  if (Array.isArray(shape)) {
    if (shape.length === 0) {
      return lazySequenceReference<Project>(runtime);
    }
    return Object.freeze(shape.map((member, index) => shapedReference<Project>({
      ...runtime,
      path: [...runtime.path, index],
    }, member)));
  }
  return lazyReference<Project>(runtime, Object.fromEntries(
    Object.entries(shape).map(([key, member]) => [key, shapedReference<Project>({
      ...runtime,
      path: [...runtime.path, key],
    }, member)]),
  ));
}

function lazySequenceReference<Project>(runtime: ReferenceRuntime): readonly unknown[] {
  const target: unknown[] = [];
  Object.defineProperty(target, referenceRuntime, { value: runtime });
  return new Proxy(target, {
    get: (value, property, receiver) => {
      if (Reflect.has(value, property)) return Reflect.get(value, property, receiver);
      if (typeof property !== "string" || !/^(?:0|[1-9][0-9]*)$/u.test(property)) {
        return Reflect.get(value, property, receiver);
      }
      return lazyReference<Project>({
        ...runtime,
        path: [...runtime.path, Number(property)],
        kind: "feature",
      });
    },
  });
}

function declarationRuntime(
  project: { readonly name: string },
  namespace: string,
  method: string,
  id: string,
  kind: FeatureKind,
  identityPrefix: readonly SemanticMemberKey[] = [],
): ReferenceRuntime {
  requireId(id, "declaration ID");
  return {
    project: project.name,
    identity: [...identityPrefix, id],
    declaration: id,
    namespace,
    method,
    path: [],
    kind,
  };
}

function keyedCollection<Key extends PropertyKey, Value>(
  keys: readonly Key[],
  build: (key: Key) => Value,
): KeyedFeatureCollection<Key, Value> {
  const byKey = Object.create(null) as Record<Key, Value>;
  const seen = new Set<PropertyKey>();
  for (const key of keys) {
    if (seen.has(key)) throw new TypeError(`duplicate child key ${JSON.stringify(String(key))}`);
    seen.add(key);
    byKey[key] = build(key);
  }
  return Object.freeze({ keys: Object.freeze([...keys]), byKey: Object.freeze(byKey) });
}

function managedPolyline<Project, Key extends string>(
  runtime: ReferenceRuntime,
  values: PolylineValues<Project, Key>,
): PolylineFeature<Project, Key> {
  if (values.vertices.length < 2) throw new TypeError("polyline requires at least two vertices");
  const keys = values.vertices.map((vertex) => vertex.key);
  const segmentKeys = values.closed === true ? keys : keys.slice(0, -1);
  const vertices = keyedCollection(keys, (key) => makeReference<Project, "point">({
    ...runtime,
    path: ["vertices", { member: key }],
    kind: "point",
  }));
  const segments = keyedCollection(segmentKeys, (key) => makeReference<Project, "curve_span">({
    ...runtime,
    path: ["segments", { member: key }],
    kind: "curve_span",
  }) as NativeCurveSpanRef<Project>);
  const filletableCorners = keyedCollection(keys, (key) => makeReference<Project, "feature_corner">({
    ...runtime,
    path: ["filletableCorners", { member: key }],
    kind: "feature_corner",
  })) as DerivedFeatureCollection<PolylineFeature<Project, Key>, Key, FeatureCornerRef<Project>>;
  return lazyReference<Project>(runtime, {
    curve: makeReference<Project, "curve">({ ...runtime, path: ["curve"], kind: "curve" }),
    vertices,
    segments,
    filletableCorners,
  }) as PolylineFeature<Project, Key>;
}

function managedNurbs<Project, Key extends string>(
  runtime: ReferenceRuntime,
  values: NurbsValues<Project, Key>,
  periodic: boolean,
): NurbsFeature<Project, Key> {
  const minimum = periodic ? 3 : 2;
  if (values.controls.length < minimum) {
    throw new TypeError(`${runtime.method} requires at least ${minimum} controls`);
  }
  if (!Number.isInteger(values.degree) || values.degree < 1) {
    throw new TypeError("NURBS degree must be a positive integer");
  }
  const keys = values.controls.map((control) => control.key);
  if (!keys.includes(values.gauge)) throw new TypeError("NURBS gauge must name a control key");
  for (const control of values.controls) requireFinite(control.weight, "NURBS control weight");
  const controls = keyedCollection(keys, (key) => Object.freeze({
    position: makeReference<Project, "point">({
      ...runtime,
      path: ["controls", { member: key }, "position"],
      kind: "point",
    }),
    weight: makeReference<Project, "scalar">({
      ...runtime,
      path: ["controls", { member: key }, "weight"],
      kind: "scalar",
    }),
  }));
  const spanCount = periodic ? keys.length : Math.max(0, keys.length - values.degree);
  const spanKeys = keys.slice(0, spanCount);
  const spans = keyedCollection(spanKeys, (key) => makeReference<Project, "curve_span">({
    ...runtime,
    path: ["spans", { member: key }],
    kind: "curve_span",
  }) as NativeCurveSpanRef<Project>);
  return lazyReference<Project>(runtime, {
    curve: makeReference<Project, "curve">({ ...runtime, path: ["curve"], kind: "curve" }),
    controls,
    spans,
  }) as NurbsFeature<Project, Key>;
}

const HOST_ONLY_AUTHORING_FAMILIES = new Set([
  "computed.fillet",
  "computed.roundedRectangleProfile",
]);

function requireAuthoringFamily(
  namespace: string,
  method: string,
  scope: "sketch" | "patch",
): void {
  const family = `${namespace}.${method}`;
  if (scope === "patch" && HOST_ONLY_AUTHORING_FAMILIES.has(family)) return;
  const availability = (
    AUTHORING_METHOD_CATALOG as Readonly<Record<string, string>>
  )[family];
  if (availability === "requires_host_snapshot") {
    throw new TypeError(
      `authoring method ${family} requires immutable host-snapshot authority`,
    );
  }
  if (availability !== "public") {
    throw new TypeError(`unsupported authoring method ${family}`);
  }
}

function namespaceProxy<Project>(
  project: { readonly name: string },
  namespace: string,
  rootKind: FeatureKind,
  usedIds: Set<string>,
  scope: "sketch" | "patch",
  identityPrefix:
    | readonly SemanticMemberKey[]
    | (() => readonly SemanticMemberKey[]) = [],
): object {
  return new Proxy(Object.create(null) as object, {
    get: (_target, property) => {
      if (typeof property !== "string") return undefined;
      return (id: string, values: Readonly<Record<string, unknown>>) => {
        requireAuthoringFamily(namespace, property, scope);
        requireId(id, "declaration ID");
        const prefix = typeof identityPrefix === "function" ? identityPrefix() : identityPrefix;
        const occurrence = JSON.stringify([...prefix, id]);
        if (usedIds.has(occurrence)) {
          throw new TypeError(`duplicate declaration ID ${JSON.stringify(id)}`);
        }
        usedIds.add(occurrence);
        validateAuthoringValues(namespace, property, values);
        const family = `${namespace}.${property}`;
        const catalog = catalogRuntimeEntry(family);
        const runtime = declarationRuntime(
          project,
          namespace,
          property,
          id,
          catalog?.feature_kind ?? rootKind,
          prefix,
        );
        if (namespace === "geometry" && property === "polyline") {
          return managedPolyline(runtime, values as unknown as PolylineValues<Project, string>);
        }
        if (namespace === "geometry" && property === "openControlNurbs") {
          return managedNurbs(runtime, values as unknown as NurbsValues<Project, string>, false);
        }
        if (namespace === "geometry" && property === "periodicControlNurbs") {
          return managedNurbs(runtime, values as unknown as NurbsValues<Project, string>, true);
        }
        if (namespace === "computed" && property === "filletSet") {
          const corners = (values.corners ?? []) as readonly FilletCorner<Project, string>[];
          if (corners.length === 0) throw new TypeError("filletSet requires at least one corner");
          const keys = corners.map((corner) => corner.key);
          const fillets = keyedCollection(keys, (key) => Object.freeze({
            corner: makeReference<Project, "feature_corner">({
              ...runtime,
              path: ["fillets", { member: key }, "corner"],
              kind: "feature_corner",
            }),
            arc: makeReference<Project, "curve_span">({
              ...runtime,
              path: ["fillets", { member: key }, "arc"],
              kind: "curve_span",
            }),
          }));
          return lazyReference<Project>(runtime, { fillets });
        }
        return catalog === undefined
          ? lazyReference<Project>(runtime)
          : shapedReference<Project>(runtime, runtimeShape(catalog.outputs));
      };
    },
  });
}

function createSketchBuilder(): SketchBuilder<SketchExecutionProject> {
  const project = Object.freeze({ name: SKETCH_PROJECT_NAME }) as SketchExecutionProject;
  const usedIds = new Set<string>();
  return Object.freeze({
    geometry: namespaceProxy(project, "geometry", "feature", usedIds, "sketch"),
    constraint: namespaceProxy(project, "constraint", "constraint", usedIds, "sketch"),
    dimension: namespaceProxy(project, "dimension", "dimension", usedIds, "sketch"),
    operation: namespaceProxy(project, "operation", "operation", usedIds, "sketch"),
    aggregate: namespaceProxy(project, "aggregate", "feature", usedIds, "sketch"),
    computed: namespaceProxy(project, "computed", "feature", usedIds, "sketch"),
    use: <Schemas extends InputSchemas, Result>(
      id: string,
      patch: PatchDefinition<Schemas, Result>,
      inputs: PatchInvocationInputs<Schemas, SketchExecutionProject>,
    ): Reproject<Result, SketchExecutionProject> => {
      requireId(id, "patch application ID");
      if (usedIds.has(id)) throw new TypeError(`duplicate declaration ID ${JSON.stringify(id)}`);
      usedIds.add(id);
      const builder = createPatchBuilder(project, [id]);
      return patch[patchRuntime].build(
        builder as unknown as PatchBuilder<PatchProject>,
        inputs as unknown as PatchInputs<Schemas, PatchProject>,
      ) as Reproject<Result, SketchExecutionProject>;
    },
    group: (label: string) => {
      if (label.length === 0) throw new TypeError("group label cannot be empty");
    },
    suppress: (declaration: OutputRef<SketchExecutionProject, FeatureKind>) => {
      referenceData(declaration);
    },
  }) as unknown as SketchBuilder<SketchExecutionProject>;
}

function createPatchBuilder<Project>(
  project: { readonly name: string },
  identityPrefix: readonly SemanticMemberKey[],
): PatchBuilder<Project> {
  const usedIds = new Set<string>();
  let memberIdentity: readonly SemanticMemberKey[] = [];
  const prefix = () => [...identityPrefix, ...memberIdentity];
  return Object.freeze({
    geometry: namespaceProxy(project, "geometry", "feature", usedIds, "patch", prefix),
    constraint: namespaceProxy(project, "constraint", "constraint", usedIds, "patch", prefix),
    dimension: namespaceProxy(project, "dimension", "dimension", usedIds, "patch", prefix),
    operation: namespaceProxy(project, "operation", "operation", usedIds, "patch", prefix),
    aggregate: namespaceProxy(project, "aggregate", "feature", usedIds, "patch", prefix),
    computed: namespaceProxy(project, "computed", "feature", usedIds, "patch", prefix),
    each: <Key extends PropertyKey, Value, Result>(
      collection: KeyedFeatureCollection<Key, Value>,
      build: (value: Value & { readonly key: Key }) => Result,
    ): DerivedFeatureCollection<typeof collection, Key, Result> => {
      const result = keyedCollection(collection.keys, (key) => {
        const value = collection.byKey[key];
        if (typeof value !== "object" || value === null) {
          throw new TypeError("each requires object-like collection members");
        }
        const prior = memberIdentity;
        memberIdentity = [...prior, String(key)];
        try {
          return build(Object.freeze({ ...value, key }));
        } finally {
          memberIdentity = prior;
        }
      });
      return result as DerivedFeatureCollection<typeof collection, Key, Result>;
    },
    mapRecord: <RecordType extends Readonly<Record<PropertyKey, unknown>>, Result>(
      record: RecordType,
      build: (value: RecordType[keyof RecordType]) => Result,
    ): { readonly [Key in keyof RecordType]: Result } => Object.fromEntries(
      Reflect.ownKeys(record).map((key) => {
        const prior = memberIdentity;
        memberIdentity = [...prior, String(key)];
        try {
          return [key, build(record[key as keyof RecordType])];
        } finally {
          memberIdentity = prior;
        }
      }),
    ) as { readonly [Key in keyof RecordType]: Result },
  }) as unknown as PatchBuilder<Project>;
}

function referenceData(value: unknown): ReferenceRuntime {
  if (typeof value !== "object" || value === null || !(referenceRuntime in value)) {
    throw new TypeError("expected a typed GeoSolve output reference");
  }
  return (value as { readonly [referenceRuntime]: ReferenceRuntime })[referenceRuntime];
}

function requireId(value: string, description: string): void {
  if (typeof value !== "string" || !/^[A-Za-z][A-Za-z0-9_.-]{0,255}$/u.test(value)) {
    throw new TypeError(`invalid ${description} ${JSON.stringify(value)}`);
  }
}

function requireFinite(value: number, description: string): void {
  if (!Number.isFinite(value)) throw new TypeError(`${description} must be finite`);
}

function validateAuthoringValues(
  namespace: string,
  method: string,
  values: Readonly<Record<string, unknown>>,
): void {
  const requiredFinite = namespace === "geometry" && method === "rationalQuadraticConic"
    ? ["middleWeight"]
    : namespace === "geometry" && (method === "parabola" || method === "hyperbola")
      ? ["trimStart", "trimEnd"]
      : [];
  for (const field of requiredFinite) {
    const value = values[field];
    if (typeof value !== "number") {
      throw new TypeError(`${method}.${field} must be a finite number`);
    }
    requireFinite(value, `${method}.${field}`);
  }
}
