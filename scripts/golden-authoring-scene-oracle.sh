#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
golden="$root/crates/geosolve-constraint-editor/tests/fixtures/golden_authoring_scene_oracle.golden.tsv"
header=$'case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint'
timeout_seconds=30
parity_timeout_seconds=60

usage() {
  printf '%s\n' \
    'usage: scripts/golden-authoring-scene-oracle.sh --survey|--check|--require-clean' >&2
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

scratch_parent="$root/target/golden-authoring-scene-oracle"
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
  constraint.horizontal-points
  constraint.vertical-points
  constraint.concentric-curves
  constraint.collinear-supports
  constraint.coincident-with-origin
  constraint.point-on-datum-axis
  constraint.collinear-with-datum-axis
  constraint.symmetric-about-datum-axis
)

authoring_cases=(
  deterministic
  seed-00
  seed-01
  seed-02
  seed-03
  seed-04
  seed-05
  seed-06
  seed-07
)

fillet_cases=(
  feature.fillet.authoring.coincident-closure.curve-pair
  feature.fillet.authoring.coincident-closure.point
  feature.fillet.authoring.native-profile.line-line
  feature.fillet.evaluation.line-circle.same-cell-lower
  feature.fillet.evaluation.line-circle.same-cell-seam
  feature.fillet.evaluation.line-circle.source-rotation.retained-start
)

scene_cases=(
  scene.current-computed.empty
  scene.current-native.withheld
  scene.current-computed.fillet
  scene.rejected-historical.detached
)

append_harness_result() {
  local case_id="$1"
  local family="$2"
  local status="$3"
  local failure_class="$4"
  local fingerprint="$5"
  append_result_row \
    "$case_id" "$family" "$status" '-' "$failure_class" "$fingerprint"
}

append_result_row() {
  local case_id="$1"
  local family="$2"
  local status="$3"
  local finding_id="$4"
  local failure_class="$5"
  local fingerprint="$6"
  if awk -F '\t' -v case_id="$case_id" '$1 == case_id { found = 1 } END { exit !found }' \
    "$rows"; then
    printf 'oracle attempted to classify %s more than once\n' "$case_id" >&2
    return 1
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$case_id" "$family" "$status" "$finding_id" "$failure_class" "$fingerprint" >>"$rows"
}

append_complete_output() {
  local output="$1"
  local expected_rows="$2"
  local expected_family="$3"
  local expected_case_id="${4:-}"
  [[ -f "$output" ]] || return 1
  [[ "$(head -n 1 "$output")" == "$header" ]] || return 1
  [[ "$(tail -n +2 "$output" | wc -l)" -eq "$expected_rows" ]] || return 1
  awk -F '\t' -v family="$expected_family" -v case_id="$expected_case_id" '
    NR == 1 { next }
    NF != 6 || $2 != family || (case_id != "" && $1 != case_id) { exit 1 }
  ' "$output" || return 1
  local row_case_id row_family row_status row_finding row_failure row_fingerprint extra
  IFS=$'\t' read -r \
    row_case_id row_family row_status row_finding row_failure row_fingerprint extra \
    < <(tail -n +2 "$output") || return 1
  [[ -z "${extra:-}" ]] || return 1
  append_result_row \
    "$row_case_id" "$row_family" "$row_status" "$row_finding" "$row_failure" "$row_fingerprint"
}

append_parity_result() {
  local case_id="$1"
  local family="$2"
  local result="$3"
  [[ -f "$result" ]] || return 1
  local status failure_class fingerprint extra
  IFS=$'\t' read -r status failure_class fingerprint extra <"$result" || return 1
  [[ -z "${extra:-}" ]] || return 1
  case "$status" in
    PASS)
      [[ "$failure_class" == '-' && "$fingerprint" == 'ok' ]] || return 1
      return 0
      ;;
    DEFECT | PANIC | TIMEOUT | HARNESS_ERROR)
      [[ -n "$failure_class" && "$failure_class" != '-' && -n "$fingerprint" ]] || return 1
      append_harness_result "$case_id" "$family" "$status" "$failure_class" "$fingerprint"
      return 2
      ;;
    *) return 1 ;;
  esac
}

