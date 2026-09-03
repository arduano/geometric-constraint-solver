// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Explicit caller-owned Node compiler for trusted custom patch modules.
 *
 * This executes only the already-imported `PatchDefinition` supplied by its
 * caller and emits canonical data. Rust remains the sole owner of
 * materialization, solving, and independent validation.
 */

import { createHash } from "node:crypto";

import { canonicalStringify } from "@geosolve/intent";

import type {
  FeatureKind,
  InputSchemas,
  KeyedFeatureCollection,
  PatchDefinition,
} from "./authoring.js";
import {
  authoringPatchRuntime,
  authoringSchemaRuntime,
} from "./authoring-private.js";
import type { PrivateSchemaRuntime } from "./authoring-private.js";
import {
  AUTHORING_METHOD_CATALOG,
  DECLARATION_RESULT_CATALOG,
} from "./generated-declaration-results.js";

export const SKETCH_CODE_SDK_ABI = "geosolve-sketch-code-v2" as const;
export const PATCH_ARTIFACT_FORMAT = "geosolve-patch-artifact-v2" as const;
export const PATCH_ARTIFACT_LIMIT = 16 * 1024 * 1024;

export type ArtifactPathSegment = string | number | { readonly member: string };

export type ArtifactTemplateBinding =
  | {
    readonly source: "input";
    readonly name: string;
    readonly path: readonly ArtifactPathSegment[];
    readonly expected_kind: FeatureKind;
  }
  | {
    readonly source: "template_output";
    readonly template: readonly string[];
    readonly path: readonly ArtifactPathSegment[];
    readonly expected_kind: FeatureKind;
  }
  | {
    readonly source: "collection_member";
    readonly input: string;
    readonly path: readonly ArtifactPathSegment[];
    readonly expected_kind: FeatureKind;
  };

export type ManagedArtifactValue =
  | { readonly kind: "null" }
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "number"; readonly value: number }
  | { readonly kind: "string"; readonly value: string }
  | {
    readonly kind: "unit";
    readonly value: { readonly unit: string; readonly value: number };
  }
  | { readonly kind: "array"; readonly value: readonly ManagedArtifactValue[] }
  | {
    readonly kind: "object";
    readonly value: Readonly<Record<string, ManagedArtifactValue>>;
  };

/** Lossless named declaration arguments with dependency bindings at any leaf. */
export type ArtifactTemplateArgument =
  | { readonly argument: "literal"; readonly value: ManagedArtifactValue }
  | { readonly argument: "binding"; readonly value: ArtifactTemplateBinding }
  | {
    readonly argument: "array";
    readonly value: readonly ArtifactTemplateArgument[];
  }
  | {
    readonly argument: "object";
    readonly value: Readonly<Record<string, ArtifactTemplateArgument>>;
  };

export interface ArtifactTemplateNode {
  /** Mandatory patch-local identity, independent of the returned object path. */
  readonly path: readonly string[];
  /** Ordinary callback return path; null keeps the declaration private. */
  readonly result_path: readonly string[] | null;
  /** Selected semantic result path; null denotes the whole typed result. */
  readonly result_output: readonly ArtifactPathSegment[] | null;
  readonly declaration_family: string;
  readonly arguments: ArtifactTemplateArgument;
  readonly outputs: readonly ArtifactTemplateOutput[];
}

export interface ArtifactTemplateOutput {
  readonly path: readonly ArtifactPathSegment[];
  readonly kind: FeatureKind;
}

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
}

export interface PatchModuleArtifact {
  readonly format: typeof PATCH_ARTIFACT_FORMAT;
  readonly sdk_abi: typeof SKETCH_CODE_SDK_ABI;
  readonly module_specifier: string;
  readonly export_name: string;
  readonly source_digest: string;
  readonly interface_digest: string;
  readonly inputs: Readonly<Record<string, FeatureKind>>;
  readonly outputs: Readonly<Record<string, FeatureKind>>;
  readonly templates: readonly ArtifactTemplateNode[];
  readonly collections: readonly ArtifactCollectionRule[];
}

export interface CompilePatchArtifactOptions<Schemas extends InputSchemas, Result> {
  readonly source: string | Uint8Array;
  readonly moduleSpecifier: string;
  readonly exportName: string;
  readonly patch: PatchDefinition<Schemas, Result>;
}

export interface CompiledPatchArtifact {
  readonly artifact: PatchModuleArtifact;
  readonly canonicalJson: string;
  readonly artifactDigest: string;
}

export function compilePatchArtifact<Schemas extends InputSchemas, Result>(
  options: CompilePatchArtifactOptions<Schemas, Result>,
): CompiledPatchArtifact {
  requireModuleSpecifier(options.moduleSpecifier);
  requireKey(options.exportName, "export name");
  const plan = recordPatchArtifact(options.patch);
  const sourceBytes = typeof options.source === "string"
    ? new TextEncoder().encode(options.source)
    : options.source;
  const interfaceJson = canonicalStringify({
    export_name: options.exportName,
    inputs: plan.inputs,
    module_specifier: options.moduleSpecifier,
    outputs: plan.outputs,
    sdk_abi: SKETCH_CODE_SDK_ABI,
  });
  const artifact: PatchModuleArtifact = {
    format: PATCH_ARTIFACT_FORMAT,
    sdk_abi: SKETCH_CODE_SDK_ABI,
    module_specifier: options.moduleSpecifier,
    export_name: options.exportName,
    source_digest: sha256(sourceBytes),
    interface_digest: sha256(new TextEncoder().encode(interfaceJson)),
    inputs: plan.inputs,
    outputs: plan.outputs,
    templates: plan.templates,
    collections: plan.collections,
  };
  const canonicalJson = canonicalArtifactJson(artifact);
  const size = new TextEncoder().encode(canonicalJson).byteLength;
  if (size > PATCH_ARTIFACT_LIMIT) {
    throw new RangeError(`patch artifact is ${size} bytes; the limit is ${PATCH_ARTIFACT_LIMIT}`);
  }
  return Object.freeze({
    artifact: deepFreeze(artifact),
    canonicalJson,
    artifactDigest: sha256(new TextEncoder().encode(canonicalJson)),
  });
}

