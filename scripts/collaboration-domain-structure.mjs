// SPDX-License-Identifier: GPL-3.0-or-later
// Per-object compiler reconstruction. No saved whole-scene inverse is accepted.
import { sourceView, replaceOwnedStatements } from "./collaboration-domain-syntax.mjs";
import { sdkDirectory } from "./workspace-runtime-paths.mjs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
const managed = await import(pathToFileURL(resolve(sdkDirectory, "managed.js")).href);
const { localizedManagedSourceEdits } = await import(pathToFileURL(resolve(sdkDirectory, "source-patch.js")).href);
const fail = (message) => { throw Object.assign(Error(message), { code: "reconciliation_conflict" }); };
const sorted = (value) => Array.isArray(value) ? value.map(sorted) : value && typeof value === "object" ? Object.fromEntries(Object.keys(value).sort().map((key) => [key, sorted(value[key])])) : value;
const same = (a, b) => JSON.stringify(sorted(a)) === JSON.stringify(sorted(b));
const patchContext = (patches) => createHash("sha256").update(JSON.stringify(sorted(patches))).digest("hex");
const semantic = (value) => Array.isArray(value) ? value.map(semantic) : value && typeof value === "object" ? Object.fromEntries(Object.entries(value).filter(([key]) => !["site", "statement_span", "symbol_span", "arguments_span", "value_span"].includes(key)).map(([key, child]) => [key, semantic(child)])) : value;
const symbol = (statement) => statement.statement === "declaration" ? statement.symbol : statement.statement === "binding" ? statement.parameter?.symbol ?? statement.variable : undefined;
const object = (entry, name) => `${entry}#${name}`;
const refs = (value) => { const result = new Set(); const visit = (node) => { if (!node || typeof node !== "object") return; if (node.kind === "reference") result.add(node.declaration); Object.values(node).forEach(visit); }; visit(value); return result; };
function outputLeaves(value, variable, path = []) {
  if (!refs(value).has(variable)) return [];
  if (value.kind === "object") return value.fields.flatMap((field) => outputLeaves(field.value, variable, [...path, field.name]));
  // Array index ownership cannot be preserved across unrelated insertions.
  if (value.kind === "array" || refs(value).size !== 1) fail("Structural output needs one stable object-key path per referenced owner");
  return [{ path, value }];
}
export function enrichInventory(compiled, source, inventory) {
  const ir = compiled.compiled.ir, view = sourceView(source), order = ir.statements.map(symbol).filter((value) => value !== undefined);
  const context = patchContext(compiled.patches);
  const bySymbol = new Map(ir.statements.filter((item) => symbol(item) !== undefined).map((item) => [symbol(item), item]));
  for (const item of inventory.objects) {
    const statement = bySymbol.get(item.declaration), owner = view.owners.get(item.declaration), index = order.indexOf(item.declaration);
    if (!statement || !owner || index < 0) fail(`Compiler inventory/source owner mismatch for ${item.declaration}`);
    let output, unsupported;
    try { output = outputLeaves(ir.output, statement.variable); } catch (error) { unsupported = error.message; output = []; }
    item.position = { previous: index ? object(compiled.entry, order[index - 1]) : null, next: index + 1 < order.length ? object(compiled.entry, order[index + 1]) : null };
    item.payload = { format: "geosolve-object-compiler-v1", statement, source: owner.source, imports: ir.imports, patchContext: context,
      output, groups: ir.statements.filter((node) => node.statement === "group").flatMap((group) => {
        const declarations = group.declarations.filter((ref) => ref.declaration === statement.variable); return declarations.length ? [{ ...group, declarations }] : [];
      }), suppression: ir.statements.filter((node) => node.statement === "suppression" && node.target.declaration === statement.variable), ...(unsupported ? { unsupported } : {}) };
  }
  inventory.objects.sort((a, b) => order.indexOf(a.declaration) - order.indexOf(b.declaration));
  return inventory;
}
function mergeOutput(output, leaf) {
  if (!leaf.path.length) { if (!same(output, leaf.value) && !(output.kind === "object" && output.fields.length === 0)) fail("Restored output root has a newer owner"); return structuredClone(leaf.value); }
  let node = output;
  for (const [index, key] of leaf.path.entries()) {
    if (node.kind !== "object") fail("Restored output path changed kind");
    let field = node.fields.find((item) => item.name === key);
    if (index === leaf.path.length - 1) {
      if (field && !same(semantic(field.value), semantic(leaf.value))) fail("Restored output property has a newer owner");
      if (!field) node.fields.push({ name: key, value: structuredClone(leaf.value), comments: [] });
    } else {
      if (!field) { field = { name: key, value: { kind: "object", fields: [], site: "restored" }, comments: [] }; node.fields.push(field); }
      node = field.value;
    }
  }
  return output;
}
function retainOutput(value, removed) {
  if (![...refs(value)].some((ref) => removed.has(ref))) return value;
  if (value.kind === "object") return { ...value, fields: value.fields.flatMap((field) => { const retained = retainOutput(field.value, removed); return retained ? [{ ...field, value: retained }] : []; }) };
  if (value.kind === "array") return { ...value, values: value.values.flatMap((child) => { const retained = retainOutput(child, removed); return retained ? [retained] : []; }) };
  return undefined;
}
function declarationFromTarget(entry, target) {
  const prefix = `${entry}#`; if (typeof target?.object !== "string" || !target.object.startsWith(prefix)) fail("Inverse target belongs to another entry");
  return target.object.slice(prefix.length);
}
function placeAll(statements, placements, entry) {
  const declarations = statements.filter((item) => symbol(item) !== undefined), names = declarations.map(symbol), byName = new Map(declarations.map((item) => [symbol(item), item]));
  const moved = new Set(placements.map((item) => item.declaration)), edges = new Map(names.map((name) => [name, new Set()]));
  const edge = (a, b) => { if (!edges.has(a) || !edges.has(b)) fail("Inverse stable statement neighbor no longer exists"); if (a !== b) edges.get(a).add(b); };
  const retained = names.filter((name) => !moved.has(name)); for (let i = 1; i < retained.length; i++) edge(retained[i - 1], retained[i]);
  for (const { declaration, position } of placements) {
    if (!edges.has(declaration)) fail("Inverse reorder owner is absent");
    if (position.previous !== null) edge(declarationFromTarget(entry, position.previous), declaration);
    if (position.next !== null) edge(declaration, declarationFromTarget(entry, position.next));
  }
  const ordered = [], pending = new Set(names);
  while (pending.size) {
    const next = names.find((name) => pending.has(name) && ![...pending].some((other) => edges.get(other).has(name)));
    if (next === undefined) fail("Inverse stable statement neighbor intents conflict");
    pending.delete(next); ordered.push(byName.get(next));
  }
  // Bindings/declarations must precede references in groups and suppression.
  return [...ordered, ...statements.filter((item) => symbol(item) === undefined)];
}
export function prepareStructuralSource(compiled, source, inverse) {
  if (!inverse || !Array.isArray(inverse.changes) || !inverse.structural) fail("Expected native prepared semantic inverse");
  const ir = structuredClone(compiled.compiled.ir), plan = inverse.structural, restoredSources = [], entry = compiled.entry;
  if ((plan.create?.length ?? 0) > 1024 || (plan.reorders?.length ?? 0) > 1024) fail("Structural inverse exceeds compiler batch limit");
  const created = plan.create ?? [], removed = new Set((plan.delete?.closure ?? []).map((target) => declarationFromTarget(entry, target)));
  for (const addition of created) {
    const payload = addition.payload, name = declarationFromTarget(entry, addition.target);
    if (!payload || payload.format !== "geosolve-object-compiler-v1" || payload.unsupported || symbol(payload.statement) !== name) fail("Unsupported or mismatched per-object compiler payload");
    if (ir.statements.some((item) => symbol(item) === name || item.variable === payload.statement.variable)) fail("Restored declaration or lexical binding is occupied");
    if (payload.patchContext !== patchContext(compiled.patches)) fail("Restored declaration compiler patch context changed");
    for (const required of payload.imports) {
      const available = ir.imports.find((item) => item.module === required.module);
      if (!available || required.bindings.some((binding) => !available.bindings.includes(binding))) fail("Restored declaration import context changed");
    }
    ir.statements.push(structuredClone(payload.statement)); restoredSources.push([name, payload.source]);
    for (const leaf of payload.output) ir.output = mergeOutput(ir.output, leaf);
    for (const group of payload.groups) {
      let existing = ir.statements.find((item) => item.statement === "group" && item.name === group.name);
      if (!existing) { existing = { ...structuredClone(group), declarations: [] }; ir.statements.push(existing); }
      for (const ref of group.declarations) if (!existing.declarations.some((item) => same(semantic(item), semantic(ref)))) existing.declarations.push(structuredClone(ref));
    }
    ir.statements.push(...structuredClone(payload.suppression));
  }
  // Place all recreated siblings together, after they all exist. Incoming order
  // is native history order and references are already lifetime-authenticated.
  const placements = created.map((item) => ({ declaration: declarationFromTarget(entry, item.target), position: item.position }));
  for (const reorder of plan.reorders ?? []) {
    const name = declarationFromTarget(entry, reorder.target);
    if (!removed.has(name)) placements.push({ declaration: name, position: reorder.after });
  }
  const removedVariables = new Set(ir.statements.filter((item) => removed.has(symbol(item))).map((item) => item.variable));
  if (removedVariables.size !== removed.size) fail("Inverse deletion closure contains an absent declaration");
  ir.statements = ir.statements.flatMap((item) => {
    if (removed.has(symbol(item)) || item.statement === "suppression" && removedVariables.has(item.target.declaration)) return [];
    if (item.statement === "group") { const declarations = item.declarations.filter((ref) => !removedVariables.has(ref.declaration)); return declarations.length ? [{ ...item, declarations }] : []; }
    return [item];
  });
  ir.output = retainOutput(ir.output, removedVariables) ?? { kind: "object", fields: [], site: "restored" };
  if (placements.length) ir.statements = placeAll(ir.statements, placements, entry);
  let candidate = managed.compileManagedSource(managed.printManagedSource(ir), { patches: compiled.patches });
  if (inverse.changes.length) {
    for (const change of inverse.changes) {
      const path = JSON.parse(change.address.property); if (path[0] !== "metadata") continue;
      const target = path[1] === "document" ? { target: "document" } : { target: path[1], declaration: declarationFromTarget(entry, change.address.target) };
      const prior = compilerMetadata({ ...compiled, compiled: candidate }).find((item) => same(item.target, target) && item.property === path[2]);
      if (!prior || !same(prior.value, change.before)) fail("Metadata inverse no longer matches native compiler value");
      candidate = managed.applyManagedSketchMutation(candidate, { mutation: "set_metadata", target, property: path[2], value: change.after }, { patches: compiled.patches }).compiled;
    }
    const values = inverse.changes.flatMap((change) => {
      const declaration = declarationFromTarget(entry, change.address.target); if (removed.has(declaration)) return [];
      const path = JSON.parse(change.address.property);
      if (path[0] === "metadata") return [];
      if (!Array.isArray(path) || path[0] === "point" && typeof path[1] === "object") fail("Mixed point/source structural inverse requires explicit native overlay replay");
      return [{ declaration, path, expected: change.before, value: change.after }];
    });
    if (values.length) candidate = managed.applyManagedSketchMutation(candidate, { mutation: "set_values", values }, { patches: compiled.patches }).compiled;
  }
  const edits = localizedManagedSourceEdits(source, candidate.normalizedSource);
  let raw = source; for (const edit of [...edits].sort((a, b) => b.start - a.start)) raw = raw.slice(0, edit.start) + edit.replacement + raw.slice(edit.end);
  // Recreated owner trivia is part of its payload. Retained objects preserve
  // their live syntax through the existing localized compiler projection.
  if (restoredSources.length) raw = replaceOwnedStatements(raw, restoredSources);
  const localized = managed.compileManagedSource(raw, { patches: compiled.patches });
  if (!same(semantic(localized.ir.statements), semantic(candidate.ir.statements)) || !same(semantic(localized.ir.output), semantic(candidate.ir.output))) fail("Restored authored statement differs from independently composed compiler meaning");
  return { source: raw, compiled: localized };
}

