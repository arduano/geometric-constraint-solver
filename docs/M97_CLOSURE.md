<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M97 dimension UX and authoring: acceptance and closure — 2026-09-09

Historical milestone record. For current setup and qualification, see the
[documentation index](README.md) and [release guide](RELEASE_QUALIFICATION.md).
Local artifact names below identify archived evidence; they are not current preview locations.

**M97 was accepted and closed on 2026-09-09.** Maintainer acceptance covers the
delivered behavior and recorded limits, without claiming an unrecorded exhaustive
human replay. No M97 implementation blocker or acceptance action remains.

## Accepted checkpoint

| Item | Value |
| --- | --- |
| Product source | `e26270cb89e5849092145b329d0cf95821a81b27` |
| Product tree | `de47931a170ce13d2dfd7feb2d65ddf6ba2ded81` |
| Complete clean-source gate | `20260908T235146-b387d273` |
| Artifact | 12 files, 28,761,396 bytes |
| Files SHA-256 | `ffea0a9f5f22a6e7c6fc33e070ef7c9706c1c6e1820f6df57557d0acbf4ecf93` |
| Manifest SHA-256 | `89c3612719ed45b894ccd68833f58157b4f3f528bff9d80e6d84237f99a7aaea` |
| WASM SHA-256 | `8d7a7dfedb5baa73ef6747ad8fd2cc371faae595f212dbe4b86d4c3cb5e34a44` |

The documentation-only closure preserved the exact qualified product.

## Delivered files, API and behavior

`DimensionPresentationState` in `geosolve-constraint-editor` owns relevance,
visibility, retained annotation placement and picking. The frontend adds Focused /
All / Hidden, contextual Inspector discovery, delayed hover and four personal pins.
Camera and selection changes retain annotation slots; geometry changes invalidate
affected anchors. Hidden callouts do not paint or intercept clicks.

`packages/geosolve-sketch-code/src/{authoring,presentation,managed}.ts` provides
`isKeyConstraint`, `isKeyParameter`, `dimensions.areKeyConstraintsByDefault`, named
`$.parameter` values and source-owned labels/help/document properties. Native
`managed.rs`, `managed_control.rs` and `prepared_mutation.rs` authenticate metadata,
project exact controls and prepare source edits. The workbench's
`bridge/authoring_metadata.rs` and frontend `authoring-metadata.tsx` connect the same
transactions to the Inspector. Code and GUI authoring share source, history and
restoration; stale or tampered requests cannot publish partial accepted state.

All 16 live samples own their presentation in source. Gridfinity retains all 20
authored measurements; the manifold retains six overview callouts, one shared 12 mm
channel width and a 2.4 mm seal width. Catalog selectors and implicit first-six
priorities are removed. Catalog order/category/legal/independent expectations remain
separate. Bounded V3 compatibility preserves old text; source edits upgrade through
authenticated V4 transactions. Personal view preferences remain outside design source.

No primitive, residual, Jacobian, solver priority or branch policy changed. M97-F001
and F002 fix retained annotation placement and exact unchanged-viewport geometry.
M97-F003 accepts organization-only source relabeling after independent retained
native validation. [Implementation evidence](M97_AUTHORING_IMPLEMENTATION.md),
[authoring contract](M97_AUTHORING_METADATA.md) and [goals](M97_GOALS.md) give the
full API, regressions and behavioral contract.

## Qualification and closeout

The accepted product passes **244/244 obligations** in 2249.9 seconds: nine freshly
executed stages and 235 authenticated unchanged-input successes. Coverage includes
format, strict Clippy, native/headless tests, optimized WASM, packages, documentation,
licenses, browser and performance. The 271-case golden is unchanged. Browser checks
pass 17/17 prefixes and 48/48 full workflows, including all 16 samples and all four
M97 workflows, with no failures, skips, retries or flaky results. The isolated
256-body sparse crossover passes independent validation in 137.73 seconds of test time.

The delivered artifact passed exact bytes/MIME/base-path checks for 13 HTTP routes
and actual Chromium/WASM manifold readiness with 182 geometry items and no errors.
At closure the artifact-binding audit passes again and the accepted endpoint responds.
These bounded checks preserve prior readiness evidence; they are not a new browser
workflow replay. Exact product qualification and publication commands are in
[M97_AUTHORING_IMPLEMENTATION.md](M97_AUTHORING_IMPLEMENTATION.md).

Closure commands:

```bash
python3 target/m97/audit-authoring-preview-artifact.py 20260908T235146-b387d273
curl --fail --silent --show-error --output /dev/null ${PREVIEW_URL}
nix-shell shell.nix --run './scripts/release-gate.sh --docs-only --since 151f837'
git diff --check
git status --short
```

The artifact audit and HTTP availability check pass. The documentation-only gate
passes for nine files and 20 added links in 21.76 seconds, signed receipt
`target/release-gate/docs/9cf8a96ae83244c7ab6b67e53c71f81e.json`. It checks this
closeout's prose, links and input boundary, preserving product evidence
under [RELEASE_QUALIFICATION.md](RELEASE_QUALIFICATION.md). Its output is retained at
`target/m97/milestone-closure-docs.log`. No repeated solver/browser gate or rebuild is
needed for acceptance prose. The closure commit and final status identify the clean
documentation descendant.

## Accepted limits and continuation

- Overview priority is eligibility; collision handling can omit a canvas label while
  retaining Inspector access. Generated per-instance overview overrides are deferred.
- Full patch input widths remain public Inspector controls; generated offsets retain
  contextual visibility and existing editing restrictions.
- Old unmarked source remains valid and unchanged on load, but no longer receives
  arbitrary first-six overview priorities. All measurements remain discoverable.
- Dense editing costs and the recorded R3 browser timeout remain documented. The
  final fixture workflow passes in 226 seconds with the original six-minute limit;
  this is not a general performance improvement claim.

M98 subsequently integrated the source metadata and Inspector APIs with local-folder
authoring and baked-profile export. Its current historical disposition is recorded
in [M98 qualification](M98_QUALIFICATION.md); the prototype was not part of M97's
qualified product identity.
