<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 focused UAT — Managed dimensions, Fillet hit priority and Typed Panel terminals

Status: **accepted at milestone level on 2026-08-29; the exact provisional trace-enabled candidate
is approved for clean-source qualification and public closeout**. The supervising user's explicit
“looks good, let's close the milestone” decision accepts M86-U1 through M86-U8 without claiming a
separately logged row-by-row replay. The former F002 candidate is withdrawn. Accepted M85 remains
public authority until the approved M86 descendant passes clean qualification, Pages publication
and exact hosted-byte verification.

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
- Former local endpoint process: `http://127.0.0.1:18101/`, PID `3879694`, invocation
  `c00b3e911ce54733be0b4b6a47756de1`.
- Former retained Tailscale process: `http://100.94.63.83:8080/`, PID `3879933`, invocation
  `e56fde25abab4481a9bd7c825d4279b6`.
- Both processes served the same immutable snapshot. Their complete eight-path ledgers are
  byte-identical at SHA-256
  `b5ad691e14198791fa801281724fc3186161b2d2aae7358affea402e9a5f0acd`.

This is historical F001 evidence and does not contain the Fillet-corner repair. Its services were
replaced only after the exact F002 bytes passed temporary local verification.

## Withdrawn pre-expansion F002 candidate identity

- Qualified source: `dbe94daf152515169b78a310cf2286f9ea04c80b`.
- Qualified tree: `77f86c0a198af12e10537dc4d6d7d90066ba48e8`.
- Clean Nix release gate: exit `0`, 6,570 lines and 440,856 bytes, from 13:46:55 through 14:20:45
  AEST on 2026-08-28. Log `/tmp/geosolve-m86-f002-gate.wSztT0Bb/release-gate.log` has SHA-256
  `34ac3e398953398495d22d480d9a11d88b03020c0235c07db61c443534fa4278`.
- Immutable snapshot: `/tmp/geosolve-m86-f002-uat.CPfe9QD8`; directories/files `0555`/`0444`,
  exactly seven regular files, zero symlinks/non-regular entries, ordered-manifest aggregate
  `e3f9581a05a8cbf5731b33625fa63f2b35e62f4ebcfdacb6d75a4486f80fc850`.
- Complete freeze/nomination evidence: `/tmp/geosolve-m86-f002-freeze-evidence.mSh9iwrm`.
- Historical local endpoint: `http://127.0.0.1:18101/`, PID `597410`, invocation
  `7cf7cbbae81a48ee8f492e1dab6e592d`.
- Historical retained Tailscale endpoint: `http://100.94.63.83:8080/`, PID `597412`, invocation
  `1a777ee174764f3cbb35f7b4863e5e95`.
- Temporary-local, final-local and Tailscale eight-path ledgers were identical at SHA-256
  `e5513ab3e36262f2ccedf175006f1283d5504180c8d0be46e1e90dded999a3df`.

Those bytes contain the first shared-corner F002 repair, but not the remote-parent-endpoint scope
expansion or F003. They are withdrawn from current UAT and preserved only as immutable historical
defect evidence. The historical PIDs do not establish a current service identity after reboot.

## Accepted provisional replacement identity

The original combined F002/F003 snapshot below is historical provisional evidence. After the user
reported that Copy repro was too large and requested diagnostic logging, the same source was
extended with a bounded, memory-only interaction trace and rebuilt as the exact candidate that
received final approval:

- Trace-enabled frozen snapshot: `/tmp/geosolve-m86-trace-uat.U1C0QPSf`; directory/files
  `0555`/`0444`, exactly seven regular files, zero symlinks, ordered-manifest aggregate
  `f5f429f70e42e3b39a8f22696c19ff81f358cfb10c43f7910baf386c9d82fd44`.
- `index.html`, JavaScript and WASM SHA-256 values are respectively
  `b5a36925ee1edd8af7dbbe9d4127b129184be131f84414af8e4ceac128e2111c`,
  `3bb6b395a6f053e5172063474a974dbd98a10163b45963bb736381ead6a02837` and
  `1626b3a9163f265dcaf7db0f2f0260930c9fe0ac3b9695ac749ed559e5fc435b`.
- Local and Tailscale services use PIDs `2433761`/`2433763`, invocations
  `3f829abfff0a46eba586c07fed507d8e`/`5f2c7c3eddac43119f380ec2e87b47c8`, and both report
  `WorkingDirectory=/tmp/geosolve-m86-trace-uat.U1C0QPSf`.
