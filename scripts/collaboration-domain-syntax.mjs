// SPDX-License-Identifier: GPL-3.0-or-later
// Compiler-admitted syntax observation only. Native inventory owns all values.
import { typescriptModuleUrl } from "./workspace-runtime-paths.mjs";
const { default: ts } = await import(typescriptModuleUrl);
const fail = (message) => { throw Object.assign(Error(message), { code: "reconciliation_conflict" }); };
export function sourceView(source) {
  const file = ts.createSourceFile("sketch.ts", source, ts.ScriptTarget.ES2022, true, ts.ScriptKind.TS);
  const call = file.statements.find(ts.isExportAssignment)?.expression;
  const body = ts.isCallExpression(call) && call.arguments.at(-1)?.body;
  if (!body || !ts.isBlock(body)) fail("Missing compiler-owned sketch callback");
  const owners = new Map();
  for (const statement of body.statements) {
    if (!ts.isVariableStatement(statement)) continue;
    const [binding] = statement.declarationList.declarations, initializer = binding.initializer;
    if (!initializer || !ts.isIdentifier(binding.name)) fail("Unsupported compiler statement owner");
    const variable = binding.name.text;
    const builder = ts.isCallExpression(initializer) && initializer.expression.getText(file).startsWith("$.");
    const declaration = builder && ts.isStringLiteral(initializer.arguments[0]) ? initializer.arguments[0].text : variable;
    const isParameter = builder && initializer.expression.getText(file) === "$.parameter";
    const value = builder ? initializer.arguments[isParameter ? 1 : initializer.expression.getText(file) === "$.use" ? 2 : 1] : initializer;
    if (owners.has(declaration)) fail("Ambiguous compiler source declaration");
    const start = ts.getLeadingCommentRanges(source, statement.pos)?.[0]?.pos ?? statement.getStart(file);
    const end = ts.getTrailingCommentRanges(source, statement.end)?.at(-1)?.end ?? statement.end;
    owners.set(declaration, { declaration, variable, value, statement, start, end, source: source.slice(start, end) });
  }
  return { source, file, call, body, owners };
}
function tokens(node, view) {
  const scanner = ts.createScanner(ts.ScriptTarget.ES2022, true, ts.LanguageVariant.Standard, node.getText(view.file)), out = [];
  for (let kind = scanner.scan(); kind !== ts.SyntaxKind.EndOfFileToken; kind = scanner.scan()) out.push([kind, scanner.getTokenText()]);
  return JSON.stringify(out);
}
function child(node, segment) {
  if (typeof segment === "string" && ts.isObjectLiteralExpression(node)) {
    const field = node.properties.find((item) => item.name && (ts.isIdentifier(item.name) || ts.isStringLiteral(item.name)) && item.name.text === segment);
    return field && (ts.isPropertyAssignment(field) ? field.initializer : ts.isShorthandPropertyAssignment(field) ? field.name : undefined);
  }
  if (ts.isArrayLiteralExpression(node)) {
    if (Number.isSafeInteger(segment)) return node.elements[segment];
    if (segment && typeof segment.member === "string") return node.elements.find((item) => {
      const key = child(item, "key"); return key && ts.isStringLiteral(key) && key.text === segment.member;
    });
  }
  return undefined;
}
export function capturedManagedPropertyTouches(basisSource, capturedSource, properties) {
  const before = sourceView(basisSource), after = sourceView(capturedSource), touched = [];
  for (const { declaration, path, value } of properties) {
    // Report only syntax containing a touched token, including its ancestors:
    // history may have recorded an earlier whole-point/array property write.
    // Unchanged siblings are never claimed.
    let a = before.owners.get(declaration)?.value, b = after.owners.get(declaration)?.value;
    for (const segment of path) { a = a && child(a, segment); b = b && child(b, segment); }
    if (a && b ? tokens(a, before) !== tokens(b, after) : Boolean(a) !== Boolean(b)) touched.push({ declaration, path });
  }
  return touched;
}
export function replaceOwnedStatements(source, replacements) {
  const view = sourceView(source), edits = [];
  for (const [declaration, replacement] of replacements) {
    const owner = view.owners.get(declaration); if (!owner) fail(`Missing restored source owner ${declaration}`);
    edits.push({ start: owner.start, end: owner.end, replacement });
  }
  for (const edit of edits.sort((a, b) => b.start - a.start)) source = source.slice(0, edit.start) + edit.replacement + source.slice(edit.end);
  return source;
}

export function capturedManagedMetadataTouches(basisSource, capturedSource, metadata) {
  const a = sourceView(basisSource), b = sourceView(capturedSource), result = [];
  const expression = (view, item) => {
    if (item.target.target === "document") {
      const options = view.call.arguments.length === 2 ? view.call.arguments[0] : undefined;
      return options && (item.property === "areKeyConstraintsByDefault" ? child(options, "dimensions") : options);
    }
    const owner = view.owners.get(item.target.declaration); if (!owner) return undefined;
    const initializer = owner.statement.declarationList.declarations[0].initializer;
    if (item.target.target === "parameter") return initializer.arguments[2];
    return ts.isCallExpression(initializer) && initializer.expression.getText(view.file) === "$.use" ? initializer.arguments[3] : owner.value;
  };
  for (const item of metadata) {
    const before = expression(a, item), after = expression(b, item), first = before && child(before, item.property), second = after && child(after, item.property);
    if (first && second ? tokens(first, a) !== tokens(second, b) : Boolean(first) !== Boolean(second)) result.push({ mutation: "set_metadata", target: item.target, property: item.property });
  }
  return result;
}
