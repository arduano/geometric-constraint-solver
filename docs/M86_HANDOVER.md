<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 closeout handover

Status: **M86 is accepted at milestone level on 2026-08-29; mechanical public closeout remains in
progress**. M86-F001-F003 and the bounded interaction trace are approved by the supervising user's
explicit close decision. The former F002 nomination remains withdrawn. Accepted source `88d1b5e`
is clean-qualified and its no-rebuild output is frozen and isolated-HTTP verified; Pages
verification and service retirement remain open. Do not reconstruct this state from chat history.

## Resume point

Read `AGENTS.md` and its required project documents, then `docs/M86_GOALS.md`,
`docs/M86_IMPLEMENTATION.md` and `docs/M86_UAT.md`. Accepted product commits are `b1e7ea2`
(expanded F002), `2cfee5b` (F003), and `49787f4` (bounded trace/UI/tests). Accepted-state commit
`88d1b5e`, tree `09018e5`, passes the complete clean gate and owns the frozen nomination below.
The supervising user's 2026-08-29 close decision authorizes publication. Accepted M85 Pages
remains public-byte authority until the M86 hosted-byte proof passes.

## Delivered work

- **M86-F001 — managed dimension Inspector edits:** commit `9050424` routes authenticated direct
  code-owned curve-length/diameter target edits through managed TypeScript source rewriting and
  atomic rematerialization. Invalid edits retain the last accepted scene and ordinary Undo/recovery.
  The supervising user's “Looks good” assessment accepts this finding.
- **M86-F002 — Fillet Select specificity:** historical commit `dbe94da` repaired the shared parent
  corner, but UAT showed the same broad Fillet surface also hid the remote endpoints of a two-leg,
  length-2 right-angle Polyline near radius `2`. The current expansion keeps the compact radius grip
  first, then any visible persistent endpoint owned by either parent, then the broad Fillet
  arc/spoke/rail, then ordinary geometry. Shared/Coincident topology stays deterministic; nearer
  opposite-parent point wins and an exact distance tie stays with the Fillet. Unrelated points remain
  below the broad surface. Hover and down share the resolver. Coincident representatives are held
  in one request-local lazy cell: broad-surface-only motion does no document-wide topology work,
  while overlapping owners share at most one traversal after an endpoint halo hits.
- **M86-F003 — Typed Panel terminal snap-back:** reproduced upper-left drag `[0,40] -> [3,38]` as
  valid native preview/release followed by retained publication rejection in computed features.
  This is an M84-F010 scope recurrence, not solver snapping. The current repair permits bounded
  public derived scalar parity only when design and accepted documents normalize the same nonempty
  redundant rectangle-alias set, and only for public edges/fragments causally sourced by curves
  referencing those points. Unrelated geometry, values beyond 8 ULP, public sweep, tangent
  orientation, winding, provenance, topology/evaluation mapping, identities, ownership and
  allocator high-water remain exact; refreshable revision/digest/lifecycle stamps stay excluded.

F003 compares the public computed DTO used by this adapter. Private continuation certificates,
including transverse-orientation metadata, are outside that DTO and are not implicated by the
Current Typed Panel case. None of F001-F003 changes solver equations, residuals, Jacobians, branch
semantics, persistence formats or public APIs.

## Withdrawn historical candidate

- Product source/tree: `dbe94daf152515169b78a310cf2286f9ea04c80b` /
  `77f86c0a198af12e10537dc4d6d7d90066ba48e8`.
- Complete clean release gate: exit `0`; log
  `/tmp/geosolve-m86-f002-gate.wSztT0Bb/release-gate.log`, SHA-256
  `34ac3e398953398495d22d480d9a11d88b03020c0235c07db61c443534fa4278`.
- Frozen seven-file snapshot: `/tmp/geosolve-m86-f002-uat.CPfe9QD8`, aggregate
  `e3f9581a05a8cbf5731b33625fa63f2b35e62f4ebcfdacb6d75a4486f80fc850`.
