#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Fail-closed, reviewed program boundary for immutable catalog data reuse.

A new passing run cannot silently bless a changed dependency boundary. The
reviewed source-map digest must be updated explicitly after an input audit.
"""
from __future__ import annotations

CONTRACT_PATH = "scripts/release_equivalence.json"
SAMPLE_ROOT = "crates/geosolve-sketch-code/assets/bundled-samples"
CATALOG = "crates/geosolve-sketch-code/assets/bundled-sample-catalog.json"
FRONTEND_CATALOG = "crates/geosolve-demo-web/frontend/src/data/samples.json"
DATA_FILES = frozenset(("manifest.json", "sketch.ts", "sketch.compiled.json", "witnesses.json",
                        "audit-edits.json", "NOTICE.md", "README.md", "fixture-cells.artifact.json",
                        "fixture-cells.patch.ts", "harness-route.patch.ts", "harness-route.artifact.json",
                        "water-channel.artifact.json", "water-channel.patch.ts"))


def catalog_data(name, value, policy):
    from release_gate import matches
    if matches(name, policy["global"]) or not matches(name, policy["owned"]):
        return False
    # Symlink targets and new file forms may contain shared program inputs.
    if value is not None and (not isinstance(value, dict) or set(value) != {"sha256", "executable"}):
        return False
    if value is not None and value["executable"]:
        return False
    if name in (CATALOG, FRONTEND_CATALOG):
        return True
    if not name.startswith(SAMPLE_ROOT + "/"):
        return False
    parts = name[len(SAMPLE_ROOT) + 1:].split("/")
    return len(parts) == 2 and parts[1] in DATA_FILES


def partition(inputs, policy):
    return {name: value for name, value in inputs.items()
            if name != CONTRACT_PATH and not catalog_data(name, value, policy)}


def reviewed(scope, inputs, policy, contract):
    from release_gate import digest
    if not isinstance(contract, dict) or contract.get("schema") != 1:
        return False
    entry = contract.get(scope)
    return (isinstance(entry, dict) and entry.get("program_sha256") == digest(partition(inputs, policy)))
