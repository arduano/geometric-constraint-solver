// SPDX-License-Identifier: GPL-3.0-or-later

import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync, realpathSync } from "node:fs";
import { dirname, extname, isAbsolute, relative, resolve, sep } from "node:path";
import { Worker } from "node:worker_threads";
import { sdkRoot, sdkDirectory as defaultSdkDirectory, typescriptModuleUrl, esbuildRoot } from "./workspace-runtime-paths.mjs";

const { default: ts } = await import(typescriptModuleUrl);
const sidecars = [".geosolve/inputs.json", ".geosolve/design.json"];
const maxFiles = 512;
const maxFileBytes = 4 * 1024 * 1024;
const maxBytes = 16 * 1024 * 1024;
const sdkName = "@geosolve/sketch-code";
const digest = (value) => createHash("sha256").update(value).digest("hex");

export class WorkspaceLoadError extends Error {
  constructor(code, detail, location = {}) {
    super(location.path ? `${location.path}${location.line ? `:${location.line}:${location.column ?? 1}` : ""}: ${detail}` : detail);
    this.name = "WorkspaceLoadError";
    this.code = code;
    this.detail = detail;
    Object.assign(this, location);
  }
}

function fail(code, detail, path, node, sourceFile) {
  const position = node && sourceFile ? sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)) : null;
  throw new WorkspaceLoadError(code, detail, { ...(path ? { path } : {}), ...(position ? { line: position.line + 1, column: position.character + 1 } : {}) });
}

function localPath(folder, path) {
  if (typeof path !== "string" || !path || path.includes("\\") || path.includes("\0") || isAbsolute(path)) fail("invalid_project", "Expected a relative local file path", path);
  const absolute = resolve(folder, path);
  const normalized = relative(folder, absolute);
  if (!normalized || normalized === ".." || normalized.startsWith(`..${sep}`)) fail("invalid_project", "Project file escapes its folder", path);
  let cursor = folder;
  for (const part of normalized.split(sep)) {
    cursor = resolve(cursor, part);
    let stat;
    try { stat = lstatSync(cursor); } catch (error) { if (error.code === "ENOENT") return { absolute, path: normalized.split(sep).join("/") }; throw error; }
    if (stat.isSymbolicLink()) fail("invalid_project", "Project files and parent directories cannot be symlinks", path);
  }
  return { absolute, path: normalized.split(sep).join("/") };
}

function readFile(folder, path, optional = false) {
  const local = localPath(folder, path);
  let stat;
  try { stat = lstatSync(local.absolute); } catch (error) { if (optional && error.code === "ENOENT") return null; fail("invalid_project", `Cannot read project file: ${error.message}`, path); }
  if (!stat.isFile() || stat.size > maxFileBytes) fail("invalid_project", "Expected a regular file of at most 4 MiB", path);
  const bytes = readFileSync(local.absolute);
  if (bytes.byteLength > maxFileBytes) fail("invalid_project", "Project file exceeds 4 MiB", path);
  let contents;
  try { contents = new TextDecoder("utf-8", { fatal: true }).decode(bytes); } catch { fail("invalid_project", "Project file is not valid UTF-8", path); }
  return { path: local.path, contents, sha256: digest(bytes) };
}

function parseJson(file) {
  try { return JSON.parse(file.contents); } catch (error) { fail("invalid_project", `Invalid JSON: ${error.message}`, file.path); }
}

