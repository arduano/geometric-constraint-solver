<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Development

Use [Getting started](GETTING_STARTED.md) to prepare the toolchain and packages.
Read [START_HERE.md](../START_HERE.md), [architecture](../ARCHITECTURE.md),
[roadmap](../PLAN.md), [acceptance](../ACCEPTANCE.md) and [scenarios](SCENARIOS.md)
before changing implementation. [AGENTS.md](../AGENTS.md) applies to coding agents.

## Work in the owning layer

Solver equations belong in Rust domain/core crates. Selection, picking, dragging
and tool state belong in the headless editor/engine. TypeScript authors semantic
intent and hosts transactions; rendering and platform input belong to the demo.
Keep host storage/publication policy distinct from reusable session mechanics.
The [architecture map](../ARCHITECTURE.md#ownership-map) identifies these owners.

For a reported solver, domain or headless-interaction defect, follow
[the defect-hardening skill](../.agents/skills/geosolve-harden-defect/SKILL.md).
Reproduce it, add an owning regression, then make the repair. New residuals need
finite-difference Jacobian checks and readable audits. Preserve tolerances, branch
state, exact failure retention and the reviewed golden corpus.

## Focused checks

Run commands from the repository root, inside the prepared development environment.
Choose the owning crate/test rather than rebuilding every suite for each edit:

```bash
cargo fmt --all -- --check
cargo test --locked -p geosolve-sketch --test m98_containment
cargo clippy --locked -p geosolve-sketch --all-targets --all-features -- -D warnings
npm --prefix packages/geosolve-sketch-code run test:types
npm --prefix crates/geosolve-demo-web/frontend run check:types
npm --prefix crates/geosolve-demo-web/frontend test
```

The example Rust test is the manifold profile-containment regression. Replace its
crate and test target with the owner of your change. Package/host tests require
[prepared WASM and JavaScript outputs](GETTING_STARTED.md#build-the-source-packages);
run `node --test` on the relevant file under `scripts/` or a package's `test/`.
Some collaboration tests also need the native `text_fixture` built in that guide.

The maintained [release runner](RELEASE_QUALIFICATION.md) discovers the complete
inventory. The implementation module `scripts/release_gate_m98.py` owns ongoing
engine, folder, collaboration, example and installed-package groups; its milestone
name is historical. Browser qualification uses actual WASM and prepared artifacts.

## Nomination and evidence

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --plan'
nix-shell shell.nix --run './scripts/release-gate.sh --preflight'
nix-shell shell.nix --run './scripts/release-gate.sh'
```

The integrated gate requires clean nominated source and authenticates unchanged
inputs before reusing evidence. Use `--fresh` for a fresh qualification run. A
focused result, successful build or screenshot alone does not close a milestone.
License checks use `cargo-deny`; if it is absent, the runner invokes
`nix-shell -p cargo-deny`. Install it separately when qualifying without Nix.
See [Release qualification](RELEASE_QUALIFICATION.md) for exact scheduling, provenance,
resumption, golden and performance rules.

For an exclusively eligible prose closeout:

```bash
./scripts/release-gate.sh --docs-only --since BASE_COMMIT
```

This mode rejects embedded or potentially consumed Markdown and does not qualify
new package contents. Package READMEs can change shipped documentation even when
no executable behavior changes; preserve the previous product identity until a
new package nomination passes its affected gates.

## Documentation and contribution hygiene

Current instructions belong in these task guides and package READMEs. Keep durable
math/API details in references and ADRs, and completed milestone evidence in
history. Link to the owner instead of duplicating evolving startup commands.
Record human UAT separately from automated results. Do not include private hosts,
credentials, machine paths or temporary process inventories in tracked prose.

Preserve licence and attribution metadata. Some Markdown is embedded in Rust
crate documentation or test fixtures; review readers before editing it. Make
small, reviewable commits when authorized, and do not rewrite published history.
A milestone report records changed files/APIs, mathematical behavior, exact checks,
acceptance results and remaining limitations.
