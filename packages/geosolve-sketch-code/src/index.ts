// SPDX-License-Identifier: GPL-3.0-or-later

import { DECLARATION_RESULT_CATALOG } from "./generated-declaration-results.js";

export { DECLARATION_RESULT_CATALOG } from "./generated-declaration-results.js";

/**
 * Typed, equation-free authoring vocabulary for optional GeoSolve code
 * projects. Values in this module carry semantic project/symbol/output paths;
 * they never contain intent node IDs, solver equations, or evaluated geometry.
 */

declare const projectTypeBrand: unique symbol;
declare const projectReferenceBrand: unique symbol;
declare const outputKindBrand: unique symbol;
declare const nativeCurveSpanBrand: unique symbol;
declare const featureOutputsBrand: unique symbol;
declare const collectionOwnerBrand: unique symbol;
declare const managedSketchBrand: unique symbol;
declare const managedSketchProjectBrand: unique symbol;
declare const unitLiteralBrand: unique symbol;

const MANAGED_SKETCH_PROJECT_NAME = "__geosolve_managed_v1__" as const;

const referenceRuntime = Symbol("geosolve.sketch-code.reference");
const collectionRuntime = Symbol("geosolve.sketch-code.collection");
const schemaRuntime = Symbol("geosolve.sketch-code.schema");
const patchRuntime = Symbol("geosolve.sketch-code.patch");
const symbolicMemberKeyRuntime = Symbol("geosolve.sketch-code.member-key");

export const SKETCH_CODE_SDK_ABI = "geosolve-sketch-code-v1" as const;
export const PATCH_ARTIFACT_FORMAT = "geosolve-patch-artifact-v1" as const;
export const PATCH_ARTIFACT_LIMIT = 16 * 1024 * 1024;

export type FeatureKind =
  | "point"
  | "curve"
  | "curve_span"
  | "scalar"
  | "constraint"
  | "dimension"
  | "profile"
  | "chain"
  | "operation"
  | "feature"
  | "feature_corner"
  | "collection";

export type SemanticPathSegment = string | number | { readonly member: string };

/** A caller-selected namespace which makes every reference project-local. */
export interface SketchProject<Name extends string = string> {
  readonly name: Name;
  readonly [projectTypeBrand]: (name: Name) => Name;
}

/** A semantic output reference. The inaccessible brands prevent raw-ID construction. */
export interface OutputRef<Project, Kind extends FeatureKind> {
  readonly [projectReferenceBrand]: (project: Project) => Project;
  readonly [outputKindBrand]: Kind;
}

/** A declaration reference with descriptor-owned named outputs. */
export type FeatureRef<Project, Kind extends FeatureKind, Outputs> =
  & OutputRef<Project, Kind>
  & Readonly<Outputs>
  & { readonly [featureOutputsBrand]: Outputs };

export type AnyFeatureRef = FeatureRef<any, FeatureKind, any>;

export type FeatureRecord<
  RecordType extends Readonly<Record<PropertyKey, AnyFeatureRef>>,
> = { readonly [Key in keyof RecordType]: RecordType[Key] };

export interface KeyedFeatureCollection<Key extends PropertyKey, Value> {
  readonly keys: readonly Key[];
  readonly byKey: Readonly<Record<Key, Value>>;
}

export type DerivedFeatureCollection<Owner, Key extends PropertyKey, Value> =
  & KeyedFeatureCollection<Key, Value>
  & { readonly [collectionOwnerBrand]: Owner };

type DescriptorResult<
  Project,
  Shape,
  Key extends PropertyKey = never,
  Owner = never,
> = Shape extends { readonly shape: "leaf"; readonly kind: infer Kind extends FeatureKind }
  ? OutputRef<Project, Kind>
  : Shape extends { readonly shape: "native_span" }
    ? NativeCurveSpanRef<Project>
  : Shape extends {
      readonly shape: "native_span_keyed";
      readonly derived_from_owner: infer Derived;
    }
    ? Derived extends true
      ? DerivedFeatureCollection<Owner, Key, NativeCurveSpanRef<Project>>
      : KeyedFeatureCollection<Key, NativeCurveSpanRef<Project>>
  : Shape extends {
      readonly shape: "keyed";
      readonly kind: infer Kind extends FeatureKind;
      readonly derived_from_owner: infer Derived;
    }
    ? Derived extends true
      ? DerivedFeatureCollection<Owner, Key, OutputRef<Project, Kind>>
      : KeyedFeatureCollection<Key, OutputRef<Project, Kind>>
    : Shape extends {
        readonly shape: "object";
        readonly fields: infer Fields;
      }
      ? { readonly [Field in keyof Fields]: DescriptorResult<Project, Fields[Field], Key, Owner> }
      : never;

type DescriptorKeyedValue<Project, Shape> = Shape extends {
  readonly shape: "keyed";
  readonly kind: infer Kind extends FeatureKind;
} ? OutputRef<Project, Kind> : never;

export type PointRef<Project> = OutputRef<Project, "point">;
export type CurveRef<Project> = OutputRef<Project, "curve">;
export type CurveSpanRef<Project> = OutputRef<Project, "curve_span">;
/** A curve span already backed by an ordinary Intent/native span. */
export type NativeCurveSpanRef<Project> = CurveSpanRef<Project> & {
  readonly [nativeCurveSpanBrand]: true;
};
export type ScalarRef<Project> = OutputRef<Project, "scalar">;
export type FeatureCornerRef<Project> = OutputRef<Project, "feature_corner">;
export type ProfileRef<Project> = OutputRef<Project, "profile">;
export type PointLikeRef<Project> = PointRef<Project> | FeatureCornerRef<Project>;

export type HorizontalConstraintRef<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["constraint.horizontal"]["outputs"]
>;

export type VerticalConstraintRef<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["constraint.vertical"]["outputs"]
>;

export type RectangleOutputs<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["geometry.rectangle"]["outputs"]
>;

export type RectangleFeature<Project> = FeatureRef<
  Project,
  "feature",
  RectangleOutputs<Project>
>;

