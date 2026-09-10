// SPDX-License-Identifier: GPL-3.0-or-later
// Small real-browser storage fixture. No solver, workbench bundle or preview server.
import { createServer, type Server } from "node:http";
import { readFile } from "node:fs/promises";
import { URL as NodeURL } from "node:url";
import { chromium, type Browser, type BrowserContext, type Page } from "@playwright/test";
import { transpileModule, ModuleKind, ScriptTarget } from "typescript";

export async function storageBrowserFixture(): Promise<{
  browser: Browser; context: BrowserContext; url: string;
  page(): Promise<Page>; close(): Promise<void>;
}> {
  const server: Server = createServer((request, response) => {
    void (async () => {
      const name = new NodeURL(request.url!, "http://fixture").pathname.slice(1);
      if (!name) {
        response.setHeader("Content-Type", "text/html");
        response.end(`<!doctype html><title>Storage fixture</title><script type="module">
          import * as storage from "/collaboration-storage";
          import * as identity from "/collaboration-tab-identity";
          window.storageModule = storage; window.identityModule = identity;
        </script>`);
        return;
      }
      if (!["collaboration-storage", "collaboration-tab-identity"].includes(name)) { response.writeHead(404).end(`Unknown fixture module: ${name}`); return; }
      const source = await readFile(new NodeURL(`./${name}.ts`, import.meta.url), "utf8");
      response.setHeader("Content-Type", "text/javascript");
      response.end(transpileModule(source, { compilerOptions: { target: ScriptTarget.ES2022, module: ModuleKind.ESNext } }).outputText);
    })().catch(error => { response.writeHead(500).end(String(error)); });
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw Error("Browser fixture has no port");
  const url = `http://geosolve-storage.test:${address.port}/`;
  let browser: Browser;
  try {
    browser = await chromium.launch({
      executablePath: process.env.GEOSOLVE_CHROMIUM_PATH ?? "/home/arduano/.nix-profile/bin/google-chrome",
      args: ["--disable-dev-shm-usage", "--no-proxy-server", "--host-resolver-rules=MAP geosolve-storage.test 127.0.0.1"],
    });
  } catch (error) { server.close(); throw error; }
  const context = await browser.newContext();
  return {
    browser, context, url,
    async page() {
      const page = await context.newPage(), errors: string[] = [];
      page.on("pageerror", error => errors.push(String(error)));
      page.on("response", response => {
        if (response.status() >= 400) void response.text().then(body => errors.push(`${response.status()} ${response.url()}: ${body}`));
      });
      await page.goto(url);
      try { await page.waitForFunction(() => Reflect.get(window, "identityModule"), undefined, { timeout: 1000 }); }
      catch (error) { await page.close(); throw Error(`${String(error)}\n${errors.join("\n")}`); }
      return page;
    },
    async close() { await browser.close(); await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve())); },
  };
}
