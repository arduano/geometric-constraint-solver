# M63 implementation — canvas constraint visualization

Status: implementation and mechanical qualification complete; human UAT pending.

## 1. Files and APIs

- `geosolve-constraint-editor` adds public typed scene annotations for constraint glyphs,
  linear/radial/angular dimensions, visibility, direct operands and hit geometry.
- `ConstraintEditor` retains typed hover identity, emits `HoverChanged`, clears hover on surface
  leave/cancel and supports annotation-first Select-mode pointer input with diagnostic context.
- The sole workbench consumes those DTOs as accessible SVG symbols, leaders, dimension geometry,
  values and related-operand highlighting.
- Three stable M63 scenario definitions and this milestone/UAT record are added. No persistence
  schema or solver API changes.

## 2. Mathematical behavior

No residual, equation, tolerance, rank, convergence or branch behavior changes. Annotation
positions are derived only from independently accepted finite geometry, persistent contacts and
existing dimension presentation values. The browser does not evaluate constraint equations.

## 3. Verification

Required final commands:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
```

All five commands passed on 2026-07-30. The first Trunk invocation was accidentally run in
parallel with the standalone WASM check and lost a transient optimized-artifact copy race;
rerunning the exact release build alone passed. No source or generated distribution input changed
between those invocations. Existing Cargo manifest `license`/`license-file` advisories remain
unchanged and non-failing.

## 4. Acceptance

Direct coverage owns complete active constraint/dimension projection across representative public
ordinary and advanced scenarios, contextual visibility, always-visible reference angles,
deterministic fan-out, hover transitions, annotation selection and presentation metadata.
Human acceptance remains pending in `docs/M63_UAT.md`.

## 5. Known limitations

- Annotation layout is intentionally compact and deterministic, not a general-purpose CAD
  drafting-layout optimizer.
- M63 does not add manual dimension-label dragging or persisted annotation placement.
- Desktop-only workbench and existing solver/branch scope remain unchanged.
