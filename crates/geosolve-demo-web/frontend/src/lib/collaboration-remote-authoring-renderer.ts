// SPDX-License-Identifier: GPL-3.0-or-later
import initialize, { renderAuthoringPreview } from "../generated/geosolve_demo_web.js";
import type { AuthoringPreview, AuthoringWorkerResponse } from "./collaboration-authoring-worker";

export interface RemotePaintRequest {
  readonly id: number;
  readonly generation: number;
  readonly presentation: string;
  readonly result: Omit<AuthoringPreview, "frame">;
}

/** Rendering only. This worker never imports the engine or opens a model session. */
export function createRemotePaintHandler(
  ready: Promise<(encoded: string) => string>,
  reply: (response: AuthoringWorkerResponse) => void,
) {
  return ({ data }: MessageEvent<RemotePaintRequest>) => {
    void ready.then(render => {
      const construction = data.result.construction;
      const frame = JSON.parse(render(JSON.stringify({ ...JSON.parse(data.presentation), view: data.result.view,
        construction: construction ? { preview: construction.preview, inference_guides: construction.inference_guides } : null })));
      reply({ id: data.id, generation: data.generation, result: { ...data.result, frame } });
    }).catch(error => reply({ id: data.id, generation: data.generation, error: String(error) }));
  };
}

if (typeof document === "undefined" && typeof self !== "undefined") {
  self.onmessage = createRemotePaintHandler(initialize().then(() => renderAuthoringPreview), response => self.postMessage(response));
}
