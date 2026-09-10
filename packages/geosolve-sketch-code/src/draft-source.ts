// SPDX-License-Identifier: GPL-3.0-or-later

import ts from "typescript";
import { localizedManagedSourceEdits } from "./source-patch.js";
import type { ManagedSourceEdit, ManagedSourcePatch } from "./source-patch.js";
import type { ManagedMetadataTarget, ManagedSketchMutation, SemanticPathSegment } from "./managed.js";

export type ManagedDraftPendingReason =
  | "ownership_missing"
  | "ownership_ambiguous"
  | "unsupported_structure"
  | "resource_limit";

export type ManagedDraftReconciliation =
  | { readonly status: "reconciled"; readonly source: string; readonly patch: ManagedSourcePatch }
  | { readonly status: "pending"; readonly source: string; readonly reason: ManagedDraftPendingReason };

export interface ManagedDraftReconciliationInput {
  readonly acceptedSource: string;
  readonly workingSource: string;
  readonly acceptedPatch: ManagedSourcePatch;
}

class Pending extends Error {
  constructor(readonly reason: ManagedDraftPendingReason) { super(reason); }
}

interface Owner {
  readonly identity: string;
  readonly family: string;
  readonly variable: string;
  readonly statement: ts.VariableStatement;
  readonly initializer: ts.Expression;
  readonly call?: ts.CallExpression;
  readonly value: ts.Expression;
}

interface RecoverySource {
  readonly source: string;
  readonly file: ts.SourceFile;
  readonly call: ts.CallExpression;
  readonly body: ts.Block;
  readonly owners: ReadonlyMap<string, readonly Owner[]>;
  readonly variables: ReadonlyMap<string, number>;
  readonly diagnostics: readonly ts.DiagnosticWithLocation[];
}

type DraftEdits = { readonly edits: readonly ManagedSourceEdit[] } | { readonly reason: ManagedDraftPendingReason };

/** Recovery syntax supplies lexical ownership only; no working text executes. */
export function reconcileDraftSourceEdits(
  acceptedSource: string,
  candidateSource: string,
  workingSource: string,
  acceptedPatch: ManagedSourcePatch,
  mutation: ManagedSketchMutation,
): DraftEdits {
  try {
    const accepted = recover(acceptedSource);
    const candidate = recover(candidateSource);
    const working = recover(workingSource);
    const edits: ManagedSourceEdit[] = [];
    const replaceValue = (old: ts.Expression, next: ts.Expression): void => {
      requireIntact(old, working);
      // Local projection retains unit-call comments, numeric spelling and
      // unchanged children. Its input is this one identified value owner.
      const offset = old.getStart(working.file);
      for (const edit of localizedManagedSourceEdits(old.getText(working.file), next.getText(candidate.file))) {
        edits.push({ ...edit, start: offset + edit.start, end: offset + edit.end });
      }
    };
    const value = (identity: string, path: readonly SemanticPathSegment[]): void => {
      const baseOwner = uniqueOwner(accepted, identity);
      const workOwner = sameOwner(baseOwner, working);
      const nextOwner = sameOwner(baseOwner, candidate);
      requireIntact(workOwner.statement, working);
      const currentValue = valueAtPath(baseOwner.value, workOwner.value, path, accepted, working);
      const nextValue = valueAtPath(baseOwner.value, nextOwner.value, path, accepted, candidate);
      replaceValue(currentValue, nextValue);
    };
    switch (mutation.mutation) {
      case "set_value": value(mutation.declaration, mutation.path); break;
      case "set_values": mutation.values.forEach((entry) => value(entry.declaration, entry.path)); break;
      case "set_metadata": {
        const oldRoot = metadataRoot(accepted, mutation.target);
        const workRoot = metadataRoot(working, mutation.target, accepted);
        const nextRoot = metadataRoot(candidate, mutation.target, accepted);
        const path = mutation.property === "areKeyConstraintsByDefault"
          ? ["dimensions", mutation.property] : [mutation.property];
        const oldValue = optionalPropertyPath(oldRoot, path);
        const workValue = optionalPropertyPath(workRoot, path);
        const nextValue = optionalPropertyPath(nextRoot, path);
        if (workValue !== undefined && nextValue !== undefined) {
          replaceValue(workValue, nextValue);
        } else if (workValue === undefined && nextValue === undefined) {
          // An already-absent override needs no text contribution.
        } else {
          // Missing/new/removed metadata needs punctuation ownership too. Only
          // reuse the authenticated local patch when that complete owner stayed
          // exact; broader structural reconciliation remains explicit pending.
          if ((oldValue === undefined) !== (workValue === undefined)) throw new Pending("unsupported_structure");
          edits.push(...mapUnchangedOwnedEdits(accepted, working, acceptedPatch));
        }
        break;
      }
      default:
        edits.push(...mapUnchangedOwnedEdits(accepted, working, acceptedPatch, true));
    }
    edits.sort((left, right) => left.start - right.start || left.end - right.end);
    for (let index = 1; index < edits.length; index += 1) {
      if (edits[index]!.start < edits[index - 1]!.end || edits[index]!.start === edits[index - 1]!.start) {
        throw new Pending("ownership_ambiguous");
      }
    }
    if (edits.length > 1_024) throw new Pending("resource_limit");
    return { edits };
  } catch (error) {
    if (error instanceof Pending) return { reason: error.reason };
    if (error instanceof RangeError) return { reason: "resource_limit" };
    if (error instanceof TypeError) return { reason: "unsupported_structure" };
    throw error;
  }
}

