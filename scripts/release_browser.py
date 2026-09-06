#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Browser leaf evidence under the reviewed immutable sample-data boundary."""
from __future__ import annotations
import copy
import math
import os
from pathlib import Path
import re
import subprocess
import time

SCHEMA = "geosolve-browser-leaf-v3"
from release_browser_context import parent_context, validate_context
SAMPLE_ROOT = "crates/geosolve-sketch-code/assets/bundled-samples"
CATALOG = "crates/geosolve-sketch-code/assets/bundled-sample-catalog.json"
CATALOG_FRONTEND = "crates/geosolve-demo-web/frontend/src/data/samples.json"
SAMPLE_PREFIX = "M92 visual workflow: "
FILES = ("tests/e2e/language-service.spec.ts", "tests/e2e/workbench.spec.ts",
         "tests/e2e/m92-sample-audit.spec.ts", "tests/e2e/release-catalog.spec.ts",
         "tests/e2e/canvas-renderer.spec.ts")


def identity(row):
    return (row["file"], row["title"], row["project"])


def sample_key(row):
    if row["file"].endswith("m92-sample-audit.spec.ts") and row["title"].startswith(SAMPLE_PREFIX):
        return row["title"][len(SAMPLE_PREFIX):]
    return None


def program_inputs(inputs, policy):
    import release_equivalence
    return release_equivalence.partition(inputs, policy)


def sample_inputs(root, snapshot, key):
    from release_gate import digest, read_json
    prefix = SAMPLE_ROOT + "/" + key + "/"
    selected = {name: value for name, value in snapshot.items() if name.startswith(prefix)}
    if not selected:
        raise ValueError("sample has no authenticated input files")
    manifest = prefix + "manifest.json"
    # Ordinal belongs exclusively to the fresh catalog/order obligation. Stable
    # project IDs, selected source and complete initial state do not consume it.
    value = read_json(root / manifest)
    value.pop("ordinal")
    selected[manifest] = {"semantic_manifest_sha256": digest(value)}
    return selected


def validate_witness(value, key):
    if value.get("format") != "geosolve-browser-sample-prefix-v2" or value.get("key") != key:
        raise ValueError("browser witness identity differs")
    if value.get("project") != "geosolve-sample-" + key or not value.get("title"):
        raise ValueError("browser witness project/title missing")
    if not isinstance(value.get("workspaceBytes"), int) or value["workspaceBytes"] <= 0:
        raise ValueError("browser witness lacks complete persisted state")
    for name in ("workspaceSha256", "acceptedSourceSha256", "fittedGeometrySha256", "canonicalAcceptedSceneSha256"):
        if not re.fullmatch("[0-9a-f]{64}", value.get(name, "")):
            raise ValueError("browser witness hash missing or malformed")
    visual = value.get("canvasVisual")
    if not isinstance(visual, dict) or visual.get("format") != "geosolve-canvas-visual-v1":
        raise ValueError("browser witness lacks actual canvas evidence")
    if not re.fullmatch("[0-9a-f]{64}", visual.get("screenshotSha256", "")):
        raise ValueError("canvas screenshot hash missing or malformed")
    for name in ("width", "height"):
        if type(visual.get(name)) is not int or visual[name] <= 0:
            raise ValueError("canvas screenshot dimensions missing or malformed")
    samples = visual.get("samples")
    if not isinstance(samples, list) or not samples:
        raise ValueError("canvas lacks visible geometry pixel samples")
    for sample in samples:
        if (not isinstance(sample, dict) or not sample.get("itemId")
                or type(sample.get("brightPixels")) is not int or sample["brightPixels"] <= 2
                or type(sample.get("x")) is not int or not 0 <= sample["x"] < visual["width"]
                or type(sample.get("y")) is not int or not 0 <= sample["y"] < visual["height"]):
            raise ValueError("canvas geometry pixel sample missing or malformed")
    diagnostics = visual.get("diagnostics", {})
    if (diagnostics.get("backend") != "webgl2" or diagnostics.get("state") != "ready"
            or not isinstance(diagnostics.get("hardware"), dict)
            or not diagnostics["hardware"].get("renderer")):
        raise ValueError("canvas witness lacks a successfully presented WebGL2 frame")
    for name in ("width", "height", "pixelRatio", "rasterResolution"):
        number = diagnostics.get(name)
        if type(number) not in (int, float) or not math.isfinite(number) or number <= 0:
            raise ValueError("canvas witness lacks a finite visible raster surface")
    return value


