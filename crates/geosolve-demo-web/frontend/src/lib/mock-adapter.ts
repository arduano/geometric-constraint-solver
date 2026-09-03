// SPDX-License-Identifier: GPL-3.0-or-later
import type { DeclarationCapabilities, DeclarationRow, PointerSample, WorkbenchAdapter, WorkbenchSnapshot } from "./adapter";
import commandsJson from "../data/commands.json";
import type { ToolCatalog, ToolCommandDefinition } from "./tool-catalog";
import { assertToolCatalog } from "./tool-catalog";

interface MockCommandEntry { group: string; id: string; label: string; }
const commands = commandsJson as {
  geometry: MockCommandEntry[];
  constraints: MockCommandEntry[];
  dimensions: MockCommandEntry[];
  modify: MockCommandEntry[];
  context: MockCommandEntry[];
};

const SOURCE = `import { sketch, mm } from "@geosolve/sketch";\n\nexport default sketch(({ line, fillet }) => {\n  const edge = line([0, 0], [mm(40), 0]);\n  fillet(edge.end, { radius: mm(4) });\n});\n`;

const disabled = (reason: string) => ({ enabled: false, reason });
const managedCapabilities = (overrides: Partial<DeclarationCapabilities> = {}): DeclarationCapabilities => ({
  select: { enabled: true },
  navigate: { enabled: true },
  edit: { enabled: true },
  move: { enabled: true },
  moveUp: { enabled: true },
  moveDown: { enabled: true },
  suppress: disabled("This declaration has no explicit source-owned suppression field."),
  delete: { enabled: true },
  ...overrides,
});
const groupCapabilities = (): DeclarationCapabilities => ({
  select: disabled("Groups organize declarations and are not selectable."),
  navigate: disabled("This group has no source declaration."),
  edit: disabled("Edit declarations inside this group."),
  move: disabled("Group reordering is not available."),
  moveUp: disabled("Group reordering is not available."),
  moveDown: disabled("Group reordering is not available."),
  suppress: disabled("Groups cannot be suppressed."),
  delete: disabled("Groups cannot be deleted here."),
});

function mockExplorer(): DeclarationRow[] {
  const source = (text: string) => ({ path: "sketch.ts", from: SOURCE.indexOf(text), to: SOURCE.indexOf(text) + text.length });
  return [{
    id: "group:sketch",
    label: "Sketch declarations",
    kind: "Group",
    rowKind: "group",
    selected: false,
    children: [
      { id: "origin", label: "Origin", kind: "Reference", rowKind: "declaration", selected: false, source: source("sketch"), children: [], capabilities: managedCapabilities({ edit: disabled("Reference declarations are read-only."), delete: disabled("Reference declarations cannot be deleted."), moveUp: disabled("Already first in this group.") }) },
      { id: "line-1", label: "Line 1", kind: "Geometry", rowKind: "declaration", selected: false, source: source("const edge"), children: [], capabilities: managedCapabilities() },
      { id: "fillet-1", label: "Fillet 1", kind: "Computed", rowKind: "declaration", selected: true, source: source("fillet(edge.end"), children: [{ id: "generated:fillet-1:arc", label: "corner / arc", kind: "Generated output", rowKind: "generated", selected: false, suppressed: false, source: source("fillet(edge.end"), children: [], capabilities: managedCapabilities({ edit: disabled("Generated outputs are edited through their source invocation."), move: disabled("Generated outputs remain ordered by their source invocation."), moveUp: disabled("Generated outputs cannot move independently."), moveDown: disabled("Generated outputs cannot move independently."), suppress: { enabled: true } }) }], capabilities: managedCapabilities({ moveDown: disabled("Already last in this group."), suppress: { enabled: true } }) },
    ],
    capabilities: groupCapabilities(),
  }];
}

