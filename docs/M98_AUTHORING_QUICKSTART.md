<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 authoring quickstart

Use editable mode when both source edits and workbench edits should change a design. Use
generator mode when ordinary TypeScript computes the design from inputs and your host owns
the controls. Geometry, constraints and profile validation use the same Rust engine.

## A local agent edit

Install the four matching archives together in a new tools directory:

```bash
npm install --offline --ignore-scripts /path/geosolve-sketch-code-0.2.0.tgz \
  /path/geosolve-engine-0.1.0.tgz /path/geosolve-collaboration-0.1.0.tgz \
  /path/geosolve-cli-0.1.0.tgz
./node_modules/.bin/geosolve init my-design
./node_modules/.bin/geosolve serve my-design --port 18109
```

Keep `serve` running. In a second terminal in the same directory, build a candidate source
file map outside the project, then obtain editing ownership and fresh expected state:

```bash
node --input-type=module <<'JS'
import { readFileSync, writeFileSync } from 'node:fs';
const source = readFileSync('my-design/sketch.ts', 'utf8');
if (!source.includes('value: mm(10)')) throw new Error('Expected the initialized 10 mm dimension');
writeFileSync('candidate-files.json', JSON.stringify({
  'sketch.ts': source.replace('value: mm(10)', 'value: mm(14)'),
}, null, 2));
JS
./node_modules/.bin/geosolve status my-design --client example-agent --out expected.json
./node_modules/.bin/geosolve takeover my-design --client example-agent --expected expected.json
./node_modules/.bin/geosolve status my-design --client example-agent --out expected.json
./node_modules/.bin/geosolve apply my-design --client example-agent --expected expected.json \
  --operation example-dimension-14-1 --files candidate-files.json
./node_modules/.bin/geosolve outcome my-design --operation example-dimension-14-1
```

The file map contains complete candidate UTF-8 contents keyed by project-relative paths.
The bridge validates the whole candidate dependency graph before publishing authored files.
The second `status` captures the new editing lease; a status read alone does not take ownership.
Use refreshed expected state and a new operation ID for each new intent. If a response is lost,
query `outcome` or retry the identical request with its original operation ID and basis.
An explicit handoff changes which browser tab or agent can author the folder.

`inspect` and `check` work without a browser. `bake` writes complete model-space profiles:

```bash
./node_modules/.bin/geosolve check my-design
./node_modules/.bin/geosolve bake my-design --out profiles.json --chord-error-mm 0.02
```

Use the maintained [manifold folder](../examples/file-workspace-manifold/README.md) for a
multi-file project with imported custom patches. Source owns its named parameters, dimensions,
labels and groups. Necessary semantic GUI overrides belong in `.geosolve/design.json`;
history/view caches are derived. See [storage and recovery](M98_WORKSPACE_STORAGE.md).

## A generator in a custom host

```ts
import { createEngine } from '@geosolve/engine';
import { defineGenerator, mm, sketch } from '@geosolve/sketch-code';

const circle = defineGenerator({
  radius: { type: 'number', default: 12, min: 1, unit: 'mm', label: 'Radius' },
}, ({ radius }) => sketch(($) => ({
  ring: $.geometry.centerRadiusCircle('ring', { center: [0, 0], radius: mm(radius) }),
})));

const engine = await createEngine();
try {
  const result = await engine.evaluate({ definition: circle, parameters: { radius: 14 } });
  if (result.status === 'accepted') {
    const profiles = await engine.exportProfiles(result, { chordErrorMm: 0.02 });
    // Draw profiles.regions in your host, or retain result.geometry for other views.
    console.log(profiles.regions.length);
    engine.release(result);
  }
} finally {
  engine.dispose();
}
```

Input descriptors declare invocation defaults, validation and control metadata. The sketch
still applies units explicitly with `mm(...)`. `isKeyParameter` belongs to an authored
`$.parameter`, and `isKeyConstraint` to a dimension. They select overview information;
they do not declare generator inputs. Plain functions without a schema are also valid.

