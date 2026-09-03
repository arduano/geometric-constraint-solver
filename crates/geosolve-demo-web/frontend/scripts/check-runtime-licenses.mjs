// SPDX-License-Identifier: GPL-3.0-or-later

import { readFile } from "node:fs/promises";

const [manifest, lock] = await Promise.all([
  readFile(new URL("../package.json", import.meta.url), "utf8").then(JSON.parse),
  readFile(new URL("../package-lock.json", import.meta.url), "utf8").then(JSON.parse),
]);
if (lock.lockfileVersion !== 3 || lock.name !== manifest.name) {
  throw new Error("package-lock.json is not the expected npm v3 workbench lock");
}
const lockedRoot = lock.packages?.[""];
const declaredRuntime = Object.keys(manifest.dependencies ?? {}).sort();
const lockedRuntime = Object.keys(lockedRoot?.dependencies ?? {}).sort();
if (JSON.stringify(declaredRuntime) !== JSON.stringify(lockedRuntime)) {
  throw new Error("package.json runtime dependencies drifted from package-lock.json");
}

const permitted = new Set(["0BSD", "Apache-2.0", "ISC", "MIT"]);
const runtimePackages = Object.entries(lock.packages)
  .filter(([path, metadata]) => path && metadata.dev !== true)
  .map(([path, metadata]) => ({
    name: path.replace(/^node_modules\//, ""),
    version: metadata.version,
    license: metadata.license,
  }));

const failures = runtimePackages.filter(
  ({ license }) => typeof license !== "string" || !permitted.has(license),
);
if (failures.length > 0) {
  throw new Error(
    `unreviewed runtime licence metadata:\n${failures
      .map(({ name, version, license }) => `  ${name}@${version}: ${license ?? "missing"}`)
      .join("\n")}`,
  );
}
const unversioned = runtimePackages.filter(
  ({ version }) => typeof version !== "string" || version.length === 0,
);
if (unversioned.length > 0) {
  throw new Error(
    `runtime packages lack an exact locked version: ${unversioned.map(({ name }) => name).join(", ")}`,
  );
}
const unlocked = Object.entries(lock.packages)
  .filter(([path, metadata]) => path && metadata.dev !== true && metadata.link !== true)
  .filter(([, metadata]) => (
    typeof metadata.resolved !== "string" || typeof metadata.integrity !== "string"
  ))
  .map(([path]) => path.replace(/^node_modules\//, ""));
if (unlocked.length > 0) {
  throw new Error(`runtime packages lack registry integrity: ${unlocked.join(", ")}`);
}

const inventory = [...new Set(runtimePackages.map(({ license }) => license))].sort();
console.log(
  `validated ${runtimePackages.length} locked browser runtime packages: ${inventory.join(", ")}`,
);