def leaf_key(program, data, row, witness):
    from release_gate import digest
    validate_witness(witness, sample_key(row))
    return digest({"schema": SCHEMA, "program": program, "sample": data,
                   "case": identity(row), "initial_state": witness})


def find_leaf(store, key, row, verified_batches=None):
    from release_gate import file_hash
    verified_batches = {} if verified_batches is None else verified_batches
    value = store.unseal(store.path / "browser-leaves" / (key + ".json"))
    if not value or value.get("schema") != SCHEMA or value.get("key") != key:
        return None
    if value.get("status") != "passed" or value.get("complete") is not True or tuple(value.get("case", [])) != identity(row):
        return None
    if not value.get("artifact") or not value.get("origin_run") or not value.get("provenance"):
        return None
    batch = value.get("batch", {})
    if not {"path", "sha256"} <= batch.keys():
        return None
    reference = (batch["path"], batch["sha256"])
    if reference not in verified_batches:
        try:
            path = Path(batch["path"])
            if not path.resolve().is_relative_to(store.path.resolve()) or file_hash(path) != batch["sha256"]:
                raise ValueError("browser batch reference differs")
            record = store.unseal(path)
            if not record or record.get("schema") != "geosolve-browser-batch-v1" or not record.get("evidence"):
                raise ValueError("browser batch is unauthenticated or incomplete")
            for filename, expected in record["evidence"].items():
                evidence_path = Path(filename)
                if (not evidence_path.resolve().is_relative_to(store.path.resolve())
                        or file_hash(evidence_path) != expected):
                    raise ValueError("browser batch evidence differs")
            verified_batches[reference] = record
        except (OSError, ValueError, KeyError, TypeError):
            verified_batches[reference] = None
    record = verified_batches[reference]
    if (not record or record.get("program") != value.get("program")
            or record.get("artifact") != value["artifact"] or record.get("provenance") != value["provenance"]
            or record.get("origin_run") != value["origin_run"]
            or list(identity(row)) not in record.get("passed_cases", [])):
        return None
    try:
        if leaf_key(value["program"], value["sample"], row, value["witness"]) != key:
            return None
    except (ValueError, KeyError, TypeError):
        return None
    return value


def validate_discovery(root, discovery, contract):
    from release_gate import browser_rows, read_json
    rows = browser_rows(discovery)
    ids = [identity(row) for row in rows]
    if not rows or len(ids) != len(set(ids)):
        raise ValueError("missing or duplicated discovered browser inventory")
    expected = read_json(root / "scripts/release_test_inventory.json")["browser_non_sample_cases"]
    non_sample = [list(identity(row)) for row in rows if sample_key(row) is None]
    if not expected or len(expected) != len({tuple(row) for row in expected}) or sorted(non_sample) != sorted(expected):
        raise ValueError("browser discovery differs from the reviewed non-sample inventory")
    keys = [entry["key"] for entry in contract["samples"]]
    sample_rows = [row for row in rows if sample_key(row)]
    if not keys or len(keys) != len(set(keys)) or sorted(map(sample_key, sample_rows)) != sorted(keys):
        raise ValueError("discovered browser sample cases differ from reviewed catalog")
    catalog_rows = [row for row in rows if row["file"].endswith("release-catalog.spec.ts")]
    if len(catalog_rows) != 1:
        raise ValueError("fresh catalog boundary is missing or duplicated")
    return rows, sample_rows, catalog_rows


