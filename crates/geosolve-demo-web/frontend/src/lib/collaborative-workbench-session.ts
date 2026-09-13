// SPDX-License-Identifier: GPL-3.0-or-later
import type { CollaborativeWorkbenchAdapter } from "./collaboration-adapter";
import { createHostedWorkbenchSession, type SessionSharedText, type WorkbenchSession } from "./workbench-session";

/** Shared text/outbox and personal history remain separate from model persistence. */
export function createCollaborativeWorkbenchSession(adapter: CollaborativeWorkbenchAdapter): WorkbenchSession {
  const sharedText: SessionSharedText = {
    get pendingCount() { return adapter.pendingSourceEdits.length; },
    get history() { return adapter.textHistory; },
    editSource: (path, edit) => adapter.editSource(path, edit),
    undoText: (redo) => adapter.undoText(redo),
  };
  return {
    ...createHostedWorkbenchSession(adapter, () => adapter.refresh()),
    projectReplacementBlockedReason: "This window follows a shared document. Open the regular demo to create or import another project.",
    get editingBlockedReason() { return adapter.editingBlockedReason; },
    sharedText,
    get banner() {
      const mirror = adapter.state?.document.mirror;
      return {
        region: "Shared document", label: "Shared document", notice: adapter.notice,
        warning: mirror && mirror.status !== "synchronized" ? {
          label: "External files need reconciliation", detail: mirror.notices.map((item) => `${item.path}: ${item.reason}`).join("\n"),
        } : undefined,
        participants: { count: adapter.participants.length, detail: adapter.participants.map((person) => `${person.userId} (${person.role})`).join(", ") },
        actions: [{ label: "Reconnect", run: () => adapter.client.retry() }],
        recoveryActions: adapter.recoveryActions,
        downloads: adapter.pending.length || adapter.pendingSourceEdits.length ? [{
          label: "Download pending work", filename: "pending-shared-work.json",
          contents: () => JSON.stringify({ requests: adapter.client.checkpoint(), sourceEdits: adapter.pendingSourceEdits }, null, 2),
        }] : [],
      };
    },
    subscribe: (listener) => adapter.subscribe(listener),
  };
}
