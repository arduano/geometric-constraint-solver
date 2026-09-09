// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { createWorkspaceSession, isWorkspaceNavigation } from "./workspace-session.mjs";
const first = "first-editor";
const second = "second-editor";

test("one active editor, explicit handoff, old lease and restart rejection", () => {
  const session = createWorkspaceSession();
  const initial = session.join(first);
  assert.equal(initial.editor.canEdit, true);
  assert.equal(session.join(second).editor.canEdit, false);
  assert.throws(() => session.verify(second, initial.authority), /read only/);
  const handed = session.takeover(second, initial.authority);
  assert.equal(handed.editor.canEdit, true);
  assert.throws(() => session.verify(first, initial.authority), /read only/);
  assert.throws(() => session.takeover(first, initial.authority), /ownership changed/);
  session.verify(second, handed.authority);
  const restarted = createWorkspaceSession();
  restarted.join(second);
  assert.throws(() => restarted.verify(second, handed.authority), /session changed/);
});

test("selection changes cannot authorize stale target-relative commands; navigation remains cheap", () => {
  const session = createWorkspaceSession();
  const initial = session.join(first).authority;
  session.advance();
  assert.throws(() => session.verify(first, initial), /interaction state changed/);
  session.verify(first, initial, { navigation: true });
  session.verify(first, session.state(first).authority);
  for (const command of ["history.undo", "history.redo", "authoring.metadata.set", "authoring.parameter.extract", "unknown"]) {
    assert.equal(isWorkspaceNavigation("dispatch", { command }), false);
  }
  assert.equal(isWorkspaceNavigation("wheelBatch", []), true);
});

test("ordered terminal gesture retains its starting authority but cannot survive a handoff", () => {
  const session = createWorkspaceSession();
  const initial = session.join(first).authority;
  session.beginGesture(first, initial, 7);
  session.advance();
  session.verify(first, initial, { pointerId: 7 });
  assert.throws(() => session.verify(first, initial, { pointerId: 8 }), /interaction state changed/);
  session.endGesture();
  assert.throws(() => session.verify(first, initial, { pointerId: 7 }), /interaction state changed/);
  const current = session.state(first).authority;
  session.beginGesture(first, current, 9);
  const handed = session.takeover(second, current);
  assert.throws(() => session.verify(first, current, { pointerId: 9 }), /read only/);
  session.verify(second, handed.authority);
});
