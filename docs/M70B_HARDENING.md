<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B-H1–H3 test hardening and defect survey

Status: the bounded H1 test-only survey, complete release qualification and byte-verified UAT
publication remain historically clean. H2 preserved that exact 193-row corpus under
milestone-neutral names. Subsequent human UAT opened `M70B-F003` in computed-Fillet authoring and
`M70B-F004` in persisted computed-Fillet source-edit branch traversal, both outside the original
matrix. H3 added four isolated, reviewed `feature.fillet` rows and recorded the exact discovery
state as 193 `PASS` plus four `DEFECT`. The now-authorized repairs resolve both findings at their
headless owners, and the current 197-row golden fixture records 197 `PASS` with the four input
fingerprints unchanged. Both focused repair suites, both aggregate golden modes, formatting,
warnings-denied workspace Clippy, locked all-feature workspace tests and the relevant WASM build
pass. Clean release nomination and supervising-human M70B review and approval remain pending.

M70B-H1 originated this matrix. M70B-H2 moved the unchanged 193-row corpus and driver to
milestone-neutral names so later findings can reuse it without rewriting H1 history. The original
golden bytes and SHA-256 remained unchanged. H2 clean qualification and independent skill forward
tests passed on source `47584bdb607c722df508eae56584726954a03205`.

M70B-H3 kept every H1/H2 row record byte-identical and appended only the missing systemic Fillet
axes already proven at their narrow owners. Its discovery fixture SHA-256 was
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`; that historical hash records
the four reviewed defects and does not supersede the historical H1/H2 source or hash evidence
below. The repaired fixture changes only those four dispositions to `PASS` and has current
SHA-256 `035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`.

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

The historical H1/H2 machine-readable matrix contained 193 independently classified rows:

- 16 `ResolvedConstraintKind` families, each with one deterministic witness and eight seeded
  variants: 144 rows;
- all five `DimensionKind` families, each with one deterministic creation witness and eight
  seeded create/edit/Undo/Redo variants: 45 rows; and
- four reachable scene-authority states: four rows.

The fixed 256-bit base seed is:

```text
aa6ab88cc8aa4878c51d78db3d1b993355406fce8c6c42353a850c05696c2edd
```

Each family/variant derives a separate ChaCha test seed from that value and the stable family ID,
so inventory reordering cannot silently change a witness. The eight indices also schedule every
combination of reversed span direction, perturbed-recovery geometry and operand order reversal
rather than leaving those booleans to chance. Endpoint-continuity seed 03 deliberately retains a
pre-satisfied unequal-rate Parametric-C2 witness while seed 07 remains its displaced,
operand-swapped recovery counterpart. Seeded geometry varies finite
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
- path-oriented endpoint tangency, signed G2 curvature and rate-explicit first/second Parametric-C2
  derivatives where applicable;
- one accepted history checkpoint and exact-current accepted authority;
- independent hard validation plus a public geometric postcondition;
- no movement for a pre-satisfied witness; and
- no collapse of protected lines or positive-radius circles during recovery.

Every dimension row independently measures accepted point distance, segment length, radius,
diameter or directed/unwrapped angle. It verifies creation, Driving mode, dimension/scalar identity,
typed target and ModelUnits/AcuteDegrees display metadata, one finite display-target edit, exact
target restoration through Undo and Redo, history shape, finite accepted geometry and independent
hard validation after every accepted transition.

The reachable scene rows are:

- `scene.current-computed.empty`;
- `scene.current-computed.fillet`;
- `scene.current-native.withheld`; and
- `scene.rejected-historical.detached`.

H3 adds exactly four process-isolated `feature.fillet` rows without changing any of those original
records:

- `feature.fillet.authoring.coincident-closure.point`;
- `feature.fillet.authoring.coincident-closure.curve-pair`;
- `feature.fillet.evaluation.line-circle.same-cell-lower`; and
- `feature.fillet.evaluation.line-circle.same-cell-seam`.

The active inventory is therefore 197 rows. The original 193 remain `PASS`; the two authoring rows
now pass after the `M70B-F003` repair and the two evaluation rows now pass after the `M70B-F004`
repair. Their case IDs and input fingerprints remain stable across discovery and repair.

## Driver and classification contract

`scripts/golden-authoring-scene-oracle.sh` runs every authoring, Fillet-feature and scene row in its
own process with a 30-second runtime limit and a five-second hard-kill grace period. Semantic
failures, panics, timeouts (`124` or hard-kill `137`) and harness errors are rows rather than an
instruction to stop; later rows still run. A nonzero child exit is never accepted merely because
it wrote a complete TSV. Child output must match the requested case and family. H1/H2 required
their exact 193 `(case_id, family)` pairs; the H3 driver requires the exact current 197-pair
inventory and rejects a missing, duplicate or unexpected Fillet row.

The stable TSV schema is:

```text
case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint
```

The three operator modes are:

```bash
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
```

`--survey` always executes the complete matrix. `--check` requires exact agreement with
`crates/geosolve-constraint-editor/tests/fixtures/golden_authoring_scene_oracle.golden.tsv`.
`--require-clean` additionally fails if any recorded row is not `PASS`; at the historical H3
discovery checkpoint it therefore failed intentionally on the four reviewed findings even though
`--check` succeeded. The current fixture contains no expected non-`PASS` row, and its aggregate
`--check` and `--require-clean` reruns both pass. Scratch output lives
under the ignored workspace `target/` tree so a full system `/tmp` cannot turn semantic results
into false harness failures. `GEOSOLVE_GOLDEN_ORACLE_CASE` selects exactly one row inside each
child. Every authoring PASS fingerprint is `input-<fnv1a64>` over the effective post-scheduling
variant; the golden therefore detects seed/scheduling drift instead of recording an uninformative
`ok`.

If a future row fails, preserve its family, case ID, fixed seed, minimized variant fingerprint and
exact family command. Deduplicate common root causes, assign the next active-milestone `M*-F*`
identity only after independent reproduction and add a `GEOSOLVE_REPRO_V1` payload whenever the
fixture can be represented as a workbench workspace.
Do not weaken an oracle or implement a production correction during the discovery phase.

## Historical H1/H2 survey result and readable checklist

The completed H1/H2 matrix recorded 193/193 `PASS`, zero semantic defects, zero panics, zero
timeouts and zero harness errors within its declared scope. It opened no finding at H1 survey time.
Later human UAT opened `M70B-F003`: the matrix has no computed-Fillet authoring rows, so its green
result cannot detect a Coincident-closed triangle closure rejected through both point and curve-pair
collection. The compact public-Rust fixture needs no reproduction payload and is retained in the
focused owner regression instead. F004 then supplied two exact workspace payloads; the matrix's
single unchanged precomposed Fillet scene cannot exercise their persistent nonlinear branch after
a source edit. Both payloads are therefore retained as one focused feature-owner characterization,
not misreported as two defects or broad passing rows.

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
payload-derived regressions, while resolved M70B-F003 and M70B-F004 retain focused positive
headless regressions. M70B remains active until broad qualification and explicit supervising-human
UAT approval.

Historical H3 F003 discovery evidence used the negative test name below:

```text
cargo test --locked -p geosolve-constraint-editor \
  --test m70b_closed_triangle_fillet \
  m70b_f003_coincident_triangle_closure_is_not_filletable_by_point_or_curve_pair \
  -- --exact --nocapture
