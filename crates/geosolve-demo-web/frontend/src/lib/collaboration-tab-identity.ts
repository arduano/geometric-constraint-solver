// SPDX-License-Identifier: GPL-3.0-or-later
import { CollaborationPendingStore, type RetainedCollaborationWork } from "./collaboration-storage";

export interface CollaborationTabIdentityOptions {
  /** Existing geosolve.collaboration.${baseUrl} namespace, without the client ID. */
  readonly scope: string;
  /** Fresh navigations fork. Reloads resume the exact previous outbox by default.
   * Hosts may select resume explicitly for their recovery action. */
  readonly mode?: "new" | "resume";
  readonly sessionStorage?: Pick<Storage, "getItem" | "setItem">;
  readonly leaseMs?: number;
  readonly heartbeatMs?: number;
  readonly resumeTimeoutMs?: number;
  readonly onLost?: (error: Error) => void;
}
export interface CollaborationTabIdentity {
  readonly clientId: string;
  readonly storage: CollaborationPendingStore;
  readonly resumed: boolean;
  /** Work from the copied/replaced hint remains available to its own editor or
   * explicit recovery/export. A fresh tab never silently adopts or deletes it. */
  readonly retained: readonly RetainedCollaborationWork[];
  readonly lost: Promise<Error>;
  assertOwned(): Promise<void>;
  release(): Promise<void>;
}
export class CollaborationRecoveryWaitingError extends Error {
  constructor(readonly clientId: string) {
    super("The previous editor still owns its pending work. Close that editor and retry recovery, or open a separate editor. Its saved requests have been preserved.");
  }
}
function randomId(): string {
  // Available on ordinary Tailscale HTTP; randomUUID and WebLocks are not.
  return Array.from(globalThis.crypto.getRandomValues(new Uint8Array(16)), byte => byte.toString(16).padStart(2, "0")).join("");
}
function defaultMode(): "new" | "resume" {
  const navigation = globalThis.performance?.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
  return navigation?.type === "reload" ? "resume" : "new";
}

/** Acquire before creating the network client. Copied sessionStorage is only a
 * hint; transactional IDB claims own the outbox. An abnormal reload waits for the
 * old lease and retries the same IDs after fencing the prior page. Ordinary new
 * tabs always fork, even when the copied owner's lease has expired. */
export async function acquireCollaborationTabIdentity(options: CollaborationTabIdentityOptions): Promise<CollaborationTabIdentity> {
  const leaseMs = options.leaseMs ?? 15_000, heartbeatMs = options.heartbeatMs ?? Math.min(3_000, Math.max(1, Math.floor(leaseMs / 3)));
  const resumeTimeoutMs = options.resumeTimeoutMs ?? leaseMs + 5_000;
  if (!options.scope || ![leaseMs, heartbeatMs, resumeTimeoutMs].every(value => Number.isSafeInteger(value) && value > 0)
    || heartbeatMs >= leaseMs) throw Error("Invalid editor ownership timing");
  const session = options.sessionStorage ?? globalThis.sessionStorage;
  const key = `${options.scope}.client`, previous = session.getItem(key);
  const resumed = (options.mode ?? defaultMode()) === "resume" && !!previous;
  const clientId = resumed ? previous! : randomId(), ownerId = randomId();
  const retained = (await CollaborationPendingStore.retained(options.scope)).filter(item => item.clientId !== clientId);
  let lostResolve!: (error: Error) => void;
  const lost = new Promise<Error>(resolve => { lostResolve = resolve; });
  let heartbeat: ReturnType<typeof setInterval> | undefined;
  let stopped = false;
  const onLost = (error: Error) => {
    if (stopped) return;
    stopped = true; clearInterval(heartbeat); lostResolve(error); options.onLost?.(error);
  };
  const deadline = Date.now() + resumeTimeoutMs;
  let storage: CollaborationPendingStore;
  for (;;) {
    const claimed = await CollaborationPendingStore.claim({ scope: options.scope, clientId, ownerId, leaseMs, onLost });
    if ("store" in claimed) { storage = claimed.store; break; }
    if (!resumed || Date.now() >= deadline) throw new CollaborationRecoveryWaitingError(clientId);
    await new Promise(resolve => setTimeout(resolve, Math.max(1, Math.min(250, claimed.busyUntil - Date.now() + 1, deadline - Date.now()))));
  }
  try { session.setItem(key, clientId); }
  catch (error) { await storage.release(); throw error; }
  // Timer/check failures revoke this page locally; a suspended page must validate
  // on pageshow before editing or replay. Neither release nor loss deletes data.
  heartbeat = setInterval(() => { void storage.assertOwned().catch(onLost); }, heartbeatMs);
  return {
    clientId, storage, resumed, retained, lost,
    assertOwned: () => storage.assertOwned(),
    async release() { stopped = true; clearInterval(heartbeat); await storage.release(); },
  };
}