export function compilerMetadata(compiled) {
  const ir = compiled.compiled.ir, result = [];
  const observe = (target, expression, fields) => {
    for (const property of fields) {
      const field = expression?.kind === "object" ? expression.fields.find((entry) => entry.name === property)?.value : undefined;
      if (field && !["boolean", "string"].includes(field.kind)) fail("Native metadata is not a literal");
      result.push({ object: `${compiled.entry}#${target.declaration ?? "@document"}`, target, property,
        value: field === undefined ? null : { kind: field.kind === "boolean" ? "bool" : "string", value: field.value } });
    }
  };
  observe({ target: "document" }, ir.document, ["title", "description"]);
  observe({ target: "document" }, ir.document?.fields.find((item) => item.name === "dimensions")?.value, ["areKeyConstraintsByDefault"]);
  for (const statement of ir.statements) {
    if (statement.statement === "binding" && statement.parameter) observe({ target: "parameter", declaration: statement.parameter.symbol }, statement.parameter.presentation, ["label", "description", "isKeyParameter"]);
    if (statement.statement === "declaration") observe({ target: "declaration", declaration: statement.symbol }, statement.patch ? statement.presentation : statement.arguments,
      ["label", "description", ...(statement.patch === null && statement.builder_path[0] === "dimension" ? ["isKeyConstraint"] : [])]);
  }
  return result;
}
export function metadataChanges(before, after, mutation) {
  const prior = compilerMetadata(before), current = compilerMetadata(after), key = (entry) => JSON.stringify([entry.object, entry.target.target, entry.property]);
  const previous = new Map(prior.map((item) => [key(item), item]));
  return current.flatMap((item) => {
    const old = previous.get(key(item));
    return old && (!same(old.value, item.value) || mutation?.mutation === "set_metadata" && same(mutation.target, item.target) && mutation.property === item.property)
      ? [{ object: item.object, target: item.target, property: item.property, before: old.value, after: item.value }] : [];
  });
}
