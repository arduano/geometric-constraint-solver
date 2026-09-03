// SPDX-License-Identifier: GPL-3.0-or-later

import type { ToolCatalog } from "./tool-catalog";
import type {
  CompiledManagedSource,
  ManagedSketchMutation,
} from "./managed-compiler";

export const WORKBENCH_PROTOCOL_VERSION = 1 as const;

export type WorkspaceMode = "design" | "split" | "code";
export type ProjectStatus = "accepted" | "dirty" | "failed";

export const PREPARED_MANAGED_MUTATION_FORMAT =
  "geosolve-prepared-managed-mutation-v1" as const;
export const PREPARED_MANAGED_SOURCE_FORMAT =
  "geosolve-prepared-managed-source-v1" as const;

export interface CodeSessionIdentity {
  session: number;
  revision: number;
  digest: string;
}

/** Rust-authenticated permission for one exact browser-hosted compilation. */
export interface PreparedManagedMutationTicket {
  format: typeof PREPARED_MANAGED_MUTATION_FORMAT;
  ticketDigest: string;
  project: string;
  session: CodeSessionIdentity;
  acceptedSourceDigest: string;
  acceptedIrDigest: string;
  acceptedArtifactDigest: string;
  acceptedExpansionDigest: string;
  declarationNameHighWater: number;
  candidateDeclarationNameHighWater: number;
  baseSemanticsDigest: string;
  candidateSemanticsDigest: string;
  mutation: ManagedSketchMutation;
}

/** Exact accepted compiler envelope paired with its one prepared mutation. */
export interface PreparedManagedMutationRequest {
  ticket: PreparedManagedMutationTicket;
  current: CompiledManagedSource;
}

/** Rust-authenticated permission to compile one exact raw source candidate. */
export interface PreparedManagedSourceTicket {
  format: typeof PREPARED_MANAGED_SOURCE_FORMAT;
  ticketDigest: string;
  project: string;
  session: CodeSessionIdentity;
  acceptedSourceDigest: string;
  acceptedIrDigest: string;
  acceptedArtifactDigest: string;
  acceptedExpansionDigest: string;
  declarationNameHighWater: number;
  candidateInputSourceDigest: string;
}

export interface PreparedManagedSourceRequest {
  ticket: PreparedManagedSourceTicket;
  current: CompiledManagedSource;
  candidateSource: string;
}

export type PendingManagedMutation =
  | { kind: "managed"; request: PreparedManagedMutationRequest }
  | { kind: "source"; request: PreparedManagedSourceRequest };

export interface WorkbenchProblem {
  id: string;
  severity: "error" | "warning" | "info";
  title: string;
  detail: string;
  file?: string;
  line?: number;
  column?: number;
}

export interface SourceFileSnapshot {
  path: string;
  language: "typescript" | "json" | "text";
  contents: string;
  readOnly: boolean;
}

export interface DeclarationCapability {
  enabled: boolean;
  reason?: string;
}

export interface DeclarationCapabilities {
  select: DeclarationCapability;
  navigate: DeclarationCapability;
  edit: DeclarationCapability;
  move: DeclarationCapability;
  moveUp: DeclarationCapability;
  moveDown: DeclarationCapability;
  suppress: DeclarationCapability;
  delete: DeclarationCapability;
}

/**
 * Rust-owned ordered declaration projection. `id` is a semantic row address,
 * never an Intent node/debug string. Only transient drag hover lives in React.
 */
export interface DeclarationRow {
  id: string;
  label: string;
  kind: string;
  rowKind: "group" | "declaration" | "generated";
  selected: boolean;
  suppressed?: boolean;
  source?: { path: string; from: number; to: number };
  children: DeclarationRow[];
  capabilities: DeclarationCapabilities;
}

export interface WorkbenchSnapshot {
  version: typeof WORKBENCH_PROTOCOL_VERSION;
  revision: number;
  project: { title: string; sampleKey?: string; status: ProjectStatus };
  presentation: { activeTool: string; gridVisible: boolean; canUndo: boolean; canRedo: boolean; canFinish: boolean; geometryRole: "profile" | "construction"; selectedGeometryRole?: "profile" | "construction" | "mixed" };
  frame: { svg: string; ariaLabel: string };
  source: { selectedPath: string; files: SourceFileSnapshot[]; dirty: boolean };
  explorer: DeclarationRow[];
  selection?: { id: string; label: string; kind: string; ownership?: string; source?: { path: string; from: number; to: number } };
  parameters: Array<{ id: string; label: string; value: string; unit?: string; editable: boolean }>;
  problems: WorkbenchProblem[];
  pendingManagedMutation?: PendingManagedMutation;
}