def filtered_discovery(discovery, wanted):
    result = copy.deepcopy(discovery)
    def visit(suite):
        specs = []
        for spec in suite.get("specs", []):
            spec["tests"] = [test for test in spec["tests"]
                             if (spec["file"], spec["title"], test["projectName"]) in wanted]
            if spec["tests"]:
                specs.append(spec)
        suite["specs"] = specs
        for child in suite.get("suites", []):
            visit(child)
    visit(result)
    return result


def validate_partial_rows(discovery, actual):
    """A failed batch may donate completed single-attempt leaves, never its verdict."""
    from release_gate import browser_rows, validate_browser_report
    expected = {identity(row) for row in browser_rows(discovery)}
    rows = browser_rows(actual)
    actual_ids = [identity(row) for row in rows]
    if len(actual_ids) != len(set(actual_ids)) or set(actual_ids) != expected:
        raise ValueError("browser batch inventory differs from exact discovery")
    if actual.get("errors"):
        return []
    good = []
    for row in rows:
        selected = {identity(row)}
        try:
            validate_browser_report(filtered_discovery(discovery, selected),
                                    filtered_discovery(actual, selected))
            good.append(row)
        except ValueError:
            continue
    return good


def invoke_playwright(arguments, **kwargs):
    return subprocess.run(arguments, **kwargs)


