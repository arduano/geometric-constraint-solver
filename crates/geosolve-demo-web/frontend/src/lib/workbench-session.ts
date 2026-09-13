// SPDX-License-Identifier: GPL-3.0-or-later
import type { WorkbenchAdapter, WorkbenchSnapshot, DeclarationRow, AuthoringMetadataSnapshot } from "./adapter";
import type { AuthoringEditLifecycle } from "./authoring-edit";
import type { SourceEditorEdit } from "../components/code-editor";
import type { GeneratorInputDefinition } from "../components/generator-inputs";
import { resolvePendingManagedMutationSnapshot } from "./pending-managed-mutation";

export type ProjectSaveIntent = "auto" | "manual" | "replacement";
export interface SessionAction {
  readonly label: string;
  run(): void | Promise<unknown>;
}
export interface SessionDownload {
  readonly label: string;
  readonly filename: string;
  contents(): string;
}
export interface SessionBanner {
  readonly region: string;
  readonly label: string;
  readonly path?: string;
  readonly notice: string;
  readonly error?: boolean;
  readonly warning?: { readonly label: string; readonly detail: string };
  readonly participants?: { readonly count: number; readonly detail: string };
  readonly actions: readonly SessionAction[];
  readonly recoveryActions?: readonly SessionAction[];
  readonly downloads?: readonly SessionDownload[];
}
/** Shared text is independently durable; a dirty buffer need not block model edits. */
export interface SessionSharedText {
  readonly pendingCount: number;
  readonly history?: { readonly canUndo: boolean; readonly undoUnavailable?: string | null };
  editSource(path: string, edit: SourceEditorEdit): Promise<unknown>;
  undoText(redo?: boolean): Promise<unknown>;
}
export interface SessionGeneratorInputs {
  readonly definitions: Record<string, GeneratorInputDefinition>;
  readonly values: Record<string, unknown>;
  readonly disabledReason?: string;
  apply(values: Record<string, unknown>): Promise<WorkbenchSnapshot>;
}
export interface SessionOpen {
  readonly snapshot: WorkbenchSnapshot;
  readonly draft: string;
  readonly notices: readonly string[];
}
/** Host policies exposed to the UI without transport, lease or journal internals. */
export interface WorkbenchSession {
  readonly adapter: WorkbenchAdapter;
  readonly projectReplacementBlockedReason?: string;
  readonly editingBlockedReason?: string;
  readonly banner?: SessionBanner;
  readonly sharedText?: SessionSharedText;
  readonly generatorInputs?: SessionGeneratorInputs;
  readonly authoringEdits?: AuthoringEditLifecycle;
  readonly persistence: {
    readonly automatic: boolean;
    readonly safe: boolean;
    readonly label: string;
    save(intent: ProjectSaveIntent): Promise<{ issue?: string; saved: boolean }>;
    replaced(): void;
  };
  open(isCurrent: () => boolean): Promise<SessionOpen | undefined>;
  prepareSnapshot(snapshot: WorkbenchSnapshot): Promise<WorkbenchSnapshot>;
  /** Exact installation acknowledgement; observing or preparing grants no authority. */
  installSnapshot(snapshot: WorkbenchSnapshot): boolean;
  subscribe?(listener: (snapshot?: WorkbenchSnapshot) => void): () => void;
  draftChanged?(dirty: boolean): void;
  persistDraft?(snapshot: WorkbenchSnapshot, contents: string): string | null;
}

export async function prepareWorkbenchSnapshot(adapter: WorkbenchAdapter, next: WorkbenchSnapshot) {
  const resolve = () => resolvePendingManagedMutationSnapshot(adapter, next);
  return next.pendingManagedMutation && adapter.activity ? adapter.activity.track(resolve) : resolve();
}

/** The no-storage host default. Folder receipts override preparation/installation. */
export function createHostedWorkbenchSession(adapter: WorkbenchAdapter, refresh: () => Promise<unknown>): WorkbenchSession {
  const session: WorkbenchSession = {
    adapter,
    persistence: {
      automatic: false, safe: false, label: "Refresh saved file",
      async save() { await refresh(); return { saved: true }; },
      replaced() {},
    },
    async open(isCurrent) {
      const snapshot = await this.prepareSnapshot(await adapter.construct({ version: 2 }));
      if (!isCurrent()) return undefined;
      return { snapshot, draft: selectedContents(snapshot), notices: [] };
    },
    prepareSnapshot: (next) => prepareWorkbenchSnapshot(adapter, next),
    installSnapshot: () => true,
  };
  return session;
}
export function selectedContents(snapshot: WorkbenchSnapshot) {
  return snapshot.source.files.find((file) => file.path === snapshot.source.selectedPath)?.contents ?? "";
}

/** Restrict interactions without changing accepted values, geometry or authority. */
export function readOnlyWorkbenchPresentation(snapshot: WorkbenchSnapshot, reason: string): WorkbenchSnapshot {
  const result = { ...snapshot, presentation: { ...snapshot.presentation },
    dimensions: snapshot.dimensions ? { ...snapshot.dimensions } : undefined };
  result.presentation.canUndo = false;
  result.presentation.canRedo = false;
  result.presentation.canFinish = false;
  delete result.presentation.selectedGeometryRole;
  const restrict = (rows: DeclarationRow[]): DeclarationRow[] => rows.map((row) => ({ ...row,
    capabilities: { ...row.capabilities, ...Object.fromEntries(["edit", "move", "moveUp", "moveDown", "suppress", "delete"].map((action) => [action, { enabled: false, reason }])) },
    children: restrict(row.children),
  }));
  result.explorer = restrict(result.explorer);
  const metadata = (value: AuthoringMetadataSnapshot | undefined) => value ? { ...value, editable: false, canExtract: false, reason } : undefined;
  if (result.authoringDocument) result.authoringDocument = { ...result.authoringDocument, editable: false, reason };
  if (result.selection) result.selection = { ...result.selection, metadata: metadata(result.selection.metadata) };
  result.parameters = result.parameters.map((parameter) => ({ ...parameter, editable: false, metadata: metadata(parameter.metadata) }));
  if (result.dimensions) {
    result.dimensions.entries = result.dimensions.entries.map((entry) => ({ ...entry, editable: false, reason, metadata: metadata(entry.metadata) }));
    result.dimensions.parameters = result.dimensions.parameters.map((parameter) => ({ ...parameter, editable: false, metadata: metadata(parameter.metadata) }));
    if (result.dimensions.allMeasurements) result.dimensions.allMeasurements = result.dimensions.allMeasurements.map((entry) => ({ ...entry, editable: false, reason, metadata: metadata(entry.metadata) }));
  }
  return result;
}