function recover(source: string): RecoverySource {
  const file = ts.createSourceFile("sketch.ts", source, ts.ScriptTarget.ES2022, true, ts.ScriptKind.TS);
  const exports = file.statements.filter(ts.isExportAssignment);
  if (exports.length > 1) throw new Pending("ownership_ambiguous");
  const expression = exports[0]?.expression;
  if (expression === undefined || !ts.isCallExpression(expression)
    || !ts.isIdentifier(expression.expression) || expression.expression.text !== "sketch") throw new Pending("ownership_missing");
  const callback = expression.arguments.at(-1);
  if (callback === undefined || !ts.isArrowFunction(callback) || !ts.isBlock(callback.body)
    || callback.parameters.length !== 1 || !ts.isIdentifier(callback.parameters[0]!.name)
    || callback.parameters[0]!.name.text !== "$") throw new Pending("ownership_missing");
  const owners = new Map<string, Owner[]>();
  const variables = new Map<string, number>();
  if (callback.body.statements.length > 65_536) throw new Pending("resource_limit");
  for (const statement of callback.body.statements) {
    if (!ts.isVariableStatement(statement)) continue;
    for (const declaration of statement.declarationList.declarations) {
      if (!ts.isIdentifier(declaration.name)) continue;
      const variable = declaration.name.text;
      variables.set(variable, (variables.get(variable) ?? 0) + 1);
      if (statement.declarationList.declarations.length !== 1 || declaration.initializer === undefined) continue;
      const initializer = declaration.initializer;
      const call = ts.isCallExpression(initializer) ? initializer : undefined;
      const builder = call === undefined ? undefined : builderFamily(call.expression);
      const symbol = call?.arguments[0];
      const hasSymbol = builder !== undefined && symbol !== undefined && ts.isStringLiteral(symbol);
      const identity = hasSymbol ? symbol.text : variable;
      const family = builder ?? "binding";
      const value = hasSymbol ? call!.arguments[builder === "use" ? 2 : 1] : initializer;
      // Keep malformed/ambiguous declarations as candidates so recovery cannot
      // silently choose a second valid statement with the same stable symbol.
      const owner: Owner = { identity, family, variable, statement, initializer,
        ...(call === undefined ? {} : { call }), value: value ?? initializer };
      const entries = owners.get(identity) ?? [];
      entries.push(owner);
      owners.set(identity, entries);
    }
  }
  return { source, file, call: expression, body: callback.body, owners, variables,
    diagnostics: (file as ts.SourceFile & { readonly parseDiagnostics: readonly ts.DiagnosticWithLocation[] }).parseDiagnostics };
}