./scripts/golden-authoring-scene-oracle.sh --check
```

That first command passed at H3 by asserting the exact defect signature and transactional
retention. The current positive regression is
`m70b_f003_coincident_triangle_closure_is_filletable_by_point_or_curve_pair`; the focused suite
passes with either Coincident endpoint, the first/last spans in both orders, exact three-corner
preview/publication and one Current feature containing three Fillet arcs:

```text
cargo test --locked -p geosolve-constraint-editor \
  --test m70b_closed_triangle_fillet
```

The root cause was direct point-ID topology: operand incidence, same-polyline join eligibility and
retained-endpoint hints did not recognize active transitive Coincident equivalence. The repair adds
deterministic `SketchDocument::point_coincidence_representatives()` derived only from active,
explicit Coincident constraints and uses those representatives in all three topology decisions.
Coordinate proximity never welds points.

Historical H3 F004 discovery evidence reconstructed payload fingerprints
`4752:daa87c91c75abf9f` and
`4750:beda1885b15e38b5` as one two-case feature-owner regression. It asserts finite independently
hard-valid accepted sketches, exact retained branch metadata, `NoLocalRoot` with no partial output,
explicitly re-anchored same-cell valid roots and unchanged sketch/feature identities. The unchanged
H1/H2 193-row check remained green, demonstrating that the layered golden strategy first routed
this computed-feature source-edit/branch case to its narrow owner rather than pretending the broad
matrix detected it.

```text
cargo test --locked -p geosolve-sketch-features --lib \
  tests::m70b_f004_line_circle_same_branch_roots_are_rejected_beyond_seed_window \
  -- --exact --nocapture
