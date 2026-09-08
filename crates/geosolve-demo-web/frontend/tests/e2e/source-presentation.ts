// SPDX-License-Identifier: GPL-3.0-or-later
import ts from "typescript";

/** Read the authored display label independently of the browser projection. */
export function authoredLabel(source: string, symbol: string): string {
  const file = ts.createSourceFile("sketch.ts", source, ts.ScriptTarget.Latest, true);
  let label: string | undefined;
  let found = false;
  const visit = (node: ts.Node) => {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.name.text === symbol) {
      found = true;
      const call = node.initializer;
      if (call && ts.isCallExpression(call)) {
        const method = ts.isPropertyAccessExpression(call.expression) ? call.expression.name.text : "";
        const options = call.arguments[method === "parameter" ? 2 : method === "use" ? 3 : 1];
        if (options && ts.isObjectLiteralExpression(options)) {
          for (const property of options.properties) {
            if (ts.isPropertyAssignment(property) && (ts.isIdentifier(property.name) || ts.isStringLiteral(property.name)) && property.name.text === "label" && ts.isStringLiteral(property.initializer)) {
              label = property.initializer.text;
            }
          }
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  if (!found) throw Error(`Missing source declaration ${symbol}`);
  return label || symbol;
}