function initialSnapshot(): WorkbenchSnapshot {
  return {
    version: 1,
    revision: 1,
    project: { title: "Untitled sketch", status: "accepted" },
    presentation: { activeTool: "select", gridVisible: true, canUndo: false, canRedo: false, canFinish: false, geometryRole: "profile" },
    frame: {
      ariaLabel: "Accepted GeoSolve sketch viewport",
      svg: `<svg viewBox="0 0 900 600" role="img" aria-label="Empty accepted sketch"><defs><pattern id="grid" width="24" height="24" patternUnits="userSpaceOnUse"><path d="M 24 0 L 0 0 0 24" fill="none" stroke="#353a40" stroke-width="1"/></pattern></defs><rect width="900" height="600" fill="url(#grid)"/><path d="M180 390 L430 390 L610 210" fill="none" stroke="#e7a83e" stroke-width="3"/><circle cx="180" cy="390" r="6" fill="#f1c36d"/><circle cx="610" cy="210" r="6" fill="#f1c36d"/></svg>`,
    },
    source: { selectedPath: "sketch.ts", dirty: false, files: [{ path: "sketch.ts", language: "typescript", contents: SOURCE, readOnly: false }, { path: "fillets.ts", language: "typescript", contents: "export const radius = 4;\n", readOnly: true }] },
    explorer: mockExplorer(),
    selection: {
      id: "fillet-1",
      label: "Fillet 1",
      kind: "Fillet",
      ownership: "Modifiable in source",
      source: {
        path: "sketch.ts",
        from: SOURCE.indexOf("mm(4)"),
        to: SOURCE.indexOf("mm(4)") + "mm(4)".length,
      },
    },
    parameters: [{ id: "radius", label: "Radius", value: "4", unit: "mm", editable: true }],
    problems: [],
  };
}

