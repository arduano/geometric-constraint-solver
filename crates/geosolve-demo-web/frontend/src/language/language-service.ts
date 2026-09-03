// SPDX-License-Identifier: GPL-3.0-or-later

import ts from "typescript";
import {
  GEOSOLVE_SKETCH_CODE_AUTHORING,
  GEOSOLVE_SKETCH_CODE_INDEX,
  LANGUAGE_SERVICE_TYPESCRIPT_VERSION,
  TYPESCRIPT_STANDARD_LIBRARY,
} from "./generated/language-service-declarations";
import {
  TYPESCRIPT_LANGUAGE_FILE_LIMIT,
  TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
  TYPESCRIPT_LANGUAGE_SOURCE_LIMIT,
  TYPESCRIPT_LANGUAGE_VERSION,
  type TypeScriptLanguageCompletionResult,
  type TypeScriptLanguageDiagnostic,
  type TypeScriptLanguageFile,
  type TypeScriptLanguageHover,
  type TypeScriptLanguageQueryKind,
  type TypeScriptLanguageRequest,
  type TypeScriptLanguageResponse,
  type TypeScriptLanguageSignature,
  type TypeScriptLanguageSyncRequest,
  type TypeScriptLanguageWorkerResponse,
} from "./protocol";

const PROJECT_ROOT = "/project";
const STANDARD_LIBRARY = "/lib.es2022.d.ts";
const SDK_ROOT = "/node_modules/@geosolve/sketch-code";
const SDK_INDEX = `${SDK_ROOT}/index.d.ts`;
const SDK_AUTHORING = `${SDK_ROOT}/authoring.d.ts`;
const DIAGNOSTIC_LIMIT = 200;
const COMPLETION_LIMIT = 250;

interface ProjectIdentity {
  project: string;
  revision: number;
  file: string;
}

export class TypeScriptProjectLanguageService {
  readonly typescriptVersion = TYPESCRIPT_LANGUAGE_VERSION;

  private project = "";
  private revision = -1;
  private selectedFile = "";
  private projectVersion = 0;
  private readonly files = new Map<string, string>([
    [STANDARD_LIBRARY, TYPESCRIPT_STANDARD_LIBRARY],
    [SDK_INDEX, GEOSOLVE_SKETCH_CODE_INDEX],
    [SDK_AUTHORING, GEOSOLVE_SKETCH_CODE_AUTHORING],
  ]);
  private readonly projectFiles = new Set<string>();
  private readonly compilerOptions: ts.CompilerOptions = {
    allowImportingTsExtensions: true,
    allowSyntheticDefaultImports: true,
    exactOptionalPropertyTypes: true,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    noEmit: true,
    noUncheckedIndexedAccess: true,
    skipLibCheck: true,
    strict: true,
    target: ts.ScriptTarget.ES2022,
  };
  private readonly service: ts.LanguageService;

  constructor() {
    if (
      ts.version !== TYPESCRIPT_LANGUAGE_VERSION
      || LANGUAGE_SERVICE_TYPESCRIPT_VERSION !== TYPESCRIPT_LANGUAGE_VERSION
    ) {
      throw new Error(
        `GeoSolve requires TypeScript ${TYPESCRIPT_LANGUAGE_VERSION}; loaded ${ts.version}`,
      );
    }
    const host: ts.LanguageServiceHost = {
      directoryExists: (path) => this.directoryExists(path),
      fileExists: (path) => this.files.has(canonicalPath(path)),
      getCompilationSettings: () => this.compilerOptions,
      getCurrentDirectory: () => PROJECT_ROOT,
      getDefaultLibFileName: () => STANDARD_LIBRARY,
      getDirectories: () => [],
      getNewLine: () => "\n",
      getProjectVersion: () => String(this.projectVersion),
      getScriptFileNames: () => [...this.projectFiles, SDK_INDEX, SDK_AUTHORING],
      getScriptSnapshot: (path) => {
        const contents = this.files.get(canonicalPath(path));
        return contents === undefined ? undefined : ts.ScriptSnapshot.fromString(contents);
      },
      getScriptVersion: (path) => this.projectFiles.has(canonicalPath(path))
        ? String(this.revision)
        : "1",
      readFile: (path) => this.files.get(canonicalPath(path)),
      readDirectory: () => [],
      realpath: canonicalPath,
      resolveModuleNames: (names, containingFile) => names.map((name) => {
        if (name === "@geosolve/sketch-code") {
          return {
            extension: ts.Extension.Dts,
            isExternalLibraryImport: true,
            resolvedFileName: SDK_INDEX,
          };
        }
        return ts.resolveModuleName(
          name,
          canonicalPath(containingFile),
          this.compilerOptions,
          {
            directoryExists: (path) => this.directoryExists(path),
            fileExists: (path) => this.files.has(canonicalPath(path)),
            getCurrentDirectory: () => PROJECT_ROOT,
            getDirectories: () => [],
            readFile: (path) => this.files.get(canonicalPath(path)),
            realpath: canonicalPath,
          },
        ).resolvedModule;
      }),
      useCaseSensitiveFileNames: () => true,
    };
    this.service = ts.createLanguageService(
      host,
      ts.createDocumentRegistry(true, PROJECT_ROOT),
    );
  }

