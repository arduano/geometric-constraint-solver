// SPDX-License-Identifier: GPL-3.0-or-later
import type { PendingCheckpoint } from "../../../../../packages/geosolve-collaboration/src/client";

interface Owner {
  readonly format: "geosolve-client-owner-v1";
  readonly scope: string;
  readonly clientId: string;
  readonly ownerId: string | null;
  readonly fence: number;
  readonly expiresAt: number;
}
export interface PendingClaimOptions {
  readonly scope: string;
  readonly clientId: string;
  /** Random page lifetime token; never recovered from copied sessionStorage. */
  readonly ownerId: string;
  readonly leaseMs: number;
  readonly onLost?: (error: Error) => void;
}
export class CollaborationOwnershipLostError extends Error {
  constructor() { super("This editor no longer owns its pending work. Reload to recover it; its saved requests have been preserved."); }
}
export interface RetainedCollaborationWork {
  readonly clientId: string;
  readonly pendingRequests: number;
  readonly ownedUntil: number | null;
}

let opening: Promise<IDBDatabase> | undefined;
function open(): Promise<IDBDatabase> {
  if (opening) return opening;
  opening = new Promise<IDBDatabase>((resolve, reject) => {
    if (!globalThis.indexedDB) { reject(Error("Browser storage is unavailable; shared editing requires a persistent outbox")); return; }
    // Keep v1 outboxes and their exact keys. The version adds ownership records.
    const request = indexedDB.open("geosolve.collaboration.v1", 2);
    let settled = false;
    request.onupgradeneeded = () => {
      for (const store of ["outboxes", "owners"]) {
        if (!request.result.objectStoreNames.contains(store)) request.result.createObjectStore(store);
      }
    };
    request.onsuccess = () => {
      const database = request.result;
      if (settled) { database.close(); return; }
      settled = true;
      database.onversionchange = () => { database.close(); opening = undefined; };
      resolve(database);
    };
    request.onerror = () => { if (!settled) { settled = true; reject(request.error ?? Error("Shared work storage could not open")); } };
    request.onblocked = () => { if (!settled) { settled = true; reject(Error("Another tab is blocking shared work storage; close or reload the older editor. Its work has been preserved.")); } };
  });
  void opening.catch(() => { opening = undefined; });
  return opening;
}

type Stores = { owners: IDBObjectStore; outboxes: IDBObjectStore };
async function records<T>(key: string, mode: IDBTransactionMode,
  run: (stores: Stores, owner: unknown, pending: unknown) => T, readPending = true): Promise<T> {
  const database = await open();
  return new Promise<T>((resolve, reject) => {
    const transaction = database.transaction(["owners", "outboxes"], mode);
    const stores = { owners: transaction.objectStore("owners"), outboxes: transaction.objectStore("outboxes") };
    const owner = stores.owners.get(key), pending = readPending ? stores.outboxes.get(key) : undefined;
    let result: T, ready = false, failure: unknown;
    // Both reads and any guarded writes remain in one native IDB transaction.
    (pending ?? owner).onsuccess = () => {
      try { result = run(stores, owner.result, pending?.result); ready = true; }
      catch (error) { failure = error; transaction.abort(); }
    };
    transaction.oncomplete = () => ready ? resolve(result) : reject(Error("Shared work storage completed without a result"));
    transaction.onerror = transaction.onabort = () => reject(failure ?? transaction.error ?? Error("Shared work storage transaction failed"));
  });
}

