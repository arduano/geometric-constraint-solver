#!/usr/bin/env bash
#
# Bundle the fuzz campaign's receipts (crash artifacts, corpus, run logs and the
# fuzz-crate source) into a single zip for analysis, so results can be passed
# back without shipping the whole workspace. The build cache (fuzz/target/) and
# the copied domain crates (crates/) are excluded by default -- they are large
# and derivable from the repo -- and can be re-included with FUZZ_BUNDLE_INCLUDE.
#
# Reads the campaign output from <out-root>/ in the CURRENT WORKING DIRECTORY
# (default: results, matching fuzz-campaign.sh's FUZZ_OUT_ROOT) and writes
# <name>-receipts-<timestamp>.zip next to it, with a SUMMARY.md and MANIFEST.txt.
#
# Usage:
#   scripts/fuzz-bundle.sh [--help]
#
# Configuration (environment variables):
#   FUZZ_OUT_ROOT          campaign output dir (default: results)
#   FUZZ_BUNDLE_TARGETS    space-separated target names (default: all present)
#   FUZZ_BUNDLE_INCLUDE    comma list of parts to bundle (order-independent):
#                          default: artifacts,corpus,logs,fuzz_src
#                          add: build_cache   fuzz/target/ (full reproducibility)
#                          add: crates        copied domain crates (self-contained)
#   FUZZ_BUNDLE_OUT        output dir for the zip (default: current dir)
#   FUZZ_BUNDLE_NAME       zip base name (default: <out-root>-receipts-<stamp>)
#
set -uo pipefail

usage() {
  cat <<'EOF'
fuzz-bundle: bundle campaign receipts (crashes/corpus/logs) into a zip.
Reads <out-root>/ in the current directory (default: results) and writes
<out-root>-receipts-<timestamp>.zip next to it.

Options:
  -h, --help   show this help

Environment:
  FUZZ_OUT_ROOT         campaign output dir (default: results)
  FUZZ_BUNDLE_TARGETS   space-separated target names (default: all present)
  FUZZ_BUNDLE_INCLUDE   comma list: artifacts,corpus,logs,fuzz_src
                        (+ build_cache,crates)
  FUZZ_BUNDLE_OUT       output dir for the zip (default: current dir)
  FUZZ_BUNDLE_NAME      zip base name (default: <out-root>-receipts-<stamp>)
EOF
}

OUT_ROOT="${FUZZ_OUT_ROOT:-results}"
FUZZ_BUNDLE_TARGETS="${FUZZ_BUNDLE_TARGETS:-}"
FUZZ_BUNDLE_INCLUDE="${FUZZ_BUNDLE_INCLUDE:-artifacts,corpus,logs,fuzz_src}"
OUT_DIR="${FUZZ_BUNDLE_OUT:-$(pwd)}"
STAMP="$(date +%Y%m%d-%H%M%S 2>/dev/null || date)"
ZIP_NAME="${FUZZ_BUNDLE_NAME:-${OUT_ROOT//[^A-Za-z0-9]/_}-receipts-${STAMP}}"
ZIP_PATH="$OUT_DIR/$ZIP_NAME.zip"

die() { echo "ERROR: $*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --) shift; break ;;
    -*) die "unknown option: $1" ;;
    *) break ;;
  esac
  shift
done

[ -d "$OUT_ROOT" ] || die "OUT_ROOT '$OUT_ROOT' not found in $(pwd); run the campaign first (set FUZZ_OUT_ROOT if elsewhere)."
command -v zip >/dev/null 2>&1 || die "required tool 'zip' not found on PATH (install 'zip')."

has_part() { case ",$FUZZ_BUNDLE_INCLUDE," in *",$1,"*) return 0 ;; *) return 1 ;; esac; }