export type LineOutputs<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["geometry.line"]["outputs"]
>;

export type LineFeature<Project> = FeatureRef<Project, "feature", LineOutputs<Project>>;

export type CircleOutputs<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["geometry.circle"]["outputs"]
>;

export type CircleFeature<Project> = FeatureRef<Project, "feature", CircleOutputs<Project>>;

export type FilletOutputs<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["computed.fillet"]["outputs"]
>;

export type FilletFeature<Project> = FeatureRef<Project, "feature", FilletOutputs<Project>>;

export type FilletSetOutputs<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["computed.filletSet"]["outputs"]
>;

/** One branch-explicit native computed FilletSet declaration. */
export type FilletSetFeature<Project> = FeatureRef<
  Project,
  "feature",
  FilletSetOutputs<Project>
>;

export type RoundedRectangleOutputs<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["geometry.rounded_rectangle"]["outputs"]
>;

export type RoundedRectangleFeature<Project> = FeatureRef<
  Project,
  "feature",
  RoundedRectangleOutputs<Project>
>;

type PolylineResultDescriptor = (typeof DECLARATION_RESULT_CATALOG)["geometry.polyline"]["outputs"];

export interface PolylineOutputs<Project, Key extends PropertyKey> {
  readonly vertices: DescriptorResult<
    Project,
    PolylineResultDescriptor["fields"]["vertices"],
    Key
  >;
  readonly segments: DescriptorResult<
    Project,
    PolylineResultDescriptor["fields"]["segments"],
    Key
  >;
  readonly filletableCorners: DerivedFeatureCollection<
    PolylineFeature<Project, Key>,
    Key,
    DescriptorKeyedValue<
      Project,
      PolylineResultDescriptor["fields"]["filletableCorners"]
    >
  >;
}

export type PolylineFeature<Project, Key extends PropertyKey> = FeatureRef<
  Project,
  "feature",
  PolylineOutputs<Project, Key>
>;

export type GenericFeature<Project> = FeatureRef<Project, "feature", object>;

interface BindingInput {
  readonly source: "input";
  readonly name: string;
  readonly path: readonly SemanticPathSegment[];
  readonly expected_kind: FeatureKind;
}

interface BindingMember {
  readonly source: "collection_member";
  readonly input: string;
  readonly path: readonly SemanticPathSegment[];
  readonly expected_kind: FeatureKind;
}

interface BindingTemplate {
  readonly source: "template_output";
  readonly templateId: number;
  readonly output: string;
  readonly expected_kind: FeatureKind;
}

type RuntimeBinding = BindingInput | BindingMember | BindingTemplate;

interface ReferenceRuntime {
  readonly project: string;
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
  readonly expectedKind: FeatureKind;
  readonly generation: number;
  readonly binding?: RuntimeBinding;
  readonly templateId?: number;
}

interface CollectionRuntime {
  readonly inputName?: string;
  readonly memberKind: FeatureKind;
  readonly memberFactory: () => unknown;
  readonly ruleId?: number;
}

function isReference(value: unknown): value is { readonly [referenceRuntime]: ReferenceRuntime } {
  return typeof value === "object" && value !== null && referenceRuntime in value;
}

function referenceData(value: unknown): ReferenceRuntime {
  if (!isReference(value)) throw new TypeError("expected a typed semantic reference");
  return value[referenceRuntime];
}

function collectionData(value: unknown): CollectionRuntime | undefined {
  if (typeof value !== "object" || value === null || !(collectionRuntime in value)) return undefined;
  return (value as { readonly [collectionRuntime]: CollectionRuntime })[collectionRuntime];
}

function output<Project, Kind extends FeatureKind>(
  root: ReferenceRuntime,
  field: string,
  kind: Kind,
): OutputRef<Project, Kind> {
  return outputPath(root, [field], kind);
}

function outputPath<Project, Kind extends FeatureKind>(
  root: ReferenceRuntime,
  path: readonly string[],
  kind: Kind,
): OutputRef<Project, Kind> {
  const binding = root.binding === undefined
    ? undefined
    : root.binding.source === "template_output"
      ? { ...root.binding, output: path.join("."), expected_kind: kind }
      : root.binding.source === "input"
        ? {
          ...root.binding,
          path: [...root.binding.path, ...path],
          expected_kind: kind,
        }
        : { ...root.binding, path: [...root.binding.path, ...path], expected_kind: kind };
  return makeReference({
    ...root,
    path: [...root.path, ...path],
    expectedKind: kind,
    ...(binding === undefined ? {} : { binding }),
  });
}

function nativeCurveSpanOutput<Project>(
  root: ReferenceRuntime,
  path: readonly string[],
): NativeCurveSpanRef<Project> {
  return outputPath<Project, "curve_span">(root, path, "curve_span") as
    NativeCurveSpanRef<Project>;
}

function namedTemplateOutput<Project, Kind extends FeatureKind>(
  root: ReferenceRuntime,
  semanticPath: readonly string[],
  templateOutput: string,
  kind: Kind,
): OutputRef<Project, Kind> {
  if (root.binding?.source !== "template_output") {
    return outputPath(root, semanticPath, kind);
  }
  return makeReference({
    ...root,
    path: [...root.path, ...semanticPath],
    expectedKind: kind,
    binding: { ...root.binding, output: templateOutput, expected_kind: kind },
  });
}

function makeReference<Project, Kind extends FeatureKind>(
  runtime: ReferenceRuntime,
): OutputRef<Project, Kind> {
  const value = Object.create(null) as Record<PropertyKey, unknown>;
  Object.defineProperty(value, referenceRuntime, { value: runtime });
  return Object.freeze(value) as unknown as OutputRef<Project, Kind>;
}

function featureReference<Project, Kind extends FeatureKind, Outputs>(
  runtime: ReferenceRuntime,
  outputs: (root: ReferenceRuntime) => Outputs,
): FeatureRef<Project, Kind, Outputs> {
  const value = Object.create(null) as Record<PropertyKey, unknown>;
  Object.defineProperty(value, referenceRuntime, { value: runtime });
  Object.assign(value, outputs(runtime));
  return Object.freeze(value) as FeatureRef<Project, Kind, Outputs>;
}