export class MockWorkbenchAdapter implements WorkbenchAdapter {
  protected state = initialSnapshot();
  async construct(_input?: { version: 1; persistedProject?: string }): Promise<WorkbenchSnapshot> { return structuredClone(this.state); }
  async toolCatalog(): Promise<ToolCatalog> { return structuredClone(MOCK_TOOL_CATALOG); }
  async snapshot(): Promise<WorkbenchSnapshot> { return structuredClone(this.state); }
  async managedCompilerContext() { return { version: 1 as const, patches: {} }; }
  async dispatch(input: { command: string; payload?: unknown }): Promise<WorkbenchSnapshot> {
    if (input.command === "managed.mutation.resolve") {
      const pending = this.state.pendingManagedMutation;
      const receipt = input.payload as {
        ticketDigest: string;
        compiled?: { normalizedSource: string };
        mutation?: { compiled: { normalizedSource: string } };
      };
      const expectedDigest = pending?.request.ticket.ticketDigest;
      if (!pending || expectedDigest !== receipt.ticketDigest) {
        throw new Error("managed mutation receipt does not match the pending mock ticket");
      }
      const managed = this.state.source.files.find((file) => file.path === "sketch.ts");
      const compiled = receipt.compiled ?? receipt.mutation?.compiled;
      if (!compiled) throw new Error("managed mutation mock receipt has no compiled candidate");
      if (managed) managed.contents = compiled.normalizedSource;
      this.state.pendingManagedMutation = undefined;
      this.state.project.status = "accepted";
      this.state.revision += 1;
      return structuredClone(this.state);
    }
    if (input.command === "managed.mutation.abort") {
      const pending = this.state.pendingManagedMutation;
      const failure = input.payload as {
        ticketDigest: string;
        candidateSource: string;
        diagnostic: string;
        span: { start: number; end: number };
      };
      const expectedDigest = pending?.request.ticket.ticketDigest;
      if (!pending || expectedDigest !== failure.ticketDigest) {
        throw new Error("managed mutation abort does not match the pending mock ticket");
      }
      const managed = this.state.source.files.find((file) => file.path === "sketch.ts");
      if (managed) managed.contents = failure.candidateSource;
      const candidateBytes = new TextEncoder().encode(failure.candidateSource);
      const prefix = new TextDecoder("utf-8", { fatal: true }).decode(
        candidateBytes.slice(0, failure.span.start),
      );
      const line = prefix.split("\n").length;
      const column = Array.from(prefix.split("\n").at(-1) ?? "").length + 1;
      this.state.pendingManagedMutation = undefined;
      this.state.source.dirty = true;
      this.state.project.status = "failed";
      this.state.problems = [{
        id: `managed-mutation-${this.state.revision}`,
        severity: "error",
        title: "Managed source mutation rejected",
        detail: failure.diagnostic,
        file: "sketch.ts",
        line,
        column,
      }];
      return structuredClone(this.state);
    }
    if (input.command === "sample.open") {
      const sample = input.payload as { key: string; title: string };
      this.state.project = { title: sample.title, sampleKey: sample.key, status: "accepted" };
      this.state.revision += 1;
    }
    if (input.command === "project.new" || input.command === "project.new-code") {
      this.state = initialSnapshot();
      this.state.project.title = input.command === "project.new-code" ? "Untitled code sketch" : "Untitled sketch";
      this.state.revision += 1;
    }
    if (input.command === "source.change") {
      const contents = String((input.payload as { contents: string }).contents);
      this.state.source.files[0] = { ...this.state.source.files[0], contents };
      this.state.source.dirty = true;
      this.state.project.status = "dirty";
    }
    if (input.command === "source.prepare") {
      const contents = String((input.payload as { contents: string }).contents);
      this.state.source.files[0] = { ...this.state.source.files[0], contents };
    }
    if (input.command === "source.prepare" || input.command === "source.revert") {
      this.state.source.dirty = false;
      this.state.project.status = "accepted";
      this.state.revision += 1;
    }
    if (input.command === "source.select") this.state.source.selectedPath = String((input.payload as { path: string }).path);
    if (input.command === "selection.select" || input.command === "declaration.select") {
      const id = String((input.payload as { id: string }).id);
      const item = findDeclaration(this.state.explorer, id);
      if (item) {
        markSelected(this.state.explorer, id);
        this.state.selection = { id, label: item.label, kind: item.kind, source: item.source };
      }
    }
    if (input.command === "declaration.suppression.set") {
      const edit = input.payload as { id: string; suppressed: boolean };
      const item = findDeclaration(this.state.explorer, edit.id);
      if (item) item.suppressed = edit.suppressed;
      this.state.revision += 1;
    }
    if (input.command === "declaration.delete") {
      removeDeclaration(this.state.explorer, String((input.payload as { id: string }).id));
      this.state.revision += 1;
    }
    if (input.command === "parameter.edit") {
      const edit = input.payload as { id: string; value: string };
      const previous = this.state.parameters.find((parameter) => parameter.id === edit.id)?.value;
      this.state.parameters = this.state.parameters.map((parameter) => parameter.id === edit.id ? { ...parameter, value: edit.value } : parameter);
      if (previous !== undefined) {
        this.state.source.files[0] = { ...this.state.source.files[0], contents: this.state.source.files[0].contents.replace(`radius: mm(${previous})`, `radius: mm(${edit.value})`) };
      }
      this.state.revision += 1;
      this.state.presentation.canUndo = true;
      this.state.presentation.canRedo = false;
    }
    if (input.command === "history.undo" && this.state.presentation.canUndo) {
      this.state.presentation.canUndo = false;
      this.state.presentation.canRedo = true;
    }
    if (input.command === "history.redo" && this.state.presentation.canRedo) {
      this.state.presentation.canUndo = true;
      this.state.presentation.canRedo = false;
    }
    if (input.command === "tool.select") {
      this.state.presentation.activeTool = String((input.payload as { id: string }).id);
      this.state.presentation.canFinish = false;
    }
    if (input.command === "tool.finish") this.state.presentation.canFinish = false;
    if (input.command === "geometry.authoring-role.toggle") this.state.presentation.geometryRole = this.state.presentation.geometryRole === "profile" ? "construction" : "profile";
    if (input.command === "geometry.role.toggle" && this.state.presentation.selectedGeometryRole) this.state.presentation.selectedGeometryRole = this.state.presentation.selectedGeometryRole === "construction" ? "profile" : "construction";
    if (input.command === "view.grid.toggle") this.state.presentation.gridVisible = !this.state.presentation.gridVisible;
    return structuredClone(this.state);
  }
  async pointer(_input: PointerSample): Promise<WorkbenchSnapshot | null> { return null; }
  async wheel(): Promise<WorkbenchSnapshot | null> { return null; }
  async resize(): Promise<WorkbenchSnapshot | null> { return null; }
  async cancel(_input: { version: 1; reason: "escape" | "lost-capture" | "blur" }): Promise<WorkbenchSnapshot | null> { return null; }
  async exportProject() { return { version: 1 as const, filename: "project.json", contents: JSON.stringify(this.state, null, 2) }; }
  async persistProject() { return { version: 1 as const, contents: JSON.stringify(this.state) }; }
  async exportReproduction() { return { version: 1 as const, filename: "geosolve-reproduction.txt", contents: JSON.stringify(this.state) }; }
  async exportInteractionTrace() { return { version: 1 as const, filename: "geosolve-interaction-trace.txt", contents: "No interaction trace events recorded." }; }
}