function builderFamily(expression: ts.Expression): string | undefined {
  const parts: string[] = [];
  let current = expression;
  while (ts.isPropertyAccessExpression(current)) {
    parts.unshift(current.name.text);
    current = current.expression;
  }
  return ts.isIdentifier(current) && current.text === "$" && parts.length !== 0 ? parts.join(".") : undefined;
}

function uniqueOwner(source: RecoverySource, identity: string): Owner {
  const owners = source.owners.get(identity) ?? [];
  if (owners.length === 0) throw new Pending("ownership_missing");
  const owner = owners[0]!;
  if (owners.length !== 1 || source.variables.get(owner.variable) !== 1) throw new Pending("ownership_ambiguous");
  if ((owner.statement.declarationList.flags & ts.NodeFlags.Const) === 0) throw new Pending("ownership_missing");
  return owner;
}

function sameOwner(accepted: Owner, working: RecoverySource): Owner {
  const owner = uniqueOwner(working, accepted.identity);
  if (owner.family !== accepted.family) throw new Pending("ownership_missing");
  if (owner.family === "use") {
    const patch = owner.call?.arguments[1];
    const basePatch = accepted.call?.arguments[1];
    if (patch === undefined || basePatch === undefined || patch.getText() !== basePatch.getText()) throw new Pending("ownership_missing");
  }
  return owner;
}

function requireIntact(node: ts.Node, source: RecoverySource): void {
  const start = node.getStart(source.file);
  if (node.end <= start || source.diagnostics.some((diagnostic) => {
    const at = diagnostic.start;
    return at >= start && at < node.end || at < start && at + (diagnostic.length ?? 0) > start;
  })) throw new Pending("ownership_missing");
}

function property(object: ts.Expression, name: string): ts.Expression | undefined {
  if (!ts.isObjectLiteralExpression(object)) throw new Pending("ownership_missing");
  const matches = object.properties.filter((entry) =>
    entry.name !== undefined && (ts.isIdentifier(entry.name) || ts.isStringLiteral(entry.name)) && entry.name.text === name);
  if (object.properties.some((entry) => ts.isSpreadAssignment(entry)
    || entry.name !== undefined && ts.isComputedPropertyName(entry.name))
    || matches.length > 1) throw new Pending("ownership_ambiguous");
  const match = matches[0];
  if (match === undefined) return undefined;
  if (!ts.isPropertyAssignment(match)) throw new Pending("unsupported_structure");
  return match.initializer;
}

function optionalPropertyPath(root: ts.Expression | undefined, path: readonly string[]): ts.Expression | undefined {
  let result = root;
  for (const name of path) if (result !== undefined) result = property(result, name);
  return result;
}

