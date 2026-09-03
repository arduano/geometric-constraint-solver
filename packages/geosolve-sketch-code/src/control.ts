// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Live code-control protocol, independent of the authoring and managed-source
 * compilers. These DTOs exactly mirror the bounded Rust/WASM RPC wire.
 */

export type SemanticPathSegment = string | number | { readonly member: string };

function byteCompare(left: string, right: string): number {
  const a = new TextEncoder().encode(left);
  const b = new TextEncoder().encode(right);
  const length = Math.min(a.length, b.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return a.length - b.length;
}

/** Maximum strict managed code-control request accepted by Rust/WASM. */
export const MAX_CODE_CONTROL_RPC_REQUEST_BYTES = 16 * 1024 * 1024;
/** Maximum response for a managed edit or outer Undo/Redo receipt. */
export const MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES = 16 * 1024 * 1024;
/** Maximum read-only managed-control response. */
export const MAX_CODE_CONTROL_RPC_RESPONSE_BYTES = 64 * 1024 * 1024;

const MAX_MANAGED_CONTROL_COUNT = 8_192;
const MAX_MANAGED_CONTROL_CONSUMERS = 65_536;
const MAX_MANAGED_VALUE_DEPTH = 64;
const MAX_MANAGED_VALUE_NODES = 16_384;
const MAX_CONTROL_PATH_SEGMENTS = 256;

export interface ManagedWireUnitLiteral {
  readonly unit: string;
  readonly value: number;
}

/** Exact Serde shape of Rust `ManagedValue`. */
export type ManagedValue =
  | { readonly kind: "null" }
  | { readonly kind: "bool"; readonly value: boolean }
  | { readonly kind: "number"; readonly value: number }
  | { readonly kind: "string"; readonly value: string }
  | { readonly kind: "unit"; readonly value: ManagedWireUnitLiteral }
  | { readonly kind: "array"; readonly value: readonly ManagedValue[] }
  | { readonly kind: "object"; readonly value: Readonly<Record<string, ManagedValue>> }
  | {
      readonly kind: "reference";
      readonly value: {
        readonly declaration: string;
        readonly path: readonly SemanticPathSegment[];
      };
    };

export interface ManagedControlBound {
  readonly value: number;
  readonly inclusive: boolean;
}

export type ManagedControlNumberKind = "real" | "integer" | "natural";

export type ManagedControlSchema =
  | {
      readonly kind: "number";
      readonly number: ManagedControlNumberKind;
      readonly minimum: ManagedControlBound | null;
      readonly maximum: ManagedControlBound | null;
    }
  | {
      readonly kind: "unit";
      readonly unit: string;
      readonly number: ManagedControlNumberKind;
      readonly minimum: ManagedControlBound | null;
      readonly maximum: ManagedControlBound | null;
    }
  | { readonly kind: "boolean" }
  | { readonly kind: "choice"; readonly choices: readonly string[] }
  | { readonly kind: "text" };

export type ManagedOwnedSpanKind =
  | "declaration"
  | "symbol"
  | "arguments"
  | "literal"
  | "reference"
  | "organization";

export interface ManagedControlSpan {
  readonly start: number;
  readonly end: number;
}

export interface ManagedControlSource {
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
  readonly kind: ManagedOwnedSpanKind;
  readonly span: ManagedControlSpan;
  readonly source_text: string;
}

export interface ManagedControlNavigation {
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
}

export type ManagedControlReadOnlyReason =
  | "structure"
  | "reference"
  | "structural_identity"
  | "solver_instance"
  | "null"
  | "absent"
  | "incompatible_schemas"
  | "unproven_transform";

export interface GeneratedMemberAddress {
  readonly invocation: string;
  readonly template: readonly string[];
  readonly member_key: readonly string[];
  readonly output: readonly string[];
}

export interface GeneratedMemberIdentity {
  readonly allocation: number;
  readonly generation: number;
}

export type ManagedControlConsumerTarget =
  | {
      readonly target: "declaration";
      readonly declaration: string;
      readonly family: string;
    }
  | {
      readonly target: "generated";
      readonly address: GeneratedMemberAddress;
      readonly identity: GeneratedMemberIdentity;
      readonly artifact_digest: string;
      readonly family: string;
    };

export interface ManagedControlConsumer {
  readonly target: ManagedControlConsumerTarget;
  readonly property: readonly SemanticPathSegment[];
}

export interface ManagedControlToken {
  readonly id: string;
  readonly project: string;
  readonly project_digest: string;
  readonly source_digest: string;
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
  readonly expected: ManagedValue;
  readonly generation_digest: string;
  readonly authentication: string;
}

export type ManagedControlAccess =
  | { readonly access: "editable"; readonly token: ManagedControlToken }
  | {
      readonly access: "read_only";
      readonly reason: ManagedControlReadOnlyReason;
      readonly navigation: ManagedControlNavigation | null;
    };

export interface ManagedControl {
  readonly id: string;
  readonly source: ManagedControlSource;
  readonly value: ManagedValue;
  readonly schema: ManagedControlSchema | null;
  readonly consumers: readonly ManagedControlConsumer[];
  readonly access: ManagedControlAccess;
}

export interface ManagedControlManifest {
  readonly project: string;
  readonly project_digest: string;
  readonly source_digest: string;
  readonly expansion_digest: string;
  readonly controls: readonly ManagedControl[];
}

export interface ManagedControlEdit {
  readonly token: ManagedControlToken;
  readonly value: ManagedValue;
}

export interface ManagedControlEditBatch {
  readonly edits: readonly ManagedControlEdit[];
}

export interface CodeSessionIdentity {
  readonly session: number;
  readonly revision: number;
  readonly digest: string;
}

export interface CodeSessionReceipt {
  readonly before: CodeSessionIdentity;
  readonly after: CodeSessionIdentity;
  readonly label: string;
  readonly retained_failure: boolean;
}

export interface CodeControlSnapshot {
  readonly identity: CodeSessionIdentity;
  readonly manifest: ManagedControlManifest;
  readonly can_undo: boolean;
  readonly can_redo: boolean;
}

export interface CodeControlEditReceipt {
  readonly receipt: CodeSessionReceipt;
  readonly diagnostic: string | null;
}

export interface CodeControlHistoryReceipt {
  readonly identity: CodeSessionIdentity;
  readonly moved: boolean;
  readonly receipt: CodeSessionReceipt | null;
}

export type CodeControlRpcRequest =
  | { readonly method: "inspect_managed_controls" }
  | {
      readonly method: "edit_managed_controls";
      readonly expected: CodeSessionIdentity;
      readonly batch: ManagedControlEditBatch;
    }
  | { readonly method: "undo"; readonly expected: CodeSessionIdentity }
  | { readonly method: "redo"; readonly expected: CodeSessionIdentity };

export type CodeControlFailureCode =
  | "invalid_request"
  | "request_too_large"
  | "response_too_large"
  | "code_workbench_unavailable"
  | "code_workbench_busy"
  | "control_inspection_rejected"
  | "control_edit_rejected"
  | "stale_code_session"
  | "history_rejected"
  | "editor_publication_rejected";

export interface CodeControlFailure {
  readonly code: CodeControlFailureCode;
  readonly message: string;
  readonly identity: CodeSessionIdentity | null;
}

export type CodeControlResponse<Value> =
  | { readonly outcome: "success"; readonly value: Value }
  | { readonly outcome: "failure"; readonly failure: CodeControlFailure };

export interface CodeControlManagedControlsSuccess {
  readonly result: "managed_controls";
  readonly snapshot: CodeControlSnapshot;
}

export interface CodeControlManagedEditSuccess {
  readonly result: "managed_control_edit";
  readonly receipt: CodeControlEditReceipt;
}

export interface CodeControlHistorySuccess {
  readonly result: "history";
  readonly receipt: CodeControlHistoryReceipt;
}

export type CodeControlInspectResponse =
  CodeControlResponse<CodeControlManagedControlsSuccess>;
export type CodeControlEditResponse = CodeControlResponse<CodeControlManagedEditSuccess>;
export type CodeControlHistoryResponse = CodeControlResponse<CodeControlHistorySuccess>;

export interface CodeControlRpcTransport {
  apply(canonicalRequestJson: string): Promise<string>;
}

export class CodeControlRpcProtocolError extends TypeError {
  constructor(message: string) {
    super(message);
    this.name = "CodeControlRpcProtocolError";
  }
}

/** Typed client for the live workbench's separate outer code authority. */
export class CodeControlClient {
  constructor(private readonly transport: CodeControlRpcTransport) {}

  async inspect(): Promise<CodeControlInspectResponse> {
    const response = await this.transport.apply(encodeCodeControlRpcRequest({
      method: "inspect_managed_controls",
    }));
    return decodeCodeControlResponse(response, "managed_controls");
  }

  async edit(
    snapshot: Pick<CodeControlSnapshot, "identity" | "manifest">,
    batch: ManagedControlEditBatch,
  ): Promise<CodeControlEditResponse> {
    authenticateManagedControlBatch(snapshot.manifest, batch);
    const response = await this.transport.apply(encodeCodeControlRpcRequest({
      method: "edit_managed_controls",
      expected: snapshot.identity,
      batch,
    }));
    const decoded = decodeCodeControlResponse(response, "managed_control_edit");
    bindCodeControlEditResponse(decoded, snapshot.identity);
    return decoded;
  }

  async undo(expected: CodeSessionIdentity): Promise<CodeControlHistoryResponse> {
    const response = await this.transport.apply(encodeCodeControlRpcRequest({
      method: "undo",
      expected,
    }));
    const decoded = decodeCodeControlResponse(response, "history");
    bindCodeControlHistoryResponse(decoded, expected, "Undo");
    return decoded;
  }

  async redo(expected: CodeSessionIdentity): Promise<CodeControlHistoryResponse> {
    const response = await this.transport.apply(encodeCodeControlRpcRequest({
      method: "redo",
      expected,
    }));
    const decoded = decodeCodeControlResponse(response, "history");
    bindCodeControlHistoryResponse(decoded, expected, "Redo");
    return decoded;
  }
}

/** Validates and deterministically encodes one closed Rust request. */
export function encodeCodeControlRpcRequest(value: CodeControlRpcRequest): string {
  const request = controlRecord(value, "code-control RPC request");
  const method = controlString(request.method, "code-control RPC method");
  let checked: unknown;
  switch (method) {
    case "inspect_managed_controls":
      controlExactKeys(request, ["method"], "managed-control inspection request");
      checked = { method };
      break;
    case "edit_managed_controls":
      controlExactKeys(
        request,
        ["method", "expected", "batch"],
        "managed-control edit request",
      );
      const expected = parseCodeSessionIdentity(request.expected, "expected code-session identity");
      const batch = parseManagedControlBatch(request.batch, "managed-control batch");
      checked = {
        method,
        expected,
        batch,
      };
      break;
    case "undo":
    case "redo":
      controlExactKeys(request, ["method", "expected"], `code-control ${method} request`);
      checked = {
        method,
        expected: parseCodeSessionIdentity(request.expected, "expected code-session identity"),
      };
      break;
    default:
      throw new CodeControlRpcProtocolError(
        `code-control RPC method has unknown value ${JSON.stringify(method)}`,
      );
  }
  const encoded = codeControlJson(checked);
  requireCodeControlSize(encoded, MAX_CODE_CONTROL_RPC_REQUEST_BYTES, "code-control RPC request");
  return encoded;
}

function decodeCodeControlResponse(
  response: string,
  expected: "managed_controls",
): CodeControlInspectResponse;
function decodeCodeControlResponse(
  response: string,
  expected: "managed_control_edit",
): CodeControlEditResponse;
function decodeCodeControlResponse(
  response: string,
  expected: "history",
): CodeControlHistoryResponse;
function decodeCodeControlResponse(
  response: string,
  expected: "managed_controls" | "managed_control_edit" | "history",
): CodeControlResponse<unknown> {
  try {
    if (typeof response !== "string") {
      throw new CodeControlRpcProtocolError("code-control RPC transport returned a non-string response");
    }
    requireCodeControlSize(
      response,
      expected === "managed_controls"
        ? MAX_CODE_CONTROL_RPC_RESPONSE_BYTES
        : MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES,
      "code-control RPC response",
    );
    let parsed: unknown;
    try {
      parsed = JSON.parse(response) as unknown;
    } catch {
      throw new CodeControlRpcProtocolError("code-control RPC response is not valid JSON");
    }
    const envelope = controlRecord(parsed, "code-control RPC response");
    const outcome = controlString(envelope.outcome, "code-control RPC outcome");
    if (outcome === "failure") {
      controlExactKeys(envelope, ["outcome", "failure"], "code-control failure response");
      return { outcome, failure: parseCodeControlFailure(envelope.failure) };
    }
    if (outcome !== "success") {
      throw new CodeControlRpcProtocolError(
        `code-control RPC outcome has unknown value ${JSON.stringify(outcome)}`,
      );
    }
    controlExactKeys(envelope, ["outcome", "value"], "code-control success response");
    const value = controlRecord(envelope.value, "code-control success value");
    if (value.result !== expected) {
      throw new CodeControlRpcProtocolError(
        `code-control RPC expected result ${expected}, received ${String(value.result)}`,
      );
    }
    switch (expected) {
      case "managed_controls":
        controlExactKeys(value, ["result", "snapshot"], "managed-controls success");
        return {
          outcome,
          value: {
            result: expected,
            snapshot: parseCodeControlSnapshot(value.snapshot),
          },
        };
      case "managed_control_edit":
        controlExactKeys(value, ["result", "receipt"], "managed-control edit success");
        return {
          outcome,
          value: {
            result: expected,
            receipt: parseCodeControlEditReceipt(value.receipt),
          },
        };
      case "history":
        controlExactKeys(value, ["result", "receipt"], "code-control history success");
        return {
          outcome,
          value: {
            result: expected,
            receipt: parseCodeControlHistoryReceipt(value.receipt),
          },
        };
    }
  } catch (error) {
    if (error instanceof CodeControlRpcProtocolError) throw error;
    const message = error instanceof Error ? error.message : String(error);
    throw new CodeControlRpcProtocolError(`invalid code-control RPC response: ${message}`);
  }
}

function parseCodeControlFailure(value: unknown): CodeControlFailure {
  const failure = controlRecord(value, "code-control failure");
  controlExactKeys(failure, ["code", "message", "identity"], "code-control failure");
  return {
    code: controlEnum(failure.code, CODE_CONTROL_FAILURE_CODES, "code-control failure code"),
    message: controlBoundedString(failure.message, 1024 * 1024, "code-control failure message"),
    identity: failure.identity === null
      ? null
      : parseCodeSessionIdentity(failure.identity, "failure code-session identity"),
  };
}

function parseCodeControlSnapshot(value: unknown): CodeControlSnapshot {
  const snapshot = controlRecord(value, "code-control snapshot");
  controlExactKeys(
    snapshot,
    ["identity", "manifest", "can_undo", "can_redo"],
    "code-control snapshot",
  );
  return {
    identity: parseCodeSessionIdentity(snapshot.identity, "code-control snapshot identity"),
    manifest: parseManagedControlManifest(snapshot.manifest, "code-control snapshot manifest"),
    can_undo: controlBoolean(snapshot.can_undo, "code-control can-undo flag"),
    can_redo: controlBoolean(snapshot.can_redo, "code-control can-redo flag"),
  };
}

function parseCodeControlEditReceipt(value: unknown): CodeControlEditReceipt {
  const result = controlRecord(value, "managed-control edit receipt");
  controlExactKeys(result, ["receipt", "diagnostic"], "managed-control edit receipt");
  const receipt = parseCodeSessionReceipt(result.receipt, "managed-control code receipt");
  const diagnostic = result.diagnostic === null
    ? null
    : controlBoundedString(result.diagnostic, 1024 * 1024, "managed-control diagnostic");
  if (receipt.retained_failure !== (diagnostic !== null)) {
    throw new CodeControlRpcProtocolError(
      "managed-control diagnostic does not match retained-failure state",
    );
  }
  return { receipt, diagnostic };
}

function parseCodeControlHistoryReceipt(value: unknown): CodeControlHistoryReceipt {
  const result = controlRecord(value, "code-control history receipt");
  controlExactKeys(result, ["identity", "moved", "receipt"], "code-control history receipt");
  const identity = parseCodeSessionIdentity(result.identity, "history code-session identity");
  const moved = controlBoolean(result.moved, "history moved flag");
  const receipt = result.receipt === null
    ? null
    : parseCodeSessionReceipt(result.receipt, "history code-session receipt");
  if (moved !== (receipt !== null)) {
    throw new CodeControlRpcProtocolError("history moved flag does not match its receipt");
  }
  if (receipt !== null && !sameCodeSessionIdentity(receipt.after, identity)) {
    throw new CodeControlRpcProtocolError("history receipt does not produce its published identity");
  }
  return { identity, moved, receipt };
}

function bindCodeControlEditResponse(
  response: CodeControlEditResponse,
  expected: CodeSessionIdentity,
): void {
  if (
    response.outcome === "success"
    && !sameCodeSessionIdentity(response.value.receipt.receipt.before, expected)
  ) {
    throw new CodeControlRpcProtocolError(
      "managed-control receipt does not start at the requested code-session identity",
    );
  }
}

function bindCodeControlHistoryResponse(
  response: CodeControlHistoryResponse,
  expected: CodeSessionIdentity,
  action: "Undo" | "Redo",
): void {
  if (response.outcome !== "success") return;
  const result = response.value.receipt;
  const before = result.receipt?.before ?? result.identity;
  if (!sameCodeSessionIdentity(before, expected)) {
    throw new CodeControlRpcProtocolError(
      `${action} receipt does not start at the requested code-session identity`,
    );
  }
}

function parseCodeSessionReceipt(value: unknown, label: string): CodeSessionReceipt {
  const receipt = controlRecord(value, label);
  controlExactKeys(receipt, ["before", "after", "label", "retained_failure"], label);
  const result = {
    before: parseCodeSessionIdentity(receipt.before, `${label} before identity`),
    after: parseCodeSessionIdentity(receipt.after, `${label} after identity`),
    label: controlBoundedString(receipt.label, 1024 * 1024, `${label} label`),
    retained_failure: controlBoolean(receipt.retained_failure, `${label} retained-failure flag`),
  };
  if (result.before.session !== result.after.session) {
    throw new CodeControlRpcProtocolError(`${label} crosses code sessions`);
  }
  return result;
}

function parseCodeSessionIdentity(value: unknown, label: string): CodeSessionIdentity {
  const identity = controlRecord(value, label);
  controlExactKeys(identity, ["session", "revision", "digest"], label);
  return {
    session: controlSafeUnsigned(identity.session, Number.MAX_SAFE_INTEGER, `${label} session`),
    revision: controlSafeUnsigned(identity.revision, Number.MAX_SAFE_INTEGER, `${label} revision`),
    digest: controlHex(identity.digest, 64, `${label} digest`),
  };
}

function parseManagedControlManifest(value: unknown, label: string): ManagedControlManifest {
  const manifest = controlRecord(value, label);
  controlExactKeys(
    manifest,
    ["project", "project_digest", "source_digest", "expansion_digest", "controls"],
    label,
  );
  const project = controlKey(manifest.project, `${label} project`);
  const projectDigest = controlHex(manifest.project_digest, 64, `${label} project digest`);
  const sourceDigest = controlHex(manifest.source_digest, 64, `${label} source digest`);
  const controls = controlArray(manifest.controls, MAX_MANAGED_CONTROL_COUNT, `${label} controls`);
  const state = { valueNodes: 0, consumers: 0 };
  const ids = new Set<string>();
  const parsedControls = controls.map((control, index) => {
    const parsed = parseManagedControl(
      control,
      `${label} control ${index}`,
      state,
      { project, projectDigest, sourceDigest },
    );
    if (ids.has(parsed.id)) {
      throw new CodeControlRpcProtocolError(`${label} contains duplicate control ${parsed.id}`);
    }
    ids.add(parsed.id);
    return parsed;
  });
  return {
    project,
    project_digest: projectDigest,
    source_digest: sourceDigest,
    expansion_digest: controlHex(manifest.expansion_digest, 64, `${label} expansion digest`),
    controls: parsedControls,
  };
}

function parseManagedControl(
  value: unknown,
  label: string,
  state: { valueNodes: number; consumers: number },
  authority: { project: string; projectDigest: string; sourceDigest: string },
): ManagedControl {
  const control = controlRecord(value, label);
  controlExactKeys(control, ["id", "source", "value", "schema", "consumers", "access"], label);
  const id = controlHex(control.id, 64, `${label} ID`);
  const source = parseManagedControlSource(control.source, `${label} source`);
  state.valueNodes = 0;
  const managedValue = parseManagedValue(control.value, `${label} value`, state, 0);
  const schema = control.schema === null
    ? null
    : parseManagedControlSchema(control.schema, `${label} schema`);
  const consumers = controlArray(
    control.consumers,
    MAX_MANAGED_CONTROL_CONSUMERS,
    `${label} consumers`,
  ).map((consumer, index) => {
    state.consumers += 1;
    if (state.consumers > MAX_MANAGED_CONTROL_CONSUMERS) {
      throw new CodeControlRpcProtocolError("managed-control manifest has too many consumers");
    }
    return parseManagedControlConsumer(consumer, `${label} consumer ${index}`);
  });
  const access = parseManagedControlAccess(control.access, `${label} access`, state);
  if (access.access === "editable") {
    if (schema === null) {
      throw new CodeControlRpcProtocolError(`${label} editable control has no schema`);
    }
    const token = access.token;
    if (
      token.id !== id
      || token.project !== authority.project
      || token.project_digest !== authority.projectDigest
      || token.source_digest !== authority.sourceDigest
      || token.declaration !== source.declaration
      || codeControlCanonicalJson(token.path) !== codeControlCanonicalJson(source.path)
      || codeControlCanonicalJson(token.expected) !== codeControlCanonicalJson(managedValue)
    ) {
      throw new CodeControlRpcProtocolError(`${label} editable token does not match its control`);
    }
  } else if (schema !== null) {
    throw new CodeControlRpcProtocolError(`${label} read-only control unexpectedly has a schema`);
  }
  return { id, source, value: managedValue, schema, consumers, access };
}

function parseManagedControlSource(value: unknown, label: string): ManagedControlSource {
  const source = controlRecord(value, label);
  controlExactKeys(source, ["declaration", "path", "kind", "span", "source_text"], label);
  const result = {
    declaration: controlKey(source.declaration, `${label} declaration`),
    path: parseControlPath(source.path, `${label} path`),
    kind: controlEnum(source.kind, MANAGED_SPAN_KINDS, `${label} kind`),
    span: parseManagedControlSpan(source.span, `${label} span`),
    source_text: controlString(source.source_text, `${label} source text`),
  };
  if (new TextEncoder().encode(result.source_text).byteLength !== result.span.end - result.span.start) {
    throw new CodeControlRpcProtocolError(`${label} text does not match its byte span`);
  }
  return result;
}

function parseManagedControlSpan(value: unknown, label: string): ManagedControlSpan {
  const span = controlRecord(value, label);
  controlExactKeys(span, ["start", "end"], label);
  const start = controlSafeUnsigned(span.start, Number.MAX_SAFE_INTEGER, `${label} start`);
  const end = controlSafeUnsigned(span.end, Number.MAX_SAFE_INTEGER, `${label} end`);
  if (end < start) throw new CodeControlRpcProtocolError(`${label} end precedes start`);
  return { start, end };
}

function parseManagedControlSchema(value: unknown, label: string): ManagedControlSchema {
  const schema = controlRecord(value, label);
  const kind = controlString(schema.kind, `${label} kind`);
  switch (kind) {
    case "number":
      controlExactKeys(schema, ["kind", "number", "minimum", "maximum"], label);
      return {
        kind,
        number: controlEnum(schema.number, CONTROL_NUMBER_KINDS, `${label} number kind`),
        minimum: parseOptionalControlBound(schema.minimum, `${label} minimum`),
        maximum: parseOptionalControlBound(schema.maximum, `${label} maximum`),
      };
    case "unit":
      controlExactKeys(schema, ["kind", "unit", "number", "minimum", "maximum"], label);
      return {
        kind,
        unit: controlKey(schema.unit, `${label} unit`),
        number: controlEnum(schema.number, CONTROL_NUMBER_KINDS, `${label} number kind`),
        minimum: parseOptionalControlBound(schema.minimum, `${label} minimum`),
        maximum: parseOptionalControlBound(schema.maximum, `${label} maximum`),
      };
    case "boolean":
    case "text":
      controlExactKeys(schema, ["kind"], label);
      return { kind };
    case "choice":
      controlExactKeys(schema, ["kind", "choices"], label);
      return {
        kind,
        choices: controlArray(schema.choices, MAX_MANAGED_CONTROL_COUNT, `${label} choices`)
          .map((choice, index) => controlString(choice, `${label} choice ${index}`)),
      };
    default:
      throw new CodeControlRpcProtocolError(`${label} has unknown kind ${JSON.stringify(kind)}`);
  }
}

function parseOptionalControlBound(value: unknown, label: string): ManagedControlBound | null {
  if (value === null) return null;
  const bound = controlRecord(value, label);
  controlExactKeys(bound, ["value", "inclusive"], label);
  return {
    value: controlFinite(bound.value, `${label} value`),
    inclusive: controlBoolean(bound.inclusive, `${label} inclusive flag`),
  };
}

function parseManagedControlAccess(
  value: unknown,
  label: string,
  state: { valueNodes: number },
): ManagedControlAccess {
  const access = controlRecord(value, label);
  const kind = controlString(access.access, `${label} kind`);
  if (kind === "editable") {
    controlExactKeys(access, ["access", "token"], label);
    return { access: kind, token: parseManagedControlToken(access.token, `${label} token`, state) };
  }
  if (kind === "read_only") {
    controlExactKeys(access, ["access", "reason", "navigation"], label);
    return {
      access: kind,
      reason: controlEnum(access.reason, CONTROL_READ_ONLY_REASONS, `${label} reason`),
      navigation: access.navigation === null
        ? null
        : parseManagedControlNavigation(access.navigation, `${label} navigation`),
    };
  }
  throw new CodeControlRpcProtocolError(`${label} has unknown access ${JSON.stringify(kind)}`);
}

function parseManagedControlNavigation(value: unknown, label: string): ManagedControlNavigation {
  const navigation = controlRecord(value, label);
  controlExactKeys(navigation, ["declaration", "path"], label);
  return {
    declaration: controlKey(navigation.declaration, `${label} declaration`),
    path: parseControlPath(navigation.path, `${label} path`),
  };
}

function parseManagedControlToken(
  value: unknown,
  label: string,
  state: { valueNodes: number },
): ManagedControlToken {
  const token = controlRecord(value, label);
  controlExactKeys(
    token,
    [
      "id",
      "project",
      "project_digest",
      "source_digest",
      "declaration",
      "path",
      "expected",
      "generation_digest",
      "authentication",
    ],
    label,
  );
  state.valueNodes = 0;
  return {
    id: controlHex(token.id, 64, `${label} ID`),
    project: controlKey(token.project, `${label} project`),
    project_digest: controlHex(token.project_digest, 64, `${label} project digest`),
    source_digest: controlHex(token.source_digest, 64, `${label} source digest`),
    declaration: controlKey(token.declaration, `${label} declaration`),
    path: parseControlPath(token.path, `${label} path`),
    expected: parseManagedValue(token.expected, `${label} expected value`, state, 0),
    generation_digest: controlHex(token.generation_digest, 64, `${label} generation digest`),
    authentication: controlHex(token.authentication, 64, `${label} authentication`),
  };
}

function parseManagedControlConsumer(value: unknown, label: string): ManagedControlConsumer {
  const consumer = controlRecord(value, label);
  controlExactKeys(consumer, ["target", "property"], label);
  return {
    target: parseManagedControlConsumerTarget(consumer.target, `${label} target`),
    property: parseControlPath(consumer.property, `${label} property`),
  };
}

function parseManagedControlConsumerTarget(
  value: unknown,
  label: string,
): ManagedControlConsumerTarget {
  const consumer = controlRecord(value, label);
  const target = controlString(consumer.target, `${label} kind`);
  if (target === "declaration") {
    controlExactKeys(consumer, ["target", "declaration", "family"], label);
    return {
      target,
      declaration: controlKey(consumer.declaration, `${label} declaration`),
      family: controlKey(consumer.family, `${label} family`),
    };
  }
  if (target === "generated") {
    controlExactKeys(
      consumer,
      ["target", "address", "identity", "artifact_digest", "family"],
      label,
    );
    return {
      target,
      address: parseGeneratedMemberAddress(consumer.address, `${label} address`),
      identity: parseGeneratedMemberIdentity(consumer.identity, `${label} identity`),
      artifact_digest: controlHex(consumer.artifact_digest, 64, `${label} artifact digest`),
      family: controlKey(consumer.family, `${label} family`),
    };
  }
  throw new CodeControlRpcProtocolError(`${label} has unknown target ${JSON.stringify(target)}`);
}

function parseGeneratedMemberAddress(value: unknown, label: string): GeneratedMemberAddress {
  const address = controlRecord(value, label);
  controlExactKeys(address, ["invocation", "template", "member_key", "output"], label);
  const strings = (candidate: unknown, field: string) =>
    controlArray(candidate, 64, `${label} ${field}`)
      .map((segment, index) => controlKey(segment, `${label} ${field} ${index}`));
  return {
    invocation: controlKey(address.invocation, `${label} invocation`),
    template: strings(address.template, "template"),
    member_key: strings(address.member_key, "member key"),
    output: strings(address.output, "output"),
  };
}

function parseGeneratedMemberIdentity(value: unknown, label: string): GeneratedMemberIdentity {
  const identity = controlRecord(value, label);
  controlExactKeys(identity, ["allocation", "generation"], label);
  return {
    allocation: controlSafeUnsigned(
      identity.allocation,
      Number.MAX_SAFE_INTEGER,
      `${label} allocation`,
    ),
    generation: controlSafeUnsigned(identity.generation, 0xffff_ffff, `${label} generation`),
  };
}

function parseManagedControlBatch(value: unknown, label: string): ManagedControlEditBatch {
  const batch = controlRecord(value, label);
  controlExactKeys(batch, ["edits"], label);
  const state = { valueNodes: 0 };
  const ids = new Set<string>();
  const edits = controlArray(batch.edits, MAX_MANAGED_CONTROL_COUNT, `${label} edits`)
    .map((candidate, index): ManagedControlEdit => {
      const edit = controlRecord(candidate, `${label} edit ${index}`);
      controlExactKeys(edit, ["token", "value"], `${label} edit ${index}`);
      const token = parseManagedControlToken(edit.token, `${label} edit ${index} token`, state);
      if (ids.has(token.id)) {
        throw new CodeControlRpcProtocolError(`${label} repeats control ${token.id}`);
      }
      ids.add(token.id);
      state.valueNodes = 0;
      const replacement = parseManagedValue(
        edit.value,
        `${label} edit ${index} value`,
        state,
        0,
      );
      return {
        token,
        value: replacement,
      };
    });
  if (edits.length === 0) {
    throw new CodeControlRpcProtocolError(`${label} must contain at least one edit`);
  }
  return { edits };
}

function authenticateManagedControlBatch(
  manifest: ManagedControlManifest,
  batch: ManagedControlEditBatch,
): void {
  const controls = new Map(manifest.controls.map((control) => [control.id, control]));
  for (const edit of batch.edits) {
    const control = controls.get(edit.token.id);
    if (
      control === undefined
      || control.access.access !== "editable"
      || codeControlCanonicalJson(control.access.token) !== codeControlCanonicalJson(edit.token)
    ) {
      throw new CodeControlRpcProtocolError(
        `managed-control batch token ${edit.token.id} is absent, read-only, or foreign`,
      );
    }
  }
}

function parseManagedValue(
  value: unknown,
  label: string,
  state: { valueNodes: number },
  depth: number,
): ManagedValue {
  state.valueNodes += 1;
  if (state.valueNodes > MAX_MANAGED_VALUE_NODES) {
    throw new CodeControlRpcProtocolError("managed values exceed the node limit");
  }
  if (depth > MAX_MANAGED_VALUE_DEPTH) {
    throw new CodeControlRpcProtocolError("managed value exceeds the depth limit");
  }
  const managed = controlRecord(value, label);
  const kind = controlString(managed.kind, `${label} kind`);
  switch (kind) {
    case "null":
      controlExactKeys(managed, ["kind"], label);
      return { kind };
    case "bool":
      controlExactKeys(managed, ["kind", "value"], label);
      return { kind, value: controlBoolean(managed.value, `${label} value`) };
    case "number":
      controlExactKeys(managed, ["kind", "value"], label);
      return { kind, value: controlFinite(managed.value, `${label} value`) };
    case "string":
      controlExactKeys(managed, ["kind", "value"], label);
      return { kind, value: controlString(managed.value, `${label} value`) };
    case "unit": {
      controlExactKeys(managed, ["kind", "value"], label);
      const unit = controlRecord(managed.value, `${label} unit`);
      controlExactKeys(unit, ["unit", "value"], `${label} unit`);
      return {
        kind,
        value: {
          unit: controlKey(unit.unit, `${label} unit name`),
          value: controlFinite(unit.value, `${label} unit value`),
        },
      };
    }
    case "array":
      controlExactKeys(managed, ["kind", "value"], label);
      return {
        kind,
        value: controlArray(managed.value, MAX_MANAGED_CONTROL_COUNT, `${label} array`)
          .map((child, index) => parseManagedValue(child, `${label} item ${index}`, state, depth + 1)),
      };
    case "object": {
      controlExactKeys(managed, ["kind", "value"], label);
      const object = controlRecord(managed.value, `${label} object`);
      const entries = Object.entries(object);
      if (entries.length > MAX_MANAGED_CONTROL_COUNT) {
        throw new CodeControlRpcProtocolError(`${label} object has too many fields`);
      }
      const result = Object.fromEntries(
        entries
          .sort(([left], [right]) => byteCompare(left, right))
          .map(([key, child]) => {
            controlString(key, `${label} object field`);
            return [
              key,
              parseManagedValue(child, `${label}.${key}`, state, depth + 1),
            ] as const;
          }),
      );
      return { kind, value: result };
    }
    case "reference": {
      controlExactKeys(managed, ["kind", "value"], label);
      const reference = controlRecord(managed.value, `${label} reference`);
      controlExactKeys(reference, ["declaration", "path"], `${label} reference`);
      return {
        kind,
        value: {
          declaration: controlKey(reference.declaration, `${label} reference declaration`),
          path: parseControlPath(reference.path, `${label} reference path`),
        },
      };
    }
    default:
      throw new CodeControlRpcProtocolError(`${label} has unknown kind ${JSON.stringify(kind)}`);
  }
}

function parseControlPath(value: unknown, label: string): readonly SemanticPathSegment[] {
  return controlArray(value, MAX_CONTROL_PATH_SEGMENTS, label).map((segment, index) => {
    if (typeof segment === "string") return controlString(segment, `${label} segment ${index}`);
    if (typeof segment === "number") {
      return controlSafeUnsigned(segment, Number.MAX_SAFE_INTEGER, `${label} index ${index}`);
    }
    const member = controlRecord(segment, `${label} member ${index}`);
    controlExactKeys(member, ["member"], `${label} member ${index}`);
    return { member: controlString(member.member, `${label} member ${index}`) };
  });
}

function codeControlJson(value: unknown): string {
  if (value === null) return "null";
  if (typeof value === "string" || typeof value === "boolean") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new CodeControlRpcProtocolError("code-control numbers must be finite");
    }
    return Object.is(value, -0) ? "-0.0" : String(value);
  }
  if (Array.isArray(value)) return `[${value.map(codeControlJson).join(",")}]`;
  if (typeof value === "object" && value !== null) {
    return `{${Object.entries(value)
      .map(([key, child]) => `${JSON.stringify(key)}:${codeControlJson(child)}`)
      .join(",")}}`;
  }
  throw new CodeControlRpcProtocolError("code-control value is not finite JSON data");
}