run_authoring_parity() {
  local case_id="$1"
  local family="$2"
  local manifest="$3"
  local directory="$4"
  local result="$5"
  local log="$6"
  local export_test="${7:-golden_backend_parity_export}"
  local validate_test="${8:-golden_backend_parity_validate}"
  rm -f "$result"
  set +e
  timeout -k 5s "${parity_timeout_seconds}s" env \
    GEOSOLVE_GOLDEN_PARITY_MANIFEST="$manifest" \
    GEOSOLVE_GOLDEN_PARITY_DIRECTORY="$directory" \
    GEOSOLVE_GOLDEN_PARITY_RESULT="$result" \
    cargo test --locked -p geosolve-sketch-code \
      --test golden_backend_parity "$export_test" -- --exact --nocapture \
      >"$log" 2>&1
  local exit_code=$?
  set -e
  if [[ "$exit_code" -ne 0 ]]; then
    classify_failed_process \
      "$case_id" "$family" "$exit_code" "$log" "$parity_timeout_seconds"
    return 1
  fi
  set +e
  append_parity_result "$case_id" "$family" "$result"
  local parity_status=$?
  set -e
  if [[ "$parity_status" -eq 2 ]]; then
    return 1
  elif [[ "$parity_status" -ne 0 ]]; then
    append_harness_result "$case_id" "$family" HARNESS_ERROR parity-export malformed-result
    return 1
  fi

  local request="$directory/compile-batch.request.json"
  local compiled="$directory/compile-batch.output.json"
  if [[ ! -s "$request" ]]; then
    append_harness_result "$case_id" "$family" HARNESS_ERROR parity-export missing-compile-batch
    return 1
  fi
  set +e
  timeout -k 5s "${parity_timeout_seconds}s" npm \
    --prefix packages/geosolve-sketch-code run --silent compile:managed:batch:deno \
    <"$request" >"$compiled" 2>"$log.compile"
  exit_code=$?
  set -e
  if [[ "$exit_code" -ne 0 || ! -s "$compiled" ]]; then
    local detail="batch-exit-$exit_code"
    if [[ -s "$log.compile" ]]; then
      detail="$(tr '\t\r\n' '   ' <"$log.compile" | cut -c1-512)"
    fi
    append_harness_result "$case_id" "$family" DEFECT managed-compile "$detail"
    return 1
  fi

  rm -f "$result"
  set +e
  timeout -k 5s "${parity_timeout_seconds}s" env \
    GEOSOLVE_GOLDEN_PARITY_MANIFEST="$manifest" \
    GEOSOLVE_GOLDEN_PARITY_DIRECTORY="$directory" \
    GEOSOLVE_GOLDEN_PARITY_RESULT="$result" \
    cargo test --locked -p geosolve-sketch-code \
      --test golden_backend_parity "$validate_test" -- --exact --nocapture \
      >>"$log" 2>&1
  exit_code=$?
  set -e
  if [[ "$exit_code" -ne 0 ]]; then
    classify_failed_process \
      "$case_id" "$family" "$exit_code" "$log" "$parity_timeout_seconds"
    return 1
  fi
  set +e
  append_parity_result "$case_id" "$family" "$result"
  parity_status=$?
  set -e
  if [[ "$parity_status" -eq 0 ]]; then
    return 0
  elif [[ "$parity_status" -eq 2 ]]; then
    return 1
  fi
  append_harness_result "$case_id" "$family" HARNESS_ERROR parity-validate malformed-result
  return 1
}

