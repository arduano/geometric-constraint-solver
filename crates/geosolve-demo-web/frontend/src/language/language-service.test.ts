// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import {
  handleTypeScriptLanguageRequest,
  TypeScriptProjectLanguageService,
} from "./language-service";
import {
  TYPESCRIPT_LANGUAGE_FILE_LIMIT,
  TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
  TYPESCRIPT_LANGUAGE_SOURCE_LIMIT,
  TYPESCRIPT_LANGUAGE_VERSION,
  type TypeScriptLanguageResponse,
  type TypeScriptLanguageWorkerResponse,
} from "./protocol";

const SOURCE = `"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const segment = $.geometry.segment("segment", {
    start: [0, 0],
    end: [10, 0],
  });
  return { segment };
});
`;

describe("TypeScript project language service", () => {
  it("type-checks a valid managed sketch without ambient declaration drift", () => {
    const service = synchronized(SOURCE);
    expect(service.diagnostics("sketch.ts")).toEqual([]);
    service.dispose();
  });

  it("uses the pinned exact SDK for precise semantic diagnostics", () => {
    const service = synchronized(SOURCE.replace("end: [10, 0]", "end: [10, \"wrong\"]"));
    const diagnostics = service.diagnostics("sketch.ts");
    const wrong = diagnostics.find((diagnostic) => diagnostic.message.includes("not assignable to type 'number'"));

    expect(service.typescriptVersion).toBe(TYPESCRIPT_LANGUAGE_VERSION);
    expect(wrong).toMatchObject({
      from: expect.any(Number),
      to: expect.any(Number),
      severity: "error",
      source: "TypeScript",
    });
    expect(SOURCE_WITH_WRONG.slice(wrong!.from, wrong!.to)).toBe('"wrong"');
    service.dispose();
  });

  it("offers contextual managed-sketch methods and argument properties", () => {
    const methodSource = SOURCE.replace("$.geometry.segment", "$.geometry.seg");
    const methodService = synchronized(methodSource);
    const methodPosition = methodSource.indexOf("$.geometry.seg") + "$.geometry.seg".length;
    const methods = methodService.completion("sketch.ts", methodPosition, true);
    expect(methods?.options).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: "segment", kind: "method" }),
    ]));
    methodService.dispose();

    const propertySource = SOURCE.replace("    end: [10, 0],", "    en");
    const propertyService = synchronized(propertySource);
    const propertyPosition = propertySource.indexOf("    en") + "    en".length;
    const properties = propertyService.completion("sketch.ts", propertyPosition, true);
    expect(properties?.options).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: "end" }),
    ]));
    propertyService.dispose();
  });

  it("returns hover types and signature help from SDK declarations", () => {
    const service = synchronized(SOURCE);
    const method = SOURCE.indexOf("segment(\"segment\"") + 2;
    expect(service.hover("sketch.ts", method)).toMatchObject({
      display: expect.stringContaining("segment"),
      kind: "method",
    });

    const signaturePosition = SOURCE.indexOf('"segment", {') + '"segment",'.length;
    const signature = service.signature("sketch.ts", signaturePosition);
    expect(signature).toMatchObject({
      display: expect.stringContaining("SegmentValues"),
      activeParameter: 1,
      overloadCount: expect.any(Number),
    });

    const incomplete = SOURCE.replace("  return { segment };", "  mm(\n  return { segment };");
    service.sync({
      protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
      kind: "sync",
      project: "test-project",
      revision: 2,
      file: "sketch.ts",
      files: [{ path: "sketch.ts", contents: incomplete }],
    });
    expect(service.signature("sketch.ts", incomplete.indexOf("  mm(") + "  mm(".length)).toMatchObject({
      display: "mm(value: number): UnitLiteral<\"mm\">",
      activeParameter: 0,
    });
    service.dispose();
  });

  it("re-analyzes relative project modules when their declarations change", () => {
    const service = new TypeScriptProjectLanguageService();
    const sketch = SOURCE.replace("end: [10, 0]", "end: [width, 0]")
      .replace(
        'import { sketch, mm } from "@geosolve/sketch-code";',
        'import { sketch, mm } from "@geosolve/sketch-code";\nimport { width } from "./values.js";',
      );
    synchronizeFiles(service, sketch, "export const width = \"wide\";", 1);
    expect(service.diagnostics("sketch.ts")).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 2322, severity: "error" }),
    ]));

    synchronizeFiles(service, sketch, "export const width = 10;", 2);
    expect(service.diagnostics("sketch.ts")).toEqual([]);
    service.dispose();
  });

  it("rejects stale revision queries without evaluating current state", () => {
    const service = synchronized(SOURCE);
    const response = handleTypeScriptLanguageRequest(service, {
      protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
      kind: "diagnostics",
      request: 1,
      project: "test-project",
      revision: 0,
      file: "sketch.ts",
    }) as TypeScriptLanguageResponse<"diagnostics">;
    expect(response.stale).toBe(true);
    expect("result" in response).toBe(false);
    service.dispose();
  });

  it("reports malformed synchronization without terminating the worker boundary", () => {
    const service = synchronized(SOURCE);
    const response = handleTypeScriptLanguageRequest(service, {
      protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
      kind: "sync",
      project: "test-project",
      revision: 2,
      file: "sketch.ts",
      files: Array.from(
        { length: TYPESCRIPT_LANGUAGE_FILE_LIMIT + 1 },
        (_, index) => ({ path: `file-${index}.ts`, contents: "" }),
      ),
    });

    expect(response).toMatchObject({
      kind: "sync-error",
      project: "test-project",
      revision: 2,
      error: "malformed TypeScript language-service sync request",
    });
    expect(service.identity()).toMatchObject({
      project: "test-project",
      revision: 1,
      file: "/project/sketch.ts",
    });
    expect(service.diagnostics("sketch.ts")).toEqual([]);
    service.dispose();
  });

  it("reports a valid-shape project beyond the analysis bound and retains prior state", () => {
    const service = synchronized(SOURCE);
    const main = `export {};${" ".repeat(TYPESCRIPT_LANGUAGE_SOURCE_LIMIT - "export {};".length)}`;
    const response = synchronizeThroughWorker(
      service,
      [
        { path: "sketch.ts", contents: main },
        { path: "patches/helper.patch.ts", contents: "export {};" },
      ],
      2,
    );

    expect(response).toMatchObject({
      kind: "sync-error",
      project: "test-project",
      revision: 2,
      error: "TypeScript language-service project exceeds its source limit",
    });
    expect(service.identity().revision).toBe(1);
    expect(service.diagnostics("sketch.ts")).toEqual([]);
    service.dispose();
  });

  it("accepts a project exactly at the bounded large-source limit", () => {
    const service = new TypeScriptProjectLanguageService();
    const response = synchronizeThroughWorker(
      service,
      [
        { path: "sketch.ts", contents: "export {};" },
        {
          path: "large.ts",
          contents: " ".repeat(TYPESCRIPT_LANGUAGE_SOURCE_LIMIT - "export {};".length),
        },
      ],
      1,
    );

    expect(response).toBeNull();
    expect(service.identity()).toMatchObject({
      project: "test-project",
      revision: 1,
      file: "/project/sketch.ts",
    });
    expect(service.diagnostics("sketch.ts")).toEqual([]);
    service.dispose();
  });
});

const SOURCE_WITH_WRONG = SOURCE.replace("end: [10, 0]", "end: [10, \"wrong\"]");

function synchronized(source: string): TypeScriptProjectLanguageService {
  const service = new TypeScriptProjectLanguageService();
  synchronizeFiles(service, source, undefined, 1);
  return service;
}

function synchronizeFiles(
  service: TypeScriptProjectLanguageService,
  source: string,
  values: string | undefined,
  revision: number,
): void {
  service.sync({
    protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
    kind: "sync",
    project: "test-project",
    revision,
    file: "sketch.ts",
    files: [
      { path: "sketch.ts", contents: source },
      ...(values === undefined ? [] : [{ path: "values.ts", contents: values }]),
    ],
  });
}

function synchronizeThroughWorker(
  service: TypeScriptProjectLanguageService,
  files: Array<{ path: string; contents: string }>,
  revision: number,
): TypeScriptLanguageWorkerResponse | null {
  return handleTypeScriptLanguageRequest(service, {
    protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
    kind: "sync",
    project: "test-project",
    revision,
    file: "sketch.ts",
    files,
  });
}