function pendingValue(value: unknown, clientId: string): PendingCheckpoint | undefined {
  if (value === undefined) return undefined;
  const p = value as PendingCheckpoint;
  const malformed = () => Error("Saved shared work is unreadable or belongs to another editor; it has been preserved");
  if (!p || typeof p !== "object" || p.format !== "geosolve-client-pending-v1" || p.clientId !== clientId
    || ![p.documentId, p.documentEpoch, p.userId, p.clientId].every(item => typeof item === "string" && item.length > 0)
    || !Array.isArray(p.requests) || p.requests.length > 128) throw malformed();
  const ids = new Set<string>();
  for (const item of p.requests) {
    if (!item || !["commands", "text"].includes(item.route) || typeof item.requestId !== "string" || !item.requestId
      || !item.body || typeof item.body !== "object" || Array.isArray(item.body)
      || item.body.requestId !== item.requestId || ids.has(item.requestId)) throw malformed();
    ids.add(item.requestId);
  }
  if (new TextEncoder().encode(JSON.stringify(p)).length > 8 * 1024 * 1024) throw malformed();
  return p;
}
function ownerValue(value: unknown, scope: string, clientId: string): Owner | undefined {
  if (value === undefined) return undefined;
  const owner = value as Owner;
  if (!owner || owner.format !== "geosolve-client-owner-v1" || owner.scope !== scope || owner.clientId !== clientId
    || !(owner.ownerId === null || typeof owner.ownerId === "string" && owner.ownerId.length > 0)
    || !Number.isSafeInteger(owner.fence) || owner.fence < 1
    || !Number.isSafeInteger(owner.expiresAt) || owner.expiresAt < 0) {
    throw Error("Saved editor ownership is unreadable; its pending work has been preserved");
  }
  return owner;
}
function sameDocument(a: PendingCheckpoint, b: PendingCheckpoint): boolean {
  return a.documentId === b.documentId && a.documentEpoch === b.documentEpoch && a.userId === b.userId;
}

/** Storage capability for exactly one page lifetime. All access, including ACK
 * removal, authenticates its current fence in the same transaction as the data.
 * A local lease grants no server authority and cannot cancel in-flight packets. */
export class CollaborationPendingStore {
  private tail: Promise<unknown> = Promise.resolve();
  private releasing?: Promise<void>;
  private state: "owned" | "releasing" | "released" | "lost" = "owned";
  private readonly key: string;
  private constructor(private readonly claim: PendingClaimOptions, private readonly fence: number) {
    this.key = `${claim.scope}.${claim.clientId}`;
  }

  /** An unexpired owner always wins. Expired takeover is only called by the
   * host's explicit reload/recovery path, never merely to open another tab. */
  static async claim(options: PendingClaimOptions): Promise<
    { readonly store: CollaborationPendingStore } | { readonly busyUntil: number }
  > {
    if (![options.scope, options.clientId, options.ownerId].every(value => typeof value === "string" && value.length > 0)
      || !Number.isSafeInteger(options.leaseMs) || options.leaseMs < 1) throw Error("Invalid editor ownership claim");
    const key = `${options.scope}.${options.clientId}`;
    return records(key, "readwrite", (stores, rawOwner, rawPending) => {
      const owner = ownerValue(rawOwner, options.scope, options.clientId);
      pendingValue(rawPending, options.clientId); // Never mask malformed retained work.
      const now = Date.now();
      if (owner?.ownerId && owner.expiresAt > now) return { busyUntil: owner.expiresAt };
      const fence = (owner?.fence ?? 0) + 1;
      if (!Number.isSafeInteger(fence)) throw Error("Editor ownership generations are exhausted; its pending work has been preserved");
      stores.owners.put({ format: "geosolve-client-owner-v1", scope: options.scope, clientId: options.clientId,
        ownerId: options.ownerId, fence, expiresAt: now + options.leaseMs } satisfies Owner, key);
      return { store: new CollaborationPendingStore(options, fence) };
    });
  }

  /** Read-only recovery/export. This does not claim, retry, delete or rewrite work. */
  static inspect(scope: string, clientId: string): Promise<{
    readonly retained: RetainedCollaborationWork; readonly pending: PendingCheckpoint | undefined;
  }> {
    return records(`${scope}.${clientId}`, "readonly", (_stores, rawOwner, rawPending) => {
      const owner = ownerValue(rawOwner, scope, clientId), pending = pendingValue(rawPending, clientId);
      return { retained: { clientId, pendingRequests: pending?.requests.length ?? 0,
        ownedUntil: owner?.ownerId ? owner.expiresAt : null }, pending };
    });
  }

