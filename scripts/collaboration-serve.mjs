// SPDX-License-Identifier: GPL-3.0-or-later
import { readFile, writeFile, unlink } from "node:fs/promises";
import { resolve, join } from "node:path";
import { openCollaborationRuntime } from "./collaboration-runtime.mjs";
import { releaseArtifactModuleUrl, packagedWorkbenchManifest, workbenchDist } from "./workspace-runtime-paths.mjs";
const { hash, mediaType, readManifest } = await import(releaseArtifactModuleUrl);

/** Capture a nominated artifact once; requests cannot read the authored folder
 * or serve changed build bytes. The integrated gate still owns qualification. */
export async function collaborationArtifactRoutes(manifest, directory) {
  const artifact = await readManifest(manifest, directory);
  if (artifact.manifest.publicBase !== "./" && artifact.manifest.publicBase !== "/") throw Error("Shared workbench artifact must use the root public base");
  const routes = new Map();
  for (const file of artifact.manifest.files) {
    const body = await readFile(resolve(artifact.directory, file.path));
    if (hash(body) !== file.sha256) throw Error("Workbench artifact changed while capturing server routes");
    routes.set(`/${file.path}`, { body, type: mediaType(file.path) });
  }
  if (!routes.has("/index.html")) throw Error("Shared workbench artifact has no index");
  routes.set("/", routes.get("/index.html")); return routes;
}

export async function serveCollaborativeProject(folder, { invitationsFile, artifactManifest, initialize = false, port = 0, hostname = "127.0.0.1", authoringPreview = {} } = {}) {
  if (!invitationsFile || !(artifactManifest ?? packagedWorkbenchManifest)) throw Error("Collaboration requires explicit --invitations and a prepared --artifact file");
  const invitationPath = resolve(invitationsFile);
  const principals = JSON.parse(await readFile(invitationPath, "utf8"));
  if (!Array.isArray(principals) || !principals.length || principals.length > 128) throw Error("Expected 1..128 trusted invitations");
  const invitations = new Map();
  for (const principal of principals) {
    if (!principal || Object.keys(principal).some(key => !["token", "userId", "role"].includes(key))
      || typeof principal.token !== "string" || principal.token.length < 32 || principal.token.length > 256
      || invitations.has(principal.token) || !["editor", "viewer"].includes(principal.role) || typeof principal.userId !== "string") throw Error("Expected unique invitations with token, userId and editor/viewer role");
    invitations.set(principal.token, { userId: principal.userId, role: principal.role });
  }
  const staticRoutes = await collaborationArtifactRoutes(artifactManifest ?? packagedWorkbenchManifest, artifactManifest ? undefined : workbenchDist);
  const runtime = await openCollaborationRuntime(folder, { initialize, invitations, staticRoutes, workbenchScenes: true, mirror: true, authoringPreview });
  const sessionFile = join(folder, ".geosolve/collaboration/session.json");
  try {
    const address = await runtime.listen(port, hostname), origin = `http://${hostname.includes(":") ? `[${hostname}]` : hostname}:${address.port}`;
    await writeFile(sessionFile, JSON.stringify({ format: "geosolve-collaboration-session-v1", folder: resolve(folder), origin, documentId: runtime.host.configuration.documentId,
      documentEpoch: runtime.host.configuration.documentEpoch, pid: process.pid, invitationsFile: invitationPath }) + "\n", { mode: 0o600 });
    let closing;
    return { runtime, origin, urls: principals.map(({ token, userId, role }) => ({ userId, role, url: `${origin}/?collaboration=1#invite=${encodeURIComponent(token)}` })),
      close() { return closing ??= (async () => { try { await unlink(sessionFile).catch(error => { if (error.code !== "ENOENT") throw error; }); } finally { await runtime.close(); } })(); },
    };
  } catch (error) { await runtime.close(); throw error; }
}
