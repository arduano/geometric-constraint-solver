// SPDX-License-Identifier: GPL-3.0-or-later

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { parseArgs } from "node:util";
import { pathToFileURL } from "node:url";
import { hash, mediaType, readManifest } from "./release-artifact-lib.mjs";

export async function serveArtifact(manifestPath, { port = 4173, host = "127.0.0.1", directory } = {}) {
  const artifact = await readManifest(manifestPath, directory);
  const prefix = artifact.manifest.publicBase === "./" ? "/" : artifact.manifest.publicBase;
  const routes = new Map();
  for (const file of artifact.manifest.files) {
    const body = await readFile(resolve(artifact.directory, file.path));
    if (hash(body) !== file.sha256) throw new Error("artifact changed while preparing server");
    routes.set(`${prefix}${file.path}`, { body, type: mediaType(file.path) });
  }
  routes.set(prefix, routes.get(`${prefix}index.html`));
  // Serve only authenticated captured bytes. Missing routes fail 404 instead of
  // silently serving the index, including the absent production compiler page.
  const server = createServer((request, response) => {
    const route = routes.get(request.url?.split("?", 1)[0]);
    if (!route || !["GET", "HEAD"].includes(request.method)) {
      response.writeHead(404, { "Content-Type": "text/plain", "Cache-Control": "no-store" });
      response.end("Not found\n");
      return;
    }
    response.writeHead(200, { "Content-Type": route.type, "Content-Length": route.body.length, "Cache-Control": "no-store" });
    response.end(request.method === "HEAD" ? undefined : route.body);
  });
  await new Promise((accept, reject) => { server.once("error", reject); server.listen(port, host, accept); });
  return { server, artifact, baseUrl: `http://${host}:${server.address().port}${prefix}` };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const { values } = parseArgs({ options: { manifest: { type: "string" }, directory: { type: "string" }, port: { type: "string" } } });
  const manifest = values.manifest ?? process.env.GEOSOLVE_E2E_ARTIFACT_MANIFEST;
  const port = Number(values.port ?? process.env.GEOSOLVE_E2E_PORT ?? "4173");
  if (!manifest || !Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error("usage: node scripts/serve-artifact.mjs --manifest <manifest> [--directory <moved-copy>] [--port 4173]");
  }
  const { server, baseUrl } = await serveArtifact(manifest, { port, directory: values.directory });
  console.log(`serving verified artifact at ${baseUrl}`);
  for (const signal of ["SIGINT", "SIGTERM"]) process.once(signal, () => server.close(() => process.exit(0)));
}
