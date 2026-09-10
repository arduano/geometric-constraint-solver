// SPDX-License-Identifier: GPL-3.0-or-later

import ts from "typescript";
import type { CompiledManagedSource } from "./managed.js";
import type { ManagedSourceEdit, ManagedSourcePatch } from "./source-patch.js";

export interface CapturedManagedSourceRebaseInput {
  readonly basis: { readonly source: string; readonly compiled: CompiledManagedSource };
  readonly capturedSource: string;
  readonly latest: { readonly source: string; readonly compiled: CompiledManagedSource };
  /** Trusted host lifetime-ledger findings, never inferred from equal names. */
  readonly invalidatedDeclarations?: readonly string[];
}

export interface CapturedManagedSourceRebase {
  readonly source: string;
  readonly compiled: CompiledManagedSource;
  readonly patch: ManagedSourcePatch;
  readonly basisSourceDigest: string;
  readonly capturedSourceDigest: string;
  readonly latestSourceDigest: string;
  /** Host must authenticate each capture-basis generation before publication. */
  readonly requiredStableDeclarations: readonly string[];
}

export class CapturedSourceRebaseConflict extends Error {
  constructor(message: string) { super(message); this.name = "CapturedSourceRebaseConflict"; }
}

export function capturedManagedDeclarationChanges(basisSource: string, capturedSource: string): readonly string[] {
  const basis = view(basisSource);
  const captured = view(capturedSource);
  return [...basis.owners].flatMap(([identity, owner]) => {
    const next = captured.owners.get(identity);
    return next === undefined || owner.family !== next.family || signature(owner.initializer) !== signature(next.initializer) ? [identity] : [];
  });
}

interface View {
  readonly source: string;
  readonly file: ts.SourceFile;
  readonly call: ts.CallExpression;
  readonly statements: readonly ts.Statement[];
  readonly owners: ReadonlyMap<string, Owner>;
}

interface Owner {
  readonly identity: string;
  readonly variable: string;
  readonly family: string;
  readonly statement: ts.VariableStatement;
  readonly initializer: ts.Expression;
}

interface SliceEdit { readonly start: number; readonly end: number; readonly replacement: string }

function conflict(message: string): never { throw new CapturedSourceRebaseConflict(message); }

