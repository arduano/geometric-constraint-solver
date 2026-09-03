// SPDX-License-Identifier: GPL-3.0-or-later

import ts from "typescript";

import { compileManagedSource } from "../dist/src/managed.js";

const PINNED_DENO_VERSION = "2.9.4";
const PINNED_TYPESCRIPT_VERSION = "5.9.2";
const MAX_INPUT_BYTES = 64 * 1024 * 1024;
const MAX_ENTRIES = 8;

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

async function readInput() {
  const chunks = [];
  let total = 0;
  for await (const chunk of Deno.stdin.readable) {
    total += chunk.byteLength;
    if (total > MAX_INPUT_BYTES) {
      throw new TypeError(`managed compiler batch exceeds ${MAX_INPUT_BYTES} bytes`);
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

function batchInput(value) {
  if (!isRecord(value) || Object.keys(value).some((key) => key !== "entries")) {
    throw new TypeError("managed compiler batch must contain only entries");
  }
  if (!Array.isArray(value.entries) || value.entries.length > MAX_ENTRIES) {
    throw new TypeError(`managed compiler batch entries must be an array of at most ${MAX_ENTRIES}`);
  }
  const seen = new Set();
  return value.entries.map((entry) => {
    if (!isRecord(entry) || Object.keys(entry).some((key) => key !== "id" && key !== "source")) {
      throw new TypeError("managed compiler batch entry has an invalid shape");
    }
    if (!Number.isSafeInteger(entry.id) || entry.id < 0 || seen.has(entry.id)) {
      throw new TypeError("managed compiler batch entry ID is invalid or duplicated");
    }
    if (typeof entry.source !== "string") {
      throw new TypeError("managed compiler batch entry source must be text");
    }
    seen.add(entry.id);
    return entry;
  });
}

function errorDetail(error) {
  return error instanceof Error
    ? {
      name: error.name,
      message: error.message,
      ...(isRecord(error.span) ? { span: error.span } : {}),
    }
    : { name: "Error", message: String(error) };
}

async function main() {
  if (Deno.version.deno !== PINNED_DENO_VERSION) {
    throw new Error(`managed compiler requires Deno ${PINNED_DENO_VERSION}; found ${Deno.version.deno}`);
  }
  if (ts.version !== PINNED_TYPESCRIPT_VERSION) {
    throw new Error(`managed compiler requires TypeScript ${PINNED_TYPESCRIPT_VERSION}; found ${ts.version}`);
  }
  const entries = batchInput(JSON.parse(await readInput()));
  const output = entries.map((entry) => {
    try {
      return { id: entry.id, compiled: compileManagedSource(entry.source) };
    } catch (error) {
      return { id: entry.id, error: errorDetail(error) };
    }
  });
  await Deno.stdout.write(
    new TextEncoder().encode(`${JSON.stringify({ entries: output })}\n`),
  );
}

try {
  await main();
} catch (error) {
  console.error(JSON.stringify({ error: errorDetail(error) }));
  Deno.exitCode = 1;
}