- Historical local: `http://127.0.0.1:18101/`, PID `597410`.
- Historical Tailscale: `http://100.94.63.83:8080/`, PID `597412`.
- Both endpoints served the frozen snapshot at nomination; their exact eight-path ledger SHA-256 was
  `e5513ab3e36262f2ccedf175006f1283d5504180c8d0be46e1e90dded999a3df`.

The snapshot predates remote-parent-endpoint behavior and F003. It is withdrawn from current UAT
and remains immutable historical defect evidence. The current services below no longer serve it;
the snapshot remains intact as rollback evidence.

## Historical provisional combined candidate

- Served-build identity: saved pre-gate 160,117-byte, 2,976-line binary patch over HEAD
  `4730e156e17cf3df88b9681a22961d41b686c2ff`, tree
  `23a76c3b7141f10064d899113b97135932d23033`; patch SHA-256
  `feafcc2a717a9c1bf9ff7a708b705903b2e18c6ef67327f533d66819a8784a57`; status SHA-256
  `945ef3534016a5735c42c6fedaf72e66be2acc41ce9dc764db6e5896c3636b5a`. Saved pre/post-gate patch
  and status files are byte-identical. Subsequent documentation-only worktree edits are outside
  that served-build patch and do not alter the frozen seven-file candidate.
- Complete provisional gate: `GEOSOLVE_ALLOW_DIRTY=1`, pipeline `0 0`, 2026-08-28 18:44:26 through
  19:10:14 AEST. The 6,582-line, 441,920-byte log
  `/tmp/geosolve-m86-f002-f003-gate.yDrJlI8n/release-gate.log` has SHA-256
  `93b645c2a2f1850f589b406943f3618da4fc833a42ff6066884602ec3e31ddb6`. Pre/post patch and status
  bytes compare exactly. This is comprehensive dirty-worktree evidence, not clean-source
  nomination.
- Exact no-rebuild snapshot: `/tmp/geosolve-m86-f002-f003-uat.yGY3Nvly`, exactly seven top-level
  regular files, zero symlinks/nested entries, directory/files `0555`/`0444`, ordered-manifest
  aggregate `8f5a4ffcd96819b986ba81a9467d0c83a64365b2d21338cd134e164fa4444ce4`.
- Complete freeze/service evidence:
  `/tmp/geosolve-m86-f002-f003-freeze-evidence.EsMzxE2v`. Temporary, local and Tailscale eight-path
  ledgers are byte-identical at SHA-256
  `dca3e6eeba66e12c873ba4b5ba9b6cadd489060f0e4c7d5ce1070ed3344ec96f`.
- Historical local service: `http://127.0.0.1:18101/`, PID `969297`, invocation
  `c1681e5beb234ce487dbf9b639cbd9dd`.
- Historical Tailscale UAT: `http://100.94.63.83:8080/`, PID `973390`, invocation
  `c77a5b3abbe94752b864af9bda53c355`.
- Temporary verifier `127.0.0.1:18102`, PID `965128`, invocation
  `a06c89f580744568b0d39677ee776da1`, passed before either listener was replaced and is stopped.
  Those retained services pointed only at the combined frozen snapshot and were subsequently
  replaced by the trace-enabled services below.

## Accepted trace-enabled candidate

The user requested bounded gesture logging after the complete reproduction payload became too
large to paste. The resulting memory-only `Copy trace` surface records exact pointer, semantic
route, parity, publication and rollback evidence without entering source, history, persistence or
reproduction payloads. Its hard export bound is 128 KiB.

- Frozen snapshot: `/tmp/geosolve-m86-trace-uat.U1C0QPSf`, exactly seven regular files, zero
  symlinks, directory/files `0555`/`0444`, ordered-manifest aggregate
  `f5f429f70e42e3b39a8f22696c19ff81f358cfb10c43f7910baf386c9d82fd44`.
