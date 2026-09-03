<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M91 goals: cohesive code-driven authoring

## Outcome

Status: **Implementation complete; nomination evidence pending.** No candidate is nominated and no
human UAT row has run.

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
Automated evidence does not accept a human UAT row and public deployment is outside this milestone.

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
managed source; generated declaration/source mutations remain atomic; the exact M90-F005 workspace
still restores through authenticated historical migration; and M90-U1 through M90-U10 remain
transferred, unexecuted and neither passed nor waived.

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

`M91-F001` remains a pending release-bundle repair. The pre-repair optimized WASM is `27,296,927`
bytes against the strict `< 20 MiB` limit and the distribution is `36,085,444` bytes against the
strict `< 30 MiB` limit because the 37 raw compiler envelopes contribute about `10.96 MB` of
duplicate embedded JSON. The proposed pure-Rust repair deterministically zlib-compresses only the
bundled compiler envelopes at build time and lazily reconstructs their exact authenticated UTF-8
bytes under the existing managed-wire ceiling. It must land and pass focused corruption/bounds and
byte-identity regressions before final qualification begins.

Nomination evidence is deliberately unresolved in this drafting commit:

- final source: `@M91_FINAL_COMMIT@`;
- final tree: `@M91_FINAL_TREE@`;
- clean release log: `@M91_RELEASE_LOG@`;
- clean release log SHA-256: `@M91_RELEASE_LOG_SHA@`;
- frozen snapshot: `@M91_SNAPSHOT@`;
- external manifest: `@M91_MANIFEST@`;
- ordered snapshot aggregate: `@M91_SNAPSHOT_SHA@`;
- release WASM path/bytes/SHA-256: `@M91_WASM_ARTIFACT@`, `@M91_WASM_BYTES@`,
  `@M91_WASM_SHA@`;
- staging/live HTTP ledger SHA-256: `@M91_HTTP_LEDGER_SHA@`;
- Tailscale service/PID/invocation/URL: `@M91_SERVICE@`, `@M91_SERVICE_PID@`,
  `@M91_SERVICE_INVOCATION@`, `@M91_UAT_URL@`.

Do not change this document to “Candidate nominated; awaiting composite human UAT” until every
placeholder above is replaced with verified evidence from one clean committed source and its
unchanged immutable served bytes.
