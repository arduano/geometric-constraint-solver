// SPDX-License-Identifier: GPL-3.0-or-later
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { EditorView } from "@codemirror/view";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { MockWorkbenchAdapter } from "./lib/mock-adapter";
import {
  PREPARED_MANAGED_MUTATION_FORMAT,
  type PendingManagedMutation,
  type PointerSample,
} from "./lib/adapter";
import { compileManagedSource } from "./lib/managed-compiler";
import type { ProjectReadResult, ProjectStore } from "./lib/project-storage";

afterEach(() => vi.restoreAllMocks());

class TestProjectStore implements ProjectStore {
  writes = 0;
  removals = 0;

  constructor(
    public value: string | null = null,
    public writeIssue: string | null = null,
    public removeIssue: string | null = null,
  ) {}

  async read(): Promise<ProjectReadResult> {
    return {
      value: this.value,
      issue: null,
      provenance: this.value === null ? "confirmed-empty" as const : "indexed-db" as const,
      autosaveSafe: true,
    };
  }
  async write(value: string) {
    this.writes += 1;
    if (this.writeIssue) return { value: false, issue: this.writeIssue };
    this.value = value;
    return { value: true, issue: null };
  }
  async remove() {
    this.removals += 1;
    if (this.removeIssue) return { value: false, issue: this.removeIssue };
    this.value = null;
    return { value: true, issue: null };
  }
}

async function ready(
  adapter: MockWorkbenchAdapter = new MockWorkbenchAdapter(),
  projectStore: ProjectStore = new TestProjectStore(),
) {
  const user = userEvent.setup();
  const result = render(<App adapter={adapter} projectStore={projectStore} />);
  await screen.findByText("Untitled sketch");
  return { user, adapter, projectStore, ...result };
}

const APP_MANAGED_SOURCE = `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const edge = $.geometry.segment("edge", { start: [0, 0], end: [20, 0] });
  return { edge };
});
`;
const APP_MANAGED_CURRENT = compileManagedSource(APP_MANAGED_SOURCE);
const testDigest = (digit: string) => digit.repeat(64);
function pendingManagedMutation(ticketDigit: string): PendingManagedMutation {
  return {
    kind: "managed",
    request: {
      ticket: {
        format: PREPARED_MANAGED_MUTATION_FORMAT,
        ticketDigest: testDigest(ticketDigit),
        project: "app-test",
        session: { session: 1, revision: 0, digest: testDigest("1") },
        acceptedSourceDigest: APP_MANAGED_CURRENT.ir.source_digest,
        acceptedIrDigest: APP_MANAGED_CURRENT.ir.ir_digest,
        acceptedArtifactDigest: APP_MANAGED_CURRENT.artifact.artifact_digest,
        acceptedExpansionDigest: testDigest("2"),
        declarationNameHighWater: 0,
        candidateDeclarationNameHighWater: 0,
        baseSemanticsDigest: testDigest("3"),
        candidateSemanticsDigest: testDigest("4"),
        mutation: {
          mutation: "set_suppressed",
          target: { target: "declaration", declaration: "edge" },
          suppressed: true,
        },
      },
      current: APP_MANAGED_CURRENT,
    },
  };
}