/** Compiler-admitted syntax only. All edits address complete known owners. */
export function rebaseCapturedSourceEdits(
  basisSource: string,
  capturedSource: string,
  latestSource: string,
  invalidated: ReadonlySet<string>,
): { readonly edits: readonly ManagedSourceEdit[]; readonly requiredStableDeclarations: readonly string[] } {
  const basis = view(basisSource);
  const captured = view(capturedSource);
  const latest = view(latestSource);
  const required = new Set<string>();
  const edits: SliceEdit[] = [];
  // Captured lifecycle/import/output changes need separate dependency analysis.
  // A latest-only lifecycle change is safe if each edited stable owner survives.
  if (structuralSignature(basis) !== structuralSignature(captured)) conflict("captured declaration order, imports, references or lifecycle changed concurrently");
  for (const [identity, old] of basis.owners) {
    const capture = captured.owners.get(identity)!;
    const changed = signature(old.initializer) !== signature(capture.initializer);
    if (changed) {
      required.add(identity);
      if (invalidated.has(identity)) conflict(`captured declaration ${identity} has a changed lifetime`);
    }
    const current = latest.owners.get(identity);
    if (current === undefined) {
      if (changed) conflict(`captured declaration ${identity} was deleted or renamed`);
      continue;
    }
    if (current.family !== old.family || current.variable !== old.variable) {
      if (changed) conflict(`captured declaration ${identity} changed its family or lexical binding`);
      continue;
    }
    mergeTrailing(old.statement, capture.statement, current.statement, basis, captured, latest, edits);
    if (rawOwned(old.statement, basis) === rawOwned(capture.statement, captured)) continue;
    const text = mergeNode(old.initializer, capture.initializer, current.initializer, basis, captured, latest);
    const captureStart = capture.statement.getStart(captured.file);
    let statement = captured.source.slice(captureStart, capture.statement.end);
    statement = splice(statement, [{
      start: capture.initializer.getStart(captured.file) - captureStart,
      end: capture.initializer.end - captureStart, replacement: text,
    }]);
    // Prefer captured leading comments only if they actually changed at capture.
    const oldLeading = leading(old.statement, basis);
    const capturedLeading = leading(capture.statement, captured);
    const currentLeading = leading(current.statement, latest);
    if (oldLeading.text !== capturedLeading.text) {
      statement = capturedLeading.text + statement;
      edits.push({ start: currentLeading.start, end: current.statement.end, replacement: statement });
    } else {
      edits.push({ start: current.statement.getStart(latest.file), end: current.statement.end, replacement: statement });
    }
  }
  // Comments on imports, groups and the output have exact lexical owners too.
  // A changed output's formatting cannot overwrite latest output membership.
  const plainBasis = [...basis.file.statements.filter((node) => !ts.isExportAssignment(node)), ...basis.statements.filter((node) => !ts.isVariableStatement(node))];
  const plainCapture = [...captured.file.statements.filter((node) => !ts.isExportAssignment(node)), ...captured.statements.filter((node) => !ts.isVariableStatement(node))];
  const plainLatest = [...latest.file.statements.filter((node) => !ts.isExportAssignment(node)), ...latest.statements.filter((node) => !ts.isVariableStatement(node))];
  for (const [index, old] of plainBasis.entries()) {
    const capture = plainCapture[index]!;
    const changedText = rawOwned(old, basis) !== rawOwned(capture, captured);
    const changedTrailing = trailing(old, basis).text !== trailing(capture, captured).text;
    if (!changedText && !changedTrailing) continue;
    const matches = plainLatest.filter((node) => signature(node) === signature(old));
    if (matches.length !== 1) conflict("captured comments or formatting lost their unique statement owner");
    const current = matches[0]!;
    if (changedText) edits.push({ start: leading(current, latest).start, end: current.end, replacement: rawOwned(capture, captured) });
    mergeTrailing(old, capture, current, basis, captured, latest, edits);
  }
  const oldExport = basis.file.statements.find(ts.isExportAssignment)!;
  const captureExport = captured.file.statements.find(ts.isExportAssignment)!;
  const latestExport = latest.file.statements.find(ts.isExportAssignment)!;
  const oldExportLeading = leading(oldExport, basis);
  const captureExportLeading = leading(captureExport, captured);
  const latestExportLeading = leading(latestExport, latest);
  if (oldExportLeading.text !== captureExportLeading.text) edits.push({
    start: latestExportLeading.start, end: latestExport.getStart(latest.file), replacement: captureExportLeading.text,
  });
  const oldFooter = basis.source.slice(oldExport.end);
  const captureFooter = captured.source.slice(captureExport.end);
  if (oldFooter !== captureFooter) edits.push({ start: latestExport.end, end: latest.source.length, replacement: captureFooter });
  const oldDocument = basis.call.arguments.length === 2 ? basis.call.arguments[0] : undefined;
  const captureDocument = captured.call.arguments.length === 2 ? captured.call.arguments[0] : undefined;
  const latestDocument = latest.call.arguments.length === 2 ? latest.call.arguments[0] : undefined;
  if (oldDocument !== undefined && captureDocument !== undefined && latestDocument !== undefined) {
    if (oldDocument.getText(basis.file) !== captureDocument.getText(captured.file)) edits.push({
      start: latestDocument.getStart(latest.file), end: latestDocument.end,
      replacement: mergeNode(oldDocument, captureDocument, latestDocument, basis, captured, latest),
    });
  } else if (signatureOptional(oldDocument) !== signatureOptional(captureDocument)) {
    // Options added/removed as a whole need an exact latest envelope owner.
    if (signatureOptional(oldDocument) !== signatureOptional(latestDocument)) conflict("document options structure changed concurrently");
    if (latestDocument !== undefined) {
      const callback = latest.call.arguments.at(-1)!;
      edits.push({ start: latestDocument.getStart(latest.file), end: callback.getStart(latest.file), replacement: captureDocument === undefined ? "" : `${captureDocument.getText(captured.file)}, ` });
    } else if (captureDocument !== undefined) {
      const callback = latest.call.arguments.at(-1)!;
      edits.push({ start: callback.getStart(latest.file), end: callback.getStart(latest.file), replacement: `${captureDocument.getText(captured.file)}, ` });
    }
  }
  edits.sort((a, b) => a.start - b.start || a.end - b.end);
  if (edits.length > 1_024) conflict("captured Apply exceeds the localized edit bound");
  let end = -1;
  const result = edits.flatMap((edit) => {
    if (edit.start < end) conflict("captured Apply has overlapping lexical owners");
    end = edit.end;
    const expected = latestSource.slice(edit.start, edit.end);
    return expected === edit.replacement ? [] : [{ ...edit, expected }];
  });
  return { edits: result, requiredStableDeclarations: [...required] };
}

