// SPDX-License-Identifier: GPL-3.0-or-later

import ts from "typescript";

/** Offsets use JavaScript/editor UTF-16 code units, never UTF-8 byte offsets. */
export interface ManagedSourceEdit {
  readonly start: number;
  readonly end: number;
  readonly expected: string;
  readonly replacement: string;
}

/** Text authority only: this does not grant permission to publish a CAD edit. */
export interface ManagedSourcePatch {
  readonly baseSourceDigest: string;
  readonly candidateSourceDigest: string;
  readonly edits: readonly ManagedSourceEdit[];
}

interface Splice {
  readonly start: number;
  readonly end: number;
  readonly replacement: string;
}

/**
 * Project an already validated semantic mutation onto its original syntax.
 * AST ownership supplies stable statement/property matches; this is not a
 * whole-file character diff. Comments and trivia on retained nodes are untouched.
 */
export function localizedManagedSourceEdits(
  source: string,
  candidate: string,
): readonly ManagedSourceEdit[] {
  const before = ts.createSourceFile("sketch.ts", source, ts.ScriptTarget.ES2022, true, ts.ScriptKind.TS);
  const after = ts.createSourceFile("sketch.ts", candidate, ts.ScriptTarget.ES2022, true, ts.ScriptKind.TS);
  const edits: Splice[] = [];
  const newline = source.includes("\r\n") ? "\r\n" : "\n";
  const add = (start: number, end: number, replacement: string): void => {
    if (source.slice(start, end) !== replacement) edits.push({ start, end, replacement });
  };
  const replacement = (node: ts.Node): string => node.getText(after).replaceAll("\n", newline);
  const children = (node: ts.Node): ts.Node[] => {
    const result: ts.Node[] = [];
    ts.forEachChild(node, (child) => { result.push(child); });
    return result;
  };
  const ownedStart = (node: ts.Node): number =>
    ts.getLeadingCommentRanges(source, node.pos)?.[0]?.pos ?? node.getStart(before);
  const ownedEnd = (node: ts.Node): number =>
    ts.getTrailingCommentRanges(source, node.end)?.at(-1)?.end ?? node.end;
  const key = (node: ts.Node): string => {
    if (ts.isVariableStatement(node)) return `variable:${node.declarationList.declarations[0]!.name.getText(node.getSourceFile())}`;
    if (ts.isReturnStatement(node)) return "return";
    if (ts.isArrowFunction(node)) return "callback";
    if (ts.isPropertyAssignment(node) || ts.isShorthandPropertyAssignment(node)) {
      return `property:${ts.isStringLiteral(node.name) ? node.name.text : node.name.getText(node.getSourceFile())}`;
    }
    if (ts.isExpressionStatement(node) && ts.isCallExpression(node.expression)) {
      const call = node.expression;
      const method = call.expression.getText(node.getSourceFile());
      if (method === "$.group" && ts.isStringLiteral(call.arguments[0]!)) return `group:${call.arguments[0].text}`;
    }
    // Token values ignore quote style and trivia without interpreting any code.
    const scanner = ts.createScanner(ts.ScriptTarget.ES2022, true, ts.LanguageVariant.Standard, node.getText(node.getSourceFile()));
    const tokens: string[] = [];
    for (let token = scanner.scan(); token !== ts.SyntaxKind.EndOfFileToken; token = scanner.scan()) {
      tokens.push(`${token}:${scanner.getTokenValue() || scanner.getTokenText()}`);
    }
    return tokens.join("|");
  };
  const commaAfter = (node: ts.Node, limit: number): Splice | undefined => {
    const scanner = ts.createScanner(ts.ScriptTarget.ES2022, true, ts.LanguageVariant.Standard, source.slice(node.end, limit));
    if (scanner.scan() !== ts.SyntaxKind.CommaToken) return undefined;
    return { start: node.end + scanner.getTokenPos(), end: node.end + scanner.getTextPos(), replacement: "" };
  };
  const diffList = (
    oldNodes: readonly ts.Node[],
    newNodes: readonly ts.Node[],
    close: number,
    commaSeparated: boolean,
  ): void => {
    const oldKeys = occurrenceKeys(oldNodes.map(key));
    const newKeys = occurrenceKeys(newNodes.map(key));
    const oldByKey = new Map(oldKeys.map((value, index) => [value, index]));
    const retained = increasingMatches(newKeys.map((value) => oldByKey.get(value)));
    const keptOld = new Set([...retained].map((index) => oldByKey.get(newKeys[index]!)!));
    let lastKept = -1;
    for (const index of keptOld) if (index > lastKept) lastKept = index;
    for (const [index, node] of oldNodes.entries()) {
      if (keptOld.has(index)) continue;
      // A moved statement brings its own leading/trailing comments. Deletion
      // uses the same ownership, never a line-based range covering its neighbor.
      add(commaSeparated ? node.getStart(before) : ownedStart(node), commaSeparated ? node.end : ownedEnd(node), "");
      if (commaSeparated) {
        const comma = commaAfter(node, oldNodes[index + 1]?.getStart(before) ?? close);
        if (comma !== undefined) add(comma.start, comma.end, "");
      }
    }
    let pending: string[] = [];
    const emit = (anchor: number, atEnd: boolean): void => {
      if (pending.length === 0) return;
      if (commaSeparated) {
        const lastNode = lastKept < 0 ? undefined : oldNodes[lastKept];
        const needsComma = atEnd && lastNode !== undefined && commaAfter(lastNode, oldNodes[lastKept + 1]?.getStart(before) ?? close) === undefined;
        add(anchor, anchor, `${needsComma ? ", " : ""}${pending.join(", ")}${atEnd ? "" : ", "}`);
      } else {
        const lineStart = source.lastIndexOf("\n", anchor - 1) + 1;
        const prefix = source.slice(lineStart, anchor);
        const indent = /^[\t ]*$/u.test(prefix) ? prefix : "  ";
        add(anchor, anchor, `${pending.join(`${newline}${indent}`)}${newline}${indent}`);
      }
      pending = [];
    };
    for (const [index, node] of newNodes.entries()) {
      const oldIndex = oldByKey.get(newKeys[index]!);
      if (retained.has(index)) {
        const old = oldNodes[oldIndex!]!;
        emit(commaSeparated ? old.getStart(before) : ownedStart(old), false);
        diff(old, node);
      } else if (oldIndex !== undefined) {
        const old = oldNodes[oldIndex]!;
        pending.push(source.slice(commaSeparated ? old.getStart(before) : ownedStart(old), commaSeparated ? old.end : ownedEnd(old)));
      } else {
        pending.push(replacement(node));
      }
    }
    emit(close, true);
  };
  const diff = (old: ts.Node, next: ts.Node): void => {
    if (ts.isCallExpression(next) && next.expression.getText(after) === "$.parameter"
      && next.arguments[1] !== undefined && key(old) === key(next.arguments[1])) {
      // Extraction wraps the scalar without rewriting spelling or unit comments.
      add(old.getStart(before), old.getStart(before), `$.parameter(${replacement(next.arguments[0]!)}, `);
      add(old.end, old.end, `${next.arguments[2] === undefined ? "" : `, ${replacement(next.arguments[2])}`})`);
      return;
    }
    if (ts.isObjectLiteralExpression(old) && ts.isObjectLiteralExpression(next)) {
      diffList(old.properties, next.properties, old.end - 1, true);
      return;
    }
    if (ts.isBlock(old) && ts.isBlock(next)) {
      diffList(old.statements, next.statements, old.end - 1, false);
      return;
    }
    if (ts.isNamedImports(old) && ts.isNamedImports(next)) {
      diffList(old.elements, next.elements, old.end - 1, true);
      return;
    }
    if (ts.isArrayLiteralExpression(old) && ts.isArrayLiteralExpression(next)) {
      if (old.elements.length === next.elements.length) {
        old.elements.forEach((node, index) => diff(node, next.elements[index]!));
      } else {
        diffList(old.elements, next.elements, old.end - 1, true);
      }
      return;
    }
    if (ts.isCallExpression(old) && ts.isCallExpression(next)) {
      diff(old.expression, next.expression);
      if (old.arguments.length === next.arguments.length) {
        old.arguments.forEach((node, index) => diff(node, next.arguments[index]!));
      } else {
        diffList(old.arguments, next.arguments, old.end - 1, true);
      }
      return;
    }
    if (ts.isShorthandPropertyAssignment(old) && ts.isPropertyAssignment(next)
      && ts.isIdentifier(next.initializer) && old.name.text === next.initializer.text) return;
    if (old.kind !== next.kind) {
      add(old.getStart(before), old.end, replacement(next));
      return;
    }
    const oldChildren = children(old);
    const newChildren = children(next);
    if (oldChildren.length !== newChildren.length) {
      // Only expression-local structural replacements are allowed here. The
      // envelope and declaration lists must always go through owned slots.
      if (ts.isSourceFile(old) || ts.isStatement(old)) throw new TypeError("unsupported localized managed source structure");
      add(old.getStart(before), old.end, replacement(next));
    } else if (oldChildren.length !== 0) {
      oldChildren.forEach((node, index) => diff(node, newChildren[index]!));
    } else {
      const sameLiteral = ts.isStringLiteral(old) && ts.isStringLiteral(next) && old.text === next.text
        || ts.isNumericLiteral(old) && ts.isNumericLiteral(next) && Number(old.text) === Number(next.text);
      if (!sameLiteral && old.getText(before) !== next.getText(after)) add(old.getStart(before), old.end, replacement(next));
    }
  };
  diff(before, after);
  // Merge touching splices into one exact check, including inserts at a removed
  // node's boundary. Original offsets always address the unchanged input.
  edits.sort((left, right) => left.start - right.start || left.end - right.end);
  const merged: Splice[] = [];
  for (const edit of edits) {
    const previous = merged.at(-1);
    if (previous !== undefined && edit.start < previous.end) throw new TypeError("overlapping localized managed source edits");
    if (previous !== undefined && edit.start === previous.end) {
      merged[merged.length - 1] = { start: previous.start, end: edit.end, replacement: previous.replacement + edit.replacement };
    } else merged.push(edit);
  }
  return merged.map((edit) => ({ ...edit, expected: source.slice(edit.start, edit.end) }));
}

function occurrenceKeys(keys: readonly string[]): string[] {
  const counts = new Map<string, number>();
  return keys.map((key) => {
    const count = counts.get(key) ?? 0;
    counts.set(key, count + 1);
    return JSON.stringify([key, count]);
  });
}

/** O(n log n) stable matching, bounded by the compiler's statement/text limits. */
function increasingMatches(indexes: readonly (number | undefined)[]): Set<number> {
  const tails: number[] = [];
  const previous = new Map<number, number>();
  indexes.forEach((value, index) => {
    if (value === undefined) return;
    let low = 0;
    let high = tails.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (indexes[tails[middle]!]! < value) low = middle + 1;
      else high = middle;
    }
    if (low > 0) previous.set(index, tails[low - 1]!);
    tails[low] = index;
  });
  const result = new Set<number>();
  for (let index = tails.at(-1); index !== undefined; index = previous.get(index)) result.add(index);
  return result;
}