export function createProject<const Name extends string>(name: Name): SketchProject<Name> {
  if (name === MANAGED_SKETCH_PROJECT_NAME) {
    throw new TypeError("managed sketch project name is reserved");
  }
  if (!validKey(name)) throw new TypeError(`invalid project key ${JSON.stringify(name)}`);
  return Object.freeze({ name }) as SketchProject<Name>;
}

function rootRuntime(project: SketchProject<any>, declaration: string): ReferenceRuntime {
  if (!validKey(declaration)) throw new TypeError(`invalid semantic symbol ${JSON.stringify(declaration)}`);
  return { project: project.name, declaration, path: [], expectedKind: "feature", generation: 0 };
}

function rectangleFromRuntime<Project>(root: ReferenceRuntime): RectangleFeature<Project> {
  return featureReference(root, (runtime) => ({
    corners: Object.freeze({
      lowerLeft: outputPath<Project, "feature_corner">(
        runtime,
        ["corners", "lowerLeft"],
        "feature_corner",
      ),
      lowerRight: outputPath<Project, "feature_corner">(
        runtime,
        ["corners", "lowerRight"],
        "feature_corner",
      ),
      upperRight: outputPath<Project, "feature_corner">(
        runtime,
        ["corners", "upperRight"],
        "feature_corner",
      ),
      upperLeft: outputPath<Project, "feature_corner">(
        runtime,
        ["corners", "upperLeft"],
        "feature_corner",
      ),
    }),
    edges: Object.freeze({
      bottom: nativeCurveSpanOutput<Project>(runtime, ["edges", "bottom"]),
      right: nativeCurveSpanOutput<Project>(runtime, ["edges", "right"]),
      top: nativeCurveSpanOutput<Project>(runtime, ["edges", "top"]),
      left: nativeCurveSpanOutput<Project>(runtime, ["edges", "left"]),
    }),
    profile: output<Project, "profile">(runtime, "profile", "profile"),
  }));
}

export function rectangle<Project extends SketchProject<any>>(
  project: Project,
  declaration: string,
): RectangleFeature<Project> {
  return rectangleFromRuntime(rootRuntime(project, declaration));
}

export function point<Project extends SketchProject<any>>(
  project: Project,
  declaration: string,
): PointRef<Project> {
  const root = rootRuntime(project, declaration);
  return makeReference({ ...root, path: ["point"], expectedKind: "point" });
}

function lineFromRuntime<Project>(root: ReferenceRuntime): LineFeature<Project> {
  return featureReference(root, (runtime) => ({
    start: output<Project, "point">(runtime, "start", "point"),
    end: output<Project, "point">(runtime, "end", "point"),
    span: nativeCurveSpanOutput<Project>(runtime, ["span"]),
  }));
}

export function line<Project extends SketchProject<any>>(
  project: Project,
  declaration: string,
  start: PointLikeRef<NoInfer<Project>>,
  end: PointLikeRef<NoInfer<Project>>,
): LineFeature<Project> {
  referenceData(start);
  referenceData(end);
  return lineFromRuntime(rootRuntime(project, declaration));
}

function filletFromRuntime<Project>(root: ReferenceRuntime): FilletFeature<Project> {
  return featureReference(root, (runtime) => ({
    arc: output<Project, "curve_span">(runtime, "arc", "curve_span"),
  }));
}

function roundedRectangleFromRuntime<Project>(
  root: ReferenceRuntime,
): RoundedRectangleFeature<Project> {
  return featureReference(root, (runtime) => ({
    profile: namedTemplateOutput<Project, "profile">(runtime, ["profile"], "profile", "profile"),
    mounts: Object.freeze({
      nw: namedTemplateOutput<Project, "point">(runtime, ["mounts", "nw"], "nw", "point"),
      ne: namedTemplateOutput<Project, "point">(runtime, ["mounts", "ne"], "ne", "point"),
      se: namedTemplateOutput<Project, "point">(runtime, ["mounts", "se"], "se", "point"),
      sw: namedTemplateOutput<Project, "point">(runtime, ["mounts", "sw"], "sw", "point"),
    }),
  }));
}

/** Preserve the exact caller-owned record keys in the mapped Fillet result. */
export function fillets<
  Project extends SketchProject<any>,
  Corners extends Readonly<Record<PropertyKey, FeatureCornerRef<NoInfer<Project>>>>,
>(
  project: Project,
  corners: Corners,
): FeatureRecord<{ readonly [Key in keyof Corners]: FilletFeature<Project> }> {
  return Object.fromEntries(
    Reflect.ownKeys(corners).map((key) => [
      key,
      filletFromRuntime(rootRuntime(project, `fillet.${String(key)}`)),
    ]),
  ) as FeatureRecord<{ readonly [Key in keyof Corners]: FilletFeature<Project> }>;
}

export function polyline<
  Project extends SketchProject<any>,
  const Key extends string,
>(
  project: Project,
  declaration: string,
  keys: readonly Key[],
): PolylineFeature<Project, Key> {
  const root = rootRuntime(project, declaration);
  const vertices = keyedCollection(keys, (key) =>
    makeReference<Project, "point">({ ...root, path: ["vertices", { member: key }], expectedKind: "point" }));
  const segments = keyedCollection(keys, (key) =>
    makeReference<Project, "curve_span">({
      ...root,
      path: ["segments", { member: key }],
      expectedKind: "curve_span",
    }) as NativeCurveSpanRef<Project>);
  const corners = keyedCollection(keys, (key) =>
    makeReference<Project, "feature_corner">({
      ...root,
      path: ["filletableCorners", { member: key }],
      expectedKind: "feature_corner",
    }));
  return featureReference(root, () => ({
    vertices,
    segments,
    filletableCorners: corners as DerivedFeatureCollection<
      PolylineFeature<Project, Key>,
      Key,
      FeatureCornerRef<Project>
    >,
  }));
}

