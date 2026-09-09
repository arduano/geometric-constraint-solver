#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
import { readFileSync, writeFileSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { initProject, serveProject, checkProject, bakeProject } from "./file-workspace.mjs";
import { readWorkspaceSnapshot } from "./workspace-loader.mjs";

function parseFlags(args) {
  const flags = {};
  for (let i = 0; i < args.length; i += 2) {
    const name = args[i];
    if (!name.startsWith("--") || !args[i + 1] || Object.hasOwn(flags, name.slice(2))) throw Error(`Expected unique --flag value pairs; received ${name}`);
    flags[name.slice(2)] = args[i + 1];
  }
  return flags;
}
function jsonFile(path) {
  if (!path) throw Error("A JSON input file is required");
  return JSON.parse(readFileSync(resolve(path), "utf8"));
}
async function connect(folder, flags) {
  const session = jsonFile(resolve(folder, ".geosolve/session.json"));
  if (realpathSync(session.folder) !== realpathSync(folder)) throw Error("Bridge session belongs to another folder");
  const origin = new URL(session.origin);
  if (origin.protocol !== "http:" || origin.hostname !== "127.0.0.1" || origin.username || origin.password) throw Error("Expected a local folder bridge");
  const clientId = flags.client ?? `cli-${randomUUID()}`;
  const rpc = async (method, input, basis, operationId) => {
    const response = await fetch(new URL("/api/rpc", origin), { method: "POST", headers: {
      Authorization: `Bearer ${session.token}`, "Content-Type": "application/json",
    }, body: JSON.stringify({ method, input, clientId, authority: basis?.authority, baseHash: basis?.currentHash, operationId }), signal: AbortSignal.timeout(300000) });
    const body = await response.json();
    if (!response.ok) throw Object.assign(Error(body.error ?? `Bridge request failed (${response.status})`), { details: body });
    return body;
  };
  return { rpc, clientId };
}

/** Machine-readable commands. Explicit expected state and operation IDs make agent retries reviewable. */
export async function runCli(args) {
  const [command, folderArg, ...rest] = args;
  if (!command || !folderArg) throw Error("Usage: geosolve init|serve|inspect|check|status|takeover|set|apply|outcome|recover|bake <folder> [--flag value]");
  const folder = resolve(folderArg);
  const flags = parseFlags(rest);
  if (command === "init") return initProject(folder);
  if (command === "serve") {
    const session = await serveProject(folder, { port: Number(flags.port ?? 0) });
    for (const signal of ["SIGINT", "SIGTERM"]) process.once(signal, () => { void session.close().then(() => process.exit(0)); });
    return { ok: true, url: session.url, folder, pid: process.pid };
  }
  if (command === "check") {
    return checkProject(folder);
  }
  if (command === "bake") return bakeProject(folder, flags.out, Number(flags["chord-error-mm"]), flags.output);
  if (command === "inspect") {
    const snapshot = readWorkspaceSnapshot(folder);
    return { ok: true, folder, entry: snapshot.entry, mode: snapshot.mode, revision: snapshot.revision,
      sourceHash: snapshot.sourceHash, toolchain: snapshot.toolchain, inputs: snapshot.inputs,
      files: snapshot.files.map(({ path, sha256, contents }) => ({ path, sha256, bytes: Buffer.byteLength(contents), ...(flags.contents === "true" ? { contents } : {}) })) };
  }
  const { rpc, clientId } = await connect(folder, flags);
  if (command === "status") {
    const observed = await rpc("snapshot");
    const result = { ok: true, clientId, state: observed.state, parameters: observed.result.parameters };
    if (flags.out) writeFileSync(resolve(flags.out), JSON.stringify(result, null, 2) + "\n");
    return result;
  }
  if (command === "outcome") return { ok: true, operation: await rpc("operation.outcome", { operationId: flags.operation }) };
  if (command === "recover" && !flags.resolve) return { ok: true, recovery: await rpc("recovery.inspect") };
  const expected = jsonFile(flags.expected);
  const basis = expected.state ?? expected;
  if (!flags.client) throw Error("Mutation requires --client <stable client ID> matching the inspected session");
  if (command === "takeover") return { ok: true, clientId, ...await rpc("session.takeover", undefined, basis) };
  if (!flags.operation) throw Error("Mutation requires --operation <unique intent ID>; reuse it only for the same retry");
  if (command === "set") {
    const values = flags.values ? jsonFile(flags.values) : { [flags.name]: JSON.parse(flags.value) };
    if (!flags.values && !flags.name) throw Error("set requires --values <JSON file> or --name <input> --value <JSON value>");
    if (basis.mode === "editable") {
      if (flags.values || typeof values[flags.name] !== "number") throw Error("Editable set requires one --name parameter ID and numeric --value");
      return { ok: true, clientId, ...await rpc("dispatch", { version: 2, command: "parameter.edit", payload: { id: flags.name, value: values[flags.name] } }, basis, flags.operation) };
    }
    return { ok: true, clientId, ...await rpc("inputs.set", { values }, basis, flags.operation) };
  }
  if (command === "apply") return { ok: true, clientId, ...await rpc("files.apply", { files: jsonFile(flags.files) }, basis, flags.operation) };
  if (command === "recover") return { ok: true, ...await rpc("recovery.resolve", jsonFile(flags.resolve), basis, flags.operation) };
  throw Error(`Unknown command ${command}`);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runCli(process.argv.slice(2)).then((result) => { console.log(JSON.stringify(result, null, 2)); if (result.ok === false) process.exitCode = 1; },
    (error) => { console.error(JSON.stringify({ ok: false, error: String(error), ...error.details }, null, 2)); process.exitCode = 1; });
}
