<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M99 cleanup qualification

M99 implementation and mechanical qualification are complete on 2026-09-13.
Clean product `eb4d2e06933a7ed5e02533f0359abff28040991a`, tree
`24618ad2b40f85f5d8dfebc261b9ee3dc995edd3`, passes all **297/297** integrated
obligations in run `20260913T155413-41e9f715`: 32 fresh stages and 265 authenticated
reused successes, 2704.185 s wall time. Every signed stage receipt authenticates;
source remained unchanged. This completes the [M99 plan](M99_CLEANUP.md), without
recording human acceptance at qualification time or closing M98's outstanding UAT.
The maintainer's subsequent M99 acceptance and closure are recorded in
[M99_CLOSURE.md](M99_CLOSURE.md); these qualified product bytes remain unchanged.

## Files, APIs and ownership

- `geosolve-constraint-editor` owns generated authoring catalogs,
  `CompletedConstructionReceipt`, `DetachedCanvas`, camera/input/selection,
  presentation and native workspace/reproduction codecs.
- `geosolve-sketch-code` owns source insertion/names/metadata, complete native/source
  terminal validation, accepted source navigation, Inspector projections and
  source-workspace/history admission.
- `geosolve-sketch-engine` owns shared authoring preparation, compiler validation,
  installation, persistable `EditableSession`, accepted inspection/browsing and
  `interaction_seed`; its WASM and TypeScript APIs expose the same services.
- Standalone, folder and collaborative adapters consume those owners through
  `WorkbenchSession`. Browser and Node worker helpers share lifetime mechanics;
  hosts retain their distinct storage, text, publication and history policies.
- `packages/geosolve-cli/runtime` and its ordinary build/package assets own Node
  hosting. The old demo execution copy and milestone-specific runtime paths are
  removed. the downstream CAD project's two consumers use installed `geosolve bake` and retain complete
  package/input provenance, including bundled compiler dependencies.

No equations, primitives, constraints, priority semantics or solver algorithms are
added or changed. Independent residual validation, explicit branches, failure
retention and separate sketch/linkage domains remain mandatory. Detailed removal
and coverage mapping is preserved in [the implementation record](M99_CLEANUP.md).

## Commands and integrated evidence

Commands below ran from the repository checkout in the pinned environment.
The `target/` helpers are historical evidence tooling, not fresh-checkout prerequisites:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260913T152300-cc3b4efc'
python3 target/m98/verify-qualification.py 20260913T155413-41e9f715
python3 target/m98/coordination/tool-parity/extract-final-evidence.py \
  20260913T155413-41e9f715 target/m99/final-summary-20260913T155413-41e9f715.json
