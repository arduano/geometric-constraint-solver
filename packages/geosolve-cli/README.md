<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# GeoSolve local authoring server and CLI

Use ordinary TypeScript files as the shared authoring surface for a person, an AI
agent and the browser workbench. Saving code updates the validated sketch; supported
canvas and Inspector edits update its source or explicit semantic design sidecar.
The CLI also validates and exports sketches without opening a browser.

The CLI archive contains the compiler, Rust WASM engines and a frozen workbench. Install
the four matching local archives together; no registry, Rust toolchain or source checkout
is needed. Node 22 or newer and Linux `flock` from util-linux are required. Each CLI archive
targets the Linux CPU architecture recorded in its package metadata; the SDK and engine
archives are architecture independent.

```bash
npm install --offline --ignore-scripts /path/geosolve-sketch-code-0.2.0.tgz \
  /path/geosolve-engine-0.1.0.tgz /path/geosolve-collaboration-0.1.0.tgz \
  /path/geosolve-cli-0.1.0.tgz
./node_modules/.bin/geosolve init my-design
./node_modules/.bin/geosolve inspect my-design
./node_modules/.bin/geosolve check my-design
./node_modules/.bin/geosolve serve my-design
```

Open the exact session URL printed by `serve`. The server listens on loopback. The local
files remain authoritative; pending edits, conflicts and recovery are shown in the
workbench. In ordinary folder mode, one bridge and one editing tab own a folder at a time.

`geosolve check <folder>` returns machine-readable independent validation without opening
the workbench. `geosolve bake <folder> --out <outside-file.json> --chord-error-mm 0.08`
exports bounded model-space profiles, including holes. Generator projects can select a
named output with `--output /outline`. Source-defined input labels, descriptions, ranges
and choices also appear in the workbench.

For an agent, first inspect live authority with
`geosolve status <folder> --client <stable-id> --out <state.json>`. Mutations require that
state with `--expected <state.json>`, the same `--client`, and a unique `--operation` ID.
Use `set` for a parameter/input, `apply --files <path-to-contents-map.json>` for a complete
source intent, or `outcome --operation <id>` to resolve an uncertain retry. An operation
ID may be reused only for the same intent. Recovery inspection and explicit resolution
use `recover`; retained competing bytes are not silently discarded.

Generator code runs in terminable workers so cancellation and time limits preserve the
last accepted result. It is trusted local code, with the same access as the CLI process.
Workers do not provide a security sandbox. The filesystem recovery contract targets Linux;
an independent writer retaining an old descriptor is preserved in recovery data.

## Shared editing demo

The installed CLI can also run the reference collaboration server. It accepts
multiple editors and viewers, synchronizes unfinished source text, and independently
validates model edits on the server. Canvas navigation and selection remain local.
Personal Undo preserves other editors' contributions when its ownership checks pass.

Supply an invitations JSON file containing an array of `{ token, userId, role }`
objects. Each token must be unique and between 32 and 256 characters; use a freshly
generated random token for each participant. Roles are `editor` or `viewer`.

```bash
./node_modules/.bin/geosolve serve my-design --collaboration true \
  --invitations /path/to/invitations.json --initialize true \
  --authoring-preview client
```

Open the participant URLs printed by the server. Omit `--initialize true` when
restarting the same shared project so its journal, working text and history restore.
An installed package includes the matching workbench artifact. A source checkout
additionally needs `--artifact /path/to/production-artifact.json` from the release
artifact builder. See the [collaboration package](../geosolve-collaboration/README.md)
for the protocol, authority and bounded-history contracts.

## Building the archives from a prepared checkout

Follow [Getting started](../../docs/GETTING_STARTED.md) to install the development
toolchain and build the SDK, native WASM packages and workbench first.

Production host modules live in this package's `runtime/` directory. The CLI owns
its bundler dependency and ordinary `dist/` build output; runtime execution and
packaging do not read a milestone's `target/` directory. The release artifact
reader is shared with the browser build tools through this package.

```bash
npm --prefix packages/geosolve-cli ci --ignore-scripts
npm --prefix packages/geosolve-cli run build
node packages/geosolve-cli/bin/geosolve.mjs inspect examples/file-workspace
node packages/geosolve-cli/scripts/package.mjs --out target/packages \
  --dist /absolute/path/to/frozen/geosolve-production
node --test scripts/package-m98.test.mjs
```

The packager consumes existing SDK, engine, CLI and WASM build outputs. It
does not build Rust, install packages or contact a registry. It refuses an existing output
directory and records the SHA-256 of every shipped runtime file and archive in
`packages.json`. Pass the candidate's frozen production directory for final qualification;
the mutable development distribution is only suitable for a packaging smoke check.

The authoring package bundles its TypeScript and intent dependencies for offline use.
The CLI includes esbuild's JavaScript and matching native bundler binary; all solving
remains pure Rust WASM. Third-party notices and project licensing travel with the archives.
