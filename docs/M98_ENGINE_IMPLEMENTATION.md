<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 headless authoring and engine implementation

The authoring SDK records ordinary synchronous `sketch(...)` executions into a bounded
`geosolve-generated-sketch-v1` artifact. `recordedSketch` retrieves immutable evaluated
arguments, semantic references, groups, applications, parameters, metadata and nested
outputs. `defineGenerator` adds optional typed invocation inputs, defaults and validation.
The existing managed compiler and its lexical/execution receipt validation remain strict.

`scripts/workspace-loader.mjs` captures complete local TypeScript dependencies, manifest
and optional sidecar/input files. A terminable worker bundles only captured project bytes
and the exact installed SDK. Editable mode compiles fresh local patch artifacts and pins;
generator mode executes the default function with declared or host-supplied inputs. The
result status is `compiled`, because only native admission can establish accepted geometry.

`geosolve-sketch-engine` and its dedicated WASM adapter share the existing equation-free
expansion, computed feature recipes and independent native residual validation. Generated
programs do not fabricate managed source receipts. The native tests include the actual
manifold through both managed and generated admission. Computed profile export and
editable sessions remain under implementation.

`packages/geosolve-engine` exposes `createEngine`, `evaluate`, `evaluateGenerated`,
`compileProject`, `evaluateEditable`, `exportProfiles`, `release` and `dispose`. Accepted
result objects are immutable, owned by their engine and retained across later rejected
requests. Generator callbacks run in their caller's realm; cancellation can suppress queued
work but cannot preempt a running inline callback. First-party loaders use terminable workers.
No React, DOM, workbench or preview server is needed to run the dedicated engine.

Focused development evidence:

- Recorder agent: 50 SDK/runtime/compiler tests passed before the local module path
  amendment. The actual manifold records 146 declarations, five patch applications,
  two named parameters and seven groups.
- `node --test packages/geosolve-sketch-code/dist/test/patch-compiler.test.js`: 14/14
  after ordinary local `.ts`/`.js` module paths with traversal/remote rejection.
- `node --test scripts/workspace-loader.test.mjs`: 7/7 with the rebuilt SDK.
- `node packages/geosolve-engine/scripts/build.mjs`: JavaScript and declarations pass.
- `node --test packages/geosolve-engine/test/engine.test.mjs packages/geosolve-engine/test/native.test.mjs`:
  five wrapper/actual-WASM tests pass. Two independent circles from a loop export at
  radius 12 mm; structural count and radius changes export one 8 mm circle; rejected
  input and forged managed source retain prior accepted results.
- The native agent built the initial dedicated release WASM with
  `CARGO_BUILD_JOBS=2 nix-shell shell.nix --run 'node packages/geosolve-engine/scripts/build-wasm.mjs'`.
  That initial artifact predates full computed export and is development evidence only.

No solver residual equation, unsafe implementation or golden corpus change is involved.
The package, website, complete project sidecar workflow and integrated release gate still
need qualification before M98 can be nominated.

## Completed native and package seams (2026-09-09)

The dedicated native and WASM engines now admit ordinary evaluated generators and
strict complete CodeProjects through the same expansion and independently validated
materialization. `Engine.openEditableSession` exposes revision-bound project and
semantic-overlay updates, Undo/Redo, and `exportDesign`; immutable accepted result
handles remain independently exportable until released. Generator results never gain
reverse source authority. The semantic sidecar contains project identity, keyed
reconciliation and authored overrides, without solved geometry or an editor checkpoint.

Full computed manifold export passes at chord error 0.02 mm: 18 bounded arrangement
faces, finite vertices, correct outer/hole winding, total signed area 28,800 mm².
Named output selection returns complete regions touched by a selected outer boundary,
including all their holes. Production topology and numerical uncertainty still reject
unsupported, open or ambiguous geometry; the engine does not infer material/cut semantics.

Focused commands executed in the M98 worktree:

- `cargo test --locked -p geosolve-demo-web --lib m98_ -- --test-threads=1`:
  2/2 pass, including sidecar point reconstruction and foreign-project rejection.
- `cargo clippy --locked -p geosolve-demo-web -p geosolve-sketch-code --all-targets -- -D warnings`:
  pass after the sidecar initializer takes borrowed keyed state.
- Dedicated engine native sessions 2/2 and WASM adapter 3/3 pass; native and wasm32
  strict Clippy pass. Actual Node release WASM session and existing engine tests pass.
- `node packages/geosolve-engine/scripts/build.mjs` and wrapper tests pass.

This component evidence is not an integrated M98 nomination. The final gate, archives,
served-byte verification and supervising-user acceptance remain required.
