# FUZZING.md — GeoSolve fuzzing program

**Status:** in progress (cargo-fuzz path, explicitly chosen)
**Goal:** a comprehensive, coverage-guided fuzzing program across GeoSolve's *core layers*,
run in identify/hotfix mode for ~1 hour per fuzz area, then offloaded to a dedicated machine
(out of scope here).

---

## 0. Non-negotiables (from AGENTS.md)

- Pure Rust. No C/C++/Fortran *solver* FFI or SuiteSparse bindings in the library.
- Do not broaden the primitive/constraint set — fuzz only the **existing** family inventory
  (24 constraints + 5 dimensions + 6 fillet + 4 scene-authority = 271 golden rows).
- The solver may return success only after independent residual validation
  (`maximum_normalized_hard_residual <= 1e-9`, `hard_validity == Valid`).
- Never turn NaN/Inf or invalid geometry into convergence.
- Every bug involving convergence, rank, scaling, or a branch flip gets a regression scenario.
- Small, reviewable commits; keep `geosolve-sketch` / `geosolve-linkage` as separate domain
  models over `geosolve-core`; `geosolve-demo-web` stays a separate WASM crate.

The **fuzz harness** may use `extern "C"` / `unsafe` (libFuzzer ABI) — it is a *standalone*
`fuzz/` project, not a workspace member, so it does not inherit the workspace
`unsafe_code = forbid` lint. The library under test stays `unsafe`-free. If we later make
`fuzz/` a workspace member, an ADR is required first.

---

## 1. Framework — cargo-fuzz (libFuzzer)

- `cargo-fuzz 0.13.2` is installed. libFuzzer gives coverage-guided mutation, automatic crash
  minimization, and corpus management — the right signal for identify/hotfix mode.
- **Toolchain (findings 2026-09-21):** `rustc 1.82 nightly`; `cc`/`gcc` on PATH; `rustup`
  already has the `llvm-tools` component (for `libfuzzer-sys`). `clang`/`llvm-config` are NOT on
  PATH but are available in Nix:
  - clang: `/nix/store/*/bin/clang` (21.1.8 / 22.1.8)
  - llvm-config: `/nix/store/7xrdf0yy2766lpkbpmlmyg0b6brps38v-llvm-21.1.8-dev/bin/llvm-config`
  Resolution order:
  1. Run under a Nix shell that exposes clang + llvm-config, e.g.
     `nix-shell -p clang llvmPackages_21.llvm rustc cargo` (or extend `shell.nix` with `clang` /
     `llvmPackages_21.llvm` / `lld` — `lld` is already in `shell.nix`).
  2. If `cargo fuzz` still can't locate LLVM, point it explicitly:
     `export LLVM_TOOLS_DIR=/nix/store/...-llvm-21.1.8-dev/lib/llvm-21.1.8/lib` and set
     `CC`/`CXX`/`LLVM_CONFIG` to the Nix paths.
  3. Fallback (guaranteed now): in-process `proptest` (already vendored) — same harness,
     no minimization/coverage edge.
- End state on the dedicated machine: full `cargo-fuzz`. Build here, copy `fuzz/` there to run.

---

## 2. Architecture

Two units, clean dependency boundary, standalone so it is portable and does not inherit the
workspace lint/test gate.

```
crates/geosolve-survey/      # NEW shared core (pure Rust, NO unsafe, workspace member)
  src/
    family.rs                # 24 constraints + 5 dimensions + fillet + scene tables; FamilySubject
    variant.rs               # FuzzVariant = Variant + canonical LE codec; option_index ->
                             #          tangent%2 / curvature%3 / continuity%4
    fixture.rs               # MatrixFixture builder (from golden test, using public SketchDocument API)
    survey.rs                # survey(family, variant) -> SurveyOutcome
    outcome.rs               # SurveyOutcome + check() (acceptance policy) + audit descriptor
  tests/golden_parity.rs     # pins geosolve-survey to the golden TSV (no drift; golden test stays source of truth)

fuzz/                        # STANDALONE cargo-fuzz project (NOT a workspace member; unsafe allowed)
  Cargo.toml + fuzz.toml     # deps: geosolve-survey, geosolve-constraint-editor, geosolve-sketch, geosolve-core (path)
  src/
    model.rs                 # FuzzInput codec: decode arbitrary bytes -> (Family, Variant) + perturbation seed
                             #          (clamped/normalized; NEVER panics on decode)
    harness.rs               # run_survey() / perturb() / decode helpers; FuzzDefect
  fuzz_targets/
    01_authoring_survey.rs       # scope A
    02_fixture_perturbation.rs   # scope B
    03_core_solver.rs            # scope C (prototype)
    04_fillet.rs                 # optional
  corpus/
    01_authoring_survey/ 02_fixture_perturbation/ 03_core_solver/ 04_fillet/
```