/** @internal Build-time structural execution; never exported from the package root. */
export function recordPatchArtifact<Schemas extends InputSchemas, Result>(
  definition: PatchDefinition<Schemas, Result>,
): PatchArtifactPlan {
  const runtime = authoringPatchRuntime(definition as object);
  const recorder = new StructuralPatchRecorder();
  const inputs = Object.fromEntries(
    sortedEntries(runtime.inputs).map(([name, schema]) => [
      name,
      symbolicInput(name, authoringSchemaRuntime(schema)),
    ]),
  );
  const result = runtime.build(recorder.builder, inputs);
  const outputs: Record<string, FeatureKind> = {};
  visitResult(result, [], outputs, recorder.templates, recorder.rules);
  if (Object.keys(outputs).length === 0) {
    throw new TypeError("patch must return at least one typed output");
  }

  const templates = recorder.templates.map((template): ArtifactTemplateNode => ({
    path: [template.localId],
    result_path: template.path ?? null,
    result_output: template.resultOutput ?? null,
    declaration_family: template.declarationFamily,
    arguments: artifactArgument(template.arguments, recorder.templates),
    outputs: template.outputs,
  }));
  const collections = recorder.rules.map((rule): ArtifactCollectionRule => {
    const templates = rule.templateIds.map((id) => {
      const template = recorder.templates[id];
      if (template === undefined) throw new TypeError("collection references an unknown declaration");
      return [template.localId];
    });
    const base = {
      path: rule.path ?? [`collection_${rule.id}`],
      input: rule.input,
      templates,
    };
    return rule.rule === "each"
      ? { rule: "each", ...base, member_key_field: "key" }
      : { rule: "map_record", ...base };
  });
  return {
    inputs: sortedObject(Object.fromEntries(
      sortedEntries(runtime.inputs).map(([name, schema]) => [
        name,
        schemaFeatureKind(authoringSchemaRuntime(schema)),
      ]),
    )),
    outputs: sortedObject(outputs),
    templates,
    collections,
  };
}

interface RuntimeReference {
  readonly kind: FeatureKind;
  readonly binding: RuntimeBinding;
  readonly templateId?: number;
  readonly path: readonly ArtifactPathSegment[];
}

type RuntimeBinding =
  | {
    readonly source: "input";
    readonly name: string;
    readonly path: readonly ArtifactPathSegment[];
    readonly expected_kind: FeatureKind;
  }
  | {
    readonly source: "collection_member";
    readonly input: string;
    readonly path: readonly ArtifactPathSegment[];
    readonly expected_kind: FeatureKind;
  }
  | {
    readonly source: "template_output";
    readonly templateId: number;
    readonly path: readonly ArtifactPathSegment[];
    readonly expected_kind: FeatureKind;
  };

interface SymbolicCollection {
  readonly inputName: string;
  readonly ruleId?: number;
  readonly memberFactory: () => unknown;
}

const referenceRuntime = Symbol("geosolve.patch-compiler.reference");
const collectionRuntime = Symbol("geosolve.patch-compiler.collection");

interface PendingTemplate {
  readonly id: number;
  readonly localId: string;
  path?: string[];
  resultSelectionRecorded?: boolean;
  resultOutput?: ArtifactPathSegment[];
  readonly declarationFamily: string;
  readonly arguments: PendingTemplateArgument;
  readonly outputs: readonly ArtifactTemplateOutput[];
}

type PendingTemplateArgument =
  | { readonly argument: "literal"; readonly value: ManagedArtifactValue }
  | { readonly argument: "binding"; readonly value: RuntimeBinding }
  | {
    readonly argument: "array";
    readonly value: readonly PendingTemplateArgument[];
  }
  | {
    readonly argument: "object";
    readonly value: Readonly<Record<string, PendingTemplateArgument>>;
  };

interface PendingRule {
  readonly id: number;
  readonly rule: "each" | "map_record";
  path?: string[];
  readonly input: string;
  readonly templateIds: number[];
}

class StructuralPatchRecorder {
  readonly templates: PendingTemplate[] = [];
  readonly rules: PendingRule[] = [];
  private readonly usedIds = new Set<string>();
  private collectionContext: number[] | undefined;

  readonly builder = Object.freeze({
    geometry: this.namespace("geometry", "feature"),
    constraint: this.namespace("constraint", "constraint"),
    dimension: this.namespace("dimension", "dimension"),
    operation: this.namespace("operation", "operation"),
    aggregate: this.namespace("aggregate", "feature"),
    computed: this.namespace("computed", "feature"),
    each: <Key extends PropertyKey, Value, Result>(
      collection: KeyedFeatureCollection<Key, Value>,
      build: (value: Value & { readonly key: Key }) => Result,
    ): unknown => this.dynamicCollection("each", collection, build as (value: never) => Result),
    mapRecord: <RecordType extends Readonly<Record<PropertyKey, unknown>>, Result>(
      record: RecordType,
      build: (value: RecordType[keyof RecordType]) => Result,
    ): unknown => this.dynamicCollection("map_record", record, build as (value: never) => Result),
  });

