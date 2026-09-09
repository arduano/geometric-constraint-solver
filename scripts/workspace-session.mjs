// SPDX-License-Identifier: GPL-3.0-or-later
import { randomUUID } from "node:crypto";

/** Authority for one bridge lifetime and one explicitly selected UI editor. */
export function createWorkspaceSession() {
  const epoch = randomUUID();
  let editor = null;
  let lease = 0;
  let revision = 0;
  let gesture = null;
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
  function verify(id, authority, { navigation = false, pointerId } = {}) {
    client(id);
    if (id !== editor) reject("this tab is read only. Take over editing to change this project.");
    if (!authority || authority.epoch !== epoch || authority.lease !== lease) reject("the editor session changed. Refresh before retrying this intent.");
    const continuation = gesture?.clientId === id && gesture.pointerId === pointerId;
    if (!navigation && !continuation && authority.revision !== revision) reject("the installed interaction state changed. Refresh before retrying this intent.");
  }
  return {
    state, join,
    takeover(id, expected) {
      client(id);
      if (!expected || expected.epoch !== epoch || expected.lease !== lease) reject("editing ownership changed before handoff. Refresh and try again.");
      editor = id; lease++; revision++; gesture = null;
      return state(id);
    },
    verify,
    advance() { revision++; },
    beginGesture(id, authority, pointerId) {
      verify(id, authority);
      if (!Number.isSafeInteger(pointerId) || pointerId < 0) throw Error("Invalid pointer identity");
      if (gesture && gesture.pointerId !== pointerId) reject("finish the active gesture first.");
      gesture = { clientId: id, pointerId };
    },
    endGesture() { gesture = null; },
  };
}

// A conservative explicit inventory. Unknown commands go through mutation guards.
const navigationCommands = new Set([
  "view.fit", "view.origin", "view.grid.toggle", "view.construction.toggle",
  "explorer.visibility.set", "explorer.visibility.isolate", "explorer.visibility.restore",
  "dimensions.mode", "dimensions.pin", "dimensions.clearPins", "dimensions.hover", "dimensions.hover.clear",
  "dimensions.navigation.begin", "dimensions.navigation.end", "source.select", "declaration.source.open",
]);
export function isWorkspaceNavigation(method, input, snapshot) {
  if (["resize", "wheel", "wheelBatch", "cancel"].includes(method)) return true;
  if (method === "dispatch") return navigationCommands.has(input?.command);
  return method === "pointer" && input?.phase === "move" && !(input.buttons & 1) && snapshot?.presentation.activeTool === "select";
}

/** Selection/tool changes invalidate commands whose target comes from the displayed UI. */
export function workspaceInteractionKey(snapshot) {
  return JSON.stringify([snapshot.revision, snapshot.navigation?.selectionKey, snapshot.selection?.id,
    snapshot.presentation.activeTool, snapshot.presentation.geometryRole]);
}

/** Target-bearing UI actions need revision guards but cannot change authored files. */
export function isWorkspacePresentation(method, input) {
  return method === "dispatch" && [
    "selection.select", "selection.clear", "declaration.select", "navigation.rows.select",
    "dimensions.focus", "tool.select", "geometry.role.set",
  ].includes(input?.command);
}
