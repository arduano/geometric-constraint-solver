// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openCollaborationRuntime } from "./collaboration-runtime.mjs";
import { runCli } from "./geosolve-cli.mjs";

test("shared CLI status, latest value, raw working edits and Apply use invited durable HTTP authority", async (t) => {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-shared-cli-"));
  t.after(() => rm(folder, { recursive: true, force: true }));
  const source = '"use geosolve sketch";\nimport {sketch,mm} from "@geosolve/sketch-code";\nexport default sketch(($)=>{const bore=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(2)});return {bore};});\n';
  await writeFile(join(folder, "geosolve.json"), JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }));
  await writeFile(join(folder, "sketch.ts"), source);
  const invitation = { token: "trusted-cli-token", userId: "alice", role: "editor" };
  const runtime = await openCollaborationRuntime(folder, { initialize: true, invitations: new Map([[invitation.token, { userId: invitation.userId, role: invitation.role }]]) });
  t.after(() => runtime.close());
  const address = await runtime.listen(), origin = `http://127.0.0.1:${address.port}`;
  const invitationsFile = join(folder, ".geosolve/invitations.json");
  await writeFile(invitationsFile, JSON.stringify([invitation]));
  await writeFile(join(folder, ".geosolve/collaboration/session.json"), JSON.stringify({ format: "geosolve-collaboration-session-v1", folder, origin, invitationsFile, ...runtime.host.configuration }));
  const base = ["--collaboration", "true", "--user", "alice", "--client", "agent-tab"], expected = join(folder, ".geosolve/expected.json");
  const status = await runCli(["status", folder, ...base, "--out", expected]); assert.equal(status.ok, true);
  const command = ["set", folder, ...base, "--expected", expected, "--operation", "resize", "--declaration", "bore", "--path", '["radius"]', "--value", '{"kind":"unit","value":{"unit":"mm","value":5}}'];
  const changed = await runCli(command); assert.equal(changed.ok, true, JSON.stringify(changed));
  assert.deepEqual((await runCli(command)).receipt, changed.receipt);
  assert.deepEqual((await runCli(["outcome", folder, ...base, "--operation", "resize"])).receipt, changed.receipt);
  const latest = await runCli(["status", folder, ...base, "--out", expected]), input = latest.state.authority.acceptedInput;
  const edits = join(folder, ".geosolve/edits.json");
  await writeFile(edits, JSON.stringify([{ kind: "splice", path: "sketch.ts", start_utf16: 0, delete_utf16: 0, insert: "// CLI draft 😀\n" }, { kind: "create_file", path: "notes.ts", text: "// notes\n" }]));
  const draft = await runCli(["draft", folder, ...base, "--expected", expected, "--operation", "draft-edit", "--edits", edits]); assert.equal(draft.applied, false);
  const raw = await runCli(["status", folder, ...base, "--out", expected]);
  assert.equal(raw.state.authority.acceptedInput, input); assert.match(raw.state.document.working.files["sketch.ts"], /CLI draft 😀/u);
  const applied = await runCli(["apply", folder, ...base, "--expected", expected, "--operation", "apply-draft"]); assert.equal(applied.ok, true, JSON.stringify(applied));
  assert.equal(runtime.transport.stats().sessions, 0, "finished CLI clients explicitly release their server sessions");
  await writeFile(expected, JSON.stringify({ ...raw, documentEpoch: "foreign-life" }));
  await assert.rejects(runCli(["undo", folder, ...base, "--expected", expected, "--operation", "foreign"]), /another shared document/u);
});