function view(source: string): View {
  const file = ts.createSourceFile("sketch.ts", source, ts.ScriptTarget.ES2022, true, ts.ScriptKind.TS);
  const call = file.statements.find(ts.isExportAssignment)!.expression as ts.CallExpression;
  const callback = call.arguments.at(-1) as ts.ArrowFunction;
  const statements = (callback.body as ts.Block).statements;
  const owners = new Map<string, Owner>();
  for (const statement of statements) {
    if (!ts.isVariableStatement(statement)) continue;
    const declaration = statement.declarationList.declarations[0]!;
    const initializer = declaration.initializer!;
    const variable = (declaration.name as ts.Identifier).text;
    const parts: string[] = [];
    let expression: ts.Expression = ts.isCallExpression(initializer) ? initializer.expression : initializer;
    while (ts.isPropertyAccessExpression(expression)) { parts.unshift(expression.name.text); expression = expression.expression; }
    let family = ts.isIdentifier(expression) && expression.text === "$" ? parts.join(".") : "binding";
    const identity = family === "binding" ? variable : ((initializer as ts.CallExpression).arguments[0] as ts.StringLiteral).text;
    if (family === "use") family += `:${(initializer as ts.CallExpression).arguments[1]!.getText(file)}`;
    if (owners.has(identity)) conflict(`ambiguous captured declaration ${identity}`);
    owners.set(identity, { identity, variable, family, statement, initializer });
  }
  return { source, file, call, statements, owners };
}

function structuralSignature(source: View): string {
  const byStatement = new Map([...source.owners.values()].map((owner) => [owner.statement, owner]));
  const variables = new Set([...source.owners.values()].map((owner) => owner.variable));
  return JSON.stringify([
    source.file.statements.filter(ts.isImportDeclaration).map(signature),
    source.statements.map((statement) => {
      if (!ts.isVariableStatement(statement)) return signature(statement);
      const owner = byStatement.get(statement)!;
      const references: string[] = [];
      const visit = (node: ts.Node): void => {
        if (ts.isIdentifier(node) && (ts.isPropertyAssignment(node.parent) ? node.parent.initializer === node : true)
          && variables.has(node.text)) references.push(node.text);
        ts.forEachChild(node, visit);
      };
      visit(owner.initializer);
      return [owner.identity, owner.variable, owner.family, references];
    }),
  ]);
}

function signatureOptional(node: ts.Node | undefined): string { return node === undefined ? "absent" : signature(node); }

function signature(node: ts.Node): string {
  if (ts.isIdentifier(node) || ts.isStringLiteral(node)) return JSON.stringify([node.kind, node.text]);
  if (ts.isNumericLiteral(node)) return JSON.stringify([node.kind, Number(node.text)]);
  if (ts.isShorthandPropertyAssignment(node)) return JSON.stringify([ts.SyntaxKind.PropertyAssignment, node.name.text, signature(node.name)]);
  if (ts.isPropertyAssignment(node) && (ts.isIdentifier(node.name) || ts.isStringLiteral(node.name))) {
    return JSON.stringify([node.kind, node.name.text, signature(node.initializer)]);
  }
  const children: string[] = [];
  ts.forEachChild(node, (child) => { children.push(signature(child)); });
  return JSON.stringify([node.kind, ts.isPrefixUnaryExpression(node) ? node.operator : null, children]);
}