  sync(request: TypeScriptLanguageSyncRequest): void {
    validateSyncRequest(request);
    for (const path of this.projectFiles) this.files.delete(path);
    this.projectFiles.clear();
    for (const file of request.files) {
      const path = projectPath(file.path);
      this.files.set(path, file.contents);
      this.projectFiles.add(path);
    }
    this.project = request.project;
    this.revision = request.revision;
    this.selectedFile = projectPath(request.file);
    this.projectVersion += 1;
    this.service.cleanupSemanticCache();
  }

  identity(): ProjectIdentity {
    return {
      project: this.project,
      revision: this.revision,
      file: this.selectedFile,
    };
  }

  matches(identity: ProjectIdentity): boolean {
    return identity.project === this.project
      && identity.revision === this.revision
      && projectPath(identity.file) === this.selectedFile;
  }

  diagnostics(file: string): TypeScriptLanguageDiagnostic[] {
    const path = this.requireFile(file);
    const sourceLength = this.files.get(path)?.length ?? 0;
    const diagnostics = [
      ...this.service.getSyntacticDiagnostics(path),
      ...this.service.getSemanticDiagnostics(path),
    ];
    const seen = new Set<string>();
    return diagnostics.flatMap((diagnostic) => {
      const from = clampPosition(diagnostic.start ?? 0, sourceLength);
      const to = clampPosition(from + (diagnostic.length ?? 0), sourceLength);
      const message = ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n");
      const key = `${diagnostic.code}:${from}:${to}:${message}`;
      if (seen.has(key)) return [];
      seen.add(key);
      return [{
        from,
        to,
        severity: diagnosticSeverity(diagnostic.category),
        message,
        code: diagnostic.code,
        source: "TypeScript" as const,
      }];
    }).slice(0, DIAGNOSTIC_LIMIT);
  }

  completion(
    file: string,
    position: number,
    explicit: boolean,
  ): TypeScriptLanguageCompletionResult | null {
    const path = this.requireFile(file);
    const source = this.files.get(path) ?? "";
    requirePosition(position, source.length);
    const completion = this.service.getCompletionsAtPosition(path, position, {
      includeCompletionsForModuleExports: false,
      includeCompletionsWithInsertText: true,
      includeCompletionsWithSnippetText: false,
      triggerKind: explicit
        ? ts.CompletionTriggerKind.Invoked
        : ts.CompletionTriggerKind.TriggerCharacter,
    });
    if (!completion) return null;
    const fallback = completion.optionalReplacementSpan
      ?? wordSpan(source, position);
    const fallbackFrom = clampPosition(fallback.start, source.length);
    const fallbackTo = clampPosition(fallback.start + fallback.length, source.length);
    const options = completion.entries.slice(0, COMPLETION_LIMIT).map((entry) => {
      const span = entry.replacementSpan;
      return {
        label: entry.name,
        insertText: entry.insertText ?? entry.name,
        ...(entry.filterText ? { filterText: entry.filterText } : {}),
        kind: entry.kind,
        ...(entry.kindModifiers ? { detail: entry.kindModifiers } : {}),
        ...(span ? {
          from: clampPosition(span.start, source.length),
          to: clampPosition(span.start + span.length, source.length),
        } : {}),
        sortText: entry.sortText,
      };
    });
    return {
      from: fallbackFrom,
      to: fallbackTo,
      options,
      incomplete: completion.isIncomplete === true,
    };
  }