./scripts/golden-authoring-scene-oracle.sh --check
```

The current positive regression is
`m70b_f004_line_circle_persisted_evaluation_traverses_complete_radial_branch_cell`; the complete
locked all-feature owner suite passes 42/42:

```text
cargo test --locked -p geosolve-sketch-features --all-features
```

The root cause was persisted evaluation narrowing every non-affine parent to 12.5% of its explicit
cell around the stored seed. A line paired with a Circle or CircularArc now searches the complete
certified tangent-orientation cell: constant circular curvature makes that traversal branch-local.
Generic nonlinear parents and direct-manipulation continuation retain the narrower seed-connected
guard, and radius continuation is unchanged.

## M70B-H3 Fillet inventory and current repaired checklist

H3 expanded the broad oracle only after the two root causes had focused owner characterizations.
It added one compact process-isolated row for each public route/branch dimension. The authorized
repairs retain the exact case and input identities while changing the current dispositions to
positive behavior:

| Case ID | Public route or branch | Current status | Resolved finding | Input fingerprint |
| --- | --- | --- | --- | --- |
| `feature.fillet.authoring.coincident-closure.point` | Point-to-corner authoring | `PASS` | `M70B-F003` | `input-4ba571059db7afff` |
| `feature.fillet.authoring.coincident-closure.curve-pair` | Last/first span-pair authoring | `PASS` | `M70B-F003` | `input-d04adbf29c08b9bd` |
| `feature.fillet.evaluation.line-circle.same-cell-lower` | Same-cell lower root, winding 0 | `PASS` | `M70B-F004` | `input-f9920c3cf170130d` |
| `feature.fillet.evaluation.line-circle.same-cell-seam` | Same-cell seam root, winding 1 | `PASS` | `M70B-F004` | `input-2da21ef04cfb4246` |

| Inventory cut | PASS | DEFECT | PANIC | TIMEOUT | HARNESS_ERROR | Total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Historical H1/H2 | 193 | 0 | 0 | 0 | 0 | 193 |
| Historical H3 discovery | 193 | 4 | 0 | 0 | 0 | 197 |
| Current repaired fixture | 197 | 0 | 0 | 0 | 0 | 197 |

`crates/geosolve-constraint-editor/tests/golden_fillet_oracle.rs` owns the four current rows. The
authoring pair calls the public headless feature-authoring and retained-coordinator transactions.
The evaluation pair calls public persisted computed-feature evaluation directly. Only a
`NoLocalRoot` diagnostic path uses public contact reseeding to prove that a viable same-cell root
was withheld; a current `PASS` independently requires finite accepted geometry, hard validity,
contact/source incidence, radius, tangency, signed side, native source/span identity,
parameter/winding representation and membership in the unchanged Local cell.

Current focused repair evidence:

```text
cargo test --locked -p geosolve-constraint-editor \
  --test m70b_closed_triangle_fillet
cargo test --locked -p geosolve-sketch-features --all-features
```

Both focused suites pass. The repaired 197-row fixture records 197 `PASS`, zero defects, panics,
timeouts or harness errors, and has SHA-256
`035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`. Aggregate oracle
`--check`/`--require-clean`, full workspace tests, warnings-denied Clippy, formatting and the
relevant WASM check pass; no clean repair release or replacement publication is claimed.

## Qualification ledger

Historical H1 focused commands completed:

```text
cargo test --locked -p geosolve-constraint-editor \
  --test golden_authoring_oracle golden_oracle_inventory_and_tsv_schema_are_exhaustive -- --exact
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
cargo test --locked -p geosolve-demo-web --lib \
  workbench::tests::golden_scene_authority_oracle_survey -- --exact
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web \
  --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Historical H1 observed outcomes were 1/1 inventory pass, 193/193 survey rows pass, exact golden check pass,
clean-oracle gate pass, 4/4 scene rows pass, the complete editor crate pass (271 unit tests plus all
named integration suites, including the 2/2 oracle harness), demo-web 97/97 library plus 1/1 native
decoder pass, focused warnings-denied Clippy pass, WASM test compilation, formatting pass and diff
check pass. The historical H1/H2 golden SHA-256 is
`803c443d12a7362993fd557bd96d9db496ce162579d0ae08e2feff57b009e19b`.

Driver fault injection with a temporary fake Cargo executable also completed without leaving a
repository file: ordinary nonzero exit, timeout `124`, hard-kill `137` and wrong-family output were
classified independently while the remaining rows continued and the exact inventory remained
present.