export interface PointerSample {
  version: typeof WORKBENCH_PROTOCOL_VERSION;
  phase: "down" | "move" | "up";
  pointerId: number;
  x: number;
  y: number;
  buttons: number;
  modifiers: { alt: boolean; ctrl: boolean; meta: boolean; shift: boolean };
}

export interface WorkbenchAdapter {
  construct(input: { version: 1; persistedProject?: string }): Promise<WorkbenchSnapshot>;
  toolCatalog(): Promise<ToolCatalog>;
  snapshot(): Promise<WorkbenchSnapshot>;
  dispatch(input: { version: 1; command: string; payload?: unknown }): Promise<WorkbenchSnapshot>;
  managedCompilerContext(): Promise<{ version: 1; patches: Record<string, unknown> }>;
  pointer(input: PointerSample): Promise<WorkbenchSnapshot | null>;
  wheel(input: { version: 1; x: number; y: number; deltaX: number; deltaY: number; ctrl: boolean }): Promise<WorkbenchSnapshot | null>;
  resize(input: { version: 1; width: number; height: number; pixelRatio: number }): Promise<WorkbenchSnapshot | null>;
  cancel(input: { version: 1; reason: "escape" | "lost-capture" | "blur" }): Promise<WorkbenchSnapshot | null>;
  exportProject(): Promise<{ version: 1; filename: string; contents: string }>;
  persistProject(): Promise<{ version: 1; contents: string }>;
  exportReproduction(): Promise<{ version: 1; filename: string; contents: string }>;
  exportInteractionTrace(): Promise<{ version: 1; filename: string; contents: string }>;
}

export function assertWorkbenchSnapshot(value: WorkbenchSnapshot): WorkbenchSnapshot {
  const geometryRole = value.presentation?.geometryRole;
  const selectedGeometryRole = value.presentation?.selectedGeometryRole;
  if (value.version !== WORKBENCH_PROTOCOL_VERSION || !Number.isSafeInteger(value.revision) || typeof value.presentation?.activeTool !== "string" || typeof value.presentation?.gridVisible !== "boolean" || typeof value.presentation?.canUndo !== "boolean" || typeof value.presentation?.canRedo !== "boolean" || typeof value.presentation?.canFinish !== "boolean" || (geometryRole !== "profile" && geometryRole !== "construction") || (selectedGeometryRole !== undefined && selectedGeometryRole !== "profile" && selectedGeometryRole !== "construction" && selectedGeometryRole !== "mixed") || !Array.isArray(value.explorer) || !value.explorer.every(validDeclarationRow) || (value.pendingManagedMutation !== undefined && !validPreparedManagedMutation(value.pendingManagedMutation))) {
    throw new Error("Unsupported or malformed workbench snapshot");
  }
  return value;
}

function validPreparedManagedMutation(value: unknown): value is PendingManagedMutation {
  if (!record(value) || !record(value.request)) return false;
  if ((value.kind !== "managed" && value.kind !== "source") || !record(value.request.ticket) || !record(value.request.current)) {
    return false;
  }
  const ticket = value.request.ticket;
  const current = value.request.current;
  const common = digest(ticket.ticketDigest)
    && typeof ticket.project === "string"
    && ticket.project.length > 0
    && validSessionIdentity(ticket.session)
    && digest(ticket.acceptedSourceDigest)
    && digest(ticket.acceptedIrDigest)
    && digest(ticket.acceptedArtifactDigest)
    && digest(ticket.acceptedExpansionDigest)
    && safeInteger(ticket.declarationNameHighWater, 0)
    && digest(current.inputSourceDigest)
    && typeof current.normalizedSource === "string"
    && record(current.ir)
    && record(current.artifact)
    && typeof current.canonicalIrJson === "string"
    && typeof current.canonicalArtifactJson === "string";
  if (!common) return false;
  if (value.kind === "source") {
    return ticket.format === PREPARED_MANAGED_SOURCE_FORMAT
      && digest(ticket.candidateInputSourceDigest)
      && typeof value.request.candidateSource === "string";
  }
  return ticket.format === PREPARED_MANAGED_MUTATION_FORMAT
    && safeInteger(ticket.candidateDeclarationNameHighWater, 0)
    && digest(ticket.baseSemanticsDigest)
    && digest(ticket.candidateSemanticsDigest)
    && validManagedMutation(ticket.mutation);
}

