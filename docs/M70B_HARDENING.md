<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B-H1 test hardening and defect survey

Status: the bounded test-only survey is implemented and currently clean. It changes no runtime
solver, sketch, authoring or workbench behavior. Release qualification and a fresh UAT publication
remain separate gates before the supervising-human M70B review resumes.

## Scope and authority

M70B-H1 exists to replace repeated human discovery of basic authoring-path failures with a
deterministic, continue-through-failure oracle. It covers the ordinary retained headless authoring
path and every reachable scene-authority state affected by M70B. M70 auto-constraint drafting,
browser event behavior, new primitives and production fixes are intentionally excluded.

The survey calls the real public interaction and domain boundaries:

1. `AuthoringState` resolves compatible preselection and repeated-pick authoring;
2. `RetainedEditorCoordinator` applies the resulting typed application;
3. the retained sketch session independently solves and validates the exact current input;
4. public document/contact/measurement APIs verify the stored definition, branch metadata and
   accepted geometric postcondition; and
5. dimension cases edit one display target and then exercise Undo and Redo.

The test contains independent semantic checks, not a second residual implementation. A row can
pass only when publication is current, the result is finite, hard validity is `Valid`, independent
hard-residual validation passed and the maximum normalized hard residual is at most `1e-9`.

The workbench-side scene oracle calls the ordinary `compose_editor_scene` path and checks complete
scene equality, accepted/design provenance, visible problem metadata and whether retained-session
authentication is allowed or forbidden. It does not manufacture `ComputedSceneState::Absent`:
ordinary coordinator construction evaluates even an empty feature document, so the reachable
empty state is a current empty computed snapshot.

## Deterministic matrix

The machine-readable matrix contains 193 independently classified rows:

- 16 `ResolvedConstraintKind` families, each with one deterministic witness and eight seeded
  variants: 144 rows;
- all five `DimensionKind` families, each with one deterministic creation witness and eight
  seeded create/edit/Undo/Redo variants: 45 rows; and
- four reachable scene-authority states: four rows.

The fixed 256-bit base seed is:

```text
aa6ab88cc8aa4878c51d78db3d1b993355406fce8c6c42353a850c05696c2edd
```

Each family/variant derives a separate ChaCha test seed from that value. The eight indices also
schedule every combination of reversed span direction, perturbed-recovery geometry and operand
order reversal rather than leaving those booleans to chance. Seeded geometry varies finite
translation, scale (`0.25`, `1`, `4`), rotation and contact parameter. Horizontal and Vertical
retain world-axis rotation by definition. Tangency covers aligned and opposed orientation;
Equal-curvature cycles signed, same-sign magnitude and opposite-sign magnitude; endpoint
continuity cycles G0, G1, G2 and rate-explicit parametric C2.

Every constraint row verifies:

- selection applicability and terminal authoring resolution;
- deterministic parity between compatible preselection and repeated-pick authoring for the
  deterministic witness;
- the exact resolved constraint family and typed stored definition;
- contact domain, parameter, neighbourhood, winding and orientation where applicable;
- one accepted history checkpoint and exact-current accepted authority;
- independent hard validation plus a public geometric postcondition;
- no movement for a pre-satisfied witness; and
- no collapse of protected lines or positive-radius circles during recovery.

Every dimension row verifies creation, Driving mode, typed unit and target metadata, one finite
display-target edit, exact target restoration through Undo and Redo, history shape, finite
accepted geometry and independent hard validation after every accepted transition.

The reachable scene rows are:

- `scene.current-computed.empty`;
- `scene.current-computed.fillet`;
- `scene.current-native.withheld`; and
- `scene.rejected-historical.detached`.

## Driver and classification contract

`scripts/m70b-hardening-oracle.sh` runs every authoring family in its own process with a 30-second
runtime limit, then runs the scene matrix. Semantic failures, panics, timeouts and harness errors
are rows rather than an instruction to stop; later families still run. Per-case Rust panic
isolation preserves the other eight variants in that family whenever the test process remains
healthy.