# Resolve the target list: every directory under OUT_ROOT, or an explicit subset.
mapfile -t targets < <(find "$OUT_ROOT" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' 2>/dev/null | sort)
if [ -n "$FUZZ_BUNDLE_TARGETS" ]; then
  wanted=(); for w in $FUZZ_BUNDLE_TARGETS; do wanted+=("$w"); done
  targets=()
  for t in "${wanted[@]}"; do
    [ -d "$OUT_ROOT/$t" ] || die "FUZZ_BUNDLE_TARGETS: no such target '$t' under '$OUT_ROOT/'"
    targets+=("$t")
  done
fi
[ "${#targets[@]}" -gt 0 ] || die "no target directories found under '$OUT_ROOT/'"

# Stage the bundle, then zip its contents so the archive root holds the summary
# and per-target trees (no outer staging prefix).
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/geosolve-fuzz-bundle.XXXXXX")" || die "mktemp failed"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$OUT_DIR"

SUMMARY="$STAGE/SUMMARY.md"
{
  echo "# GeoSolve fuzz campaign receipts"
  echo "# generated: $(date -Is 2>/dev/null || date)"
  echo "# out_root: $OUT_ROOT"
  echo "# parts included: $FUZZ_BUNDLE_INCLUDE"
  echo "# default exclusions: fuzz/target/ (build cache), crates/ (copied domain sources)"
  echo
} >"$SUMMARY"

for name in "${targets[@]}"; do
  src="$OUT_ROOT/$name"
  dst="$STAGE/$name"
  mkdir -p "$dst"

  # Copy only the requested parts (contents, not the parent dir) so counts below
  # reflect what was actually bundled.
  if has_part artifacts && [ -d "$src/artifacts" ]; then
    mkdir -p "$dst/artifacts"; cp -a "$src/artifacts"/. "$dst/artifacts/" 2>/dev/null || true
  fi
  if has_part corpus && [ -d "$src/fuzz/corpus/$name" ]; then
    mkdir -p "$dst/corpus"; cp -a "$src/fuzz/corpus/$name"/. "$dst/corpus/" 2>/dev/null || true
  fi
  if has_part logs && [ -f "$src/run.log" ]; then
    cp -a "$src/run.log" "$dst/run.log" 2>/dev/null || true
  fi
  if has_part fuzz_src && [ -d "$src/fuzz" ]; then
    mkdir -p "$dst/fuzz"; cp -a "$src/fuzz"/. "$dst/fuzz/" 2>/dev/null || true
    # fuzz_src must not carry the build cache; drop it unless build_cache is
    # explicitly requested (then re-added below at the same location).
    if ! has_part build_cache && [ -d "$dst/fuzz/target" ]; then
      rm -rf "$dst/fuzz/target"
    fi
  fi
  if has_part crates && [ -d "$src/crates" ]; then
    mkdir -p "$dst/crates"; cp -a "$src/crates"/. "$dst/crates/" 2>/dev/null || true
  fi
  if has_part build_cache && [ -d "$src/fuzz/target" ]; then
    mkdir -p "$dst/fuzz/target"; cp -a "$src/fuzz/target"/. "$dst/fuzz/target/" 2>/dev/null || true
  fi

  c_art="$(find "$dst/artifacts" -type f 2>/dev/null | wc -l | tr -d ' ')"
  c_corpus="$(find "$dst/corpus" -type f 2>/dev/null | wc -l | tr -d ' ')"
  c_fuzz="$(find "$dst/fuzz" -type f 2>/dev/null | wc -l | tr -d ' ')"
  log_present="no"; [ -f "$dst/run.log" ] && log_present="yes"

  {
    echo "## $name"
    echo
    echo "- artifacts (crashes): $c_art"
    echo "- corpus inputs:       $c_corpus"
    echo "- fuzz source files:   $c_fuzz"
    echo "- run log:             $log_present"
    if [ "$c_art" -gt 0 ]; then
      echo
      echo "crash files:"
      while IFS= read -r f; do
        rel="${f#"$STAGE"/}"
        printf -- "  - %s (%s)\n" "$rel" "$(du -h "$f" 2>/dev/null | cut -f1)"
      done < <(find "$dst/artifacts" -type f 2>/dev/null | sort)
    fi
    echo
  } >>"$SUMMARY"
done

# Full file manifest (relative path + size) for the archive.
{
  echo "# Manifest: relative path, size in bytes"
  ( cd "$STAGE" && find . -type f ! -name SUMMARY.md ! -name MANIFEST.txt -printf '%P\t%s\n' 2>/dev/null | sort )
} >"$STAGE/MANIFEST.txt"

( cd "$STAGE" && zip -r -q "$ZIP_PATH" . ) || die "zip failed"

size="$(du -h "$ZIP_PATH" 2>/dev/null | cut -f1)"
echo "Bundled ${#targets[@]} target(s): ${targets[*]}"
echo "  zip:  $ZIP_PATH ($size)"
echo "  parts: $FUZZ_BUNDLE_INCLUDE"
