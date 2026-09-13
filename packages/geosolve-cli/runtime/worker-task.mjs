// SPDX-License-Identifier: GPL-3.0-or-later
import { createWorkerLifetime, createWorkerRequest } from "./worker-lifetime.mjs";

/** Lifetime of one disposable worker. Queue policy and publication stay with its caller. */
export function runWorkerTask({ url, workerData, memoryMb, signal, timeoutMs, decode, failure }) {
  if (signal?.aborted) return Promise.reject(failure("cancelled"));
  const request = createWorkerRequest();
  let worker;
  const finish = (error, result) => {
    void worker.stop();
    if (error) request.reject(error); else request.resolve(result);
  };
  try {
    worker = createWorkerLifetime({ url, workerData, memoryMb,
      onMessage(message) {
        try { finish(null, decode(message)); } catch (error) { finish(error); }
      },
      onError: error => finish(failure("error", error)),
      onExit: code => finish(failure("exit", code)),
    });
    request.watch({ signal, onAbort: () => finish(failure("cancelled")),
      timeoutMs, onTimeout: () => finish(failure("timeout")) });
  } catch (error) { request.reject(error); }
  return request.promise;
}