Why a separate `geosolve-survey` crate (not a `pub mod` in constraint-editor): keeps
constraint-editor's public API untouched (lowest risk to the acceptance gate), crisp dependency
boundary, clearly "a fuzz crate". `golden_parity` guards against drift without touching the
existing golden test. Extraction of the golden test's helpers into `geosolve-survey` as the
single source of truth is a clean follow-up once the program is stable.

---

## 3. The fuzz signal — `SurveyOutcome::check()`

The signal is the oracle's own invariants (currently private in the golden test), centralized
here so the fuzzer asserts exactly AGENTS.md's convergence/rank/scaling/branch policy:

```rust
pub struct SurveyOutcome {
    pub family: Family, pub variant: FuzzVariant,
    pub accepted: bool,                       // solver returned success-like status
    pub hard_validity: SketchHardValidity,    // must == Valid
    pub residuals_validated: bool,            // must be true
    pub max_normalized_hard_residual: f64,    // must be <= 1e-9
    pub rank: i32, pub mobility: i32,
    pub finite: bool,                         // validate_finite_geometry (no NaN/Inf)
    pub geometry_ok: bool,                    // validate_constraint_geometry (branch/orientation)
}
impl SurveyOutcome {
    pub fn check(&self) -> Result<(), FuzzDefect> {
        if !self.accepted                         { return Err(Defect::Rejected); }
        if self.hard_validity != SketchHardValidity::Valid { return Err(Defect::Invalid); }
        if !self.residuals_validated              { return Err(Defect::ResidualsUnvalidated); }
        if self.max_normalized_hard_residual > 1e-9 { return Err(Defect::Residual(self.max_normalized_hard_residual)); }
        if !self.finite                           { return Err(Defect::NonFinite); }
        if !self.geometry_ok                      { return Err(Defect::Geometry); }
        Ok(())
    }
}
```

`check()` panicking in a fuzz target == a libFuzzer-found bug. Every such bug is by construction
a convergence/rank/scaling/branch-flip issue and therefore feeds the pipeline in §6.

---

## 4. Fuzz targets (layer × input space)

| Target | Layer | Input space | Signal |
|---|---|---|---|
| `01_authoring_survey` | constraint-editor authoring | **Family** (all 24+5+fillet+scene) × **Variant** (option-index latin square 2×3×4 × 8 seeds + deterministic) | `check()`; golden fingerprint on witnesses |
| `02_fixture_perturbation` | constraint-editor → core solver | adversarial numeric perturbation of `translation/scale/rotation/contact_parameter` + fixture geometry | `check()`; branch-stability under perturbation |
| `03_core_solver` (prototype) | geosolve-core direct | arbitrary `SketchDocument` (random points/curves/scalars) via `RetainedSketchDocumentSession` | accepted/finite/residual after solve |
| `04_fillet` (optional) | constraint-editor fillet | the 6 fillet cases × perturbation | fillet closure/acceptance |

- **`01` (scope A, priority):** decode `FuzzInput` → `(family, variant)`; seed corpus with the 271
  golden witnesses (re-encoded via the same `FuzzInput` codec) + the full option-index latin square
  per family. Assert `check()` + golden fingerprint for witnesses.
- **`02` (scope B, priority, highest value):** decode `FuzzInput`, apply a **deterministic,
  reproducible** structured perturbation derived from a byte suffix (so crashes reproduce exactly),
  then `check()`; plus a branch-stability check — the perturbed accepted geometry must agree with
  the unperturbed accepted geometry within tolerance (catches branch flips).
- **`03` (scope C, prototype):** random-document generator → retained-session solve → assert
  accepted/finite/residual. Deliberately simple; primary value is coverage of core solve paths and
  edge geometry.
