<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B-H1 test hardening and defect survey

Status: the bounded H1 test-only survey, complete release qualification and byte-verified UAT
publication remain historically clean. Subsequent human UAT opened `M70B-F003` in computed-Fillet
authoring outside the 193-row matrix. Its focused headless characterization changes no runtime
solver, sketch, authoring or workbench behavior. Supervising-human M70B review and approval remain
pending.

M70B-H1 originated this matrix. M70B-H2 moves the unchanged 193-row corpus and driver to
milestone-neutral names so later findings can reuse it without rewriting H1 history. The original
golden bytes and SHA-256 remain unchanged. H2 clean qualification and independent skill forward
tests pass on source `47584bdb607c722df508eae56584726954a03205`.

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

## Driver and classification contract

`scripts/golden-authoring-scene-oracle.sh` runs every authoring and scene row in its own process with a
30-second runtime limit and a five-second hard-kill grace period. Semantic failures, panics,
timeouts (`124` or hard-kill `137`) and harness errors are rows rather than an instruction to stop;
later rows still run. A nonzero child exit is never accepted merely because it wrote a complete
TSV. Child output must match the requested case and family, and the final inventory must contain
the exact 193 `(case_id, family)` pairs.

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
`--require-clean` additionally fails if any recorded row is not `PASS`. Scratch output lives under
the ignored workspace `target/` tree so a full system `/tmp` cannot turn semantic results into
false harness failures. `GEOSOLVE_GOLDEN_ORACLE_CASE` selects exactly one row inside each child.
Every authoring PASS fingerprint is `input-<fnv1a64>` over the effective post-scheduling variant;
the golden therefore detects seed/scheduling drift instead of recording an uninformative `ok`.

If a future row fails, preserve its family, case ID, fixed seed, minimized variant fingerprint and
exact family command. Deduplicate common root causes, assign the next active-milestone `M*-F*`
identity only after independent reproduction and add a `GEOSOLVE_REPRO_V1` payload whenever the
fixture can be represented as a workbench workspace.
Do not weaken an oracle or implement a production correction during the discovery phase.

## Survey result and readable defect checklist

The completed matrix still records 193/193 `PASS`, zero semantic defects, zero panics, zero
timeouts and zero harness errors within its declared scope. It opened no finding at H1 survey time.
Later human UAT opened `M70B-F003`: the matrix has no computed-Fillet authoring rows, so its green
result cannot detect a Coincident-closed triangle closure rejected through both point and curve-pair
collection. The compact public-Rust fixture needs no reproduction payload and is retained in the
focused owner regression instead.

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
payload-derived regressions, and open M70B-F003 retains its focused headless characterization.
M70B remains active until explicit supervising-human UAT approval.

Focused F003 evidence:

```text
cargo test --locked -p geosolve-constraint-editor \
  --test m70b_closed_triangle_fillet \
  m70b_f003_coincident_triangle_closure_is_not_filletable_by_point_or_curve_pair \
  -- --exact --nocapture
./scripts/golden-authoring-scene-oracle.sh --check
```

The first command passes by asserting the exact open-defect signature and transactional retention;
it must be converted to positive success expectations during an authorized repair. The second
remains 193/193 green and therefore demonstrates the broad matrix's computed-feature-authoring
blind spot rather than resolution of F003.

## Qualification ledger

Focused commands completed so far:

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

Observed outcomes are 1/1 inventory pass, 193/193 survey rows pass, exact golden check pass,
clean-oracle gate pass, 4/4 scene rows pass, the complete editor crate pass (271 unit tests plus all
named integration suites, including the 2/2 oracle harness), demo-web 97/97 library plus 1/1 native
decoder pass, focused warnings-denied Clippy pass, WASM test compilation, formatting pass and diff
check pass. The final golden SHA-256 is
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
