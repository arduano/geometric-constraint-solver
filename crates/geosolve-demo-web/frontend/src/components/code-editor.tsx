// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef, useState } from "react";
import { javascript } from "@codemirror/lang-javascript";
import { Compartment, EditorState } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import {
  createTypeScriptLanguageWorker,
  TypeScriptLanguageWorkerClient,
  type TypeScriptLanguageProject,
  type TypeScriptLanguageWorkerPort,
} from "../language/client";
import {
  forceTypeScriptDiagnostics,
  showTypeScriptDiagnostics,
  type TypeScriptLanguageStatus,
  typeScriptLanguageExtensions,
} from "../language/editor-extensions";
import { TYPESCRIPT_LANGUAGE_VERSION } from "../language/protocol";

interface CodeEditorProps {
  value: string;
  readOnly?: boolean;
  onChange: (value: string) => void;
  navigation?: EditorNavigation | null;
  languageProject?: TypeScriptLanguageProject | null;
  languageWorkerFactory?: () => TypeScriptLanguageWorkerPort | null;
}

export interface EditorNavigation {
  /** Changes for every deliberate navigation, even when the span is unchanged. */
  request: number;
  /** Exact CodeMirror/JavaScript UTF-16 code-unit positions. */
  from: number;
  to: number;
}

export function CodeEditor({
  value,
  readOnly = false,
  onChange,
  navigation,
  languageProject = null,
  languageWorkerFactory = createTypeScriptLanguageWorker,
}: CodeEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const languageCompartment = useRef(new Compartment());
  const languageClient = useRef<TypeScriptLanguageWorkerClient | null>(null);
  const languageProjectRef = useRef(languageProject);
  const onChangeRef = useRef(onChange);
  const [languageStatus, setLanguageStatus] = useState<TypeScriptLanguageStatus | null>(null);
  onChangeRef.current = onChange;
  languageProjectRef.current = languageProject;
  const languageEnabled = languageProject !== null;
  const synchronize = (editor: EditorView) => {
    const client = languageClient.current;
    const project = languageProjectRef.current;
    if (!client || !project) return;
    client.sync(projectWithDocument(project, editor.state.doc.toString()));
  };

  useEffect(() => {
    if (!host.current) return;
    const state = EditorState.create({
      doc: value,
      extensions: [
        lineNumbers(),
        highlightActiveLineGutter(),
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        javascript({ typescript: true }),
        oneDark,
        EditorState.readOnly.of(readOnly),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            synchronize(update.view);
            onChangeRef.current(update.state.doc.toString());
          }
        }),
        languageCompartment.current.of([]),
        EditorView.theme({
          "&": { height: "100%", fontSize: "13px", backgroundColor: "hsl(var(--canvas))" },
          ".cm-scroller": { overflow: "auto", fontFamily: "var(--font-mono)" },
          ".cm-gutters": { backgroundColor: "hsl(var(--surface))", borderRight: "1px solid hsl(var(--border))" },
          ".cm-activeLine": { backgroundColor: "rgb(255 255 255 / .035)" },
        }),
      ],
    });
    view.current = new EditorView({ state, parent: host.current });
    return () => {
      languageClient.current?.dispose();
      languageClient.current = null;
      view.current?.destroy();
      view.current = null;
    };
    // Preserve the editor DOM, cursor and scroll; external source replacement is handled below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [readOnly]);

  useEffect(() => {
    const editor = view.current;
    if (!editor) return;
    languageClient.current?.dispose();
    languageClient.current = null;
    if (!languageEnabled) {
      editor.dispatch({ effects: languageCompartment.current.reconfigure([]) });
      setLanguageStatus(null);
      return;
    }
    try {
      const worker = languageWorkerFactory();
      if (!worker) {
        setLanguageStatus(languageFailure("Web Workers are unavailable"));
        return;
      }
      const client = new TypeScriptLanguageWorkerClient(
        worker,
        (error) => setLanguageStatus(languageFailure(error)),
      );
      languageClient.current = client;
      editor.dispatch({
        effects: languageCompartment.current.reconfigure(typeScriptLanguageExtensions({
          client,
          synchronize,
          report: setLanguageStatus,
        })),
      });
      setLanguageStatus({ state: "checking", version: TYPESCRIPT_LANGUAGE_VERSION, diagnostics: 0, errors: 0, warnings: 0 });
      synchronize(editor);
      forceTypeScriptDiagnostics(editor);
    } catch (error) {
      setLanguageStatus(languageFailure(error));
    }
    return () => {
      languageClient.current?.dispose();
      languageClient.current = null;
    };
    // The editor DOM is intentionally retained when analysis starts or stops.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [languageEnabled, languageWorkerFactory]);

  useEffect(() => {
    const editor = view.current;
    if (!editor || editor.state.doc.toString() === value) return;
    editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: value } });
  }, [value]);

  useEffect(() => {
    const editor = view.current;
    const client = languageClient.current;
    if (!editor || !client || !languageProject) return;
    client.sync(projectWithDocument(languageProject, editor.state.doc.toString()));
    forceTypeScriptDiagnostics(editor);
  }, [languageProject]);

  useEffect(() => {
    const editor = view.current;
    if (!editor || !navigation) return;
    const { from, to } = navigation;
    if (
      !Number.isSafeInteger(from)
      || !Number.isSafeInteger(to)
      || from < 0
      || to < from
      || to > editor.state.doc.length
    ) return;
    editor.dispatch({
      selection: { anchor: from, head: to },
      effects: EditorView.scrollIntoView(from, { y: "center" }),
    });
    editor.focus();
  }, [navigation]);

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const observer = new ResizeObserver(() => view.current?.requestMeasure());
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return <section className="flex min-h-0 flex-1 flex-col overflow-hidden">
    <div ref={host} className="min-h-0 flex-1 overflow-hidden" aria-label="Source editor" />
    {languageStatus && <button
      type="button"
      className={`flex h-6 shrink-0 items-center gap-2 border-t border-border bg-surface px-2 text-left text-[10px] outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-accent ${languageStatus.state === "error" || languageStatus.errors > 0 ? "text-red-300" : languageStatus.warnings > 0 ? "text-amber-300" : "text-muted"}`}
      title={languageStatus.state === "error" ? languageStatus.message : "TypeScript editor diagnostics. Ctrl/Cmd+Shift+M opens the diagnostics panel."}
      onClick={() => { if (view.current) showTypeScriptDiagnostics(view.current); }}
    >
      <span className={`size-1.5 rounded-full ${languageStatus.state === "checking" ? "bg-sky-300" : languageStatus.state === "error" || languageStatus.errors > 0 ? "bg-red-400" : languageStatus.warnings > 0 ? "bg-amber-300" : "bg-emerald-400"}`} />
      <span aria-live="polite">{languageStatusLabel(languageStatus)}</span>
      <span className="ml-auto text-muted">TypeScript {languageStatus.version}</span>
    </button>}
  </section>;
}