  private namespace(namespace: string, rootKind: FeatureKind): object {
    return new Proxy(Object.create(null) as object, {
      get: (_target, property) => {
        if (typeof property !== "string") return undefined;
        return (localId: string, values: Readonly<Record<string, unknown>>) =>
          this.template(namespace, property, rootKind, localId, values);
      },
    });
  }

  private template(
    namespace: string,
    method: string,
    rootKind: FeatureKind,
    localId: string,
    values: Readonly<Record<string, unknown>>,
  ): unknown {
    requirePatchAuthoringFamily(namespace, method);
    requireKey(localId, "patch-local declaration ID");
    if (this.usedIds.has(localId)) {
      throw new TypeError(`duplicate patch-local declaration ID ${JSON.stringify(localId)}`);
    }
    this.usedIds.add(localId);
    const id = this.templates.length;
    const family = `${namespace}.${method}`;
    const exactRootKind = declarationRootKind(family, rootKind);
    const outputs = resultOutputs(namespace, method, exactRootKind, values);
    const argumentsValue = normalizeTemplateArguments(namespace, method, values);
    this.templates.push({
      id,
      localId,
      declarationFamily: family,
      arguments: pendingArgument(argumentsValue),
      outputs,
    });
    this.collectionContext?.push(id);
    return shapedTemplateReference(id, family, exactRootKind, outputs, values);
  }

  private dynamicCollection<Result>(
    rule: "each" | "map_record",
    value: unknown,
    build: (value: never) => Result,
  ): unknown {
    const source = collectionData(value);
    if (source?.inputName === undefined) {
      throw new TypeError(`${rule} requires a symbolic collection input`);
    }
    const id = this.rules.length;
    const templateIds: number[] = [];
    const member = source.memberFactory();
    const callbackMember = rule === "each" ? withSymbolicMemberKey(member) : member;
    const before = this.collectionContext;
    let result: Result;
    this.collectionContext = templateIds;
    try {
      result = build(callbackMember as never);
    } finally {
      this.collectionContext = before;
    }
    if (templateIds.length === 0) throw new TypeError(`${rule} callback must record a declaration`);
    visitCollectionMemberResult(result!, [], new Set(templateIds), this.templates);
    this.rules.push({ id, rule, input: source.inputName, templateIds });
    return symbolicCollection(source.inputName, source.memberFactory, id);
  }
}

const HOST_ONLY_PATCH_FAMILIES = new Set([
  "computed.fillet",
  "computed.roundedRectangleProfile",
]);

function requirePatchAuthoringFamily(namespace: string, method: string): void {
  const family = `${namespace}.${method}`;
  if (HOST_ONLY_PATCH_FAMILIES.has(family)) return;
  const availability = (
    AUTHORING_METHOD_CATALOG as Readonly<Record<string, string>>
  )[family];
  if (availability === "requires_host_snapshot") {
    throw new TypeError(
      `patch authoring method ${family} requires immutable host-snapshot authority`,
    );
  }
  if (availability !== "public") {
    throw new TypeError(`unsupported patch authoring method ${family}`);
  }
}

function normalizeTemplateArguments(
  namespace: string,
  method: string,
  values: Readonly<Record<string, unknown>>,
): Readonly<Record<string, unknown>> {
  const normalized: Record<string, unknown> = { ...values };
  for (const field of curveArgumentFields(namespace, method)) {
    if (field in normalized) normalized[field] = unwrapFeatureOutput(normalized[field], "curve");
  }
  for (const field of spanArgumentFields(namespace, method)) {
    if (field in normalized) normalized[field] = unwrapFeatureOutput(normalized[field], "span");
  }

  if (namespace === "geometry" && method === "tangentArc") {
    normalized.source = normalizeSpanContainer(normalized.source);
  }
  if (namespace === "operation" && method === "associativeFillet") {
    normalized.parents = normalizeFilletParents(normalized.parents);
  }
  if (namespace === "operation" && method === "linearPattern") {
    normalized.sources = normalizeReferenceArray(normalized.sources, "curve");
  }
  if (namespace === "aggregate") {
    normalized.spans = normalizeReferenceArray(normalized.spans, "span");
  }
  if (namespace === "computed" && method === "filletSet") {
    normalized.corners = normalizeFilletCorners(normalized.corners);
  }
  return normalized;
}

function curveArgumentFields(namespace: string, method: string): readonly string[] {
  if (namespace === "constraint") {
    if (["concentric", "equalRadius", "circleCircleTangency"].includes(method)) {
      return ["first", "second"];
    }
    if (method === "lineCircleTangency") return ["circle"];
    if (method === "circleArcTangency") return ["circle", "arc"];
    if (method === "lineLineFillet" || method === "curveCurveFillet") return ["fillet"];
  }
  if (namespace === "dimension" && (method === "radius" || method === "diameter")) {
    return ["curve"];
  }
  if (namespace === "operation" && method === "mirror") return ["source"];
  return [];
}

function spanArgumentFields(namespace: string, method: string): readonly string[] {
  if (namespace === "constraint") {
    if (["horizontal", "vertical", "collinearWithDatumAxis"].includes(method)) return ["span"];
    if (["horizontalPointToMidpoint", "verticalPointToMidpoint", "midpoint"].includes(method)) {
      return ["line"];
    }
    if (method === "pointOnCurve") return ["curve"];
    if (["parallel", "perpendicular", "collinear", "equalLength", "curveCurveContact",
      "curveCurveTangency", "curveDirection", "equalCurvature", "endpointContinuity"].includes(method)) {
      return ["first", "second"];
    }
    if (method === "symmetricAboutLine") return ["axis"];
    if (method === "lineCircleTangency") return ["line"];
    if (method === "lineCurveTangency") return ["line", "curve"];
    if (method === "lineLineFillet" || method === "curveCurveFillet") return ["first", "second"];
  }
  if (namespace === "dimension") {
    if (method === "curveLength") return ["curve"];
    if (["orientedAngle", "supportingLineOffset", "exactTranslatedSegmentOffset"].includes(method)) {
      return ["first", "second"];
    }
    if (method === "profileOffset") return ["target"];
  }
  if (namespace === "operation") {
    if (["split", "break", "trim"].includes(method)) return ["source"];
    if (method === "extend") return ["source", "target"];
    if (method === "mirror") return ["axis"];
    if (method === "chamfer") return ["first", "second"];
  }
  return [];
}

