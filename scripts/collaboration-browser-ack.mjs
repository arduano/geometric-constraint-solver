// SPDX-License-Identifier: GPL-3.0-or-later

/** Observe successful native storage transactions without changing their writes.
 * A text request is removed only after the client drains and parses its ACK.
 * Chromium can report ERR_ABORTED after a streaming fetch has fully succeeded;
 * Playwright Response.finished() then stays pending even though the app finished.
 */
export async function trackCommittedTextOutbox(context) {
  await context.addInitScript(() => {
    const observations = [];
    Object.defineProperty(window, "geosolveCommittedOutboxes", { value: observations });
    const transactions = new WeakMap(), original = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function (value, key) {
      const request = Reflect.apply(original, this, arguments);
      if (this.transaction.db.name !== "geosolve.collaboration.v1" || this.name !== "outboxes"
        || value?.format !== "geosolve-client-pending-v1") return request;
      const transaction = this.transaction;
      let writes = transactions.get(transaction);
      if (!writes) {
        writes = new Map(); transactions.set(transaction, writes);
        transaction.addEventListener("complete", () => {
          for (const snapshot of writes.values()) observations.push({ ...snapshot, at: performance.now() });
          if (observations.length > 256) observations.splice(0, observations.length - 256);
        }, { once: true });
      }
      // Only the final write to this key survives the transaction. Retain IDs,
      // never the shared source or native text bytes, in this bounded test trace.
      writes.set(key, {
        key, clientId: value.clientId, documentId: value.documentId,
        documentEpoch: value.documentEpoch, userId: value.userId,
        requests: value.requests.map(item => ({ requestId: item.requestId, route: item.route })),
      });
      return request;
    };
  });
}

export async function waitForCommittedTextAck(page, response, timeout = 3000) {
  if (response.status() !== 200 || response.request().method() !== "POST"
    || !new URL(response.url()).pathname.endsWith("/text")) throw Error("Expected successful text response headers");
  const requestId = response.request().postDataJSON()?.requestId;
  if (typeof requestId !== "string" || !requestId) throw Error("Text response has no request identity");
  const scope = `geosolve.collaboration.${new URL("./", response.url()).href}`;
  // Return the completed witness in one browser call. Fetching and disposing a
  // remote JSHandle would add two unrelated round trips to the latency sample.
  let deadline;
  try {
    return await Promise.race([
      page.evaluate(({ requestId, scope, timeout }) => new Promise((resolve, reject) => {
        const started = performance.now();
        const poll = () => {
          const clientId = sessionStorage.getItem(`${scope}.client`);
          const snapshots = window.geosolveCommittedOutboxes ?? [];
          const hasRequest = snapshot => snapshot.requests.some(item => item.route === "text" && item.requestId === requestId);
          for (let index = 0; clientId && index < snapshots.length; index++) {
            const pending = snapshots[index];
            if (pending.clientId !== clientId || pending.key !== `${scope}.${clientId}` || !hasRequest(pending)) continue;
            const accepted = snapshots.slice(index + 1).find(snapshot =>
              ["key", "clientId", "documentId", "documentEpoch", "userId"].every(key => snapshot[key] === pending[key])
              && snapshot.requests.every(item => item.requestId !== requestId));
            if (accepted) {
              resolve({ requestId, clientId: accepted.clientId, documentId: accepted.documentId,
                documentEpoch: accepted.documentEpoch, queuedAt: pending.at, acknowledgedAt: accepted.at });
              return;
            }
          }
          if (performance.now() - started >= timeout) { reject(Error("Timeout waiting for committed text ACK")); return; }
          setTimeout(poll, 10);
        };
        poll();
      }), { requestId, scope, timeout }),
      // A blocked page must not prevent the test's own deadline from firing.
      new Promise((_, reject) => { deadline = setTimeout(() => reject(Error("Timeout waiting for committed text ACK")), timeout); }),
    ]);
  } finally { clearTimeout(deadline); }
}