function keyedCollection<Key extends PropertyKey, Value>(
  keys: readonly Key[],
  create: (key: Key) => Value,
): KeyedFeatureCollection<Key, Value> {
  const unique = new Set<PropertyKey>();
  const byKey = Object.create(null) as Record<Key, Value>;
  for (const key of keys) {
    if (unique.has(key)) throw new TypeError(`duplicate collection key ${String(key)}`);
    unique.add(key);
    byKey[key] = create(key);
  }
  return Object.freeze({ keys: Object.freeze([...keys]), byKey: Object.freeze(byKey) });
}

export interface UnitLiteral<Unit extends string = string> {
  readonly unit: Unit;
  readonly value: number;
  readonly [unitLiteralBrand]: Unit;
}

export function mm(value: number): UnitLiteral<"mm"> {
  requireFinite(value, "millimetre value");
  return Object.freeze({ unit: "mm", value }) as UnitLiteral<"mm">;
}

export interface ValueSchema<Value> {
  readonly [schemaRuntime]: SchemaRuntime;
  readonly __value?: Value;
}

interface SchemaRuntime {
  readonly kind: FeatureKind;
  readonly feature?: string;
  readonly element?: SchemaRuntime;
  readonly fields?: Readonly<Record<string, SchemaRuntime>>;
}

type SchemaValue<Schema, Project> = Schema extends ValueSchema<infer Value>
  ? Reproject<Value, Project>
  : never;

type Reproject<Value, Project> =
  Value extends RectangleFeature<any> ? RectangleFeature<Project>
    : Value extends PolylineFeature<any, infer Key> ? PolylineFeature<Project, Key>
    : Value extends LineFeature<any> ? LineFeature<Project>
    : Value extends CircleFeature<any> ? CircleFeature<Project>
    : Value extends FilletFeature<any> ? FilletFeature<Project>
    : Value extends RoundedRectangleFeature<any> ? RoundedRectangleFeature<Project>
    : Value extends NativeCurveSpanRef<any> ? NativeCurveSpanRef<Project>
    : Value extends KeyedFeatureCollection<infer Key, infer Element>
      ? KeyedFeatureCollection<Key, Reproject<Element, Project>>
    : Value extends OutputRef<any, infer Kind> ? OutputRef<Project, Kind>
    : Value extends Readonly<Record<PropertyKey, unknown>>
      ? { readonly [Key in keyof Value]: Reproject<Value[Key], Project> }
    : Value;

export type InputSchemas = Readonly<Record<string, ValueSchema<unknown>>>;
export type PatchInputs<Schemas extends InputSchemas, Project> = {
  readonly [Key in keyof Schemas]: SchemaValue<Schemas[Key], Project>;
};

type FeatureSchemaValue<Name extends string> = Name extends "rectangle"
  ? RectangleFeature<unknown>
  : Name extends "polyline"
    ? PolylineFeature<unknown, string>
    : GenericFeature<unknown>;

function schema<Value>(runtime: SchemaRuntime): ValueSchema<Value> {
  return Object.freeze({ [schemaRuntime]: runtime }) as ValueSchema<Value>;
}

export const t = Object.freeze({
  point: (): ValueSchema<PointRef<unknown>> => schema({ kind: "point" }),
  corner: (): ValueSchema<FeatureCornerRef<unknown>> => schema({ kind: "feature_corner" }),
  curve: (): ValueSchema<CurveRef<unknown>> => schema({ kind: "curve" }),
  curveSpan: (): ValueSchema<CurveSpanRef<unknown>> => schema({ kind: "curve_span" }),
  length: (): ValueSchema<ScalarRef<unknown>> => schema({ kind: "scalar" }),
  scalar: (): ValueSchema<ScalarRef<unknown>> => schema({ kind: "scalar" }),
  feature: <const Name extends string>(name: Name): ValueSchema<FeatureSchemaValue<Name>> =>
    schema({ kind: "feature", feature: name }),
  keyed: <Value>(element: ValueSchema<Value>): ValueSchema<KeyedFeatureCollection<string, Value>> =>
    schema({ kind: "collection", element: element[schemaRuntime] }),
  record: <Value>(element: ValueSchema<Value>): ValueSchema<Readonly<Record<string, Value>>> =>
    schema({ kind: "collection", element: element[schemaRuntime] }),
  fields: <const Fields extends Readonly<Record<string, ValueSchema<unknown>>>>(
    fields: Fields,
  ): ValueSchema<{ readonly [Key in keyof Fields]: SchemaValue<Fields[Key], unknown> }> =>
    schema({
      kind: "collection",
      fields: Object.fromEntries(
        Object.entries(fields).map(([key, value]) => [key, value[schemaRuntime]]),
      ),
    }),
});

export interface EachOptions<Value> {
  readonly memberKeyField?: string;
  readonly key?: (value: Value) => PropertyKey;
}

export interface EditLensDeclaration {
  readonly output: readonly SemanticPathSegment[];
  readonly invocationArgument: readonly string[];
  readonly expectedKind: FeatureKind;
}

export interface PatchRecorder<Project> {
  fillet(
    values: {
      readonly corner: FeatureCornerRef<Project>;
      readonly radius: ScalarRef<Project> | UnitLiteral;
    },
  ): FilletFeature<Project>;
  line(start: PointLikeRef<Project>, end: PointLikeRef<Project>): LineFeature<Project>;
  circle(center: PointRef<Project>, radius: ScalarRef<Project> | UnitLiteral): CircleFeature<Project>;
  roundedRectangle(
    width: ScalarRef<Project> | UnitLiteral,
    height: ScalarRef<Project> | UnitLiteral,
    cornerRadius: ScalarRef<Project> | UnitLiteral,
  ): RoundedRectangleFeature<Project>;
  each<Key extends PropertyKey, Value, Result>(
    collection: KeyedFeatureCollection<Key, Value>,
    build: (value: Value & { readonly key: Key }) => Result,
    options?: EachOptions<Value & { readonly key: Key }>,
  ): DerivedFeatureCollection<typeof collection, Key, Result>;
  mapRecord<RecordType extends Readonly<Record<PropertyKey, unknown>>, Result>(
    record: RecordType,
    build: (value: RecordType[keyof RecordType]) => Result,
  ): { readonly [Key in keyof RecordType]: Result };
  record<const Key extends string, Result>(
    keys: readonly Key[],
    build: (key: Key) => Result,
  ): Readonly<Record<Key, Result>>;
  editLens(lens: EditLensDeclaration): void;
}

