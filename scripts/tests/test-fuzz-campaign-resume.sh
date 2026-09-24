#!/usr/bin/env bash
# Regression test: fuzz-campaign resumability + read-only-source robustness.
# Exercises resume/fresh/force-fresh paths with FAKE cargo/tools (no Rust build).
# Also runs the whole scenario against READ-ONLY source crates (mimics the
# `nix run` case where sources are copied from an immutable Nix store) to prove
# the resume/refresh force the working tree writable.
set -uo pipefail

ORIGIN_SCRIPT="$(cd "$(dirname "$0")/../.." && pwd)/scripts/fuzz-campaign.sh"
TESTROOT="$(mktemp -d)"
FAKEBIN="$TESTROOT/bin"
REPO="$TESTROOT/repo"
mkdir -p "$FAKEBIN" "$REPO/scripts" "$REPO/crates/geo-core" \
         "$REPO/fuzz/src" "$REPO/fuzz/fuzz_targets" "$REPO/fuzz/tests" "$REPO/fuzz/corpus"

# ---------------------------------------------------------------- fake tools
# `exec "$@"` breaks when the command list starts with a dash option
# (e.g. `taskset -c ...`), so these strip their own leading options first.
printf '#!/usr/bin/env bash\necho 4\n' > "$FAKEBIN/nproc"
printf '#!/usr/bin/env bash\nexec "$@"\n' > "$FAKEBIN/cargo-fuzz"
# taskset -c <cpulist> <cmd...>  (script name is NOT in $@; use [[ ]] so a leading -c isn't a test op)
printf '#!/usr/bin/env bash\n[[ $1 == "-c" ]] && { shift; shift; }\nexec "$@"\n' > "$FAKEBIN/taskset"
# timeout <duration> <cmd...>
printf '#!/usr/bin/env bash\nshift\nexec "$@"\n' > "$FAKEBIN/timeout"
# pgrep: no children (kill_subtree safety)
printf '#!/usr/bin/env bash\nexit 1\n' > "$FAKEBIN/pgrep"
cat > "$FAKEBIN/cargo" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  test)
    # prefill_corpus: write golden seeds into $GOLDEN_CORPUS_DIR
    if [ -n "${GOLDEN_CORPUS_DIR:-}" ]; then
      mkdir -p "$GOLDEN_CORPUS_DIR"
      for i in 1 2 3; do printf 'seed %s' "$i" > "$GOLDEN_CORPUS_DIR/seed_$i.bin"; done
    fi
    ;;
  fuzz)
    shift
    crate=""; name=""
    while [ $# -gt 0 ]; do
      case "$1" in
        --fuzz-dir) shift; crate="$1"; shift; name="$1" ;;
        --) break ;;
      esac
      shift
    done
    # simulate a discovery: add one file to this target's corpus
    if [ -n "$crate" ] && [ -n "$name" ]; then
      mkdir -p "$crate/corpus/$name"
      printf 'discovered %s' "$(date +%s%N)" > "$crate/corpus/$name/discovered_$(date +%s%N).bin"
    fi
    ;;
