#!/usr/bin/env bash
#
# Deep-mutation fuzzing campaign for the four geosolve libFuzzer targets.
#
# Runs every target in parallel, each pinned to a dedicated subset of CPU cores
# via `taskset`, and writes all results (per-target corpus, crash artifacts,
# build cache and run logs) under <out-root>/ in the CURRENT WORKING DIRECTORY.
#
# Usage:
#   scripts/fuzz-campaign.sh [OUT_ROOT]
#
# Configuration (environment variables):
#   FUZZ_RESERVE         cores kept free for the OS            (default: 2)
#   FUZZ_OUT_ROOT        results directory name                (default: results)
#   FUZZ_DURATION        default per-target wall clock         (default: run until stopped)
#                        value forms: 12h / 30m / 3600s
#   FUZZ_RUNS_<target>   -runs budget for one target          (default: unlimited)
#   FUZZ_DURATION_<target> wall clock for one target          (default: $FUZZ_DURATION)
#   FUZZ_FORKS_<target>  worker count for one target          (default: see CAMPAIGN)
#
# Examples:
#   # 12h per target on the default core split
#   FUZZ_DURATION=12h ./scripts/fuzz-campaign.sh
#   # named output dir, custom budget on one target
#   FUZZ_OUT_ROOT=campaign2 FUZZ_RUNS_03_core_solver=50000000 ./scripts/fuzz-campaign.sh
#
set -uo pipefail

# --------------------------------------------------------------------------
# Configuration
# --------------------------------------------------------------------------
OUT_ROOT="${FUZZ_OUT_ROOT:-${1:-results}}"
FUZZ_RESERVE="${FUZZ_RESERVE:-2}"

# <target>|<forks>  (forks = libFuzzer workers pinned to this target's cores)
# Default split for a 36-core host with 2 cores reserved for the OS:
#   7 + 10 + 10 + 7 = 34 usable cores.
CAMPAIGN=(
  "01_authoring_survey|7"
  "02_fixture_perturbation|10"
  "03_core_solver|10"
  "04_fillet|7"
)

# libFuzzer flags shared by every target.
LIBFUZZER_ARGS=( -timeout=30 -detect_leaks=1 -print_final_stats=1 )

# --------------------------------------------------------------------------
# Locate the repository (this script lives at <root>/scripts/)
# --------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." >/dev/null 2>&1 && pwd)"
FUZZ_DIR="$REPO_ROOT/fuzz"
CRATES_DIR="$REPO_ROOT/crates"

for tool in cargo cargo-fuzz taskset timeout pgrep; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "ERROR: required tool '$tool' not found on PATH" >&2
    exit 127
  }
done

# `nproc` reports ONLINE cores; taskset affinity only accepts online CPUs, so the
# CPU map must be built from this set (not `nproc --all`, which counts offline ones).
if command -v nproc >/dev/null 2>&1; then CORES="$(nproc)"; else CORES="$(getconf _NPROCESSORS_ONLN)"; fi
USABLE=$(( CORES - FUZZ_RESERVE ))
echo "cores=$CORES reserve=$FUZZ_RESERVE usable=$USABLE"

# --------------------------------------------------------------------------
# Build the working set, applying per-target fork overrides.
# --------------------------------------------------------------------------
names=(); forks=()
for entry in "${CAMPAIGN[@]}"; do
  IFS='|' read -r name fork <<<"$entry"
  var="FUZZ_FORKS_$name"
  if [ -n "${!var:-}" ]; then fork="${!var}"; fi
  names+=("$name"); forks+=("$fork")
done