  /** Persistent recovery discovery survives later reloads and replaced tab hints. */
  static async retained(scope: string): Promise<readonly RetainedCollaborationWork[]> {
    const database = await open();
    return new Promise((resolve, reject) => {
      const transaction = database.transaction(["owners", "outboxes"], "readonly");
      const owners = transaction.objectStore("owners"), prefix = `${scope}.`;
      const request = transaction.objectStore("outboxes").openCursor(IDBKeyRange.bound(prefix, `${prefix}\uffff`));
      const result: RetainedCollaborationWork[] = [];
      let failure: unknown;
      request.onsuccess = () => {
        const cursor = request.result;
        if (!cursor) return;
        try {
          const clientId = (cursor.value as PendingCheckpoint)?.clientId;
          // A nested API URL can have the same prefix but a different full key.
          if (typeof clientId !== "string") throw Error("Saved shared work is unreadable; it has been preserved");
          if (cursor.key === `${prefix}${clientId}`) {
            const pending = pendingValue(cursor.value, clientId)!;
            if (pending.requests.length) {
              const ownerRequest = owners.get(cursor.key);
              ownerRequest.onsuccess = () => {
                try {
                  const owner = ownerValue(ownerRequest.result, scope, clientId);
                  result.push({ clientId, pendingRequests: pending.requests.length, ownedUntil: owner?.ownerId ? owner.expiresAt : null });
                } catch (error) { failure = error; transaction.abort(); }
              };
            }
          }
          cursor.continue();
        } catch (error) { failure = error; transaction.abort(); }
      };
      transaction.oncomplete = () => resolve(result);
      transaction.onerror = transaction.onabort = () => reject(failure ?? transaction.error ?? Error("Saved work recovery inventory failed"));
    });
  }

  read(): Promise<PendingCheckpoint | undefined> {
    return this.guarded((_stores, pending) => pending);
  }
  write(checkpoint: PendingCheckpoint): Promise<void> {
    const saved = structuredClone(checkpoint);
    return this.guarded((stores, existing) => {
      const next = pendingValue(saved, this.claim.clientId)!;
      if (existing?.requests.length && !sameDocument(existing, next)) {
        throw Error("Pending work belongs to another document or user; export or recover it before switching identities");
      }
      stores.outboxes.put(next, this.key);
    });
  }
  /** Revalidate on reconnect/pageshow as well as before any request retry. */
  assertOwned(): Promise<void> { return this.guarded(() => undefined, false); }

  release(): Promise<void> {
    if (this.releasing) return this.releasing;
    if (this.state !== "owned") return Promise.resolve();
    this.state = "releasing";
    this.releasing = this.ordered(() => records(this.key, "readwrite", (stores, rawOwner) => {
      const owner = ownerValue(rawOwner, this.claim.scope, this.claim.clientId);
      if (owner?.ownerId === this.claim.ownerId && owner.fence === this.fence) {
        stores.owners.put({ ...owner, ownerId: null, expiresAt: 0 }, this.key);
      }
    }, false)).finally(() => { this.state = "released"; });
    return this.releasing;
  }
  private guarded<T>(run: (stores: Stores, pending: PendingCheckpoint | undefined) => T, readPending = true): Promise<T> {
    if (this.state !== "owned") return Promise.reject(new CollaborationOwnershipLostError());
    return this.ordered(async () => {
      if (this.state === "lost" || this.state === "released") throw new CollaborationOwnershipLostError();
      try {
        return await records(this.key, "readwrite", (stores, rawOwner, rawPending) => {
          const owner = ownerValue(rawOwner, this.claim.scope, this.claim.clientId);
          if (!owner || owner.ownerId !== this.claim.ownerId || owner.fence !== this.fence) throw new CollaborationOwnershipLostError();
          const pending = pendingValue(rawPending, this.claim.clientId);
          const result = run(stores, pending);
          // An expired but still-current owner can renew atomically; a successful
          // takeover always changes the fence first and makes this path reject.
          stores.owners.put({ ...owner, expiresAt: Date.now() + this.claim.leaseMs }, this.key);
          return result;
        }, readPending);
      } catch (error) {
        // Storage loss is also fail-closed: no in-memory editing without an outbox.
        this.state = "lost";
        this.claim.onLost?.(error instanceof Error ? error : Error(String(error)));
        throw error;
      }
    });
  }
  private ordered<T>(run: () => Promise<T>): Promise<T> {
    const next = this.tail.then(run, run); this.tail = next.catch(() => {}); return next;
  }
}
