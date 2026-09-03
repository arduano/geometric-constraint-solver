// SPDX-License-Identifier: GPL-3.0-or-later

import {
  autocompletion,
  pickedCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import { StateEffect, StateField, type Extension } from "@codemirror/state";
import {
  diagnosticCount,
  forceLinting,
  lintGutter,
  linter,
  lintKeymap,
  openLintPanel,
  type Diagnostic,
} from "@codemirror/lint";
import {
  EditorView,
  hoverTooltip,
  keymap,
  showTooltip,
  ViewPlugin,
  type Tooltip,
  type ViewUpdate,
} from "@codemirror/view";
import type { TypeScriptLanguageWorkerClient } from "./client";
import {
  TYPESCRIPT_LANGUAGE_VERSION,
  type TypeScriptLanguageDiagnostic,
  type TypeScriptLanguageHover,
  type TypeScriptLanguageSignature,
} from "./protocol";

export type TypeScriptLanguageStatus =
  | { state: "checking"; version: string; diagnostics: number; errors: number; warnings: number }
  | { state: "ready"; version: string; diagnostics: number; errors: number; warnings: number }
  | { state: "error"; version: string; diagnostics: number; errors: number; warnings: number; message: string };

interface LanguageExtensionOptions {
  client: TypeScriptLanguageWorkerClient;
  synchronize(view: EditorView): void;
  report(status: TypeScriptLanguageStatus): void;
}

const setSignature = StateEffect.define<TypeScriptLanguageSignature | null>();

const signatureField = StateField.define<TypeScriptLanguageSignature | null>({
  create: () => null,
  update(value, transaction) {
    if (transaction.docChanged) value = null;
    for (const effect of transaction.effects) {
      if (effect.is(setSignature)) value = effect.value;
    }
    return value;
  },
  provide: (field) => showTooltip.from(
    field,
    (signature) => signature ? signatureTooltip(signature) : null,
  ),
});

export function typeScriptLanguageExtensions(options: LanguageExtensionOptions): Extension[] {
  const diagnostics = linter(async (view) => {
    options.synchronize(view);
    options.report(emptyStatus("checking"));
    try {
      const result = await options.client.diagnostics();
      if (!result) return [];
      options.report(statusForDiagnostics(result));
      return result.map(codeMirrorDiagnostic);
    } catch (error) {
      options.report({
        state: "error",
        version: TYPESCRIPT_LANGUAGE_VERSION,
        diagnostics: 0,
        errors: 0,
        warnings: 0,
        message: error instanceof Error ? error.message : String(error),
      });
      return [];
    }
  }, { delay: 350 });

  const signaturePlugin = ViewPlugin.fromClass(class {
    private timeout: ReturnType<typeof setTimeout> | undefined;
    private generation = 0;
    private active = false;

    constructor(private readonly view: EditorView) {}

    update(update: ViewUpdate) {
      if (update.transactions.some((transaction) => transaction.effects.some(
        (effect) => effect.is(setSignature) && effect.value === null,
      ))) this.active = false;
      if (update.selectionSet && !update.docChanged) {
        this.active = false;
        this.clear();
        return;
      }
      if (!update.docChanged) return;

      const inserted = insertedText(update);
      const hasTrigger = /[,(]/u.test(inserted);
      const wasVisible = update.startState.field(signatureField) !== null;
      this.active = hasTrigger || ((this.active || wasVisible) && inserted.length > 0);
      if (this.active) this.schedule();
      else this.clear();
    }

    destroy() {
      this.generation += 1;
      if (this.timeout !== undefined) clearTimeout(this.timeout);
    }

    private schedule() {
      this.generation += 1;
      const generation = this.generation;
      if (this.timeout !== undefined) clearTimeout(this.timeout);
      const selection = this.view.state.selection.main;
      if (!selection.empty) {
        queueMicrotask(() => {
          if (generation === this.generation) {
            this.view.dispatch({ effects: setSignature.of(null) });
          }
        });
        return;
      }
      this.timeout = setTimeout(() => {
        options.synchronize(this.view);
        const position = this.view.state.selection.main.head;
        const document = this.view.state.doc.toString();
        void options.client.signature(position).then((signature) => {
          if (
            generation !== this.generation
            || document !== this.view.state.doc.toString()
            || position !== this.view.state.selection.main.head
          ) return;
          if (!signature) this.active = false;
          this.view.dispatch({ effects: setSignature.of(signature) });
        }).catch(() => {
          if (generation === this.generation) {
            this.view.dispatch({ effects: setSignature.of(null) });
          }
        });
      }, 90);
    }

    private clear() {
      this.generation += 1;
      const generation = this.generation;
      if (this.timeout !== undefined) clearTimeout(this.timeout);
      queueMicrotask(() => {
        if (generation === this.generation && this.view.state.field(signatureField) !== null) {
          this.view.dispatch({ effects: setSignature.of(null) });
        }
      });
    }
  });

  return [
    diagnostics,
    lintGutter(),
    keymap.of(lintKeymap),
    autocompletion({
      activateOnTyping: true,
      override: [(context) => completionSource(options, context)],
    }),
    hoverTooltip(async (view, position) => {
      options.synchronize(view);
      try {
        const hover = await options.client.hover(position);
        return hover ? hoverDescription(hover) : null;
      } catch {
        return null;
      }
    }, { hoverTime: 350 }),
    signatureField,
    signaturePlugin,
    keymap.of([{
      key: "Mod-Shift-Space",
      run: (view) => requestSignatureHelp(options, view),
    }, {
      key: "Escape",
      run: (view) => {
        if (view.state.field(signatureField) === null) return false;
        view.dispatch({ effects: setSignature.of(null) });
        return true;
      },
    }]),
    EditorView.theme({
      ".cm-tooltip.cm-tooltip-hover, .cm-tooltip.cm-tooltip-signature": {
        maxWidth: "min(620px, 70vw)",
        border: "1px solid hsl(var(--border))",
        borderRadius: "6px",
        backgroundColor: "hsl(var(--raised))",
        color: "hsl(var(--foreground))",
        boxShadow: "0 10px 30px rgb(0 0 0 / .35)",
      },
      ".cm-geosolve-language-tooltip": {
        padding: "7px 9px",
        fontFamily: "var(--font-mono)",
        fontSize: "12px",
        lineHeight: "1.45",
        whiteSpace: "pre-wrap",
      },
      ".cm-tooltip.cm-tooltip-signature": {
        pointerEvents: "none",
      },
      ".cm-geosolve-language-doc": {
        marginTop: "5px",
        color: "hsl(var(--muted))",
        fontFamily: "var(--font-sans)",
      },
      ".cm-geosolve-active-parameter": {
        borderRadius: "3px",
        backgroundColor: "rgb(251 191 36 / .18)",
        color: "hsl(var(--accent))",
        fontWeight: "600",
      },
      ".cm-lintRange-error": { backgroundImage: "none", textDecoration: "underline wavy #f87171" },
      ".cm-lintRange-warning": { backgroundImage: "none", textDecoration: "underline wavy #fbbf24" },
    }),
  ];
}

export function forceTypeScriptDiagnostics(view: EditorView): void {
  forceLinting(view);
}

export function showTypeScriptDiagnostics(view: EditorView): boolean {
  return diagnosticCount(view.state) > 0 && openLintPanel(view);
}

async function completionSource(
  options: LanguageExtensionOptions,
  context: CompletionContext,
): Promise<CompletionResult | null> {
  if (!context.view) return null;
  options.synchronize(context.view);
  try {
    const result = await options.client.completion(context.pos, context.explicit);
    if (!result || result.options.length === 0) return null;
    return {
      from: result.from,
      to: result.to,
      options: result.options.map((option): Completion => ({
        label: option.label,
        apply: (view, completion, from, to) => {
          const replaceFrom = option.from ?? from;
          const replaceTo = option.to ?? to;
          view.dispatch({
            changes: { from: replaceFrom, to: replaceTo, insert: option.insertText },
            selection: { anchor: replaceFrom + option.insertText.length },
            annotations: pickedCompletion.of(completion),
          });
        },
        ...(option.filterText ? { displayLabel: option.filterText } : {}),
        ...(option.detail ? { detail: option.detail } : {}),
        sortText: option.sortText,
        type: completionKind(option.kind),
      })),
      validFor: /^[$\w]*$/u,
    };
  } catch {
    return null;
  }
}

function codeMirrorDiagnostic(diagnostic: TypeScriptLanguageDiagnostic): Diagnostic {
  return {
    from: diagnostic.from,
    to: diagnostic.to,
    severity: diagnostic.severity,
    message: diagnostic.message,
    source: `TypeScript TS${diagnostic.code}`,
  };
}

function statusForDiagnostics(diagnostics: TypeScriptLanguageDiagnostic[]): TypeScriptLanguageStatus {
  return {
    state: "ready",
    version: TYPESCRIPT_LANGUAGE_VERSION,
    diagnostics: diagnostics.length,
    errors: diagnostics.filter((diagnostic) => diagnostic.severity === "error").length,
    warnings: diagnostics.filter((diagnostic) => diagnostic.severity === "warning").length,
  };
}

function emptyStatus(state: "checking"): Extract<TypeScriptLanguageStatus, { state: "checking" }> {
  return {
    state,
    version: TYPESCRIPT_LANGUAGE_VERSION,
    diagnostics: 0,
    errors: 0,
    warnings: 0,
  };
}

function completionKind(kind: string): string {
  switch (kind) {
    case "method":
    case "function":
    case "property":
    case "class":
    case "interface":
    case "enum":
    case "keyword":
      return kind;
    case "const":
      return "constant";
    case "type":
    case "type parameter":
      return "type";
    case "module":
      return "namespace";
    case "let":
    case "var":
    case "parameter":
      return "variable";
    default:
      return "text";
  }
}

function insertedText(update: ViewUpdate): string {
  let inserted = "";
  for (const transaction of update.transactions) {
    if (!transaction.docChanged || !transaction.isUserEvent("input")) continue;
    transaction.changes.iterChanges((_fromA, _toA, _fromB, _toB, text) => {
      inserted += text.toString();
    });
  }
  return inserted;
}

function requestSignatureHelp(options: LanguageExtensionOptions, view: EditorView): boolean {
  options.synchronize(view);
  const position = view.state.selection.main.head;
  const document = view.state.doc.toString();
  void options.client.signature(position).then((signature) => {
    if (
      document === view.state.doc.toString()
      && position === view.state.selection.main.head
    ) view.dispatch({ effects: setSignature.of(signature) });
  }).catch(() => undefined);
  return true;
}

function hoverDescription(hover: TypeScriptLanguageHover): Tooltip {
  return {
    pos: hover.from,
    end: hover.to,
    above: true,
    create: () => ({ dom: languageTooltip(hover.display, hover.documentation) }),
  };
}

function signatureTooltip(signature: TypeScriptLanguageSignature): Tooltip {
  return {
    pos: signature.to,
    above: true,
    create: () => {
      const dom = document.createElement("div");
      dom.className = "cm-geosolve-language-tooltip cm-tooltip-signature";
      const code = document.createElement("code");
      code.append(signature.prefix);
      signature.parameters.forEach((parameter, index) => {
        if (index > 0) code.append(signature.separator);
        const span = document.createElement("span");
        if (index === signature.activeParameter) span.className = "cm-geosolve-active-parameter";
        span.textContent = parameter.display;
        code.append(span);
      });
      code.append(signature.suffix);
      dom.append(code);
      const active = signature.parameters[signature.activeParameter];
      const documentation = active?.documentation ?? signature.documentation;
      if (documentation) dom.append(documentationElement(documentation));
      if (signature.overloadCount > 1) {
        dom.append(documentationElement(`${signature.overload + 1} of ${signature.overloadCount} overloads`));
      }
      return { dom };
    },
  };
}

function languageTooltip(display: string, documentation?: string): HTMLElement {
  const dom = document.createElement("div");
  dom.className = "cm-geosolve-language-tooltip";
  const code = document.createElement("code");
  code.textContent = display;
  dom.append(code);
  if (documentation) dom.append(documentationElement(documentation));
  return dom;
}

function documentationElement(documentation: string): HTMLElement {
  const element = document.createElement("div");
  element.className = "cm-geosolve-language-doc";
  element.textContent = documentation;
  return element;
}
