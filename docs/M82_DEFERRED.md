<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M82 closeout — computed all-family Offset deferred

Status: **closed by explicit scope withdrawal on 2026-08-21; no M82 product behavior accepted**.

## Decision

Hands-on review found that the computed all-family Offset prototype was not fit for the intended
CAD workflow. M82 is therefore closed as a design exploration, not accepted as a feature. A future
milestone may revisit arbitrary-curve Offset only after approving a materially better ownership,
constraint and topology design.

The supported product remains the accepted M81 behavior baseline. That includes M80's exact native
`ProfileOffset` for authenticated Line/Circle/CircularArc faces and ordered chains, plus ordinary
native-published line-arc-line Fillet topology. It does not include computed general-curve Offset,
computed-feature Offset persistence, generated inverse-edit proxies or the M82 feature UI.

## Preserved prototype

The complete exploration is preserved locally and on GitHub at
`archive/m82-certified-computed-offset-2026-08-21`. The branch points exactly to commit
`d1e2613bff131718df860dc98285fc5d1cf217ab`, tree
`849e10709915a669a9d57de1b2df9b4ccf94d6ee`, and retains:

- the certified parallel-curve and topology-checking kernels;
- computed-feature v2 intent/evaluation and editor/coordinator integration;
- transient source-owned inverse controls;
- the exact periodic-NURBS and fresh-Bezier failure regressions;
- all-family golden coverage, ADR 0038, implementation notes, UAT scorecard and qualification
  evidence.

That branch is research material only. Its architecture is not active project policy and its
mechanically qualified candidate is not an accepted release.

## Mainline disposition

Rollback commit `fa54f30cfaaef3f9fcc1f3e1526fc5a8e5188292` reverses the exact 12-commit M82
range, newest to oldest, without rewriting history. Its tree is
`17b2eeab0eda39e19df81d3cf3e505ceac274825`, exactly equal to M81 closeout commit
`e3cbb8f2ae2800181545bb3405704bdcc3ff46a6` and its tree.

No M82 production code, public API, persistence schema or golden row is retained on `main`.
Potentially reusable pieces such as compound operation accounting, certified containment and
computed-scene fallback remain archive-only because they have no independently accepted non-M82
contract. Mining one later requires its own scoped design and qualification.

Consequences:

- the reviewed authoring/scene golden returns to 271 `PASS` rows with SHA-256
  `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`;
- M82-only computed Offset workspace payloads are not supported by `main`; use the archive branch
  to inspect them;
- the rejected M82 candidate was never published to GitHub Pages, so the existing accepted M81
  Pages deployment remains public product authority;
- former Tailscale UAT PID `3024723` is retired, port `8080` is no longer listening, and frozen
  snapshot `/tmp/geosolve-m82-uat.G4pmMH` remains read-only historical evidence.

## Rollback qualification

The following clean committed-tree command completed with exit `0` on `fa54f30`:

```bash
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

It passed formatting and diff hygiene, warnings-denied workspace Clippy/Rustdoc, locked
all-feature workspace tests, the unchanged 271-row golden gate, native/WASM parity, demo WASM,
benchmark compilation, M14/M32 performance budgets, the release-only 256-moving-body sparse
crossover in 100.27 seconds, licence/package checks and Trunk 0.21.14 release assembly. Relevant
suite totals include editor 404/404, demo 154/154, sketch 39/39 and sketch-features 46/46.

No hands-on feature UAT or new feature Pages publication is claimed. The supervising caller's
explicit decision accepts only the withdrawal, exact baseline restoration, archival preservation
and milestone closure.