function unwrapFeatureOutput(value: unknown, output: "curve" | "span"): unknown {
  if (referenceData(value) === undefined || typeof value !== "object" || value === null) return value;
  const selected = (value as Readonly<Record<string, unknown>>)[output];
  return referenceData(selected) === undefined ? value : selected;
}

function normalizeSpanContainer(value: unknown): unknown {
  if (!isPlainRecord(value)) return value;
  return { ...value, span: unwrapFeatureOutput(value.span, "span") };
}

function normalizeFilletParents(value: unknown): unknown {
  return Array.isArray(value) ? value.map(normalizeSpanContainer) : value;
}

function normalizeFilletCorners(value: unknown): unknown {
  if (!Array.isArray(value)) return value;
  return value.map((corner) => isPlainRecord(corner)
    ? { ...corner, parents: normalizeFilletParents(corner.parents) }
    : corner);
}

function normalizeReferenceArray(value: unknown, output: "curve" | "span"): unknown {
  return Array.isArray(value) ? value.map((member) => unwrapFeatureOutput(member, output)) : value;
}

function isPlainRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    && referenceData(value) === undefined && !isUnit(value);
}

type CatalogResultShape =
  | { readonly shape: "leaf"; readonly kind: FeatureKind }
  | { readonly shape: "native_span" }
  | {
    readonly shape: "object";
    readonly fields: Readonly<Record<string, CatalogResultShape>>;
  }
  | { readonly shape: "tuple"; readonly items: readonly CatalogResultShape[] }
  | {
    readonly shape: "dynamic_keyed";
    readonly source: string;
    readonly member: CatalogResultShape;
  };

function resultOutputs(
  namespace: string,
  method: string,
  _rootKind: FeatureKind,
  values: Readonly<Record<string, unknown>>,
): readonly ArtifactTemplateOutput[] {
  const family = `${namespace}.${method}`;
  const shape = declarationResultShape(family);
  if (containsUnresolvedPlanShape(shape)) {
    throw new TypeError(
      `declaration family ${JSON.stringify(family)} has plan-dependent outputs that cannot be recorded structurally`,
    );
  }
  const outputs: ArtifactTemplateOutput[] = [];
  collectResultOutputs(shape, [], values, outputs, family);
  if (outputs.length === 0) {
    throw new TypeError(
      `declaration family ${JSON.stringify(family)} has plan-dependent outputs that cannot be recorded structurally`,
    );
  }
  return Object.freeze(outputs.sort((left, right) => compareArtifactPaths(left.path, right.path)));
}

function declarationResultShape(family: string): CatalogResultShape {
  if (family === "computed.fillet") {
    return { shape: "object", fields: { arc: { shape: "native_span" } } };
  }
  if (family === "computed.roundedRectangleProfile") {
    return {
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
    };
  }
  const entry = (DECLARATION_RESULT_CATALOG as unknown as Readonly<Record<string, {
    readonly outputs: CatalogResultShape;
  }>>)[family];
  if (entry === undefined) throw new TypeError(`unsupported declaration family ${JSON.stringify(family)}`);
  return entry.outputs;
}

function declarationRootKind(family: string, fallback: FeatureKind): FeatureKind {
  if (family === "computed.fillet" || family === "computed.roundedRectangleProfile") {
    return "feature";
  }
  const entry = (DECLARATION_RESULT_CATALOG as unknown as Readonly<Record<string, {
    readonly feature_kind: FeatureKind;
  }>>)[family];
  return entry?.feature_kind ?? fallback;
}

function containsUnresolvedPlanShape(shape: CatalogResultShape): boolean {
  switch (shape.shape) {
    case "leaf":
    case "native_span": return false;
    case "object": {
      const children = Object.values(shape.fields);
      return children.length === 0 || children.some(containsUnresolvedPlanShape);
    }
    case "tuple": return shape.items.length === 0 || shape.items.some(containsUnresolvedPlanShape);
    case "dynamic_keyed": return containsUnresolvedPlanShape(shape.member);
  }
}

function collectResultOutputs(
  shape: CatalogResultShape,
  path: readonly ArtifactPathSegment[],
  values: Readonly<Record<string, unknown>>,
  outputs: ArtifactTemplateOutput[],
  family: string,
): void {
  switch (shape.shape) {
    case "leaf":
      outputs.push(Object.freeze({ path: Object.freeze([...path]), kind: shape.kind }));
      return;
    case "native_span":
      outputs.push(Object.freeze({ path: Object.freeze([...path]), kind: "curve_span" }));
      return;
    case "object":
      for (const [name, child] of sortedEntries(shape.fields)) {
        collectResultOutputs(child, [...path, name], values, outputs, family);
      }
      return;
    case "tuple":
      for (const [index, child] of shape.items.entries()) {
        collectResultOutputs(child, [...path, index], values, outputs, family);
      }
      return;
    case "dynamic_keyed":
      outputs.push(Object.freeze({ path: Object.freeze([...path]), kind: "collection" }));
      for (const key of dynamicResultKeys(shape.source, values, family)) {
        collectResultOutputs(shape.member, [...path, { member: key }], values, outputs, family);
      }
  }
}

