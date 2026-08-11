<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Canonical GeoSolve defect testing

Use a layered golden strategy. Keep the 193-row authoring/scene oracle as the stable broad
compatibility matrix; do not turn it into the only regression suite or a dumping ground for exact
defects.

## Testing layers

1. **Owning-layer exact regression:** minimize the report at the public Rust boundary that owns the
   behavior. Preserve a supplied scene/payload fingerprint when it is semantically relevant.
2. **Stable golden matrix:** cover the complete public authoring-family inventory, deterministic
   seeded transformations, lifecycle operations, and reachable scene-authority states. Expand it
   only for systemic gaps.
3. **Thin integration parity:** test WASM or workbench composition only when data or authority is
   lost across that boundary. Never use a browser snapshot as the mathematical oracle.
4. **Human UAT:** reserve for discoverability, visual presentation, and interaction feel after the
   headless contract is mechanically qualified.

## Ownership routing

| Evidence or symptom | First regression owner | Required emphasis |
|---|---|---|
| Residual, Jacobian, convergence, rank, scaling, NaN/Inf, invalid success | `geosolve-core` | Independent residual validation; finite-difference Jacobian for equation changes |
| Sketch primitives, constraints, dimensions, contact domains, branch state, document solve/history | `geosolve-sketch` | Public document/session APIs; persistent IDs; explicit branches; atomic rejection |
| Rigid-body/linkage semantics, mobility, planar/3D parity | `geosolve-linkage` | Domain API and independent kinematic/mobility invariants |
| Applicability, operand collection, picking, authoring state, guides, drag policy, interaction lifecycle | `geosolve-constraint-editor` | Presentation-independent state machine and accepted-scene inputs |
| Current-only feature transactions, retained history, prepared-input publication, scene authentication | Retained editor coordinator | Exact accepted revision/input authority and no partial publication |
| Scene composition or native/WASM presentation DTO parity | Headless scene owner first; `geosolve-demo-web` only for the crossed adapter | Same authoritative accepted scene; no duplicated equations |
| CSS, layout, labels, icons, or browser-only text selection | Presentation owner; outside this skill | Escalate into this workflow only if a Rust headless contract is implicated |

Start at the lowest owner supported by the evidence, not automatically at `geosolve-core`. Move
outward only when the lower layer passes and the failure survives at the next public boundary.

## Defect record

Capture these facts before repair:

- exact user steps and intended versus observed behavior;
- source revision and relevant accepted revision/input identity;
- payload version, length, checksum/fingerprint, and original bytes or durable fixture when supplied;
- smallest public owning boundary and minimal reproducer;
- failure class: `DEFECT`, `PANIC`, `TIMEOUT`, or `HARNESS_ERROR`;
- independently checked invariants and unrelated geometry/state that must remain stable;
- focused test name, commands, disposition, and known limitations.

Verify every claimed existing test target against the repository. Label a not-yet-created
regression name as proposed rather than placing it among commands that ran.

Assign the next active-milestone ID, such as `M70B-F003` or `M71-F001`, only after independent
reproduction. Do not assign an ID to an unconfirmed report, harness fault, or duplicate root cause.
Keep historical IDs stable when infrastructure becomes milestone-neutral.

## Independent invariants

Do not trust a success-like status as the oracle. Check the applicable invariants directly:

- every accepted coordinate, scalar, residual, Jacobian entry, diagnostic, and audit value is
  finite;
- independently recomputed normalized hard residual is at most `1e-9`;
- invalid geometry, NaN/Inf, a rejected branch, or failed validation never becomes convergence;
- a rejection preserves the complete prior accepted document, geometry, revision, history,
  branches, diagnostics, and publication authority;
- branch, orientation, contact neighborhood/domain, span, and winding choices remain explicit;
- rank, DOF/mobility, active bounds, and redundancy/conflict evidence match the intended state;
- temporary drag targets and locality affect only their declared entities; unrelated free geometry
  remains stable when that stability is part of the interaction contract;
- persistent IDs and audit descriptors identify the public semantic sources involved;
- hard/soft priority semantics remain explicit and are not approximated by undocumented weights.

When geometry is not unique, assert semantic validity, continuity/locality, and retained branch
state rather than a single arbitrary coordinate solution. For every new or changed residual, add a
central finite-difference Jacobian comparison and a structured human-readable audit descriptor.

## Discovery and repair modes

In **diagnosis mode**, run non-mutating reproduction and focused tests, identify the owner and root
cause, and stop before edits.

In **test-hardening mode**, build the complete defect checklist without production corrections.
Run cases independently with timeouts, catch panics at the harness boundary where possible, record
every row, and continue after failures. Add test or evidence changes only when authorized.

In **repair mode**, first demonstrate the defect with the focused owner regression. Apply the
smallest production correction, then prove the regression and relevant collateral pass. Never
weaken an invariant or reclassify invalid behavior as success to close a finding.

## Expanding the golden matrix

Add a matrix dimension only when a defect reveals systemic missing coverage in at least one of
these categories:

- a public constraint/dimension/authoring family;
- operand order or commutative selection path;
- explicit branch, orientation, domain, span, winding, or contact-neighborhood option;
- translation, rotation, scale, reversed span, displaced seed, or another deterministic
  metamorphic transform;
- create, edit, delete, Undo, Redo, restore, or retained-history lifecycle;
- Current versus historical, computed versus native, accepted versus rejected, or withheld scene
  authority;
- a repeatable panic, timeout, or process-isolation failure class.

Otherwise keep the case as a focused named regression. Preserve fixed seeds, exact case IDs,
input fingerprints, row ordering, and the complete inventory. Use `--survey` to discover and
classify; use `--check` to compare against reviewed bytes; use `--require-clean` for release
qualification. Never copy survey output over the golden file without reviewing every changed row,
its reproduction, finding identity, and expected disposition.

## Qualification sequence

Run the narrowest focused owner test first, then broaden proportionally. Use the current generic
oracle entry points:

```bash
cargo test --locked -p geosolve-constraint-editor \
  --test golden_authoring_oracle golden_oracle_inventory_and_tsv_schema_are_exhaustive -- --exact
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
```

For adapter-crossing failures, also run the exact native `geosolve-demo-web --lib` scene test and
the relevant locked WASM check. Before milestone nomination, run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
```

Run `./scripts/release-gate.sh` from a clean nominated source for a complete release claim. During
development only, `GEOSOLVE_ALLOW_DIRTY=1 ./scripts/release-gate.sh` may provide provisional
evidence; never describe that dirty run as clean candidate qualification.