- Local service PID/invocation: `2433761`/`3f829abfff0a46eba586c07fed507d8e` on
  `127.0.0.1:18101`; Tailscale service PID/invocation:
  `2433763`/`5f2c7c3eddac43119f380ec2e87b47c8` on `100.94.63.83:8080`.
- `/` and all seven files match the snapshot on both endpoints. Evidence
  `/tmp/geosolve-m86-trace-http-verify.fXS06h` has results SHA-256
  `b2de59e63fc30a2dcbef108e671b1038103983bb95fea53d7410bb5799d080f3`.
- Post-trace full demo-web passes 316/316; focused trace, Clippy, formatting, WASM, release build,
  browser-smoke and unchanged-golden checks pass. The temporary `18103` listener is retired.

The supervising user's “looks good, let's close the milestone” decision accepts M86-U1-U8 and this
trace-enabled descendant without claiming a separately logged row-by-row replay.

## Clean committed-source nomination

- Source/tree: `88d1b5e06a7ce8ffe38931f792492f6f837a1d74` /
  `09018e5aeb7e824396ae2ee2c70a3e30912414fa`.
- Complete clean Nix release gate: pipeline `0 0`, 2026-08-29 13:16:02–13:35:50 AEST; identical
  empty pre/post status. Log `/tmp/geosolve-m86-clean-gate.w0UKa8fu/release-gate.log` is 6,573
  lines, 438,432 bytes, SHA-256
  `e3adef1b33f1b840d9bc44ea7e30a5c76248766187d705eb1bdeb682cfc3bad0`.
- Exact no-rebuild snapshot: `/tmp/geosolve-m86-clean-uat.d7DF9hcM`, exactly seven top-level regular
  files, zero symlinks/nested entries, directory/files `0555`/`0444`, ordered-manifest aggregate
  `d9d88bfb8ac4acd3f8d45f2cbbc965694297d76b61192be12c4fe65b9deb557e`.
- Complete freeze/HTTP evidence: `/tmp/geosolve-m86-clean-freeze-evidence.MDl33z2L`. Temporary
  `127.0.0.1:18104`, PID/invocation `3502269`/`c3a6eeec322d454ab97b246fbd67e8e1`, exact-verifies `/`
  and all seven files; results SHA-256 is
  `cda649921ad08469b50642f23afda334c3fff821848a8365718e0713ca9f909b`.
- The isolated verifier is inactive/dead with `MainPID=0`; curl exits `7` with HTTP `000`. The
  accepted trace UAT services remain untouched until exact Pages verification passes.

## Current checkpoint evidence

- `m75_hover_pointer_parity` passes 18/18 on native and 18/18 on WASM.
- `broad_fillet_hover_defers_coincidence_work_until_an_endpoint_halo_is_hit` proves a real broad
  arc sample outside every point halo returns no endpoint and leaves the request-local Coincident
  cell uninitialized. Full editor qualification passes 430 library tests plus every integration and
  doc-test target.
- Focused F003 Typed Panel three-target, causal-boundary, matching-alias-set, exact-discrete-state,
  public rectangle scalar and Compass Rose collateral tests pass. The three automated targets run
  sequentially in one retained session; M86-U8 owns human pointer-feel confirmation.
- Warnings-denied affected-crate Clippy and the unchanged 271-row golden evidence pass.
- The pre-trace complete demo-web run passes 307/307 plus binary, integration and doc-test targets;
  the post-trace library run passes 316/316.
- The complete workspace/release gate, no-rebuild freeze and served-byte verification pass for the
  exact provisional patch identity above. The later clean committed-source gate and isolated
  no-rebuild verification also pass at `88d1b5e` / `09018e5`.

## Remaining work

1. Publish the approved descendant through GitHub Pages, download and exact-verify its separately
   rebuilt artifact and hosted paths, then retire both M86 services.
2. Record workflow/artifact/deployment, hosted-byte and service-retirement
   evidence consistently, commit the closeout record and mark M86 complete.