describe("M88 workbench interaction contract", () => {
  it("re-clicks, replacement, outside pointer and Escape light-dismiss one transient surface", async () => {
    const { user } = await ready();
    const sketch = screen.getByRole("button", { name: "Sketch" });
    await user.click(sketch);
    expect(screen.getByRole("region", { name: "Sketch tools" })).toBeVisible();
    await user.click(sketch);
    expect(screen.queryByRole("region", { name: "Sketch tools" })).not.toBeInTheDocument();

    await user.click(sketch);
    await user.click(screen.getByRole("button", { name: "Constraint" }));
    expect(screen.getByRole("menuitem", { name: "Coincident" })).toBeVisible();
    expect(screen.queryByRole("menuitem", { name: "Segment" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^code$/i }));
    expect(screen.queryByRole("region", { name: "Constraint tools" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^code$/i })).toHaveAttribute("aria-pressed", "true");

    await user.click(screen.getByRole("button", { name: /^design$/i }));
    await user.click(screen.getByRole("button", { name: "Dimension" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Dimension tools" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Dimension" })).toHaveFocus();
  });

  it("closes a category after selection while keeping the chosen tool and docked settings active", async () => {
    const { user } = await ready();
    await user.click(screen.getByRole("button", { name: "Sketch" }));
    await user.click(screen.getByRole("menuitem", { name: "Segment" }));
    expect(screen.queryByRole("region", { name: "Sketch tools" })).not.toBeInTheDocument();
    expect(screen.getByText("Click the canvas to continue · Esc cancels")).toBeVisible();
    expect(screen.getByText("Segment", { selector: "span.font-medium" })).toBeVisible();
    expect(screen.getByRole("application")).toHaveFocus();

    await user.click(screen.getByRole("application"));
    expect(screen.getByText("Click the canvas to continue · Esc cancels")).toBeVisible();
    await user.keyboard("{Escape}");
    expect(screen.queryByText("Click the canvas to continue · Esc cancels")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Select" })).toHaveFocus();
  });

  it("disables Finish until Rust reports an actionable retained draft", async () => {
    class FinishAdapter extends MockWorkbenchAdapter {
      commands: string[] = [];
      finishable = false;
      override async dispatch(input: { command: string; payload?: unknown }) {
        this.commands.push(input.command);
        return super.dispatch(input);
      }
      override async pointer() {
        if (!this.finishable) return null;
        const next = await this.snapshot();
        next.presentation.canFinish = true;
        return next;
      }
    }
    const adapter = new FinishAdapter();
    const { user } = await ready(adapter);
    await user.click(screen.getByRole("button", { name: "Sketch" }));
    await user.click(screen.getByRole("menuitem", { name: "Polyline" }));

    const finish = screen.getByRole("button", { name: "Finish" });
    expect(finish).toBeDisabled();
    await user.click(finish);
    expect(adapter.commands).not.toContain("tool.finish");

    adapter.finishable = true;
    // Exercise the same snapshot installation boundary used by pointer input.
    fireEvent.pointerDown(screen.getByRole("application"), { pointerId: 4, buttons: 1, clientX: 20, clientY: 20 });
    fireEvent.pointerUp(screen.getByRole("application"), { pointerId: 4, buttons: 0, clientX: 20, clientY: 20 });
    await waitFor(() => expect(finish).toBeEnabled());
    expect(screen.getByText("Click the canvas to continue · Enter or Finish completes · Esc cancels")).toBeVisible();
    screen.getByRole("application").focus();
    await user.keyboard("{Enter}");
    expect(adapter.commands).toContain("tool.finish");
  });

  it("enables history only when authoritative movement is available and no source draft blocks it", async () => {
    class HistoryAdapter extends MockWorkbenchAdapter {
      commands: string[] = [];
      override async dispatch(input: { command: string; payload?: unknown }) {
        this.commands.push(input.command);
        return super.dispatch(input);
      }
    }
    const adapter = new HistoryAdapter();
    const { user, container } = await ready(adapter);
    const undo = screen.getByRole("button", { name: "Undo" });
    const redo = screen.getByRole("button", { name: "Redo" });
    expect(undo).toBeDisabled();
    expect(redo).toBeDisabled();

    const parameters = screen.getByRole("tab", { name: "Parameters" });
    parameters.focus();
    await user.keyboard("{Enter}");
    const radius = screen.getByRole("textbox", { name: "Radius" });
    await user.clear(radius);
    await user.type(radius, "7{Enter}");
    await waitFor(() => expect(undo).toBeEnabled());
    expect(redo).toBeDisabled();

    await user.click(undo);
    await waitFor(() => expect(redo).toBeEnabled());
    expect(undo).toBeDisabled();
    await user.click(redo);
    await waitFor(() => expect(undo).toBeEnabled());
    expect(redo).toBeDisabled();

    await user.click(screen.getByRole("button", { name: /^code$/i }));
    await user.click(container.querySelector<HTMLElement>(".cm-content")!);
    await user.keyboard("{End}// local draft");
    await waitFor(() => expect(undo).toBeDisabled());
    expect(redo).toBeDisabled();
    expect(undo).toHaveAttribute("title", "Apply or Revert the source draft before moving history");
    const historyCommands = adapter.commands.filter((command) => command.startsWith("history."));
    await user.click(undo);
    await user.click(redo);
    expect(adapter.commands.filter((command) => command.startsWith("history."))).toEqual(historyCommands);
  });

  it("keeps the complete primary authoring inventory within two actions", async () => {
    const { user } = await ready();
    await user.click(screen.getByRole("button", { name: "Sketch" }));
    expect(screen.getAllByRole("menuitem")).toHaveLength(25);
    expect(new Set(screen.getAllByRole("menuitem").map((item) => item.querySelector("svg")?.dataset.iconKey))).toHaveProperty("size", 25);
    for (const family of ["Point", "Lines", "Rectangles", "Circles", "Arcs", "Ellipses", "Béziers", "Conics", "Splines"]) expect(screen.getByRole("heading", { name: family })).toBeVisible();
    expect(screen.getByRole("menuitem", { name: "Segment" }).querySelector('svg[data-icon-key="geometry-segment"]')).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Constraint" }));
    expect(screen.getAllByRole("menuitem")).toHaveLength(13);
    for (const group of ["Placement", "Orientation", "Equality & symmetry", "Curve join"]) expect(screen.getByRole("heading", { name: group })).toBeVisible();
    expect(screen.getByRole("menuitem", { name: "Coincident" }).querySelector('svg[data-icon-key="coincident"]')).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Dimension" }));
    expect(screen.getAllByRole("menuitem")).toHaveLength(5);
    expect(screen.getByRole("menuitem", { name: "Radius" }).querySelector('svg[data-icon-key="radius"]')).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Modify" }));
    expect(screen.getByRole("menuitem", { name: "Fillet" })).toBeVisible();
    expect(screen.getByRole("menuitem", { name: "Offset" })).toBeVisible();
    expect(screen.getAllByRole("menuitem")).toHaveLength(2);
    expect(screen.queryByRole("menuitem", { name: /grid|fit|origin/i })).not.toBeInTheDocument();
    expect(screen.getByRole("toolbar", { name: "Canvas view" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Fit sketch" })).toBeVisible();
    expect(screen.queryByRole("menuitem", { name: "Trim" })).not.toBeInTheDocument();
  });

  it("keeps canvas view and geometry-role commands outside authoring categories without replacing the active tool", async () => {
    class RecordingAdapter extends MockWorkbenchAdapter {
      commands: string[] = [];
      override async dispatch(input: { command: string; payload?: unknown }) { this.commands.push(input.command); return super.dispatch(input); }
    }
    const adapter = new RecordingAdapter();
    const { user } = await ready(adapter);
    expect(screen.queryByRole("button", { name: /New curves:/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sketch" }));
    await user.click(screen.getByRole("menuitem", { name: "Segment" }));
    expect(screen.getByText("Segment", { selector: "span.font-medium" })).toBeVisible();
    const role = screen.getByRole("button", { name: "New curves: Profile. Change to Construction" });
    expect(role).toHaveAttribute("aria-pressed", "false");

    await user.click(screen.getByRole("button", { name: "Hide grid" }));
    expect(screen.getByRole("button", { name: "Show grid" })).toBeVisible();
    expect(screen.getByText("Segment", { selector: "span.font-medium" })).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Fit sketch" }));
    await user.click(screen.getByRole("button", { name: "Center on origin" }));
    await user.click(role);

    expect(adapter.commands.slice(-4)).toEqual(["view.grid.toggle", "view.fit", "view.origin", "geometry.authoring-role.toggle"]);
    expect(screen.getByRole("button", { name: "New curves: Construction. Change to Profile" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("Segment", { selector: "span.font-medium" })).toBeVisible();
  });

  it("keeps selected-curve role editing distinct from new-curve authoring", async () => {
    class SelectedRoleAdapter extends MockWorkbenchAdapter {
      commands: string[] = [];
      override async construct() {
        const snapshot = await super.construct();
        snapshot.presentation.selectedGeometryRole = "profile";
        return snapshot;
      }
      override async dispatch(input: { command: string; payload?: unknown }) {
        this.commands.push(input.command);
        const snapshot = await super.dispatch(input);
        if (input.command !== "geometry.role.toggle") snapshot.presentation.selectedGeometryRole = "profile";
        return snapshot;
      }
    }
    const adapter = new SelectedRoleAdapter();
    const { user } = await ready(adapter);
    const role = screen.getByRole("button", { name: "Selected curves: Profile. Change to Construction" });
    expect(role).toHaveAttribute("aria-pressed", "false");
    await user.click(role);
    expect(adapter.commands).toContain("geometry.role.toggle");
    expect(screen.queryByRole("button", { name: /Selected curves:/ })).not.toBeInTheDocument();
  });

  it("overlays action feedback without moving the workspace", async () => {
    class RejectingAdapter extends MockWorkbenchAdapter {
      override async dispatch(input: { command: string; payload?: unknown }) {
        if (input.command === "tool.select" && (input.payload as { id?: string })?.id === "offset") {
          throw new Error("Profile Offset authoring is unavailable: topology capture did not produce a complete operand index");
        }
        return super.dispatch(input);
      }
    }
    const { user } = await ready(new RejectingAdapter());
    await user.click(screen.getByRole("button", { name: "Modify" }));
    await user.click(screen.getByRole("menuitem", { name: "Offset" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveClass("fixed", "bottom-3");
    expect(alert).not.toHaveClass("shrink-0");
    expect(alert).toHaveTextContent("Profile Offset authoring is unavailable");
    expect(screen.getByRole("application")).toBeVisible();
  });

  it("cancels a captured canvas gesture before deactivating authoring", async () => {
    class CapturingAdapter extends MockWorkbenchAdapter { cancels = 0; override async cancel() { this.cancels += 1; return null; } }
    const adapter = new CapturingAdapter();
    const { user, container } = await ready(adapter);
    await user.click(screen.getByRole("button", { name: "Sketch" }));
    await user.click(screen.getByRole("menuitem", { name: "Segment" }));
    fireEvent.pointerDown(screen.getByRole("application"), { pointerId: 7, buttons: 1, clientX: 20, clientY: 20 });
    await user.keyboard("{Escape}");
    expect(adapter.cancels).toBe(1);
    expect(screen.getByText("Click the canvas to continue · Esc cancels")).toBeVisible();
  });

  it("offers all 37 authoritative samples through semantic, searchable groups", async () => {
    const { user } = await ready();
    await user.click(screen.getByRole("button", { name: "File menu" }));
    await user.click(screen.getByRole("menuitem", { name: /Open/ }));
    expect(screen.getByText("37 samples")).toBeVisible();
    expect(screen.getByRole("heading", { name: "Mechanisms" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Mechanisms" })).not.toBeInTheDocument();

    const search = screen.getByPlaceholderText("Search 37 samples…");
    await user.type(search, "typed panel");
    expect(screen.getByText("1 samples")).toBeVisible();
    await user.click(screen.getByRole("button", { name: /Typed panel · keyed Fillets/ }));
    await screen.findByText("Typed panel · keyed Fillets");
    expect(screen.queryByLabelText("Open project")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "File menu" }));
    await user.click(screen.getByRole("menuitem", { name: /Open/ }));
    const recent = screen.getByRole("region", { name: "Recent" });
    expect(within(recent).getByRole("button", { name: /Typed panel · keyed Fillets/ })).toBeVisible();
    expect(JSON.parse(localStorage.getItem("geosolve-workbench-recents-v1") ?? "null")).toMatchObject({
      version: 1,
      entries: [{ kind: "code", key: "typed-panel" }],
    });
  });

  it("keeps Code first-class and removes hidden focusable workspaces", async () => {
    const { user, container } = await ready();
    await user.click(screen.getByRole("button", { name: /^code$/i }));
    expect(screen.getByRole("region", { name: "Code workspace" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Explorer unavailable in Code layout" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Details unavailable in Code layout" })).toBeDisabled();
    expect(screen.queryByRole("navigation", { name: "Primary tools" })).not.toBeInTheDocument();
    const editor = container.querySelector(".cm-editor");
    expect(editor).toBeInTheDocument();
    expect(screen.queryByRole("application")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^design$/i }));
    expect(screen.getByRole("application")).toBeVisible();
    expect(screen.queryByRole("region", { name: "Code workspace" })).not.toBeInTheDocument();
    expect(container.querySelector(".cm-editor")).toBe(editor);
    await user.click(screen.getByRole("button", { name: /^split$/i }));
    expect(screen.getByRole("region", { name: "Code workspace" })).toBeVisible();
    expect(container.querySelector(".cm-editor")).toBe(editor);
    expect(screen.getByRole("separator", { name: "Resize canvas and code" })).toBeVisible();
  });

  it("implements the advertised Open and Save shortcuts", async () => {
    class ShortcutAdapter extends MockWorkbenchAdapter {
      saves = 0;
      override async persistProject() { this.saves += 1; return super.persistProject(); }
    }
    const adapter = new ShortcutAdapter();
    const { user } = await ready(adapter);
    const before = adapter.saves;

    await user.keyboard("{Control>}o{/Control}");
    expect(screen.getByRole("dialog", { name: "Open project" })).toBeVisible();
    await user.keyboard("{Escape}");
    await user.keyboard("{Control>}s{/Control}");
    await waitFor(() => expect(adapter.saves).toBeGreaterThan(before));
  });

  it("preserves secondary-click browser behavior without semantic pointer dispatch", async () => {
    class PointerAdapter extends MockWorkbenchAdapter {
      pointers: PointerSample[] = [];
      override async pointer(input: PointerSample) { this.pointers.push(input); return null; }
    }
    const adapter = new PointerAdapter();
    await ready(adapter);
    const canvas = screen.getByRole("application");
    const secondary = (type: string, values: Record<string, number>) => {
      const event = new Event(type, { bubbles: true, cancelable: true });
      for (const [name, value] of Object.entries(values)) Object.defineProperty(event, name, { value });
      fireEvent(canvas, event);
    };
    secondary("pointerdown", { pointerId: 91, button: 2, buttons: 2, clientX: 40, clientY: 40 });
    secondary("pointermove", { pointerId: 91, button: -1, buttons: 2, clientX: 45, clientY: 45 });
    secondary("pointerup", { pointerId: 91, button: 2, buttons: 0, clientX: 45, clientY: 45 });
    expect(adapter.pointers).toEqual([]);

    const contextMenu = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    expect(canvas.dispatchEvent(contextMenu)).toBe(true);
    expect(contextMenu.defaultPrevented).toBe(false);
  });

  it("blocks project replacement and exact reproduction while a local source draft is unapplied", async () => {
    class ReproductionAdapter extends MockWorkbenchAdapter {
      reproductions = 0;
      override async exportReproduction() { this.reproductions += 1; return super.exportReproduction(); }
    }
    const adapter = new ReproductionAdapter();
    const { user, container } = await ready(adapter);
    await user.click(screen.getByRole("button", { name: /^code$/i }));
    await user.click(container.querySelector<HTMLElement>(".cm-content")!);
    await user.keyboard("{End}// keep this local draft");

    await user.keyboard("{Control>}o{/Control}");
    await user.click(screen.getByRole("button", { name: /Typed panel · keyed Fillets/ }));
    expect(screen.getByText("Untitled sketch")).toBeVisible();
    expect(await screen.findByRole("alert")).toHaveTextContent("Apply or Revert the current source draft before replacing this project");

    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Diagnostics" }));
    await user.click(screen.getByRole("menuitem", { name: "Download reproduction" }));
    expect(adapter.reproductions).toBe(0);
    expect(screen.getByRole("alert")).toHaveTextContent("Apply or Revert the current source draft before downloading an exact reproduction");
  });

  it("keeps each source keystroke local and crosses into Rust only on Apply", async () => {
    class RecordingAdapter extends MockWorkbenchAdapter {
      commands: string[] = [];
      override async construct() { const snapshot = await super.construct(); snapshot.source.dirty = true; snapshot.project.status = "dirty"; return snapshot; }
      override async dispatch(input: { command: string; payload?: unknown }) { this.commands.push(input.command); return super.dispatch(input); }
    }
    const adapter = new RecordingAdapter();
    const { user, container } = await ready(adapter);
    await user.click(screen.getByRole("button", { name: /^code$/i }));
    const editor = container.querySelector<HTMLElement>(".cm-content")!;
    await user.click(editor);
    await user.keyboard("{End}// draft");
    expect(adapter.commands).not.toContain("source.change");
    await user.click(screen.getByRole("button", { name: "Apply" }));
    expect(adapter.commands).toContain("source.prepare");
  });

  it("wires Explorer, Parameters and source tabs to semantic adapter commands", async () => {
    class RecordingAdapter extends MockWorkbenchAdapter {
      commands: Array<{ command: string; payload?: unknown }> = [];
      override async dispatch(input: { command: string; payload?: unknown }) { this.commands.push(input); return super.dispatch(input); }
    }
    const adapter = new RecordingAdapter();
    const { user, container } = await ready(adapter);
    const parameters = screen.getByRole("tab", { name: "Parameters" });
    parameters.focus();
    await user.keyboard("{Enter}");
    const radius = screen.getByRole("textbox", { name: "Radius" });
    await user.clear(radius);
    await user.type(radius, "7{Enter}");
    expect(adapter.commands.some(({ command, payload }) => command === "parameter.edit" && (payload as { value: string }).value === "7")).toBe(true);

    await user.click(screen.getByRole("button", { name: /^code$/i }));
    await waitFor(() => expect(container.querySelector(".cm-content")?.textContent).toContain("radius: mm(7)"));
    await user.click(screen.getByRole("button", { name: /^design$/i }));

    const explorer = screen.getByRole("complementary", { name: "Explorer" });
    const line = within(explorer).getByRole("button", { name: "Line 1" });
    expect(line).toHaveTextContent(/^Line 1$/);
    expect(line.querySelector("svg")).toHaveClass("shrink-0");
    await user.click(line);
    expect(adapter.commands.at(-1)).toMatchObject({ command: "declaration.select", payload: { id: "line-1" } });

    await user.click(screen.getByRole("button", { name: /^code$/i }));
    await user.click(screen.getByRole("tab", { name: "fillets.ts" }));
    expect(adapter.commands.at(-1)).toMatchObject({ command: "source.select", payload: { path: "fillets.ts" } });
    await waitFor(() => expect(container.querySelector(".cm-content")?.textContent).toContain("export const radius = 4;"));
  });

  it("composes accessible Explorer row, group, isolate and construction visibility controls", async () => {
    class VisibilityAdapter extends MockWorkbenchAdapter {
      commands: Array<{ command: string; payload?: unknown }> = [];

      constructor() {
        super();
        const other = structuredClone(this.state.explorer[0]);
        other.id = "group:other";
        other.label = "Other declarations";
        other.children = [];
        this.state.explorer.push(other);
      }

      override async dispatch(input: { command: string; payload?: unknown }) {
        this.commands.push(input);
        return super.dispatch(input);
      }
    }

    const adapter = new VisibilityAdapter();
    const { user } = await ready(adapter);
    const explorer = screen.getByRole("complementary", { name: "Explorer" });
    const construction = within(explorer).getByRole("button", {
      name: "Hide construction geometry",
    });
    expect(construction).toHaveAttribute("aria-pressed", "true");
    await user.click(construction);
    expect(adapter.commands.at(-1)).toMatchObject({ command: "view.construction.toggle" });
    expect(within(explorer).getByRole("button", {
      name: "Show construction geometry",
    })).toHaveAttribute("aria-pressed", "false");

    await user.click(within(explorer).getByRole("button", { name: "Hide Line 1" }));
    expect(adapter.commands.at(-1)).toMatchObject({
      command: "explorer.visibility.set",
      payload: { id: "line-1", visible: false },
    });
    expect(within(explorer).getByRole("button", { name: "Show Line 1" }))
      .toHaveAttribute("aria-pressed", "false");
    expect(within(explorer).getByRole("button", { name: "Hide Sketch declarations" }))
      .toHaveAttribute("data-visibility-state", "mixed");
    expect(within(explorer).getByRole("button", { name: "Hide Sketch declarations" }))
      .toHaveAttribute("aria-pressed", "mixed");

    await user.click(within(explorer).getByRole("button", { name: "Hide Sketch declarations" }));
    expect(within(explorer).getByRole("button", { name: "Hide Origin" }))
      .toHaveAttribute("title", expect.stringContaining("hidden by an ancestor group"));
    await user.click(within(explorer).getByRole("button", { name: "Show Sketch declarations" }));
    expect(within(explorer).getByRole("button", { name: "Show Line 1" }))
      .toHaveAttribute("aria-pressed", "false");

    await user.click(within(explorer).getByRole("button", {
      name: "Isolate Sketch declarations",
    }));
    expect(adapter.commands.at(-1)).toMatchObject({
      command: "explorer.visibility.isolate",
      payload: { id: "group:sketch" },
    });
    expect(within(explorer).getByRole("button", { name: "Show Other declarations" }))
      .toHaveAttribute("aria-pressed", "false");
    const restore = within(explorer).getByRole("button", {
      name: "Restore visibility before isolate",
    });
    expect(restore).toBeEnabled();
    restore.focus();
    await user.keyboard("{Enter}");
    expect(adapter.commands.at(-1)).toMatchObject({ command: "explorer.visibility.restore" });
    expect(within(explorer).getByRole("button", { name: "Hide Other declarations" }))
      .toHaveAttribute("aria-pressed", "true");
    expect(within(explorer).getByRole("button", {
      name: "Restore visibility before isolate",
    })).toBeDisabled();
  });

  it("renders the same ordered source-owned declaration tree in Explorer and Code", async () => {
    class RecordingAdapter extends MockWorkbenchAdapter {
      commands: Array<{ command: string; payload?: unknown }> = [];
      override async dispatch(input: { command: string; payload?: unknown }) { this.commands.push(input); return super.dispatch(input); }
    }
    const adapter = new RecordingAdapter();
    const { user } = await ready(adapter);
    await user.click(screen.getByRole("button", { name: /^code$/i }));
    await user.click(screen.getByRole("tab", { name: "Generated" }));

    const panel = screen.getByRole("list", { name: "Ordered declarations" });
    const origin = within(panel).getByRole("button", { name: "Origin" });
    const line = within(panel).getByRole("button", { name: "Line 1" });
    const fillet = within(panel).getByRole("button", { name: "Fillet 1" });
    const generated = within(panel).getByRole("button", { name: "corner / arc generated" });
    expect(origin.compareDocumentPosition(line) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(line.compareDocumentPosition(fillet) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(fillet.compareDocumentPosition(generated) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    await user.click(line);
    expect(adapter.commands.at(-1)).toMatchObject({ command: "declaration.select", payload: { id: "line-1" } });
    const actions = screen.getByRole("group", { name: "Line 1 actions" });
    await user.click(within(actions).getByRole("button", { name: "Move up" }));
    expect(adapter.commands.at(-1)).toMatchObject({ command: "declaration.move", payload: { id: "line-1", direction: "up" } });
    await user.click(within(actions).getByRole("button", { name: "Edit source" }));
    expect(adapter.commands.at(-1)).toMatchObject({ command: "declaration.source.open", payload: { id: "line-1" } });

    await user.click(screen.getByRole("tab", { name: "Generated" }));
    await user.click(screen.getByRole("button", { name: "corner / arc generated" }));
    await user.click(within(screen.getByRole("group", { name: "corner / arc actions" })).getByRole("button", { name: "Suppress" }));
    expect(adapter.commands.at(-1)).toMatchObject({ command: "declaration.suppression.set", payload: { id: "generated:fillet-1:arc", suppressed: true } });
  });

  it("keeps Restore reachable when suppression removes the selectable native declaration", async () => {
    class SuppressedAdapter extends MockWorkbenchAdapter {
      override async construct() {
        const snapshot = await super.construct();
        const group = snapshot.explorer[0];
        const fillet = group?.children.find((row) => row.id === "fillet-1");
        if (!fillet) throw new Error("mock Fillet declaration is unavailable");
        fillet.selected = false;
        fillet.suppressed = true;
        fillet.capabilities.select = {
          enabled: false,
          reason: "Suppression removed the accepted scene declaration.",
        };
        snapshot.selection = undefined;
        return snapshot;
      }
    }
    await ready(new SuppressedAdapter());
    const explorer = screen.getByRole("complementary", { name: "Explorer" });
    const actions = within(explorer).getByRole("group", { name: "Fillet 1 actions" });
    expect(within(actions).getByRole("button", { name: "Restore" })).toBeEnabled();
  });

  it("forwards uncaptured hover moves to Rust", async () => {
    class PointerAdapter extends MockWorkbenchAdapter { phases: string[] = []; override async pointer(input: PointerSample) { this.phases.push(input.phase); return null; } }
    const adapter = new PointerAdapter();
    await ready(adapter);
    fireEvent.pointerMove(screen.getByRole("application"), { pointerId: 3, buttons: 0, clientX: 25, clientY: 30 });
    expect(adapter.phases).toContain("move");
  });

  it("resolves a pointer-returned managed mutation before installing its snapshot", async () => {
    class ManagedPointerAdapter extends MockWorkbenchAdapter {
      commands: string[] = [];
      override async pointer(input: PointerSample) {
        if (input.phase !== "up") return null;
        this.state.pendingManagedMutation = pendingManagedMutation("a");
        const pending = await this.snapshot();
        pending.project.title = "Unresolved compiler request";
        return pending;
      }
      override async dispatch(input: { command: string; payload?: unknown }) {
        this.commands.push(input.command);
        return super.dispatch(input);
      }
    }
    const adapter = new ManagedPointerAdapter();
    const { user, container } = await ready(adapter);
    const canvas = screen.getByRole("application");

    fireEvent.pointerDown(canvas, { pointerId: 17, buttons: 1, clientX: 20, clientY: 20 });
    fireEvent.pointerUp(canvas, { pointerId: 17, buttons: 0, clientX: 20, clientY: 20 });

    await waitFor(() => expect(adapter.commands).toContain("managed.mutation.resolve"));
    expect(screen.queryByText("Unresolved compiler request")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^code$/i }));
    await waitFor(() => expect(container.querySelector(".cm-content")?.textContent).toContain("$.suppress(edge);"));
  });

  it("resolves a command-returned managed mutation before installing its snapshot", async () => {
    class ManagedCommandAdapter extends MockWorkbenchAdapter {
      commands: string[] = [];
      override async dispatch(input: { command: string; payload?: unknown }) {
        this.commands.push(input.command);
        if (input.command === "declaration.select") {
          this.state.pendingManagedMutation = pendingManagedMutation("b");
          const pending = await this.snapshot();
          pending.project.title = "Unresolved command compiler request";
          return pending;
        }
        return super.dispatch(input);
      }
    }
    const adapter = new ManagedCommandAdapter();
    const { user, container } = await ready(adapter);

    await user.click(within(screen.getByRole("complementary", { name: "Explorer" })).getByRole("button", { name: "Line 1" }));

    await waitFor(() => expect(adapter.commands).toEqual([
      "declaration.select",
      "managed.mutation.resolve",
    ]));
    expect(screen.queryByText("Unresolved command compiler request")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^code$/i }));
    await waitFor(() => expect(container.querySelector(".cm-content")?.textContent).toContain("$.suppress(edge);"));
  });

  it("distinguishes normal pointer-up capture release from genuine capture loss", async () => {
    class PointerAdapter extends MockWorkbenchAdapter {
      phases: string[] = [];
      cancels: string[] = [];
      override async pointer(input: PointerSample) { this.phases.push(input.phase); return null; }
      override async cancel(input: { version: 1; reason: "escape" | "lost-capture" | "blur" }) { this.cancels.push(input.reason); return null; }
    }
    const adapter = new PointerAdapter();
    await ready(adapter);
    const canvas = screen.getByRole("application");

    fireEvent.pointerDown(canvas, { pointerId: 7, buttons: 1, clientX: 20, clientY: 20 });
    fireEvent.pointerUp(canvas, { pointerId: 7, buttons: 0, clientX: 20, clientY: 20 });
    fireEvent.lostPointerCapture(canvas, { pointerId: 7 });
    await waitFor(() => expect(adapter.phases).toEqual(["down", "up"]));
    expect(adapter.cancels).toEqual([]);

    fireEvent.pointerDown(canvas, { pointerId: 8, buttons: 1, clientX: 30, clientY: 30 });
    fireEvent.pointerCancel(canvas, { pointerId: 8 });
    fireEvent.lostPointerCapture(canvas, { pointerId: 8 });
    await waitFor(() => expect(adapter.cancels).toEqual(["lost-capture"]));

    fireEvent.pointerDown(canvas, { pointerId: 9, buttons: 1, clientX: 40, clientY: 40 });
    fireEvent.lostPointerCapture(canvas, { pointerId: 9 });
    await waitFor(() => expect(adapter.cancels).toEqual(["lost-capture", "lost-capture"]));
  });

  it("opens an authenticated managed owner at its exact source span", async () => {
    const { user, container } = await ready();
    await user.click(screen.getByRole("button", { name: "Open in code" }));
    expect(screen.getByRole("button", { name: /^code$/i })).toHaveAttribute("aria-pressed", "true");
    await waitFor(() => expect(container.querySelector(".cm-content")).toHaveFocus());
    const view = EditorView.findFromDOM(container.querySelector<HTMLElement>(".cm-editor")!)!;
    const expected = view.state.doc.toString().indexOf("mm(4)");
    expect(view.state.selection.main).toMatchObject({ from: expected, to: expected + "mm(4)".length });
  });

  it("opens Rust byte spans exactly across BMP and astral Unicode", async () => {
    class UnicodeNavigationAdapter extends MockWorkbenchAdapter {
      override async construct() {
        const snapshot = await super.construct();
        const prefix = "// naïve 東京 🧭\nconst radius = ";
        const target = 'mm("半径😀")';
        const contents = `${prefix}${target};\n`;
        const bytes = (value: string) => new TextEncoder().encode(value).byteLength;
        snapshot.source.selectedPath = "sketch.ts";
        snapshot.source.files[0] = { ...snapshot.source.files[0]!, contents };
        snapshot.selection = {
          id: "unicode-radius",
          label: "Unicode radius",
          kind: "Parameter",
          ownership: "Modifiable in source",
          source: {
            path: "sketch.ts",
            from: bytes(prefix),
            to: bytes(prefix + target),
          },
        };
        return snapshot;
      }
    }

    const { user, container } = await ready(new UnicodeNavigationAdapter());
    await user.click(screen.getByRole("button", { name: "Open in code" }));
    await waitFor(() => expect(container.querySelector(".cm-content")).toHaveFocus());
    const view = EditorView.findFromDOM(container.querySelector<HTMLElement>(".cm-editor")!)!;
    expect(view.state.sliceDoc(
      view.state.selection.main.from,
      view.state.selection.main.to,
    )).toBe('mm("半径😀")');
  });

  it("refuses a split authenticated byte span without dispatching editor navigation", async () => {
    class InvalidNavigationAdapter extends MockWorkbenchAdapter {
      override async construct() {
        const snapshot = await super.construct();
        const contents = "é target\n";
        snapshot.source.files[0] = { ...snapshot.source.files[0]!, contents };
        snapshot.selection = {
          id: "split-byte-span",
          label: "Split byte span",
          kind: "Parameter",
          ownership: "Modifiable in source",
          source: { path: "sketch.ts", from: 1, to: 2 },
        };
        return snapshot;
      }
    }

    const { user } = await ready(new InvalidNavigationAdapter());
    await user.click(screen.getByRole("button", { name: "Open in code" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("source offset splits a UTF-8 code point");
    expect(screen.getByRole("button", { name: /^design$/i })).toHaveAttribute("aria-pressed", "true");
  });

  it("restores bounded browser persistence and surfaces canonical export refusals", async () => {
    class PersistenceAdapter extends MockWorkbenchAdapter {
      restored?: string;
      exports = 0;
      override async construct(input?: { version: 1; persistedProject?: string }) { this.restored = input?.persistedProject; return super.construct(); }
      override async exportProject(): Promise<never> { this.exports += 1; throw new Error("dirty draft cannot be exported canonically"); }
    }
    const adapter = new PersistenceAdapter();
    const projectStore = new TestProjectStore("saved-workspace");
    const baseline = await adapter.snapshot();
    const source = baseline.source.files[0]!;
    localStorage.setItem("geosolve.source-draft.v1", JSON.stringify({ version: 1, title: baseline.project.title, sampleKey: null, path: source.path, base: source.contents, contents: `${source.contents}// restored local draft\n` }));
    const { user, container } = await ready(adapter, projectStore);
    expect(adapter.restored).toBe("saved-workspace");
    await user.click(screen.getByRole("button", { name: /^code$/i }));
    expect(container.querySelector(".cm-content")).toHaveTextContent("restored local draft");
    await user.click(screen.getByRole("button", { name: "File menu" }));
    await user.click(screen.getByRole("menuitem", { name: "Export canonical project…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Apply or Revert the current source draft");
    expect(adapter.exports).toBe(0);
    expect(projectStore.value).toBe("saved-workspace");
    expect(projectStore.writes).toBe(0);
  });

  it("fences automatic saves after an uncertain read until the user explicitly saves", async () => {
    class CountingPersistenceAdapter extends MockWorkbenchAdapter {
      saves = 0;
      override async persistProject() { this.saves += 1; return super.persistProject(); }
    }
    class UncertainProjectStore extends TestProjectStore {
      override async read() {
        return {
          value: "legacy-fallback",
          issue: "Browser storage could not read the saved project: temporary IndexedDB failure",
          provenance: "uncertain" as const,
          autosaveSafe: false,
        };
      }
    }
    const adapter = new CountingPersistenceAdapter();
    const projectStore = new UncertainProjectStore("unread-indexed-project");
    const { user } = await ready(adapter, projectStore);
    expect(await screen.findByRole("alert")).toHaveTextContent("Automatic project saving is paused");

    const parameters = screen.getByRole("tab", { name: "Parameters" });
    parameters.focus();
    await user.keyboard("{Enter}");
    const radius = screen.getByRole("textbox", { name: "Radius" });
    await user.clear(radius);
    await user.type(radius, "7{Enter}");
    await waitFor(() => expect(radius).toHaveValue("7"));
    expect(adapter.saves).toBe(0);
    expect(projectStore.writes).toBe(0);
    expect(projectStore.value).toBe("unread-indexed-project");
    expect(screen.getByRole("alert")).toHaveTextContent("Automatic project saving is paused");

    await user.click(screen.getByRole("button", { name: "File menu" }));
    await user.click(screen.getByRole("menuitem", { name: /Open/ }));
    await user.click(screen.getByRole("button", { name: "New sketch" }));
    await Promise.resolve();
    expect(adapter.saves).toBe(0);
    expect(projectStore.writes).toBe(0);
    expect(projectStore.value).toBe("unread-indexed-project");

    await user.click(screen.getByRole("button", { name: "File menu" }));
    await user.click(screen.getByRole("menuitem", { name: /^Save in browser/ }));
    await waitFor(() => expect(projectStore.writes).toBe(1));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());

    const updatedRadius = screen.getByRole("textbox", { name: "Radius" });
    await user.clear(updatedRadius);
    await user.type(updatedRadius, "8{Enter}");
    await waitFor(() => expect(projectStore.writes).toBe(2));
  });

  it("does not rewrite an exact loaded payload on boot or a later no-op autosave", async () => {
    class ExactPersistenceAdapter extends MockWorkbenchAdapter {
      saves = 0;
      override async persistProject() {
        this.saves += 1;
        return { version: 1 as const, contents: "exact-saved-project" };
      }
    }
    const adapter = new ExactPersistenceAdapter();
    const projectStore = new TestProjectStore("exact-saved-project");

    const { user } = await ready(adapter, projectStore);
    await Promise.resolve();

    expect(adapter.saves).toBe(0);
    expect(projectStore.writes).toBe(0);
    expect(projectStore.value).toBe("exact-saved-project");

    const parameters = screen.getByRole("tab", { name: "Parameters" });
    parameters.focus();
    await user.keyboard("{Enter}");
    const radius = screen.getByRole("textbox", { name: "Radius" });
    await user.clear(radius);
    await user.type(radius, "7{Enter}");
    await waitFor(() => expect(adapter.saves).toBe(1));
    expect(projectStore.writes).toBe(0);

    await user.click(screen.getByRole("button", { name: "File menu" }));
    await user.click(screen.getByRole("menuitem", { name: /^Save in browser/ }));
    await waitFor(() => expect(projectStore.writes).toBe(1));
  });

  it("does not rewrite the last successfully written payload on a later no-op autosave", async () => {
    class StablePersistenceAdapter extends MockWorkbenchAdapter {
      saves = 0;
      override async persistProject() {
        this.saves += 1;
        return { version: 1 as const, contents: "stable-project" };
      }
    }
    const adapter = new StablePersistenceAdapter();
    const projectStore = new TestProjectStore();
    const { user } = await ready(adapter, projectStore);
    const parameters = screen.getByRole("tab", { name: "Parameters" });
    parameters.focus();
    await user.keyboard("{Enter}");

    let radius = screen.getByRole("textbox", { name: "Radius" });
    await user.clear(radius);
    await user.type(radius, "7{Enter}");
    await waitFor(() => expect(projectStore.writes).toBe(1));

    radius = screen.getByRole("textbox", { name: "Radius" });
    await user.clear(radius);
    await user.type(radius, "8{Enter}");
    await waitFor(() => expect(adapter.saves).toBe(2));
    expect(projectStore.writes).toBe(1);
    expect(projectStore.value).toBe("stable-project");
  });

  it("explicitly saves a project replacement even when its visible identity tuple collides", async () => {
    class CollidingReplacementAdapter extends MockWorkbenchAdapter {
      generation = 0;
      override async dispatch(input: { command: string; payload?: unknown }) {
        const next = await super.dispatch(input);
        if (input.command !== "project.new") return next;
        this.generation += 1;
        this.state.revision = 0;
        return this.snapshot();
      }
      override async persistProject() {
        return { version: 1 as const, contents: `replacement-${this.generation}` };
      }
    }
    const adapter = new CollidingReplacementAdapter();
    const projectStore = new TestProjectStore("replacement-0");
    const { user } = await ready(adapter, projectStore);

    await user.click(screen.getByRole("button", { name: "File menu" }));
    await user.click(screen.getByRole("menuitem", { name: /Open/ }));
    await user.click(screen.getByRole("button", { name: "New sketch" }));

    await waitFor(() => expect(projectStore.value).toBe("replacement-1"));
    expect(projectStore.writes).toBe(1);
  });

  it("retains rejected saved bytes when constructing the fresh fallback also fails", async () => {
    class RejectingPersistenceAndFallbackAdapter extends MockWorkbenchAdapter {
      override async construct(input?: { version: 1; persistedProject?: string }): Promise<never> {
        if (input?.persistedProject) throw new Error("saved payload is invalid");
        throw new Error("fresh construction failed");
      }
    }
    const projectStore = new TestProjectStore("rejected-saved-workspace");

    render(<App adapter={new RejectingPersistenceAndFallbackAdapter()} projectStore={projectStore} />);

    expect(await screen.findByText("Workbench unavailable")).toBeVisible();
    expect(projectStore.removals).toBe(0);
    expect(projectStore.writes).toBe(0);
    expect(projectStore.value).toBe("rejected-saved-workspace");
  });

  it("retains unread IndexedDB authority when its legacy fallback is also rejected", async () => {
    class RejectingLegacyFallbackAdapter extends MockWorkbenchAdapter {
      override async construct(input?: { version: 1; persistedProject?: string }) {
        if (input?.persistedProject) throw new Error("legacy fallback is invalid");
        return super.construct();
      }
    }
    class UncertainProjectStore extends TestProjectStore {
      override async read() {
        return {
          value: "legacy-fallback",
          issue: "Browser storage could not read the saved project: temporary IndexedDB failure",
          provenance: "uncertain" as const,
          autosaveSafe: false,
        };
      }
    }
    const projectStore = new UncertainProjectStore("unread-indexed-project");

    render(<App adapter={new RejectingLegacyFallbackAdapter()} projectStore={projectStore} />);

    expect(await screen.findByText("Untitled sketch")).toBeVisible();
    expect(await screen.findByRole("alert")).toHaveTextContent("IndexedDB authority was unread");
    expect(projectStore.removals).toBe(0);
    expect(projectStore.writes).toBe(0);
    expect(projectStore.value).toBe("unread-indexed-project");
  });

  it("does not remove rejected saved bytes before a fresh fallback is ready", async () => {
    let releaseFallback!: () => void;
    const fallbackGate = new Promise<void>((resolve) => { releaseFallback = resolve; });
    class DelayedFallbackAdapter extends MockWorkbenchAdapter {
      fallbackStarted = false;
      override async construct(input?: { version: 1; persistedProject?: string }) {
        if (input?.persistedProject) throw new Error("saved payload is invalid");
        this.fallbackStarted = true;
        await fallbackGate;
        return super.construct();
      }
    }
    const adapter = new DelayedFallbackAdapter();
    const projectStore = new TestProjectStore("rejected-saved-workspace");
    render(<App adapter={adapter} projectStore={projectStore} />);
    await waitFor(() => expect(adapter.fallbackStarted).toBe(true));

    expect(projectStore.removals).toBe(0);
    expect(projectStore.value).toBe("rejected-saved-workspace");

    releaseFallback();
    expect(await screen.findByText("Untitled sketch")).toBeVisible();
    await waitFor(() => expect(projectStore.removals).toBe(1));
    await waitFor(() => expect(projectStore.writes).toBe(1));
  });

  it("loads with safe defaults and a durable alert when browser storage reads are blocked", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("Access is blocked", "SecurityError");
    });

    await ready();
    expect(screen.getByText("Untitled sketch")).toBeVisible();
    expect(await screen.findByRole("alert")).toHaveTextContent(/Browser storage could not read/);
    expect(screen.getByRole("alert")).toHaveTextContent("SecurityError: Access is blocked");
  });

  it("keeps rendering and reports draft and manual-save quota failures", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("Storage quota exceeded", "QuotaExceededError");
    });

    const projectStore = new TestProjectStore(
      null,
      "Browser storage could not write the saved project: QuotaExceededError: Storage quota exceeded",
    );
    const { user, container } = await ready(new MockWorkbenchAdapter(), projectStore);
    expect(await screen.findByRole("alert")).toHaveTextContent(/Browser storage could not write/);

    await user.click(screen.getByRole("button", { name: /^code$/i }));
    await user.click(container.querySelector<HTMLElement>(".cm-content")!);
    await user.keyboard("{End}// quota draft");
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("the unapplied source draft"));

    await user.click(screen.getByRole("button", { name: "File menu" }));
    await user.click(screen.getByRole("menuitem", { name: /^Save in browser/ }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("the saved project"));
    expect(screen.getByRole("alert")).toHaveTextContent("QuotaExceededError: Storage quota exceeded");
    expect(screen.getByRole("region", { name: "Code workspace" })).toBeVisible();
  });

  it("recovers a rejected saved project even when its storage removal is blocked", async () => {
    class RejectingPersistenceAdapter extends MockWorkbenchAdapter {
      override async construct(input?: { version: 1; persistedProject?: string }) {
        if (input?.persistedProject) throw new Error("saved payload is invalid");
        return super.construct();
      }
    }
    const projectStore = new TestProjectStore(
      "invalid-saved-workspace",
      null,
      "Browser storage could not remove the saved project: SecurityError: Removal is blocked",
    );

    await ready(new RejectingPersistenceAdapter(), projectStore);
    expect(screen.getByText("Untitled sketch")).toBeVisible();
    expect(await screen.findByRole("alert")).toHaveTextContent("Saved workspace could not be restored and was reset: saved payload is invalid");
    expect(screen.getByRole("alert")).toHaveTextContent("Browser storage could not remove the saved project: SecurityError: Removal is blocked");
  });

  it("gives the modal first Escape ownership", async () => {
    const { user } = await ready();
    await user.click(screen.getByRole("button", { name: "Sketch" }));
    await user.click(screen.getByRole("menuitem", { name: "Segment" }));
    await user.click(screen.getByRole("button", { name: "Diagnostics" }));
    await user.click(screen.getByRole("menuitem", { name: "Interaction trace…" }));
    expect(screen.getByRole("dialog", { name: "Interaction trace" })).toBeVisible();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Interaction trace" })).not.toBeInTheDocument());
    expect(screen.getByText("Click the canvas to continue · Esc cancels")).toBeVisible();
  });

  it("has no serious or critical axe violations in the loaded design shell", async () => {
    const { container } = await ready();
    const result = await axe.run(container, { resultTypes: ["violations"], rules: { "color-contrast": { enabled: false } } });
    expect(result.violations.filter(({ impact }) => impact === "serious" || impact === "critical")).toEqual([]);
  });
});