- `/` plus all seven files on both endpoints match the frozen snapshot byte-for-byte. Verification
  evidence is `/tmp/geosolve-m86-trace-http-verify.fXS06h`; its results SHA-256 is
  `b2de59e63fc30a2dcbef108e671b1038103983bb95fea53d7410bb5799d080f3`.
- The trace export is capped at 128 KiB, excludes managed source/reproduction/history/persistence,
  and records raw/coalesced pointer input, native terminal, code parity, publication, rollback and
  exact floating-point mismatch evidence. The post-UAT source descendant's demo-web tests pass
  316/316 and the unchanged golden `--check` passes.

This approved snapshot is still provisional UAT evidence rather than clean-source nomination. The
earlier combined candidate record follows for provenance:

- Served-build identity: saved pre-gate 160,117-byte, 2,976-line binary patch over HEAD
  `4730e156e17cf3df88b9681a22961d41b686c2ff`, tree
  `23a76c3b7141f10064d899113b97135932d23033`; patch SHA-256
  `feafcc2a717a9c1bf9ff7a708b705903b2e18c6ef67327f533d66819a8784a57`; status SHA-256
  `945ef3534016a5735c42c6fedaf72e66be2acc41ce9dc764db6e5896c3636b5a`. Saved pre/post-gate patch
  and status files are byte-identical. Subsequent documentation-only worktree edits are outside
  this served-build patch and do not alter the frozen seven-file candidate.
- Complete dirty-worktree gate: pipeline `0 0`, 2026-08-28 18:44:26–19:10:14 AEST; log
  `/tmp/geosolve-m86-f002-f003-gate.yDrJlI8n/release-gate.log`, 6,582 lines, 441,920 bytes,
  SHA-256 `93b645c2a2f1850f589b406943f3618da4fc833a42ff6066884602ec3e31ddb6`.
- Exact no-rebuild frozen snapshot: `/tmp/geosolve-m86-f002-f003-uat.yGY3Nvly`; directory/files
  `0555`/`0444`, seven top-level regular files, zero symlinks/nested entries, ordered-manifest
  aggregate `8f5a4ffcd96819b986ba81a9467d0c83a64365b2d21338cd134e164fa4444ce4`.
- Complete evidence: `/tmp/geosolve-m86-f002-f003-freeze-evidence.EsMzxE2v`. Temporary, local and
  Tailscale eight-path ledgers are byte-identical at SHA-256
  `dca3e6eeba66e12c873ba4b5ba9b6cadd489060f0e4c7d5ce1070ed3344ec96f`.
- Historical combined-candidate local endpoint: `http://127.0.0.1:18101/`, PID/invocation
  `969297`/`c1681e5beb234ce487dbf9b639cbd9dd`.
- Historical combined-candidate UAT endpoint: `http://100.94.63.83:8080/`, PID/invocation
  `973390`/`c77a5b3abbe94752b864af9bda53c355`.

The candidate first passed all eight paths on isolated `127.0.0.1:18102`, temporary PID/invocation
`965128`/`a06c89f580744568b0d39677ee776da1`; only then were the local and Tailscale listeners
replaced. The temporary service is stopped. Those retained services served only the combined
frozen snapshot before the trace-enabled services above replaced them; the withdrawn snapshot
remains intact as rollback evidence.

## Candidate prerequisites

- Exact M86-F001 owner and thin Inspector-adapter regressions pass.
- Valid target edit, retained-invalid rejection and Undo, accepted diameter `5 -> 8`, exact
  Undo/Redo, immediate stable-alias reselection and one outer-history semantics pass with
  independently validated finite accepted geometry. History tests prove alias availability rather
  than durable UI selection.
- Expanded F002 native and WASM parity pass 18/18, including both remote endpoints, compact-grip
  precedence, Coincident/non-topology, nearer-point, cross-Fillet and post-Apply behavior. A focused
  unit regression also proves broad arc-only hover performs no document-wide Coincident traversal.
- Focused F003 owner/boundary and Compass Rose collateral tests pass. Three Typed Panel targets keep
  exact terminal/publication/reload coordinates, two canonical drafts, one revision, finite Current
  Fillets and independently validated Hard residual `<= 1e-9`. Negative guards retain causal
  source-curve scope, the 8-ULP bound and exact public discrete Fillet state. The automated targets
  run sequentially in one retained session; U8 supplies the human pointer-feel check.