function dynamicResultKeys(
  source: string,
  values: Readonly<Record<string, unknown>>,
  family: string,
): readonly string[] {
  const keyed = (name: string): readonly string[] => {
    const entries = values[name];
    if (!Array.isArray(entries)) {
      throw new TypeError(`${family} requires a concrete ${name} array to authenticate result paths`);
    }
    return entries.map((entry, index) => {
      if (!isPlainRecord(entry) || typeof entry.key !== "string") {
        throw new TypeError(`${family} ${name}[${index}] requires a string key`);
      }
      requireKey(entry.key, `${name} key`);
      return entry.key;
    });
  };
  switch (source) {
    case "polyline_vertices": return keyed("vertices");
    case "polyline_segments": {
      const keys = keyed("vertices");
      return values.closed === true ? keys : keys.slice(0, -1);
    }
    case "polyline_corners": {
      const keys = keyed("vertices");
      return values.closed === true ? keys : keys.slice(1, -1);
    }
    case "spline_controls": return keyed("controls");
    case "spline_spans": {
      const keys = keyed("controls");
      const degree = values.degree;
      if (!Number.isSafeInteger(degree) || (degree as number) < 1) {
        throw new TypeError(`${family} requires a positive integer degree`);
      }
      const count = family === "geometry.periodicControlNurbs"
        ? keys.length
        : Math.max(0, keys.length - (degree as number));
      return keys.slice(0, count);
    }
    case "fillet_corners": return keyed("corners");
    default:
      throw new TypeError(`${family} has unsupported plan-dependent result source ${JSON.stringify(source)}`);
  }
}

function compareArtifactPaths(
  left: readonly ArtifactPathSegment[],
  right: readonly ArtifactPathSegment[],
): number {
  const shared = Math.min(left.length, right.length);
  for (let index = 0; index < shared; index += 1) {
    const order = compareArtifactPathSegment(left[index]!, right[index]!);
    if (order !== 0) return order;
  }
  return left.length - right.length;
}

function compareArtifactPathSegment(left: ArtifactPathSegment, right: ArtifactPathSegment): number {
  const rank = (value: ArtifactPathSegment): number =>
    typeof value === "string" ? 0 : typeof value === "number" ? 1 : 2;
  const rankOrder = rank(left) - rank(right);
  if (rankOrder !== 0) return rankOrder;
  if (typeof left === "string" && typeof right === "string") return compareText(left, right);
  if (typeof left === "number" && typeof right === "number") return left - right;
  if (typeof left === "object" && typeof right === "object") {
    return compareText(left.member, right.member);
  }
  return 0;
}

function artifactPathKey(path: readonly ArtifactPathSegment[]): string {
  return JSON.stringify(path);
}

function shapedTemplateReference(
  templateId: number,
  family: string,
  rootKind: FeatureKind,
  _outputs: readonly ArtifactTemplateOutput[],
  values: Readonly<Record<string, unknown>>,
): object {
  const root = makeReference({
    kind: rootKind,
    path: [],
    templateId,
    binding: {
      source: "template_output", templateId, path: [], expected_kind: rootKind,
    },
  });
  const shape = declarationResultShape(family);
  if (shape.shape !== "object") {
    throw new TypeError(`declaration family ${JSON.stringify(family)} must return a named object shape`);
  }
  for (const [name, child] of sortedEntries(shape.fields)) {
    root[name] = materializeTemplateResultShape(
      templateId,
      family,
      child,
      [name],
      values,
    );
  }
  deepFreeze(root);
  return root;
}

function materializeTemplateResultShape(
  templateId: number,
  family: string,
  shape: CatalogResultShape,
  path: readonly ArtifactPathSegment[],
  values: Readonly<Record<string, unknown>>,
): unknown {
  switch (shape.shape) {
    case "leaf": return templateOutputReference(templateId, path, shape.kind);
    case "native_span": return templateOutputReference(templateId, path, "curve_span");
    case "object": {
      const object = Object.create(null) as Record<string, unknown>;
      for (const [name, child] of sortedEntries(shape.fields)) {
        object[name] = materializeTemplateResultShape(
          templateId,
          family,
          child,
          [...path, name],
          values,
        );
      }
      return Object.freeze(object);
    }
    case "tuple": return Object.freeze(shape.items.map((child, index) =>
      materializeTemplateResultShape(templateId, family, child, [...path, index], values)));
    case "dynamic_keyed": {
      const keys = dynamicResultKeys(shape.source, values, family);
      const collection = makeReference({
        kind: "collection",
        path,
        templateId,
        binding: { source: "template_output", templateId, path, expected_kind: "collection" },
      });
      const byKey = Object.create(null) as Record<string, unknown>;
      for (const key of keys) {
        byKey[key] = materializeTemplateResultShape(
          templateId,
          family,
          shape.member,
          [...path, { member: key }],
          values,
        );
      }
      collection.keys = Object.freeze([...keys]);
      collection.byKey = Object.freeze(byKey);
      return deepFreeze(collection);
    }
  }
}

function templateOutputReference(
  templateId: number,
  path: readonly ArtifactPathSegment[],
  kind: FeatureKind,
): object {
  return makeReference({
    kind,
    path,
    templateId,
    binding: { source: "template_output", templateId, path, expected_kind: kind },
  });
}

function makeReference(runtime: RuntimeReference): Record<PropertyKey, unknown> {
  const value = Object.create(null) as Record<PropertyKey, unknown>;
  Object.defineProperty(value, referenceRuntime, { value: runtime });
  return value;
}

function referenceData(value: unknown): RuntimeReference | undefined {
  return typeof value === "object" && value !== null && referenceRuntime in value
    ? (value as { readonly [referenceRuntime]: RuntimeReference })[referenceRuntime]
    : undefined;
}

