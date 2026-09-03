// SPDX-License-Identifier: GPL-3.0-or-later

export const TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION = 1 as const;
export const TYPESCRIPT_LANGUAGE_VERSION = "5.9.2" as const;
export const TYPESCRIPT_LANGUAGE_SOURCE_LIMIT = 4 * 1024 * 1024;
export const TYPESCRIPT_LANGUAGE_FILE_LIMIT = 64;

export interface TypeScriptLanguageFile {
  path: string;
  contents: string;
}

export interface TypeScriptLanguageSyncRequest {
  protocol: typeof TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION;
  kind: "sync";
  project: string;
  revision: number;
  file: string;
  files: TypeScriptLanguageFile[];
}

interface TypeScriptLanguageQueryBase {
  protocol: typeof TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION;
  request: number;
  project: string;
  revision: number;
  file: string;
}

export interface TypeScriptDiagnosticsRequest extends TypeScriptLanguageQueryBase {
  kind: "diagnostics";
}

export interface TypeScriptCompletionRequest extends TypeScriptLanguageQueryBase {
  kind: "completion";
  position: number;
  explicit: boolean;
}

export interface TypeScriptHoverRequest extends TypeScriptLanguageQueryBase {
  kind: "hover";
  position: number;
}

export interface TypeScriptSignatureRequest extends TypeScriptLanguageQueryBase {
  kind: "signature";
  position: number;
}

export type TypeScriptLanguageRequest =
  | TypeScriptLanguageSyncRequest
  | TypeScriptDiagnosticsRequest
  | TypeScriptCompletionRequest
  | TypeScriptHoverRequest
  | TypeScriptSignatureRequest;

export type TypeScriptDiagnosticSeverity = "error" | "warning" | "info";

export interface TypeScriptLanguageDiagnostic {
  from: number;
  to: number;
  severity: TypeScriptDiagnosticSeverity;
  message: string;
  code: number;
  source: "TypeScript";
}

export interface TypeScriptLanguageCompletion {
  label: string;
  insertText: string;
  filterText?: string;
  kind: string;
  detail?: string;
  from?: number;
  to?: number;
  sortText: string;
}

export interface TypeScriptLanguageCompletionResult {
  from: number;
  to: number;
  options: TypeScriptLanguageCompletion[];
  incomplete: boolean;
}

export interface TypeScriptLanguageHover {
  from: number;
  to: number;
  kind: string;
  display: string;
  documentation?: string;
}

export interface TypeScriptLanguageSignature {
  from: number;
  to: number;
  display: string;
  prefix: string;
  suffix: string;
  separator: string;
  parameters: Array<{
    display: string;
    documentation?: string;
    optional: boolean;
  }>;
  activeParameter: number;
  documentation?: string;
  overload: number;
  overloadCount: number;
}

export interface TypeScriptLanguageResults {
  diagnostics: TypeScriptLanguageDiagnostic[];
  completion: TypeScriptLanguageCompletionResult | null;
  hover: TypeScriptLanguageHover | null;
  signature: TypeScriptLanguageSignature | null;
}

export type TypeScriptLanguageQueryKind = keyof TypeScriptLanguageResults;

export interface TypeScriptLanguageResponse<
  Kind extends TypeScriptLanguageQueryKind = TypeScriptLanguageQueryKind,
> {
  protocol: typeof TYPESCRIPT_LANGUAGE_PROTOCOL_VERSION;
  request: number;
  project: string;
  revision: number;
  kind: Kind;
  typescriptVersion: typeof TYPESCRIPT_LANGUAGE_VERSION;
  stale: boolean;
  result?: TypeScriptLanguageResults[Kind];
  error?: string;
}
