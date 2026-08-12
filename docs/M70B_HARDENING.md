<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B-H1–H3 test hardening and defect survey

Status: the bounded H1 test-only survey, complete release qualification and byte-verified UAT
publication remain historically clean. H2 preserved that exact 193-row corpus under
milestone-neutral names. Subsequent human UAT opened `M70B-F003` in computed-Fillet authoring and
`M70B-F004` in persisted computed-Fillet source-edit branch traversal, both outside the original
matrix. H3 added four isolated, reviewed `feature.fillet` rows and recorded the exact discovery
state as 193 `PASS` plus four `DEFECT`. The authorized F003/F004 repairs resolve both findings at
their headless owners and produced a fully qualified 197/197-`PASS` replacement with the four input
fingerprints unchanged. F005 then adds one source-rotation row for payload
`4228:0823d31f269300af`; the current 198-row golden records 198 `PASS` at SHA-256
`bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`. Its 45-test feature-owner
suite, nine-test retained movement suite, focused golden tests, all aggregate golden modes,
formatting, warnings-denied all-workspace Clippy, locked all-feature workspace tests and the
relevant WASM check pass. Clean source `d400c4a8201f6afc531f5b504424d6430dbf3937`
passes the complete release gate, and its fresh immutable seven-file publication is byte-verified
through Tailscale. The supervising human subsequently reported the F005 movement behavior fixed and
requested sign-off once the closing regressions were satisfactory. Clean closing source `48e3cc3`
passes the complete release gate with the focused two-previously-Current projected-drag transaction
and CircularArc transport/domain parity regressions. The unchanged 198/198 golden and byte-identical
F005 release output confirm the intended layered result. The scoped decision closes M70B without
claiming an exhaustive Cartesian oracle or unrecorded UAT replay.

M70B-H1 originated this matrix. M70B-H2 moved the unchanged 193-row corpus and driver to
milestone-neutral names so later findings can reuse it without rewriting H1 history. The original
golden bytes and SHA-256 remained unchanged. H2 clean qualification and independent skill forward
tests passed on source `47584bdb607c722df508eae56584726954a03205`.

M70B-H3 kept every H1/H2 row record byte-identical and appended only the missing systemic Fillet
axes already proven at their narrow owners. Its discovery fixture SHA-256 was
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`; that historical hash records
the four reviewed defects and does not supersede the historical H1/H2 source or hash evidence
below. The F003/F004 repair checkpoint changed only those four dispositions to `PASS` and had
SHA-256 `035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`.
F005 preserves those 197 records byte-for-byte, appends one passing source-rotation row and yields
the current SHA-256 `bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`.

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

That historical H3 inventory is 197 rows. The original 193 remain `PASS`; the two authoring rows
now pass after the `M70B-F003` repair and the two evaluation rows now pass after the `M70B-F004`
repair. Their case IDs and input fingerprints remain stable across discovery and repair.

F005 appends one systemic persisted-evaluation movement dimension without changing any prior row:

- `feature.fillet.evaluation.line-circle.source-rotation.retained-start`.

The active inventory is therefore 198 rows, all `PASS`. Its input fingerprint is
`input-04658a77db2dc779` and its payload identity is `4228:0823d31f269300af`.

## Driver and classification contract

`scripts/golden-authoring-scene-oracle.sh` runs every authoring, Fillet-feature and scene row in its
own process with a 30-second runtime limit and a five-second hard-kill grace period. Semantic
failures, panics, timeouts (`124` or hard-kill `137`) and harness errors are rows rather than an
instruction to stop; later rows still run. A nonzero child exit is never accepted merely because
it wrote a complete TSV. Child output must match the requested case and family. H1/H2 required
their exact 193 `(case_id, family)` pairs; H3 required 197 pairs and the current driver requires
the exact 198-pair inventory, rejecting a missing, duplicate or unexpected Fillet row.

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
headless regressions. M70B is closed under the supervising human's requested scoped sign-off
recorded below.

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
`m70b_f004_line_circle_persisted_evaluation_traverses_complete_radial_branch_cell`; at the F004
repair checkpoint the complete locked all-feature owner suite passed 42/42. F005 extends the
current suite to 45/45:

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
| `feature.fillet.evaluation.line-circle.source-rotation.retained-start` | Moved affine source, overlapping transported cells, retained Start | `PASS` | `M70B-F005` | `input-04658a77db2dc779` |

| Inventory cut | PASS | DEFECT | PANIC | TIMEOUT | HARNESS_ERROR | Total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Historical H1/H2 | 193 | 0 | 0 | 0 | 0 | 193 |
| Historical H3 discovery | 193 | 4 | 0 | 0 | 0 | 197 |
| Post-F003/F004 repaired fixture | 197 | 0 | 0 | 0 | 0 | 197 |
| Current F005 fixture | 198 | 0 | 0 | 0 | 0 | 198 |

`crates/geosolve-constraint-editor/tests/golden_fillet_oracle.rs` owns the five current rows. The
authoring pair calls the public headless feature-authoring and retained-coordinator transactions.
The three evaluation rows call public persisted computed-feature evaluation directly. Only a
`NoLocalRoot` diagnostic path uses public contact reseeding to prove that a viable root was
withheld. Every current `PASS` independently requires finite accepted geometry, hard validity,
contact/source incidence, radius, tangency, signed side, native source/span identity and explicit
parameter/winding representation. F004 additionally requires membership in the unchanged Local
cell; F005 requires a fresh stored-to-seed-to-candidate certificate-overlap chain and rejection of
the alternate root across the real orientation barrier.

Current focused repair evidence:

```text
cargo test --locked -p geosolve-constraint-editor \
  --test m70b_closed_triangle_fillet