function symbolicInput(name: string, schema: PrivateSchemaRuntime): unknown {
  if (schema.kind === "keyed" || schema.kind === "record") {
    const element = schema.element === undefined
      ? { kind: "feature" } as PrivateSchemaRuntime
      : authoringSchemaRuntime(schema.element);
    return symbolicCollection(name, () => symbolicCollectionMember(name, element));
  }
  return symbolicReferenceTree(name, schema, {
    source: "input", name, path: [], expected_kind: schemaFeatureKind(schema),
  });
}

function symbolicCollectionMember(input: string, schema: PrivateSchemaRuntime): unknown {
  return symbolicReferenceTree(input, schema, {
    source: "collection_member", input, path: [], expected_kind: schemaFeatureKind(schema),
  });
}

function symbolicReferenceTree(
  name: string,
  schema: PrivateSchemaRuntime,
  binding: RuntimeBinding,
): Record<PropertyKey, unknown> {
  const root = makeReference({ kind: schemaFeatureKind(schema), path: [], binding });
  if (schema.kind !== "feature") return root;
  const family = featureSchemaFamily(schema.feature ?? "feature");
  const shape = declarationResultShape(family);
  if (shape.shape !== "object") {
    throw new TypeError(`feature schema ${JSON.stringify(schema.feature)} has no named result shape`);
  }
  for (const [field, child] of sortedEntries(shape.fields)) {
    root[field] = symbolicInputResultShape(child, extendBinding(binding, [field], "feature"));
  }
  return root;
}

function featureSchemaFamily(feature: string): string {
  const family = {
    sketchPoint: "geometry.sketchPoint",
    segment: "geometry.segment",
    polyline: "geometry.polyline",
    rectangle: "geometry.twoPointAlignedRectangle",
    circle: "geometry.centerRadiusCircle",
    arc: "geometry.centerArc",
    ellipse: "geometry.centerAxesEllipse",
    ellipticalArc: "geometry.centerAxesEllipticalArc",
    quadraticBezier: "geometry.quadraticBezier",
    cubicBezier: "geometry.cubicBezier",
    conic: "geometry.rationalQuadraticConic",
    parabola: "geometry.parabola",
    hyperbola: "geometry.hyperbola",
    nurbs: "geometry.openControlNurbs",
    fillet: "computed.fillet",
    filletSet: "computed.filletSet",
  } as const satisfies Readonly<Record<string, string>>;
  const resolved = (family as Readonly<Record<string, string>>)[feature];
  if (resolved === undefined) throw new TypeError(`unsupported feature schema ${JSON.stringify(feature)}`);
  return resolved;
}

function symbolicInputResultShape(shape: CatalogResultShape, binding: RuntimeBinding): unknown {
  switch (shape.shape) {
    case "leaf": return makeReference({
      kind: shape.kind,
      path: binding.path,
      binding: { ...binding, expected_kind: shape.kind },
    });
    case "native_span": return makeReference({
      kind: "curve_span",
      path: binding.path,
      binding: { ...binding, expected_kind: "curve_span" },
    });
    case "object": {
      const object = makeReference({ kind: "feature", path: binding.path, binding });
      for (const [name, child] of sortedEntries(shape.fields)) {
        object[name] = symbolicInputResultShape(
          child,
          extendBinding(binding, [name], "feature"),
        );
      }
      return object;
    }
    case "tuple": return shape.items.map((child, index) =>
      symbolicInputResultShape(child, extendBinding(binding, [index], "feature")));
    case "dynamic_keyed": {
      const collectionBinding = { ...binding, expected_kind: "collection" as const };
      const collection = makeReference({
        kind: "collection",
        path: binding.path,
        binding: collectionBinding,
      });
      collection.keys = Object.freeze([]);
      collection.byKey = new Proxy(Object.create(null) as object, {
        get: (_target, property) => {
          if (typeof property !== "string") return undefined;
          requireKey(property, "feature member key");
          return symbolicInputResultShape(
            shape.member,
            extendBinding(collectionBinding, [{ member: property }], "feature"),
          );
        },
      });
      return collection;
    }
  }
}

function arrayIndex(value: string): number | undefined {
  if (!/^(?:0|[1-9][0-9]*)$/u.test(value)) return undefined;
  const index = Number(value);
  return Number.isSafeInteger(index) ? index : undefined;
}

function extendBinding(
  binding: RuntimeBinding,
  suffix: readonly ArtifactPathSegment[],
  expectedKind: FeatureKind,
): RuntimeBinding {
  return { ...binding, path: [...binding.path, ...suffix], expected_kind: expectedKind };
}

function symbolicCollection(
  inputName: string,
  memberFactory: () => unknown,
  ruleId?: number,
): object {
  const value = Object.create(null) as Record<PropertyKey, unknown>;
  Object.defineProperty(value, collectionRuntime, {
    value: { inputName, memberFactory, ...(ruleId === undefined ? {} : { ruleId }) },
  });
  return Object.freeze(value);
}

function collectionData(value: unknown): SymbolicCollection | undefined {
  return typeof value === "object" && value !== null && collectionRuntime in value
    ? (value as { readonly [collectionRuntime]: SymbolicCollection })[collectionRuntime]
    : undefined;
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
  Object.defineProperty(keyed, "key", { value: "__member__", enumerable: true });
  return keyed;
}