- Warnings-denied affected-crate Clippy and unchanged 271-row golden evidence pass.
- A fresh complete demo-web run passes 307/307 library tests plus its binary, integration and
  doc-test targets. Full editor qualification passes 430 library tests plus all integration/doc
  targets.
- The complete provisional workspace/release gate, no-rebuild freeze and exact temporary/local/
  Tailscale byte verification pass. A clean committed-source gate remains required before Pages;
  no publication occurs before explicit approval.

The human scorecard was accepted against the live immutable trace-enabled replacement; browser
cache and local storage could otherwise preserve an earlier candidate. The historical target
viewports were approximately `1440x900` and `1024x720`.

## Scorecard

| ID | Human action | Pass condition | Status |
|---|---|---|---|
| M86-U1 | Open PC Water Manifold, select dimension alias `code.dimension.2cabcaba35f1866930e2549cbd95d899abeb2656e495bf047909f1d92176218b`, and change its Inspector value from `16` to `8`. | The source declaration `topScrewRail3Length` visibly changes only from `target: mm(16)` to `target: mm(8)`; geometry/annotation update to `8`, selection stays on the same semantic dimension, no global provenance error appears and one outer history entry is added. | accepted by scoped F001 approval |
| M86-U2 | Edit a second managed curve-length dimension and one managed screw diameter through Inspector. | Both direct supported families update their own managed `target`, remain finite/current and do not disturb unrelated declarations or geometry. | accepted by scoped F001 approval |
| M86-U3 | Enter an invalid direct target such as `0`, inspect the result, then recover through Code source correction or Undo. | The failed managed value/error remains available while the last accepted complete manifold stays visible; no blank/partial scene or stale global error appears, and the existing code-session recovery path restores or publishes normally. | accepted by scoped F001 approval |
| M86-U4 | After a valid edit, exercise Undo, Redo, reload and Copy/Load repro. | Exact prior/edited source and native values round-trip as one outer step; project identity, semantic alias availability and accepted geometry remain coherent. Selection need only be restored where the ordinary history/reload UX already promises it. | accepted by scoped F001 approval |
| M86-U5 | Create or open an ordinary GUI-owned dimension and edit it, then spot-check an unrelated code-owned point drag and source Apply. | GUI dimensions retain their existing Inspector behavior; point overlays and ordinary managed source editing are unchanged, with no duplicate history or delayed mutation. | accepted by scoped F001 approval |
| M86-U6 | Draw one open right-angle Polyline with two length-2 legs. Repeat the reported radius-2 workflow; because exact `2` is the tangent-at-endpoint fold boundary, use accepted radius `1.99` for deterministic verification. Select the Fillet and hover/click/drag each of the two remote parent endpoints beneath its broad visible surface. | The middle of either endpoint marker receives Point hover, selects that persistent Point and begins an ordinary Point drag. The result remains stable after deselect/reselect and after Apply. | accepted by final milestone approval |
| M86-U7 | On the same scene, use the compact radius grip, then sample the spoke/rail/arc away from a parent endpoint. Also recheck the shared corner and, optionally, an unrelated overlapping point. | The compact grip remains the most specific radius control. Parent endpoints beat the broad surface; the broad surface still selects/drags the Fillet away from them and stays above unrelated points and passive curves. No hover/down disagreement appears. | accepted by final milestone approval |
| M86-U8 | Open **Typed Panel · keyed Fillets** and repeatedly drag its upper-left corner away from `[0, 40]`, including motions corresponding to `[3, 38]`, `[5, 37]` and `[-2, 36]` where practical. Release, wait, interact again and reload. | Each accepted release remains at the last preview instead of returning to the pre-drag location. Both keyed Fillets remain visible/current, no terminal-authority error appears, and reload preserves the terminal. | accepted by final milestone approval |

The scorecard records milestone-level approval and does not invent a separately logged row-by-row
replay. U1-U5 retain the earlier scoped F001 assessment; the 2026-08-29 final close decision accepts
the replacement U6-U8 scope and the trace-enabled descendant.

## Approval and publication

Explicit final supervising-user approval now passes and the accepted product source is committed.
Clean qualification, Pages publication, downloaded-artifact/hosted-path verification and retained-
service retirement remain the mechanical closeout steps.
