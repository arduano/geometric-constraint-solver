<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 focused UAT — Managed dimension Inspector editing

Status: **clean-qualified immutable candidate nominated; M86-U1 through M86-U5 are pending**.
Accepted M85 remains public authority until explicit approval and post-approval Pages publication.

## Candidate identity

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

All candidate prerequisites above pass. Hard-refresh the reused Tailscale origin before testing;
browser cache/local storage may survive earlier milestone candidates.

Test the exact immutable candidate at approximately `1440x900` and `1024x720`.

## Pending scorecard

| ID | Human action | Pass condition | Status |
|---|---|---|---|
| M86-U1 | Open PC Water Manifold, select dimension alias `code.dimension.2cabcaba35f1866930e2549cbd95d899abeb2656e495bf047909f1d92176218b`, and change its Inspector value from `16` to `8`. | The source declaration `topScrewRail3Length` visibly changes only from `target: mm(16)` to `target: mm(8)`; geometry/annotation update to `8`, selection stays on the same semantic dimension, no global provenance error appears and one outer history entry is added. | pending |
| M86-U2 | Edit a second managed curve-length dimension and one managed screw diameter through Inspector. | Both direct supported families update their own managed `target`, remain finite/current and do not disturb unrelated declarations or geometry. | pending |
| M86-U3 | Enter an invalid direct target such as `0`, inspect the result, then recover through Code source correction or Undo. | The failed managed value/error remains available while the last accepted complete manifold stays visible; no blank/partial scene or stale global error appears, and the existing code-session recovery path restores or publishes normally. | pending |
| M86-U4 | After a valid edit, exercise Undo, Redo, reload and Copy/Load repro. | Exact prior/edited source and native values round-trip as one outer step; project identity, semantic alias availability and accepted geometry remain coherent. Selection need only be restored where the ordinary history/reload UX already promises it. | pending |
| M86-U5 | Create or open an ordinary GUI-owned dimension and edit it, then spot-check an unrelated code-owned point drag and source Apply. | GUI dimensions retain their existing Inspector behavior; point overlays and ordinary managed source editing are unchanged, with no duplicate history or delayed mutation. | pending |

## Approval and publication

Explicit supervising-user approval is required. After approval, record the scorecard, publish the
accepted descendant through GitHub Pages, download and exact-verify the separately rebuilt artifact
and hosted paths, retire retained candidate services and close M86. Until then M86 remains open.
