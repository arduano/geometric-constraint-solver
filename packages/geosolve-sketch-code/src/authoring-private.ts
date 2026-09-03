// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Private rendezvous between the public authoring runtime and the explicit
 * build-time patch compiler. Keeping this state in WeakMaps lets the compiler
 * execute a trusted callback without exporting transport hooks from the
 * authoring package root.
 */

export type PrivateSchemaKind =
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

export interface PrivateSchemaRuntime {
  readonly kind: PrivateSchemaKind;
  readonly feature?: string;
  readonly element?: object;
}

export interface PrivatePatchRuntime {
  readonly inputs: Readonly<Record<string, object>>;
  readonly build: (builder: unknown, inputs: unknown) => unknown;
}

const schemaRuntimes = new WeakMap<object, PrivateSchemaRuntime>();
const patchRuntimes = new WeakMap<object, PrivatePatchRuntime>();

export function registerAuthoringSchema(
  schema: object,
  runtime: PrivateSchemaRuntime,
): void {
  schemaRuntimes.set(schema, runtime);
}

export function authoringSchemaRuntime(schema: object): PrivateSchemaRuntime {
  const runtime = schemaRuntimes.get(schema);
  if (runtime === undefined) {
    throw new TypeError("value schema was not created by this GeoSolve SDK instance");
  }
  return runtime;
}

export function registerPatchRuntime(
  patch: object,
  runtime: PrivatePatchRuntime,
): void {
  patchRuntimes.set(patch, runtime);
}

export function authoringPatchRuntime(patch: object): PrivatePatchRuntime {
  const runtime = patchRuntimes.get(patch);
  if (runtime === undefined) {
    throw new TypeError("patch was not created by this GeoSolve SDK instance");
  }
  return runtime;
}
