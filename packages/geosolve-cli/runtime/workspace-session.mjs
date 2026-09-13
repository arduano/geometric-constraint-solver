// SPDX-License-Identifier: GPL-3.0-or-later
import { randomUUID } from "node:crypto";

/** Authority for one bridge lifetime and one explicitly selected UI editor. */
export function createWorkspaceSession() {
  const epoch = randomUUID();
  let editor = null;
  let lease = 0;
  let revision = 0;
  const identity = () => ({ epoch, lease, revision });
  const reject = (message) => { const error = Error(`Conflict: ${message}`); error.conflict = true; throw error; };
  const client = (id) => {
    if (typeof id !== "string" || !/^[a-zA-Z0-9_-]{8,128}$/.test(id)) throw Error("A valid workspace client ID is required");
    return id;
  };
  const state = (id) => ({ authority: identity(), editor: { clientId: editor, canEdit: editor === id } });
  function join(id) {
    client(id);
    if (editor === null) { editor = id; lease++; }
    return state(id);
  }
  function verify(id, authority) {
    client(id);
    if (id !== editor) reject("this tab is read only. Take over editing to change this project.");
    if (!authority || authority.epoch !== epoch || authority.lease !== lease) reject("the editor session changed. Refresh before retrying this intent.");
    if (authority.revision !== revision) reject("the accepted model revision changed. Refresh before retrying this intent.");
  }
  return {
    state, join,
    takeover(id, expected) {
      client(id);
      if (!expected || expected.epoch !== epoch || expected.lease !== lease) reject("editing ownership changed before handoff. Refresh and try again.");
      editor = id; lease++; revision++;
      return state(id);
    },
    verify,
    advance() { revision++; },
  };
}
