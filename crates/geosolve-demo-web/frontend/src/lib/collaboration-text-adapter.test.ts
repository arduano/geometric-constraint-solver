// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it, vi } from "vitest";
import type { TextEdit } from "./collaboration-text-adapter";
import { CollaborationTextWorker } from "./collaboration-text-adapter";
import type { TextWorkerResponse, TextWorkerUpdate } from "./collaboration-text-worker";

class Transport extends EventTarget {
  postMessage = vi.fn(); terminate = vi.fn();
  reply(response: TextWorkerResponse) { this.dispatchEvent(new MessageEvent("message", { data: response })); }
}
const update: TextWorkerUpdate = { snapshot: { revision: { heads: ["accepted"] }, files: { "sketch.ts": "text" } }, changes: [], checkpoint: [], history: { undo: 0, redo: 0 } };

it("never coalesces admitted CRDT edits, changes or history requests while a reply is held", async () => {
  const transport = new Transport(), text = new CollaborationTextWorker(transport);
  const revision = { heads: ["original"] };
  const edits: TextEdit[] = [{ kind: "splice", path: "sketch.ts", start_utf16: 0, delete_utf16: 0, insert: "😀" }];
  const pending = [text.open([1], [2], [3]), text.edit(revision, edits), text.receive([[4]]), text.undo(), text.redo(), text.snapshot()];
  expect(transport.postMessage.mock.calls.map(([message]) => message)).toEqual([
    { id: 1, method: "open", actor: [1], checkpoint: [2], saved: [3] },
    { id: 2, method: "edit", revision, edits }, { id: 3, method: "receive", changes: [[4]] },
    { id: 4, method: "undo" }, { id: 5, method: "redo" }, { id: 6, method: "snapshot" },
  ]);
  for (let id = 1; id <= 6; ++id) transport.reply({ id, result: update });
  expect(await Promise.all(pending)).toEqual(Array(6).fill(update));
  text.dispose();
});

it("rejects every outstanding text request on decoding failure and ignores late replies after disposal", async () => {
  const transport = new Transport(), text = new CollaborationTextWorker(transport);
  const pending = [text.open([1], []), text.snapshot()].map(promise => expect(promise).rejects.toThrow("could not be decoded"));
  transport.dispatchEvent(new MessageEvent("messageerror"));
  await Promise.all(pending);
  transport.reply({ id: 1, result: update }); text.dispose();
  await expect(text.snapshot()).rejects.toThrow("could not be decoded");
  expect(transport.terminate).toHaveBeenCalledOnce();
  expect(transport.postMessage).toHaveBeenCalledTimes(2);
});