run_scene_parity() {
  local case_id="$1"
  local directory="$2"
  local result="$3"
  local log="$4"
  rm -f "$result"
  set +e
  timeout -k 5s "${parity_timeout_seconds}s" env \
    GEOSOLVE_GOLDEN_ORACLE_CASE="$case_id" \
    GEOSOLVE_GOLDEN_PARITY_DIRECTORY="$directory" \
    GEOSOLVE_GOLDEN_PARITY_RESULT="$result" \
    cargo test --locked -p geosolve-demo-web --lib \
      workbench::tests::golden_scene_backend_parity::golden_scene_backend_parity_export \
      -- --exact --nocapture >"$log" 2>&1
  local exit_code=$?
  set -e
  if [[ "$exit_code" -ne 0 ]]; then
    classify_failed_process \
      "$case_id" scene-authority "$exit_code" "$log" "$parity_timeout_seconds"
    return 1
  fi
  set +e
  append_parity_result "$case_id" scene-authority "$result"
  local parity_status=$?
  set -e
  if [[ "$parity_status" -eq 2 ]]; then
    return 1
  elif [[ "$parity_status" -ne 0 ]]; then
    append_harness_result \
      "$case_id" scene-authority HARNESS_ERROR scene-parity-export malformed-result
    return 1
  fi

  local request="$directory/compile-batch.request.json"
  local compiled="$directory/compile-batch.output.json"
  if [[ ! -s "$request" ]]; then
    append_harness_result \
      "$case_id" scene-authority HARNESS_ERROR scene-parity-export missing-compile-batch
    return 1
  fi
  set +e
  timeout -k 5s "${parity_timeout_seconds}s" npm \
    --prefix packages/geosolve-sketch-code run --silent compile:managed:batch:deno \
    <"$request" >"$compiled" 2>"$log.compile"
  exit_code=$?
  set -e
  if [[ "$exit_code" -ne 0 || ! -s "$compiled" ]]; then
    local detail="batch-exit-$exit_code"
    if [[ -s "$log.compile" ]]; then
      detail="$(tr '\t\r\n' '   ' <"$log.compile" | cut -c1-512)"
    fi
    append_harness_result "$case_id" scene-authority DEFECT managed-compile "$detail"
    return 1
  fi

  rm -f "$result"
  set +e
  timeout -k 5s "${parity_timeout_seconds}s" env \
    GEOSOLVE_GOLDEN_ORACLE_CASE="$case_id" \
    GEOSOLVE_GOLDEN_PARITY_DIRECTORY="$directory" \
    GEOSOLVE_GOLDEN_PARITY_RESULT="$result" \
    cargo test --locked -p geosolve-demo-web --lib \
      workbench::tests::golden_scene_backend_parity::golden_scene_backend_parity_validate \
      -- --exact --nocapture >>"$log" 2>&1
  exit_code=$?
  set -e
  if [[ "$exit_code" -ne 0 ]]; then
    classify_failed_process \
      "$case_id" scene-authority "$exit_code" "$log" "$parity_timeout_seconds"
    return 1
  fi
  set +e
  append_parity_result "$case_id" scene-authority "$result"
  parity_status=$?
  set -e
  if [[ "$parity_status" -eq 0 ]]; then
    return 0
  elif [[ "$parity_status" -eq 2 ]]; then
    return 1
  fi
  append_harness_result \
    "$case_id" scene-authority HARNESS_ERROR scene-parity-validate malformed-result
  return 1
}

classify_failed_process() {
  local case_id="$1"
  local family="$2"
  local exit_code="$3"
  local log="$4"
  local reported_timeout_seconds="${5:-$timeout_seconds}"
  if [[ "$exit_code" -eq 124 || "$exit_code" -eq 137 ]]; then
    append_harness_result \
      "$case_id" "$family" TIMEOUT case-timeout "${reported_timeout_seconds}s"
  elif rg -q 'panicked at|test result: FAILED' "$log"; then
    append_harness_result "$case_id" "$family" PANIC test-process "exit-$exit_code"
  else
    append_harness_result "$case_id" "$family" HARNESS_ERROR test-process "exit-$exit_code"
  fi
}

cd "$root"
preflight_log="$scratch/preflight.log"
if ! timeout -k 5s 300s cargo test --locked -p geosolve-constraint-editor \
  --test golden_authoring_oracle golden_oracle_inventory_and_tsv_schema_are_exhaustive \
  -- --exact >"$preflight_log" 2>&1; then
  printf '%s\n' 'authoring-oracle inventory/compile preflight failed' >&2
  cat "$preflight_log" >&2
  exit 1
fi
if ! timeout -k 5s 300s cargo test --locked -p geosolve-constraint-editor \
  --test golden_fillet_oracle golden_fillet_oracle_inventory_and_tsv_schema_are_exhaustive \
  -- --exact >"$preflight_log" 2>&1; then
  printf '%s\n' 'Fillet-oracle inventory/compile preflight failed' >&2
  cat "$preflight_log" >&2
  exit 1
fi
if ! timeout -k 5s 300s cargo test --locked -p geosolve-demo-web --lib --no-run \
  >"$preflight_log" 2>&1; then
  printf '%s\n' 'scene-oracle compile preflight failed' >&2
  cat "$preflight_log" >&2
  exit 1
