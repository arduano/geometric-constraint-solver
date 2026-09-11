// SPDX-License-Identifier: GPL-3.0-or-later

import type { ToolCatalog } from "./tool-catalog";
import type { WorkbenchActivity } from "./workbench-activity";
import { assertDrawFrame, type DrawFrame } from "./canvas-scene";
import type {
  CompiledManagedSource,
  ManagedSketchMutation,
} from "./managed-compiler";

export const WORKBENCH_PROTOCOL_VERSION = 2 as const;

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
  /** Optional opaque shared editor display identity, independent of model revision. */
  sharedRevision?:number;
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
  visible: boolean;
  effectiveVisible: boolean;
  visibilityState: "visible" | "hidden" | "mixed";
  source?: { path: string; from: number; to: number };
  children: DeclarationRow[];
  capabilities: DeclarationCapabilities;
}

export interface NavigationSnapshot {
  /** Opaque native accepted source and scene identity. */
  authority: string;
  selectionKey: string;
  rows: Array<{ id: string; state: "selected" | "partial" }>;
  sources: Array<{ path: string; from: number; to: number }>;
  itemCount: number;
  canNavigateSource: boolean;
  unavailableReason?: string;
  notice?: string;
}

export type DimensionDisplayMode = "focused" | "all" | "hidden";

/** Accepted source-owned presentation and the native authority for editing it. */
export interface AuthoringMetadataSnapshot {
  authority: string;
  target: { kind: "dimension" | "parameter" | "declaration"; id: string };
  label?: string;
  description?: string;
  isKeyConstraint?: boolean;
  isKeyParameter?: boolean;
  hasKeyOverride?: boolean;
  editable: boolean;
  reason?: string;
  canExtract?: boolean;
}

export interface AuthoringDocumentSnapshot {
  authority: string;
  title: string;
  description: string;
  areKeyConstraintsByDefault: boolean;
  editable: boolean;
  reason?: string;
}

export interface AuthoringMetadataChanges {
  label?: string | null;
  description?: string | null;
  title?: string | null;
  isKeyConstraint?: boolean | null;
  isKeyParameter?: boolean | null;
  areKeyConstraintsByDefault?: boolean | null;
}

export interface AuthoringMetadataCommand {
  authority: string;
  target: AuthoringMetadataSnapshot["target"] | { kind: "document" };
  changes: AuthoringMetadataChanges;
}

export interface AuthoringParameterExtractionCommand {
  authority: string;
  id: string;
  label?: string;
  description?: string;
  isKeyParameter?: boolean;
}

export interface ParameterEntry {
  id: string;
  rowKey?: string;
  label: string;
  value: string;
  unit?: string;
  editable: boolean;
  defaultPriority?: boolean;
  metadata?: AuthoringMetadataSnapshot;
  consumers?: string[];
}

/** Accepted native metadata: the frontend does not infer measurement ownership. */
export interface DimensionEntry {
  id: string;
  /** Stable native presentation identity; never used as command authority. */
  rowKey?: string;
  label: string;
  value: string;
  unit?: string;
  kind: string;
  reference: boolean;
  generated: boolean;
  /** Native design-intent priority, independent of the user's pins. */
  defaultPriority?: boolean;
  pinned: boolean;
  visible: boolean;
  focused: boolean;
  editable: boolean;
  reason?: string;
  contextual?: boolean;
  metadata?: AuthoringMetadataSnapshot;
}

export interface DimensionsSnapshot {
  mode: DimensionDisplayMode;
  entries: DimensionEntry[];
  parameters: ParameterEntry[];
  allMeasurements?: DimensionEntry[];
  pinCount: number;
}

