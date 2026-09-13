// SPDX-License-Identifier: GPL-3.0-or-later
import type { FolderWorkbenchAdapter } from "./folder-adapter";
import { createHostedWorkbenchSession, prepareWorkbenchSnapshot, type WorkbenchSession } from "./workbench-session";

/** Folder lease, installed source basis and edit witnesses stay with their adapter. */
export function createFolderWorkbenchSession(adapter: FolderWorkbenchAdapter): WorkbenchSession {
  return {
    ...createHostedWorkbenchSession(adapter, () => adapter.refresh()),
    projectReplacementBlockedReason: "This window follows the open local folder. Open the regular demo to create or import another project.",
    get editingBlockedReason() { return adapter.editingBlockedReason; },
    get banner() {
      return {
        region: "Local folder", label: adapter.isGenerator ? "Generator" : "Local folder",
        path: adapter.state?.paths.source, notice: adapter.notice, error: !adapter.state?.ok,
        actions: [
          ...(adapter.state?.editor && !adapter.state.editor.canEdit ? [{ label: "Take over editing", run: () => adapter.takeOver() }] : []),
          { label: "Refresh from disk", run: () => adapter.refresh() },
          ...(adapter.pendingOperationId ? [{ label: "Check save status", run: () => adapter.checkPendingOperation() }] : []),
        ],
        downloads: adapter.pending ? [{ label: "Download pending intent", filename: "geosolve-pending-intent.json", contents: () => adapter.pending }] : [],
      };
    },
    get generatorInputs() {
      return adapter.isGenerator ? {
        definitions: adapter.installedState?.inputDefinitions ?? {},
        values: adapter.installedState?.inputs ?? {},
        disabledReason: adapter.state?.editor?.canEdit === false ? "Take over editing to change generator inputs."
          : adapter.installedState?.capabilities?.generatorInputs === false ? "This generator does not expose editable inputs." : undefined,
        apply: (values: Record<string, unknown>) => adapter.setGeneratorInputs(values),
      } : undefined;
    },
    authoringEdits: adapter,
    prepareSnapshot: async (next) => adapter.prepareSnapshot(await prepareWorkbenchSnapshot(adapter, next)),
    installSnapshot: (next) => adapter.installSnapshot(next),
    subscribe: (listener) => adapter.subscribe(listener),
    draftChanged: (dirty) => adapter.draftChanged(dirty),
  };
}