  hover(file: string, position: number): TypeScriptLanguageHover | null {
    const path = this.requireFile(file);
    const sourceLength = this.files.get(path)?.length ?? 0;
    requirePosition(position, sourceLength);
    const info = this.service.getQuickInfoAtPosition(path, position);
    if (!info) return null;
    const documentation = documentationText(info.documentation, info.tags);
    return {
      from: clampPosition(info.textSpan.start, sourceLength),
      to: clampPosition(info.textSpan.start + info.textSpan.length, sourceLength),
      kind: info.kind,
      display: ts.displayPartsToString(info.displayParts),
      ...(documentation ? { documentation } : {}),
    };
  }

  signature(file: string, position: number): TypeScriptLanguageSignature | null {
    const path = this.requireFile(file);
    const sourceLength = this.files.get(path)?.length ?? 0;
    requirePosition(position, sourceLength);
    const help = this.service.getSignatureHelpItems(path, position, {
      triggerReason: { kind: "invoked" },
    });
    if (!help || help.items.length === 0) return null;
    const overload = Math.min(help.selectedItemIndex, help.items.length - 1);
    const item = help.items[overload];
    if (!item) return null;
    const prefix = ts.displayPartsToString(item.prefixDisplayParts);
    const suffix = ts.displayPartsToString(item.suffixDisplayParts);
    const separator = ts.displayPartsToString(item.separatorDisplayParts);
    const parameters = item.parameters.map((parameter) => ({
      display: ts.displayPartsToString(parameter.displayParts),
      ...(ts.displayPartsToString(parameter.documentation)
        ? { documentation: ts.displayPartsToString(parameter.documentation) }
        : {}),
      optional: parameter.isOptional,
    }));
    const activeParameter = parameters.length === 0
      ? 0
      : Math.min(help.argumentIndex, parameters.length - 1);
    return {
      from: clampPosition(help.applicableSpan.start, sourceLength),
      to: clampPosition(
        help.applicableSpan.start + help.applicableSpan.length,
        sourceLength,
      ),
      display: `${prefix}${parameters.map((parameter) => parameter.display).join(separator)}${suffix}`,
      prefix,
      suffix,
      separator,
      parameters,
      activeParameter,
      ...(documentationText(item.documentation, item.tags)
        ? { documentation: documentationText(item.documentation, item.tags) }
        : {}),
      overload,
      overloadCount: help.items.length,
    };
  }

  dispose(): void {
    this.service.dispose();
  }

  private requireFile(file: string): string {
    const path = projectPath(file);
    if (!this.projectFiles.has(path)) {
      throw new Error(`TypeScript project does not contain ${JSON.stringify(file)}`);
    }
    return path;
  }

  private directoryExists(path: string): boolean {
    const prefix = `${canonicalPath(path).replace(/\/$/u, "")}/`;
    return [...this.files.keys()].some((file) => file.startsWith(prefix));
  }
}

