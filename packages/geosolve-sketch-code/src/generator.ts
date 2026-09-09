// SPDX-License-Identifier: GPL-3.0-or-later

import type { Sketch, SupportedUnit } from "./authoring.js";
import { validatePresentation } from "./presentation.js";
import type { PresentationOptions } from "./presentation.js";

/** Generator inputs describe invocation values, independently of sketch overview parameters. */
export type GeneratorInput = PresentationOptions & (
  | { readonly type: "number" | "integer"; readonly default: number; readonly min?: number; readonly max?: number; readonly unit?: SupportedUnit }
  | { readonly type: "boolean"; readonly default: boolean }
  | { readonly type: "string"; readonly default: string }
  | { readonly type: "choice"; readonly choices: readonly string[]; readonly default: string }
);

export type GeneratorInputs = Readonly<Record<string, GeneratorInput>>;
export type GeneratorInputValues<Inputs extends GeneratorInputs> = {
  readonly [Key in keyof Inputs]: Inputs[Key] extends { readonly type: "number" | "integer" } ? number
    : Inputs[Key] extends { readonly type: "boolean" } ? boolean
    : Inputs[Key] extends { readonly type: "choice"; readonly choices: readonly (infer Choice)[] } ? Choice
    : string;
};

export interface GeneratorDefinition<Inputs extends GeneratorInputs, Result> {
  (parameters?: Partial<GeneratorInputValues<Inputs>>): Sketch<Result>;
  readonly inputs: Inputs;
  /** Validate and fill defaults without evaluating or solving the design. */
  readonly parseInputs: (parameters?: Partial<GeneratorInputValues<Inputs>>) => GeneratorInputValues<Inputs>;
}

/** Optional discoverable inputs for ordinary, synchronous TypeScript sketch functions. */
export function defineGenerator<const Inputs extends GeneratorInputs, Result>(
  inputs: Inputs,
  build: (parameters: GeneratorInputValues<Inputs>) => Sketch<Result>,
): GeneratorDefinition<Inputs, Result> {
  requireObject(inputs, "generator inputs");
  if (typeof build !== "function") throw new TypeError("generator requires a builder function");
  const entries = Object.entries(inputs).map(([name, descriptor]) => {
    if (!/^[A-Za-z][A-Za-z0-9_.-]{0,255}$/u.test(name)) throw new TypeError(`invalid generator input ID ${JSON.stringify(name)}`);
    requireObject(descriptor, `generator input ${name}`);
    validatePresentation(Object.fromEntries(Object.entries(descriptor).filter(([key]) => key === "label" || key === "description")));
    const allowed = new Set(["type", "default", "label", "description",
      ...(descriptor.type === "number" || descriptor.type === "integer" ? ["min", "max", "unit"] : []),
      ...(descriptor.type === "choice" ? ["choices"] : []),
    ]);
    for (const key of Object.keys(descriptor)) if (!allowed.has(key)) throw new TypeError(`unknown generator input field ${name}.${key}`);
    if (descriptor.type === "number" || descriptor.type === "integer") {
      for (const limit of [descriptor.min, descriptor.max]) if (limit !== undefined && (typeof limit !== "number" || !Number.isFinite(limit))) throw new TypeError(`generator input ${name} limits must be finite`);
      if (descriptor.min !== undefined && descriptor.max !== undefined && descriptor.min > descriptor.max) throw new TypeError(`generator input ${name} minimum exceeds maximum`);
      if (descriptor.unit !== undefined && !["mm", "cm", "m", "inch", "deg", "rad"].includes(descriptor.unit)) throw new TypeError(`unsupported generator input unit ${name}`);
    }
    if (descriptor.type === "choice" && (!Array.isArray(descriptor.choices) || descriptor.choices.length === 0 || descriptor.choices.some((choice) => typeof choice !== "string") || new Set(descriptor.choices).size !== descriptor.choices.length)) throw new TypeError(`generator input ${name} requires distinct string choices`);
    validateInput(name, descriptor, descriptor.default);
    return [name, Object.freeze({ ...descriptor, ...(descriptor.type === "choice" ? { choices: Object.freeze([...descriptor.choices]) } : {}) })] as const;
  });
  const schemas = Object.freeze(Object.fromEntries(entries)) as Inputs;
  const parseInputs = (parameters: Partial<GeneratorInputValues<Inputs>> = {}): GeneratorInputValues<Inputs> => {
    requireObject(parameters, "generator parameters");
    for (const name of Object.keys(parameters)) if (!Object.hasOwn(schemas, name)) throw new TypeError(`unknown generator input ${name}`);
    return Object.freeze(Object.fromEntries(entries.map(([name, descriptor]) => {
      const value = Object.hasOwn(parameters, name) ? (parameters as Record<string, unknown>)[name] : descriptor.default;
      validateInput(name, descriptor, value);
      return [name, value];
    }))) as GeneratorInputValues<Inputs>;
  };
  return Object.freeze(Object.assign(
    (parameters?: Partial<GeneratorInputValues<Inputs>>) => build(parseInputs(parameters)),
    { inputs: schemas, parseInputs },
  ));
}

function validateInput(name: string, descriptor: GeneratorInput, value: unknown): void {
  const fail = (message: string): never => { throw new TypeError(`generator input ${name} ${message}`); };
  switch (descriptor.type) {
    case "number":
    case "integer":
      if (typeof value !== "number" || !Number.isFinite(value)) fail("must be a finite number");
      if (descriptor.type === "integer" && !Number.isSafeInteger(value)) fail("must be a safe integer");
      if (descriptor.min !== undefined && (value as number) < descriptor.min) fail(`must be at least ${descriptor.min}`);
      if (descriptor.max !== undefined && (value as number) > descriptor.max) fail(`must be at most ${descriptor.max}`);
      return;
    case "boolean": if (typeof value !== "boolean") fail("must be boolean"); return;
    case "string": if (typeof value !== "string") fail("must be a string"); return;
    case "choice": if (typeof value !== "string" || !descriptor.choices.includes(value)) fail("must be one of its declared choices"); return;
    default: fail("has an unsupported type");
  }
}

function requireObject(value: unknown, name: string): asserts value is Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value) || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) throw new TypeError(`${name} must be a plain object`);
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== "string" || !("value" in Object.getOwnPropertyDescriptor(value, key)!)) throw new TypeError(`${name} requires ordinary string-keyed fields`);
  }
}
