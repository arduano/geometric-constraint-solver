<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M35 cancellation latency

Cancellation is cooperative. Wall time is evidence only and never controls a solve,
profile result, work limit, or publication decision. The deterministic checkpoint and
work-counter report remains the authoritative outcome.

## Reproduction

The ignored `measure_native_profile_cancellation_latency` regression scans candidate
pairs for 1,000 disjoint line spans. A host thread requests cancellation after the
worker enters the public profile operation. The probe records request-to-return time
for 20 independent runs and verifies that every return is the typed `Cancelled`
outcome. Run it on an otherwise idle machine with:

```text
cargo test --release --locked -p geosolve-sketch --test m35 \
  measure_native_profile_cancellation_latency -- --ignored --nocapture
```

Record the CPU, operating system, Rust version, build profile, run count, and maximum
printed latency when refreshing this evidence. The result bounds the candidate-loop
checkpoint path on that machine; it is not a portable deadline guarantee. Dense
factorization and rank kernels are non-interruptible internally, so their cancellation
boundary is the documented `Before*`/`After*` checkpoint pair. Deterministic
`Factorizations` and `RankKernels` limits authorize each such kernel before it starts.
Controlled dense kernels additionally authorize both matrix dimensions before execution.

The ignored core `measure_native_dense_kernel_boundary_latency` regression measures those
exact controlled wrappers. Its fixed representative input bound is one dense 256-by-256
finite matrix: factorization solves one 256-row right-hand side with QR, and rank diagnostics
run the production dense SVD path. It records the maximum complete `Before*`-to-`After*`
window over 20 runs. This is the non-interruptible cancellation window when a request races
immediately after kernel authorization. Reproduce it with:

```text
cargo test --release --locked -p geosolve-core --lib \
  measure_native_dense_kernel_boundary_latency -- --ignored --nocapture
```

The 256-by-256 dimensions are the supported M35 hard bound for controlled dense kernels, not
only a representative evidence workload. `OperationController` clamps each configured dense
row and column limit to 256; callers may choose lower limits but cannot raise this bound. A
controlled dense factorization or rank kernel whose row or column dimension exceeds its
effective limit returns typed `WorkExhausted` evidence before entering the kernel and cannot
publish a result. Legacy operations without an `OperationControl` remain unrestricted. M102X may
replace or broaden this policy only with new bounded-kernel evidence and contract updates.

## Reference run

On 2026-07-23 the exact command above completed all 20 typed-cancellation runs with a
maximum request-to-return latency of **12.623086 ms**. The machine was an Intel Core
i5-14400F (10 cores, 16 logical CPUs), Linux 7.1.1 x86-64, using
`rustc 1.97.1 (8bab26f4f 2026-07-14)` and the Cargo release profile. This is retained
as measured host evidence, not as a correctness threshold or portable guarantee.

On the same host and date, the exact dense-kernel command above completed 20 release runs.
The maximum controlled 256-by-256 QR factorization window was **2.687691 ms**, and the
maximum controlled 256-by-256 rank-SVD window was **7.323588 ms**. Both probes verified the
corresponding exact deterministic counter was one for every measured run.