total=0; for f in "${forks[@]}"; do total=$(( total + f )); done
# The CPU map below is a sequential, non-overlapping split of cores 0..total-1, so
# every target keeps a distinct range. That only fits if total <= usable cores; if
# the campaign's fork budget exceeds the online cores, shrink the largest targets
# first (each keeps >= 1 worker) until the split fits. This keeps every assigned
# core online, so `taskset -c <range>` never fails on an out-of-range list.
while (( total > USABLE && total > ${#names[@]} )); do
  max_i=0
  for i in "${!forks[@]}"; do
    (( forks[i] > forks[max_i] )) && max_i=$i
  done
  (( forks[max_i] > 1 )) || break
  forks[max_i]=$(( forks[max_i] - 1 ))
  total=$(( total - 1 ))
done
if (( total > USABLE )); then
  echo "WARNING: forks ($total) exceed usable cores ($USABLE); will oversubscribe." >&2
fi

# --------------------------------------------------------------------------
# Signal handling: tear down every target's process tree on Ctrl-C / TERM.
# --------------------------------------------------------------------------
PIDS=()
stopping=0
kill_subtree() {
  local pid="$1" sig="${2:-TERM}" kid
  for kid in $(pgrep -P "$pid" 2>/dev/null); do kill_subtree "$kid" "$sig"; done
  kill "-$sig" "$pid" 2>/dev/null || true
}
shutdown() {
  (( stopping )) && return
  stopping=1
  echo "Shutdown requested; stopping ${#PIDS[@]} targets..." >&2
  for pid in "${PIDS[@]}"; do kill_subtree "$pid" TERM; done
  wait 2>/dev/null
  echo "All targets stopped." >&2
  exit "${1:-130}"
}
trap 'shutdown 130' INT TERM

# --------------------------------------------------------------------------
# Seed a target's corpus from the golden seeds (regenerating them if needed).
# --------------------------------------------------------------------------
seed_corpus() {
  local name="$1" crate="$2"
  local src="$FUZZ_DIR/corpus/$name"
  local dst="$crate/corpus/$name"
  if ls "$src"/* >/dev/null 2>&1; then
    mkdir -p "$dst"
    cp -f "$src"/* "$dst/" 2>/dev/null || true
    echo "  seeded $name from repo corpus ($(find "$dst" -type f | wc -l) files)"
  else
    echo "  repo corpus empty for $name; running prefill_corpus test..."
    GOLDEN_CORPUS_DIR="$(pwd)/$dst" \
      cargo test --manifest-path "$crate/Cargo.toml" --test prefill_corpus -- --exact prefill_golden_corpus >/dev/null 2>&1
    echo "  prefilled $name ($(find "$dst" -type f 2>/dev/null | wc -l) files)"
  fi
}

# --------------------------------------------------------------------------
# Launch one target in its own process group, writing everything under $work.
# --------------------------------------------------------------------------
launch_target() {
  local name="$1" fork="$2" cpus="$3" crate="$4" work="$5" log="$6" runs="$7" duration="$8"

  local lf=( -fork="$fork" -jobs="$fork" -workers="$fork" "${LIBFUZZER_ARGS[@]}" -artifact_prefix="$work/artifacts/" )
  [ -n "$runs" ] && lf+=( -runs="$runs" )

  local launch=( taskset -c "$cpus" )
  [ -n "$duration" ] && launch+=( timeout "$duration" )
  launch+=( cargo fuzz run -s none --fuzz-dir "$crate" "$name" -- )
  launch+=( "${lf[@]}" )

  {
    echo "== $(date -Is) start"
    echo "target=$name forks=$fork cpus=$cpus runs=${runs:-inf} duration=${duration:-inf}"
    echo "manifest=$crate/Cargo.toml out=$work"
    echo "cmd: ${launch[*]}"
  } >"$log" 2>&1

  # setsid -> new process group; $! is the group leader so `kill -- -<pid>`
  # (used by kill_subtree) reaches every worker.
  setsid bash -c 'exec "$@"' _ "${launch[@]}" >>"$log" 2>&1 &
  PIDS+=("$!")
  printf '  %-26s forks=%-2s cpus=%-8s runs=%-8s duration=%-6s (pid %s)\n' \
    "$name" "$fork" "$cpus" "${runs:-inf}" "${duration:-inf}" "${PIDS[-1]}"
}

# --------------------------------------------------------------------------
# Launch everything.
# --------------------------------------------------------------------------
rm -rf "$OUT_ROOT"
mkdir -p "$OUT_ROOT"

core_cursor=0
for i in "${!names[@]}"; do
  name="${names[$i]}"; fork="${forks[$i]}"
  start=$core_cursor
  end=$(( core_cursor + fork - 1 ))
  core_cursor=$(( core_cursor + fork ))
  cpus="$start-$end"

  work="$OUT_ROOT/$name"
  crate="$work/fuzz"
  mkdir -p "$crate"

  # The fuzz crate resolves its geosolve deps via `path = "../crates/*"`, so the
  # domain crates must sit beside it (results/<target>/crates/), not inside the
  # fuzz crate. Copy them once per target; keep the build cache at $crate/target/
  # across runs.
  cp -rf "$CRATES_DIR" "$work/"

  # The domain crates inherit `edition`, lints and `[workspace.dependencies]`
  # from the workspace root manifest, so that root Cargo.toml must sit above
  # them (results/<target>/Cargo.toml). Copy it once per target.
  cp -f "$REPO_ROOT/Cargo.toml" "$work/" 2>/dev/null

  # Refresh source only; keep the build cache at $crate/target/ across runs.
  cp -f "$FUZZ_DIR/Cargo.toml" "$FUZZ_DIR/Cargo.lock" "$FUZZ_DIR/fuzz.toml" "$FUZZ_DIR/.gitignore" "$crate/" 2>/dev/null
  cp -rf "$FUZZ_DIR/src" "$FUZZ_DIR/fuzz_targets" "$FUZZ_DIR/tests" "$crate/"

  # Fresh corpus + artifacts each run. libFuzzer (esp. in -fork mode) requires the
  # `-artifact_prefix` directory to already exist, so create it or every fork worker
  # fails to start with "The required directory ... does not exist".
  rm -rf "$crate/corpus" "$crate/artifacts" "$work/artifacts"
  mkdir -p "$crate/corpus" "$work/artifacts"
  seed_corpus "$name" "$crate"

  runs_var="FUZZ_RUNS_$name"
  duration_var="FUZZ_DURATION_$name"
  launch_target "$name" "$fork" "$cpus" "$crate" "$work" "$work/run.log" \
    "${!runs_var:-}" "${!duration_var:-${FUZZ_DURATION:-}}"
done

echo
echo "All targets launched. Results under $OUT_ROOT/"
echo "  log:      $OUT_ROOT/<target>/run.log"
echo "  corpus:   $OUT_ROOT/<target>/fuzz/corpus/<target>/"
echo "  artifacts: $OUT_ROOT/<target>/artifacts/"
echo "  build cache: $OUT_ROOT/<target>/fuzz/target/  (persisted between runs)"
echo "Ctrl-C to stop early."

wait
echo
echo "Campaign complete. Results under $OUT_ROOT/"