export interface PatchDefinition<Schemas extends InputSchemas, Result> {
  readonly inputs: Schemas;
  readonly [patchRuntime]: {
    readonly build: (
      recorder: PatchRecorder<PatchBuildProject>,
      inputs: PatchInputs<Schemas, PatchBuildProject>,
    ) => Result;
  };
}

/** Opaque project namespace used while a trusted patch is symbolically recorded. */
export interface PatchBuildProject extends SketchProject<"__geosolve_patch_build__"> {}

export function definePatch<Schemas extends InputSchemas, Result>(
  inputs: Schemas,
  build: (
    recorder: PatchRecorder<PatchBuildProject>,
    inputs: PatchInputs<Schemas, PatchBuildProject>,
  ) => Result,
): PatchDefinition<Schemas, Result> {
  return Object.freeze({ inputs, [patchRuntime]: { build } });
}

/** Project brand used only while type-checking a managed-v1 sketch file. */
export interface ManagedSketchProject
  extends SketchProject<typeof MANAGED_SKETCH_PROJECT_NAME> {
  readonly [managedSketchProjectBrand]: true;
}

export interface ManagedPolylineVertex<Key extends string = string> {
  readonly key: Key;
  readonly position: readonly [number, number];
}

/** A managed line endpoint is either literal geometry or a lexical point output. */
export type ManagedLineEndpoint<Project> =
  | PointLikeRef<NoInfer<Project>>
  | readonly [number, number];

export interface ManagedGeometryBuilder<Project> {
  rectangle(
    symbol: string,
    values: {
      readonly lowerLeft: readonly [number, number];
      readonly upperRight: readonly [number, number];
    },
  ): RectangleFeature<Project>;
  line(
    symbol: string,
    values: {
      readonly start: ManagedLineEndpoint<Project>;
      readonly end: ManagedLineEndpoint<Project>;
    },
  ): LineFeature<Project>;
  polyline<const Vertices extends readonly ManagedPolylineVertex[]>(
    symbol: string,
    values: {
      readonly vertices: Vertices;
      readonly closed: boolean;
    },
  ): PolylineFeature<Project, Vertices[number]["key"]>;
}

export interface ManagedConstraintBuilder<Project> {
  horizontal(
    symbol: string,
    values: {
      readonly curve: NativeCurveSpanRef<Project> | LineFeature<Project>;
      readonly suppressed?: boolean;
    },
  ): HorizontalConstraintRef<Project>;
  vertical(
    symbol: string,
    values: {
      readonly curve: NativeCurveSpanRef<Project> | LineFeature<Project>;
      readonly suppressed?: boolean;
    },
  ): VerticalConstraintRef<Project>;
}

export type ManagedFilletNeighborhood =
  | { readonly kind: "interior" | "start" | "end" }
  | {
    readonly kind: "local";
    readonly lower: number;
    readonly upper: number;
  };

export interface ManagedFilletParent<Project> {
  readonly span: NativeCurveSpanRef<Project>;
  readonly parameter: number;
  readonly winding: number;
  readonly neighborhood: ManagedFilletNeighborhood;
  readonly normalSide: "left" | "right";
  readonly retainedEndpoint: "start" | "end";
  readonly periodicAnchor: null | {
    readonly parameter: number;
    readonly winding: number;
  };
}

export interface ManagedFilletCorner<Project> {
  readonly parents: readonly [
    ManagedFilletParent<Project>,
    ManagedFilletParent<Project>,
  ];
  readonly endpointOrder: "firstThenSecond" | "secondThenFirst";
  readonly sweep: "counterClockwise" | "clockwise";
}

export interface ManagedComputedBuilder<Project> {
  filletSet(
    symbol: string,
    values: {
      readonly radius: number | UnitLiteral<"mm">;
      readonly corners: readonly ManagedFilletCorner<NoInfer<Project>>[];
      readonly suppressed: boolean;
    },
  ): FilletSetFeature<Project>;
}

type ManagedInvocationValue<Value> =
  Value extends OutputRef<any, "scalar"> ? Value | UnitLiteral
    : Value extends KeyedFeatureCollection<infer Key, infer Element>
      ? KeyedFeatureCollection<Key, ManagedInvocationValue<Element>>
    : Value extends Readonly<Record<PropertyKey, unknown>>
      ? { readonly [Key in keyof Value]: ManagedInvocationValue<Value[Key]> }
    : Value;

type ManagedPatchInputs<Schemas extends InputSchemas, Project> = {
  readonly [Key in keyof Schemas]: ManagedInvocationValue<SchemaValue<Schemas[Key], Project>>;
};

type ManagedPatchResult<Result, Project> = Reproject<Result, Project>;

export interface ManagedSketchBuilder<Project> {
  readonly geometry: ManagedGeometryBuilder<Project>;
  readonly constraint: ManagedConstraintBuilder<Project>;
  readonly computed: ManagedComputedBuilder<Project>;
  use<Schemas extends InputSchemas, Result>(
    symbol: string,
    patch: PatchDefinition<Schemas, Result>,
    inputs: ManagedPatchInputs<Schemas, Project>,
  ): ManagedPatchResult<Result, Project>;
  organize(name: string, declarations: readonly unknown[]): void;
  outputs<const Outputs extends Readonly<Record<string, unknown>>>(outputs: Outputs): Outputs;
}

/**
 * Compile-time envelope for the deterministic managed-v1 source subset.
 *
 * The callback is deliberately not executed. Rust parses the source text and
 * the browser consumes pinned data artifacts; this value exists so the exact
 * source users and AI author is checked against the branded SDK vocabulary.
 */
export interface ManagedSketch<Result> {
  readonly [managedSketchBrand]: Result;
}

