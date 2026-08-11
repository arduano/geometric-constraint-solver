#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
golden="$root/crates/geosolve-constraint-editor/tests/fixtures/m70b_hardening_oracle.golden.tsv"
header=$'case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint'
timeout_seconds=30

usage() {
  printf '%s\n' \
    'usage: scripts/m70b-hardening-oracle.sh --survey|--check|--require-clean' >&2
}

if [[ $# -ne 1 ]]; then
  usage
  exit 2
fi
mode="$1"
case "$mode" in
  --survey | --check | --require-clean) ;;
  *)
    usage
    exit 2
    ;;
esac

scratch_parent="$root/target/m70b-hardening"
mkdir -p "$scratch_parent"
scratch="$(mktemp -d "$scratch_parent/run.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
export TMPDIR="$scratch"
rows="$scratch/rows.tsv"
actual="$scratch/actual.tsv"
classified="$scratch/classified.tsv"
: >"$rows"

families=(
  constraint.fixed-point
  constraint.coincident-points
  constraint.point-on-curve
  constraint.curve-contact
  constraint.horizontal-line
  constraint.vertical-line
  constraint.parallel-lines
  constraint.perpendicular-lines
  constraint.radial-line
  constraint.equal-length
  constraint.equal-radius
  constraint.equal-curvature
  constraint.midpoint
  constraint.symmetric-about-line
  constraint.curve-tangency
  constraint.endpoint-continuity
  dimension.point-distance
  dimension.segment-length
  dimension.radius
  dimension.diameter
  dimension.oriented-angle
)

append_harness_result() {
  local case_id="$1"
  local family="$2"
  local status="$3"
  local failure_class="$4"
  local fingerprint="$5"
  printf '%s\t%s\t%s\t-\t%s\t%s\n' \
    "$case_id" "$family" "$status" "$failure_class" "$fingerprint" >>"$rows"
}

append_complete_output() {
  local output="$1"
  local expected_rows="$2"
  [[ -f "$output" ]] || return 1
  [[ "$(head -n 1 "$output")" == "$header" ]] || return 1
  [[ "$(tail -n +2 "$output" | wc -l)" -eq "$expected_rows" ]] || return 1
  tail -n +2 "$output" >>"$rows"
}

classify_failed_process() {
  local case_id="$1"
  local family="$2"
  local exit_code="$3"
  local log="$4"
  if [[ "$exit_code" -eq 124 ]]; then
    append_harness_result "$case_id" "$family" TIMEOUT family-timeout "${timeout_seconds}s"
  elif rg -q 'panicked at|test result: FAILED' "$log"; then
    append_harness_result "$case_id" "$family" PANIC test-process "exit-$exit_code"
  else
    append_harness_result "$case_id" "$family" HARNESS_ERROR test-process "exit-$exit_code"
  fi
}

cd "$root"
preflight_log="$scratch/preflight.log"
if ! timeout 300 cargo test --locked -p geosolve-constraint-editor \
  --test m70b_authoring_oracle oracle_inventory_and_tsv_schema_are_exhaustive \
  -- --exact >"$preflight_log" 2>&1; then
  printf '%s\n' 'authoring-oracle inventory/compile preflight failed' >&2
  cat "$preflight_log" >&2
  exit 1
fi
if ! timeout 300 cargo test --locked -p geosolve-demo-web --lib --no-run \
  >"$preflight_log" 2>&1; then
  printf '%s\n' 'scene-oracle compile preflight failed' >&2
  cat "$preflight_log" >&2
  exit 1
fi

for family in "${families[@]}"; do
  output="$scratch/${family//./_}.tsv"
  log="$scratch/${family//./_}.log"
  set +e
  timeout "${timeout_seconds}s" env \
    GEOSOLVE_M70B_ORACLE_FAMILY="$family" \
    GEOSOLVE_M70B_ORACLE_OUTPUT="$output" \
    cargo test --locked -p geosolve-constraint-editor \
      --test m70b_authoring_oracle oracle_family_survey -- --exact --nocapture \
      >"$log" 2>&1
  exit_code=$?
  set -e
  if [[ "$exit_code" -ne 124 ]] && append_complete_output "$output" 9; then
    continue
  fi
  classify_failed_process "$family.harness" "$family" "$exit_code" "$log"
done

scene_output="$scratch/scene.tsv"
scene_log="$scratch/scene.log"
set +e
timeout "${timeout_seconds}s" env \
  GEOSOLVE_M70B_ORACLE_OUTPUT="$scene_output" \
  cargo test --locked -p geosolve-demo-web m70b_scene_authority_oracle_survey \
    -- --nocapture >"$scene_log" 2>&1
scene_exit_code=$?
set -e
if ! { [[ "$scene_exit_code" -ne 124 ]] && append_complete_output "$scene_output" 4; }; then
  classify_failed_process scene.harness scene-authority "$scene_exit_code" "$scene_log"
fi

{
  printf '%s\n' "$header"
  LC_ALL=C sort -t $'\t' -k1,1 "$rows"
} >"$actual"

if ! awk -F '\t' '
  NR == 1 {
    if ($0 != "case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint") exit 1
    next
  }
  NF != 6 { exit 1 }
  $3 !~ /^(PASS|DEFECT|PANIC|TIMEOUT|HARNESS_ERROR)$/ { exit 1 }
  seen[$1]++ > 0 { exit 1 }
' "$actual"; then
  printf '%s\n' 'oracle emitted malformed or duplicate rows' >&2
  cat "$actual" >&2
  exit 1
fi

if [[ -f "$golden" ]]; then
  if ! awk -F '\t' '
    NR == 1 { next }
    NF != 6 { exit 1 }
    seen[$1]++ > 0 { exit 1 }
    $3 == "PASS" && ($4 != "-" || $5 != "-" || $6 != "ok") { exit 1 }
    $3 != "PASS" && ($4 !~ /^M70B-F[0-9][0-9][0-9]+$/ || $5 == "-" || $5 == "" || $6 == "" || $6 == "ok") { exit 1 }
  ' "$golden"; then
    printf '%s\n' 'oracle golden contains an unclassified or malformed row' >&2
    exit 1
  fi
  awk -F '\t' -v OFS='\t' '
    NR == FNR {
      if (FNR > 1) {
        signature[$1] = $2 SUBSEP $3 SUBSEP $5 SUBSEP $6
        finding[$1] = $4
      }
      next
    }
    FNR == 1 { print; next }
    {
      current = $2 SUBSEP $3 SUBSEP $5 SUBSEP $6
      if (($1 in signature) && signature[$1] == current) $4 = finding[$1]
      print
    }
  ' "$golden" "$actual" >"$classified"
else
  cp "$actual" "$classified"
fi

case "$mode" in
  --survey)
    cat "$classified"
    ;;
  --check | --require-clean)
    if [[ ! -f "$golden" ]]; then
      printf 'oracle golden is missing: %s\n' "$golden" >&2
      exit 1
    fi
    if ! cmp -s "$golden" "$classified"; then
      printf '%s\n' 'oracle result differs from the recorded checklist:' >&2
      diff -u "$golden" "$classified" >&2 || true
      exit 1
    fi
    if [[ "$mode" == '--require-clean' ]]; then
      if ! awk -F '\t' 'NR > 1 && $3 != "PASS" { print; dirty = 1 } END { exit dirty }' \
        "$classified"; then
        printf '%s\n' 'oracle contains known defects or harness failures' >&2
        exit 1
      fi
    fi
    printf 'oracle checklist matches: %s\n' "$golden"
    ;;
esac
