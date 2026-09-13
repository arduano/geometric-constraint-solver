// SPDX-License-Identifier: GPL-3.0-or-later
import type { TextWorkerRequest, TextWorkerResponse, TextWorkerUpdate } from "./collaboration-text-worker";
import type { TextEdit, TextRevision } from "../../../../../packages/geosolve-collaboration/src/index";
export type { TextWorkerUpdate, TextEdit, TextRevision };
import { WorkerRequestChannel, type WorkerTransport, type WithoutWorkerId } from "./worker-channel";

/** Every CRDT request is sent in order. Text merging stays in its native owner. */
export class CollaborationTextWorker {
  private readonly channel: WorkerRequestChannel<WithoutWorkerId<TextWorkerRequest>, TextWorkerResponse, TextWorkerUpdate>;
  constructor(worker: WorkerTransport = new Worker(new URL("./collaboration-text-worker.ts", import.meta.url), { type: "module" })) {
    this.channel = new WorkerRequestChannel(worker, {
      stoppedMessage: "Shared text worker stopped", unreadableMessage: "Shared text response could not be decoded",
      responseError: (response) => "error" in response ? response.error : undefined,
      decode: (response) => { if ("error" in response) throw Error(response.error); return response.result; },
    });
  }
  open(actor:readonly number[],checkpoint:readonly number[],saved?:readonly number[]){return this.channel.request({method:"open",actor,checkpoint,saved});}
  edit(revision:TextRevision,edits:readonly TextEdit[]){return this.channel.request({method:"edit",revision,edits});}
  receive(changes:readonly (readonly number[])[]){return this.channel.request({method:"receive",changes});}
  undo(){return this.channel.request({method:"undo"});}
  redo(){return this.channel.request({method:"redo"});}
  snapshot(){return this.channel.request({method:"snapshot"});}
  dispose(){this.channel.fail(Error("Shared text worker disposed"));}
}