## Release qualification and publication

Nominated source `dd645d99e705e56c80ab2a4a136f7a4d03baafbf` passed:

```text
env NO_COLOR=true \
  TMPDIR=/home/arduano/programming/geometric-constraint-solver/target \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

The complete gate passed formatting, warnings-denied Clippy, locked all-feature workspace tests,
native/WASM M70 transition parity, the demo-web WASM check, warnings-denied rustdoc, benchmark
compilation, M14/M32 performance budgets, package/licence and single-workbench/Git-hygiene checks,
the 256-moving-body sparse crossover in `123.32s`, and Trunk 0.21.14 release assembly. Only the
pre-existing non-failing Cargo `license` plus `license-file` notices appeared.

The fresh read-only seven-file snapshot is `/tmp/geosolve-m70b-h1-uat.viSB9G`, served at
`http://100.94.63.83:8080/`. Its exact file hashes are:

| File | SHA-256 |
| --- | --- |
| `API_COMPATIBILITY.md` | `af91333ed578f05ec49c76fd10c18dd0ead0f9f845b8ff45279de5a6cbc7b80e` |
| `LICENSE` | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-7be0279dd606ae0c.js` | `a6fb10ec3fac3021c5b2c5f92e1bbbd96f2ef0920a1e10c990ab4244ce04adda` |
| `geosolve-demo-web-7be0279dd606ae0c_bg.wasm` | `a379c7c8307fda6715e22a3e64d786942bf4095505a3fc972c02fc38e2dbb63e` |
| `index.html` | `1ad69307a269c0e9f7431e7c0c077b39cb0a490985c15360e38992e5646200f1` |
| `styles-36c74d05d21a90c9.css` | `49a0d71647856a30e798707860ffa9da4dbdbd1ec2f4faeafa412726f0e69048` |

The ordered manifest aggregate is
`f33cc593dbe719f192a5a08ea293678f4c053adbe6b9bf4f44f8bae662f53019`.
Because H1 changes test infrastructure only, these release bytes intentionally match the F002
candidate. Proxy- and cache-bypassed requests through the actual Tailscale address byte-compared
`/` and every asset with the frozen snapshot. Human UAT remains pending.

### M70B-H2 canonical workflow qualification

Clean source `47584bdb607c722df508eae56584726954a03205` passed the same complete command after the generic
oracle became a mandatory release-gate step. The gate passed formatting, warnings-denied Clippy,
locked all-feature workspace tests, 193/193 isolated golden rows, native/WASM M70 transition
parity, the demo-web WASM check, warnings-denied rustdoc, benchmark compilation, M14/M32
performance budgets, package/licence and single-workbench/Git-hygiene checks, the 256-moving-body
sparse crossover in `142.95s`, and Trunk 0.21.14 release assembly. Only the pre-existing
non-failing Cargo `license` plus `license-file` notices appeared.

The official skill validator passed. Fresh-context forward tests demonstrated that an incomplete
historical solver/headless report remains diagnosis-only, routes to the narrow drag/session owner,
does not assign a finding or expand the broad matrix without a payload body, and makes no
production fix; a pure CSS-only report does not invoke the workflow. Fault injection continued
through all 193 rows and classified 188 ordinary passes, one panic, two timeout/hard-kill rows and
two harness errors independently.

The golden SHA-256 remains
`803c443d12a7362993fd557bd96d9db496ce162579d0ae08e2feff57b009e19b`. Every generated release
file retains the H1 hash listed above, and the ordered manifest aggregate remains
`f33cc593dbe719f192a5a08ea293678f4c053adbe6b9bf4f44f8bae662f53019`. H2 therefore leaves the
served H1 product candidate unchanged and requires no Tailscale republish. Human UAT remains
pending.

### M70B-H3 discovery and current repair gate state

H3 changed only test infrastructure and documentation, so the H1 product distribution and release
manifest above remained the last qualified bytes. Its historical 197-row `--check` passed against
golden SHA-256 `a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`, while
`--require-clean` intentionally failed on the two F003 and two F004 rows and no others.

The authorized repair now changes production behavior at the headless authoring and computed
feature-evaluation owners. The current fixture is 197/197 `PASS` at SHA-256
`035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`; both focused owner suites,
both aggregate golden checks, formatting, warnings-denied workspace Clippy, locked all-feature
workspace tests and the relevant WASM build pass. The complete clean release gate and replacement
publication remain pending, so the H1 distribution is still the last qualified and served product
rather than a repair candidate.
