<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Getting started

The [browser demo](https://arduano.github.io/geometric-constraint-solver/) needs no
installation. It is a published snapshot. Use a local checkout or matching offline
packages for the latest source and server features.

## Install the packaged local server

If you have the four matching `.tgz` archives, follow the
[CLI installation guide](../packages/geosolve-cli/README.md). They contain the
compiler, WASM engines and frozen workbench, and install together without a Rust
toolchain or registry access. Installed use requires Node 22 or newer and Linux
`flock`; the CLI archive must match your CPU architecture. Registry publication is
not assumed.

After installation, run `./node_modules/.bin/geosolve init my-design` and
`./node_modules/.bin/geosolve serve my-design` from the tools directory.
Open the exact session URL printed by the server.
The [authoring guide](AUTHORING.md) explains how source and UI edits interact.

## Prepare a source checkout

Run the remaining commands from the repository root. Build prerequisites are:

- Rust/Cargo 1.90 or newer for the full workspace, with the matching
  `wasm32-unknown-unknown` standard library, rustfmt and Clippy. The domain baseline
  in [Cargo.toml](../Cargo.toml) is 1.89; collaboration requires 1.90.
- `wasm-bindgen` CLI at the exact version of `wasm-bindgen` in [Cargo.lock](../Cargo.lock),
  Binaryen's `wasm-opt`, and a WASM-capable linker such as `lld`.
- Node 22 or newer, npm, and Deno for restricted source execution and qualification.
- Python 3 for the release runner; Linux `flock` for the filesystem server.
- Chromium and its system dependencies for actual-browser tests. Playwright can
  install its matching browser after frontend dependencies are installed.

[shell.nix](../shell.nix) supplies the main development tools:

```bash
nix-shell shell.nix
```

It uses your configured `nixpkgs`; it is not a self-contained toolchain pin and does
not install the Rust WASM target. Select a compatible Nix environment or use
rustup with a matching target and `wasm-bindgen` CLI. For a rustup-managed compiler:

```bash
rustup target add wasm32-unknown-unknown
rustup component add rustfmt clippy
```

Do not mix a target standard library from one Rust toolchain with another compiler.
The build scripts check the binding CLI against `Cargo.lock`. Qualification records
exact tool and dependency identities rather than assuming every development shell
is equivalent. Initial dependency installation and a cold build need network access;
this differs from the offline installed-server workflow.

## Build the source packages

Install locked JavaScript dependencies, then build in dependency order:

```bash
npm --prefix packages/geosolve-intent ci --ignore-scripts
npm --prefix packages/geosolve-intent run build
npm --prefix packages/geosolve-sketch-code ci --ignore-scripts
npm --prefix packages/geosolve-sketch-code run build
npm --prefix packages/geosolve-engine install --ignore-scripts --package-lock=false
npm --prefix crates/geosolve-demo-web/frontend ci --ignore-scripts
npm --prefix packages/geosolve-cli ci --ignore-scripts
npm --prefix packages/geosolve-engine run build:wasm
npm --prefix packages/geosolve-engine run build
npm --prefix packages/geosolve-collaboration run build:wasm
npm --prefix packages/geosolve-collaboration run build
npm --prefix packages/geosolve-cli run build
npm --prefix crates/geosolve-demo-web/frontend run wasm:release
npm --prefix crates/geosolve-demo-web/frontend run build:ui
```

Engine and collaboration WASM are separate packages; the demo's WASM build does
not prepare them. The engine install links its local SDK dependency; this package
has no lockfile or external dependencies of its own. Its TypeScript build uses
TypeScript and esbuild from the installed SDK/frontend dependencies, so keep this order.
Optimized WASM is recommended for dense samples such as the manifold.

Native Rust libraries can also be built independently with
`cargo build --locked --workspace`. Some collaboration tests require a native text
fixture:

```bash
cargo build --locked -p geosolve-collaboration --example text_fixture
```

## Run the browser workbench

After the package preparation above:

```bash
npm --prefix crates/geosolve-demo-web/frontend run dev:ui -- --host 127.0.0.1
```

Open Vite's printed URL. This serves the standalone browser workbench. Its source
editor and canvas use the in-browser engine; it does not watch a local project
folder. Rust changes require rebuilding the affected WASM package. `npm run dev`
in the frontend builds development demo WASM first; use the optimized preparation
above when evaluating dense-sketch responsiveness.

## Run a project folder from source

Build frozen frontend artifacts into a new output directory:

```bash
mkdir -p target/local-demo
npm --prefix crates/geosolve-demo-web/frontend run build:release-artifacts -- \
  --out ../../../target/local-demo/artifacts --base ./ --wasm-package src/generated
node packages/geosolve-cli/bin/geosolve.mjs init target/local-demo/my-design
node packages/geosolve-cli/bin/geosolve.mjs inspect target/local-demo/my-design
node packages/geosolve-cli/bin/geosolve.mjs check target/local-demo/my-design
GEOSOLVE_DIST="$PWD/target/local-demo/artifacts/geosolve-production" \
  node packages/geosolve-cli/bin/geosolve.mjs serve target/local-demo/my-design
```

The npm script resolves its output and prepared-WASM paths from the frontend
directory. `--wasm-package src/generated` reuses the packages built above; omitting
it rebuilds all three WASM packages. The artifact builder refuses an existing output
directory, so choose a new one for each rebuild. Ordinary folder serving selects
its static directory with `GEOSOLVE_DIST`; `--artifact` is a shared-server option.
The server prints its session URL; open it and edit the project's
`sketch.ts`. Supported Inspector/canvas edits write back to the project. A single
folder bridge has one active editing owner; use the [shared server](../packages/geosolve-cli/README.md#shared-editing-demo)
for multiple editors. Installed CLI packages already contain a matching workbench
and need neither a distribution override nor a shared artifact argument.

Stop a server with Ctrl-C. Restart an existing folder with `serve`, preserving the
whole folder and its `.geosolve` state. Shared servers additionally retain their
invitations and journal; omit `--initialize true` on restart. Source-only copies
are not complete backups of accepted state, drafts or personal history. Read the
[storage contract](M98_WORKSPACE_STORAGE.md) and [collaboration recovery contract](M98_COLLABORATION.md)
before migrating or pruning project state.

## Build offline archives and qualify

The [CLI packaging instructions](../packages/geosolve-cli/README.md#building-the-archives-from-a-prepared-checkout)
consume the prepared production directory and create all four archives with a
file/hash manifest. Use a new package output directory. A development package
smoke check is not release qualification; [the release runner](RELEASE_QUALIFICATION.md)
records the exact source, artifacts and passing obligations for a nomination.
