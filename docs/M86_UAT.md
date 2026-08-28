<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 focused UAT — Managed dimensions and Fillet source-corner picking

Status: **M86-U1 through M86-U5 are accepted by the supervising user's “Looks good” F001
assessment; an F002 replacement candidate and M86-U6/U7 remain pending**. Accepted M85 remains
public authority until explicit final M86 approval and post-approval Pages publication.

## Superseded F001 candidate identity

- Qualified source: `90504245e19858f986d5f506f6e42d237e9665b5`.
- Qualified tree: `65e092540dab82618d1129229b566a2e791aa40c`.
- Clean Nix release gate: exit `0`, 6,534 lines and 436,494 bytes, from 12:04:44 through 12:24:38
  AEST on 2026-08-28. Log `/tmp/geosolve-m86-nix-gate.CSQkgk/release-gate.log` has SHA-256
  `4b81a1d12df503ef220b10645f3f4266891edc1eeaf87f3647071e2a77c33584`.
- Immutable snapshot: `/tmp/geosolve-m86-uat.vdEFAxsF`; directories/files `0555`/`0444`, exactly
  seven regular files, zero symlinks/non-regular entries, ordered-manifest aggregate
  `1f872c6b51317ff810b48ab8965e1e0a0f6cb45feb01f5cbfe654eafbedd5882`.
- Complete freeze/nomination evidence: `/tmp/geosolve-m86-freeze-evidence.6fU7WpCl`.
- Local endpoint: `http://127.0.0.1:18101/`, PID `3879694`, invocation
  `c00b3e911ce54733be0b4b6a47756de1`.
- Retained Tailscale endpoint: `http://100.94.63.83:8080/`, PID `3879933`, invocation
  `e56fde25abab4481a9bd7c825d4279b6`.
- Both endpoints serve the same immutable snapshot. Their complete eight-path ledgers are
  byte-identical at SHA-256
  `b5ad691e14198791fa801281724fc3186161b2d2aae7358affea402e9a5f0acd`.

This candidate remains live only until the F002 replacement passes clean qualification and exact
temporary verification. It is historical F001 evidence and does not contain the Fillet-corner
repair.

## Candidate prerequisites

- Exact M86-F001 owner and thin Inspector-adapter regressions pass.
- Valid target edit, retained-invalid rejection and Undo, accepted diameter `5 -> 8`, exact
  Undo/Redo, immediate stable-alias reselection and one outer-history semantics pass with
  independently validated finite accepted geometry. History tests prove alias availability rather
  than durable UI selection.
- Relevant native/WASM tests, formatting, warnings-denied Clippy, unchanged clean golden and the
  complete clean release gate pass.
- The exact gate output is frozen without rebuild and exact-verified on temporary local and
  retained Tailscale endpoints. No Pages publication occurs before explicit approval.

All F001 candidate prerequisites above pass. The F002 replacement must additionally pass exact
native/WASM source-corner, Coincident/non-topology and post-Apply regressions before it replaces
these bytes. Hard-refresh the reused Tailscale origin before testing; browser cache/local storage
may survive earlier milestone candidates.

Test the exact immutable candidate at approximately `1440x900` and `1024x720`.

## Scorecard

| ID | Human action | Pass condition | Status |
|---|---|---|---|
| M86-U1 | Open PC Water Manifold, select dimension alias `code.dimension.2cabcaba35f1866930e2549cbd95d899abeb2656e495bf047909f1d92176218b`, and change its Inspector value from `16` to `8`. | The source declaration `topScrewRail3Length` visibly changes only from `target: mm(16)` to `target: mm(8)`; geometry/annotation update to `8`, selection stays on the same semantic dimension, no global provenance error appears and one outer history entry is added. | accepted by scoped F001 approval |
| M86-U2 | Edit a second managed curve-length dimension and one managed screw diameter through Inspector. | Both direct supported families update their own managed `target`, remain finite/current and do not disturb unrelated declarations or geometry. | accepted by scoped F001 approval |
| M86-U3 | Enter an invalid direct target such as `0`, inspect the result, then recover through Code source correction or Undo. | The failed managed value/error remains available while the last accepted complete manifold stays visible; no blank/partial scene or stale global error appears, and the existing code-session recovery path restores or publishes normally. | accepted by scoped F001 approval |
| M86-U4 | After a valid edit, exercise Undo, Redo, reload and Copy/Load repro. | Exact prior/edited source and native values round-trip as one outer step; project identity, semantic alias availability and accepted geometry remain coherent. Selection need only be restored where the ordinary history/reload UX already promises it. | accepted by scoped F001 approval |
| M86-U5 | Create or open an ordinary GUI-owned dimension and edit it, then spot-check an unrelated code-owned point drag and source Apply. | GUI dimensions retain their existing Inspector behavior; point overlays and ordinary managed source editing are unchanged, with no duplicate history or delayed mutation. | accepted by scoped F001 approval |
| M86-U6 | Draw two joined lines or a rectangle, apply a computed Fillet, select the Fillet so its radius affordance is visible, then hover and click the original native corner beneath the rail/arc hitbox. | The native corner gets the point hover state, clicking selects the point rather than the Fillet, and a drag begins as an ordinary point drag. The result is the same before and after deselect/reselect and after Apply. | pending F002 candidate |
| M86-U7 | On the same scene, drag the Fillet radius from its grip/rail and click the computed arc away from the native corner; optionally place an unrelated point at a Fillet contact and try it. | Radius drag and computed Fillet selection remain available away from the exact source corner; an unrelated overlapping point does not globally steal the Fillet surface, and passive native curves remain below it. | pending F002 candidate |

The F001 statuses record the supervising user's milestone-level “Looks good” assessment; they do
not claim a separately logged row-by-row replay. F002 has its own focused U6/U7 recheck because it
supersedes the nominated bytes.

## Approval and publication

Explicit final supervising-user approval after F002 is required. After approval, record the scorecard, publish the
accepted descendant through GitHub Pages, download and exact-verify the separately rebuilt artifact
and hosted paths, retire retained candidate services and close M86. Until then M86 remains open.