function visitResult(
  value: unknown,
  path: string[],
  outputs: Record<string, FeatureKind>,
  templates: PendingTemplate[],
  rules: PendingRule[],
): void {
  const reference = referenceData(value);
  if (reference !== undefined) {
    const outputName = path[0] ?? "result";
    if (outputs[outputName] === undefined) outputs[outputName] = reference.kind;
    if (reference.templateId !== undefined) {
      const template = templates[reference.templateId];
      if (template === undefined) throw new TypeError("result references an unknown declaration");
      const resultPath = path.length === 0 ? [template.localId] : [...path];
      if (template.path !== undefined && !samePath(template.path, resultPath)) {
        throw new TypeError("one declaration result cannot be returned at multiple semantic paths");
      }
      template.path = resultPath;
      recordTemplateResultOutput(reference, template);
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
    }
    return;
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("patch outputs must be typed references, records, or keyed collections");
  }
  for (const [key, child] of sortedEntries(value as Record<string, unknown>)) {
    if (path.length === 0) requireKey(key, "patch output key");
    if (path.length === 0 && referenceData(child) === undefined && collectionData(child) === undefined) {
      outputs[key] = "collection";
    }
    visitResult(child, [...path, key], outputs, templates, rules);
  }
}

function visitCollectionMemberResult(
  value: unknown,
  path: string[],
  templateIds: ReadonlySet<number>,
  templates: PendingTemplate[],
): void {
  const reference = referenceData(value);
  if (reference !== undefined) {
    if (reference.templateId === undefined || !templateIds.has(reference.templateId)) {
      throw new TypeError("collection callback must return its recorded declaration result");
    }
    const template = templates[reference.templateId];
    if (template === undefined) throw new TypeError("collection result references an unknown declaration");
    if (path.length !== 0) {
      if (template.path !== undefined && !samePath(template.path, path)) {
        throw new TypeError("one collection declaration result cannot be returned at multiple paths");
      }
      template.path = [...path];
    }
    recordTemplateResultOutput(reference, template);
    return;
  }
  if (collectionData(value) !== undefined || typeof value !== "object" || value === null
      || Array.isArray(value)) {
    throw new TypeError("collection callback results must be typed declaration references or records");
  }
  for (const [key, child] of sortedEntries(value as Record<string, unknown>)) {
    requireKey(key, "collection result key");
    visitCollectionMemberResult(child, [...path, key], templateIds, templates);
  }
}

function recordTemplateResultOutput(reference: RuntimeReference, template: PendingTemplate): void {
  const binding = reference.binding;
  if (binding.source !== "template_output" || binding.templateId !== template.id) {
    throw new TypeError("declaration result is missing exact output provenance");
  }
  const resultOutput = [...binding.path];
  if (resultOutput.length !== 0) {
    const descriptor = template.outputs.find((output) => sameArtifactPath(output.path, resultOutput));
    if (descriptor === undefined || descriptor.kind !== reference.kind) {
      throw new TypeError("a selected declaration result must name one catalog-authenticated output path");
    }
  }
  if (template.resultSelectionRecorded === true
      && !sameArtifactPath(template.resultOutput ?? [], resultOutput)) {
    throw new TypeError("one declaration cannot expose multiple selected outputs at one return path");
  }
  template.resultSelectionRecorded = true;
  if (resultOutput.length !== 0) template.resultOutput = resultOutput;
}

function artifactBinding(
  binding: RuntimeBinding,
  templates: readonly PendingTemplate[],
): ArtifactTemplateBinding {
  if (binding.source !== "template_output") return binding;
  const template = templates[binding.templateId];
  if (template === undefined) throw new TypeError("binding references an unknown declaration");
  return {
    source: "template_output",
    template: [template.localId],
    path: binding.path,
    expected_kind: binding.expected_kind,
  };
}

function pendingArgument(value: unknown): PendingTemplateArgument {
  const reference = referenceData(value);
  if (reference !== undefined) {
    return { argument: "binding", value: reference.binding };
  }
  if (Array.isArray(value)) {
    return { argument: "array", value: value.map(pendingArgument) };
  }
  if (isUnit(value) || value === null || typeof value !== "object") {
    return { argument: "literal", value: managedValue(value) };
  }
  return {
    argument: "object",
    value: sortedObject(Object.fromEntries(
      sortedEntries(value as Record<string, unknown>)
        .map(([name, child]) => [name, pendingArgument(child)]),
    )),
  };
}

function artifactArgument(
  argument: PendingTemplateArgument,
  templates: readonly PendingTemplate[],
): ArtifactTemplateArgument {
  switch (argument.argument) {
    case "literal": return argument;
    case "binding": return {
      argument: "binding",
      value: artifactBinding(argument.value, templates),
    };
    case "array": return {
      argument: "array",
      value: argument.value.map((child) => artifactArgument(child, templates)),
    };
    case "object": return {
      argument: "object",
      value: sortedObject(Object.fromEntries(
        sortedEntries(argument.value).map(([name, child]) => [
          name,
          artifactArgument(child, templates),
        ]),
      )),
    };
  }
}

function schemaFeatureKind(schema: PrivateSchemaRuntime): FeatureKind {
  switch (schema.kind) {
    case "point": return "point";
    case "corner": return "feature_corner";
    case "curve": return "curve";
    case "curveSpan": return "curve_span";
    case "scalar":
    case "length":
    case "angle": return "scalar";
    case "keyed":
    case "record": return "collection";
    case "feature": return "feature";
  }
}

function managedValue(value: unknown): ManagedArtifactValue {
  if (value === null) return { kind: "null" };
  if (typeof value === "boolean") return { kind: "bool", value };
  if (typeof value === "number") {
    requireFinite(value, "declaration number");
    return { kind: "number", value };
  }
  if (typeof value === "string") return { kind: "string", value };
  if (isUnit(value)) return { kind: "unit", value };
  if (Array.isArray(value)) return { kind: "array", value: value.map(managedValue) };
  if (typeof value === "object" && value !== null) {
    return {
      kind: "object",
      value: sortedObject(Object.fromEntries(
        sortedEntries(value as Record<string, unknown>).map(([name, child]) => [name, managedValue(child)]),
      )),
    };
  }
  throw new TypeError("declaration literals must be finite data-only values");
}