cargo test --locked -p geosolve-sketch-features --all-features
cargo test --locked -p geosolve-constraint-editor \
  --test m70b_f005_retained_movement
```

All three focused suites pass, including F005's exact payload-derived feature-owner regression
`m70b_f005_line_circle_source_rotation_transports_persisted_branch_cell`. The current 198-row
fixture records 198 `PASS`, zero defects, panics, timeouts or harness errors, and has SHA-256
`bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`. Aggregate oracle
`--survey`/`--check`/`--require-clean`, formatting, warnings-denied all-workspace Clippy, locked
all-feature workspace tests and the relevant WASM build pass. The clean release gate and
replacement publication evidence is recorded below.

### M70B-F005 movement-certificate checklist

The exact loadable capsule is preserved at
`crates/geosolve-demo-web/tests/fixtures/m70b_f005_repro.txt`; its identity is
`4228:0823d31f269300af`, and an ordinary-decoder/web-workspace regression restores it before
checking the Current computed scene. The accepted sketch is finite, hard-valid at normalized
residual `0`, rank zero and seven DOF. Before repair, public computed-feature evaluation
returned `Failed(NoLocalRoot)` and withheld the Fillet. The valid circle contact
`7.909322804062922` is only `0.051999730670326` beyond the stored upper certificate, is transverse
at about `0.527757`, and has a root-centred current certificate overlapping the fresh certificate
at the persisted seed. The alternate root at about `9.021239181530` is across the real orientation
barrier.

The owner regression freezes exact accepted/feature JSON, IDs/revisions/digest, rank/DOF, persistent
normal sides, retention, winding, anchor, endpoint order and sweep; independently checks source
incidence, radius, tangency, signed offsets and transversality; requires full-circle non-trimming;
and proves evaluation leaves accepted input and feature bytes unchanged. The broad row adds the
missing systemic dimension—affine-source rotation moving a root beyond a stale certificate—and
uses the same independent invariants. Existing same-cell, exact-fold, offset-singularity,
high-curvature remote-root and radius-continuation tests remain the negative/collateral controls.

The separate retained movement suite contains nine exact tests rather than duplicating a static
payload row nine times in the golden. It covers a true pointer gesture across both the stale
certificate edge and the real cardinal point, stepwise full-period winding, durable re-anchoring,
mixed Current/Failed state, exact replay/edit binding, stale scene authority, a genuine finite-
parent barrier, reverse recovery, terminal-invalid release, and first-sample rejection. Its
independent invariants require paired accepted/native and computed input identity, unchanged
branch semantics, finite incidence/radius/tangency, no root hop, last-complete-scene retention,
targeted limit attribution, no history on an entirely invalid gesture, and exact Undo/Redo/replay/
reload recovery. Coordinator unit regressions additionally force allocator exhaustion between
continued evaluation and cold durability publication, reject replay after a host parameter-input
change, prove a non-Edit constraint action cannot persist an unrecorded feature revision, and
require a direct edit that newly fails one Fillet while re-anchoring another to replay the exact
recorded dispositions. The
cold feature regression compares exact generated geometry/provenance, contact metadata, discarded
construction fragments and dispositions while ignoring only evaluation-local IDs. The existing web
scene test proves targeted computed-feature metadata highlights
only the named sources and renders a local icon; no browser-side branch inference is added.

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

The fresh read-only seven-file snapshot was `/tmp/geosolve-m70b-h1-uat.viSB9G`, served at
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
`/` and every asset with the frozen snapshot. Human UAT remained pending at that historical
checkpoint.

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
`f33cc593dbe719f192a5a08ea293678f4c053adbe6b9bf4f44f8bae662f53019`. H2 therefore left the
then-served H1 product candidate unchanged and required no Tailscale republish. Human UAT remained
pending at that historical checkpoint.

### M70B-H3 discovery, F003/F004 gate and F005 qualified state

H3 changed only test infrastructure and documentation, so the H1 product distribution and release
manifest above remained the last qualified bytes. Its historical 197-row `--check` passed against
golden SHA-256 `a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`, while
`--require-clean` intentionally failed on the two F003 and two F004 rows and no others.

The authorized F003/F004 repair changed production behavior at the headless authoring and computed
feature-evaluation owners and passed the complete qualification recorded below. F005 subsequently
adds one source-rotation certificate-transport row without altering those 197 records. The current
fixture is 198/198 `PASS` at SHA-256
`bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`; its exact owner regression,
45-test owner suite, nine-test retained movement suite, focused golden tests, all aggregate golden
modes, formatting, warnings-denied all-workspace Clippy, locked all-feature workspace tests and the
relevant WASM check pass. F005's clean release qualification and replacement publication pass as
recorded below.

### M70B-F003/F004 replacement qualification and publication

Clean `main` source `0ef60ef47035e8b1fb1eece2c38d05ccdfdc4abf` passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The complete gate exited zero, including formatting, warnings-denied workspace Clippy, locked
all-feature workspace tests, the 197/197 clean golden oracle, native/WASM transition parity, the
demo-web WASM check, warnings-denied rustdoc, benchmark compilation, performance budgets,
package/licence and Git-hygiene checks, and release Trunk assembly.

The immutable replacement snapshot is `/tmp/geosolve-m70b-f003-f004-uat.lKC2xY`; the directory is
mode `0555` and each of its exactly seven files is mode `0444`:

| File | SHA-256 |
| --- | --- |
| `API_COMPATIBILITY.md` | `af91333ed578f05ec49c76fd10c18dd0ead0f9f845b8ff45279de5a6cbc7b80e` |
| `LICENSE` | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-f582f5825ff9a317.js` | `8fd77fcef71dacc2a9b2e6e38748827cb0cb73d771d4b0b1378e96e603cdac47` |
| `geosolve-demo-web-f582f5825ff9a317_bg.wasm` | `48a0382678ccffee08c15621e9d5c34708d4d9aedfbdad1fa519806974c75836` |
| `index.html` | `fb7ea6cddc7603a876ad90d6537d42434b82565a8245606217af593598f1ab79` |
| `styles-36c74d05d21a90c9.css` | `49a0d71647856a30e798707860ffa9da4dbdbd1ec2f4faeafa412726f0e69048` |