function validSessionIdentity(value: unknown): value is CodeSessionIdentity {
  return record(value)
    && safeInteger(value.session, 1)
    && safeInteger(value.revision, 0)
    && digest(value.digest);
}

function validManagedMutation(value: unknown): value is ManagedSketchMutation {
  if (!record(value) || typeof value.mutation !== "string") return false;
  switch (value.mutation) {
    case "insert_declarations":
      return Array.isArray(value.declarations) && value.declarations.every((draft) =>
        record(draft)
        && typeof draft.variable === "string"
        && typeof draft.symbol === "string"
        && Array.isArray(draft.builder_path)
        && draft.builder_path.every((part) => typeof part === "string")
        && record(draft.arguments)
        && (draft.patch === undefined || draft.patch === null || typeof draft.patch === "string")
        && (draft.group === undefined || typeof draft.group === "string")
        && (draft.suppressed === undefined || typeof draft.suppressed === "boolean")
        && (draft.comments === undefined || (Array.isArray(draft.comments) && draft.comments.every((comment) => typeof comment === "string")))
      );
    case "reorder_declaration":
      return typeof value.declaration === "string"
        && (value.before === null || typeof value.before === "string");
    case "set_suppressed":
      return validManagedMutationTarget(value.target)
        && typeof value.suppressed === "boolean";
    case "set_value":
      return typeof value.declaration === "string"
        && Array.isArray(value.path)
        && value.path.every((part) =>
          typeof part === "string"
          || safeInteger(part, 0)
          || (record(part) && typeof part.member === "string")
        )
        && record(value.expected)
        && record(value.value);
    case "set_values":
      return Array.isArray(value.values)
        && value.values.length > 0
        && value.values.every((entry) =>
          record(entry)
          && typeof entry.declaration === "string"
          && Array.isArray(entry.path)
          && entry.path.every((part) =>
            typeof part === "string"
            || safeInteger(part, 0)
            || (record(part) && typeof part.member === "string")
          )
          && record(entry.expected)
          && record(entry.value)
        );
    case "delete":
      return validManagedMutationTarget(value.target);
    default:
      return false;
  }
}

function validManagedMutationTarget(value: unknown): boolean {
  if (!record(value)) return false;
  if (value.target === "declaration") return typeof value.declaration === "string";
  if (value.target !== "generated" || !record(value.address)) return false;
  return typeof value.address.invocation === "string"
    && stringArray(value.address.template)
    && stringArray(value.address.member_key)
    && stringArray(value.address.output);
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function digest(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{64}$/u.test(value);
}

function safeInteger(value: unknown, minimum: number): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= minimum;
}

function stringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((part) => typeof part === "string");
}

function validDeclarationRow(row: DeclarationRow): boolean {
  const capability = (value: DeclarationCapability | undefined) => typeof value?.enabled === "boolean" && (value.reason === undefined || typeof value.reason === "string");
  const actions = row?.capabilities;
  return Boolean(row)
    && typeof row.id === "string"
    && typeof row.label === "string"
    && typeof row.kind === "string"
    && (row.rowKind === "group" || row.rowKind === "declaration" || row.rowKind === "generated")
    && typeof row.selected === "boolean"
    && (row.suppressed === undefined || typeof row.suppressed === "boolean")
    && (row.source === undefined || (typeof row.source.path === "string" && Number.isSafeInteger(row.source.from) && Number.isSafeInteger(row.source.to) && row.source.from >= 0 && row.source.to >= row.source.from))
    && Array.isArray(row.children)
    && row.children.every(validDeclarationRow)
    && capability(actions?.select)
    && capability(actions?.navigate)
    && capability(actions?.edit)
    && capability(actions?.move)
    && capability(actions?.moveUp)
    && capability(actions?.moveDown)
    && capability(actions?.suppress)
    && capability(actions?.delete);
}
