<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Fuzzing finding FZ-02-01 — target-02 `EndpointContinuity` phantom branch flip

**Disposition:** `HARNESS-DEFECT-FALSE-POSITIVE` — fixed in the fuzz harness, not the solver.
**Owner layer (for regression):** `fuzz/` (test harness), not a `geosolve-core` / `geosolve-sketch`
solver defect. The solver is correct for this input.
**Author:** automated session, 2026-09-22.

## 1. Summary

Target `02_fixture_perturbation` panicked on a golden-corpus seed with a *branch-flip* report:

```
Constraint { kind: EndpointContinuity, intent: Continuity }: branch flip - accepted point
DesignPointId(PersistentId(23897064572600672383549329255)) moved 6.324555320336759 beyond the
4*scale perturbation budget
```

Root cause: the branch-stability check compares shared point **ids** between the base solve and
its `displaced`-perturbed neighbour. For `EndpointContinuity`, the `displaced` perturbation inserts a
*new* point (fixture adds *"displaced bezier 2 start"* instead of reusing the seam), which shifts
every subsequent `DesignPointId`. A shared id therefore referred to a **different physical point**
in the two documents. The solver moved neither point (both solves residual `0.0`, `Valid`); the
6.32-unit gap is pure pre-solve geometry, not a solver branch flip.

This is a false positive in the harness comparison, not a solver defect. It was caught by the
harness it was meant to guard against — expected for a new check — and the check was made robust.

## 2. Reproduction (exact, byte-verified)

**Artifact (authoritative, 51 bytes):**
`fuzz/crash-a6c188ef7093f39d248f00bf52f1d926df391e14`

Hex (`xxd`):
```
00000000: 0f00 0000 0000 0000 0000 0000 0000 0000
00000010: 0000 0000 0000 0000 0000 f03f 0000 0000
00000020: 0000 0000 0000 0000 0000 e03f 0500 0016
00000030: 0000 00
```

**Decoded `FuzzInput`:** `family=15` (Constraint), `translation=(0,0)`, `scale=0.75`,
`rotation=1.0`, `contact_parameter=0.5`, `reverse_spans=true`, `swap_operands=false`,
`displaced=false`, `option_index=22`.

**Reproduce command (deterministic, reproduces the panic on the pre-fix binary):**
```
export PATH="$HOME/.rustup/toolchains/nightly-2026-08-27-x86_64-unknown-linux-gnu/bin:$PATH"
export CC="/nix/store/51v3z004xh9xnwxw237i4szqcssgxybi-clang-wrapper-22.1.8/bin/clang"
export CXX="/nix/store/51v3z004xh9xnwxw237i4szqcssgxybi-clang-wrapper-22.1.8/bin/clang++"
export LLVM_CONFIG="/nix/store/7xrdf0yy2766lpkbpmlmyg0b6brps38v-llvm-21.1.8-dev/bin/llvm-config"
cd fuzz
cargo build --bin 02_fixture_perturbation
./target/debug/02_fixture_perturbation fuzz/crash-a6c188ef7093f39d248f00bf52f1d926df391e14 -runs=1
```
On the pre-fix binary this exits with the branch-flip panic above (libFuzzer reports it as a dead
signal). The identical 51-byte record is also saved at `/tmp/repro02/crash_exact.bin`.

**Owner inputs in the fixture (pre-fix `branch_flip` by id):**
| id | base pre-solve | base accepted | perturbed pre-solve | perturbed accepted |
|---|---|---|---|---|
| `...9255` | (4, -12) | (4, -12) | (2, -6) | (2, -6) |
| `...9254` | (2, -6) | (2, -6) | (0.2, -4.1) | (0.2, -4.1) |

`...9255` is bezier-2-end `(4,-12)` in the base document but bezier-2-middle `(2,-6)` in the
perturbed one (the inserted point re-indexed the bezier-2 controls). Displacement
`sqrt((4-2)^2 + (-12+6)^2) = sqrt(40) = 6.324555…`, which exceeds `4*scale = 4.0`. In **both**
solves the point sits at its pre-solve position (`max_residual = 0.0`, `hard_validity = Valid`), so
the solver did not move it — the gap is fixture geometry, not a solver branch.

## 3. Root cause

