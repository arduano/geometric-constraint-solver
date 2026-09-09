// SPDX-License-Identifier: GPL-3.0-or-later

import type { FeatureKind, SupportedUnit } from "./authoring.js";
import type { ParameterOptions, PresentationOptions, SketchOptions } from "./presentation.js";

/** Evaluated declarations only: this format grants no managed/source mutation authority. */
export const GENERATED_SKETCH_ARTIFACT_FORMAT = "geosolve-generated-sketch-v1" as const;
export const GENERATED_SKETCH_SDK_ABI = "geosolve-sketch-code-v2" as const;
export type GeneratedIdentity = readonly string[];
export type GeneratedPathSegment = string | number | { readonly member: string };
export interface GeneratedReference {
  readonly identity: GeneratedIdentity;
  readonly path: readonly GeneratedPathSegment[];
  readonly kind: FeatureKind;
}
export type GeneratedValue =
  | { readonly kind: "null" }
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "number"; readonly value: number }
  | { readonly kind: "string"; readonly value: string }
  | { readonly kind: "unit"; readonly value: { readonly unit: SupportedUnit; readonly value: number } }
  | { readonly kind: "array"; readonly value: readonly GeneratedValue[] }
  | { readonly kind: "object"; readonly value: Readonly<Record<string, GeneratedValue>> }
  | { readonly kind: "reference"; readonly value: GeneratedReference };

export interface GeneratedDeclaration {
  readonly identity: GeneratedIdentity;
  readonly family: string;
  readonly arguments: GeneratedValue;
}
export interface GeneratedApplication {
  readonly identity: GeneratedIdentity;
  readonly declarations: readonly GeneratedIdentity[];
  readonly inputs: GeneratedValue;
  readonly input_presentation: Readonly<Record<string, ParameterOptions>>;
  readonly presentation: PresentationOptions;
  readonly output: GeneratedValue;
}
export interface GeneratedSketchArtifact {
  readonly format: typeof GENERATED_SKETCH_ARTIFACT_FORMAT;
  readonly sdk_abi: typeof GENERATED_SKETCH_SDK_ABI;
  /** Execution order; patch-local declarations are fully evaluated and scoped by identity. */
  readonly declarations: readonly GeneratedDeclaration[];
  readonly applications: readonly GeneratedApplication[];
  readonly parameters: readonly { readonly id: string; readonly value: GeneratedValue; readonly presentation: ParameterOptions }[];
  readonly groups: readonly { readonly name: string; readonly declarations: readonly GeneratedIdentity[] }[];
  readonly suppressions: readonly GeneratedIdentity[];
  readonly document: SketchOptions;
  readonly output: GeneratedValue;
}
