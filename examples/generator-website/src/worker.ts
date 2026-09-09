// SPDX-License-Identifier: GPL-3.0-or-later
import { createEngine } from "@geosolve/engine";
import { footprint } from "./generator";
import type { GenerateRequest, GenerateResponse } from "./protocol";

// Generator execution, Rust solving and profile export all live in this terminable
// worker. No workbench, renderer, editor or UI state is imported by the engine.
self.onmessage = async ({ data }: MessageEvent<GenerateRequest>) => {
  let engine: Awaited<ReturnType<typeof createEngine>> | undefined;
  try {
    const started = performance.now();
    const inputs = footprint.parseInputs(data.inputs);
    engine = await createEngine();
    const accepted = await engine.evaluate({ definition: footprint, parameters: inputs });
    if (accepted.status !== "accepted") throw Error(accepted.diagnostics.map((item) => item.detail).join("; "));
    const profiles = await engine.exportProfiles(accepted, { chordErrorMm: 0.08, output: "/outline" });
    self.postMessage({ revision: data.revision, status: "accepted", profiles, inputs,
      inputDigest: accepted.input_digest, elapsedMs: performance.now() - started,
      curveCount: accepted.geometry.curves.length,
    } satisfies GenerateResponse);
  } catch (error) {
    self.postMessage({ revision: data.revision, status: "rejected", detail: error instanceof Error ? error.message : String(error) } satisfies GenerateResponse);
  } finally { engine?.dispose(); }
};