function rawOwned(node: ts.Node, source: View): string { return source.source.slice(leading(node, source).start, node.end); }

function leading(node: ts.Node, source: View): { readonly start: number; readonly text: string } {
  const start = ts.getLeadingCommentRanges(source.source, node.pos)?.[0]?.pos ?? node.getStart(source.file);
  return { start, text: source.source.slice(start, node.getStart(source.file)) };
}

function trailing(node: ts.Node, source: View): { readonly end: number; readonly text: string; readonly lineComment: boolean } {
  const ranges = ts.getTrailingCommentRanges(source.source, node.end) ?? [];
  const end = ranges.at(-1)?.end ?? node.end;
  return { end, text: source.source.slice(node.end, end), lineComment: ranges.at(-1)?.kind === ts.SyntaxKind.SingleLineCommentTrivia };
}

function mergeTrailing(old: ts.Node, capture: ts.Node, current: ts.Node, basis: View, captured: View, latest: View, edits: SliceEdit[]): void {
  const before = trailing(old, basis);
  const changed = trailing(capture, captured);
  if (before.text === changed.text) return;
  const now = trailing(current, latest);
  const newline = latest.source.includes("\r\n") ? "\r\n" : "\n";
  const needsNewline = changed.lineComment && !/^[\t ]*\r?\n/u.test(latest.source.slice(now.end));
  edits.push({ start: current.end, end: now.end, replacement: changed.text + (needsNewline ? newline : "") });
}

function mergeNode(old: ts.Expression, capture: ts.Expression, current: ts.Expression, basis: View, captured: View, latest: View): string {
  const oldSignature = signature(old);
  const captureSignature = signature(capture);
  const currentSignature = signature(current);
  if (currentSignature === oldSignature || currentSignature === captureSignature) return capture.getText(captured.file);
  if (ts.isObjectLiteralExpression(old) && ts.isObjectLiteralExpression(capture) && ts.isObjectLiteralExpression(current)) {
    return mergeObject(old, capture, current, basis, captured, latest);
  }
  if (ts.isArrayLiteralExpression(old) && ts.isArrayLiteralExpression(capture) && ts.isArrayLiteralExpression(current)) {
    if (old.elements.length !== capture.elements.length || old.elements.length !== current.elements.length) {
      if (captureSignature === oldSignature) return current.getText(latest.file);
      return conflict("array structure changed while a captured property was edited");
    }
    const keys = (array: ts.ArrayLiteralExpression): string => JSON.stringify(array.elements.map((node) => {
      if (!ts.isObjectLiteralExpression(node)) return undefined;
      const key = fields(node).get("key");
      return key === undefined ? undefined : signature(fieldValue(key));
    }));
    if (keys(old) !== keys(capture) || keys(old) !== keys(current)) {
      if (captureSignature === oldSignature) return current.getText(latest.file);
      return conflict("keyed array ownership changed while a captured property was edited");
    }
    return replaceChildren(capture, captured, capture.elements.map((node, index) => ({
      node, text: mergeNode(old.elements[index]!, node, current.elements[index]!, basis, captured, latest),
    })));
  }
  if (ts.isCallExpression(old) && ts.isCallExpression(capture) && ts.isCallExpression(current)
    && signature(old.expression) === signature(capture.expression) && signature(old.expression) === signature(current.expression)) {
    if (old.arguments.length !== capture.arguments.length || old.arguments.length !== current.arguments.length) {
      if (captureSignature === oldSignature) return current.getText(latest.file);
      return conflict("call arguments changed while a captured property was edited");
    }
    return replaceChildren(capture, captured, capture.arguments.map((node, index) => ({
      node, text: mergeNode(old.arguments[index]!, node, current.arguments[index]!, basis, captured, latest),
    })));
  }
  // At the same identified leaf, later captured Apply wins. An unchanged
  // captured leaf carries the latest accepted value instead.
  return captureSignature === oldSignature ? current.getText(latest.file) : capture.getText(captured.file);
}

