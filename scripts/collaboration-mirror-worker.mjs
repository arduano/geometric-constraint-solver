// SPDX-License-Identifier: GPL-3.0-or-later
import { parentPort, workerData } from "node:worker_threads";

const PROTOCOL = "geosolve-mirror-worker-v1";
let mirror, active, nextRequest = 0, nextCallback = 0, callback, closing = false;
const errorMessage = error => String(error?.message ?? error).slice(0, 8192);
function send(value) { parentPort.postMessage({ protocol: PROTOCOL, ...value }); }
function finishClose() {
  if (!closing || active) return;
  send({kind:"closed"}); parentPort.close();
}
function rpc(method, value = null) {
  if (closing) return Promise.reject(Error("Mirror worker is closing; pending work is preserved"));
  if (!active || callback) return Promise.reject(Error("Mirror callback mailbox is unavailable"));
  return new Promise((resolve, reject) => {
    callback = {id:++nextCallback,requestId:active.id,resolve,reject};
    send({kind:"callback",id:callback.id,requestId:callback.requestId,method,value});
  });
}
function protocolError(message) {
  closing = true;
  // Drain active filesystem code through its existing rejected callback/phase
  // boundaries rather than leave a write chain after port shutdown.
  callback?.reject(Error(message)); callback = undefined;
  send({kind:"fatal",error:message}); finishClose();
}
parentPort.on("message", message => {
  if (!message || message.protocol !== PROTOCOL) { protocolError("Foreign mirror bridge message"); return; }
  if (message.kind === "close") {
    if (closing) return;
    closing = true;
    // A parent callback already started must settle at the parent before close()
    // resolves. Refusing here preserves pending admission and lets fs work drain.
    callback?.reject(Error("Mirror closing; retry exact persisted admission")); callback = undefined;
    finishClose(); return;
  }
  if (message.kind === "callback_result") {
    if (closing) return; // Late result after close is never publication authority.
    if (!callback || message.id !== callback.id || message.requestId !== callback.requestId || typeof message.ok !== "boolean") { protocolError("Mismatched mirror callback result"); return; }
    const pending = callback; callback = undefined;
    if (message.ok) pending.resolve(message.value); else pending.reject(Error(errorMessage(message.error)));
    return;
  }
  if (message.kind !== "request" || closing || active || !Number.isSafeInteger(message.id) || message.id !== nextRequest + 1
    || !["initialize", "reconcile", "status"].includes(message.method) || (message.method === "initialize") !== !mirror) {
    protocolError("Unknown, duplicate or concurrent mirror request"); return;
  }
  nextRequest = message.id; active = {id:message.id};
  void (async () => {
    if (message.method === "initialize") {
      // Dynamic import ensures no native shared-text/Automerge work reaches root.
      const { createCollaborationMirror } = await import("./collaboration-mirror.mjs");
      mirror = await createCollaborationMirror({ ...workerData,
        readCommitted: () => rpc("readCommitted"),
        admitWorkingEdits: value => rpc("admitWorkingEdits",value),
        onPhase: (name, detail) => {
          if (closing) throw Error("Mirror closing; durable file plan is preserved");
          return workerData.phases ? rpc("onPhase",{name,detail}) : undefined;
        },
      });
      return null;
    }
    return mirror[message.method]();
  })().then(value => send({kind:"result",id:message.id,ok:true,value}), error => send({kind:"result",id:message.id,ok:false,error:errorMessage(error)}))
    .finally(() => { active = undefined; finishClose(); });
});
