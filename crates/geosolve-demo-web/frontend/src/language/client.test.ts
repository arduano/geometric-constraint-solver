// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import {
  TypeScriptLanguageWorkerClient,
  type TypeScriptLanguageWorkerPort,
} from "./client";
import {
  TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
  TYPESCRIPT_LANGUAGE_VERSION,
  type TypeScriptLanguageRequest,
  type TypeScriptLanguageResponse,
} from "./protocol";

class FakeWorker implements TypeScriptLanguageWorkerPort {
  messages: TypeScriptLanguageRequest[] = [];
  listeners = new Set<(event: MessageEvent<TypeScriptLanguageResponse>) => void>();
  terminated = false;

  postMessage(message: TypeScriptLanguageRequest): void { this.messages.push(message); }
  addEventListener(_type: "message", listener: (event: MessageEvent<TypeScriptLanguageResponse>) => void): void { this.listeners.add(listener); }
  removeEventListener(_type: "message", listener: (event: MessageEvent<TypeScriptLanguageResponse>) => void): void { this.listeners.delete(listener); }
  terminate(): void { this.terminated = true; }
  respond(response: TypeScriptLanguageResponse): void {
    for (const listener of this.listeners) listener({ data: response } as MessageEvent<TypeScriptLanguageResponse>);
  }
}

describe("TypeScript language worker client", () => {
  it("revision-gates responses and cancels superseded source analysis", async () => {
    const worker = new FakeWorker();
    const client = new TypeScriptLanguageWorkerClient(worker);
    client.sync(project("const first = 1;"));
    const old = client.diagnostics();
    const oldRequest = worker.messages.at(-1)!;

    client.sync(project("const second = 2;"));
    expect(await old).toBeNull();
    const current = client.diagnostics();
    const request = worker.messages.at(-1)!;
    expect(request).toMatchObject({ kind: "diagnostics", revision: 1 });

    worker.respond({
      protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
      request: (oldRequest as { request: number }).request,
      project: "project",
      revision: 0,
      kind: "diagnostics",
      typescriptVersion: TYPESCRIPT_LANGUAGE_VERSION,
      stale: false,
      result: [{ from: 0, to: 1, severity: "error", message: "stale", code: 1, source: "TypeScript" }],
    });
    worker.respond({
      protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
      request: (request as { request: number }).request,
      project: "project",
      revision: 1,
      kind: "diagnostics",
      typescriptVersion: TYPESCRIPT_LANGUAGE_VERSION,
      stale: false,
      result: [],
    });
    expect(await current).toEqual([]);
    client.dispose();
    expect(worker.terminated).toBe(true);
  });

  it("does not resynchronize byte-identical project state", () => {
    const worker = new FakeWorker();
    const client = new TypeScriptLanguageWorkerClient(worker);
    expect(client.sync(project("const stable = 1;"))).toBe(0);
    expect(client.sync(project("const stable = 1;"))).toBe(0);
    expect(worker.messages.filter((message) => message.kind === "sync")).toHaveLength(1);
    client.dispose();
  });

  it("settles pending and late queries harmlessly when disposed", async () => {
    const worker = new FakeWorker();
    const client = new TypeScriptLanguageWorkerClient(worker);
    client.sync(project("const pending = 1;"));
    const pending = client.hover(6);

    client.dispose();

    expect(await pending).toBeNull();
    expect(await client.diagnostics()).toBeNull();
    expect(worker.terminated).toBe(true);
  });
});

function project(contents: string) {
  return {
    key: "project",
    file: "sketch.ts",
    files: [{ path: "sketch.ts", contents }],
  };
}