function findDeclaration(rows: DeclarationRow[], id: string): DeclarationRow | undefined {
  for (const row of rows) {
    if (row.id === id) return row;
    const child = findDeclaration(row.children, id);
    if (child) return child;
  }
  return undefined;
}

function markSelected(rows: DeclarationRow[], id: string) {
  for (const row of rows) {
    row.selected = row.id === id;
    markSelected(row.children, id);
  }
}

function removeDeclaration(rows: DeclarationRow[], id: string): boolean {
  const index = rows.findIndex((row) => row.id === id);
  if (index >= 0) { rows.splice(index, 1); return true; }
  return rows.some((row) => removeDeclaration(row.children, id));
}

function mockTool(entry: MockCommandEntry, stableId: string, iconKey: string): ToolCommandDefinition {
  return {
    stableId,
    toolId: entry.id,
    label: entry.label,
    group: entry.group,
    icon: {
      key: iconKey,
      svg: `<svg class="wb-palette-icon" viewBox="-10 -10 20 20" aria-hidden="true" focusable="false" data-icon-key="${iconKey}"><path d="M-8 5L8-5"/><circle cx="-6" cy="4" r="1.25"/><circle cx="6" cy="-4" r="1.25"/></svg>`,
    },
  };
}

const MOCK_TOOL_CATALOG = assertToolCatalog({
  version: 1,
  select: mockTool({ group: "Selection", id: "select", label: "Select" }, "sketch.select", "geometry-select"),
  sections: [
    { id: "sketch", label: "Sketch", description: "Draw points, lines, profiles, and advanced curves.", commands: commands.geometry.map((entry) => mockTool(entry, `sketch.variant.${entry.id}`, `geometry-${entry.id}`)) },
    { id: "constraint", label: "Constraint", description: "Relate geometry by placement, orientation, and continuity.", commands: commands.constraints.map((entry) => mockTool(entry, `constraint.${entry.id}`, entry.id)) },
    { id: "dimension", label: "Dimension", description: "Control linear, circular, and angular measurements.", commands: commands.dimensions.map((entry) => mockTool(entry, `dimension.${entry.id}`, entry.id)) },
    { id: "modify", label: "Modify", description: "Create profile operations without changing drawing mode.", commands: commands.modify.map((entry) => mockTool(entry, `modify.${entry.id}`, entry.id === "fillet" ? "feature-fillet" : "modify-offset")) },
  ],
  geometryRole: mockTool(commands.context[0]!, "inspector.geometry-role", "geometry-role-construction"),
});
