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

export type CoincidentConstraintRef<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["constraint.coincident"]["outputs"]
>;

export type FixedPointConstraintRef<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["constraint.fixedPoint"]["outputs"]
>;

export type FixedCoordinateConstraintRef<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["constraint.fixedCoordinate"]["outputs"]
>;

export type SymmetricAboutDatumAxisConstraintRef<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["constraint.symmetricAboutDatumAxis"]["outputs"]
>;

export type CurveLengthDimensionRef<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["dimension.curveLength"]["outputs"]
>;

export type DiameterDimensionRef<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["dimension.diameter"]["outputs"]
>;

export type RadiusDimensionRef<Project> = DescriptorResult<
  Project,
  (typeof DECLARATION_RESULT_CATALOG)["dimension.radius"]["outputs"]
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
  radius(
    curve: CurveRef<Project>,
    target: ScalarRef<Project> | UnitLiteral,
  ): RadiusDimensionRef<Project>;
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
      readonly role?: "profile" | "construction";
    },
  ): LineFeature<Project>;
  circle(
    symbol: string,
    values: {
      readonly center: ManagedLineEndpoint<Project>;
      readonly radius: number | UnitLiteral<"mm">;
    },
  ): CircleFeature<Project>;
  polyline<const Vertices extends readonly ManagedPolylineVertex[]>(
    symbol: string,
    values: {
      readonly vertices: Vertices;
      readonly closed: boolean;
    },
  ): PolylineFeature<Project, Vertices[number]["key"]>;
}

export interface ManagedConstraintBuilder<Project> {
  coincident(
    symbol: string,
    values: {
      readonly first: PointLikeRef<Project>;
      readonly second: PointLikeRef<Project>;
      readonly suppressed?: boolean;
    },
  ): CoincidentConstraintRef<Project>;
  fixedPoint(
    symbol: string,
    values: {
      readonly point: PointLikeRef<Project>;
      readonly target: readonly [number, number];
      readonly suppressed?: boolean;
    },
  ): FixedPointConstraintRef<Project>;
  fixedCoordinate(
    symbol: string,
    values: {
      readonly point: PointLikeRef<Project>;
      readonly axis: "x" | "y";
      readonly target: number | UnitLiteral<"mm">;
      readonly suppressed?: boolean;
    },
  ): FixedCoordinateConstraintRef<Project>;
  symmetricAboutDatumAxis(
    symbol: string,
    values: {
      readonly first: PointLikeRef<Project>;
      readonly second: PointLikeRef<Project>;
      readonly axis: "x" | "y";
      readonly suppressed?: boolean;
    },
  ): SymmetricAboutDatumAxisConstraintRef<Project>;
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

export interface ManagedDimensionBuilder<Project> {
  curveLength(
    symbol: string,
    values: {
      readonly curve: NativeCurveSpanRef<Project> | LineFeature<Project>;
      readonly target: number | UnitLiteral<"mm">;
      readonly mode?: "driving" | "reference";
      readonly suppressed?: boolean;
    },
  ): CurveLengthDimensionRef<Project>;
  diameter(
    symbol: string,
    values: {
      readonly curve: CurveRef<Project> | CircleFeature<Project>;
      readonly target: number | UnitLiteral<"mm">;
      readonly mode?: "driving" | "reference";
      readonly suppressed?: boolean;
    },
  ): DiameterDimensionRef<Project>;
  radius(
    symbol: string,
    values: {
      readonly curve: CurveRef<Project> | CircleFeature<Project>;
      readonly target: number | UnitLiteral<"mm">;
      readonly mode?: "driving" | "reference";
      readonly suppressed?: boolean;
    },
  ): RadiusDimensionRef<Project>;
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
  readonly dimension: ManagedDimensionBuilder<Project>;
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
  /** Exact template output exposed at `path`; null means the path is only a namespace. */
  readonly result_output: string | null;
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
  resultOutput?: string;
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

