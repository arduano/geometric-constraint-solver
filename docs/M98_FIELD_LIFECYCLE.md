<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 field lifecycle review

Owner: first-party React authoring fields and folder transport. No Rust solver,
accepted-geometry contract, residual or golden oracle changes.

Review at storage checkpoint `771a5f7` reproduced two presentation lifecycle defects:

- Focusing a dimension input dispatched `dimensions.focus` before the user's first
  keystroke. The folder bridge advances selection authority asynchronously. Typing
  before that response captured the previous revision, so the ensuing field commit
  could correctly reject as stale despite no external edit. The exact component test
  initially observed one unexpected focus command; the adapter characterization also
  recorded observed revision 2 with pending edit revision 1.
- A dimension submitted as 14 and then changed locally to 16 lost the newer input when
  the delayed accepted value 14 arrived. Its unconditional value effect cancelled the
  edit and replaced the visible text. The exact test initially observed 14 instead of 16.

`useAuthoringField` in `frontend/src/lib/authoring-edit.ts` now owns shared draft versions,
submission deduplication, accepted-value reconciliation and cancellation. Dimensions,
parameters and metadata fields, including multiline descriptions, consume it. A delayed
echo can acknowledge the submitted draft but cannot clear later or unsubmitted input.
Explicit cancellation restores the latest accepted value. Changing a draft again allows
a new submission even when its text matches an earlier rejected attempt.

Folder dimension input focus starts field ownership without changing selection; the
explicit Inspect button still selects the measurement. Ordinary demo input focus retains
its accepted inspection behavior. No background fetch or component render grants newer
source authority; the folder adapter remains responsible for installation and draft bases.

Focused development evidence on 2026-09-09:

- Before repair, `npx vitest run src/components/dimension-inspector.test.tsx`:
  two exact lifecycle regressions failed, nine collateral tests passed.
- After repair, from `crates/geosolve-demo-web/frontend`,
  `npx vitest run src/lib/authoring-edit.test.tsx src/components/dimension-inspector.test.tsx src/components/authoring-metadata.test.tsx`:
  **26/26 passed**, no skips.
- `npx tsc -b`: passed.
- `git diff --check`: passed.

Independent integration findings remain assigned to the bridge/adapter owner:

- Explicit refresh clears field bases while unchanged underlying values can leave
  component draft text visible. The new `folder-lifecycle-review.test.ts` initially
  failed because a surviving draft used `disk-after` instead of `disk-before`.
- An actual-WASM secondary tab's 600×400 resize returned HTTP 409 because it was read
  only. A viewer must not resize the shared active editor projection; the integration
  should return an appropriate viewer projection or no-op without a spurious edit error.
- Current public `compileManagedSource` and actual bridge normalization drop a leading
  SPDX comment from `examples/file-workspace/sketch.ts`. `printManagedSource` currently
  begins at the managed directive. The managed compiler owner must preserve the file
  preamble without weakening authenticated reversible-source semantics.

These findings and broader M98 integration are not claimed resolved or release-qualified
by this component change. Transient coordination details and repro commands are retained
in `target/m98/coordination/bridge-review-findings.md` while implementation proceeds.
