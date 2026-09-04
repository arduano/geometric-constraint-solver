<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M91 goals: cohesive code-driven authoring

## Outcome

Status: **Accepted at milestone level by explicit supervising-user approval on 2026-09-04; public
GitHub Pages closeout remains pending.**

The implementation is mechanically qualified and its immutable candidate is published on the
Tailscale-only UAT endpoint below. The supervising user's blanket approval of every part of the UAT
accepts M91-U1 through M91-U14 as Pass without claiming a separately logged row-by-row replay.

M91 combines five independently owned improvements into one code-authoritative workbench and one
composite UAT. It makes authored contact-range changes behave like intentional design edits, adds
TypeScript language intelligence, converts the complete visible sample catalog to managed source,
adds non-destructive Explorer visibility, and makes native/managed semantic parity a release oracle.
M90-U1 through M90-U10 transfer unchanged into this milestone.

## Required product contract

- A contact's intrinsic parameter topology comes from its referenced curve. Source may select a
  line's unbounded supporting-line topology and may independently author an inclusive admissible
  range. Winding, locality, orientation and active bounds remain explicit.
- A structurally changed source candidate may use the prior authenticated retained/accepted pair
  only as numerical continuation. Candidate source remains the sole authority for topology,
  branches, range, host inputs and publication.
- The code editor provides syntax and semantic diagnostics, completion, hover and signature help
  against the exact pinned managed-sketch declarations. Analysis runs off the interaction path and
  never publishes or mutates sketch authority.
- All 37 visible samples open as code projects whose `sketch.ts` is sole authoring authority. Native
  constructors remain internal reference/oracle fixtures, not a second user-facing sample mode.
- Every sample cold-materializes to finite independently validated geometry with normalized Hard
  residual at most `1e-9`. Declared DOF and intended mechanism mobility remain truthful; Scotch
  Yoke, Scissor Jack and Five-stage Scissor Tower each have a deterministic intended drag.
- Explorer eyes compose item, group and global-filter visibility without changing source,
  suppression, solver participation or history. Group isolate/restore and a pinned Construction
  filter retain child choices and survive workspace persistence/reprojection.
- Every applicable golden authoring, lifecycle, Fillet and accepted-scene row has an actual managed
  execution counterpart. Parity compares semantic geometry and constraints, explicit branch/contact
  state, dimensions, operations, rank/DOF, independent residual validation, lifecycle and accepted
  scene authority—not backend IDs or serialized bytes.
- A reviewed exclusion ledger names genuinely host-external, compiler/source-only or unsupported
  cross-backend rows. Exclusion never counts as parity.

## Release contract

Closure requires locked all-feature Rust tests, formatting, warnings-denied Clippy, package and
frontend checks, fixture/declaration drift checks, dual-backend golden `--survey`, `--check` and
`--require-clean`, optimized release WASM, distribution validation and the complete clean-source
release gate. The nominated artifact must be an immutable byte-verified Tailscale-only snapshot.
Automated evidence does not accept a human UAT row. After explicit human acceptance, the approved
descendant must publish through GitHub Pages, pass artifact and hosted-byte verification, and only
then retire the accepted UAT services; those public closeout steps are still pending.

## Implemented workstreams

The integrated implementation now contains all five frozen workstreams:

1. intrinsic contact topology, optional authored admissible ranges, retained numerical continuation
   and specific infeasible-range diagnostics/history;
2. TypeScript 5.9.2 Worker diagnostics, completion, hover and signature help against generated
   pinned declarations, including stale-query, disposal and UTF-8 project-bound recovery;
3. one 37-entry source-authoritative sample catalog, including deterministic mechanism drags,
   native-reference semantic checks and release-WASM frame/source checks;
4. persistent Explorer row/group visibility, isolate/restore and a global Construction filter that
   affect presentation and picking without changing source, solve state, suppression or history;
5. a dual-backend semantic oracle for applicable authoring, lifecycle, computed-Fillet and
   accepted-scene rows, with a reviewed fail-closed exclusion ledger.