The ordered manifest aggregate is
`96cc64dec998074ede56e3e38fb919a4854d0e0dbb8030138393e01a3d0844d3`. The historical publication
was bound only to the Tailscale address. Proxy- and cache-bypassed fetches proved that `/` matched
`index.html` and every served asset byte-matched its immutable local counterpart. F005 superseded
that publication, so no obsolete PID or claim that it still occupies the shared endpoint is
retained. The F003/F004 targeted human rechecks and supervising-human approval remained pending at
that checkpoint.

### M70B-F005 replacement qualification and publication

Clean `main` source `d400c4a8201f6afc531f5b504424d6430dbf3937` passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The complete gate exited zero, including formatting, warnings-denied workspace Clippy, locked
all-feature workspace tests, the 198/198 clean golden oracle, native/WASM transition parity, the
demo-web WASM check, warnings-denied rustdoc, benchmark compilation, M14/M32 performance budgets,
package/licence and Git-hygiene checks, the 256-moving-body sparse crossover in `152.49s`, and
Trunk 0.21.14 release assembly. Only the pre-existing non-failing Cargo `license` plus
`license-file` notices appeared.

The immutable replacement snapshot is `/tmp/geosolve-m70b-f005-uat.Q5c9Wi`; the directory is mode
`0555` and each of its exactly seven files is mode `0444`:

| File | SHA-256 |
| --- | --- |
| `API_COMPATIBILITY.md` | `af91333ed578f05ec49c76fd10c18dd0ead0f9f845b8ff45279de5a6cbc7b80e` |
| `LICENSE` | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-b2164249dc1c486.js` | `d018c92ee6f2d437244a28200c026e462da4585a9f808eabeea0b1208c26768f` |
| `geosolve-demo-web-b2164249dc1c486_bg.wasm` | `622a2f77e63574b624aecb94919994464f58671115cdbd4802283ada80c20907` |
| `index.html` | `5088006b11625fab097b3a38c6abad8d7cf0d3c3d91875b3fcf17626dbe34c1d` |
| `styles-36c74d05d21a90c9.css` | `49a0d71647856a30e798707860ffa9da4dbdbd1ec2f4faeafa412726f0e69048` |

The ordered manifest aggregate is
`3173fa529fa14fab5783cf4cb4733b17db5e6850ff5d6c63022fe712a0be4c7f`. PID `1841268` serves that
snapshot at `http://100.94.63.83:8080/` and is bound only to the Tailscale address. Proxy- and
cache-bypassed fetches proved that `/` matches `index.html` and every served asset byte-matches its
immutable local counterpart. This candidate supersedes the F003/F004 snapshot for the F005
movement-continuity recheck in `docs/M70B_UAT.md`. The supervising human reported that movement
behavior fixed and requested sign-off once the focused closing regressions were satisfactory.

### Closing multi-feature transaction audit

The broad golden remains unchanged at 198 rows because this is a sequence-level retained-
coordinator invariant, not a new static authoring or branch dimension. The focused regression
begins with two distinct computed features that are both `Current`, makes only one invalid during
projected dragging, and proves complete-scene withholding, paired last-valid scene
and release-coordinate retention, failing-feature-only attribution, reverse recovery and terminal
release of only the last valid sample. It is
`coordinator::tests::projected_drag_rejects_one_new_failure_beside_another_current_fillet` in
`crates/geosolve-constraint-editor/src/coordinator.rs`.

The public feature-owner CircularArc/affine permutation covers finite arcs as well as full circles.
Both parent orders carry a regular root beyond a stale Local witness without changing explicit
intent, independently validate finite incidence, radius, tangency, side and bounded-domain state,
and reject a same-orientation supporting-circle root that lies beyond the native arc endpoint. This
remains focused owner coverage, not a sixth golden row. The public integration regression is
`circular_arc_transport_crosses_stale_cell_and_stops_at_endpoint_in_both_orders` in
`crates/geosolve-sketch-features/tests/m70b_circular_arc_transport.rs`.

Clean source `48e3cc3` passes
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` with both additions. The
gate includes the unchanged 198/198 clean golden, 276/276 editor library tests, the 45/45 feature
library suite plus the new finite-arc integration test, all locked workspace tests, native/WASM
parity, warnings-denied Clippy and rustdoc, benchmark/package/licence checks, the 149.13-second
256-moving-body sparse crossover and Trunk 0.21.14 release assembly. The generated seven-file
distribution byte-matches `/tmp/geosolve-m70b-f005-uat.Q5c9Wi` at ordered-manifest aggregate
`3173fa529fa14fab5783cf4cb4733b17db5e6850ff5d6c63022fe712a0be4c7f`. PID `1841268` remains live
on the Tailscale-only endpoint, so no republish is required. The supervising human requested these
regressions and sign-off once satisfactory; that scoped decision closes M70B without claiming an
unrecorded exhaustive replay of every prepared UAT step.