function projectWithDocument(
  project: TypeScriptLanguageProject,
  contents: string,
): TypeScriptLanguageProject {
  return {
    ...project,
    files: project.files.map((file) => file.path === project.file
      ? { ...file, contents }
      : file),
  };
}

function languageFailure(error: unknown): TypeScriptLanguageStatus {
  return {
    state: "error",
    version: TYPESCRIPT_LANGUAGE_VERSION,
    diagnostics: 0,
    errors: 0,
    warnings: 0,
    message: error instanceof Error ? error.message : String(error),
  };
}

function languageStatusLabel(status: TypeScriptLanguageStatus): string {
  if (status.state === "checking") return "Checking TypeScript…";
  if (status.state === "error") return "TypeScript analysis unavailable";
  if (status.diagnostics === 0) return "No TypeScript issues";
  if (status.errors === 0 && status.warnings === 0) {
    return status.diagnostics === 1 ? "1 TypeScript suggestion" : `${status.diagnostics} TypeScript suggestions`;
  }
  const errors = status.errors === 1 ? "1 error" : `${status.errors} errors`;
  const warnings = status.warnings === 1 ? "1 warning" : `${status.warnings} warnings`;
  return status.errors > 0 && status.warnings > 0
    ? `${errors}, ${warnings}`
    : status.errors > 0 ? errors : warnings;
}
