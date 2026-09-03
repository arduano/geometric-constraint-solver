// SPDX-License-Identifier: GPL-3.0-or-later

import ts from "typescript";

import { compileManagedSource } from "../dist/src/managed.js";

const PINNED_DENO_VERSION = "2.9.4";
const PINNED_TYPESCRIPT_VERSION = "5.9.2";
const MAX_INPUT_BYTES = 64 * 1024 * 1024;

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function compilerInput(value) {
  if (!isRecord(value)) {
    throw new TypeError("managed sketch compiler input must be an object");
  }
  const unknown = Object.keys(value).filter((key) =>
    key !== "source" && key !== "patches"
  );
  if (unknown.length !== 0) {
    throw new TypeError(
      `managed sketch compiler input has unknown field ${
        JSON.stringify(unknown[0])
      }`,
    );
  }
  if (typeof value.source !== "string") {
    throw new TypeError("managed sketch compiler input source must be a string");
  }
  if (value.patches === undefined) return { source: value.source };
  if (!isRecord(value.patches)) {
    throw new TypeError("managed sketch compiler input patches must be an object");
  }
  const patches = Object.create(null);
  for (const [binding, plan] of Object.entries(value.patches)) {
    if (!isRecord(plan)) {
      throw new TypeError(
        `managed sketch patch ${JSON.stringify(binding)} must be an object`,
      );
    }
    patches[binding] = plan;
  }
  return { source: value.source, patches };
}

async function readInput() {
  const chunks = [];
  let total = 0;
  for await (const chunk of Deno.stdin.readable) {
    total += chunk.byteLength;
    if (total > MAX_INPUT_BYTES) {
      throw new TypeError(
        `managed sketch compiler input exceeds ${MAX_INPUT_BYTES} bytes`,
      );
    }
    chunks.push(chunk);
  }
  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

async function main() {
  if (Deno.version.deno !== PINNED_DENO_VERSION) {
    throw new Error(
      `managed sketch compiler requires Deno ${PINNED_DENO_VERSION}; found ${Deno.version.deno}`,
    );
  }
  if (ts.version !== PINNED_TYPESCRIPT_VERSION) {
    throw new Error(
      `managed sketch compiler requires TypeScript ${PINNED_TYPESCRIPT_VERSION}; found ${ts.version}`,
    );
  }
  const input = compilerInput(JSON.parse(await readInput()));
  const compiled = compileManagedSource(
    input.source,
    input.patches === undefined ? {} : { patches: input.patches },
  );
  await Deno.stdout.write(
    new TextEncoder().encode(`${JSON.stringify(compiled)}\n`),
  );
}

try {
  await main();
} catch (error) {
  const detail = error instanceof Error
    ? {
      name: error.name,
      message: error.message,
      ...(isRecord(error.span) ? { span: error.span } : {}),
    }
    : { name: "Error", message: String(error) };
  console.error(JSON.stringify({ error: detail }));
  Deno.exitCode = 1;
}
