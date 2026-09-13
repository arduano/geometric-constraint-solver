// SPDX-License-Identifier: GPL-3.0-or-later
import { beforeAll, describe, expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import initializePresentation, { BrowsingHandle, InteractionHandle } from "../generated/geosolve_demo_web.js";
import initializeEngine, * as engineWasm from "../../../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm.js";
import { createEngine } from "../../../../../packages/geosolve-engine/src/index";
import { assertWorkbenchSnapshot } from "./adapter";
import type { BrowsingInitialization } from "./collaboration-browsing-worker";

beforeAll(async () => {
  await initializePresentation({ module_or_path: await readFile(resolve(process.cwd(), "src/generated/geosolve_demo_web_bg.wasm")) });
});
async function engineFixture() {
  return createEngine({ wasmModule: { ...engineWasm, default: initializeEngine }, wasm: await readFile(resolve(process.cwd(), "../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm_bg.wasm")) });
}

describe("native browsing initialization without an editing workbench", () => {
  it("enriches a bare engine seed and retains native namespace for navigation and dimension edits", async () => {
    const engine = await engineFixture();
    const compiled = JSON.parse(await readFile(resolve(process.cwd(), "../../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"), "utf8"));
    const project = engine.compileProject({ project: "browser-initialization", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
    const session = engine.openEditableSession(project);
    let browsing: BrowsingHandle | undefined, canvas: InteractionHandle | undefined;
    try {
      const seed = session.interactionSeed({ width: 900, height: 700 });
      expect((seed.dimensions as { ids: Record<string, unknown> }).ids).toEqual({});
      browsing = new BrowsingHandle(JSON.stringify({ project: session.exportProject(), design: session.exportDesign(), seed }));
      const initial = JSON.parse(browsing.initialize()) as BrowsingInitialization;
      const snapshot = assertWorkbenchSnapshot(initial.snapshot);
      expect(initial.seed.scene).toBe(seed.scene);
      expect(initial.seed.sceneKey).toBe(seed.sceneKey);
      expect(initial.seed.bindings).toEqual(seed.bindings);
      expect(snapshot.frame.scene.items.length).toBeGreaterThan(0);
      expect(snapshot.source.files[0]?.contents).toBe(compiled.normalizedSource);
      expect(snapshot.explorer.length).toBeGreaterThan(0);
      expect(snapshot.dimensions?.mode).toBe("focused");
      expect(snapshot.dimensions?.allMeasurements?.length).toBeGreaterThan(0);
      const key = Object.values((initial.seed.dimensions as { ids: Record<string, unknown> }).ids);
      expect(key.length).toBeGreaterThan(0);
      canvas = new InteractionHandle(JSON.stringify(initial.seed));
      const before = JSON.parse(canvas.state());
      const selected = JSON.parse(browsing.navigate(JSON.stringify({ state: before, command: "navigation.rows.select", payload: { authority: snapshot.navigation!.authority, ids: ["managed:bar"], mode: "replace" } })));
      expect(selected.state.selection.length).toBeGreaterThan(0);
      canvas.restoreSelection(JSON.stringify({ expected: before, state: selected.state }));
      const updated = JSON.parse(browsing.update(canvas.state()));
      const dimension = updated.dimensions.allMeasurements.find((entry: { editable: boolean }) => entry.editable);
      expect(dimension).toBeDefined();
      const mutation = JSON.parse(browsing.describe(JSON.stringify({ state: JSON.parse(canvas.state()), authority: updated.authoringDocument.authority, command: "dimensions.edit", payload: { id: dimension.id, value: "24" } })));
      expect(mutation).toBeTruthy();
      expect(session.exportProject()).toBe(project);
      canvas.dispatch(JSON.stringify({ version: 2, command: "dimensions.pin", payload: { id: dimension.id, pinned: true } }));
      canvas.dispatch(JSON.stringify({ version: 2, command: "dimensions.mode", payload: { mode: "all" } }));
      canvas.dispatch(JSON.stringify({ version: 2, command: "view.construction.toggle", payload: null }));
      const presentation = JSON.parse(canvas.exportPresentation());
      expect(presentation.dimensions.mode).toBe("all");
      expect(presentation.dimensions.pins).toHaveLength(1);
      expect(presentation.constructionVisible).toBe(false);
      expect(presentation).not.toHaveProperty("selection");
      const restored = new BrowsingHandle(JSON.stringify({ project: session.exportProject(), design: session.exportDesign(), seed, presentation }));
      let restoredCanvas: InteractionHandle | undefined;
      try {
        const startup = JSON.parse(restored.initialize()) as BrowsingInitialization;
        restoredCanvas = new InteractionHandle(JSON.stringify(startup.seed));
        expect(JSON.parse(restoredCanvas.exportPresentation())).toEqual(presentation);
        expect(startup.snapshot.dimensions?.pinCount).toBe(1);
      } finally { restoredCanvas?.free(); restored.free(); }
      expect(() => browsing!.initialize()).toThrow(/already initialized/);
    } finally { canvas?.free(); browsing?.free(); session.dispose(); }
  });

  it("restores saved visibility and dimension intent through the native presentation codec", async () => {
    const engine = await engineFixture();
    const compiled = JSON.parse(await readFile(resolve(process.cwd(), "../../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"), "utf8"));
    const project = engine.compileProject({ project: "browser-saved-view", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
    const session = engine.openEditableSession(project);
    const seed = session.interactionSeed();
    const handle = new BrowsingHandle(JSON.stringify({ project: session.exportProject(), design: session.exportDesign(), seed,
      presentation: { hiddenRows: ["managed:bar", "managed:removed"], constructionVisible: false, dimensions: { mode: "hidden", pins: [] } },
    }));
    try {
      const initial = JSON.parse(handle.initialize()) as BrowsingInitialization;
      const snapshot = assertWorkbenchSnapshot(initial.snapshot);
      expect(snapshot.dimensions?.mode).toBe("hidden");
      expect(snapshot.presentation.constructionVisible).toBe(false);
      expect(initial.seed.visibility).toEqual({ hiddenRows: ["managed:bar"], isolateRestore: null, constructionVisible: false });
      const row = snapshot.explorer.flatMap((entry) => [entry, ...entry.children]).find((entry) => entry.id === "managed:bar");
      expect(row?.visible).toBe(false);
      expect(initial.seed.scene).toBe(seed.scene);
    } finally { handle.free(); session.dispose(); }
  });

  it("initializes generated output with native rows and no managed source authority", async () => {
    const engine = await engineFixture();
    const generated = {
      format: "geosolve-generated-sketch-v1", sdk_abi: "geosolve-sketch-code-v2",
      declarations: [{ identity: ["hole"], family: "geometry.centerRadiusCircle", arguments: { kind: "object", value: {
        center: { kind: "array", value: [{ kind: "number", value: 0 }, { kind: "number", value: 0 }] },
        radius: { kind: "unit", value: { unit: "mm", value: 2 } },
      } } }], applications: [], parameters: [], groups: [], suppressions: [], document: { title: "Generated hole" },
      output: { kind: "reference", value: { identity: ["hole"], path: ["curve"], kind: "curve" } },
    };
    const evaluation = await engine.evaluateGenerated(generated as Parameters<typeof engine.evaluateGenerated>[0]);
    if (evaluation.status !== "accepted") throw Error("generator fixture failed");
    const seed = engine.interactionSeed(evaluation);
    let browsing: BrowsingHandle | undefined, canvas: InteractionHandle | undefined;
    try {
      browsing = new BrowsingHandle(JSON.stringify({ generated: JSON.stringify(generated), seed }));
      const initial = JSON.parse(browsing.initialize()) as BrowsingInitialization;
      const snapshot = assertWorkbenchSnapshot(initial.snapshot);
      expect(snapshot.project.title).toBe("Generated hole");
      expect(snapshot.source.files).toEqual([]);
      expect(snapshot.authoringDocument).toBeUndefined();
      expect(snapshot.frame.scene.items.length).toBeGreaterThan(0);
      expect(snapshot.explorer.length).toBeGreaterThan(0);
      canvas = new InteractionHandle(JSON.stringify(initial.seed));
      canvas.wheel(JSON.stringify({ version: 2, x: 300, y: 250, deltaX: 0, deltaY: -30, ctrl: false }));
      expect(JSON.parse(browsing.update(canvas.state())).explorer.length).toBeGreaterThan(0);
      expect(() => browsing!.describe(JSON.stringify({ state: JSON.parse(canvas!.state()), authority: "none", command: "parameter.edit", payload: { id: "radius", value: "3" } }))).toThrow();
    } finally { canvas?.free(); browsing?.free(); }
  });
});
