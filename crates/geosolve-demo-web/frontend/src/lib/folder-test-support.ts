// SPDX-License-Identifier: GPL-3.0-or-later
// Transport fixtures deliberately return only the engine protocol. Native
// mathematics and snapshot composition have their own actual-WASM tests.
import { vi } from "vitest";
import type { WorkbenchSnapshot } from "./adapter";
import type { FolderModelSnapshot } from "./folder-adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";
import type { BrowsingChrome, BrowsingModel, LocalBrowsingClient } from "./collaboration-browsing-adapter";
import type { InteractionSeed, LocalInteractionClient, LocalInteractionUpdate } from "./local-interaction-adapter";

export function folderModel(fixture: WorkbenchSnapshot, revision = 1, sceneKey = "accepted-A"): FolderModelSnapshot {
  return { format: "geosolve-folder-model-v1", revision, status: "accepted", mode: "editable",
    model: { project: "{}", design: {} as never, sourceDesignDigest: `digest-${sceneKey}` },
    source: structuredClone(fixture.source), history: { canUndo: fixture.presentation.canUndo, canRedo: fixture.presentation.canRedo },
    problems: [], seed: { sceneKey, selection: "server-selection" } };
}
export async function folderBrowser(fixture: WorkbenchSnapshot, beforeInitialize?: () => Promise<void>) {
  const catalog = await new MockWorkbenchAdapter().toolCatalog();
  let camera = 0, selection = "none", sceneKey = "accepted-A";
  const frame = () => camera === 0 && selection === "none" && sceneKey === "accepted-A" ? fixture.frame : { ...fixture.frame, scene: { ...fixture.frame.scene, provenance: { camera: String(camera), selection, scene: sceneKey } } };
  const update = (selectionChanged = false): LocalInteractionUpdate => ({ frame: frame(), state: { sceneKey, camera, selection }, selectionChanged, serverFrameCompatible: false });
  const local: LocalInteractionClient = {
    construct: vi.fn(async seed => { sceneKey = String(seed.sceneKey); return update(); }),
    replace: vi.fn(async (seed, preserve) => { sceneKey = String(seed.sceneKey); if (!preserve) selection = String(seed.selection); return update(); }),
    state: vi.fn(async () => update().state), dispose: vi.fn(),
    update: vi.fn(async (method, input) => {
      if (["wheel", "resize", "dispatch"].includes(method)) camera++;
      const pointer = input as { phase?: string; buttons?: number; x?: number };
      const changed = method === "pointer" && pointer.phase === "down" && pointer.buttons === 1;
      if (changed) selection = `point-${pointer.x}`;
      if (method === "restoreSelection") selection = String((input as { state: { selection: string } }).state.selection);
      return update(changed);
    }),
  };
  const browsers: LocalBrowsingClient[] = [];
  const createBrowsing = () => {
    let model!: BrowsingModel;
    const chrome = (): BrowsingChrome => ({ explorer: fixture.explorer, navigation: fixture.navigation, dimensions: fixture.dimensions,
      parameters: fixture.parameters, problems: fixture.problems, selection: null, selectedGeometryRole: null,
      authoringDocument: fixture.authoringDocument ?? { authority: "local-inspector", title: "Sketch", description: "", areKeyConstraintsByDefault: false, editable: true } });
    const browser: LocalBrowsingClient = {
      initialize: vi.fn(async (next: BrowsingModel, seed: InteractionSeed) => { await beforeInitialize?.(); model = next; return { kind: "initialized" as const, model, seed, snapshot: structuredClone(fixture), toolCatalog: catalog }; }),
      replace: vi.fn(async next => { model = next; return { kind: "ready" as const, model }; }),
      present: vi.fn(async view => ({ kind: "chrome" as const, model, view, chrome: chrome() })),
      navigate: vi.fn(async (view, _command, payload) => ({ kind: "navigation" as const, model, view, chrome: chrome(), state: { ...view.state, selection: String((payload as { ids?: string[] }).ids?.[0] ?? "row") } })),
      describe: vi.fn(async view => ({ kind: "mutation" as const, model, view, mutation: { mutation: "set_value", declaration: "radius", path: [], value: 12 } as never })),
      dispose: vi.fn(),
    };
    browsers.push(browser); return browser;
  };
  return { local, createBrowsing, browsers, update };
}
