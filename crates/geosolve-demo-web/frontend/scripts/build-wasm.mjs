// SPDX-License-Identifier: GPL-3.0-or-later

import { mkdtemp, readFile, rm, rename } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const frontendDirectory = resolve(scriptDirectory, "..");
const repositoryRoot = resolve(frontendDirectory, "../../..");
const generatedDirectory = resolve(frontendDirectory, "src/generated");
const release = process.argv.includes("--release");
const target = "wasm32-unknown-unknown";
const profile = release ? "release" : "debug";

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    cwd: repositoryRoot,
    encoding: "utf8",
    stdio: options.capture ? "pipe" : "inherit",
    ...options,
  });
  if (result.error) {
    throw new Error(`could not run ${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    if (options.capture && result.stderr) process.stderr.write(result.stderr);
    throw new Error(`${command} exited with status ${result.status}`);
  }
  return result.stdout?.trim() ?? "";
}

const cargoLock = await readFile(resolve(repositoryRoot, "Cargo.lock"), "utf8");
const lockedVersion = cargoLock.match(
  /\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"/,
)?.[1];
if (!lockedVersion) throw new Error("Cargo.lock does not contain wasm-bindgen");

const cliVersion = run("wasm-bindgen", ["--version"], { capture: true }).match(/([0-9]+\.[0-9]+\.[0-9]+)/)?.[1];
if (cliVersion !== lockedVersion) {
  throw new Error(
    `wasm-bindgen CLI ${cliVersion ?? "unknown"} does not match Cargo.lock ${lockedVersion}; use the repository Nix shell`,
  );
}

const cargoArguments = [
  "build",
  "--locked",
  "-p",
  "geosolve-demo-web",
  "--target",
  target,
];
if (release) cargoArguments.push("--release");
run("cargo", cargoArguments);

const metadata = JSON.parse(
  run("cargo", ["metadata", "--locked", "--format-version", "1", "--no-deps"], {
    capture: true,
  }),
);
const inputWasm = resolve(
  metadata.target_directory,
  target,
  profile,
  "geosolve_demo_web.wasm",
);

const stagingDirectory = await mkdtemp(resolve(frontendDirectory, ".generated-wasm-"));
let published = false;
try {
  run("wasm-bindgen", [
    "--target",
    "web",
    "--out-dir",
    stagingDirectory,
    "--out-name",
    "geosolve_demo_web",
    inputWasm,
  ]);

  const generatedTypes = await readFile(
    resolve(stagingDirectory, "geosolve_demo_web.d.ts"),
    "utf8",
  );
  const workbenchContract = [
    "export class WorkbenchHandle",
    "constructor(request: string);",
    "snapshot(): string;",
    "dispatch(request: string): string;",
    "pointer(request: string): string;",
    "wheel(request: string): string;",
    "resize(request: string): string;",
    "cancel(request: string): string;",
    "exportProject(): string;",
    "persistProject(): string;",
    "exportReproduction(): string;",
    "exportInteractionTrace(): string;",
    "intentRpc(request: string): string;",
    "codeControlRpc(request: string): string;",
  ];
  const missingWorkbenchExports = workbenchContract.filter(
    (signature) => !generatedTypes.includes(signature),
  );
  if (missingWorkbenchExports.length > 0) {
    throw new Error(
      `generated WASM bindings do not satisfy the React workbench contract:\n${missingWorkbenchExports
        .map((signature) => `  ${signature}`)
        .join("\n")}`,
    );
  }

  if (release) {
    const wasm = resolve(stagingDirectory, "geosolve_demo_web_bg.wasm");
    const optimized = resolve(stagingDirectory, "geosolve_demo_web_bg.optimized.wasm");
    run("wasm-opt", ["-Oz", wasm, "-o", optimized]);
    await rename(optimized, wasm);
  }

  // A failed bind or optimization must not destroy the last usable package.
  await rm(generatedDirectory, { recursive: true, force: true });
  await rename(stagingDirectory, generatedDirectory);
  published = true;
} finally {
  if (!published) await rm(stagingDirectory, { recursive: true, force: true });
}

console.log(`generated ${profile} WASM bindings in ${generatedDirectory}`);
