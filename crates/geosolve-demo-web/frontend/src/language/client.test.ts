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
  type TypeScriptLanguageWorkerResponse,
} from "./protocol";

class FakeWorker implements TypeScriptLanguageWorkerPort {
  messages: TypeScriptLanguageRequest[] = [];
  messageListeners = new Set<(event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void>();
  errorListeners = new Set<(event: ErrorEvent) => void>();
  terminated = false;

  postMessage(message: TypeScriptLanguageRequest): void { this.messages.push(message); }
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void)
      | ((event: ErrorEvent) => void),
  ): void {
    if (type === "message") {
      this.messageListeners.add(listener as (event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void);
    } else {
      this.errorListeners.add(listener as (event: ErrorEvent) => void);
    }
  }
  removeEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void)
      | ((event: ErrorEvent) => void),
  ): void {
    if (type === "message") {
      this.messageListeners.delete(listener as (event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void);
    } else {
      this.errorListeners.delete(listener as (event: ErrorEvent) => void);
    }
  }
  terminate(): void { this.terminated = true; }
  respond(response: TypeScriptLanguageWorkerResponse): void {
    for (const listener of this.messageListeners) {
      listener({ data: response } as MessageEvent<TypeScriptLanguageWorkerResponse>);
    }
  }
  fail(message: string): void {
    for (const listener of this.errorListeners) listener({ message } as ErrorEvent);
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

  it("rejects pending analysis on sync refusal and recovers on a newer valid project", async () => {
    const worker = new FakeWorker();
    const unavailable: string[] = [];
    const client = new TypeScriptLanguageWorkerClient(
      worker,
      (error) => unavailable.push(error.message),
    );
    client.sync(project("const rejected = 1;"));
    const pending = client.diagnostics();
    worker.respond({
      protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
      kind: "sync-error",
      project: "project",
      revision: 0,
      typescriptVersion: TYPESCRIPT_LANGUAGE_VERSION,
      error: "TypeScript language-service project exceeds its source limit",
    });

    await expect(pending).rejects.toThrow("exceeds its source limit");
    await expect(client.hover(0)).rejects.toThrow("exceeds its source limit");
    expect(unavailable).toEqual(["TypeScript language-service project exceeds its source limit"]);

    client.sync(project("const recovered = 1;"));
    const recovered = client.diagnostics();
    const request = worker.messages.at(-1)!;
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
    await expect(recovered).resolves.toEqual([]);
    client.dispose();
  });

  it("settles pending analysis and reports an unavailable crashed worker", async () => {
    const worker = new FakeWorker();
    const unavailable: string[] = [];
    const client = new TypeScriptLanguageWorkerClient(
      worker,
      (error) => unavailable.push(error.message),
    );
    client.sync(project("const pending = 1;"));
    const pending = client.signature(5);

    worker.fail("language worker crashed");

    await expect(pending).rejects.toThrow("language worker crashed");
    await expect(client.diagnostics()).rejects.toThrow("language worker crashed");
    expect(unavailable).toEqual(["language worker crashed"]);
    client.dispose();
  });
});

function project(contents: string) {
  return {
    key: "project",
    file: "sketch.ts",
    files: [{ path: "sketch.ts", contents }],
  };
}