  radius(
    curve: CurveRef<PatchBuildProject>,
    target: ScalarRef<PatchBuildProject> | UnitLiteral,
  ): RadiusDimensionRef<PatchBuildProject> {
    return this.template(
      "radius",
      "dimension.radius",
      { curve, target },
      { dimension: "dimension" },
      (root) => output<PatchBuildProject, "dimension">(root, "dimension", "dimension"),
    );
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
    let result: Result;
    this.collectionContext = templateIds;
    try {
      result = build(callbackMember as never);
    } finally {
      this.collectionContext = before;
    }
    if (templateIds.length === 0) throw new TypeError(`${rule} callback must record a template`);
    visitCollectionMemberResult(result!, new Set(templateIds), this.templates);
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
    result_output: template.resultOutput ?? null,
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
      if (template !== undefined) {
        const resultPath = path.length === 0 ? [template.familyPath] : [...path];
        if (template.path !== undefined && !sameStringPath(template.path, resultPath)) {
          throw new TypeError("one template result cannot be exported at multiple semantic paths");
        }
        template.path = resultPath;
        const binding = runtime.binding;
        if (binding?.source !== "template_output" || binding.templateId !== runtime.templateId) {
          throw new TypeError("template result is missing exact output provenance");
        }
        recordTemplateResultOutput(runtime, template);
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

function sameStringPath(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((segment, index) => segment === right[index]);
}

function visitCollectionMemberResult(
  value: unknown,
  templateIds: ReadonlySet<number>,
  templates: PendingTemplate[],
): void {
  if (isReference(value)) {
    const runtime = referenceData(value);
    if (runtime.templateId === undefined || !templateIds.has(runtime.templateId)) {
      throw new TypeError("collection callback must return its recorded template result");
    }
    const template = templates[runtime.templateId];
    if (template === undefined) throw new TypeError("collection result references an unknown template");
    recordTemplateResultOutput(runtime, template);
    return;
  }
  if (collectionData(value) !== undefined || typeof value !== "object" || value === null
      || Array.isArray(value)) {
    throw new TypeError("collection callback results must be typed template references or records");
  }
  for (const child of Object.values(value as Record<string, unknown>)) {
    visitCollectionMemberResult(child, templateIds, templates);
  }
}

function recordTemplateResultOutput(runtime: ReferenceRuntime, template: PendingTemplate): void {
  const binding = runtime.binding;
  if (binding?.source !== "template_output" || binding.templateId !== runtime.templateId) {
    throw new TypeError("template result is missing exact output provenance");
  }
  const outputNames = Object.keys(template.outputs);
  const resultOutput = binding.output === "feature"
    ? (outputNames.length === 1 ? outputNames[0] : undefined)
    : binding.output;
  if (resultOutput === undefined || template.outputs[resultOutput] === undefined) {
    throw new TypeError("a multi-output template result must select one explicit named output");
  }
  if (template.resultOutput !== undefined && template.resultOutput !== resultOutput) {
    throw new TypeError("one template cannot export multiple selected outputs");
  }
  template.resultOutput = resultOutput;
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

/** Maximum strict managed code-control request accepted by Rust/WASM. */
export const MAX_CODE_CONTROL_RPC_REQUEST_BYTES = 16 * 1024 * 1024;
/** Maximum response for a managed edit or outer Undo/Redo receipt. */
export const MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES = 16 * 1024 * 1024;
/** Maximum read-only managed-control response. */
export const MAX_CODE_CONTROL_RPC_RESPONSE_BYTES = 64 * 1024 * 1024;

const MAX_MANAGED_CONTROL_COUNT = 8_192;
const MAX_MANAGED_CONTROL_CONSUMERS = 65_536;
const MAX_MANAGED_VALUE_DEPTH = 64;
const MAX_MANAGED_VALUE_NODES = 16_384;
const MAX_CONTROL_PATH_SEGMENTS = 256;

export interface ManagedWireUnitLiteral {
  readonly unit: string;
  readonly value: number;
}

/** Exact Serde shape of Rust `ManagedValue`. */
export type ManagedValue =
  | { readonly kind: "null" }
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "number"; readonly value: number }
  | { readonly kind: "string"; readonly value: string }
  | { readonly kind: "unit"; readonly value: ManagedWireUnitLiteral }
  | { readonly kind: "array"; readonly value: readonly ManagedValue[] }
  | { readonly kind: "object"; readonly value: Readonly<Record<string, ManagedValue>> }
  | {
      readonly kind: "reference";
      readonly value: {
        readonly declaration: string;
        readonly path: readonly SemanticPathSegment[];
      };
    };

export interface ManagedControlBound {
  readonly value: number;
  readonly inclusive: boolean;
}

export type ManagedControlNumberKind = "real" | "integer" | "natural";

export type ManagedControlSchema =
  | {
      readonly kind: "number";
      readonly number: ManagedControlNumberKind;
      readonly minimum: ManagedControlBound | null;
      readonly maximum: ManagedControlBound | null;
    }
  | {
      readonly kind: "unit";
      readonly unit: string;
      readonly number: ManagedControlNumberKind;
      readonly minimum: ManagedControlBound | null;
      readonly maximum: ManagedControlBound | null;
    }
  | { readonly kind: "boolean" }
  | { readonly kind: "choice"; readonly choices: readonly string[] }
  | { readonly kind: "text" };

export type ManagedOwnedSpanKind =
  | "declaration"
  | "symbol"
  | "arguments"
  | "literal"
  | "reference"
  | "organization"
  | "outputs";

export interface ManagedControlSpan {
  readonly start: number;
  readonly end: number;
}

export interface ManagedControlSource {
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
  readonly kind: ManagedOwnedSpanKind;
  readonly span: ManagedControlSpan;
  readonly source_text: string;
}

export interface ManagedControlNavigation {
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
}

export type ManagedControlReadOnlyReason =
  | "structure"
  | "reference"
  | "structural_identity"
  | "solver_instance"
  | "null"
  | "absent"
  | "incompatible_schemas"
  | "unproven_transform";

export interface GeneratedMemberAddress {
  readonly invocation: string;
  readonly template: readonly string[];
  readonly member_key: readonly string[];
  readonly output: readonly string[];
}

export interface GeneratedMemberIdentity {
  readonly allocation: number;
  readonly generation: number;
}

export type ManagedControlConsumerTarget =
  | {
      readonly target: "declaration";
      readonly declaration: string;
      readonly family: string;
    }
  | {
      readonly target: "generated";
      readonly address: GeneratedMemberAddress;
      readonly identity: GeneratedMemberIdentity;
      readonly artifact_digest: string;
      readonly family: string;
    };

export interface ManagedControlConsumer {
  readonly target: ManagedControlConsumerTarget;
  readonly property: readonly SemanticPathSegment[];
}

export interface ManagedControlToken {
  readonly id: string;
  readonly project: string;
  readonly project_digest: string;
  readonly source_digest: string;
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
  readonly expected: ManagedValue;
  readonly generation_digest: string;
  readonly authentication: string;
}

export type ManagedControlAccess =
  | { readonly access: "editable"; readonly token: ManagedControlToken }
  | {
      readonly access: "read_only";
      readonly reason: ManagedControlReadOnlyReason;
      readonly navigation: ManagedControlNavigation | null;
    };

export interface ManagedControl {
  readonly id: string;
  readonly source: ManagedControlSource;
  readonly value: ManagedValue;
  readonly schema: ManagedControlSchema | null;
  readonly consumers: readonly ManagedControlConsumer[];
  readonly access: ManagedControlAccess;
}

export interface ManagedControlManifest {
  readonly project: string;
  readonly project_digest: string;
  readonly source_digest: string;
  readonly expansion_digest: string;
  readonly controls: readonly ManagedControl[];
}

export interface ManagedControlEdit {
  readonly token: ManagedControlToken;
  readonly value: ManagedValue;
}

export interface ManagedControlEditBatch {
  readonly edits: readonly ManagedControlEdit[];
}

export interface CodeSessionIdentity {
  readonly session: number;
  readonly revision: number;
  readonly digest: string;
}

export interface CodeSessionReceipt {
  readonly before: CodeSessionIdentity;
  readonly after: CodeSessionIdentity;
  readonly label: string;
  readonly retained_failure: boolean;
}

export interface CodeControlSnapshot {
  readonly identity: CodeSessionIdentity;
  readonly manifest: ManagedControlManifest;
  readonly can_undo: boolean;
  readonly can_redo: boolean;
}

export interface CodeControlEditReceipt {
  readonly receipt: CodeSessionReceipt;
  readonly diagnostic: string | null;
}

export interface CodeControlHistoryReceipt {
  readonly identity: CodeSessionIdentity;
  readonly moved: boolean;
  readonly receipt: CodeSessionReceipt | null;
}

export type CodeControlRpcRequest =
  | { readonly method: "inspect_managed_controls" }
  | {
      readonly method: "edit_managed_controls";
      readonly expected: CodeSessionIdentity;
      readonly batch: ManagedControlEditBatch;
    }
  | { readonly method: "undo"; readonly expected: CodeSessionIdentity }
  | { readonly method: "redo"; readonly expected: CodeSessionIdentity };

export type CodeControlFailureCode =
  | "invalid_request"
  | "request_too_large"
  | "response_too_large"
  | "code_workbench_unavailable"
  | "code_workbench_busy"
  | "control_inspection_rejected"
  | "control_edit_rejected"
  | "stale_code_session"
  | "history_rejected"
  | "editor_publication_rejected";

export interface CodeControlFailure {
  readonly code: CodeControlFailureCode;
  readonly message: string;
  readonly identity: CodeSessionIdentity | null;
}

export type CodeControlResponse<Value> =
  | { readonly outcome: "success"; readonly value: Value }
  | { readonly outcome: "failure"; readonly failure: CodeControlFailure };

export interface CodeControlManagedControlsSuccess {
  readonly result: "managed_controls";
  readonly snapshot: CodeControlSnapshot;
}

export interface CodeControlManagedEditSuccess {
  readonly result: "managed_control_edit";
  readonly receipt: CodeControlEditReceipt;
}

export interface CodeControlHistorySuccess {
  readonly result: "history";
  readonly receipt: CodeControlHistoryReceipt;
}

export type CodeControlInspectResponse =
  CodeControlResponse<CodeControlManagedControlsSuccess>;
export type CodeControlEditResponse = CodeControlResponse<CodeControlManagedEditSuccess>;
export type CodeControlHistoryResponse = CodeControlResponse<CodeControlHistorySuccess>;

export interface CodeControlRpcTransport {
  apply(canonicalRequestJson: string): string | Promise<string>;
}

export class CodeControlRpcProtocolError extends TypeError {
  constructor(message: string) {
    super(message);
    this.name = "CodeControlRpcProtocolError";
  }
}

/** Typed client for the live workbench's separate outer code authority. */
export class CodeControlClient {
  constructor(private readonly transport: CodeControlRpcTransport) {}

  async inspect(): Promise<CodeControlInspectResponse> {
    const response = await this.transport.apply(encodeCodeControlRpcRequest({
      method: "inspect_managed_controls",
    }));
    return decodeCodeControlResponse(response, "managed_controls");
  }

  async edit(
    snapshot: Pick<CodeControlSnapshot, "identity" | "manifest">,
    batch: ManagedControlEditBatch,
  ): Promise<CodeControlEditResponse> {
    authenticateManagedControlBatch(snapshot.manifest, batch);
    const response = await this.transport.apply(encodeCodeControlRpcRequest({
      method: "edit_managed_controls",
      expected: snapshot.identity,
      batch,
    }));
    const decoded = decodeCodeControlResponse(response, "managed_control_edit");
    bindCodeControlEditResponse(decoded, snapshot.identity);
    return decoded;
  }

  async undo(expected: CodeSessionIdentity): Promise<CodeControlHistoryResponse> {
    const response = await this.transport.apply(encodeCodeControlRpcRequest({
      method: "undo",
      expected,
    }));
    const decoded = decodeCodeControlResponse(response, "history");
    bindCodeControlHistoryResponse(decoded, expected, "Undo");
    return decoded;
  }

  async redo(expected: CodeSessionIdentity): Promise<CodeControlHistoryResponse> {
    const response = await this.transport.apply(encodeCodeControlRpcRequest({
      method: "redo",
      expected,
    }));
    const decoded = decodeCodeControlResponse(response, "history");
    bindCodeControlHistoryResponse(decoded, expected, "Redo");
    return decoded;
  }
}

/** Validates and deterministically encodes one closed Rust request. */
export function encodeCodeControlRpcRequest(value: CodeControlRpcRequest): string {
  const request = controlRecord(value, "code-control RPC request");
  const method = controlString(request.method, "code-control RPC method");
  let checked: unknown;
  switch (method) {
    case "inspect_managed_controls":
      controlExactKeys(request, ["method"], "managed-control inspection request");
      checked = { method };
      break;
    case "edit_managed_controls":
      controlExactKeys(
        request,
        ["method", "expected", "batch"],
        "managed-control edit request",
      );
      const expected = parseCodeSessionIdentity(request.expected, "expected code-session identity");
      const batch = parseManagedControlBatch(request.batch, "managed-control batch");
      checked = {
        method,
        expected,
        batch,
      };
      break;
    case "undo":
    case "redo":
      controlExactKeys(request, ["method", "expected"], `code-control ${method} request`);
      checked = {
        method,
        expected: parseCodeSessionIdentity(request.expected, "expected code-session identity"),
      };
      break;
    default:
      throw new CodeControlRpcProtocolError(
        `code-control RPC method has unknown value ${JSON.stringify(method)}`,
      );
  }
  const encoded = codeControlJson(checked);
  requireCodeControlSize(encoded, MAX_CODE_CONTROL_RPC_REQUEST_BYTES, "code-control RPC request");
  return encoded;
}

function decodeCodeControlResponse(
  response: string,
  expected: "managed_controls",
): CodeControlInspectResponse;
function decodeCodeControlResponse(
  response: string,
  expected: "managed_control_edit",
): CodeControlEditResponse;
function decodeCodeControlResponse(
  response: string,
  expected: "history",
): CodeControlHistoryResponse;
function decodeCodeControlResponse(
  response: string,
  expected: "managed_controls" | "managed_control_edit" | "history",
): CodeControlResponse<unknown> {
  try {
    if (typeof response !== "string") {
      throw new CodeControlRpcProtocolError("code-control RPC transport returned a non-string response");
    }
    requireCodeControlSize(
      response,
      expected === "managed_controls"
        ? MAX_CODE_CONTROL_RPC_RESPONSE_BYTES
        : MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES,
      "code-control RPC response",
    );
    let parsed: unknown;
    try {
      parsed = JSON.parse(response) as unknown;
    } catch {
      throw new CodeControlRpcProtocolError("code-control RPC response is not valid JSON");
    }
    const envelope = controlRecord(parsed, "code-control RPC response");
    const outcome = controlString(envelope.outcome, "code-control RPC outcome");
    if (outcome === "failure") {
      controlExactKeys(envelope, ["outcome", "failure"], "code-control failure response");
      return { outcome, failure: parseCodeControlFailure(envelope.failure) };
    }
    if (outcome !== "success") {
      throw new CodeControlRpcProtocolError(
        `code-control RPC outcome has unknown value ${JSON.stringify(outcome)}`,
      );
    }
    controlExactKeys(envelope, ["outcome", "value"], "code-control success response");
    const value = controlRecord(envelope.value, "code-control success value");
    if (value.result !== expected) {
      throw new CodeControlRpcProtocolError(
        `code-control RPC expected result ${expected}, received ${String(value.result)}`,
      );
    }
    switch (expected) {
      case "managed_controls":
        controlExactKeys(value, ["result", "snapshot"], "managed-controls success");
        return {
          outcome,
          value: {
            result: expected,
            snapshot: parseCodeControlSnapshot(value.snapshot),
          },
        };
      case "managed_control_edit":
        controlExactKeys(value, ["result", "receipt"], "managed-control edit success");
        return {
          outcome,
          value: {
            result: expected,
            receipt: parseCodeControlEditReceipt(value.receipt),
          },
        };
      case "history":
        controlExactKeys(value, ["result", "receipt"], "code-control history success");
        return {
          outcome,
          value: {
            result: expected,
            receipt: parseCodeControlHistoryReceipt(value.receipt),
          },
        };
    }
  } catch (error) {
    if (error instanceof CodeControlRpcProtocolError) throw error;
    const message = error instanceof Error ? error.message : String(error);
    throw new CodeControlRpcProtocolError(`invalid code-control RPC response: ${message}`);
  }
}

function parseCodeControlFailure(value: unknown): CodeControlFailure {
  const failure = controlRecord(value, "code-control failure");
  controlExactKeys(failure, ["code", "message", "identity"], "code-control failure");
  return {
    code: controlEnum(failure.code, CODE_CONTROL_FAILURE_CODES, "code-control failure code"),
    message: controlBoundedString(failure.message, 1024 * 1024, "code-control failure message"),
    identity: failure.identity === null
      ? null
      : parseCodeSessionIdentity(failure.identity, "failure code-session identity"),
  };
}

function parseCodeControlSnapshot(value: unknown): CodeControlSnapshot {
  const snapshot = controlRecord(value, "code-control snapshot");
  controlExactKeys(
    snapshot,
    ["identity", "manifest", "can_undo", "can_redo"],
    "code-control snapshot",
  );
  return {
    identity: parseCodeSessionIdentity(snapshot.identity, "code-control snapshot identity"),
    manifest: parseManagedControlManifest(snapshot.manifest, "code-control snapshot manifest"),
    can_undo: controlBoolean(snapshot.can_undo, "code-control can-undo flag"),
    can_redo: controlBoolean(snapshot.can_redo, "code-control can-redo flag"),
  };
}

function parseCodeControlEditReceipt(value: unknown): CodeControlEditReceipt {
  const result = controlRecord(value, "managed-control edit receipt");
  controlExactKeys(result, ["receipt", "diagnostic"], "managed-control edit receipt");
  const receipt = parseCodeSessionReceipt(result.receipt, "managed-control code receipt");
  const diagnostic = result.diagnostic === null
    ? null
    : controlBoundedString(result.diagnostic, 1024 * 1024, "managed-control diagnostic");
  if (receipt.retained_failure !== (diagnostic !== null)) {
    throw new CodeControlRpcProtocolError(
      "managed-control diagnostic does not match retained-failure state",
    );
  }
  return { receipt, diagnostic };
}

function parseCodeControlHistoryReceipt(value: unknown): CodeControlHistoryReceipt {
  const result = controlRecord(value, "code-control history receipt");
  controlExactKeys(result, ["identity", "moved", "receipt"], "code-control history receipt");
  const identity = parseCodeSessionIdentity(result.identity, "history code-session identity");
  const moved = controlBoolean(result.moved, "history moved flag");
  const receipt = result.receipt === null
    ? null
    : parseCodeSessionReceipt(result.receipt, "history code-session receipt");
  if (moved !== (receipt !== null)) {
    throw new CodeControlRpcProtocolError("history moved flag does not match its receipt");
  }
  if (receipt !== null && !sameCodeSessionIdentity(receipt.after, identity)) {
    throw new CodeControlRpcProtocolError("history receipt does not produce its published identity");
  }
  return { identity, moved, receipt };
}

function bindCodeControlEditResponse(
  response: CodeControlEditResponse,
  expected: CodeSessionIdentity,
): void {
  if (
    response.outcome === "success"
    && !sameCodeSessionIdentity(response.value.receipt.receipt.before, expected)
  ) {
    throw new CodeControlRpcProtocolError(
      "managed-control receipt does not start at the requested code-session identity",
    );
  }
}

function bindCodeControlHistoryResponse(
  response: CodeControlHistoryResponse,
  expected: CodeSessionIdentity,
  action: "Undo" | "Redo",
): void {
  if (response.outcome !== "success") return;
  const result = response.value.receipt;
  const before = result.receipt?.before ?? result.identity;
  if (!sameCodeSessionIdentity(before, expected)) {
    throw new CodeControlRpcProtocolError(
      `${action} receipt does not start at the requested code-session identity`,
    );
  }
}

function parseCodeSessionReceipt(value: unknown, label: string): CodeSessionReceipt {
  const receipt = controlRecord(value, label);
  controlExactKeys(receipt, ["before", "after", "label", "retained_failure"], label);
  const result = {
    before: parseCodeSessionIdentity(receipt.before, `${label} before identity`),
    after: parseCodeSessionIdentity(receipt.after, `${label} after identity`),
    label: controlBoundedString(receipt.label, 1024 * 1024, `${label} label`),
    retained_failure: controlBoolean(receipt.retained_failure, `${label} retained-failure flag`),
  };
  if (result.before.session !== result.after.session) {
    throw new CodeControlRpcProtocolError(`${label} crosses code sessions`);
  }
  return result;
}

function parseCodeSessionIdentity(value: unknown, label: string): CodeSessionIdentity {
  const identity = controlRecord(value, label);
  controlExactKeys(identity, ["session", "revision", "digest"], label);
  return {
    session: controlSafeUnsigned(identity.session, Number.MAX_SAFE_INTEGER, `${label} session`),
    revision: controlSafeUnsigned(identity.revision, Number.MAX_SAFE_INTEGER, `${label} revision`),
    digest: controlHex(identity.digest, 64, `${label} digest`),
  };
}

function parseManagedControlManifest(value: unknown, label: string): ManagedControlManifest {
  const manifest = controlRecord(value, label);
  controlExactKeys(
    manifest,
    ["project", "project_digest", "source_digest", "expansion_digest", "controls"],
    label,
  );
  const project = controlKey(manifest.project, `${label} project`);
  const projectDigest = controlHex(manifest.project_digest, 64, `${label} project digest`);
  const sourceDigest = controlHex(manifest.source_digest, 64, `${label} source digest`);
  const controls = controlArray(manifest.controls, MAX_MANAGED_CONTROL_COUNT, `${label} controls`);
  const state = { valueNodes: 0, consumers: 0 };
  const ids = new Set<string>();
  const parsedControls = controls.map((control, index) => {
    const parsed = parseManagedControl(
      control,
      `${label} control ${index}`,
      state,
      { project, projectDigest, sourceDigest },
    );
    if (ids.has(parsed.id)) {
      throw new CodeControlRpcProtocolError(`${label} contains duplicate control ${parsed.id}`);
    }
    ids.add(parsed.id);
    return parsed;
  });
  return {
    project,
    project_digest: projectDigest,
    source_digest: sourceDigest,
    expansion_digest: controlHex(manifest.expansion_digest, 64, `${label} expansion digest`),
    controls: parsedControls,
  };
}

function parseManagedControl(
  value: unknown,
  label: string,
  state: { valueNodes: number; consumers: number },
  authority: { project: string; projectDigest: string; sourceDigest: string },
): ManagedControl {
  const control = controlRecord(value, label);
  controlExactKeys(control, ["id", "source", "value", "schema", "consumers", "access"], label);
  const id = controlHex(control.id, 64, `${label} ID`);
  const source = parseManagedControlSource(control.source, `${label} source`);
  state.valueNodes = 0;
  const managedValue = parseManagedValue(control.value, `${label} value`, state, 0);
  const schema = control.schema === null
    ? null
    : parseManagedControlSchema(control.schema, `${label} schema`);
  const consumers = controlArray(
    control.consumers,
    MAX_MANAGED_CONTROL_CONSUMERS,
    `${label} consumers`,
  ).map((consumer, index) => {
    state.consumers += 1;
    if (state.consumers > MAX_MANAGED_CONTROL_CONSUMERS) {
      throw new CodeControlRpcProtocolError("managed-control manifest has too many consumers");
    }
    return parseManagedControlConsumer(consumer, `${label} consumer ${index}`);
  });
  const access = parseManagedControlAccess(control.access, `${label} access`, state);
  if (access.access === "editable") {
    if (schema === null) {
      throw new CodeControlRpcProtocolError(`${label} editable control has no schema`);
    }
    const token = access.token;
    if (
      token.id !== id
      || token.project !== authority.project
      || token.project_digest !== authority.projectDigest
      || token.source_digest !== authority.sourceDigest
      || token.declaration !== source.declaration
      || codeControlCanonicalJson(token.path) !== codeControlCanonicalJson(source.path)
      || codeControlCanonicalJson(token.expected) !== codeControlCanonicalJson(managedValue)
    ) {
      throw new CodeControlRpcProtocolError(`${label} editable token does not match its control`);
    }
  } else if (schema !== null) {
    throw new CodeControlRpcProtocolError(`${label} read-only control unexpectedly has a schema`);
  }
  return { id, source, value: managedValue, schema, consumers, access };
}

function parseManagedControlSource(value: unknown, label: string): ManagedControlSource {
  const source = controlRecord(value, label);
  controlExactKeys(source, ["declaration", "path", "kind", "span", "source_text"], label);
  const result = {
    declaration: controlKey(source.declaration, `${label} declaration`),
    path: parseControlPath(source.path, `${label} path`),
    kind: controlEnum(source.kind, MANAGED_SPAN_KINDS, `${label} kind`),
    span: parseManagedControlSpan(source.span, `${label} span`),
    source_text: controlString(source.source_text, `${label} source text`),
  };
  if (new TextEncoder().encode(result.source_text).byteLength !== result.span.end - result.span.start) {
    throw new CodeControlRpcProtocolError(`${label} text does not match its byte span`);
  }
  return result;
}

function parseManagedControlSpan(value: unknown, label: string): ManagedControlSpan {
  const span = controlRecord(value, label);
  controlExactKeys(span, ["start", "end"], label);
  const start = controlSafeUnsigned(span.start, Number.MAX_SAFE_INTEGER, `${label} start`);
  const end = controlSafeUnsigned(span.end, Number.MAX_SAFE_INTEGER, `${label} end`);
  if (end < start) throw new CodeControlRpcProtocolError(`${label} end precedes start`);
  return { start, end };
}

function parseManagedControlSchema(value: unknown, label: string): ManagedControlSchema {
  const schema = controlRecord(value, label);
  const kind = controlString(schema.kind, `${label} kind`);
  switch (kind) {
    case "number":
      controlExactKeys(schema, ["kind", "number", "minimum", "maximum"], label);
      return {
        kind,
        number: controlEnum(schema.number, CONTROL_NUMBER_KINDS, `${label} number kind`),
        minimum: parseOptionalControlBound(schema.minimum, `${label} minimum`),
        maximum: parseOptionalControlBound(schema.maximum, `${label} maximum`),
      };
    case "unit":
      controlExactKeys(schema, ["kind", "unit", "number", "minimum", "maximum"], label);
      return {
        kind,
        unit: controlKey(schema.unit, `${label} unit`),
        number: controlEnum(schema.number, CONTROL_NUMBER_KINDS, `${label} number kind`),
        minimum: parseOptionalControlBound(schema.minimum, `${label} minimum`),
        maximum: parseOptionalControlBound(schema.maximum, `${label} maximum`),
      };
    case "boolean":
    case "text":
      controlExactKeys(schema, ["kind"], label);
      return { kind };
    case "choice":
      controlExactKeys(schema, ["kind", "choices"], label);
      return {
        kind,
        choices: controlArray(schema.choices, MAX_MANAGED_CONTROL_COUNT, `${label} choices`)
          .map((choice, index) => controlString(choice, `${label} choice ${index}`)),
      };
    default:
      throw new CodeControlRpcProtocolError(`${label} has unknown kind ${JSON.stringify(kind)}`);
  }
}

function parseOptionalControlBound(value: unknown, label: string): ManagedControlBound | null {
  if (value === null) return null;
  const bound = controlRecord(value, label);
  controlExactKeys(bound, ["value", "inclusive"], label);
  return {
    value: controlFinite(bound.value, `${label} value`),
    inclusive: controlBoolean(bound.inclusive, `${label} inclusive flag`),
  };
}

function parseManagedControlAccess(
  value: unknown,
  label: string,
  state: { valueNodes: number },
): ManagedControlAccess {
  const access = controlRecord(value, label);
  const kind = controlString(access.access, `${label} kind`);
  if (kind === "editable") {
    controlExactKeys(access, ["access", "token"], label);
    return { access: kind, token: parseManagedControlToken(access.token, `${label} token`, state) };
  }
  if (kind === "read_only") {
    controlExactKeys(access, ["access", "reason", "navigation"], label);
    return {
      access: kind,
      reason: controlEnum(access.reason, CONTROL_READ_ONLY_REASONS, `${label} reason`),
      navigation: access.navigation === null
        ? null
        : parseManagedControlNavigation(access.navigation, `${label} navigation`),
    };
  }
  throw new CodeControlRpcProtocolError(`${label} has unknown access ${JSON.stringify(kind)}`);
}

function parseManagedControlNavigation(value: unknown, label: string): ManagedControlNavigation {
  const navigation = controlRecord(value, label);
  controlExactKeys(navigation, ["declaration", "path"], label);
  return {
    declaration: controlKey(navigation.declaration, `${label} declaration`),
    path: parseControlPath(navigation.path, `${label} path`),
  };
}

function parseManagedControlToken(
  value: unknown,
  label: string,
  state: { valueNodes: number },
): ManagedControlToken {
  const token = controlRecord(value, label);
  controlExactKeys(
    token,
    [
      "id",
      "project",
      "project_digest",
      "source_digest",
      "declaration",
      "path",
      "expected",
      "generation_digest",
      "authentication",
    ],
    label,
  );
  state.valueNodes = 0;
  return {
    id: controlHex(token.id, 64, `${label} ID`),
    project: controlKey(token.project, `${label} project`),
    project_digest: controlHex(token.project_digest, 64, `${label} project digest`),
    source_digest: controlHex(token.source_digest, 64, `${label} source digest`),
    declaration: controlKey(token.declaration, `${label} declaration`),
    path: parseControlPath(token.path, `${label} path`),
    expected: parseManagedValue(token.expected, `${label} expected value`, state, 0),
    generation_digest: controlHex(token.generation_digest, 64, `${label} generation digest`),
    authentication: controlHex(token.authentication, 64, `${label} authentication`),
  };
}

function parseManagedControlConsumer(value: unknown, label: string): ManagedControlConsumer {
  const consumer = controlRecord(value, label);
  controlExactKeys(consumer, ["target", "property"], label);
  return {
    target: parseManagedControlConsumerTarget(consumer.target, `${label} target`),
    property: parseControlPath(consumer.property, `${label} property`),
  };
}

function parseManagedControlConsumerTarget(
  value: unknown,
  label: string,
): ManagedControlConsumerTarget {
  const consumer = controlRecord(value, label);
  const target = controlString(consumer.target, `${label} kind`);
  if (target === "declaration") {
    controlExactKeys(consumer, ["target", "declaration", "family"], label);
    return {
      target,
      declaration: controlKey(consumer.declaration, `${label} declaration`),
      family: controlKey(consumer.family, `${label} family`),
    };
  }
  if (target === "generated") {
    controlExactKeys(
      consumer,
      ["target", "address", "identity", "artifact_digest", "family"],
      label,
    );
    return {
      target,
      address: parseGeneratedMemberAddress(consumer.address, `${label} address`),
      identity: parseGeneratedMemberIdentity(consumer.identity, `${label} identity`),
      artifact_digest: controlHex(consumer.artifact_digest, 64, `${label} artifact digest`),
      family: controlKey(consumer.family, `${label} family`),
    };
  }
  throw new CodeControlRpcProtocolError(`${label} has unknown target ${JSON.stringify(target)}`);
}

function parseGeneratedMemberAddress(value: unknown, label: string): GeneratedMemberAddress {
  const address = controlRecord(value, label);
  controlExactKeys(address, ["invocation", "template", "member_key", "output"], label);
  const strings = (candidate: unknown, field: string) =>
    controlArray(candidate, 64, `${label} ${field}`)
      .map((segment, index) => controlKey(segment, `${label} ${field} ${index}`));
  return {
    invocation: controlKey(address.invocation, `${label} invocation`),
    template: strings(address.template, "template"),
    member_key: strings(address.member_key, "member key"),
    output: strings(address.output, "output"),
  };
}

function parseGeneratedMemberIdentity(value: unknown, label: string): GeneratedMemberIdentity {
  const identity = controlRecord(value, label);
  controlExactKeys(identity, ["allocation", "generation"], label);
  return {
    allocation: controlSafeUnsigned(
      identity.allocation,
      Number.MAX_SAFE_INTEGER,
      `${label} allocation`,
    ),
    generation: controlSafeUnsigned(identity.generation, 0xffff_ffff, `${label} generation`),
  };
}

function parseManagedControlBatch(value: unknown, label: string): ManagedControlEditBatch {
  const batch = controlRecord(value, label);
  controlExactKeys(batch, ["edits"], label);
  const state = { valueNodes: 0 };
  const ids = new Set<string>();
  const edits = controlArray(batch.edits, MAX_MANAGED_CONTROL_COUNT, `${label} edits`)
    .map((candidate, index): ManagedControlEdit => {
      const edit = controlRecord(candidate, `${label} edit ${index}`);
      controlExactKeys(edit, ["token", "value"], `${label} edit ${index}`);
      const token = parseManagedControlToken(edit.token, `${label} edit ${index} token`, state);
      if (ids.has(token.id)) {
        throw new CodeControlRpcProtocolError(`${label} repeats control ${token.id}`);
      }
      ids.add(token.id);
      state.valueNodes = 0;
      const replacement = parseManagedValue(
        edit.value,
        `${label} edit ${index} value`,
        state,
        0,
      );
      return {
        token,
        value: replacement,
      };
    });
  if (edits.length === 0) {
    throw new CodeControlRpcProtocolError(`${label} must contain at least one edit`);
  }
  return { edits };
}

function authenticateManagedControlBatch(
  manifest: ManagedControlManifest,
  batch: ManagedControlEditBatch,
): void {
  const controls = new Map(manifest.controls.map((control) => [control.id, control]));
  for (const edit of batch.edits) {
    const control = controls.get(edit.token.id);
    if (
      control === undefined
      || control.access.access !== "editable"
      || codeControlCanonicalJson(control.access.token) !== codeControlCanonicalJson(edit.token)
    ) {
      throw new CodeControlRpcProtocolError(
        `managed-control batch token ${edit.token.id} is absent, read-only, or foreign`,
      );
    }
  }
}

function parseManagedValue(
  value: unknown,
  label: string,
  state: { valueNodes: number },
  depth: number,
): ManagedValue {
  state.valueNodes += 1;
  if (state.valueNodes > MAX_MANAGED_VALUE_NODES) {
    throw new CodeControlRpcProtocolError("managed values exceed the node limit");
  }
  if (depth > MAX_MANAGED_VALUE_DEPTH) {
    throw new CodeControlRpcProtocolError("managed value exceeds the depth limit");
  }
  const managed = controlRecord(value, label);
  const kind = controlString(managed.kind, `${label} kind`);
  switch (kind) {
    case "null":
      controlExactKeys(managed, ["kind"], label);
      return { kind };
    case "bool":
      controlExactKeys(managed, ["kind", "value"], label);
      return { kind, value: controlBoolean(managed.value, `${label} value`) };
    case "number":
      controlExactKeys(managed, ["kind", "value"], label);
      return { kind, value: controlFinite(managed.value, `${label} value`) };
    case "string":
      controlExactKeys(managed, ["kind", "value"], label);
      return { kind, value: controlString(managed.value, `${label} value`) };
    case "unit": {
      controlExactKeys(managed, ["kind", "value"], label);
      const unit = controlRecord(managed.value, `${label} unit`);
      controlExactKeys(unit, ["unit", "value"], `${label} unit`);
      return {
        kind,
        value: {
          unit: controlKey(unit.unit, `${label} unit name`),
          value: controlFinite(unit.value, `${label} unit value`),
        },
      };
    }
    case "array":
      controlExactKeys(managed, ["kind", "value"], label);
      return {
        kind,
        value: controlArray(managed.value, MAX_MANAGED_CONTROL_COUNT, `${label} array`)
          .map((child, index) => parseManagedValue(child, `${label} item ${index}`, state, depth + 1)),
      };
    case "object": {
      controlExactKeys(managed, ["kind", "value"], label);
      const object = controlRecord(managed.value, `${label} object`);
      const entries = Object.entries(object);
      if (entries.length > MAX_MANAGED_CONTROL_COUNT) {
        throw new CodeControlRpcProtocolError(`${label} object has too many fields`);
      }
      const result = Object.fromEntries(
        entries
          .sort(([left], [right]) => byteCompare(left, right))
          .map(([key, child]) => {
            controlString(key, `${label} object field`);
            return [
              key,
              parseManagedValue(child, `${label}.${key}`, state, depth + 1),
            ] as const;
          }),
      );
      return { kind, value: result };
    }
    case "reference": {
      controlExactKeys(managed, ["kind", "value"], label);
      const reference = controlRecord(managed.value, `${label} reference`);
      controlExactKeys(reference, ["declaration", "path"], `${label} reference`);
      return {
        kind,
        value: {
          declaration: controlKey(reference.declaration, `${label} reference declaration`),
          path: parseControlPath(reference.path, `${label} reference path`),
        },
      };
    }
    default:
      throw new CodeControlRpcProtocolError(`${label} has unknown kind ${JSON.stringify(kind)}`);
  }
}

function parseControlPath(value: unknown, label: string): readonly SemanticPathSegment[] {
  return controlArray(value, MAX_CONTROL_PATH_SEGMENTS, label).map((segment, index) => {
    if (typeof segment === "string") return controlString(segment, `${label} segment ${index}`);
    if (typeof segment === "number") {
      return controlSafeUnsigned(segment, Number.MAX_SAFE_INTEGER, `${label} index ${index}`);
    }
    const member = controlRecord(segment, `${label} member ${index}`);
    controlExactKeys(member, ["member"], `${label} member ${index}`);
    return { member: controlString(member.member, `${label} member ${index}`) };
  });
}

function codeControlJson(value: unknown): string {
  if (value === null) return "null";
  if (typeof value === "string" || typeof value === "boolean") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new CodeControlRpcProtocolError("code-control numbers must be finite");
    }
    return Object.is(value, -0) ? "-0.0" : String(value);
  }
  if (Array.isArray(value)) return `[${value.map(codeControlJson).join(",")}]`;
  if (typeof value === "object" && value !== null) {
    return `{${Object.entries(value)
      .map(([key, child]) => `${JSON.stringify(key)}:${codeControlJson(child)}`)
      .join(",")}}`;
  }
  throw new CodeControlRpcProtocolError("code-control value is not finite JSON data");
}

function codeControlCanonicalJson(value: unknown): string {
  if (value === null) return "null";
  if (typeof value === "string" || typeof value === "boolean") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new CodeControlRpcProtocolError("code-control numbers must be finite");
    }
    return Object.is(value, -0) ? "-0.0" : String(value);
  }
  if (Array.isArray(value)) return `[${value.map(codeControlCanonicalJson).join(",")}]`;
  if (typeof value === "object" && value !== null) {
    return `{${Object.entries(value)
      .sort(([left], [right]) => byteCompare(left, right))
      .map(([key, child]) => `${JSON.stringify(key)}:${codeControlCanonicalJson(child)}`)
      .join(",")}}`;
  }
  throw new CodeControlRpcProtocolError("code-control value is not finite JSON data");
}

function controlRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new CodeControlRpcProtocolError(`${label} must be an object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new CodeControlRpcProtocolError(`${label} must be a plain object`);
  }
  return value as Record<string, unknown>;
}

function controlExactKeys(value: Record<string, unknown>, expected: readonly string[], label: string): void {
  const actual = Object.keys(value).sort(byteCompare);
  const wanted = [...expected].sort(byteCompare);
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    throw new CodeControlRpcProtocolError(`${label} has unknown or missing fields`);
  }
}

function controlArray(value: unknown, maximum: number, label: string): readonly unknown[] {
  if (!Array.isArray(value)) throw new CodeControlRpcProtocolError(`${label} must be an array`);
  if (value.length > maximum) {
    throw new CodeControlRpcProtocolError(`${label} exceeds ${maximum} entries`);
  }
  return value;
}

function controlString(value: unknown, label: string): string {
  if (typeof value !== "string" || controlHasUnpairedSurrogate(value)) {
    throw new CodeControlRpcProtocolError(`${label} must be a valid Unicode string`);
  }
  return value;
}

function controlBoundedString(value: unknown, maximumBytes: number, label: string): string {
  const result = controlString(value, label);
  if (new TextEncoder().encode(result).byteLength > maximumBytes) {
    throw new CodeControlRpcProtocolError(`${label} exceeds ${maximumBytes} bytes`);
  }
  return result;
}

function controlKey(value: unknown, label: string): string {
  const result = controlString(value, label);
  if (
    result.length === 0
    || new TextEncoder().encode(result).byteLength > 256
    || /\p{Cc}/u.test(result)
  ) {
    throw new CodeControlRpcProtocolError(`${label} is not a valid bounded key`);
  }
  return result;
}

function controlHex(value: unknown, width: number, label: string): string {
  const result = controlString(value, label);
  if (result.length !== width || !/^[0-9a-f]+$/u.test(result)) {
    throw new CodeControlRpcProtocolError(
      `${label} must be exactly ${width} lowercase hexadecimal characters`,
    );
  }
  return result;
}

function controlFinite(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new CodeControlRpcProtocolError(`${label} must be a finite number`);
  }
  return value;
}

function controlSafeUnsigned(value: unknown, maximum: number, label: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > maximum) {
    throw new CodeControlRpcProtocolError(`${label} must be an exactly represented unsigned integer`);
  }
  return value as number;
}

function controlBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") throw new CodeControlRpcProtocolError(`${label} must be a boolean`);
  return value;
}

function controlEnum<const Values extends readonly string[]>(
  value: unknown,
  allowed: Values,
  label: string,
): Values[number] {
  const result = controlString(value, label);
  if (!(allowed as readonly string[]).includes(result)) {
    throw new CodeControlRpcProtocolError(`${label} has unknown value ${JSON.stringify(result)}`);
  }
  return result as Values[number];
}

function requireCodeControlSize(value: string, maximum: number, label: string): void {
  if (value.length > maximum || new TextEncoder().encode(value).byteLength > maximum) {
    throw new CodeControlRpcProtocolError(`${label} exceeds ${maximum} bytes`);
  }
}

function sameCodeSessionIdentity(left: CodeSessionIdentity, right: CodeSessionIdentity): boolean {
  return left.session === right.session
    && left.revision === right.revision
    && left.digest === right.digest;
}

function controlHasUnpairedSurrogate(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (next < 0xdc00 || next > 0xdfff) return true;
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      return true;
    }
  }
  return false;
}

const CONTROL_NUMBER_KINDS = [
  "real",
  "integer",
  "natural",
] as const satisfies readonly ManagedControlNumberKind[];
const MANAGED_SPAN_KINDS = [
  "declaration",
  "symbol",
  "arguments",
  "literal",
  "reference",
  "organization",
  "outputs",
] as const satisfies readonly ManagedOwnedSpanKind[];
const CONTROL_READ_ONLY_REASONS = [
  "structure",
  "reference",
  "structural_identity",
  "solver_instance",
  "null",
  "absent",
  "incompatible_schemas",
  "unproven_transform",
] as const satisfies readonly ManagedControlReadOnlyReason[];
const CODE_CONTROL_FAILURE_CODES = [
  "invalid_request",
  "request_too_large",
  "response_too_large",
  "code_workbench_unavailable",
  "code_workbench_busy",
  "control_inspection_rejected",
  "control_edit_rejected",
  "stale_code_session",
  "history_rejected",
  "editor_publication_rejected",
] as const satisfies readonly CodeControlFailureCode[];