fi
if ! timeout -k 5s 300s cargo test --locked -p geosolve-sketch-code \
  --test golden_backend_parity \
  golden_backend_parity_exclusion_ledger_is_explicit_and_reviewable -- --exact \
  >"$preflight_log" 2>&1; then
  printf '%s\n' 'dual-backend parity compile preflight failed' >&2
  cat "$preflight_log" >&2
  exit 1
fi
if ! timeout -k 5s 300s npm --prefix packages/geosolve-intent ci --ignore-scripts \
  >"$preflight_log" 2>&1 || \
  ! timeout -k 5s 300s npm --prefix packages/geosolve-intent run build \
  >>"$preflight_log" 2>&1 || \
  ! timeout -k 5s 300s npm --prefix packages/geosolve-sketch-code ci --ignore-scripts \
  >"$preflight_log" 2>&1 || \
  ! timeout -k 5s 300s npm --prefix packages/geosolve-sketch-code run build \
  >>"$preflight_log" 2>&1; then
  printf '%s\n' 'pinned managed TypeScript compiler preflight failed' >&2
  cat "$preflight_log" >&2
  exit 1
fi

for family in "${families[@]}"; do
  for oracle_case in "${authoring_cases[@]}"; do
    case_id="$family.$oracle_case"
    stem="${case_id//./_}"
    output="$scratch/$stem.tsv"
    log="$scratch/$stem.log"
    parity_manifest="$scratch/$stem.parity.json"
    parity_directory="$scratch/$stem.parity"
    parity_result="$scratch/$stem.parity-result.tsv"
    set +e
    timeout -k 5s "${timeout_seconds}s" env \
      GEOSOLVE_GOLDEN_ORACLE_FAMILY="$family" \
      GEOSOLVE_GOLDEN_ORACLE_CASE="$oracle_case" \
      GEOSOLVE_GOLDEN_ORACLE_OUTPUT="$output" \
      GEOSOLVE_GOLDEN_ORACLE_PARITY_MANIFEST="$parity_manifest" \
      cargo test --locked -p geosolve-constraint-editor \
        --test golden_authoring_oracle golden_oracle_family_survey -- --exact --nocapture \
        >"$log" 2>&1
    exit_code=$?
    set -e
    if [[ "$exit_code" -eq 0 ]] && [[ -f "$parity_manifest" ]]; then
      if run_authoring_parity \
        "$case_id" "$family" "$parity_manifest" "$parity_directory" \
        "$parity_result" "$log.parity"; then
        if append_complete_output "$output" 1 "$family" "$case_id"; then
          continue
        fi
        append_harness_result "$case_id" "$family" HARNESS_ERROR native-output malformed-output
      fi
      continue
    fi
    classify_failed_process "$case_id" "$family" "$exit_code" "$log"
  done
done

for case_id in "${fillet_cases[@]}"; do
  stem="${case_id//./_}"
  output="$scratch/$stem.tsv"
  log="$scratch/$stem.log"
  parity_manifest="$scratch/$stem.parity.json"
  parity_directory="$scratch/$stem.parity"
  parity_result="$scratch/$stem.parity-result.tsv"
  set +e
  timeout -k 5s "${timeout_seconds}s" env \
    GEOSOLVE_GOLDEN_ORACLE_CASE="$case_id" \
    GEOSOLVE_GOLDEN_ORACLE_OUTPUT="$output" \
    GEOSOLVE_GOLDEN_ORACLE_PARITY_MANIFEST="$parity_manifest" \
    cargo test --locked -p geosolve-constraint-editor \
      --test golden_fillet_oracle golden_fillet_oracle_survey -- --exact --nocapture \
      >"$log" 2>&1
  exit_code=$?
  set -e
  if [[ "$exit_code" -eq 0 ]] && [[ -f "$parity_manifest" ]]; then
    if run_authoring_parity \
      "$case_id" feature.fillet "$parity_manifest" "$parity_directory" \
      "$parity_result" "$log.parity" \
      golden_fillet_backend_parity_export golden_fillet_backend_parity_validate; then
      if append_complete_output "$output" 1 feature.fillet "$case_id"; then
        continue
      fi
      append_harness_result "$case_id" feature.fillet HARNESS_ERROR native-output malformed-output
    fi
    continue
  fi
  classify_failed_process "$case_id" feature.fillet "$exit_code" "$log"
