#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${1:-}" == --reference ]]; then
  shift
  if [[ $# -ne 0 ]]; then
    printf '%s\n' 'the historical reference gate accepts no additional arguments' >&2
    exit 2
  fi
  exec bash "$root/scripts/release-gate-reference.sh"
fi
export PYTHONDONTWRITEBYTECODE=1
exec python3 "$root/scripts/release_gate.py" "$@"
