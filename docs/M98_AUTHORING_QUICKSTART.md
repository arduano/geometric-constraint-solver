<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 authoring quickstart

Use editable mode when both source edits and workbench edits should change a design. Use
generator mode when ordinary TypeScript computes the design from inputs and your host owns
the controls. Geometry, constraints and profile validation use the same Rust engine.

## A local agent edit

Install the three matching archives together in a new tools directory:

```bash
npm install --offline --ignore-scripts /path/geosolve-sketch-code-0.2.0.tgz \
  /path/geosolve-engine-0.1.0.tgz /path/geosolve-cli-0.1.0.tgz
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