done

for case_id in "${scene_cases[@]}"; do
  stem="${case_id//./_}"
  scene_output="$scratch/$stem.tsv"
  scene_log="$scratch/$stem.log"
  parity_directory="$scratch/$stem.parity"
  parity_result="$scratch/$stem.parity-result.tsv"
  set +e
  timeout -k 5s "${timeout_seconds}s" env \
    GEOSOLVE_GOLDEN_ORACLE_CASE="$case_id" \
    GEOSOLVE_GOLDEN_ORACLE_OUTPUT="$scene_output" \
    cargo test --locked -p geosolve-demo-web --lib \
      workbench::tests::golden_scene_authority_oracle_survey \
      -- --exact --nocapture >"$scene_log" 2>&1
  scene_exit_code=$?
  set -e
  if [[ "$scene_exit_code" -eq 0 ]] && \
    run_scene_parity "$case_id" "$parity_directory" "$parity_result" "$scene_log.parity"; then
    if append_complete_output "$scene_output" 1 scene-authority "$case_id"; then
      continue
    fi
    append_harness_result \
      "$case_id" scene-authority HARNESS_ERROR native-output malformed-output
    continue
  fi
  if awk -F '\t' -v case_id="$case_id" '$1 == case_id { found = 1 } END { exit !found }' \
    "$rows"; then
    continue
  fi
  classify_failed_process "$case_id" scene-authority "$scene_exit_code" "$scene_log"
done

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
  $1 ~ /^scene\./ && $2 != "scene-authority" { exit 1 }
  $1 ~ /^feature\.fillet\./ && $2 != "feature.fillet" { exit 1 }
  $1 !~ /^scene\./ && $1 !~ /^feature\.fillet\./ {
    expected_family = $1
    sub(/\.(deterministic|seed-[0-9][0-9])$/, "", expected_family)
    if ($2 != expected_family) exit 1
  }
' "$actual"; then
  printf '%s\n' 'oracle emitted malformed or duplicate rows' >&2
  cat "$actual" >&2
  exit 1
fi

expected_inventory="$scratch/expected-inventory.tsv"
actual_inventory="$scratch/actual-inventory.tsv"
{
  for family in "${families[@]}"; do
    for oracle_case in "${authoring_cases[@]}"; do
      printf '%s.%s\t%s\n' "$family" "$oracle_case" "$family"
    done
  done
  for case_id in "${fillet_cases[@]}"; do
    printf '%s\tfeature.fillet\n' "$case_id"
  done
  for case_id in "${scene_cases[@]}"; do
    printf '%s\tscene-authority\n' "$case_id"
  done
} | LC_ALL=C sort >"$expected_inventory"
tail -n +2 "$actual" | cut -f 1,2 | LC_ALL=C sort >"$actual_inventory"
expected_case_count=$((${#families[@]} * ${#authoring_cases[@]} + ${#fillet_cases[@]} + ${#scene_cases[@]}))
if [[ "$(wc -l <"$actual_inventory")" -ne "$expected_case_count" ]] || \
  ! cmp -s "$expected_inventory" "$actual_inventory"; then
  printf 'oracle did not classify the exact %s-case inventory\n' "$expected_case_count" >&2
  diff -u "$expected_inventory" "$actual_inventory" >&2 || true
  exit 1
fi

if [[ -f "$golden" ]]; then
  require_input_fingerprint=1
  if [[ "$mode" == '--survey' ]]; then
    require_input_fingerprint=0
  fi
  if ! awk -F '\t' -v require_input_fingerprint="$require_input_fingerprint" '
    NR == 1 { next }
    NF != 6 { exit 1 }
    seen[$1]++ > 0 { exit 1 }
    $3 == "PASS" && ($4 != "-" || $5 != "-") { exit 1 }
    require_input_fingerprint && $3 == "PASS" &&
      !(($2 == "scene-authority" && $6 == "ok") ||
        ($2 != "scene-authority" && $6 ~ /^input-[[:xdigit:]]+$/ && length($6) == 22)) { exit 1 }
    $3 != "PASS" && ($4 !~ /^M[0-9][0-9]*[A-Z]*-F[0-9][0-9][0-9]+$/ || $5 == "-" || $5 == "" || $6 == "" || $6 == "ok") { exit 1 }
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
