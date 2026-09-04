// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { diagnosticCount } from "@codemirror/lint";
import { EditorView } from "@codemirror/view";
import { describe, expect, it, vi } from "vitest";
import { CodeEditor } from "./code-editor";
import type { TypeScriptLanguageWorkerPort } from "../language/client";
import {
  TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
  TYPESCRIPT_LANGUAGE_VERSION,
  type TypeScriptLanguageRequest,
  type TypeScriptLanguageWorkerResponse,
} from "../language/protocol";

class EditorLanguageWorker implements TypeScriptLanguageWorkerPort {
  messageListener?: (event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void;
  errorListener?: (event: ErrorEvent) => void;
  syncs: TypeScriptLanguageRequest[] = [];
  terminated = false;

  constructor(private readonly rejectSync = false) {}

  postMessage(message: TypeScriptLanguageRequest): void {
    if (message.kind === "sync") {
      this.syncs.push(message);
      if (this.rejectSync) {
        queueMicrotask(() => this.messageListener?.({ data: {
          protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
          kind: "sync-error",
          project: message.project,
          revision: message.revision,
          typescriptVersion: TYPESCRIPT_LANGUAGE_VERSION,
          error: "TypeScript language-service project exceeds its source limit",
        } } as MessageEvent<TypeScriptLanguageWorkerResponse>));
      }
      return;
    }
    if (message.kind !== "diagnostics") return;
    const latest = this.syncs.at(-1);
    const source = latest?.kind === "sync"
      ? latest.files.find((file) => file.path === "sketch.ts")?.contents ?? ""
      : "";
    const from = source.indexOf("wrong");
    queueMicrotask(() => this.messageListener?.({ data: {
      protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
      request: message.request,
      project: message.project,
      revision: message.revision,
      kind: "diagnostics",
      typescriptVersion: TYPESCRIPT_LANGUAGE_VERSION,
      stale: false,
      result: from < 0 ? [] : [{
        from,
        to: from + "wrong".length,
        severity: "error",
        message: "Type 'string' is not assignable to type 'number'.",
        code: 2322,
        source: "TypeScript",
      }],
    } } as MessageEvent<TypeScriptLanguageWorkerResponse>));
  }

  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void)
      | ((event: ErrorEvent) => void),
  ): void {
    if (type === "message") {
      this.messageListener = listener as (event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void;
    } else {
      this.errorListener = listener as (event: ErrorEvent) => void;
    }
  }
  removeEventListener(type: "message" | "error"): void {
    if (type === "message") this.messageListener = undefined;
    else this.errorListener = undefined;
  }
  terminate(): void {
    this.terminated = true;
  }
}

describe("CodeEditor TypeScript language integration", () => {
  it("shows editor-only diagnostics and keeps source publication callback separate", async () => {
    const worker = new EditorLanguageWorker();
    const onChange = vi.fn();
    const { container } = render(<CodeEditor
      value="const radius: number = wrong;"
      onChange={onChange}
      languageProject={{
        key: "project",
        file: "sketch.ts",
        files: [{ path: "sketch.ts", contents: "const radius: number = wrong;" }],
      }}
      languageWorkerFactory={() => worker}
    />);
    await waitFor(() => expect(screen.getByText("1 error")).toBeInTheDocument());
    expect(onChange).not.toHaveBeenCalled();

    const view = EditorView.findFromDOM(container.querySelector<HTMLElement>(".cm-editor")!)!;
    view.dispatch({ changes: { from: view.state.doc.length, insert: "\n" } });
    await waitFor(() => expect(onChange).toHaveBeenCalledWith("const radius: number = wrong;\n"));
    expect(worker.syncs.at(-1)).toMatchObject({
      kind: "sync",
      files: [{ path: "sketch.ts", contents: "const radius: number = wrong;\n" }],
    });
    expect(screen.queryByText(/native|solver|materialization/iu)).not.toBeInTheDocument();
  });

  it("opens the CodeMirror diagnostics panel from the TypeScript status", async () => {
    const worker = new EditorLanguageWorker();
    render(<CodeEditor
      value="const radius: number = wrong;"
      onChange={() => undefined}
      languageProject={{ key: "project", file: "sketch.ts", files: [{ path: "sketch.ts", contents: "const radius: number = wrong;" }] }}
      languageWorkerFactory={() => worker}
    />);
    const status = await screen.findByText("1 error");
    fireEvent.click(status.closest("button")!);
    expect(screen.getByText("Type 'string' is not assignable to type 'number'.")).toBeInTheDocument();
  });

  it("shows analysis unavailable when the worker refuses project synchronization", async () => {
    const worker = new EditorLanguageWorker(true);
    render(<CodeEditor
      value="const radius = 1;"
      onChange={() => undefined}
      languageProject={{ key: "project", file: "sketch.ts", files: [{ path: "sketch.ts", contents: "const radius = 1;" }] }}
      languageWorkerFactory={() => worker}
    />);

    const status = await screen.findByText("TypeScript analysis unavailable");
    expect(status.closest("button")).toHaveAttribute(
      "title",
      "TypeScript language-service project exceeds its source limit",
    );
  });

  it("retains the editor and TypeScript extensions when read-only mode changes", async () => {
    const worker = new EditorLanguageWorker();
    const workerFactory = () => worker;
    const project = {
      key: "project",
      file: "sketch.ts",
      files: [{ path: "sketch.ts", contents: "const radius: number = wrong;" }],
    };
    const { container, rerender } = render(<CodeEditor
      value="const radius: number = wrong;"
      onChange={() => undefined}
      languageProject={project}
      languageWorkerFactory={workerFactory}
    />);
    await waitFor(() => expect(screen.getByText("1 error")).toBeInTheDocument());
    const editorElement = container.querySelector<HTMLElement>(".cm-editor")!;
    const editor = EditorView.findFromDOM(editorElement)!;
    expect(diagnosticCount(editor.state)).toBe(1);

    rerender(<CodeEditor
      value="const radius: number = wrong;"
      readOnly
      onChange={() => undefined}
      languageProject={project}
      languageWorkerFactory={workerFactory}
    />);

    const retainedElement = container.querySelector<HTMLElement>(".cm-editor")!;
    const retainedEditor = EditorView.findFromDOM(retainedElement)!;
    expect(retainedElement).toBe(editorElement);
    expect(retainedEditor).toBe(editor);
    expect(retainedEditor.state.readOnly).toBe(true);
    expect(diagnosticCount(retainedEditor.state)).toBe(1);
    expect(worker.terminated).toBe(false);
  });
});