function imports(file) {
  const source = ts.createSourceFile(file.path, file.contents, ts.ScriptTarget.ES2022, true);
  if (source.parseDiagnostics.length) {
    const diagnostic = source.parseDiagnostics[0];
    const position = source.getLineAndCharacterOfPosition(diagnostic.start ?? 0);
    throw new WorkspaceLoadError("invalid_project", ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"), { path: file.path, line: position.line + 1, column: position.character + 1 });
  }
  const found = [];
  function visit(node) {
    let specifier;
    if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) specifier = node.moduleSpecifier;
    if (ts.isImportEqualsDeclaration(node)) fail("invalid_project", "Use ordinary TypeScript imports", file.path, node, source);
    if (ts.isCallExpression(node) && (node.expression.kind === ts.SyntaxKind.ImportKeyword || ts.isIdentifier(node.expression) && node.expression.text === "require")) {
      if (node.expression.kind !== ts.SyntaxKind.ImportKeyword || node.arguments.length !== 1 || !ts.isStringLiteralLike(node.arguments[0])) fail("invalid_project", "Dependency imports must use a literal module specifier", file.path, node, source);
      specifier = node.arguments[0];
    }
    if (specifier) {
      if (!ts.isStringLiteralLike(specifier)) fail("invalid_project", "Dependency imports must use a literal module specifier", file.path, node, source);
      found.push({ specifier: specifier.text, node, source });
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  return found;
}

function resolveImport(folder, importer, imported, overrides) {
  if (imported.specifier === sdkName) return sdkName;
  if (!imported.specifier.startsWith("./") && !imported.specifier.startsWith("../")) fail("invalid_project", `Unsupported project import ${imported.specifier}; only local files and ${sdkName} are available`, importer, imported.node, imported.source);
  const base = relative(folder, resolve(folder, dirname(importer), imported.specifier));
  const extension = extname(base);
  const candidates = extension ? [base, ...(extension === ".js" ? [base.slice(0, -3) + ".ts"] : extension === ".mjs" ? [base.slice(0, -4) + ".mts"] : [])] : [base + ".ts", base + ".mts", base + ".js", `${base}/index.ts`];
  for (const candidate of candidates) {
    const local = localPath(folder, candidate);
    if (overrides?.has(local.path)) return local.path;
    let stat;
    try { stat = lstatSync(local.absolute); } catch (error) { if (error.code === "ENOENT") continue; throw error; }
    if (stat.isFile() && [".ts", ".mts", ".js", ".mjs"].includes(extname(candidate))) return local.path;
  }
  fail("invalid_project", `Cannot resolve local import ${imported.specifier}`, importer, imported.node, imported.source);
}

function toolIdentity(sdkDirectory) {
  const sourceFiles = readdirSync(resolve(sdkRoot, "src")).filter((path) => path.endsWith(".ts")).sort();
  const executableFiles = readdirSync(sdkDirectory).filter((path) => path.endsWith(".js")).sort();
  return digest(JSON.stringify({
    source: sourceFiles.map((path) => [path, digest(readFileSync(resolve(sdkRoot, "src", path)))]),
    executable: executableFiles.map((path) => [path, digest(readFileSync(resolve(sdkDirectory, path)))]),
    typescript: ts.version,
    esbuild: JSON.parse(readFileSync(resolve(esbuildRoot, "package.json"), "utf8")).version,
  }));
}

function freeze(value) {
  if (value && typeof value === "object") { for (const child of Object.values(value)) freeze(child); Object.freeze(value); }
  return value;
}

/** Capture every consumed local byte before executing trusted code in a worker. */
export function readWorkspaceSnapshot(folder, { inputs, sidecarPaths = sidecars, sdkDirectory = defaultSdkDirectory, fileOverrides } = {}) {
  folder = realpathSync(folder);
  const overrides = new Map();
  for (const [path, contents] of Object.entries(fileOverrides ?? {})) {
    const local = localPath(folder, path);
    if (typeof contents !== "string" || Buffer.byteLength(contents) > maxFileBytes) fail("invalid_project", "Candidate files require UTF-8 text of at most 4 MiB", path);
    overrides.set(local.path, contents);
  }
  const read = (path, optional = false) => overrides.has(path)
    ? { path, contents: overrides.get(path), sha256: digest(overrides.get(path)) } : readFile(folder, path, optional);
  const files = new Map();
  let bytes = 0;
  function include(path, optional = false) {
    if (files.has(path)) return files.get(path);
    const file = read(path, optional);
    if (!file) return null;
    bytes += Buffer.byteLength(file.contents);
    if (files.size >= maxFiles || bytes > maxBytes) fail("invalid_project", "Project dependency snapshot exceeds its 512 file / 16 MiB bound", path);
    files.set(file.path, file);
    return file;
  }
  const manifestFile = include("geosolve.json");
  const manifest = parseJson(manifestFile);
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) fail("invalid_project", "Project manifest must be an object", "geosolve.json");
  const legacy = manifest.format === "geosolve-folder-v1";
  if ((!legacy && manifest.format !== "geosolve-folder-v2") || (legacy && manifest.entry !== "sketch.ts") || (!legacy && !["editable", "generator"].includes(manifest.mode)) || Object.keys(manifest).some((key) => !(legacy ? ["format", "entry"] : ["format", "entry", "mode"]).includes(key))) fail("invalid_project", "Expected geosolve-folder-v1 or geosolve-folder-v2 with entry and editable/generator mode", "geosolve.json");
  const entry = localPath(folder, manifest.entry).path;
  if (![".ts", ".mts"].includes(extname(entry))) fail("invalid_project", "Project entry must be TypeScript", entry);
  const dependencies = {};
  function collect(path) {
    if (Object.hasOwn(dependencies, path)) return;
    const file = include(path);
    dependencies[path] = {};
    for (const imported of imports(file)) {
      const target = resolveImport(folder, path, imported, overrides);
      dependencies[path][imported.specifier] = target;
      if (target !== sdkName) collect(target);
    }
  }
  collect(entry);
  const absent = [];
  for (const path of sidecarPaths) if (!include(path, true)) absent.push(path);
  const savedInputs = files.get(".geosolve/inputs.json");
  const parameters = inputs ?? (savedInputs ? parseJson(savedInputs) : {});
  if (!parameters || typeof parameters !== "object" || Array.isArray(parameters)) fail("invalid_project", "Generator inputs must be an object", ".geosolve/inputs.json");
  let parameterJson;
  try { parameterJson = JSON.stringify(parameters, (_key, value) => {
    if (value === undefined || typeof value === "number" && !Number.isFinite(value) || typeof value === "function" || typeof value === "symbol" || typeof value === "bigint") throw Error("non-data input");
    return value;
  }); } catch { fail("invalid_project", "Generator inputs must be finite JSON data", ".geosolve/inputs.json"); }
  if (Buffer.byteLength(parameterJson) > maxFileBytes) fail("invalid_project", "Generator input byte bound exceeded");
  const orderedFiles = [...files.values()].sort((a, b) => a.path.localeCompare(b.path, "en"));
  for (const file of orderedFiles) if (read(file.path)?.sha256 !== file.sha256) fail("conflict", "Project files changed while taking the dependency snapshot", file.path);
  for (const path of absent) if (read(path, true)) fail("conflict", "Project sidecar appeared while taking the dependency snapshot", path);
  const toolchain = toolIdentity(sdkDirectory);
  const snapshot = { format: "geosolve-workspace-inputs-v1", folder, manifest, entry, mode: legacy ? "editable" : manifest.mode, files: orderedFiles, absent, dependencies, sourceHash: files.get(entry).sha256, inputs: JSON.parse(parameterJson), toolchain };
  snapshot.revision = digest(JSON.stringify({ files: orderedFiles.map(({ path, sha256 }) => [path, sha256]), absent, inputs: snapshot.inputs, toolchain }));
  return freeze(snapshot);
}

/** Workers bound/cancel trusted project execution; they are not a security sandbox. */
export function evaluateWorkspaceSnapshot(snapshot, { signal, timeoutMs = 15000, sdkDirectory = defaultSdkDirectory } = {}) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 300000) throw new TypeError("loader timeout must be 1..300000 ms");
  if (signal?.aborted) return Promise.reject(new WorkspaceLoadError("cancelled", "Workspace evaluation cancelled"));
  if (toolIdentity(sdkDirectory) !== snapshot.toolchain) return Promise.reject(new WorkspaceLoadError("conflict", "SDK/compiler files changed after the workspace snapshot was captured"));
  return new Promise((resolveResult, reject) => {
    const worker = new Worker(new URL("./workspace-loader-worker.mjs", import.meta.url), { workerData: { snapshot, sdkDirectory }, execArgv: [], stdout: true, stderr: true, resourceLimits: { maxOldGenerationSizeMb: 256 } });
    worker.stdout.resume(); worker.stderr.resume();
    let finished = false;
    const done = (error, result) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
      void worker.terminate();
      if (error) reject(error);
      else {
        try {
          if (toolIdentity(sdkDirectory) !== snapshot.toolchain) throw new WorkspaceLoadError("conflict", "SDK/compiler files changed during workspace evaluation");
          resolveResult(freeze(result));
        } catch (failure) { reject(failure); }
      }
    };
    const abort = () => done(new WorkspaceLoadError("cancelled", "Workspace evaluation cancelled"));
    const timer = setTimeout(() => done(new WorkspaceLoadError("timeout", `Workspace evaluation exceeded ${timeoutMs} ms`, { path: snapshot.entry })), timeoutMs);
    signal?.addEventListener("abort", abort, { once: true });
    worker.once("message", (message) => message.ok ? done(null, message.result) : done(new WorkspaceLoadError(message.error.code, message.error.detail, message.error.location)));
    worker.once("error", (error) => done(new WorkspaceLoadError("evaluation_failed", error.message, { path: snapshot.entry })));
    worker.once("exit", (code) => { if (!finished) done(new WorkspaceLoadError("evaluation_failed", `Workspace worker exited before completing (code ${code})`, { path: snapshot.entry })); });
  });
}

/** Keep only the newest requested evaluation; publication remains the caller's transaction. */
export function createWorkspaceLoader(options = {}) {
  let controller;
  let generation = 0;
  let disposed = false;
  return {
    async load(folder, request = {}) {
      if (disposed) throw new WorkspaceLoadError("cancelled", "Workspace loader is disposed");
      controller?.abort();
      controller = new AbortController();
      const current = ++generation;
      const snapshot = readWorkspaceSnapshot(folder, { ...options, ...request });
      const result = await evaluateWorkspaceSnapshot(snapshot, { ...options, ...request, signal: request.signal ? AbortSignal.any([request.signal, controller.signal]) : controller.signal });
      if (current !== generation) throw new WorkspaceLoadError("cancelled", "Workspace evaluation superseded");
      return { snapshot, result };
    },
    cancel() { generation += 1; controller?.abort(); },
    dispose() { disposed = true; generation += 1; controller?.abort(); },
  };
}
