// SPDX-License-Identifier: GPL-3.0-or-later
import { parentPort, workerData } from "node:worker_threads";
import { engineModuleUrl } from "./workspace-runtime-paths.mjs";

const { createEngine } = await import(engineModuleUrl);

let engine;
try {
  engine = await createEngine();
  const { compiled, design, profiles } = workerData;
  let result, project, semanticDesign;
  if (compiled.mode === "generator") result = await engine.evaluateGenerated(compiled.generated);
  else {
    project = engine.compileProject({ project: "code-authored-sketch", compiled: compiled.compiled,
      customFiles: compiled.customFiles, artifacts: compiled.artifacts, lock: compiled.lock });
    if (design) {
      const session = engine.openEditableSession(project, { design });
      result = session.accepted;
      semanticDesign = session.exportDesign();
      session.dispose();
    } else result = await engine.evaluateEditable(project);
  }
  if (result.status !== "accepted") throw Error(result.diagnostics.map((item) => item.detail).join("; "));
  const geometry = profiles ? await engine.exportProfiles(result, profiles) : undefined;
  const response = { result, project, design: semanticDesign, profiles: geometry, inputDefinitions: compiled.inputDefinitions, inputs: compiled.inputs };
  if (Buffer.byteLength(JSON.stringify(response)) > 64 * 1024 * 1024) throw Error("Accepted output exceeds 64 MiB");
  parentPort.postMessage({ ok: true, result: response });
} catch (error) { parentPort.postMessage({ ok: false, error: String(error) }); }
finally { engine?.dispose(); }
