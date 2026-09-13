// SPDX-License-Identifier: GPL-3.0-or-later
import { readFile, writeFile, realpath } from "node:fs/promises";
import { resolve } from "node:path";
import { collaborationClientModuleUrl } from "./workspace-runtime-paths.mjs";
const { CollaborationClient } = await import(collaborationClientModuleUrl);
const readJson = async path => JSON.parse(await readFile(resolve(path), "utf8"));

/** Local CLI uses a host-issued invited principal and the ordinary HTTP gateway.
 * Operation IDs are caller-owned so a killed process can retry exact requests. */
export async function runCollaborationCli(command, folder, flags) {
  if (!flags.user || !flags.client) throw Error("Shared CLI commands require --user <invited user> and --client <stable client ID>");
  const session = await readJson(resolve(folder, ".geosolve/collaboration/session.json"));
  if (session.format !== "geosolve-collaboration-session-v1" || await realpath(session.folder) !== await realpath(folder)) throw Error("Shared server session belongs to another folder");
  const origin = new URL(session.origin);
  if (origin.protocol !== "http:" || origin.username || origin.password || origin.origin !== session.origin) throw Error("Invalid shared server origin");
  const invitations = await readJson(session.invitationsFile), invitation = invitations.filter(item => item.userId === flags.user);
  if (invitation.length !== 1) throw Error("Expected one trusted invitation for the selected user");
  const client = new CollaborationClient({ baseUrl: new URL("/api/collaboration/", origin).href, inviteToken: invitation[0].token, clientId: flags.client });
  try {
    const state = await client.connect();
    if (client.connection.documentId !== session.documentId || client.connection.documentEpoch !== session.documentEpoch) throw Error("Shared session identity changed; inspect the current server session");
    if (command === "status") {
      const result = { ok: true, collaboration: true, documentId: client.connection.documentId, documentEpoch: client.connection.documentEpoch, clientId: flags.client, userId: flags.user, state };
      if (flags.out) await writeFile(resolve(flags.out), JSON.stringify(result, null, 2) + "\n");
      return result;
    }
    if (command === "outcome") {
      if (!flags.operation) throw Error("Outcome requires --operation <original operation ID>");
      return { ok: true, receipt: await client.receipt(flags.operation) };
    }
    if (!flags.operation || !flags.expected) throw Error("Shared edits require --expected <status JSON> and --operation <unique intent ID>; reuse only for the same retry");
    const observed = await readJson(flags.expected), expected = observed.state ?? observed;
    if (observed.documentId !== client.connection.documentId || observed.documentEpoch !== client.connection.documentEpoch
      || observed.userId !== flags.user || observed.clientId !== flags.client) throw Error("Expected status belongs to another shared document or client");
    if (command === "draft") {
      const edits = await readJson(flags.edits);
      const ack = await client.editWorking(edits, expected.document.working.revision, flags.operation);
      return { ok: true, ack, applied: false };
    }
    if (command === "text-undo" || command === "text-redo") return { ok: true, ack: await client.undoText(command === "text-redo", flags.operation), applied: false };
    let semantic;
    if (command === "set") {
      if (!flags.declaration || !flags.path || !flags.value) throw Error("Shared set requires --declaration <source symbol>, --path <JSON array> and --value <native ManagedValue JSON>");
      const object = expected.document.inventory.objects.find(item => item.declaration === flags.declaration)?.object, target = expected.document.targets[object];
      if (!target) throw Error("Expected source declaration has no current semantic target");
      semantic = { kind: "semantic", basisRevision: expected.authority.acceptedRevision, payload: { action: "values", writes: [{ target, declaration: flags.declaration, path: JSON.parse(flags.path), value: JSON.parse(flags.value) }] } };
    } else if (["apply", "undo", "redo"].includes(command)) semantic = { kind: command, basisRevision: expected.authority.acceptedRevision, payload: {} };
    else if (command === "edit") semantic = await readJson(flags.command);
    else throw Error(`Unsupported shared CLI command: ${command}`);
    let receipt = await client.submit(semantic, flags.operation);
    const deadline = Date.now() + 300_000;
    while (!receipt.outcome && Date.now() < deadline) {
      await new Promise(resolve => setTimeout(resolve, 50)); receipt = await client.receipt(flags.operation) ?? receipt;
    }
    if (!receipt.outcome) throw Error("Shared operation is still pending; query outcome using the original operation ID");
    return { ok: receipt.outcome.status === "accepted", receipt };
  } finally { await client.leave().catch(() => {}); client.dispose(); }
}
