// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Explicit caller-owned Node compiler for trusted custom patch modules.
 *
 * This is a build tool, not a browser/runtime loader. It executes only the
 * already-imported `PatchDefinition` supplied by its caller and emits the
 * canonical data-only shape admitted by Rust's `artifact.rs`.
 */

import { createHash } from "node:crypto";

import { canonicalStringify } from "@geosolve/intent";

import {
  PATCH_ARTIFACT_FORMAT,
  PATCH_ARTIFACT_LIMIT,
  SKETCH_CODE_SDK_ABI,
  recordPatchArtifact,
} from "./index.js";
import type {
  ArtifactCollectionRule,
  ArtifactTemplateNode,
  FeatureKind,
  InputSchemas,
  ManagedArtifactValue,
  PatchDefinition,
  SemanticPathSegment,
} from "./index.js";

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
  readonly edit_lenses: readonly {
    readonly output: readonly SemanticPathSegment[];
    readonly invocation_argument: readonly string[];
    readonly expected_kind: FeatureKind;
  }[];
}

export interface CompilePatchArtifactOptions<
  Schemas extends InputSchemas,
  Result,
> {
  /** Exact trusted source bytes used by the caller to load the module. */
  readonly source: string | Uint8Array;
  /** Bounded project-relative path, for example `./patches/round.patch.ts`. */
  readonly moduleSpecifier: string;
  readonly exportName: string;
  readonly patch: PatchDefinition<Schemas, Result>;
}

export interface CompiledPatchArtifact {
  readonly artifact: PatchModuleArtifact;
  /** Exact bytes accepted by `PatchModuleArtifact::from_canonical_json`. */
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
    edit_lenses: plan.edit_lenses,
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

/** Alias with a name suited to build scripts compiling one module export. */
export const compilePatchModule = compilePatchArtifact;

const rustFloat = Symbol("geosolve.sketch-code.rust-f64");

interface RustFloat {
  readonly [rustFloat]: number;
}

export function canonicalArtifactJson(artifact: PatchModuleArtifact): string {
  // Property insertion order below deliberately mirrors the Rust struct and
  // tagged-enum field order. BTreeMap-shaped objects were already UTF-8 sorted
  // by the recorder.
  const value = {
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
      declaration_family: template.declaration_family,
      inputs: template.inputs,
      fields: Object.fromEntries(
        Object.entries(template.fields).map(([key, value]) => [key, wireManagedValue(value)]),
      ),
      outputs: template.outputs,
    })),
    collections: artifact.collections.map((collection) => collection.rule === "each"
      ? {
        rule: "each" as const,
        path: collection.path,
        input: collection.input,
        member_key_field: collection.member_key_field,
        templates: collection.templates,
      }
      : {
        rule: "map_record" as const,
        path: collection.path,
        input: collection.input,
        templates: collection.templates,
      }),
    edit_lenses: artifact.edit_lenses.map((lens) => ({
      output: lens.output,
      invocation_argument: lens.invocation_argument,
      expected_kind: lens.expected_kind,
    })),
  };
  return rustCompatibleJson(value);
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

function wireManagedValue(value: ManagedArtifactValue): unknown {
  switch (value.kind) {
    case "null":
      return { kind: "null" };
    case "bool":
    case "string":
      return { kind: value.kind, value: value.value };
    case "number":
      return { kind: "number", value: { [rustFloat]: value.value } };
    case "unit":
      return {
        kind: "unit",
        value: { unit: value.value.unit, value: { [rustFloat]: value.value.value } },
      };
    case "array":
      return { kind: "array", value: value.value.map(wireManagedValue) };
    case "object":
      return {
        kind: "object",
        value: Object.fromEntries(
          Object.entries(value.value).map(([key, child]) => [key, wireManagedValue(child)]),
        ),
      };
  }
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

function deepFreeze<Value>(value: Value): Value {
  if (typeof value !== "object" || value === null || Object.isFrozen(value)) return value;
  for (const child of Object.values(value)) deepFreeze(child);
  return Object.freeze(value);
}
