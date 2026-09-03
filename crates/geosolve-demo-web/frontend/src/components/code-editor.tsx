// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef } from "react";
import { javascript } from "@codemirror/lang-javascript";
import { EditorState } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";

interface CodeEditorProps {
  value: string;
  readOnly?: boolean;
  onChange: (value: string) => void;
  navigation?: EditorNavigation | null;
}

export interface EditorNavigation {
  /** Changes for every deliberate navigation, even when the span is unchanged. */
  request: number;
  /** Exact CodeMirror/JavaScript UTF-16 code-unit positions. */
  from: number;
  to: number;
}

export function CodeEditor({ value, readOnly = false, onChange, navigation }: CodeEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

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
          if (update.docChanged) onChangeRef.current(update.state.doc.toString());
        }),
        EditorView.theme({
          "&": { height: "100%", fontSize: "13px", backgroundColor: "hsl(var(--canvas))" },
          ".cm-scroller": { overflow: "auto", fontFamily: "var(--font-mono)" },
          ".cm-gutters": { backgroundColor: "hsl(var(--surface))", borderRight: "1px solid hsl(var(--border))" },
          ".cm-activeLine": { backgroundColor: "rgb(255 255 255 / .035)" },
        }),
      ],
    });
    view.current = new EditorView({ state, parent: host.current });
    return () => { view.current?.destroy(); view.current = null; };
    // Preserve the editor DOM, cursor and scroll; external source replacement is handled below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [readOnly]);

  useEffect(() => {
    const editor = view.current;
    if (!editor || editor.state.doc.toString() === value) return;
    editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: value } });
  }, [value]);

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

  return <div ref={host} className="min-h-0 flex-1 overflow-hidden" aria-label="Source editor" />;
}
