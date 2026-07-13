# OpenCode overnight handoff

## Objective

Implement the difficult but objectively testable solver core overnight, then stop before domain/API/interaction decisions require human review.

Read these first:

- `ARCHITECTURE.md`
- `PLAN.md`
- `ACCEPTANCE.md`
- `REFERENCES.md`
- `docs/SCENARIOS.md`
- `AGENTS.md`

## Overnight assignment

Complete **M1, M2, M3 and M4** in `PLAN.md`, in order, then stop at **Human checkpoint A**.

### M1: problem representation

Expected result:

- packed scalar, `Vec2` and `Pose2` variable blocks with stable IDs;
- residual blocks with declared incidence/source IDs;
- hard/temporary/preference categories;
- structured equation-audit descriptors;
- dimensionless scaling;
- dense residual/Jacobian assembly;
- central finite-difference Jacobian verification;
- invalid/non-finite input rejection.

### M2: dense solver

Expected result:

- damped Gauss-Newton or Levenberg-Marquardt iteration;
- dense QR/SVD fallback and rank/DOF calculation;
- strict independent hard-residual validation;
- truthful `SolveTermination` and independent diagnostics;
- deterministic traces and accepted-state equation audit snapshots;
- exact, underdetermined, inconsistent, rank-deficient and badly scaled fixtures.

### M3: adversarial verification

Expected result:

- property-generated systems with known solution/rank/nullity;
- construct-valid → perturb → recover tests;
- translation/rotation/scale metamorphic tests;
- ordering/permutation determinism tests;
- failure-injection and last-valid-state tests;
- solve-trace invariant checks;
- reproducible random seeds on failure.

### M4: decomposition and diagnostics

Expected result:

- variable/residual incidence graph and connected components;
- fixed/equality alias elimination;
- unaffected-component reuse;
- structural counts plus numerical rank;
- deterministic redundant/conflict source candidates on synthetic systems;
- audit annotations for eliminated/redundant/conflicting/singular rows.

## Autonomous execution rules

1. Complete and gate one milestone before beginning the next.
2. Make one reviewable Git commit per completed milestone.
3. If a gate fails, fix it before proceeding; never weaken a tolerance or delete a test merely to pass.
4. Keep new APIs private or `pub(crate)` unless the milestone explicitly requires public exposure.
5. Prefer small explicit implementations over generic frameworks.
6. Record architectural uncertainty in `OVERNIGHT_REPORT.md`; do not invent broad abstractions to resolve it.
7. Stop immediately if satisfying a milestone would require:
   - a public sketch/linkage model decision;
   - weighted objectives silently replacing hard constraints;
   - native solver FFI or `unsafe`;
   - sparse solving;
   - browser UX work;
   - circles/arcs/tangency;
   - pseudo-arclength continuation;
   - spatial `SE(3)` mechanisms.
8. Do not begin M5 even if time remains.

## Gate after every milestone

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p geosolve-demo-web --target wasm32-unknown-unknown
(cd crates/geosolve-demo-web && trunk build --release)
```

## Final overnight report

Create `OVERNIGHT_REPORT.md` containing:

1. milestone and commit list;
2. files/APIs added;
3. algorithms implemented and important numerical choices;
4. exact verification commands and outcomes;
5. acceptance criteria passed or still failing;
6. deterministic reproduction commands/seeds for any failure;
7. public API or behavior decisions deferred to Human checkpoint A;
8. concise diff/stat and current Git status.

Then stop and wait for review.
