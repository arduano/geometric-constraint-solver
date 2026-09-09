// SPDX-License-Identifier: GPL-3.0-or-later
import { createServer } from "node:http";
import { readFileSync, realpathSync } from "node:fs";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

/** Serves only built example assets. No compilation, filesystem editing or RPC. */
export async function serveExample({ port = 0 } = {}) {
  const root = realpathSync(fileURLToPath(new URL("../dist", import.meta.url)));
  const server = createServer((request, response) => {
    try {
      if (request.method !== "GET") throw Error("GET required");
      const url = new URL(request.url, "http://localhost");
      const path = realpathSync(resolve(root, `.${decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname)}`));
      if (!path.startsWith(root + sep)) throw Error("Unknown asset");
      const type = { ".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".wasm": "application/wasm" }[extname(path)] ?? "application/octet-stream";
      response.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); response.end(readFileSync(path));
    } catch { response.writeHead(404); response.end("Unknown asset"); }
  });
  await new Promise((done, reject) => { server.once("error", reject); server.listen(port, "127.0.0.1", done); });
  return { url: `http://127.0.0.1:${server.address().port}/`, close: () => new Promise((done) => server.close(done)) };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const server = await serveExample({ port: Number(process.env.PORT ?? 4188) });
  console.log(server.url);
  for (const signal of ["SIGINT", "SIGTERM"]) process.once(signal, () => { void server.close(); });
}