function valueAtPath(base: ts.Expression, current: ts.Expression, path: readonly SemanticPathSegment[], accepted: RecoverySource, working: RecoverySource): ts.Expression {
  let old = base;
  let value = current;
  for (const segment of path) {
    if (typeof segment === "string") {
      const oldProperty = property(old, segment);
      const currentProperty = property(value, segment);
      if (oldProperty === undefined || currentProperty === undefined) throw new Pending("ownership_missing");
      old = oldProperty;
      value = currentProperty;
    } else if (typeof segment === "number" && ts.isArrayLiteralExpression(old) && ts.isArrayLiteralExpression(value)) {
      if (value.elements.some(ts.isSpreadElement)) throw new Pending("ownership_ambiguous");
      const element = old.elements[segment];
      if (element === undefined) throw new Pending("ownership_missing");
      const oldKey = ts.isObjectLiteralExpression(element) ? property(element, "key") : undefined;
      if (oldKey !== undefined && ts.isStringLiteral(oldKey)) {
        const matches = value.elements.filter((entry) => {
          if (!ts.isObjectLiteralExpression(entry)) return false;
          const entryKey = property(entry, "key");
          return entryKey !== undefined && ts.isStringLiteral(entryKey) && entryKey.text === oldKey.text;
        });
        if (matches.length > 1) throw new Pending("ownership_ambiguous");
        if (matches[0] === undefined) throw new Pending("ownership_missing");
        value = matches[0];
      } else {
        if (old.elements.length !== value.elements.length) throw new Pending("unsupported_structure");
        const indexed = value.elements[segment];
        if (indexed === undefined || ts.isOmittedExpression(indexed)) throw new Pending("ownership_missing");
        value = indexed;
      }
      old = element;
    } else throw new Pending("unsupported_structure");
  }
  requireIntact(old, accepted);
  requireIntact(value, working);
  return value;
}

function metadataRoot(source: RecoverySource, target: ManagedMetadataTarget, accepted?: RecoverySource): ts.Expression | undefined {
  if (target.target === "document") return source.call.arguments.length === 2 ? source.call.arguments[0] : undefined;
  const owner = accepted === undefined ? uniqueOwner(source, target.declaration) : sameOwner(uniqueOwner(accepted, target.declaration), source);
  requireIntact(owner.statement, source);
  if (target.target === "parameter") {
    if (owner.family !== "parameter") throw new Pending("ownership_missing");
    return owner.call?.arguments[2];
  }
  return owner.family === "use" ? owner.call?.arguments[3] : owner.value;
}

/** Map only an unchanged complete lexical owner, never an unscoped text match. */
function mapUnchangedOwnedEdits(accepted: RecoverySource, working: RecoverySource, patch: ManagedSourcePatch, structural = false): readonly ManagedSourceEdit[] {
  // Lifecycle edits may affect declaration order and dependent references. A
  // draft change anywhere inside that body needs richer structural analysis.
  if (structural && accepted.body.getText(accepted.file) !== working.body.getText(working.file)) throw new Pending("unsupported_structure");
  const regions = structural ? [{
    start: accepted.body.getStart(accepted.file), end: accepted.body.end,
    working: (): ts.Node => working.body,
  }] : [...accepted.owners.values()].flat().map((owner) => ({
    start: owner.statement.getStart(accepted.file), end: owner.statement.end,
    working: (): ts.Node => sameOwner(owner, working).statement,
  }));
  const acceptedReturn = accepted.body.statements.filter(ts.isReturnStatement);
  const workingReturn = working.body.statements.filter(ts.isReturnStatement);
  if (!structural && acceptedReturn.length === 1 && workingReturn.length === 1) regions.push({
    start: acceptedReturn[0]!.getStart(accepted.file), end: acceptedReturn[0]!.end,
    working: () => workingReturn[0]!,
  });
  if (structural) {
    for (const oldImport of accepted.file.statements.filter(ts.isImportDeclaration)) {
      const matches = working.file.statements.filter(ts.isImportDeclaration)
        .filter((entry) => entry.getText(working.file) === oldImport.getText(accepted.file));
      if (matches.length === 1) regions.push({
        start: oldImport.getStart(accepted.file), end: oldImport.end,
        working: () => matches[0]!,
      });
    }
  }
  return patch.edits.map((edit) => {
    const matches = regions.filter((region) => edit.start >= region.start && edit.end <= region.end);
    if (matches.length !== 1) throw new Pending("unsupported_structure");
    const region = matches[0]!;
    const node = region.working();
    requireIntact(node, working);
    if (accepted.source.slice(region.start, region.end) !== node.getText(working.file)) throw new Pending("unsupported_structure");
    const delta = node.getStart(working.file) - region.start;
    return { ...edit, start: edit.start + delta, end: edit.end + delta };
  });
}