- **`04` (optional):** reuse the structure of `golden_fillet_oracle.rs` if time permits.

### Input codec (`FuzzInput`)

Canonical layout (matches the oracle fingerprint's `to_bits().to_le_bytes()` convention for fp
fields): `[family: u32 LE] ++ [5 × f64 bits LE] ++ [reverse_spans, swap_operands, displaced] ++ [option_index]`.
Decoding **clamps/normalizes** (scale → positive, rotation mod 2π, contact → [0,1], flags masked,
family clamped into range) so arbitrary libFuzzer bytes never panic on decode. The golden witnesses
are encoded with this same codec → they become the seed corpus.

---

## 5. Corpus + golden integration

- `fuzz/corpus/*` is seeded from the golden witnesses — every family's `FuzzInput::golden_seeds()`
  (deterministic witness crossed with the full option-index/flag latin square) — via the
  `prefill_corpus` test in `fuzz/tests/`. Set `GOLDEN_CORPUS_DIR=fuzz/corpus/<target>` and run
  `cargo test --manifest-path fuzz/Cargo.toml --test prefill_corpus`; the test wipes any polluted
  corpus and writes the clean golden seeds. This is what makes scope A meaningful and ties fuzzing
  to the recorded oracle.
- `geosolve-survey::tests::golden_parity` reconstructs all 271 witnesses and asserts its survey
  output matches `crates/geosolve-constraint-editor/tests/fixtures/golden_authoring_scene_oracle.golden.tsv`.
- The golden test itself stays the source of truth and is left unmodified.

---

## 6. Fuzz-bug → regression pipeline (AGENTS.md compliance)

1. libFuzzer minimizes the crashing input.
2. Minimal hotfix in the affected layer (guard an unwrap, add a NaN/Inf check, fix a branch) —
   **no architecture changes**; do not overdo it.
3. Add a `proptest`/golden scenario reproducing the exact input → durable regression.

---

## 7. Barebones ≤1hr run + hotfix loop

```
# 1. toolchain (if needed)
rustup component add llvm-tools-preview   # then try: cargo fuzz --version

# 2. shared core
cargo test -p geosolve-survey --lib --test golden_parity

# 3. standalone fuzzer
cd fuzz
cargo fuzz add 01_authoring_survey 02_fixture_perturbation 03_core_solver 04_fillet
# Seed all four corpora from the golden witnesses (see fuzz/tests/prefill_corpus.rs).
for t in 01_authoring_survey 02_fixture_perturbation 03_core_solver 04_fillet; do
  GOLDEN_CORPUS_DIR="$PWD/corpus/$t" \
    cargo test --manifest-path fuzz/Cargo.toml --test prefill_corpus -- --exact
done
cargo fuzz run 01_authoring_survey      --runs 300000 --timeout 30
cargo fuzz run 02_fixture_perturbation  --runs 300000 --timeout 30
cargo fuzz run 03_core_solver           --runs 300000 --timeout 30
# (each area capped ~1hr; inspect crashes with `cargo fuzz find`, minimize with `cargo fuzz cmin`)
```

Loop per area: `run` → inspect/minimize → minimal hotfix → `cargo test -p geosolve-survey` + golden
check → continue. Cap ~1hr/area; defer remaining issues to the dedicated machine.

---

## 8. Acceptance criteria for the milestone

- `geosolve-survey` builds, no unsafe, passes `golden_parity` (271/271) and its own proptest suite.
- `fuzz/` builds (`cargo build`) and each target prefills a non-empty corpus from golden seeds.
- Each area fuzzed ≤1hr; every crash minimized + hotfixed + turned into a regression scenario.
- Workspace `fmt` / `clippy` / `test` still green after the shared-core addition.

---

## 9. Next actions (implementation order)

1. Provision LLVM toolchain (rustup component, then clang if needed).
2. Create `crates/geosolve-survey` (family, variant, fixture, survey, outcome) + `golden_parity` test.
3. Create standalone `fuzz/` project (Cargo.toml, fuzz.toml, src/model.rs, src/harness.rs).
4. Add the four fuzz targets + seed corpora.
5. Prefill + run each area ≤1hr; hotfix minimally; add regressions.
6. Update `PLAN.md` checkbox + a short note under the new Fuzzing milestone.