export interface WorkbenchSnapshot {
  version: typeof WORKBENCH_PROTOCOL_VERSION;
  revision: number;
  project: { title: string; sampleKey?: string; status: ProjectStatus };
  presentation: { activeTool: string; gridVisible: boolean; constructionVisible: boolean; visibilityRestoreAvailable: boolean; canUndo: boolean; canRedo: boolean; canFinish: boolean; geometryRole: "profile" | "construction"; selectedGeometryRole?: "profile" | "construction" | "mixed" };
  frame: { scene: DrawFrame; ariaLabel: string };
  source: { selectedPath: string; files: SourceFileSnapshot[]; dirty: boolean };
  explorer: DeclarationRow[];
  navigation?: NavigationSnapshot;
  dimensions?: DimensionsSnapshot;
  authoringDocument?: AuthoringDocumentSnapshot;
  selection?: { id: string; label: string; kind: string; ownership?: string; source?: { path: string; from: number; to: number }; metadata?: AuthoringMetadataSnapshot };
  /** Personal retained native tool state; never accepted document authority. */
  authoringContext?: {
    construction?: import("../../../../../packages/geosolve-engine/src/construction").ConstructionFrame;
    operation?: import("../../../../../packages/geosolve-engine/src/tool-operations").ToolOperationFrame;
  };
  parameters: ParameterEntry[];
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

/** Camera-only transport results retain all non-canvas state from one validated base. */
const canvasOnlySnapshots = new WeakSet<WorkbenchSnapshot>();
const canvasSnapshotSequences = new WeakMap<WorkbenchSnapshot, number>();
/** Decode order within an adapter lifetime, independent of durable document revision. */
export function stampCanvasSnapshot(snapshot: WorkbenchSnapshot, sequence: number): WorkbenchSnapshot {
  canvasSnapshotSequences.set(snapshot, sequence);
  return snapshot;
}
export function getCanvasSnapshotSequence(snapshot: WorkbenchSnapshot): number | undefined {
  return canvasSnapshotSequences.get(snapshot);
}

export function markCanvasOnlySnapshot(snapshot: WorkbenchSnapshot): WorkbenchSnapshot {
  canvasOnlySnapshots.add(snapshot);
  return snapshot;
}
export function isCanvasOnlySnapshot(snapshot: WorkbenchSnapshot): boolean {
  return canvasOnlySnapshots.has(snapshot);
}
export interface WheelSample {
  version: 2;
  x: number;
  y: number;
  deltaX: number;
  deltaY: number;
  ctrl: boolean;
}

export interface WorkbenchAdapter {
  readonly activity?: WorkbenchActivity;
  /** Accepted-scene interaction can continue while remote edits are pending. */
  readonly responsiveCanvas?: boolean;
  readonly selectedGeometryRoleBlockedReason?: string;
  construct(input: { version: 2; persistedProject?: string }): Promise<WorkbenchSnapshot>;
  toolCatalog(): Promise<ToolCatalog>;
  snapshot(): Promise<WorkbenchSnapshot>;
  dispatch(input: { version: 2; command: string; payload?: unknown }): Promise<WorkbenchSnapshot>;
  managedCompilerContext(): Promise<{ version: 2; patches: Record<string, unknown> }>;
  pointer(input: PointerSample): Promise<WorkbenchSnapshot | null>;
  wheel(input: WheelSample): Promise<WorkbenchSnapshot | null>;
  wheelBatch?(inputs: WheelSample[]): Promise<WorkbenchSnapshot | null>;
  resize(input: { version: 2; width: number; height: number; pixelRatio: number }): Promise<WorkbenchSnapshot | null>;
  cancel(input: { version: 2; reason: "escape" | "lost-capture" | "blur" }): Promise<WorkbenchSnapshot | null>;
  exportProject(): Promise<{ version: 2; filename: string; contents: string }>;
  persistProject(): Promise<{ version: 2; contents: string }>;
  exportReproduction(): Promise<{ version: 2; filename: string; contents: string }>;
  exportInteractionTrace(): Promise<{ version: 2; filename: string; contents: string }>;
}

export function assertWorkbenchSnapshot(value: WorkbenchSnapshot): WorkbenchSnapshot {
  if (!value || typeof value !== "object" || !value.frame || typeof value.frame.ariaLabel !== "string") throw new Error("Unsupported or malformed workbench snapshot");
  assertDrawFrame(value.frame.scene);
  const geometryRole = value.presentation?.geometryRole;
  const selectedGeometryRole = value.presentation?.selectedGeometryRole;
  if (value.version !== WORKBENCH_PROTOCOL_VERSION || !Number.isSafeInteger(value.revision) || typeof value.presentation?.activeTool !== "string" || typeof value.presentation?.gridVisible !== "boolean" || typeof value.presentation?.constructionVisible !== "boolean" || typeof value.presentation?.visibilityRestoreAvailable !== "boolean" || typeof value.presentation?.canUndo !== "boolean" || typeof value.presentation?.canRedo !== "boolean" || typeof value.presentation?.canFinish !== "boolean" || (geometryRole !== "profile" && geometryRole !== "construction") || (selectedGeometryRole !== undefined && selectedGeometryRole !== "profile" && selectedGeometryRole !== "construction" && selectedGeometryRole !== "mixed") || !Array.isArray(value.explorer) || !value.explorer.every(validDeclarationRow) || (value.navigation !== undefined && !validNavigationSnapshot(value.navigation)) || (value.dimensions !== undefined && !validDimensionsSnapshot(value.dimensions)) || (value.authoringDocument !== undefined && !validAuthoringDocument(value.authoringDocument)) || (value.selection?.metadata !== undefined && !validAuthoringMetadata(value.selection.metadata)) || !Array.isArray(value.parameters) || !value.parameters.every(validParameterEntry) || (value.pendingManagedMutation !== undefined && !validPreparedManagedMutation(value.pendingManagedMutation))) {
    throw new Error("Unsupported or malformed workbench snapshot");
  }
  return value;
}

function validAuthoringMetadata(value: unknown): value is AuthoringMetadataSnapshot {
  return record(value) && typeof value.authority === "string" && value.authority.length > 0
    && record(value.target) && ["dimension", "parameter", "declaration"].includes(String(value.target.kind))
    && typeof value.target.id === "string" && value.target.id.length > 0
    && typeof value.editable === "boolean"
    && ["label", "description", "reason"].every((field) => value[field] === undefined || typeof value[field] === "string")
    && ["isKeyConstraint", "isKeyParameter", "hasKeyOverride", "canExtract"].every((field) => value[field] === undefined || typeof value[field] === "boolean")
    && (value.isKeyConstraint === undefined || value.target.kind === "dimension")
    && (value.isKeyParameter === undefined || value.target.kind === "parameter");
}

function validAuthoringDocument(value: unknown): value is AuthoringDocumentSnapshot {
  return record(value) && typeof value.authority === "string" && value.authority.length > 0
    && typeof value.title === "string" && typeof value.description === "string"
    && typeof value.areKeyConstraintsByDefault === "boolean" && typeof value.editable === "boolean"
    && (value.reason === undefined || typeof value.reason === "string");
}

function validParameterEntry(value: unknown): value is ParameterEntry {
  return record(value) && typeof value.id === "string" && value.id.length > 0
    && typeof value.label === "string" && typeof value.value === "string"
    && (value.unit === undefined || typeof value.unit === "string") && typeof value.editable === "boolean"
    && (value.rowKey === undefined || (typeof value.rowKey === "string" && value.rowKey.length > 0))
    && (value.defaultPriority === undefined || typeof value.defaultPriority === "boolean")
    && (value.metadata === undefined || validAuthoringMetadata(value.metadata))
    && (value.consumers === undefined || stringArray(value.consumers));
}

function validDimensionEntries(value: unknown): value is DimensionEntry[] {
  return Array.isArray(value) && value.every((entry) => validParameterEntry(entry) && record(entry)
    && typeof entry.kind === "string" && typeof entry.reference === "boolean"
    && typeof entry.generated === "boolean" && typeof entry.pinned === "boolean"
    && typeof entry.visible === "boolean" && typeof entry.focused === "boolean"
    && (entry.contextual === undefined || typeof entry.contextual === "boolean")
    && (entry.reason === undefined || typeof entry.reason === "string"))
    && new Set(value.map((entry) => entry.id)).size === value.length
    && new Set(value.map((entry) => entry.rowKey ?? entry.id)).size === value.length;
}

function validDimensionsSnapshot(value: unknown): value is DimensionsSnapshot {
  return record(value) && ["focused", "all", "hidden"].includes(String(value.mode))
    && safeInteger(value.pinCount, 0) && value.pinCount <= 4
    && Array.isArray(value.parameters) && value.parameters.every(validParameterEntry)
    && new Set(value.parameters.map((entry) => entry.id)).size === value.parameters.length
    && validDimensionEntries(value.entries)
    && (value.allMeasurements === undefined || validDimensionEntries(value.allMeasurements));
}

function validNavigationSnapshot(value: unknown): value is NavigationSnapshot {
  return record(value) && typeof value.authority === "string" && value.authority.length > 0
    && typeof value.selectionKey === "string" && safeInteger(value.itemCount, 0)
    && typeof value.canNavigateSource === "boolean"
    && (value.unavailableReason === undefined || typeof value.unavailableReason === "string")
    && (value.notice === undefined || typeof value.notice === "string")
    && Array.isArray(value.rows) && value.rows.every((row) => record(row) && typeof row.id === "string" && (row.state === "selected" || row.state === "partial"))
    && Array.isArray(value.sources) && value.sources.every((span) => record(span) && typeof span.path === "string" && safeInteger(span.from, 0) && safeInteger(span.to, span.from));
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
    case "set_metadata": {
      if (!record(value.target)) return false;
      const target = value.target.target;
      const properties = target === "document"
        ? ["title", "description", "areKeyConstraintsByDefault"]
        : target === "parameter" ? ["label", "description", "isKeyParameter"]
          : target === "declaration" ? ["label", "description", "isKeyConstraint"] : [];
      return (target === "document" || (typeof value.target.declaration === "string" && value.target.declaration.length > 0))
        && typeof value.property === "string" && properties.includes(value.property)
        && (value.value === null || record(value.value));
    }
    case "extract_parameter":
      return typeof value.declaration === "string" && value.declaration.length > 0
        && typeof value.symbol === "string" && value.symbol.length > 0
        && typeof value.variable === "string" && value.variable.length > 0
        && Array.isArray(value.path) && value.path.every((part) => typeof part === "string" || safeInteger(part, 0) || (record(part) && typeof part.member === "string"))
        && (value.presentation === undefined || (record(value.presentation)
          && Object.keys(value.presentation).every((field) => ["label", "description", "isKeyParameter"].includes(field))
          && (value.presentation.label === undefined || typeof value.presentation.label === "string")
          && (value.presentation.description === undefined || typeof value.presentation.description === "string")
          && (value.presentation.isKeyParameter === undefined || typeof value.presentation.isKeyParameter === "boolean")));
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
    && typeof row.visible === "boolean"
    && typeof row.effectiveVisible === "boolean"
    && (row.visibilityState === "visible" || row.visibilityState === "hidden" || row.visibilityState === "mixed")
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