The stable TSV schema is:

```text
case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint
```

The three operator modes are:

```bash
./scripts/m70b-hardening-oracle.sh --survey
./scripts/m70b-hardening-oracle.sh --check
./scripts/m70b-hardening-oracle.sh --require-clean
```

`--survey` always executes the complete matrix. `--check` requires exact agreement with
`crates/geosolve-constraint-editor/tests/fixtures/m70b_hardening_oracle.golden.tsv`.
`--require-clean` additionally fails if any recorded row is not `PASS`. Scratch output lives under
the ignored workspace `target/` tree so a full system `/tmp` cannot turn semantic results into
false harness failures.

If a future row fails, preserve its family, case ID, fixed seed, minimized variant fingerprint and
exact family command. Deduplicate common root causes, assign the next `M70B-F003+` identity and add
a `GEOSOLVE_REPRO_V1` payload whenever the fixture can be represented as a workbench workspace.
Do not weaken an oracle or implement a production correction during the discovery phase.

## Survey result and readable defect checklist

The completed survey currently records 193/193 `PASS`, zero semantic defects, zero panics, zero
timeouts and zero harness errors. No `M70B-F003` finding was opened and no new reproduction payload
was required.

| Family | Rows | Result |
| --- | ---: | --- |
| Fixed point | 9 | 9/9 pass |
| Coincident points | 9 | 9/9 pass |
| Point on curve | 9 | 9/9 pass |
| Curve contact | 9 | 9/9 pass |
| Horizontal line | 9 | 9/9 pass |
| Vertical line | 9 | 9/9 pass |
| Parallel lines | 9 | 9/9 pass |
| Perpendicular lines | 9 | 9/9 pass |
| Radial line | 9 | 9/9 pass |
| Equal length | 9 | 9/9 pass |
| Equal radius | 9 | 9/9 pass |
| Equal curvature | 9 | 9/9 pass |
| Midpoint | 9 | 9/9 pass |
| Symmetric about line | 9 | 9/9 pass |
| Curve tangency | 9 | 9/9 pass |
| Endpoint continuity | 9 | 9/9 pass |
| Point-distance dimension | 9 | 9/9 pass |
| Segment-length dimension | 9 | 9/9 pass |
| Radius dimension | 9 | 9/9 pass |
| Diameter dimension | 9 | 9/9 pass |
| Oriented-angle dimension | 9 | 9/9 pass |
| Reachable scene authority | 4 | 4/4 pass |

This clean result is evidence for the bounded representative matrix, not a claim that human UAT
or every family-by-primitive Cartesian product is complete. Existing M55/M62 regressions retain
their broader applicability and curve-family ownership; M70B-F001 and M70B-F002 retain their exact
payload-derived regressions. M70B remains active until fresh release qualification/publication and
explicit supervising-human approval.

## Qualification ledger

Focused commands completed so far:

```text
cargo test --locked -p geosolve-constraint-editor \
  --test m70b_authoring_oracle oracle_inventory_and_tsv_schema_are_exhaustive -- --exact
./scripts/m70b-hardening-oracle.sh --survey
./scripts/m70b-hardening-oracle.sh --check
./scripts/m70b-hardening-oracle.sh --require-clean
cargo test --locked -p geosolve-demo-web m70b_scene_authority_oracle_survey -- --nocapture
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web \
  --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Observed outcomes are 1/1 inventory pass, 193/193 survey rows pass, exact golden check pass,
clean-oracle gate pass, 4/4 scene rows pass, the complete editor crate pass (271 unit tests plus all
named integration suites, including the new 2/2 oracle harness), demo-web 97/97 library plus 1/1
native decoder pass, focused warnings-denied Clippy pass, formatting pass and diff check pass. The
complete workspace/release gate and fresh UAT publication are recorded here after they run.