export function sketch<Result>(
  build: (builder: ManagedSketchBuilder<ManagedSketchProject>) => Result,
): ManagedSketch<Result> {
  if (typeof build !== "function") throw new TypeError("managed sketch requires a builder callback");
  return Object.freeze({}) as ManagedSketch<Result>;
}

export type ArtifactTemplateBinding =
  | {
    readonly source: "input";
    readonly name: string;
    readonly path: readonly SemanticPathSegment[];
    readonly expected_kind: FeatureKind;
  }
  | {
    readonly source: "template_output";
    readonly template: readonly string[];
    readonly output: string;
    readonly expected_kind: FeatureKind;
  }
  | {
    readonly source: "collection_member";
    readonly input: string;
    readonly path: readonly SemanticPathSegment[];
    readonly expected_kind: FeatureKind;
  };

export interface ArtifactTemplateNode {
  readonly path: readonly string[];
  readonly declaration_family: string;
  readonly inputs: Readonly<Record<string, ArtifactTemplateBinding>>;
  readonly fields: Readonly<Record<string, ManagedArtifactValue>>;
  readonly outputs: Readonly<Record<string, FeatureKind>>;
}

export type ManagedArtifactValue =
  | { readonly kind: "null" }
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "number"; readonly value: number }
  | { readonly kind: "string"; readonly value: string }
  | { readonly kind: "unit"; readonly value: UnitLiteral }
  | { readonly kind: "array"; readonly value: readonly ManagedArtifactValue[] }
  | { readonly kind: "object"; readonly value: Readonly<Record<string, ManagedArtifactValue>> };

export type ArtifactCollectionRule =
  | {
    readonly rule: "each";
    readonly path: readonly string[];
    readonly input: string;
    readonly member_key_field: string;
    readonly templates: readonly (readonly string[])[];
  }
  | {
    readonly rule: "map_record";
    readonly path: readonly string[];
    readonly input: string;
    readonly templates: readonly (readonly string[])[];
  };

export interface PatchArtifactPlan {
  readonly inputs: Readonly<Record<string, FeatureKind>>;
  readonly outputs: Readonly<Record<string, FeatureKind>>;
  readonly templates: readonly ArtifactTemplateNode[];
  readonly collections: readonly ArtifactCollectionRule[];
  readonly edit_lenses: readonly {
    readonly output: readonly SemanticPathSegment[];
    readonly invocation_argument: readonly string[];
    readonly expected_kind: FeatureKind;
  }[];
}

interface PendingTemplate {
  readonly id: number;
  path?: string[];
  readonly familyPath: string;
  readonly declarationFamily: string;
  readonly inputs: Record<string, RuntimeBinding>;
  readonly fields: Record<string, ManagedArtifactValue>;
  readonly outputs: Record<string, FeatureKind>;
}

interface PendingRule {
  readonly id: number;
  readonly rule: "each" | "map_record";
  path?: string[];
  readonly input: string;
  readonly memberKeyField?: string;
  readonly templateIds: number[];
}

class StructuralRecorder implements PatchRecorder<PatchBuildProject> {
  readonly templates: PendingTemplate[] = [];
  readonly rules: PendingRule[] = [];
  readonly lenses: EditLensDeclaration[] = [];
  private collectionContext: number[] | undefined;

  fillet(values: {
    readonly corner: FeatureCornerRef<PatchBuildProject>;
    readonly radius: ScalarRef<PatchBuildProject> | UnitLiteral;
  }): FilletFeature<PatchBuildProject> {
    return this.template(
      "fillet",
      "computed.fillet",
      values,
      { arc: "curve_span" },
      (root) => filletFromRuntime<PatchBuildProject>(root),
    );
  }

  line(
    start: PointLikeRef<PatchBuildProject>,
    end: PointLikeRef<PatchBuildProject>,
  ): LineFeature<PatchBuildProject> {
    return this.template(
      "line",
      "geometry.line",
      { start, end },
      { span: "curve_span" },
      (root) => lineFromRuntime<PatchBuildProject>(root),
    );
  }

  circle(
    center: PointRef<PatchBuildProject>,
    radius: ScalarRef<PatchBuildProject> | UnitLiteral,
  ): CircleFeature<PatchBuildProject> {
    return this.template("circle", "geometry.circle", { center, radius }, { circle: "curve" }, (root) =>
      featureReference(root, (runtime) => ({
        center: output<PatchBuildProject, "point">(runtime, "center", "point"),
        circle: output<PatchBuildProject, "curve">(runtime, "circle", "curve"),
      })));
  }

  roundedRectangle(
    width: ScalarRef<PatchBuildProject> | UnitLiteral,
    height: ScalarRef<PatchBuildProject> | UnitLiteral,
    cornerRadius: ScalarRef<PatchBuildProject> | UnitLiteral,
  ): RoundedRectangleFeature<PatchBuildProject> {
    return this.template(
      "profile",
      "geometry.rectangle",
      { width, height, cornerRadius },
      { profile: "profile", nw: "point", ne: "point", se: "point", sw: "point" },
      (root) => roundedRectangleFromRuntime<PatchBuildProject>(root),
    );
  }

  each<Key extends PropertyKey, Value, Result>(
    collection: KeyedFeatureCollection<Key, Value>,
    build: (value: Value & { readonly key: Key }) => Result,
    options?: EachOptions<Value & { readonly key: Key }>,
  ): DerivedFeatureCollection<typeof collection, Key, Result> {
    return this.dynamicCollection(
      "each",
      collection,
      build,
      options?.memberKeyField ?? "key",
      options?.key,
    ) as
      DerivedFeatureCollection<typeof collection, Key, Result>;
  }

  mapRecord<RecordType extends Readonly<Record<PropertyKey, unknown>>, Result>(
    record: RecordType,
    build: (value: RecordType[keyof RecordType]) => Result,
  ): { readonly [Key in keyof RecordType]: Result } {
    return this.dynamicCollection("map_record", record, build) as {
      readonly [Key in keyof RecordType]: Result;
    };
  }

