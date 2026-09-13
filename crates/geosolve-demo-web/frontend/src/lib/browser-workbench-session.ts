// SPDX-License-Identifier: GPL-3.0-or-later
import type { WorkbenchAdapter, WorkbenchSnapshot } from "./adapter";
import { readBrowserStorage, removeBrowserStorage, writeBrowserStorage } from "./browser-storage";
import { createProjectStore, type ProjectStore } from "./project-storage";
import { prepareWorkbenchSnapshot, selectedContents, type WorkbenchSession } from "./workbench-session";

const DRAFT_KEY = "geosolve.source-draft.v1";
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);

/** Browser storage owns restoration safety, queued saves and protected raw drafts. */
export function createBrowserWorkbenchSession(adapter: WorkbenchAdapter, store: ProjectStore = createProjectStore()): WorkbenchSession {
  let epoch = 0;
  let safe = false;
  let lastSaved: string | null | undefined;
  let saveTail: Promise<unknown> = Promise.resolve();
  const session: WorkbenchSession = {
    adapter,
    persistence: {
      automatic: true, label: "Save in browser",
      get safe() { return safe; },
      replaced() { epoch += 1; },
      save(intent) {
        const requestedEpoch = epoch;
        const save = saveTail.then(async () => {
          if (epoch !== requestedEpoch || (intent !== "manual" && !safe)) return { saved: false };
          const payload = await adapter.persistProject();
          if (epoch !== requestedEpoch) return { saved: false };
          if (intent !== "manual" && payload.contents === lastSaved && safe) return { saved: true };
          const stored = await store.write(payload.contents);
          if (!stored.value) return { saved: false, issue: stored.issue ?? "Browser storage could not write the saved project" };
          if (epoch === requestedEpoch) { lastSaved = payload.contents; safe = true; }
          return { saved: true, issue: stored.issue ?? undefined };
        });
        saveTail = save.catch(() => undefined);
        return save;
      },
    },
    async open(isCurrent) {
      const openingEpoch = ++epoch;
      safe = false; lastSaved = undefined;
      const current = () => isCurrent() && openingEpoch === epoch;
      const stored = await store.read();
      if (!current()) return undefined;
      lastSaved = stored.value; safe = stored.autosaveSafe;
      const notices: string[] = [];
      if (stored.issue) notices.push(stored.autosaveSafe ? stored.issue
        : `${stored.issue}; Automatic project saving is paused until you use Save in browser.`);
      let snapshot: WorkbenchSnapshot;
      try {
        snapshot = await session.prepareSnapshot(await adapter.construct({ version: 2, persistedProject: stored.value ?? undefined }));
      } catch (error) {
        if (!current()) return undefined;
        if (!stored.value) throw Error([errorText(error), stored.issue].filter(Boolean).join("; "));
        safe = false;
        snapshot = await session.prepareSnapshot(await adapter.construct({ version: 2 }));
        notices.push(!stored.autosaveSafe
          ? `The fallback saved workspace could not be restored: ${errorText(error)}; IndexedDB authority was unread, so the existing browser data was retained and automatic project saving remains paused until you use Save in browser.`
          : `Saved workspace could not be restored: ${errorText(error)}; your saved data was retained. Automatic project saving is paused until you use Save in browser.`);
      }
      if (!current()) return undefined;
      const restored = restoredBrowserDraft(snapshot);
      if (restored.issue) notices.push(restored.issue);
      return { snapshot, draft: restored.contents ?? selectedContents(snapshot), notices };
    },
    prepareSnapshot: (next) => prepareWorkbenchSnapshot(adapter, next),
    installSnapshot: () => true,
    persistDraft(snapshot, contents) {
      // A failed/uncertain restore protects its raw draft until explicit Save succeeds.
      if (!safe) return null;
      const file = snapshot.source.files.find((candidate) => candidate.path === snapshot.source.selectedPath);
      if (!file || file.readOnly || contents === file.contents) return removeBrowserStorage(DRAFT_KEY, "the unapplied source draft").issue;
      return writeBrowserStorage(DRAFT_KEY, JSON.stringify({ version: 1, title: snapshot.project.title,
        sampleKey: snapshot.project.sampleKey ?? null, path: file.path, base: file.contents, contents }), "the unapplied source draft").issue;
    },
  };
  return session;
}
function restoredBrowserDraft(snapshot: WorkbenchSnapshot) {
  const file = snapshot.source.files.find((candidate) => candidate.path === snapshot.source.selectedPath);
  if (!file || file.readOnly) return { contents: null, issue: null };
  const stored = readBrowserStorage(DRAFT_KEY, "the unapplied source draft");
  if (stored.issue) return { contents: null, issue: stored.issue };
  try {
    const saved = JSON.parse(stored.value ?? "null") as { version?: unknown; title?: unknown; sampleKey?: unknown; path?: unknown; base?: unknown; contents?: unknown } | null;
    if (saved?.version !== 1 || saved.title !== snapshot.project.title || (saved.sampleKey ?? null) !== (snapshot.project.sampleKey ?? null) || saved.path !== file.path || saved.base !== file.contents || typeof saved.contents !== "string") return { contents: null, issue: null };
    return { contents: saved.contents, issue: null };
  } catch {
    // Reading never cleans up a draft while fallback authority may still be unresolved.
    return { contents: null, issue: null };
  }
}
