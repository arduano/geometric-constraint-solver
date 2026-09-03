// SPDX-License-Identifier: GPL-3.0-or-later

import {
  TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
  TYPESCRIPT_LANGUAGE_VERSION,
  type TypeScriptLanguageCompletionResult,
  type TypeScriptLanguageDiagnostic,
  type TypeScriptLanguageFile,
  type TypeScriptLanguageHover,
  type TypeScriptLanguageQueryKind,
  type TypeScriptLanguageRequest,
  type TypeScriptLanguageResults,
  type TypeScriptLanguageSignature,
  type TypeScriptLanguageWorkerResponse,
} from "./protocol";

export interface TypeScriptLanguageProject {
  key: string;
  file: string;
  files: TypeScriptLanguageFile[];
}

export interface TypeScriptLanguageWorkerPort {
  postMessage(message: TypeScriptLanguageRequest): void;
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void)
      | ((event: ErrorEvent) => void),
  ): void;
  removeEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<TypeScriptLanguageWorkerResponse>) => void)
      | ((event: ErrorEvent) => void),
  ): void;
  terminate(): void;
}

interface PendingQuery<Kind extends TypeScriptLanguageQueryKind = TypeScriptLanguageQueryKind> {
  project: string;
  revision: number;
  kind: Kind;
  resolve: (value: TypeScriptLanguageResults[Kind] | null) => void;
  reject: (reason: Error) => void;
}

export class TypeScriptLanguageWorkerClient {
  private revision = -1;
  private request = 0;
  private context: TypeScriptLanguageProject | null = null;
  private readonly pending = new Map<number, PendingQuery>();
  private synchronizationError: Error | null = null;
  private workerError: Error | null = null;
  private disposed = false;

  private readonly receive = (event: MessageEvent<TypeScriptLanguageWorkerResponse>) => {
    const response = event.data;
    if (
      !response
      || response.protocol !== TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION
    ) return;
    if (response.kind === "sync-error") {
      if (
        response.project !== this.context?.key
        || response.revision !== this.revision
      ) return;
      const error = response.typescriptVersion === TYPESCRIPT_LANGUAGE_VERSION
        ? new Error(response.error)
        : versionMismatch(response.typescriptVersion);
      this.synchronizationError = error;
      this.rejectPending(error);
      this.onUnavailable?.(error);
      return;
    }
    if (!Number.isSafeInteger(response.request)) return;
    const pending = this.pending.get(response.request);
    if (!pending) return;
    this.pending.delete(response.request);
    if (response.typescriptVersion !== TYPESCRIPT_LANGUAGE_VERSION) {
      pending.reject(versionMismatch(response.typescriptVersion));
      return;
    }
    if (
      response.stale
      || response.project !== pending.project
      || response.revision !== pending.revision
      || response.kind !== pending.kind
      || this.context?.key !== pending.project
      || this.revision !== pending.revision
    ) {
      pending.resolve(null);
      return;
    }
    if (response.error) {
      pending.reject(new Error(response.error));
      return;
    }
    pending.resolve((response.result ?? null) as TypeScriptLanguageResults[typeof pending.kind] | null);
  };

  private readonly receiveWorkerError = (event: ErrorEvent) => {
    this.failWorker(new Error(event.message || "TypeScript language worker failed"));
  };

  constructor(
    private readonly worker: TypeScriptLanguageWorkerPort,
    private readonly onUnavailable?: (error: Error) => void,
  ) {
    worker.addEventListener("message", this.receive);
    worker.addEventListener("error", this.receiveWorkerError);
  }

  sync(context: TypeScriptLanguageProject): number {
    if (this.disposed) throw new Error("TypeScript language worker is disposed");
    if (sameProject(this.context, context)) return this.revision;
    this.context = cloneProject(context);
    this.revision += 1;
    this.synchronizationError = null;
    for (const pending of this.pending.values()) pending.resolve(null);
    this.pending.clear();
    if (this.workerError) return this.revision;
    try {
      this.worker.postMessage({
        protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
        kind: "sync",
        project: context.key,
        revision: this.revision,
        file: context.file,
        files: context.files,
      });
    } catch (error) {
      this.failWorker(asError(error));
    }
    return this.revision;
  }

  diagnostics(): Promise<TypeScriptLanguageDiagnostic[] | null> {
    return this.query("diagnostics", {});
  }

  completion(position: number, explicit: boolean): Promise<TypeScriptLanguageCompletionResult | null> {
    return this.query("completion", { position, explicit });
  }

  hover(position: number): Promise<TypeScriptLanguageHover | null> {
    return this.query("hover", { position });
  }

  signature(position: number): Promise<TypeScriptLanguageSignature | null> {
    return this.query("signature", { position });
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.worker.removeEventListener("message", this.receive);
    this.worker.removeEventListener("error", this.receiveWorkerError);
    this.worker.terminate();
    for (const pending of this.pending.values()) pending.resolve(null);
    this.pending.clear();
  }

  private query<Kind extends TypeScriptLanguageQueryKind>(
    kind: Kind,
    details: Kind extends "completion"
      ? { position: number; explicit: boolean }
      : Kind extends "hover" | "signature"
        ? { position: number }
        : Record<never, never>,
  ): Promise<TypeScriptLanguageResults[Kind] | null> {
    if (this.disposed) return Promise.resolve(null);
    const context = this.context;
    if (!context) return Promise.resolve(null);
    const unavailable = this.workerError ?? this.synchronizationError;
    if (unavailable) return Promise.reject(unavailable);
    this.request += 1;
    const request = this.request;
    return new Promise((resolve, reject) => {
      this.pending.set(request, {
        project: context.key,
        revision: this.revision,
        kind,
        resolve: resolve as PendingQuery["resolve"],
        reject,
      });
      try {
        this.worker.postMessage({
          protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
          kind,
          request,
          project: context.key,
          revision: this.revision,
          file: context.file,
          ...details,
        } as TypeScriptLanguageRequest);
      } catch (error) {
        this.failWorker(asError(error));
      }
    });
  }

  private rejectPending(error: Error): void {
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
  }

  private failWorker(error: Error): void {
    this.workerError = error;
    this.rejectPending(error);
    this.onUnavailable?.(error);
  }
}

export function createTypeScriptLanguageWorker(): TypeScriptLanguageWorkerPort | null {
  if (typeof Worker === "undefined") return null;
  return new Worker(
    new URL("./typescript-language.worker.ts", import.meta.url),
    { type: "module", name: "geosolve-typescript-language" },
  ) as TypeScriptLanguageWorkerPort;
}

function cloneProject(project: TypeScriptLanguageProject): TypeScriptLanguageProject {
  return {
    key: project.key,
    file: project.file,
    files: project.files.map((file) => ({ ...file })),
  };
}

function sameProject(
  left: TypeScriptLanguageProject | null,
  right: TypeScriptLanguageProject,
): boolean {
  if (
    !left
    || left.key !== right.key
    || left.file !== right.file
    || left.files.length !== right.files.length
  ) return false;
  return left.files.every((file, index) => {
    const other = right.files[index];
    return other?.path === file.path && other.contents === file.contents;
  });
}

function versionMismatch(version: unknown): Error {
  return new Error(
    `GeoSolve requires TypeScript ${TYPESCRIPT_LANGUAGE_VERSION}; worker reported ${String(version)}`,
  );
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}