  record<const Key extends string, Result>(
    keys: readonly Key[],
    build: (key: Key) => Result,
  ): Readonly<Record<Key, Result>> {
    const result = Object.create(null) as Record<Key, Result>;
    for (const key of keys) {
      if (Object.hasOwn(result, key)) throw new TypeError(`duplicate static record key ${key}`);
      result[key] = build(key);
    }
    return Object.freeze(result);
  }

  editLens(lens: EditLensDeclaration): void {
    if (lens.output.length === 0 || lens.invocationArgument.length === 0) {
      throw new TypeError("edit lens paths cannot be empty");
    }
    this.lenses.push(lens);
  }

  private dynamicCollection<Result>(
    rule: "each" | "map_record",
    value: unknown,
    build: (value: never) => Result,
    memberKeyField?: string,
    keySelector?: (value: never) => PropertyKey,
  ): unknown {
    const source = collectionData(value);
    if (source?.inputName === undefined) {
      throw new TypeError(`${rule} requires a symbolic collection input`);
    }
    const id = this.rules.length;
    const templateIds: number[] = [];
    const member = source.memberFactory();
    const callbackMember = rule === "each" ? withSymbolicMemberKey(member) : member;
    if (keySelector !== undefined && keySelector(callbackMember as never) !== symbolicMemberKeyRuntime) {
      throw new TypeError("each key selector must return the symbolic member key");
    }
    const before = this.collectionContext;
    this.collectionContext = templateIds;
    try {
      build(callbackMember as never);
    } finally {
      this.collectionContext = before;
    }
    if (templateIds.length === 0) throw new TypeError(`${rule} callback must record a template`);
    this.rules.push({
      id,
      rule,
      input: source.inputName,
      ...(memberKeyField === undefined ? {} : { memberKeyField }),
      templateIds,
    });
    return symbolicCollection(source.inputName, source.memberKind, source.memberFactory, id);
  }

  private template<Result>(
    familyPath: string,
    declarationFamily: string,
    values: Readonly<Record<string, unknown>>,
    outputs: Record<string, FeatureKind>,
    create: (root: ReferenceRuntime) => Result,
  ): Result {
    const id = this.templates.length;
    const bindings: Record<string, RuntimeBinding> = {};
    const fields: Record<string, ManagedArtifactValue> = {};
    for (const [name, value] of Object.entries(values)) {
      if (isReference(value)) {
        const binding = referenceData(value).binding;
        if (binding === undefined) throw new TypeError(`template input ${name} is not symbolic`);
        bindings[name] = binding;
      } else {
        fields[name] = managedValue(value);
      }
    }
    this.templates.push({ id, familyPath, declarationFamily, inputs: bindings, fields, outputs });
    this.collectionContext?.push(id);
    return create({
      project: "__geosolve_patch_build__",
      declaration: `template.${id}`,
      path: [],
      expectedKind: "feature",
      generation: 0,
      templateId: id,
      binding: { source: "template_output", templateId: id, output: "feature", expected_kind: "feature" },
    });
  }
}

function withSymbolicMemberKey(value: unknown): object {
  if (typeof value !== "object" || value === null) {
    throw new TypeError("each requires object-like symbolic collection members");
  }
  const keyed = Object.create(null) as Record<PropertyKey, unknown>;
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor !== undefined) Object.defineProperty(keyed, key, descriptor);
  }
  Object.defineProperty(keyed, "key", {
    value: symbolicMemberKeyRuntime,
    enumerable: true,
  });
  return Object.freeze(keyed);
}

function symbolicCollection(
  inputName: string,
  memberKind: FeatureKind,
  memberFactory: () => unknown,
  ruleId?: number,
): object {
  const value = Object.create(null) as Record<PropertyKey, unknown>;
  Object.defineProperty(value, collectionRuntime, {
    value: { inputName, memberKind, memberFactory, ...(ruleId === undefined ? {} : { ruleId }) },
  });
  return Object.freeze(value);
}

function symbolicInput(name: string, runtime: SchemaRuntime): unknown {
  const binding = (kind: FeatureKind, path: readonly SemanticPathSegment[] = []): BindingInput => ({
    source: "input",
    name,
    path,
    expected_kind: kind,
  });
  const root: ReferenceRuntime = {
    project: "__geosolve_patch_build__",
    declaration: name,
    path: [],
    expectedKind: runtime.kind,
    generation: 0,
    binding: binding(runtime.kind),
  };
  if (runtime.kind === "collection") {
    const element = runtime.element ?? Object.values(runtime.fields ?? {})[0] ?? { kind: "feature" as const };
    return symbolicCollection(name, element.kind, () => symbolicCollectionMember(name, element));
  }
  if (runtime.kind === "feature" && runtime.feature === "rectangle") return rectangleFromRuntime(root);
  if (runtime.kind === "feature" && runtime.feature === "polyline") {
    const corners = symbolicCollection(name, "feature_corner", () =>
      symbolicCollectionMember(name, { kind: "feature_corner" }));
    return featureReference(root, () => ({
      vertices: symbolicCollection(name, "point", () => symbolicCollectionMember(name, { kind: "point" })),
      segments: symbolicCollection(name, "curve_span", () => symbolicCollectionMember(name, { kind: "curve_span" })),
      filletableCorners: corners,
    }));
  }
  if (runtime.kind === "feature") return featureReference(root, () => ({}));
  return makeReference(root);
}

function symbolicCollectionMember(input: string, runtime: SchemaRuntime): unknown {
  const root: ReferenceRuntime = {
    project: "__geosolve_patch_build__",
    declaration: input,
    path: [],
    expectedKind: runtime.kind,
    generation: 0,
    binding: { source: "collection_member", input, path: [], expected_kind: runtime.kind },
  };
  if (runtime.kind === "feature" && runtime.feature === "rectangle") return rectangleFromRuntime(root);
  if (runtime.kind === "feature") return featureReference(root, () => ({}));
  return makeReference(root);
}