export function handleTypeScriptLanguageRequest(
  service: TypeScriptProjectLanguageService,
  request: TypeScriptLanguageRequest,
): TypeScriptLanguageWorkerResponse | null {
  if (request.kind === "sync") {
    try {
      service.sync(request);
      return null;
    } catch (error) {
      return {
        protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
        kind: "sync-error",
        project: request.project,
        revision: request.revision,
        typescriptVersion: TYPESCRIPT_LANGUAGE_VERSION,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }
  const kind = request.kind as TypeScriptLanguageQueryKind;
  const response = {
    protocol: TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION,
    request: request.request,
    project: request.project,
    revision: request.revision,
    kind,
    typescriptVersion: TYPESCRIPT_LANGUAGE_VERSION,
  } as const;
  try {
    validateQueryRequest(request);
    if (!service.matches(request)) return { ...response, stale: true };
    switch (request.kind) {
      case "diagnostics":
        return { ...response, kind: request.kind, stale: false, result: service.diagnostics(request.file) };
      case "completion":
        return { ...response, kind: request.kind, stale: false, result: service.completion(request.file, request.position, request.explicit) };
      case "hover":
        return { ...response, kind: request.kind, stale: false, result: service.hover(request.file, request.position) };
      case "signature":
        return { ...response, kind: request.kind, stale: false, result: service.signature(request.file, request.position) };
    }
  } catch (error) {
    return {
      ...response,
      stale: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

function validateSyncRequest(request: TypeScriptLanguageSyncRequest): void {
  if (
    request.protocol !== TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION
    || !validIdentity(request)
    || !Array.isArray(request.files)
    || request.files.length === 0
    || request.files.length > TYPESCRIPT_LANGUAGE_FILE_LIMIT
  ) {
    throw new Error("malformed TypeScript language-service sync request");
  }
  const paths = new Set<string>();
  let total = 0;
  for (const file of request.files) {
    if (!validFile(file)) throw new Error("malformed TypeScript language-service source file");
    const path = projectPath(file.path);
    if (paths.has(path)) throw new Error(`duplicate TypeScript source file ${JSON.stringify(file.path)}`);
    paths.add(path);
    total += utf8Length(file.contents);
    if (total > TYPESCRIPT_LANGUAGE_SOURCE_LIMIT) {
      throw new Error("TypeScript language-service project exceeds its source limit");
    }
  }
  if (!paths.has(projectPath(request.file))) {
    throw new Error("selected TypeScript source file is absent from the project");
  }
}

function utf8Length(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function validateQueryRequest(
  request: Exclude<TypeScriptLanguageRequest, TypeScriptLanguageSyncRequest>,
): void {
  if (
    request.protocol !== TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION
    || !validIdentity(request)
    || !Number.isSafeInteger(request.request)
    || request.request < 1
  ) {
    throw new Error("malformed TypeScript language-service query");
  }
  if (
    request.kind !== "diagnostics"
    && (!Number.isSafeInteger(request.position) || request.position < 0)
  ) {
    throw new Error("TypeScript language-service position is invalid");
  }
}

function validIdentity(value: {
  project: string;
  revision: number;
  file: string;
}): boolean {
  return value.project.length > 0
    && value.project.length <= 1024
    && Number.isSafeInteger(value.revision)
    && value.revision >= 0
    && validRelativePath(value.file);
}

function validFile(value: TypeScriptLanguageFile): boolean {
  return Boolean(value)
    && validRelativePath(value.path)
    && typeof value.contents === "string";
}

function validRelativePath(path: string): boolean {
  if (typeof path !== "string" || path.length === 0 || path.length > 1024) return false;
  const normalized = path.replaceAll("\\", "/");
  return !normalized.startsWith("/")
    && !normalized.split("/").some((part) => part === "" || part === "." || part === "..");
}

function projectPath(path: string): string {
  if (!validRelativePath(path)) throw new Error(`invalid TypeScript project path ${JSON.stringify(path)}`);
  return `${PROJECT_ROOT}/${path.replaceAll("\\", "/")}`;
}

function canonicalPath(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  const parts: string[] = [];
  for (const part of normalized.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  return `/${parts.join("/")}`;
}

function requirePosition(position: number, length: number): void {
  if (!Number.isSafeInteger(position) || position < 0 || position > length) {
    throw new Error("TypeScript language-service position is outside the selected file");
  }
}

function clampPosition(position: number, length: number): number {
  return Math.max(0, Math.min(length, position));
}

function wordSpan(source: string, position: number): ts.TextSpan {
  let start = position;
  while (start > 0 && /[$\w]/u.test(source[start - 1] ?? "")) start -= 1;
  return { start, length: position - start };
}

function diagnosticSeverity(category: ts.DiagnosticCategory): TypeScriptLanguageDiagnostic["severity"] {
  switch (category) {
    case ts.DiagnosticCategory.Error:
      return "error";
    case ts.DiagnosticCategory.Warning:
      return "warning";
    default:
      return "info";
  }
}

function documentationText(
  documentation: readonly ts.SymbolDisplayPart[] | undefined,
  tags: readonly ts.JSDocTagInfo[] | undefined,
): string {
  const body = ts.displayPartsToString(documentation ? [...documentation] : undefined);
  const tagText = (tags ?? []).map((tag) => {
    const text = ts.displayPartsToString(tag.text ? [...tag.text] : undefined);
    return text ? `@${tag.name} ${text}` : `@${tag.name}`;
  });
  return [body, ...tagText].filter(Boolean).join("\n");
}