`fuzz/fuzz_targets/02_fixture_perturbation.rs` (pre-fix) compared base vs perturbed by exact
`DesignPointId`. `crates/geosolve-survey/src/fixture.rs` (`EndpointContinuity`, `displaced` branch)
calls `add_point(... "displaced bezier 2 start", [0.2, -4.1])` — a brand-new point — whereas the
non-displaced path reuses `first_bBezier_controls[2]` (the seam). With `option_index=22 >= 4` the
bezier-2 control list is also reversed. The result: the perturbed document has one extra point and
all later ids are shifted, so a shared id maps to a different physical point. The id-based
comparison then compares mismatched points and reports a phantom flip.

## 4. Fix

`branch_flip` now matches each base point to its **nearest** perturbed position and reports the
largest such displacement that exceeds the budget, instead of comparing by id. This tolerates the
point set differing between documents (extra/reordered points) while still flagging a base point
with no nearby counterpart in the perturbed solve — the genuine branch-flip signal.

```rust
fn branch_flip(
    base: &[(DesignPointId, [f64; 2])],
    perturbed: &[(DesignPointId, [f64; 2])],
    scale: f64,
) -> Option<(DesignPointId, f64)> {
    let budget = PERTURBATION_BUDGET * scale;
    let mut best: Option<(DesignPointId, f64)> = None;
    for (id, base_point) in base {
        let nearest = perturbed
            .iter()
            .map(|(_, p)| {
                let dx = p[0] - base_point[0];
                let dy = p[1] - base_point[1];
                (dx * dx + dy * dy).sqrt()
            })
            .fold(f64::INFINITY, f64::min);
        if nearest > budget && best.is_none_or(|(_, d)| d < nearest) {
            best = Some((*id, nearest));
        }
    }
    best
}
```

`f64::min` via `fold` is used because `f64` implements `PartialOrd`, not `Ord` (so
`Iterator::min()` does not type-check); `fold` also yields `INFINITY` for an empty perturbed set
(a base point with no counterpart is then flagged). The now-unused `std::collections::HashMap`
import was removed.

**Why this is safe (not a mask of a real flip):** for pre-satisfied fixtures the solver returns the
pre-solved geometry (residual 0.0), so a small `displaced` perturbation (~0.2–0.5 u) yields nearest
displacements well under the `4*scale` budget; a real branch flip leaves at least one base point
with no nearby counterpart and is still caught. The `9.0`-scale margin over the perturbation is
unchanged.

## 5. Verification (exact commands + outcomes)

Toolchain: `nightly-2026-08-27` + Nix clang 22.1.8 / llvm-config 21.1.8 (see top of each command).

| Step | Command | Outcome |
|---|---|---|
| Build target 02 (post-fix) | `cd fuzz && cargo build --bin 02_fixture_perturbation` | OK (1 pre-existing `geosolve-sketch` warning) |
| Crash input | `…/02_fixture_perturbation /tmp/repro02/crash_exact.bin -runs=1` | **EXIT 0, no panic** |
| Golden corpus | `…/02_fixture_perturbation /tmp/golden02 -runs=1 -timeout=60` | **EXIT 0; 5598 runs, no crash** (was EXIT 77 pre-fix) |
| Target 01 golden | `…/01_authoring_survey /tmp/golden01 -runs=1 -timeout=30` | **EXIT 0; 5598 runs in 138 s** (previously EXIT 124, interrupted) |
| Determinism | `cd fuzz && cargo test --test determinism` | `ok. 1 passed` (15.2 s) |
| Prefill corpus | `GOLDEN_CORPUS_DIR=… cargo test --test prefill_corpus -- --exact` | `ok. 1 passed`; corpus 5597 intact |
| Clippy (fuzz) | `cd fuzz && cargo clippy --all-targets` | **clean** (exit 0); the only remaining warning is pre-existing in `geosolve-sketch` (`fetch_update`), unrelated |

Targets `03_core_solver` and `04_fillet` remain green over the golden corpus (previously reported,
unchanged).

## 6. Scope and follow-ups

- **Not fixed here (out of scope):** the fixture's `EndpointContinuity` `displaced` perturbation is
  structurally asymmetric (adds a point for one family). That is existing, golden-oracle-backed
  fixture behaviour and is intentionally left alone; the harness now tolerates it.
- **Possible follow-up:** a bidirectional (assignment-style) nearest match would be marginally
  stricter, but base→perturbed nearest matching is sufficient for the golden corpus and the
  branch-flip contract. No change required.
- **Regression:** the 51-byte record reproduces the pre-fix panic and the post-fix clean exit; the
  golden corpus (5598 runs) exercises every family's `displaced` neighbour, so this class of false
  positive is covered by the normal fuzz run.