esac
exit 0
EOF
chmod +x "$FAKEBIN"/*

# ---------------------------------------------------------------- minimal repo
cp "$ORIGIN_SCRIPT" "$REPO/scripts/fuzz-campaign.sh"
printf '[workspace]\n' > "$REPO/Cargo.toml"
: > "$REPO/fuzz/Cargo.lock"; : > "$REPO/fuzz/fuzz.toml"; : > "$REPO/fuzz/.gitignore"
# repo corpus is EMPTY -> seed_corpus must take the prefill path (like gitignored seeds)

cd "$REPO"
export PATH="$FAKEBIN:$PATH"
TARGET=03_core_solver
CORPUS="corpus_dir"

pass=0; fail=0
chk() { # chk "desc" <1=pass|0=fail>
  if [ "$2" -eq 1 ]; then echo "  PASS: $1"; pass=$((pass+1)); else echo "  FAIL: $1"; fail=$((fail+1)); fi
}

# run one scenario: <label> <out_root> [readonly_sources]
# runs fresh -> resume -> force-fresh and asserts the resume contract.
run_scenario() {
  local label="$1" out="$2" ro="${3:-}"
  local cpus="$out/$TARGET/fuzz/corpus/$TARGET"
  if [ -n "$ro" ]; then chmod -R a-w "$REPO/crates"; chmod 555 -R "$REPO/crates"; fi

  echo "=== SCENARIO [$label] (out_root=$out, sources: $([ -n "$ro" ] && echo READ-ONLY || echo writable)) ==="

  echo "  -- fresh --"
  local O1; O1="$(cd "$REPO" && FUZZ_OUT_ROOT="$out" FUZZ_DURATION=1s bash scripts/fuzz-campaign.sh 2>&1)"
  local c1; c1="$(find "$cpus" -type f 2>/dev/null | wc -l)"
  echo "  corpus after fresh: $c1"
  mkdir -p "$out/$TARGET/fuzz/target"
  echo RUN1 > "$out/$TARGET/fuzz/target/marker.txt"

  echo "  -- resume --"
  sleep 1
  local O2; O2="$(cd "$REPO" && FUZZ_OUT_ROOT="$out" FUZZ_DURATION=1s bash scripts/fuzz-campaign.sh 2>&1)"
  local c2; c2="$(find "$cpus" -type f 2>/dev/null | wc -l)"
  echo "  corpus after resume: $c2"
  local marker2; marker2="$(cat "$out/$TARGET/fuzz/target/marker.txt" 2>/dev/null)"

  echo "  -- force-fresh --"
  local O3; O3="$(cd "$REPO" && FUZZ_OUT_ROOT="$out" FUZZ_FORCE_FRESH=1 FUZZ_DURATION=1s bash scripts/fuzz-campaign.sh 2>&1)"
  local c3; c3="$(find "$cpus" -type f 2>/dev/null | wc -l)"
  echo "  corpus after force-fresh: $c3"

  echo "  --- assertions ---"
  echo "$O1" | grep -q "prefill_corpus test" && a=1 || a=0; chk "$label fresh: prefill path" "$a"
  echo "$O1" | grep -q "prefilled $TARGET"   && a=1 || a=0; chk "$label fresh: seeded golden seeds" "$a"
  [ "$c1" -ge 3 ] && a=1 || a=0; chk "$label fresh: >=3 golden seeds" "$a"
  echo "$O2" | grep -q "prefill_corpus test" && a=0 || a=1; chk "$label resume: no re-seed" "$a"
  echo "$O2" | grep -q "resume $TARGET from existing corpus" && a=1 || a=0; chk "$label resume: chose RESUME path" "$a"
  [ "$c2" -gt "$c1" ] && a=1 || a=0; chk "$label resume: corpus grew (not reset)" "$a"
  [ "$marker2" = "RUN1" ] && a=1 || a=0; chk "$label resume: build cache preserved" "$a"
  echo "$O3" | grep -q "prefilled $TARGET" && a=1 || a=0; chk "$label force-fresh: re-seeded" "$a"
  # read-only-source specific: the working tree must be writable (chmod fix) so
  # resume/refresh don't fail with Permission denied.
  if [ -n "$ro" ]; then
    [ -d "$out/$TARGET/crates" ] && a=1 || a=0; chk "$label: crates tree refreshed" "$a"
    test -w "$out/$TARGET/crates" && a=1 || a=0; chk "$label: crates tree writable" "$a"
  fi
}

run_scenario "writable" "results_rw"
run_scenario "readonly" "results_ro" readonly

echo
echo "RESULT: pass=$pass fail=$fail"
# clean OUT_ROOTs and source crates (read-only in the readonly scenario)
chmod -R u+rwX "$REPO/results_rw" "$REPO/results_ro" "$REPO/crates" 2>/dev/null || true
rm -rf "$TESTROOT"
[ "$fail" -eq 0 ]
