<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 implementation findings

M98 remains in implementation; these are focused owner results, not integrated
qualification or milestone acceptance. The hardening skill routes both findings to
folder persistence/transport because the existing Rust workbench retains its contracts.

## M98-F001 — observed disk state authorized an undisplayed Undo

Reproduced at `a913a6d` through `FolderWorkbenchAdapter`: display radius 10, begin an
Inspector draft, externally save radius 15, fetch a hidden background snapshot, Undo.
The request incorrectly used radius 15's hash despite no new snapshot reaching the UI.
The earlier actual-WASM reproduction overwrote the saved radius with radius 10.

The exact adapter regression failed with `radius-15` instead of `radius-10` before
repair. Snapshots now carry their own transport identity. Only explicit host installation
advances editing authority; installation rejects older snapshots and source replacements
under a pending field/source draft. Every queued command captures its authority before
later observations. Typed authoring field begin/change/commit/cancel hooks cover numeric
parameters, dimensions, names and multiline descriptions; application-wide DOM capture
and command-name suffix inference were removed. New input during a commit retains its
own immutable draft basis. Undo, metadata and extraction use the same installed basis.

Focused commands (frontend directory):

- `npx vitest run src/lib/folder-adapter.test.ts`: 5/5 pass after the original 1/2 failure.
- `npx vitest run src/lib/folder-adapter.test.ts src/components/authoring-metadata.test.tsx src/components/dimension-inspector.test.tsx src/components/side-panels.test.tsx`:
  23 tests in the three existing matching files passed at the initial two-adapter-test stage;
  no standalone `side-panels.test.tsx` exists.
- `npx tsc -b --pretty false`: passed.

Session epochs, editor leases and cross-process operation identities follow separately;
these focused tests do not claim those unimplemented guarantees.

## M98-F002 — broken derived cache prevented valid source startup

Reproduced at `a913a6d`: initialize a valid folder and put a single `0xff` byte in
`.geosolve/last-good.ts`. `openProject` threw `ERR_ENCODING_INVALID_ENCODED_DATA` before
reading valid disk source. The initial focused cache suite passed 1/3 and failed both
corrupt-cache cases.

Startup now reconstructs disk source first. Only a failed reconstruction attempts the
optional prior cache. Invalid cache bytes produce warnings; invalid cached TypeScript
restores the complete current-source diagnostic snapshot. A valid prior cache can retain
accepted geometry while the exact rejected disk source remains a draft. Cache update
failures are warnings rather than false source-save failures.

`node --test scripts/workspace-cache.test.mjs`: 4/4 pass using the retained actual WASM
runtime. This verifies the JavaScript persistence repair; the integrated M97/M98 WASM
will be rebuilt and qualified at nomination. Tests check exact source hashes and bytes,
last accepted identity, current rejected drafts and diagnostics without treating a cache
as authored authority. No solver equation, branch or golden-oracle change is involved.
