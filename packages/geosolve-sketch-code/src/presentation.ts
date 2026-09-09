// SPDX-License-Identifier: GPL-3.0-or-later

/** Plain authored presentation; these values never change solver semantics. */
export interface PresentationOptions {
  readonly label?: string;
  readonly description?: string;
}
export interface ParameterOptions extends PresentationOptions {
  readonly isKeyParameter?: boolean;
}
export interface SketchOptions {
  readonly title?: string;
  readonly description?: string;
  readonly dimensions?: { readonly areKeyConstraintsByDefault?: boolean };
}

export function validatePresentation(value: unknown, flag?: "isKeyParameter" | "isKeyConstraint"): void {
  const record = object(value, "presentation options");
  for (const name of Object.keys(record)) {
    if (name !== "label" && name !== "description" && name !== flag) throw new TypeError(`unknown presentation field ${name}`);
  }
  if (record.label !== undefined) {
    const label = record.label;
    if (typeof label !== "string" || new TextEncoder().encode(label).length > 256 || label.trim() !== label || /[\p{Cc}]/u.test(label)) {
      throw new TypeError("label must be at most 256 UTF-8 bytes without padding or control characters");
    }
  }
  validateText(record.description, "description", 2048);
  if (flag !== undefined && record[flag] !== undefined && typeof record[flag] !== "boolean") throw new TypeError(`${flag} must be boolean`);
}
export function validateSketchOptions(value: unknown): void {
  const record = object(value, "sketch options");
  for (const name of Object.keys(record)) {
    if (!["title", "description", "dimensions"].includes(name)) throw new TypeError(`unknown sketch option ${name}`);
  }
  validateText(record.title, "title", 128);
  if (typeof record.title === "string" && /[\p{Cc}]/u.test(record.title)) throw new TypeError("title cannot contain control characters");
  validateText(record.description, "description", 2048);
  if (record.dimensions !== undefined) {
    const dimensions = object(record.dimensions, "dimension defaults");
    for (const name of Object.keys(dimensions)) {
      if (name !== "areKeyConstraintsByDefault") throw new TypeError(`unknown dimension default ${name}`);
    }
    if (dimensions.areKeyConstraintsByDefault !== undefined && typeof dimensions.areKeyConstraintsByDefault !== "boolean") throw new TypeError("areKeyConstraintsByDefault must be boolean");
  }
}
function object(value: unknown, name: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new TypeError(`${name} must be an object`);
  return value as Record<string, unknown>;
}
function validateText(value: unknown, name: string, max: number): void {
  if (value !== undefined && (typeof value !== "string" || [...value].length > max || /[\u0000]/u.test(value))) throw new TypeError(`${name} must be plain text of at most ${max} characters`);
}