nix-shell shell.nix --run 'python3 target/m99/install-qualified.py 20260913T155413-41e9f715'
```

All exit 0. The integrated runner accounts for format, strict Clippy, 247 workspace
and seven additional headless obligations, doctests/documentation, benchmark build,
optimized WASM/lifecycle/parity, package/archive/license, browser and performance
checks. The reviewed golden remains **271/271**, byte-identical SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.

| Integrated host coverage | Passing cases |
| --- | ---: |
| Engine Node | 77 |
| Folder Node / browser | 123 / 11 |
| Generator Node / browser | 2 / 1 |
| Offline installed packages | 2 |
| Collaboration Node / package / frontend / browser | 151 / 44 / 91 / 17 |
| Ordinary browser opening/catalog / full workflows | 17 / 49 |

Ordinary browser evidence originates in `20260913T152300-cc3b4efc`: all 16 samples
and the catalog prefix, followed by 49 fresh full workflows. The final run reuses
that signed stage after exact input/artifact authentication; it does not claim a
second fresh browser execution. There are no failed, skipped, cancelled, retried
or flaky rows in those browser/Node inventories. Native fixture generators retain
their explicit ignored status; required ignored performance/headless cases run
through their registered obligations.

All 25 construction variants, 13 constraints, five dimensions, Fillet, Offset,
roles and metadata retain their native authoring coverage. Recovery includes lost
ACK plus restart/reload, invalid source with accepted canvas retention, Apply while
typing, failed disk publication and exact source/native/personal history.

Measured shared-browser evidence retains the existing limits:

| Scenario | Navigation p95 | Text ACK |
| --- | ---: | ---: |
| Four real editors, ten-second held solve | 276.7 ms | 137.9 ms |
| Manifold, ten-second held solve | 264.2 ms | 335.2 ms |
| Gridfinity, ten-second held solve | 152.7 ms | 221.5 ms |
| Eight editors and 24 viewers, 24 text operations | — | 262.9 ms p95 |

Navigation emits zero model/preview/scene RPCs. Three circle-drag trials have zero
reversals; local p95 is 383.4 ms on initial setup and 89.6/116.3 ms subsequently,
within the existing 500/200 ms limits. Release-to-peer times are 798.8, 405.1 and
371.0 ms. Native M14/M32/M83 and linkage performance checks pass their unchanged
budgets, including the independently validated 256-moving-body chain.

## Exact artifacts and consumer use

`artifact.transport` verifies HTTP bytes, MIME/base paths and actual manifold WASM
readiness, with no browser errors. Production manifest SHA-256 is
`42ec00b84632e82c444b9653e9692e63a9970162a5bd84da14168758f550ad48`;
its file-set digest is
`ebb23bafe6bb51744d9cbdbf0312599819159f6386008833003c849f819b4114`.
The qualified demo WASM is **20,940,556 bytes**, below the unchanged 20 MiB ceiling;
its SHA-256 is `6ca86151bdcd4c7f09fca770c5b572114ec2cca559950d76c13b442a10ea0ef6`.
The earlier 20,794,782-byte size experiment is development evidence, not this
final artifact. Only the demo adapter uses Rust `z`; shared numerical crates
retain level 3. The qualified package also verifies installed folder/shared/
generator startup. Existing preview services remain unchanged.

The offline installer uses a fresh empty npm cache and clears inherited
module/distribution overrides. It authenticates all four qualified archives and
byte-checks **285 installed files** under
`target/m99/installed-20260913T155413-41e9f715/`.

| Archive | SHA-256 |
| --- | --- |
| `geosolve-sketch-code-0.2.0.tgz` | `fd54358fa18a3085fff84e68d58c16694b62a877084d05759c0000af808072f8` |
| `geosolve-engine-0.1.0.tgz` | `f9850469a8c2737269a2ffe78e7e7397a05bc36b7ab1ec58cb453986954cc508` |
| `geosolve-collaboration-0.1.0.tgz` | `a12df28646f54c1bcb7d00da45a519e514b666c582d5afa2bd38fea9efed3da6` |
| `geosolve-cli-0.1.0.tgz` | `9090627cca8d658d49272dc39c86a5ba5aa73240af95f4ab972fdb4f62026001` |

Both final commands ran in the external downstream CAD consumer checkout with
`GEOSOLVE_CLI` set to the absolute executable path in the verified installation:

```bash
python3 scripts/real_bridge_demo.py --output-dir output/bridge-m99
python3 scripts/pi_case_demo.py --no-render --output-dir output/pi-case-m99
```

Exit 0. Widths 85/90 mm bake/import/model and pass independent geometry/port
checks: X increases 5 mm and volume 448.000006676 mm³. Both 22/25 mm case variants
pass 111 STL inspections and 1707 geometry checks each; carrier and roof move 3 mm.
The baked-profile contract remains v1, mm, 0.02 mm chord error and explicit
`region-4`. Both proofs authenticate all archive files and retain unchanged
package bytes, including two npm-generated internal executable links (287 captured
file entries). Eight focused Python receipt/provenance tests pass.

- `../minicad/output/bridge-m99/proof.json`:
  `aeee2f844d430c9dbb997c9869b61b711be412aab393627c5c1eeed97e47601d`.
- `../minicad/output/pi-case-m99/proof.json`:
  `1da7408077a350913da51e8a0fe41a9dfc2629681dffc5a88840f6d467157e4c`.

Consumer migration commit is `46c2662`. The consumer HEAD recorded by the case proof is
`dde5665`; every executed consumer source hash was checked against both the
migration commit and current files.

Full command logs, authenticated stage summaries, installation manifest and
consumer proof comparison are retained in `target/m99/` as
`integrated-final-r6.log`, `verified-final-qualification.json`,
`final-summary-20260913T155413-41e9f715.json`, `install-qualified-final.log`,
`minicad-qualified-consumer-final.log` and `minicad-final-evidence.json`.

## Limits and closeout

M98 remains mechanically qualified but human acceptance is open: U02 remains
**Fail pending human recheck**, and other unperformed human rows remain **Not run**.
The earlier focused manifold startup measurement was 6.484 s; there is no
five-second startup, 60 Hz or arbitrary-size/multi-client scalability claim.
Case rendering was deliberately omitted; physical fit and thermal review are
outside this integration check. No preview replacement, deployment or public push
was performed. Documentation-only closeout preserves the qualified product bytes
under `./scripts/release-gate.sh --docs-only --since eb4d2e0`.
