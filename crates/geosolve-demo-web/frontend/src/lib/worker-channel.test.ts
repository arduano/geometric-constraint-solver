// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it, vi } from "vitest";
import { WorkerMailbox, WorkerRequestChannel, type WorkerChannelOptions } from "./worker-channel";

type Action = { method: "replace" | "paint" | "edit"; value: number };
type Response = { id: number; generation?: number; result: number } | { id: number; generation?: number; error: string };
class Transport extends EventTarget {
  postMessage = vi.fn();
  terminate = vi.fn();
  removeEventListener = vi.fn(super.removeEventListener.bind(this));
  reply(response: Response) { this.dispatchEvent(new MessageEvent("message", { data: response })); }
}
const options: WorkerChannelOptions<Action, Response, number> = {
  stoppedMessage: "stopped", unreadableMessage: "unreadable",
  responseError: (response) => "error" in response ? response.error : undefined,
  decode: (response) => {
    if (!("result" in response) || !Number.isFinite(response.result)) throw Error("malformed result");
    return response.result;
  },
};
function mailbox(transport: Transport, limit = 4) {
  return new WorkerMailbox<Action, Response, number>(transport, {
    ...options, unopenedMessage: "unopened", replacedMessage: "replaced", fullMessage: "full", limit,
    combine: (previous, next) => previous.method === "paint" && next.method === "paint" ? next : undefined,
  });
}

it("isolates an uncloneable request and a domain refusal without losing other admitted requests", async () => {
  const transport = new Transport();
  const channel = new WorkerRequestChannel(transport, options);
  const first = channel.request({ method: "edit", value: 1 });
  transport.postMessage.mockImplementationOnce(() => { throw new DOMException("uncloneable", "DataCloneError"); });
  await expect(channel.request({ method: "edit", value: 2 })).rejects.toThrow("uncloneable");
  const refused = expect(channel.request({ method: "edit", value: 3 })).rejects.toThrow("refused");
  const last = channel.request({ method: "paint", value: 4 });
  transport.reply({ id: 3, error: "refused" });
  transport.reply({ id: 2, result: 999 });
  transport.reply({ id: 1, result: 1 }); transport.reply({ id: 4, result: 4 });
  await refused;
  expect(await first).toBe(1); expect(await last).toBe(4);
  expect(transport.terminate).not.toHaveBeenCalled();
  channel.fail(Error("disposed"));
});

it("retires malformed successful responses, rejects all pending work and removes every listener exactly once", async () => {
  const transport = new Transport(), onFailure = vi.fn();
  const channel = new WorkerRequestChannel(transport, { ...options, onFailure });
  const first = expect(channel.request({ method: "edit", value: 1 })).rejects.toThrow("malformed result");
  const second = expect(channel.request({ method: "paint", value: 2 })).rejects.toThrow("malformed result");
  transport.reply({ id: 1, result: NaN });
  await Promise.all([first, second]);
  transport.reply({ id: 2, result: 2 });
  channel.fail(Error("second disposal"));
  await expect(channel.request({ method: "edit", value: 3 })).rejects.toThrow("malformed result");
  expect(transport.terminate).toHaveBeenCalledOnce();
  expect(transport.removeEventListener.mock.calls.map(([kind]) => kind)).toEqual(["message", "error", "messageerror"]);
  expect(onFailure).toHaveBeenCalledOnce();
  expect(transport.postMessage).toHaveBeenCalledTimes(2);
});

it("counts coalesced callers against the bound and preserves edit barriers after a refusal", async () => {
  const transport = new Transport(), channel = mailbox(transport);
  const opened = channel.replace({ method: "replace", value: 0 }); await Promise.resolve();
  transport.reply({ id: 1, generation: 1, result: 0 }); await opened;
  const first = channel.request({ method: "paint", value: 1 });
  const second = channel.request({ method: "paint", value: 2 });
  const edit = expect(channel.request({ method: "edit", value: 3 })).rejects.toThrow("refused");
  const last = channel.request({ method: "paint", value: 4 });
  await expect(channel.request({ method: "paint", value: 5 })).rejects.toThrow("full");
  expect(transport.postMessage.mock.lastCall?.[0]).toEqual({ id: 2, generation: 1, method: "paint", value: 2 });
  transport.reply({ id: 2, generation: 1, result: 2 });
  expect(await first).toBe(2); expect(await second).toBe(2);
  expect(transport.postMessage.mock.lastCall?.[0].method).toBe("edit");
  transport.reply({ id: 3, generation: 1, error: "refused" }); await edit;
  expect(transport.postMessage.mock.lastCall?.[0]).toEqual({ id: 4, generation: 1, method: "paint", value: 4 });
  transport.reply({ id: 4, generation: 1, result: 4 }); expect(await last).toBe(4);
  channel.dispose(Error("disposed"));
});

it("invalidates running and coalesced work on model replacement and ignores old IDs and wrong generations", async () => {
  const transport = new Transport(), channel = mailbox(transport);
  const old = expect(channel.replace({ method: "replace", value: 1 })).rejects.toThrow("replaced"); await Promise.resolve();
  const queued = [1, 2].map(value => expect(channel.request({ method: "paint", value })).rejects.toThrow("replaced"));
  const current = channel.replace({ method: "replace", value: 2 });
  const settled = vi.fn(); void current.then(settled);
  await Promise.resolve(); await Promise.all([old, ...queued]);
  expect(transport.postMessage.mock.lastCall?.[0]).toEqual({ id: 2, generation: 2, method: "replace", value: 2 });
  transport.reply({ id: 1, generation: 1, result: 100 });
  transport.reply({ id: 2, generation: 1, result: 100 }); await Promise.resolve();
  expect(settled).not.toHaveBeenCalled();
  transport.reply({ id: 2, generation: 2, result: 2 }); expect(await current).toBe(2);
  expect(transport.terminate).not.toHaveBeenCalled();
  channel.dispose(Error("disposed"));
});

it("fails the ordered mailbox on delivery failure and settles queued work before any further post", async () => {
  const transport = new Transport(), channel = mailbox(transport);
  transport.postMessage.mockImplementationOnce(() => { throw Error("delivery failed"); });
  const opening = expect(channel.replace({ method: "replace", value: 0 })).rejects.toThrow("delivery failed");
  const queued = expect(channel.request({ method: "edit", value: 1 })).rejects.toThrow("delivery failed");
  await Promise.all([opening, queued]);
  await expect(channel.request({ method: "paint", value: 2 })).rejects.toThrow("delivery failed");
  expect(transport.postMessage).toHaveBeenCalledOnce();
  expect(transport.terminate).toHaveBeenCalledOnce();
});
