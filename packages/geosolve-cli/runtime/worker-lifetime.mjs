// SPDX-License-Identifier: GPL-3.0-or-later
import { Worker } from "node:worker_threads";

/** One worker generation. Retiring it synchronously detaches all application
 * callbacks; stop() and exited still wait for actual worker termination. Queue,
 * retry, durability and callback-drain policies remain with the supervisor. */
export function createWorkerLifetime({ url, workerData, memoryMb = 512, onMessage, onError, onExit, onOutput }) {
  const worker = new Worker(url, {
    workerData, execArgv: [], stdout: true, stderr: true,
    resourceLimits: { maxOldGenerationSizeMb: memoryMb },
  });
  let retired = false, stopping, exitResolve;
  const exited = new Promise(resolve => { exitResolve = resolve; });
  worker.on("message", message => { if (!retired) onMessage(message); });
  worker.once("error", error => { if (!retired) onError(error); });
  worker.once("exit", code => {
    exitResolve(code);
    if (!retired) onExit(code);
    retired = true;
  });
  for (const pipe of [worker.stdout, worker.stderr]) {
    if (onOutput) pipe.on("data", chunk => { if (!retired) onOutput(chunk); });
    else pipe.resume();
  }
  return {
    exited,
    post(message) {
      if (retired) throw Error("Worker generation has been retired");
      worker.postMessage(message);
    },
    stop() {
      if (!stopping) {
        retired = true;
        stopping = worker.terminate().then(() => exited);
      }
      return stopping;
    },
  };
}

/** Exactly-once request settlement with independently armed cancellation and
 * deadline guards. Supervisors install request identity before arming guards,
 * and may clear them before waiting for a worker or durable callback to stop. */
export function createWorkerRequest() {
  let resolve, reject, timer, signal, abort, settled = false;
  const promise = new Promise((accept, refuse) => { resolve = accept; reject = refuse; });
  const clear = () => {
    clearTimeout(timer);
    signal?.removeEventListener("abort", abort);
    timer = signal = abort = undefined;
  };
  const settle = (complete, value) => {
    if (settled) return;
    settled = true;
    clear();
    complete(value);
  };
  return {
    promise, clear,
    resolve: value => settle(resolve, value),
    reject: error => settle(reject, error),
    watch({ timeoutMs, onTimeout, signal: watchedSignal, onAbort }) {
      clear();
      if (settled) return;
      if (timeoutMs !== undefined) timer = setTimeout(onTimeout, timeoutMs);
      signal = watchedSignal;
      abort = onAbort;
      signal?.addEventListener("abort", abort, { once: true });
      if (signal?.aborted) abort();
    },
  };
}
