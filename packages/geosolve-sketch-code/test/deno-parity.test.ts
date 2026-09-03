// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { fillets as mappedFillets } from "../examples/typed-panel.patch.js";
import {
  type PatchArtifactPlan,
  recordPatchArtifact,
} from "../src/compiler.js";
import {
  type CompiledManagedSource,
  compileManagedSource,
} from "../src/managed.js";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const denoCompiler = "scripts/compile-managed-deno.mjs";
const compilerEnvelopeSource = readFileSync(
  resolve(packageRoot, "test/fixtures/managed-compiler-envelope.sketch.ts"),
  "utf8",
);
const compilerEnvelopeFixture = readFileSync(
  resolve(packageRoot, "test/fixtures/managed-compiler-envelope.json"),
  "utf8",
);

interface DenoCompilerInput {
  readonly source: string;
  readonly patches?: Readonly<Record<string, PatchArtifactPlan>>;
}

function compileWithDeno(input: DenoCompilerInput): CompiledManagedSource {
  const result = spawnSync("deno", [
    "run",
    "--no-config",
    "--no-lock",
    "--no-prompt",
    "--cached-only",
    "--no-remote",
    "--node-modules-dir=manual",
    "--ignore-env",
    denoCompiler,
  ], {
    cwd: packageRoot,
    encoding: "utf8",
    input: JSON.stringify(input),
    maxBuffer: 128 * 1024 * 1024,
  });
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(
    result.signal,
    null,
    `Deno compiler terminated with ${result.signal}`,
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stderr, "");
  return JSON.parse(result.stdout) as CompiledManagedSource;
}

function parityEnvelope(compiled: CompiledManagedSource) {
  return {
    normalizedSource: compiled.normalizedSource,
    ir: compiled.ir,
    artifact: compiled.artifact,
    canonicalIrJson: compiled.canonicalIrJson,
    canonicalArtifactJson: compiled.canonicalArtifactJson,
    digests: {
      source: compiled.ir.source_digest,
      ir: compiled.ir.ir_digest,
      artifactSource: compiled.artifact.source_digest,
      artifactIr: compiled.artifact.ir_digest,
      artifact: compiled.artifact.artifact_digest,
    },
  };
}

test("pinned Deno and Node exactly reproduce the managed sketch compiler envelope fixture", () => {
  const node = compileManagedSource(compilerEnvelopeSource);
  const deno = compileWithDeno({ source: compilerEnvelopeSource });
  assert.equal(JSON.stringify(node), compilerEnvelopeFixture);
  assert.equal(JSON.stringify(deno), compilerEnvelopeFixture);
  assert.deepEqual(parityEnvelope(deno), parityEnvelope(node));
});

test("pinned Deno and Node compile identical generated-member patch envelopes", () => {
  const source = `"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { fillets } from "./patches/fillet-record.patch.ts";

export default sketch(($) => {
  const panel = $.geometry.polyline("panel", {
    vertices: [
      { key: "lowerLeft", position: [0, 0] },
      { key: "lowerRight", position: [80, 0] },
      { key: "upperRight", position: [80, 40] },
      { key: "upperLeft", position: [0, 40] },
    ],
    closed: true,
  });
  const shared = mm(4);
  const corners = $.use("cornerFillets", fillets, {
    corners: {
      lowerLeft: panel.filletableCorners.byKey.lowerLeft,
      upperRight: panel.filletableCorners.byKey.upperRight,
    },
    radius: shared,
  });
  $.group("Rounded corners", [panel, corners]);
  $.suppress(corners.fillets.lowerLeft);
  return { panel, corners };
});
`;
  const patchPlan = JSON.parse(
    JSON.stringify(recordPatchArtifact(mappedFillets)),
  ) as PatchArtifactPlan;
  const input = { source, patches: { fillets: patchPlan } };
  const node = compileManagedSource(input.source, { patches: input.patches });
  const deno = compileWithDeno(input);
  assert.ok(node.artifact.generated_members.length > 0);
  assert.deepEqual(parityEnvelope(deno), parityEnvelope(node));
});
