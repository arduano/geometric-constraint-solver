<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 managed editing and interaction fixes: closure summary

M86 was accepted and closed on 2026-08-29, including M86-U1 through M86-U8 and the
bounded interaction trace. Acceptance was at milestone level; a separate row-by-row
replay was not recorded. Qualified source `88d1b5e`, tree `09018e5`, passed the
complete clean gate. Documentation descendant `ccf791f` was verified on GitHub Pages.

See [implementation](M86_IMPLEMENTATION.md) for the findings and
[UAT](M86_UAT.md) for the acceptance scope. Earlier candidates were superseded;
this document is a technical summary, not a service runbook.

## Delivered work

- **M86-F001 — managed dimension Inspector edits:** commit `9050424` routes authenticated direct
  code-owned curve-length/diameter target edits through managed TypeScript source rewriting and
  atomic rematerialization. Invalid edits retain the last accepted scene and ordinary Undo/recovery.
  The finding is accepted.
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
  historical provisional candidate. The later clean committed-source gate and isolated
  no-rebuild verification pass at `88d1b5e` / `09018e5`.

## Bounded trace

The memory-only `Copy trace` surface records pointer, semantic-route, parity,
publication and rollback evidence. It is excluded from source, history, persistence
and reproduction payloads. Exports are bounded to 128 KiB.

## Clean committed-source nomination

The exact source was `88d1b5e06a7ce8ffe38931f792492f6f837a1d74`, tree
`09018e5aeb7e824396ae2ee2c70a3e30912414fa`. The complete clean release gate passed
on 2026-08-29. Its seven-file artifact aggregate was
`d9d88bfb8ac4acd3f8d45f2cbbc965694297d76b61192be12c4fe65b9deb557e`.

## Public closeout

Documentation descendant `ccf791f131ba8de07a0d32df6938719cf4bcab12` passed
Pages run `33232073614`, artifact `9708871725`. Hosted root and all seven paths
matched the downloaded artifact's bytes, MIME types and lengths. The public artifact
aggregate was `ecf6a5550c54fe8fecc1f635500c3a2b208638beacb38ee98da379e8dcd7a7d2`.

No M86 implementation or acceptance work remains. Current release procedures are
in the [release guide](RELEASE_QUALIFICATION.md).