function isUnit(value: unknown): value is { readonly unit: string; readonly value: number } {
  return typeof value === "object" && value !== null
    && typeof (value as { readonly unit?: unknown }).unit === "string"
    && typeof (value as { readonly value?: unknown }).value === "number";
}

const rustFloat = Symbol("geosolve.sketch-code.rust-f64");
interface RustFloat { readonly [rustFloat]: number }

export function canonicalArtifactJson(artifact: PatchModuleArtifact): string {
  return rustCompatibleJson({
    format: artifact.format,
    sdk_abi: artifact.sdk_abi,
    module_specifier: artifact.module_specifier,
    export_name: artifact.export_name,
    source_digest: artifact.source_digest,
    interface_digest: artifact.interface_digest,
    inputs: artifact.inputs,
    outputs: artifact.outputs,
    templates: artifact.templates.map((template) => ({
      path: template.path,
      result_path: template.result_path,
      result_output: template.result_output,
      declaration_family: template.declaration_family,
      arguments: wireTemplateArgument(template.arguments),
      outputs: template.outputs,
    })),
    collections: artifact.collections.map((collection) =>
      collection.rule === "each"
        ? {
          rule: collection.rule,
          path: collection.path,
          input: collection.input,
          member_key_field: collection.member_key_field,
          templates: collection.templates,
        }
        : {
          rule: collection.rule,
          path: collection.path,
          input: collection.input,
          templates: collection.templates,
        }
    ),
  });
}

function wireTemplateArgument(argument: ArtifactTemplateArgument): unknown {
  switch (argument.argument) {
    case "literal": return {
      argument: "literal",
      value: wireManagedValue(argument.value),
    };
    case "binding": return argument;
    case "array": return {
      argument: "array",
      value: argument.value.map(wireTemplateArgument),
    };
    case "object": return {
      argument: "object",
      value: Object.fromEntries(
        Object.entries(argument.value).map(([key, child]) => [
          key,
          wireTemplateArgument(child),
        ]),
      ),
    };
  }
}

function wireManagedValue(value: ManagedArtifactValue): unknown {
  switch (value.kind) {
    case "null": return { kind: "null" };
    case "bool":
    case "string": return { kind: value.kind, value: value.value };
    case "number": return { kind: "number", value: { [rustFloat]: value.value } };
    case "unit": return {
      kind: "unit",
      value: { unit: value.value.unit, value: { [rustFloat]: value.value.value } },
    };
    case "array": return { kind: "array", value: value.value.map(wireManagedValue) };
    case "object": return {
      kind: "object",
      value: Object.fromEntries(
        Object.entries(value.value).map(([key, child]) => [key, wireManagedValue(child)]),
      ),
    };
  }
}

function rustCompatibleJson(value: unknown): string {
  if (isRustFloat(value)) return formatRustFloat(value[rustFloat]);
  if (value === null) return "null";
  if (typeof value === "string" || typeof value === "boolean") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new TypeError("artifact numbers must be finite");
    return String(value);
  }
  if (Array.isArray(value)) return `[${value.map(rustCompatibleJson).join(",")}]`;
  if (typeof value === "object" && value !== null) {
    return `{${Object.entries(value)
      .map(([key, child]) => `${JSON.stringify(key)}:${rustCompatibleJson(child)}`)
      .join(",")}}`;
  }
  throw new TypeError("artifact contains a non-data value");
}

function isRustFloat(value: unknown): value is RustFloat {
  return typeof value === "object" && value !== null && rustFloat in value;
}

function formatRustFloat(value: number): string {
  if (!Number.isFinite(value)) throw new TypeError("artifact numbers must be finite");
  if (Object.is(value, -0)) return "-0.0";
  const magnitude = Math.abs(value);
  if (Number.isInteger(value) && magnitude < 1e16) return `${value}.0`;
  return magnitude !== 0 && (magnitude < 1e-5 || magnitude >= 1e16)
    ? value.toExponential()
    : String(value);
}

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function requireModuleSpecifier(value: string): void {
  if (!value.startsWith("./patches/") || !value.endsWith(".patch.ts")
      || value.includes("..") || value.includes("\\")) {
    throw new TypeError(`invalid custom patch module specifier ${JSON.stringify(value)}`);
  }
}

function requireKey(value: string, label: string): void {
  if (value.length === 0 || value.length > 256 || !/^[A-Za-z0-9_.-]+$/u.test(value)) {
    throw new TypeError(`invalid ${label} ${JSON.stringify(value)}`);
  }
}

function requireFinite(value: number, label: string): void {
  if (!Number.isFinite(value)) throw new TypeError(`${label} must be finite`);
}

function samePath(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((segment, index) => segment === right[index]);
}

function sameArtifactPath(
  left: readonly ArtifactPathSegment[],
  right: readonly ArtifactPathSegment[],
): boolean {
  return artifactPathKey(left) === artifactPathKey(right);
}

function sortedEntries<Value>(
  value: Readonly<Record<string, Value>>,
): readonly (readonly [string, Value])[] {
  return Object.entries(value).sort(([left], [right]) => compareText(left, right));
}

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function sortedObject<Value>(value: Readonly<Record<string, Value>>): Readonly<Record<string, Value>> {
  return Object.freeze(Object.fromEntries(sortedEntries(value)));
}

function deepFreeze<Value>(value: Value): Value {
  if (typeof value !== "object" || value === null || Object.isFrozen(value)) return value;
  for (const child of Object.values(value)) deepFreeze(child);
  return Object.freeze(value);
}