def run(root, store, manifest, output, jobs, fresh, context):
    from release_gate import (FRONTEND, digest, file_hash, read_json,
                              validate_browser_report, write_json)
    import release_equivalence
    provenance = validate_context(root, store, manifest, output, jobs, fresh, context)
    output.mkdir(parents=True, exist_ok=False)
    artifact = context["artifact"]
    narrow_reuse = ("**" not in context["stage_contract"]["inputs"] and
                    release_equivalence.reviewed("browser", context["inputs"], context["policy"],
                                                 context["equivalence_contract"]))
    program = digest({"inputs": program_inputs(context["inputs"], context["policy"]), "policy": context["policy"],
                      "tools": context["tools"], "environment": context["environment"],
                      "managed_artifacts": context["managed_artifacts"], "contract": SCHEMA,
                      "equivalence_contract": context["equivalence_contract"]})
    environment = dict(os.environ, GEOSOLVE_E2E_ARTIFACT_MANIFEST=str(manifest))
    environment.pop("GEOSOLVE_BROWSER_PREFIX_ONLY", None)
    base = ["./node_modules/.bin/playwright", "test", *FILES, "--workers", str(min(jobs, 2)), "--reporter=json"]
    def command(label, *, discover=False, selected=None, prefix=False):
        destination = output / label
        destination.mkdir()
        env = dict(environment, GEOSOLVE_BROWSER_WITNESS_OUTPUT=str(destination / "witnesses"),
                   M92_BROWSER_AUDIT_OUTPUT=str(destination / "audit"))
        if prefix:
            env["GEOSOLVE_BROWSER_PREFIX_ONLY"] = "1"
        arguments = [*base, "--output", str(destination / "results")]
        if selected is not None:
            # Playwright grep sees project/file/title together. The escaped title
            # suffix is followed by exact discovery/result identity reconciliation.
            titles = sorted({row[1] for row in selected})
            arguments += ["--grep", "(?:" + "|".join(re.escape(title) for title in titles) + ")$"]
        if discover:
            arguments.append("--list")
        started = time.monotonic()
        with (destination / "report.json").open("w") as stdout, (destination / "stderr.log").open("w") as stderr:
            result = invoke_playwright(arguments, cwd=root / FRONTEND, env=env, stdout=stdout, stderr=stderr)
        write_json(destination / "process.json", {"command": arguments, "exit_code": result.returncode,
                   "seconds": time.monotonic() - started, "prefix_only": prefix, "discovery": discover})
        return result.returncode, read_json(destination / "report.json"), destination
    code, discovery, _ = command("discovery", discover=True)
    if code:
        raise ValueError("browser discovery failed")
    contract = read_json(root / CATALOG)
    rows, sample_rows, catalog_rows = validate_discovery(root, discovery, contract)
    ids = [identity(row) for row in rows]
    prefix_ids = {identity(row) for row in sample_rows + catalog_rows}
    code, prefix_report, prefix_dir = command("prefix", selected=prefix_ids, prefix=True)
    validate_browser_report(filtered_discovery(discovery, prefix_ids), prefix_report)
    if code:
        raise ValueError("fresh catalog/sample prefix failed")
    witnesses, leaf_keys, reused = {}, {}, {}
    verified_batches = {}
    for row in sample_rows:
        key = sample_key(row)
        witness = validate_witness(read_json(prefix_dir / "witnesses" / (key + ".json")), key)
        witnesses[key] = witness
        leaf_keys[key] = leaf_key(program, sample_inputs(root, context["snapshot"], key), row, witness)
        prior = None if fresh or not narrow_reuse else find_leaf(store, leaf_keys[key], row, verified_batches)
        if prior:
            reused[identity(row)] = prior
    full_ids = set(ids) - {identity(row) for row in catalog_rows} - set(reused)
    completed, full_dir, full_code = [], None, 0
    if full_ids:
        full_code, report, full_dir = command("full", selected=full_ids)
        completed = validate_partial_rows(filtered_discovery(discovery, full_ids), report)
        if full_code == 0:
            validate_browser_report(filtered_discovery(discovery, full_ids), report)
    validate_context(root, store, manifest, output, jobs, fresh, context)
    fresh_ids = {identity(row) for row in completed}
    leaves = []
    evidence = {str(path): file_hash(path) for path in full_dir.rglob("*") if path.is_file()} if full_dir else {}
    batch = None
    if completed and narrow_reuse:
        batch_path = output / "batch.json"
        write_json(batch_path, store.seal({"schema": "geosolve-browser-batch-v1", "origin_run": context["run"],
                   "program": program, "artifact": artifact, "provenance": provenance,
                   "passed_cases": sorted(fresh_ids), "evidence": evidence | provenance}))
        batch = {"path": str(batch_path), "sha256": file_hash(batch_path)}
    for row in completed:
        key = sample_key(row)
        if key is None or not narrow_reuse:
            continue
        full_witness = validate_witness(read_json(full_dir / "witnesses" / (key + ".json")), key)
        data = sample_inputs(root, context["snapshot"], key)
        captured_key = leaf_key(program, data, row, full_witness)
        value = {"schema": SCHEMA, "key": captured_key, "case": identity(row), "status": "passed",
                 "complete": True, "origin_run": context["run"], "artifact": artifact,
                 "program": program, "sample": data, "witness": full_witness, "batch": batch,
                 "provenance": provenance,
                 "prefix_matches_full": full_witness == witnesses[key]}
        path = store.path / "browser-leaves" / (captured_key + ".json")
        write_json(path, store.seal(value))
        leaves.append({"case": identity(row), "decision": "run", "receipt": str(path),
                       "prefix_matches_full": value["prefix_matches_full"]})
    for case, prior in reused.items():
        leaves.append({"case": case, "decision": "reuse", "origin_run": prior["origin_run"],
                       "original_artifact": prior["artifact"], "receipt_key": prior["key"]})
    complete = full_code == 0 and fresh_ids | set(reused) | {identity(row) for row in catalog_rows} == set(ids)
    write_json(output / "coverage.json", {"schema": SCHEMA, "status": "passed" if complete else "failed",
               "artifact": artifact, "program": program, "cases": sorted(ids), "leaves": leaves,
               "fresh_cases": sorted(fresh_ids), "fresh_prefix_cases": sorted(prefix_ids),
               "reused_cases": sorted(reused), "fresh": fresh, "reviewed_narrow_reuse": narrow_reuse})
    if not complete:
        raise ValueError("browser execution failed; only independent successful leaves retained")
