<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M77 implementation — CAD curve handles and implicit parameters

Status: active (2026-08-17). This record is intentionally incomplete until implementation,
qualification, frozen-candidate review and closeout are finished.

## Approved architecture

- `geosolve-sketch` owns the semantic control catalog, inverse configuration projections,
  rational ordinary/projective conversion and independently validated document edits.
- Prepared work exposes only an immutable accepted preview view. The live retained session changes
  solely when an exact patch wins compare-and-swap publication.
- `geosolve-constraint-editor` owns selected-only control identities, guides, paint/hit geometry,
  hover/click priority, pointer gestures, last-valid preview and exact property metadata.
- `geosolve-demo-web` renders headless DTOs and forwards typed inputs. It contains no curve
  equation, inverse projection, branch choice or independent hit policy.

## Implementation evidence

Pending. Record files and public APIs, mathematical behavior, focused test commands and outcomes,
acceptance coverage, release qualification, immutable candidate identity and known limitations as
the milestone progresses.

## Closeout evidence

Pending explicit human UAT disposition, accepted-source GitHub Pages publication, hosted-byte
verification and a clean final worktree.
