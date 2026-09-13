// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it, vi } from "vitest";
import { createBrowserWorkbenchSession } from "./browser-workbench-session";
import { MockWorkbenchAdapter } from "./mock-adapter";
import type { ProjectReadResult, ProjectStore } from "./project-storage";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((yes) => { resolve = yes; });
  return { promise, resolve };
}
const empty: ProjectReadResult = { value: null, issue: null, provenance: "confirmed-empty", autosaveSafe: true };
function store(read: ProjectStore["read"] = async () => empty): ProjectStore {
  return { read, write: vi.fn(async () => ({ value: true, issue: null })), remove: vi.fn(async () => ({ value: true, issue: null })) };
}

it("retires an obsolete startup read without constructing or granting save permission", async () => {
  const old = deferred<ProjectReadResult>();
  const storage = store(vi.fn().mockImplementationOnce(() => old.promise).mockResolvedValue(empty));
  const adapter = new MockWorkbenchAdapter();
  const construct = vi.spyOn(adapter, "construct");
  const session = createBrowserWorkbenchSession(adapter, storage);
  const first = session.open(() => true);
  expect((await session.open(() => true))?.snapshot.project.title).toBe("Untitled sketch");
  old.resolve({ ...empty, value: "stale saved source", autosaveSafe: false });
  expect(await first).toBeUndefined();
  expect(construct).toHaveBeenCalledTimes(1);
  expect(session.persistence.safe).toBe(true);
  expect(storage.write).not.toHaveBeenCalled();
});

it("discards an exported old project after replacement and serially saves only the replacement", async () => {
  const storage = store();
  const adapter = new MockWorkbenchAdapter();
  const session = createBrowserWorkbenchSession(adapter, storage);
  await session.open(() => true);
  const exported = deferred<{ version: 2; contents: string }>();
  const persist = vi.spyOn(adapter, "persistProject").mockImplementationOnce(() => exported.promise)
    .mockResolvedValue({ version: 2, contents: "replacement payload" });
  const old = session.persistence.save("auto");
  await vi.waitFor(() => expect(persist).toHaveBeenCalledOnce());
  session.persistence.replaced();
  const replacement = session.persistence.save("replacement");
  exported.resolve({ version: 2, contents: "old payload" });
  expect(await old).toEqual({ saved: false });
  expect(await replacement).toEqual({ saved: true, issue: undefined });
  expect(storage.write).toHaveBeenCalledExactlyOnceWith("replacement payload");
});

it("keeps an unread project and raw source protected through failed saves, then permits explicit recovery", async () => {
  const storage = store(async () => ({ ...empty, autosaveSafe: false, provenance: "uncertain", issue: "storage unread" }));
  const adapter = new MockWorkbenchAdapter();
  const session = createBrowserWorkbenchSession(adapter, storage);
  localStorage.setItem("geosolve.source-draft.v1", "protected raw bytes");
  const opened = await session.open(() => true);
  expect(opened?.notices[0]).toContain("Automatic project saving is paused");
  await session.persistence.save("auto");
  expect(storage.write).not.toHaveBeenCalled();
  expect(session.persistDraft?.(opened!.snapshot, "new fallback edit")).toBeNull();
  expect(localStorage.getItem("geosolve.source-draft.v1")).toBe("protected raw bytes");
  vi.mocked(storage.write).mockResolvedValueOnce({ value: false, issue: "disk full" });
  expect(await session.persistence.save("manual")).toEqual({ saved: false, issue: "disk full" });
  expect(session.persistence.safe).toBe(false);
  expect(await session.persistence.save("manual")).toMatchObject({ saved: true });
  expect(session.persistence.safe).toBe(true);
  session.persistDraft?.(opened!.snapshot, opened!.snapshot.source.files.find((file) => file.path === opened!.snapshot.source.selectedPath)!.contents);
  expect(localStorage.getItem("geosolve.source-draft.v1")).toBeNull();
});