/** @internal Used only by the explicit Node compiler entry point. */
export function recordPatchArtifact<Schemas extends InputSchemas, Result>(
  definition: PatchDefinition<Schemas, Result>,
): PatchArtifactPlan {
  const recorder = new StructuralRecorder();
  const inputs = Object.fromEntries(
    sortedEntries(definition.inputs).map(([name, value]) => [name, symbolicInput(name, value[schemaRuntime])]),
  ) as PatchInputs<Schemas, PatchBuildProject>;
  const result = definition[patchRuntime].build(recorder, inputs);
  const outputs: Record<string, FeatureKind> = {};
  visitResult(result, [], outputs, recorder.templates, recorder.rules);
  if (Object.keys(outputs).length === 0) throw new TypeError("patch must return at least one typed output");

  const templatePaths = new Map<number, readonly string[]>();
  for (const template of recorder.templates) {
    if (template.path === undefined) template.path = [template.familyPath];
    templatePaths.set(template.id, template.path);
  }
  const templates = recorder.templates.map((template) => ({
    path: template.path ?? [template.familyPath],
    declaration_family: template.declarationFamily,
    inputs: sortedObject(Object.fromEntries(
      sortedEntries(template.inputs).map(([key, binding]) => [key, artifactBinding(binding, templatePaths)]),
    )),
    fields: sortedObject(template.fields),
    outputs: sortedObject(template.outputs),
  }));
  const collections = recorder.rules.map((rule): ArtifactCollectionRule => {
    const base = {
      path: rule.path ?? [`collection_${rule.id}`],
      input: rule.input,
      templates: rule.templateIds.map((id) => {
        const path = templatePaths.get(id);
        if (path === undefined) throw new TypeError("collection references an unknown template");
        return path;
      }),
    };
    return rule.rule === "each"
      ? { rule: "each", ...base, member_key_field: rule.memberKeyField ?? "key" }
      : { rule: "map_record", ...base };
  });
  return {
    inputs: sortedObject(Object.fromEntries(
      sortedEntries(definition.inputs).map(([name, value]) => [name, value[schemaRuntime].kind]),
    )),
    outputs: sortedObject(outputs),
    templates,
    collections,
    edit_lenses: recorder.lenses.map((lens) => ({
      output: [...lens.output],
      invocation_argument: [...lens.invocationArgument],
      expected_kind: lens.expectedKind,
    })),
  };
}

function visitResult(
  value: unknown,
  path: string[],
  outputs: Record<string, FeatureKind>,
  templates: PendingTemplate[],
  rules: PendingRule[],
): void {
  if (isReference(value)) {
    const runtime = referenceData(value);
    const outputName = path[0] ?? "result";
    if (outputs[outputName] === undefined) outputs[outputName] = runtime.expectedKind;
    if (runtime.templateId !== undefined) {
      const template = templates[runtime.templateId];
      if (template !== undefined && template.path === undefined) {
        template.path = path.length === 0 ? [template.familyPath] : [...path];
      }
    }
    return;
  }
  const collection = collectionData(value);
  if (collection !== undefined) {
    const outputName = path[0] ?? "result";
    outputs[outputName] = "collection";
    if (collection.ruleId !== undefined) {
      const rule = rules[collection.ruleId];
      if (rule !== undefined) rule.path = path.length === 0 ? [outputName] : [...path];
      for (const templateId of rule?.templateIds ?? []) {
        const template = templates[templateId];
        if (template !== undefined && template.path === undefined) template.path = [template.familyPath];
      }
    }
    return;
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("patch outputs must be typed references, records, or keyed collections");
  }
  for (const [key, child] of sortedEntries(value as Record<string, unknown>)) {
    if (path.length === 0 && !validKey(key)) throw new TypeError(`invalid patch output key ${key}`);
    if (path.length === 0 && !isReference(child) && collectionData(child) === undefined) {
      outputs[key] = "collection";
    }
    visitResult(child, [...path, key], outputs, templates, rules);
  }
}

function artifactBinding(
  binding: RuntimeBinding,
  paths: ReadonlyMap<number, readonly string[]>,
): ArtifactTemplateBinding {
  if (binding.source === "template_output") {
    const template = paths.get(binding.templateId);
    if (template === undefined) throw new TypeError("template binding references an unknown template");
    return {
      source: "template_output",
      template,
      output: binding.output,
      expected_kind: binding.expected_kind,
    };
  }
  return binding;
}

function managedValue(value: unknown): ManagedArtifactValue {
  if (value === null) return { kind: "null" };
  if (typeof value === "boolean") return { kind: "bool", value };
  if (typeof value === "number") {
    requireFinite(value, "template number");
    return { kind: "number", value };
  }
  if (typeof value === "string") return { kind: "string", value };
  if (isUnit(value)) return { kind: "unit", value };
  if (Array.isArray(value)) return { kind: "array", value: value.map(managedValue) };
  if (typeof value === "object" && value !== null) {
    return {
      kind: "object",
      value: sortedObject(Object.fromEntries(
        sortedEntries(value as Record<string, unknown>).map(([key, child]) => [key, managedValue(child)]),
      )),
    };
  }
  throw new TypeError("template literals must be finite data-only values");
}

function isUnit(value: unknown): value is UnitLiteral {
  return typeof value === "object" && value !== null
    && Object.keys(value).length === 2
    && typeof (value as { unit?: unknown }).unit === "string"
    && typeof (value as { value?: unknown }).value === "number";
}

function requireFinite(value: number, label: string): void {
  if (!Number.isFinite(value)) throw new TypeError(`${label} must be finite`);
}

function validKey(value: string): boolean {
  return value.length > 0 && value.length <= 256 && /^[A-Za-z0-9_.-]+$/u.test(value);
}

function sortedEntries<Value>(value: Readonly<Record<string, Value>>): [string, Value][] {
  return Object.entries(value).sort(([left], [right]) => byteCompare(left, right));
}

function sortedObject<Value>(value: Readonly<Record<string, Value>>): Readonly<Record<string, Value>> {
  return Object.fromEntries(sortedEntries(value));
}

function byteCompare(left: string, right: string): number {
  const a = new TextEncoder().encode(left);
  const b = new TextEncoder().encode(right);
  const length = Math.min(a.length, b.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return a.length - b.length;
}
