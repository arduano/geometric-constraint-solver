// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { createWorkspaceSession } from "../packages/geosolve-cli/runtime/workspace-session.mjs";
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

test("every authored command requires the exact installed model revision", () => {
  const session = createWorkspaceSession();
  const initial = session.join(first).authority;
  session.advance();
  assert.throws(() => session.verify(first, initial), /model revision changed/);
  assert.throws(() => session.verify(first, initial, { navigation: true, pointerId: 7 }), /model revision changed/,
    "old pointer/navigation hints cannot relax semantic admission");
  session.verify(first, session.state(first).authority);
});

test("a locally prepared terminal cannot survive an editing handoff", () => {
  const session = createWorkspaceSession();
  const captured = session.join(first).authority;
  const handed = session.takeover(second, captured);
  assert.throws(() => session.verify(first, captured), /read only/);
  session.verify(second, handed.authority);
});
