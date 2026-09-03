// SPDX-License-Identifier: GPL-3.0-or-later
import { copyFileSync, mkdirSync } from "node:fs";
import { basename, parse, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { type Plugin } from "vite";
import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

const frontendDirectory = fileURLToPath(new URL(".", import.meta.url));
const repositoryRoot = resolve(frontendDirectory, "../../..");
const publicBase = process.env.GEOSOLVE_PUBLIC_BASE ?? "./";
const browserCompilerHarness = process.env.GEOSOLVE_BROWSER_COMPILER_HARNESS === "1";
if (publicBase !== "./" && !(publicBase.startsWith("/") && publicBase.endsWith("/"))) {
  throw new Error(`GEOSOLVE_PUBLIC_BASE must be ./ or an absolute trailing-slash path: ${publicBase}`);
}

const outputDirectory = resolve(
  frontendDirectory,
  process.env.GEOSOLVE_DIST ?? "../dist",
);
const relativeToRepository = relative(repositoryRoot, outputDirectory);
const insideRepository = outputDirectory.startsWith(`${repositoryRoot}${sep}`);
const forbiddenOutputDirectories = new Set([
  parse(outputDirectory).root,
  repositoryRoot,
  resolve(repositoryRoot, "crates"),
  resolve(repositoryRoot, "crates/geosolve-demo-web"),
  frontendDirectory,
]);
if (forbiddenOutputDirectories.has(outputDirectory)) {
  throw new Error(`refusing unsafe GEOSOLVE_DIST output directory: ${outputDirectory}`);
}
if (
  process.env.GEOSOLVE_DIST
  && (!basename(outputDirectory).startsWith("geosolve-")
    || (insideRepository && !relativeToRepository.startsWith(`target${sep}`)))
) {
  throw new Error(
    "GEOSOLVE_DIST must name a dedicated geosolve-* artifact outside the repository or under target/",
  );
}

function releaseDocuments(): Plugin {
  const documents = [
    [resolve(repositoryRoot, "LICENSE"), "LICENSE"],
    [resolve(repositoryRoot, "THIRD_PARTY_LICENSES.md"), "THIRD_PARTY_LICENSES.md"],
    [resolve(repositoryRoot, "docs/API_COMPATIBILITY.md"), "API_COMPATIBILITY.md"],
  ] as const;
  return {
    name: "geosolve-release-documents",
    apply: "build",
    closeBundle() {
      mkdirSync(outputDirectory, { recursive: true });
      for (const [source, name] of documents) {
        copyFileSync(source, resolve(outputDirectory, name));
      }
    },
  };
}

export default defineConfig({
  plugins: [react(), releaseDocuments()],
  base: publicBase,
  build: {
    assetsDir: "assets",
    chunkSizeWarningLimit: 800,
    emptyOutDir: true,
    outDir: outputDirectory,
    rollupOptions: browserCompilerHarness ? {
      input: {
        index: resolve(frontendDirectory, "index.html"),
        "compiler-parity": resolve(frontendDirectory, "compiler-parity.html"),
      },
    } : undefined,
    sourcemap: false,
    target: "es2022",
  },
  test: {
    environment: "jsdom",
    exclude: [...configDefaults.exclude, "tests/e2e/**"],
    setupFiles: "./src/test/setup.ts",
    css: true,
  },
});