function fields(node: ts.ObjectLiteralExpression): Map<string, ts.PropertyAssignment | ts.ShorthandPropertyAssignment> {
  const result = new Map<string, ts.PropertyAssignment | ts.ShorthandPropertyAssignment>();
  for (const entry of node.properties) {
    if (!(ts.isPropertyAssignment(entry) || ts.isShorthandPropertyAssignment(entry))
      || !(ts.isIdentifier(entry.name) || ts.isStringLiteral(entry.name))) conflict("unsupported captured object ownership");
    const name = entry.name.text;
    if (result.has(name)) conflict(`duplicate captured field ${name}`);
    result.set(name, entry);
  }
  return result;
}

function fieldValue(node: ts.PropertyAssignment | ts.ShorthandPropertyAssignment): ts.Expression {
  return ts.isPropertyAssignment(node) ? node.initializer : node.name;
}

function mergeObject(old: ts.ObjectLiteralExpression, capture: ts.ObjectLiteralExpression, current: ts.ObjectLiteralExpression, basis: View, captured: View, latest: View): string {
  const oldFields = fields(old);
  const captureFields = fields(capture);
  const currentFields = fields(current);
  const edits: SliceEdit[] = [];
  const start = capture.getStart(captured.file);
  for (const [name, node] of captureFields) {
    const base = oldFields.get(name);
    const now = currentFields.get(name);
    if (base === undefined) continue; // A newly captured property wins by name.
    if (now === undefined) {
      if (signature(fieldValue(base)) !== signature(fieldValue(node))) conflict(`captured property ${name} was removed`);
      const index = capture.properties.indexOf(node);
      const limit = capture.properties[index + 1]?.getStart(captured.file) ?? capture.end - 1;
      const scanner = ts.createScanner(ts.ScriptTarget.ES2022, true, ts.LanguageVariant.Standard, captured.source.slice(node.end, limit));
      const commaEnd = scanner.scan() === ts.SyntaxKind.CommaToken ? node.end + scanner.getTextPos() : node.end;
      edits.push({ start: node.getStart(captured.file) - start, end: commaEnd - start, replacement: "" });
      continue;
    }
    const value = fieldValue(node);
    edits.push({ start: value.getStart(captured.file) - start, end: value.end - start,
      replacement: mergeNode(fieldValue(base), value, fieldValue(now), basis, captured, latest) });
  }
  const added = [...currentFields].filter(([name]) => !oldFields.has(name) && !captureFields.has(name));
  if (added.length !== 0) {
    const kept = [...captureFields].filter(([name]) => !oldFields.has(name) || currentFields.has(name));
    const last = kept.at(-1)?.[1];
    const scanner = last === undefined ? undefined : ts.createScanner(ts.ScriptTarget.ES2022, true, ts.LanguageVariant.Standard, captured.source.slice(last.end, capture.end - 1));
    const hasComma = scanner?.scan() === ts.SyntaxKind.CommaToken;
    edits.push({ start: capture.end - 1 - start, end: capture.end - 1 - start,
      replacement: `${last === undefined || hasComma ? "" : ", "}${added.map(([, node]) => node.getText(latest.file)).join(", ")}` });
  }
  return splice(capture.getText(captured.file), edits);
}

function replaceChildren(node: ts.Node, source: View, children: readonly { readonly node: ts.Node; readonly text: string }[]): string {
  const start = node.getStart(source.file);
  return splice(node.getText(source.file), children.map((child) => ({
    start: child.node.getStart(source.file) - start, end: child.node.end - start, replacement: child.text,
  })));
}

function splice(source: string, edits: readonly SliceEdit[]): string {
  const chunks: string[] = [];
  let offset = 0;
  for (const edit of [...edits].sort((a, b) => a.start - b.start || a.end - b.end)) {
    if (edit.start < offset) conflict("overlapping captured property ownership");
    chunks.push(source.slice(offset, edit.start), edit.replacement);
    offset = edit.end;
  }
  chunks.push(source.slice(offset));
  return chunks.join("");
}