The maintained [generator website](../examples/generator-website/README.md) shows loops,
conditions, imported helpers, computed fillets, named profiles with holes and worker cancellation.
Its page owns the controls and renderer and imports only the SDK and headless engine.
Generator output does not grant reverse source-editing authority. Trusted generator code runs
with its host's access; terminate a worker to preempt synchronous work. An inline callback
already executing in the host cannot be interrupted by AbortSignal.

## Opt-in shared folder authoring

M98's collaboration amendment is mechanically qualified and delivered with exact
served-byte and browser verification. [The nomination](M98_QUALIFICATION.md#qualified-shared-toolbar-parity)
records the exact candidate and evidence. The opt-in shared host reuses the editable
TypeScript folder and compiler; the original single-editor mode remains available.

Create an invitations JSON file containing trusted principals:

```json
[
  { "token": "replace-with-a-random-secret-of-at-least-32-characters", "userId": "alice", "role": "editor" },
  { "token": "replace-with-a-different-random-secret-at-least-32-characters", "userId": "bob", "role": "viewer" }
]
```

Start the installed CLI with its bundled artifact:

```bash
geosolve serve ./manifold --collaboration true --initialize true \
  --invitations ./invitations.json --authoring-preview client
```

Use `--initialize true` once to create a new collaborative history from the folder's
source. Restart the same folder with the same invitations and omit `--initialize true`.
Initialization does not migrate legacy single-editor semantic overrides or history;
preserve that original folder when preparing a separate collaborative copy.

A repository launch additionally supplies `--artifact <prepared-artifact-manifest>`.
`--host <Tailscale-IP> --port <port>` binds a chosen interface. The command emits one
invitation URL per principal. Each editor has an independent camera, selection,
Inspector, visibility and tool state. CodeMirror shares raw typing, including invalid
syntax. Apply captures the current draft; later typing remains unapplied. Canvas edits
use the accepted model while the draft is incomplete, with visible reconciliation
notices when safe source writeback is unavailable.

`--authoring-preview disabled` is the default and uses client prediction.
`client` enables optional server prediction while preferring the client; `server`
prefers server prediction. The URL can override the advertised preference with
`?collaboration=1&authoringPreview=server`. Both paths render and navigate locally.
The shared toolbox exposes all 25 geometry variants, 13 constraint tools, five dimension
tools, Fillet and Profile Offset, plus native point dragging. Select a tool to reveal its
native options and staged guidance. Compatible preselection or Explorer inputs supply
operands; Finish, Step Back and Escape follow native draft capabilities. With pending
inputs, Escape clears the draft; Escape again exits. Empty tools can exit directly.
Profile/Construction sets the new-geometry default or changes selected geometry through its writable source role path.
Accepted edits enter the current user's Undo/Redo and preserve peer contributions.
Source/Inspector value and metadata edits, parameter extraction, suppression, deletion
and ordering retain the same authority. [Toolbar scope](M98_TOOL_PARITY.md) records the
ordinary writable-source requirements and archived action exclusions.

The CLI shares the same authority. Read a status checkpoint before an edit, retain
stable `--user`/`--client` identities and supply a unique `--operation` per intent:

```bash
geosolve status ./manifold --collaboration true --user alice \
  --client agent-alice --out expected.json
geosolve apply ./manifold --collaboration true --user alice \
  --client agent-alice --expected expected.json --operation apply-draft-001
geosolve outcome ./manifold --collaboration true --user alice \
  --client agent-alice --operation apply-draft-001
```

Retry an uncertain operation with its original ID and exact payload. `draft` accepts
an `--edits` JSON file of native text/file edits; `text-undo`/`text-redo` change raw
source. `undo`/`redo` change the caller's latest available model contribution and
refuse to replace a newer contribution by another editor. External file saves feed
the raw working draft through the same gateway; they do not silently Apply.

[The protocol contract](M98_COLLABORATION.md) records authority, lifecycle, recovery
and measured scale. [Server prediction](M98_AUTHORING_PREVIEW.md) documents its
separate cancellation and resource limits.