The implementation also retains the M90 clean-break guarantees: existing-point drags do not compile
managed source; generated declaration/source mutations remain atomic; and the exact M90-F005
workspace still restores through authenticated historical migration. At nomination, M90-U1 through
M90-U10 remained transferred, unexecuted and neither passed nor waived. They are now accepted as
M91-U1 through M91-U10 by composite M91 approval, without retroactively changing their historical
M90 disposition.

## Reviewed parity exclusions

The exclusion ledger contains exactly four boundaries. An exclusion is neither a pass nor a parity
substitute, and computed Fillet is never excluded:

- `constraint.external-line-collinear.*` — immutable external-line host snapshot/binding is absent;
- `constraint.external-point-coincident.*` — immutable external-point host snapshot/binding is
  absent;
- `dimension.profile-offset.*` — a flattened `SketchDocument` cannot truthfully recover Profile
  Offset's aggregate-helper/root source closure;
- `spline.noncanonical-knot-topology.*` — the typed recipe cannot reproduce knot-inserted native
  spline topology without changing authored control structure.

## Nomination boundary

`M91-F001` is resolved by `d2170c46775b412785b3a2f288c170aba9ac8155`. The pre-repair optimized
WASM was `27,296,927` bytes and the distribution was `36,085,444` bytes, above the strict
`< 20 MiB` and `< 30 MiB` limits. The pure-Rust repair deterministically zlib-compresses only the 37
bundled compiler envelopes and lazily reconstructs their exact authenticated UTF-8 bytes. All
byte-identity, declared-length, complete-input, checksum/status, wire-bound and invalid-UTF-8
regressions pass. The nominated WASM is `18,368,160` bytes and the distribution is `27,158,025`
bytes.

`M91-F002` is resolved by `6d0155151133ba2540fd1dc4b2b071f141b86064`. The first post-F001 clean
gate reached package verification and correctly failed because the package verifier did not patch
the direct unpublished `geosolve-sketch-features` dependency. The repair patches all four direct
local dependencies and adds a fail-closed manifest/list drift guard; its focused verifier and the
complete final gate pass. This was packaging infrastructure, not a solver defect.

Nomination evidence:

- source commit/tree: `6d0155151133ba2540fd1dc4b2b071f141b86064`,
  `972ad507c2cdfad2c9cd664e49feaf79ae381c81`;
- clean release log: `/home/arduano/m91-gate.t8TTq0Bj/release-gate.log`, SHA-256
  `cc4f4580a0637cfddad5d96d7510d4a5f4dc707d99010b300f1a43451a7cc8cd`;
- frozen snapshot/manifest: `/tmp/geosolve-m91-uat.17Q5LnSg`,
  `/tmp/geosolve-m91-uat.17Q5LnSg.sha256`, manifest SHA-256
  `b1e95b608b465a545791e55cc762052704f2d7139b8e4c3a9f8a68b0411a009b`;
- frozen WASM: `/tmp/geosolve-m91-uat.17Q5LnSg/assets/geosolve_demo_web_bg-tvc8MGYX.wasm`,
  `18,368,160` bytes, SHA-256
  `6832d1b6fd984076a47440ccac82ece0dfd205a9e93346dfb3cbd6783240e961`;
- identical staging/live HTTP ledger SHA-256:
  `35531210b63479565e4350b44593ebe62d228e756e67378f829c99399e86bab4`;
- Tailscale service `geosolve-m91-uat-18091.service`, PID `2142854`, invocation
  `bf93a3f5dab84809a24fdc2db6f23f4f`, URL `http://100.94.63.83:18091/`.

The frozen candidate passed 20/20 Chromium checks on both staging and live endpoints. M90 remained
byte-identical and was not restarted. At nomination, no public deployment or GitHub Pages
publication had occurred, and automated evidence accepted none of the 14 human UAT rows. On
2026-09-04 the supervising user separately accepted M91-U1 through M91-U14 by blanket/composite
approval, without claiming a separately logged row-by-row replay. Public Pages closeout remains
pending.
