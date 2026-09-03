// SPDX-License-Identifier: GPL-3.0-or-later
import {
  browserStorageIssue,
  readBrowserStorage,
  removeBrowserStorage,
  type BrowserStorageResult,
} from "./browser-storage";

export const LEGACY_PROJECT_KEY = "geosolve.project.v1";
export const PROJECT_DATABASE_NAME = "geosolve.browser-projects.v1";
export const PROJECT_OBJECT_STORE = "projects";
export const PROJECT_RECORD_KEY = "current";

const SAVED_PROJECT_LABEL = "the saved project";

export interface ProjectDatabase {
  read(): Promise<string | null>;
  write(value: string): Promise<void>;
  remove(): Promise<void>;
}

export interface ProjectStore {
  read(): Promise<ProjectReadResult>;
  write(value: string): Promise<BrowserStorageResult<boolean>>;
  remove(): Promise<BrowserStorageResult<boolean>>;
}

export type ProjectReadProvenance =
  | "indexed-db"
  | "migrated-legacy"
  | "unmigrated-legacy"
  | "confirmed-empty"
  | "uncertain";

/**
 * `autosaveSafe` is false when storage could not prove whether IndexedDB
 * already contains a newer authority. The caller may still offer an explicit
 * Save action, but must not let a freshly constructed fallback overwrite an
 * unread project automatically.
 */
export interface ProjectReadResult extends BrowserStorageResult<string | null> {
  provenance: ProjectReadProvenance;
  autosaveSafe: boolean;
}

/**
 * Exact project persistence lives in IndexedDB because valid workbench/history
 * payloads can be much larger than Web Storage's small per-origin quota.
 * Operations share one queue, so a slower earlier write cannot replace a
 * newer project revision.
 */
export function createProjectStore(database: ProjectDatabase = new IndexedDbProjectDatabase()): ProjectStore {
  let tail: Promise<void> = Promise.resolve();
  const serialized = <T>(operation: () => Promise<T>) => {
    const result = tail.then(operation, operation);
    tail = result.then(() => undefined, () => undefined);
    return result;
  };

  return {
    read: () => serialized(async () => {
      let saved: string | null;
      try {
        saved = await database.read();
      } catch (error) {
        const legacy = readBrowserStorage(LEGACY_PROJECT_KEY, SAVED_PROJECT_LABEL);
        return {
          value: legacy.value,
          issue: joinIssues(browserStorageIssue("read", SAVED_PROJECT_LABEL, error), legacy.issue),
          provenance: "uncertain",
          autosaveSafe: false,
        };
      }

      if (saved !== null) {
        const removed = removeBrowserStorage(LEGACY_PROJECT_KEY, "the migrated legacy saved project");
        return {
          value: saved,
          issue: removed.issue,
          provenance: "indexed-db",
          autosaveSafe: true,
        };
      }

      const legacy = readBrowserStorage(LEGACY_PROJECT_KEY, "the legacy saved project");
      if (legacy.issue) {
        return {
          value: null,
          issue: legacy.issue,
          provenance: "uncertain",
          autosaveSafe: false,
        };
      }
      if (legacy.value === null) {
        return {
          value: null,
          issue: null,
          provenance: "confirmed-empty",
          autosaveSafe: true,
        };
      }

      try {
        await database.write(legacy.value);
      } catch (error) {
        return {
          value: legacy.value,
          issue: browserStorageIssue("write", "the migrated saved project", error),
          provenance: "unmigrated-legacy",
          autosaveSafe: true,
        };
      }
      const removed = removeBrowserStorage(LEGACY_PROJECT_KEY, "the migrated legacy saved project");
      return {
        value: legacy.value,
        issue: removed.issue,
        provenance: "migrated-legacy",
        autosaveSafe: true,
      };
    }),
    write: (value) => serialized(async () => {
      try {
        await database.write(value);
      } catch (error) {
        return { value: false, issue: browserStorageIssue("write", SAVED_PROJECT_LABEL, error) };
      }
      const removed = removeBrowserStorage(LEGACY_PROJECT_KEY, "the migrated legacy saved project");
      return { value: true, issue: removed.issue };
    }),
    remove: () => serialized(async () => {
      let databaseIssue: string | null = null;
      try {
        await database.remove();
      } catch (error) {
        databaseIssue = browserStorageIssue("remove", SAVED_PROJECT_LABEL, error);
      }
      const legacy = removeBrowserStorage(LEGACY_PROJECT_KEY, "the unrestorable legacy saved project");
      const issue = joinIssues(databaseIssue, legacy.issue);
      return { value: issue === null, issue };
    }),
  };
}

class IndexedDbProjectDatabase implements ProjectDatabase {
  private connection: Promise<IDBDatabase> | null = null;

  async read() {
    const database = await this.open();
    return new Promise<string | null>((resolve, reject) => {
      const transaction = database.transaction(PROJECT_OBJECT_STORE, "readonly");
      const request = transaction.objectStore(PROJECT_OBJECT_STORE).get(PROJECT_RECORD_KEY);
      let value: string | null = null;
      request.onsuccess = () => {
        value = typeof request.result === "string" ? request.result : null;
      };
      transaction.oncomplete = () => resolve(value);
      transaction.onerror = () => reject(transaction.error ?? request.error ?? new Error("IndexedDB read failed"));
      transaction.onabort = () => reject(transaction.error ?? request.error ?? new Error("IndexedDB read was aborted"));
    });
  }

  async write(value: string) {
    const database = await this.open();
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction(PROJECT_OBJECT_STORE, "readwrite");
      const request = transaction.objectStore(PROJECT_OBJECT_STORE).put(value, PROJECT_RECORD_KEY);
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? request.error ?? new Error("IndexedDB write failed"));
      transaction.onabort = () => reject(transaction.error ?? request.error ?? new Error("IndexedDB write was aborted"));
    });
  }

  async remove() {
    const database = await this.open();
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction(PROJECT_OBJECT_STORE, "readwrite");
      const request = transaction.objectStore(PROJECT_OBJECT_STORE).delete(PROJECT_RECORD_KEY);
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? request.error ?? new Error("IndexedDB removal failed"));
      transaction.onabort = () => reject(transaction.error ?? request.error ?? new Error("IndexedDB removal was aborted"));
    });
  }

  private open() {
    if (this.connection) return this.connection;
    this.connection = new Promise<IDBDatabase>((resolve, reject) => {
      if (!globalThis.indexedDB) {
        reject(new Error("IndexedDB is unavailable"));
        return;
      }
      const request = globalThis.indexedDB.open(PROJECT_DATABASE_NAME, 1);
      let settled = false;
      request.onupgradeneeded = () => {
        if (!request.result.objectStoreNames.contains(PROJECT_OBJECT_STORE)) {
          request.result.createObjectStore(PROJECT_OBJECT_STORE);
        }
      };
      request.onsuccess = () => {
        const database = request.result;
        if (settled) {
          database.close();
          return;
        }
        settled = true;
        database.onversionchange = () => {
          database.close();
          this.connection = null;
        };
        resolve(database);
      };
      request.onerror = () => {
        if (settled) return;
        settled = true;
        reject(request.error ?? new Error("IndexedDB could not be opened"));
      };
      request.onblocked = () => {
        if (settled) return;
        settled = true;
        reject(new Error("IndexedDB upgrade is blocked by another tab"));
      };
    });
    this.connection.catch(() => {
      this.connection = null;
    });
    return this.connection;
  }
}

function joinIssues(...issues: Array<string | null>) {
  return issues.filter((issue): issue is string => Boolean(issue)).join("; ") || null;
}
