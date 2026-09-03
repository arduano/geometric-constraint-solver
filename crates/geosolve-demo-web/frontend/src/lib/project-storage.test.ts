// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import {
  LEGACY_PROJECT_KEY,
  createProjectStore,
  type ProjectDatabase,
} from "./project-storage";

class MemoryProjectDatabase implements ProjectDatabase {
  reads = 0;
  writes: string[] = [];
  removals = 0;
  readError: unknown = null;
  writeError: unknown = null;
  removeError: unknown = null;
  beforeWrite: ((value: string) => Promise<void>) | null = null;

  constructor(public value: string | null = null) {}

  async read() {
    this.reads += 1;
    if (this.readError) throw this.readError;
    return this.value;
  }

  async write(value: string) {
    this.writes.push(value);
    if (this.beforeWrite) await this.beforeWrite(value);
    if (this.writeError) throw this.writeError;
    this.value = value;
  }

  async remove() {
    this.removals += 1;
    if (this.removeError) throw this.removeError;
    this.value = null;
  }
}

describe("browser project persistence", () => {
  it("migrates one legacy localStorage project only after IndexedDB accepts it", async () => {
    const database = new MemoryProjectDatabase();
    localStorage.setItem(LEGACY_PROJECT_KEY, "legacy-project");

    const restored = await createProjectStore(database).read();

    expect(restored).toEqual({
      value: "legacy-project",
      issue: null,
      provenance: "migrated-legacy",
      autosaveSafe: true,
    });
    expect(database.value).toBe("legacy-project");
    expect(localStorage.getItem(LEGACY_PROJECT_KEY)).toBeNull();
  });

  it("retains and restores the legacy project when migration cannot commit", async () => {
    const database = new MemoryProjectDatabase();
    database.writeError = new DOMException("Storage quota exceeded", "QuotaExceededError");
    localStorage.setItem(LEGACY_PROJECT_KEY, "legacy-project");

    const restored = await createProjectStore(database).read();

    expect(restored.value).toBe("legacy-project");
    expect(restored.issue).toBe(
      "Browser storage could not write the migrated saved project: QuotaExceededError: Storage quota exceeded",
    );
    expect(restored.provenance).toBe("unmigrated-legacy");
    expect(restored.autosaveSafe).toBe(true);
    expect(localStorage.getItem(LEGACY_PROJECT_KEY)).toBe("legacy-project");
  });

  it("prefers the IndexedDB authority and removes a stale legacy duplicate", async () => {
    const database = new MemoryProjectDatabase("indexed-project");
    localStorage.setItem(LEGACY_PROJECT_KEY, "stale-legacy-project");

    await expect(createProjectStore(database).read()).resolves.toEqual({
      value: "indexed-project",
      issue: null,
      provenance: "indexed-db",
      autosaveSafe: true,
    });
    expect(localStorage.getItem(LEGACY_PROJECT_KEY)).toBeNull();
  });

  it("serializes writes so the latest requested revision wins", async () => {
    const database = new MemoryProjectDatabase();
    let releaseFirst!: () => void;
    const firstBlocked = new Promise<void>((resolve) => { releaseFirst = resolve; });
    database.beforeWrite = async (value) => {
      if (value === "older") await firstBlocked;
    };
    const store = createProjectStore(database);

    const older = store.write("older");
    const newer = store.write("newer");
    await Promise.resolve();
    await Promise.resolve();
    expect(database.writes).toEqual(["older"]);

    releaseFirst();
    await expect(Promise.all([older, newer])).resolves.toEqual([
      { value: true, issue: null },
      { value: true, issue: null },
    ]);
    expect(database.writes).toEqual(["older", "newer"]);
    expect(database.value).toBe("newer");
  });

  it("stores an exact project larger than the Web Storage quota without using localStorage", async () => {
    const database = new MemoryProjectDatabase();
    const project = `exact-project:${"x".repeat(6 * 1024 * 1024)}`;
    const store = createProjectStore(database);

    await expect(store.write(project)).resolves.toEqual({ value: true, issue: null });
    await expect(store.read()).resolves.toEqual({
      value: project,
      issue: null,
      provenance: "indexed-db",
      autosaveSafe: true,
    });
    expect(database.value).toBe(project);
    expect(localStorage.getItem(LEGACY_PROJECT_KEY)).toBeNull();
  });

  it("returns the last legacy value and a durable issue if IndexedDB cannot be read", async () => {
    const database = new MemoryProjectDatabase();
    database.readError = new DOMException("Access is blocked", "SecurityError");
    localStorage.setItem(LEGACY_PROJECT_KEY, "legacy-fallback");

    const restored = await createProjectStore(database).read();

    expect(restored.value).toBe("legacy-fallback");
    expect(restored.issue).toBe(
      "Browser storage could not read the saved project: SecurityError: Access is blocked",
    );
    expect(restored.provenance).toBe("uncertain");
    expect(restored.autosaveSafe).toBe(false);
    expect(localStorage.getItem(LEGACY_PROJECT_KEY)).toBe("legacy-fallback");
  });

  it("marks a confirmed empty project slot as safe for later autosave", async () => {
    const restored = await createProjectStore(new MemoryProjectDatabase()).read();

    expect(restored).toEqual({
      value: null,
      issue: null,
      provenance: "confirmed-empty",
      autosaveSafe: true,
    });
  });

  it("does not authorize autosave when neither persistence authority could be read", async () => {
    const database = new MemoryProjectDatabase("unread-indexed-project");
    database.readError = new DOMException("Temporary read failure", "UnknownError");

    const restored = await createProjectStore(database).read();

    expect(restored.value).toBeNull();
    expect(restored.provenance).toBe("uncertain");
    expect(restored.autosaveSafe).toBe(false);
    expect(database.value).toBe("unread-indexed-project");
    expect(database.writes).toEqual([]);
  });

  it("does not authorize autosave when IndexedDB is empty but the legacy slot is unreadable", async () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("Legacy storage is blocked", "SecurityError");
    });

    const restored = await createProjectStore(new MemoryProjectDatabase()).read();
    getItem.mockRestore();

    expect(restored.value).toBeNull();
    expect(restored.issue).toContain("SecurityError: Legacy storage is blocked");
    expect(restored.provenance).toBe("uncertain");
    expect(restored.autosaveSafe).toBe(false);
  });

  it("removes a stale legacy project after any later successful IndexedDB write", async () => {
    const database = new MemoryProjectDatabase();
    database.writeError = new DOMException("Temporary write failure", "UnknownError");
    localStorage.setItem(LEGACY_PROJECT_KEY, "legacy-project");
    const store = createProjectStore(database);
    const restored = await store.read();
    expect(restored.provenance).toBe("unmigrated-legacy");

    database.writeError = null;
    await expect(store.write("new-project")).resolves.toEqual({ value: true, issue: null });

    expect(database.value).toBe("new-project");
    expect(localStorage.getItem(LEGACY_PROJECT_KEY)).toBeNull();
  });
});
