<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Editable water manifold project

The accepted 240 × 120 mm manifold is an ordinary local project. Its four water channels
have a shared 12 mm full width, computed walls and tangent bends; the separate stair
passage has two rounded ends. A 2.4 mm silicone seal groove surrounds the wet circuit.

Authored files are deliberately small in number:

| File | Owns |
| --- | --- |
| `geosolve.json` | Editable mode and the entry filename |
| `sketch.ts` | Plate, routes, shared dimensions, metadata and grouping |
| `patches/water-channel.patch.ts` | A polyline passage with one rounded end |
| `patches/point-to-point-channel.patch.ts` | A polyline passage with two rounded ends |
| `patches/silicone-groove.patch.ts` | An enclosing groove around a closed polyline |

The sketch and patch sources are copied from the accepted bundled sample. There are no
precompiled patch artifacts, sample-origin records or generated manifest descriptions.
The local loader discovers and snapshots imports, compiles the three patch modules and
binds their artifacts to the actual source bytes. Edit the files to author the project.

From a prepared repository, first make a working copy:

```bash
mkdir -p target/m98
cp -a examples/file-workspace-manifold target/m98/my-manifold
node packages/geosolve-cli/bin/geosolve.mjs check target/m98/my-manifold
node packages/geosolve-cli/bin/geosolve.mjs serve target/m98/my-manifold
# Open the exact URL printed by serve, including its session token.
```

An agent can read the same files, update a source dimension or patch, then use `check`
to compile and independently validate it. While the bridge is running, `status` shows
current versus accepted revisions and diagnostics:

```bash
node packages/geosolve-cli/bin/geosolve.mjs status target/m98/my-manifold
node packages/geosolve-cli/bin/geosolve.mjs bake target/m98/my-manifold \
  --out target/m98/manifold-profiles.json --chord-error-mm 0.02
```

The profile output belongs outside the project folder and contains model-space regions,
including holes. A bounded arrangement can contain both a plate face with holes and the
individual interior faces. Select the intended named output/regions in the consuming app;
do not infer pocket depth or material removal from a 2D face list.

Useful editing paths:

1. Read `channelWidth` and `sealGrooveWidth` in `sketch.ts`; their source-defined labels and
   overview flags explain the shared intent. Change the single shared width to affect
   all four water passages.
2. Change the stair route's keyed vertices or its locating dimensions. Its patch builds
   two offsets, explicit tangent bends and two end caps from that route.
3. Change a patch source, save, and let the loader evaluate the complete imported project.
   Invalid source remains on disk with the prior accepted geometry available for diagnosis.
4. Use Inspector fields for source-owned dimensions and metadata. Point movement uses the
   explicit design sidecar when supported by the active workbench; it does not rewrite
   solved coordinates into a competing geometry cache.

Only one bridge owns the canonical folder, and one tab owns editing at a time. Use the
visible editing handoff when moving between tabs. Pending fields retain the source and
interaction revision where the edit began; an external save must not silently authorize
an older edit. Inspect retained recovery data before explicitly choosing competing bytes.

The `.gitignore` excludes derived `.geosolve/` files while allowing `design.json` and
`inputs.json` to be committed. Necessary semantic overrides belong in the inspectable
design sidecar; session, view, history and last-good records have their own derived roles.
See [`docs/M98_WORKSPACE_STORAGE.md`](../../docs/M98_WORKSPACE_STORAGE.md)
for the journal and Linux recovery contract. An external writer holding an old descriptor
cannot be treated as atomic filesystem CAS.

The custom [`generator-website`](../generator-website/README.md) example demonstrates the
other mode: ordinary TypeScript loops/conditions, host inputs and a website using the
headless engine without the editable workbench.