function codeControlCanonicalJson(value: unknown): string {
  if (value === null) return "null";
  if (typeof value === "string" || typeof value === "boolean") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new CodeControlRpcProtocolError("code-control numbers must be finite");
    }
    return Object.is(value, -0) ? "-0.0" : String(value);
  }
  if (Array.isArray(value)) return `[${value.map(codeControlCanonicalJson).join(",")}]`;
  if (typeof value === "object" && value !== null) {
    return `{${Object.entries(value)
      .sort(([left], [right]) => byteCompare(left, right))
      .map(([key, child]) => `${JSON.stringify(key)}:${codeControlCanonicalJson(child)}`)
      .join(",")}}`;
  }
  throw new CodeControlRpcProtocolError("code-control value is not finite JSON data");
}

function controlRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new CodeControlRpcProtocolError(`${label} must be an object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new CodeControlRpcProtocolError(`${label} must be a plain object`);
  }
  return value as Record<string, unknown>;
}

function controlExactKeys(value: Record<string, unknown>, expected: readonly string[], label: string): void {
  const actual = Object.keys(value).sort(byteCompare);
  const wanted = [...expected].sort(byteCompare);
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    throw new CodeControlRpcProtocolError(`${label} has unknown or missing fields`);
  }
}

function controlArray(value: unknown, maximum: number, label: string): readonly unknown[] {
  if (!Array.isArray(value)) throw new CodeControlRpcProtocolError(`${label} must be an array`);
  if (value.length > maximum) {
    throw new CodeControlRpcProtocolError(`${label} exceeds ${maximum} entries`);
  }
  return value;
}

function controlString(value: unknown, label: string): string {
  if (typeof value !== "string" || controlHasUnpairedSurrogate(value)) {
    throw new CodeControlRpcProtocolError(`${label} must be a valid Unicode string`);
  }
  return value;
}

function controlBoundedString(value: unknown, maximumBytes: number, label: string): string {
  const result = controlString(value, label);
  if (new TextEncoder().encode(result).byteLength > maximumBytes) {
    throw new CodeControlRpcProtocolError(`${label} exceeds ${maximumBytes} bytes`);
  }
  return result;
}

function controlKey(value: unknown, label: string): string {
  const result = controlString(value, label);
  if (
    result.length === 0
    || new TextEncoder().encode(result).byteLength > 256
    || /\p{Cc}/u.test(result)
  ) {
    throw new CodeControlRpcProtocolError(`${label} is not a valid bounded key`);
  }
  return result;
}

function controlHex(value: unknown, width: number, label: string): string {
  const result = controlString(value, label);
  if (result.length !== width || !/^[0-9a-f]+$/u.test(result)) {
    throw new CodeControlRpcProtocolError(
      `${label} must be exactly ${width} lowercase hexadecimal characters`,
    );
  }
  return result;
}

function controlFinite(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new CodeControlRpcProtocolError(`${label} must be a finite number`);
  }
  return value;
}

function controlSafeUnsigned(value: unknown, maximum: number, label: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > maximum) {
    throw new CodeControlRpcProtocolError(`${label} must be an exactly represented unsigned integer`);
  }
  return value as number;
}

function controlBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") throw new CodeControlRpcProtocolError(`${label} must be a boolean`);
  return value;
}

function controlEnum<const Values extends readonly string[]>(
  value: unknown,
  allowed: Values,
  label: string,
): Values[number] {
  const result = controlString(value, label);
  if (!(allowed as readonly string[]).includes(result)) {
    throw new CodeControlRpcProtocolError(`${label} has unknown value ${JSON.stringify(result)}`);
  }
  return result as Values[number];
}

function requireCodeControlSize(value: string, maximum: number, label: string): void {
  if (value.length > maximum || new TextEncoder().encode(value).byteLength > maximum) {
    throw new CodeControlRpcProtocolError(`${label} exceeds ${maximum} bytes`);
  }
}

function sameCodeSessionIdentity(left: CodeSessionIdentity, right: CodeSessionIdentity): boolean {
  return left.session === right.session
    && left.revision === right.revision
    && left.digest === right.digest;
}

function controlHasUnpairedSurrogate(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (next < 0xdc00 || next > 0xdfff) return true;
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      return true;
    }
  }
  return false;
}

const CONTROL_NUMBER_KINDS = [
  "real",
  "integer",
  "natural",
] as const satisfies readonly ManagedControlNumberKind[];
const MANAGED_SPAN_KINDS = [
  "declaration",
  "symbol",
  "arguments",
  "literal",
  "reference",
  "organization",
] as const satisfies readonly ManagedOwnedSpanKind[];
const CONTROL_READ_ONLY_REASONS = [
  "structure",
  "reference",
  "structural_identity",
  "solver_instance",
  "null",
  "absent",
  "incompatible_schemas",
  "unproven_transform",
] as const satisfies readonly ManagedControlReadOnlyReason[];
const CODE_CONTROL_FAILURE_CODES = [
  "invalid_request",
  "request_too_large",
  "response_too_large",
  "code_workbench_unavailable",
  "code_workbench_busy",
  "control_inspection_rejected",
  "control_edit_rejected",
  "stale_code_session",
  "history_rejected",
  "editor_publication_rejected",
] as const satisfies readonly CodeControlFailureCode[];
