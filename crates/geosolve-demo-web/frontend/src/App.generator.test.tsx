// SPDX-License-Identifier: GPL-3.0-or-later
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EditorView } from "@codemirror/view";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { createFolderWorkbenchSession } from "./lib/folder-workbench-session";
import { FolderWorkbenchAdapter, type FolderState } from "./lib/folder-adapter";
import { MockWorkbenchAdapter } from "./lib/mock-adapter";
import { folderBrowser, folderModel } from "./lib/folder-test-support";

async function ready() {
  const native = new MockWorkbenchAdapter();
  const fixture = await native.snapshot();
  const browser = await folderBrowser(fixture);
  const state: FolderState = {
    mode: "generator", entry: "generator.ts", authority: { epoch: "epoch", lease: 1, revision: 1 },
    editor: { clientId: "tab", canEdit: true }, ok: true, sequence: 1, currentHash: "accepted-before", acceptedHash: "accepted-before", status: "saved",
    diagnostics: [], paths: { folder: "/project", source: "/project/generator.ts" },
    capabilities: { sourceEditing: false, geometryEditing: false, generatorInputs: true },
    inputDefinitions: { width: { type: "number", default: 12, label: "Channel width", unit: "mm", min: 8, description: "Width shared by the channels." } }, inputs: { width: 12 },
    sourceFiles: [{ path: "generator.ts", language: "typescript", contents: "export default function generate() {}", readOnly: true },
      { path: "profile.ts", language: "typescript", contents: "export const profile = 12;", readOnly: true }],
  };
  const requests: Array<{ method: string; input?: { command?: string; values?: Record<string, unknown> } }> = [];
  vi.stubGlobal("fetch", vi.fn(async (_url, init) => {
    const request = JSON.parse(init.body); requests.push(request);
    if (request.method === "inputs.set") {
      state.inputs = request.input.values; state.currentHash = state.acceptedHash = "accepted-input";
      state.authority!.revision++;
    }
    if (request.method === "session.takeover") { state.editor!.canEdit = true; state.authority!.lease++; }
    const model = folderModel(fixture, state.authority!.revision, state.acceptedHash!);
    model.mode = "generator"; model.model = null; model.generated = {};
    return { ok: true, json: async () => ({ result: model, state: structuredClone(state) }) };
  }));
  vi.stubGlobal("EventSource", class extends EventTarget { close() {} });
  const adapter = new FolderWorkbenchAdapter("token", () => browser.local, { createBrowsing: browser.createBrowsing, authoring: null });
  const rendered = render(<App session={createFolderWorkbenchSession(adapter)} />);
  await screen.findByRole("region", { name: "Local folder" });
  await screen.findByRole("region", { name: "Generator inputs" });
  return { adapter, requests, state, ...rendered, user: userEvent.setup() };
}

afterEach(() => { vi.unstubAllGlobals(); sessionStorage.clear(); });

describe("generator workbench", () => {
  it("offers source inputs and inspection while source and geometry controls are read-only", async () => {
    const { adapter, requests, user, container } = await ready();
    expect(screen.getByRole("textbox", { name: "Channel width" })).toHaveValue("12");
    expect(screen.getByText("Width shared by the channels.", { exact: false })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Select" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Sketch" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: /^split$/i }));
    await screen.findByRole("tab", { name: "generator.ts" });
    const editor = EditorView.findFromDOM(container.querySelector(".cm-editor")!);
    expect(editor?.state.readOnly).toBe(true);
    await user.click(screen.getByRole("tab", { name: "profile.ts" }));
    expect(screen.getByRole("tab", { name: "profile.ts" })).toHaveAttribute("aria-selected", "true");
    expect(requests.filter((request) => request.input?.command === "source.select")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "File menu" }));
    expect(screen.getByRole("menuitem", { name: /Open/ })).toBeDisabled();
    expect(screen.getByRole("menuitem", { name: /Import/ })).toBeDisabled();
    await user.keyboard("{Escape}");
    const width = screen.getByRole("textbox", { name: "Channel width" });
    fireEvent.focus(width); fireEvent.change(width, { target: { value: "14" } });
    await user.click(screen.getByRole("button", { name: "Apply Channel width" }));
    await waitFor(() => expect(adapter.installedState?.inputs).toEqual({ width: 14 }));
    expect(requests.find((request) => request.method === "inputs.set")?.input?.values).toEqual({ width: 14 });
    expect(screen.getByRole("button", { name: "Apply Channel width" })).toBeDisabled();
  });

  it("keeps drafts across a lost lease and enables input edits only after explicit takeover", async () => {
    const { adapter, state, user } = await ready();
    const width = screen.getByRole("textbox", { name: "Channel width" });
    fireEvent.focus(width); fireEvent.change(width, { target: { value: "14" } });
    state.editor!.canEdit = false; state.authority!.lease = 2; state.currentHash = "changed-on-disk"; state.inputs = { width: 20 };
    await act(() => adapter.refresh(false));
    expect(width).toBeDisabled();
    expect(width).toHaveValue("14");
    await user.click(screen.getByRole("button", { name: "Take over editing" }));
    await waitFor(() => expect(width).toBeEnabled());
    expect(width).toHaveValue("14");
    expect(adapter.installedState?.inputs).toEqual({ width: 12 });
    expect(screen.getByRole("button", { name: "Download pending intent" })).toBeEnabled();
  });
});
