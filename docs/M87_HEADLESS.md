<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Browser-free authoring loop

`geosolve-headless` runs managed-code expansion, the native solver, independent acceptance checks,
scene fitting, SVG composition and PNG rasterization without a browser, web server, DOM, network,
Node or TypeScript runtime. These pure-Rust `inspect` and `render` paths are suitable for an AI
agent or native host that needs exact semantic controls and visual evidence from the same accepted
design authority. Source mutation remains browser-free, but deliberately invokes the pinned Deno
compiler between Rust-owned prepare and resolve phases.

M87 is accepted and closed. Its browser-free project and two-phase mutation flows remain. M92
clean-breaks the old twelve-entry demo vocabulary into the one canonical twenty-entry bundled
sample registry and replaces report v1 with deterministic `geosolve-headless-report-v2`.

M90 supersedes the historical M87 raw-source input and one-phase edit APIs. The current clean-break
contract admits only executed V3 compiler authority (`geosolve-managed-sketch-ir-v3` plus
`geosolve-executed-sketch-artifact-v3`) and uses `prepare-edit` followed by the pinned Deno mutation
sidecar and `resolve-edit`. There is no `--managed`, `--project-key`, or synchronous `edit` command.

## Inputs

Choose exactly one input for each command:

- `--project project.json` accepts a complete compiled V3 `CodeProject`, including pinned data-only
  artifacts;
- `--sample KEY` selects one of the twenty checked-in source-authoritative projects listed by
  `samples`.

Raw `sketch.ts` is not an admitted native input because Rust does not pretend to execute TypeScript.
A custom patch remains a compiled, pinned extension point: its TypeScript source is never executed
by the headless solver, and its canonical data-only artifact must already be in `project.json` or a
bundled sample.

## Inspect

```bash
cargo run --locked -p geosolve-headless -- \
  inspect --sample theo-jansen-leg > inspection.json
```

The JSON contains a deterministic solve/render report and a transient managed-control manifest.
Each editable row has an exact source token, typed schema, current value and complete bounded list
of semantic consumers. Read-only rows explain whether the value is structure, a reference, a
structural identity, a solver-instance/DoF value or another deliberately unsupported inverse.

Tokens authenticate the exact project, source bytes, expected typed value and generated consumer
identities. Do not synthesize or reuse a token after any source or project edit; inspect again.

## Two-phase exact-CAS edit

Copy one or more complete `access.token` objects from the inspection into a batch. The replacement
uses the tagged `ManagedValue` wire shape. For example, a length change is:

```json
{
  "edits": [
    {
      "token": { "copy": "the complete token from inspection.json" },
      "value": {
        "kind": "unit",
        "value": { "unit": "mm", "value": 2.0 }
      }
    }
  ]
}
```

The `token` placeholder above is explanatory, not valid input: replace it with the complete token
object verbatim. Number, boolean, choice and text replacements respectively use `number`, `bool`
and `string` tagged values. A unit replacement must retain the schema's exact unit.

First ask Rust to authenticate the current compiled project, source token, expected value and exact
permitted semantic delta. The command writes a compiler request to stdout and changes no project:

```bash
cargo run --locked -p geosolve-headless -- \
  prepare-edit --sample theo-jansen-leg --edit batch.json > /tmp/geosolve-prepared.json
```

Build the pinned TypeScript package, then pass the complete prepared request to the Deno mutation
sidecar. The checked-in sidecar requires Deno `2.9.4` and the package pins TypeScript `5.9.2`; the
repository Nix shell provides the intended toolchain. It emits a candidate compiler receipt but
has no solver or publication authority:

```bash
(
  cd packages/geosolve-sketch-code
  npm ci --ignore-scripts
  npm run build
  deno run --no-config --no-lock --no-prompt --cached-only --no-remote \
    --node-modules-dir=manual --ignore-env scripts/mutate-managed-deno.mjs \
    < /tmp/geosolve-prepared.json > /tmp/geosolve-receipt.json
)
```

Finally give both unchanged exchange files to Rust against the same input authority. Rust verifies
the ticket and complete compiler envelope, rejects any unrelated semantic change, cold-materializes
native Intent, solves, independently validates, and only then atomically publishes the candidate:

```bash
cargo run --locked -p geosolve-headless -- \
  resolve-edit --sample theo-jansen-leg \
  --prepared /tmp/geosolve-prepared.json \
  --receipt /tmp/geosolve-receipt.json \
  --out /tmp/geosolve-jansen-r2
```

A stale token, mismatched input, tampered request or receipt, wrong type/unit, non-finite value,
failed materialization or failed acceptance publishes no output directory. Shared controls edit one
source value and update every runtime-authenticated consumer. Neither preparation nor a rejected
resolution mutates the input project.

For the next stateless iteration, inspect the emitted `project.json`, then use its new tokens:

```bash
cargo run --locked -p geosolve-headless -- \
  inspect --project /tmp/geosolve-jansen-r2/project.json > inspection-r2.json
```

## Render products

```bash
cargo run --locked -p geosolve-headless -- \
  render --project project.json --out /tmp/geosolve-generation-new
```

The destination must be a new directory. On Linux, Android, Apple platforms and Redox,
publication uses an atomic no-replace directory rename and never overwrites an existing
generation. Other targets currently fail closed before publication because the crate does not yet
have a proven atomic no-clobber directory primitive for them. A successful directory contains:

- `report.json` — input/digest identities, independent validation, numerical right nullity,
  equality and bidirectional bounded DOF, ordered source-group names/counts, fitted camera and
  output hashes;
- `controls.json` — the exact transient managed-control manifest;
- `project.json` — canonical complete project authority;
- `sketch.ts` — the accepted managed source;
- `scene.svg` — deterministic static vector output;
- `scene.png` — `2000 × 1400` native raster output.

The logical scene is `1000 × 700`, fitted with a 64 px margin, a 2–2000 px/unit scale clamp and a
0.25 px chord tolerance. Static output includes grid, datums, geometry and annotations; selection,
hover, drafts, inference, action affordances and error overlays are absent. SVG/report bytes are
deterministic authority. PNG is deterministic visual evidence for the pinned build inputs, not a
solver oracle.

Every success-like result has finite current accepted geometry, independently validated hard
residuals no greater than `1e-9` (or a validated empty hard set), and Current active computed
features. No solved coordinate is ever written back into managed source.

## Showcase and scale dogfood

Every entry in the canonical registry uses the same stateless path. For example:

```bash
cargo run --locked -p geosolve-headless -- \
  inspect --sample robotic-harness-backplane > backplane-inspection.json

cargo run --locked -p geosolve-headless -- \
  render --sample gridfinity-bin-section --out /tmp/geosolve-gridfinity-new
```

The headless regressions open, inspect and render all twenty entries through the ordinary managed
path, compare report bytes, verify manifest mobility and exact group order/counts, and require
independent native acceptance. Generate products into new directories. Wall-clock and solver timing
remain outside report v2.

These outputs are 2D/2.5D design evidence and do not create CAM, toolpath, solid, printer-fit, or
manufacturing authority.
