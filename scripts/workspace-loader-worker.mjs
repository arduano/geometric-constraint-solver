// SPDX-License-Identifier: GPL-3.0-or-later

import { createHash } from "node:crypto";
import { extname, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parentPort, workerData } from "node:worker_threads";
import * as esbuild from "../crates/geosolve-demo-web/frontend/node_modules/esbuild/lib/main.js";
import ts from "../packages/geosolve-sketch-code/node_modules/typescript/lib/typescript.js";

const { snapshot, sdkDirectory } = workerData;
const files = new Map(snapshot.files.map((file) => [file.path, file]));
const sdkUrl = pathToFileURL(resolve(sdkDirectory, "index.js")).href;
const hash = (contents) => createHash("sha256").update(contents).digest("hex");

async function bundle(entry) {
  const build = await esbuild.build({
    entryPoints: [entry], bundle: true, write: false, platform: "node", format: "esm", target: "es2022", sourcemap: "inline", logLevel: "silent",
    plugins: [{ name: "captured-workspace", setup(builder) {
      builder.onResolve({ filter: /.*/ }, (args) => {
        if (args.kind === "entry-point") return { path: entry, namespace: "workspace" };
        if (args.path === "@geosolve/sketch-code") return { path: sdkUrl, external: true };
        const target = snapshot.dependencies[args.importer]?.[args.path];
        if (!target || !files.has(target)) throw Error(`Import ${args.path} from ${args.importer} is absent from the captured snapshot`);
        return { path: target, namespace: "workspace" };
      });
      builder.onLoad({ filter: /.*/, namespace: "workspace" }, ({ path }) => ({ contents: files.get(path).contents, loader: [".js", ".mjs"].includes(extname(path)) ? "js" : "ts" }));
    } }],
  });
  const emitted = build.outputFiles[0].text;
  if (Buffer.byteLength(emitted) > 32 * 1024 * 1024) throw Error("Bundled project exceeds its output byte limit");
  // All imports resolve either snapshot bytes or the exact installed trusted SDK.
  return `data:text/javascript;base64,${Buffer.from(emitted).toString("base64")}`;
}

function patchImports() {
  const file = files.get(snapshot.entry);
  const source = ts.createSourceFile(file.path, file.contents, ts.ScriptTarget.ES2022, true);
  return source.statements.filter(ts.isImportDeclaration).filter((node) => node.moduleSpecifier.text !== "@geosolve/sketch-code").map((node) => {
    const moduleSpecifier = node.moduleSpecifier.text;
    const bindings = node.importClause?.namedBindings;
    if (!bindings || !ts.isNamedImports(bindings) || node.importClause.name || bindings.elements.length !== 1 || bindings.elements.some((binding) => binding.propertyName || binding.isTypeOnly)) throw Error(`Editable patch import ${moduleSpecifier} requires one unaliased named patch export`);
    if (!moduleSpecifier.startsWith("./")) throw Error(`Editable patch import ${moduleSpecifier} must be relative to its entry`);
    return { moduleSpecifier, exportName: bindings.elements[0].name.text, file: snapshot.dependencies[snapshot.entry][moduleSpecifier] };
  });
}

async function evaluate() {
  const result = { status: "compiled", mode: snapshot.mode, revision: snapshot.revision, sourceHash: snapshot.sourceHash, entry: snapshot.entry };
  if (snapshot.mode === "generator") {
    const url = await bundle(snapshot.entry);
    esbuild.stop();
    const module = await import(url);
    const sdk = await import(sdkUrl);
    const definition = module.default;
    const inputs = typeof definition?.parseInputs === "function" ? definition.parseInputs(snapshot.inputs) : snapshot.inputs;
    const authored = typeof definition === "function" ? await definition(inputs) : definition;
    if (typeof definition !== "function" && Object.keys(inputs).length) throw Error("Generator input values require a default exported function");
    return { ...result, generated: sdk.recordedSketch(authored), inputDefinitions: definition?.inputs ?? null, inputs };
  }
  const plans = {};
  const artifacts = {};
  const customFiles = {};
  const modules = {};
  const imports = patchImports();
  const urls = await Promise.all(imports.map(async (patch) => ({ ...patch, url: await bundle(patch.file) })));
  esbuild.stop();
  const compiler = await import(pathToFileURL(resolve(sdkDirectory, "compiler.js")).href);
  const managed = await import(pathToFileURL(resolve(sdkDirectory, "managed.js")).href);
  for (const patch of urls) {
    const evaluated = await import(patch.url);
    const source = files.get(patch.file).contents;
    const compiled = compiler.compilePatchArtifact({ source, moduleSpecifier: patch.moduleSpecifier, exportName: patch.exportName, patch: evaluated[patch.exportName] });
    if (Object.hasOwn(plans, patch.exportName) || Object.hasOwn(modules, patch.moduleSpecifier)) throw Error("Editable patch imports must have unique export names and modules");
    plans[patch.exportName] = compiled.artifact;
    artifacts[compiled.artifactDigest] = compiled.artifact;
    const path = patch.moduleSpecifier.slice(2);
    customFiles[path] = { path, contents: source, source_digest: hash(source), managed: false };
    modules[patch.moduleSpecifier] = { artifact: compiled.artifactDigest, source: compiled.artifact.source_digest, interface: compiled.artifact.interface_digest, sdk_abi: compiled.artifact.sdk_abi };
  }
  const source = files.get(snapshot.entry).contents;
  const compiled = managed.compileManagedSource(source, { patches: plans });
  return { ...result, compiled, customFiles, artifacts, lock: { format: "geosolve-lock-v1", modules }, patches: plans };
}

try {
  const result = await evaluate();
  if (Buffer.byteLength(JSON.stringify(result)) > 32 * 1024 * 1024) throw Error("Workspace evaluation exceeds its output byte limit");
  parentPort.postMessage({ ok: true, result });
} catch (error) {
  const buildError = error.errors?.[0];
  const location = buildError?.location;
  const source = files.get(snapshot.entry)?.contents ?? "";
  const span = error.span;
  const offset = span ? Buffer.from(source).subarray(0, span.start).toString("utf8") : "";
  parentPort.postMessage({ ok: false, error: { code: "evaluation_failed", detail: buildError?.text ?? error.message ?? String(error), location: {
    path: location?.file?.replace(/^workspace:/u, "") ?? snapshot.entry,
    ...(location ? { line: location.line, column: location.column + 1 } : span ? { line: offset.split("\n").length, column: [...offset.split("\n").at(-1)].length + 1 } : {}),
  } } });
} finally {
  esbuild.stop();
}
