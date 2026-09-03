// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EditorView } from "@codemirror/view";
import { describe, expect, it, vi } from "vitest";
import { CodeEditor } from "./code-editor";
import type { TypeScriptLanguageWorkerPort } from "../language/client";
import {
  TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
  TYPESCRIPT_LANGUAGE_VERSION,
  type TypeScriptLanguageRequest,
  type TypeScriptLanguageResponse,
} from "../language/protocol";

class EditorLanguageWorker implements TypeScriptLanguageWorkerPort {
  listener?: (event: MessageEvent<TypeScriptLanguageResponse>) => void;
  syncs: TypeScriptLanguageRequest[] = [];

  postMessage(message: TypeScriptLanguageRequest): void {
    if (message.kind === "sync") {
      this.syncs.push(message);
      return;
    }
    if (message.kind !== "diagnostics") return;
    const latest = this.syncs.at(-1);
    const source = latest?.kind === "sync"
      ? latest.files.find((file) => file.path === "sketch.ts")?.contents ?? ""
      : "";
    const from = source.indexOf("wrong");
    queueMicrotask(() => this.listener?.({ data: {
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
    } } as MessageEvent<TypeScriptLanguageResponse>));
  }

  addEventListener(_type: "message", listener: (event: MessageEvent<TypeScriptLanguageResponse>) => void): void { this.listener = listener; }
  removeEventListener(): void { this.listener = undefined; }
  terminate(): void {}
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
});
