#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Authenticate one parent's browser invocation and its prepared artifact provenance."""
from __future__ import annotations
import os
from pathlib import Path

SCHEMA = "geosolve-browser-context-v1"


def environment_identity(environment):
    from release_gate import digest
    ignored = {"_", "PWD", "OLDPWD", "SHLVL", "GEOSOLVE_RELEASE_BROWSER_CONTEXT",
               "TMPDIR", "TMP", "TEMP", "TEMPDIR", "NIX_BUILD_TOP"}
    return {name: digest(value) for name, value in sorted(environment.items()) if name not in ignored}


def private_path(store, path):
    path = Path(path).resolve()
    if not path.is_relative_to(store.path.resolve()):
        raise ValueError("browser provenance path escapes the private receipt store")
    return path


def preparation_receipt(store, path, expected_stage, expected_key):
    from release_gate import file_hash, hash_output
    path = private_path(store, path)
    value = store.unseal(path)
    if (not value or value.get("stage") != expected_stage or value.get("key") != expected_key
            or value.get("status") != "passed" or value.get("complete") is not True):
        raise ValueError("browser preparation receipt is missing, unauthenticated or incomplete")
    for filename, expected in value["evidence"].items():
        if file_hash(private_path(store, filename)) != expected:
            raise ValueError("browser preparation evidence changed")
    for filename, expected in value["outputs"].items():
        if hash_output(filename) != expected:
            raise ValueError("browser preparation output changed")
    return value


def artifact_identity(manifest):
    from release_gate import file_hash, hash_output, read_json
    manifest = Path(manifest).resolve()
    value = read_json(manifest)
    if value.get("format") != "geosolve-release-artifact-v1" or value.get("kind") != "harness":
        raise ValueError("browser invocation requires the prepared harness manifest")
    directory = Path(value["directory"])
    if (manifest.is_symlink() or not directory.is_absolute() or directory.is_symlink()
            or not directory.is_dir() or directory.resolve().parent != manifest.parent):
        raise ValueError("browser harness directory is not an absolute regular directory")
    return {"manifest": str(manifest), "manifest_sha256": file_hash(manifest),
            "kind": "harness", "directory": str(directory.resolve()),
            "files_sha256": value["filesSha256"], "observed": hash_output(directory)}


def parent_context(runner, stage, output, environment):
    from release_gate import FRONTEND, file_hash, hash_output, read_json, stage_inputs
    import release_equivalence
    if stage.id != "browser":
        raise ValueError("browser context belongs only to the browser stage")
    output = Path(output).resolve()
    expected_output = (runner.run_dir / "stages/browser/scratch/browser").resolve()
    if output != expected_output:
        raise ValueError("browser output differs from the current parent run")
    artifact = artifact_identity(dict(stage.env)["GEOSOLVE_E2E_ARTIFACT_MANIFEST"])
    if artifact["directory"] not in {str((runner.root / name).resolve()) for name in stage.artifacts}:
        raise ValueError("browser harness is absent from parent stage artifacts")
    preparations = []
    for name in ("prepare.wasm", "prepare.browser"):
        result = runner.results.get(name)
        if name == "prepare.wasm" and result is None:
            continue
        if not result or result.get("status") != "passed" or result.get("key") != runner.keys[name]:
            raise ValueError("browser preparation has not passed for current parent inputs")
        receipt = preparation_receipt(runner.store, result["receipt_path"], name, runner.keys[name])
        if name == "prepare.browser" and str(Path(artifact["manifest"]).parent) not in receipt["outputs"]:
            raise ValueError("browser manifest is not covered by its preparation receipt")
        preparations.append({"stage": name, "key": runner.keys[name], "path": result["receipt_path"],
                             "sha256": file_hash(result["receipt_path"])})
    consumed = {name: hash_output(runner.root / name) for name in stage.artifacts}
    # Playwright itself is a consumed executable dependency of every leaf.
    frontend_modules = FRONTEND + "/node_modules"
    consumed[frontend_modules] = hash_output(runner.root / frontend_modules)
    for name in stage.artifacts:
        if consumed[name] != runner.artifact_hash(name):
            raise ValueError("browser input artifact changed since parent stage planning")
    try:
        contract = read_json(runner.root / release_equivalence.CONTRACT_PATH)
    except (OSError, ValueError):
        contract = {}
    context_path = (runner.run_dir / "stages/browser/browser-context.json").resolve()
    return {"schema": SCHEMA, "run": runner.run_id, "root": str(runner.root.resolve()),
            "run_dir": str(runner.run_dir.resolve()), "output": str(output),
            "context_path": str(context_path), "stage": stage.id, "stage_key": runner.keys[stage.id],
            "stage_contract": stage.contract(), "artifact": artifact, "preparations": preparations,
            "consumed_artifacts": consumed, "snapshot": runner.snapshot,
            "inputs": stage_inputs(stage, runner.snapshot, runner.policy),
            "policy": runner.policy, "tools": runner.tools, "environment": runner.environment,
            "invocation_environment": environment_identity(environment), "fresh": runner.fresh,
            "managed_artifacts": {name: value for name, value in consumed.items()
                                  if name.startswith(("packages/", FRONTEND + "/node_modules"))},
            "equivalence_contract": contract}


def validate_context(root, store, manifest, output, jobs, fresh, context, *, environment=None):
    from release_gate import canonical, file_hash, hash_output, source_snapshot
    environment = os.environ if environment is None else environment
    if context.get("schema") != SCHEMA or context.get("stage") != "browser":
        raise ValueError("invalid browser parent context schema/stage")
    if context.get("root") != str(root.resolve()) or context.get("output") != str(output.resolve()):
        raise ValueError("browser invocation root/output differs from signed parent")
    run_dir = private_path(store, store.path / "runs" / context["run"])
    if context["run_dir"] != str(run_dir) or output.resolve() != run_dir / "stages/browser/scratch/browser":
        raise ValueError("browser output does not belong to the signed run")
    context_path = private_path(store, context["context_path"])
    if context_path != run_dir / "stages/browser/browser-context.json":
        raise ValueError("browser context does not belong to the signed run")
    supplied_path = environment.get("GEOSOLVE_RELEASE_BROWSER_CONTEXT", "")
    if not supplied_path or Path(supplied_path).resolve() != context_path:
        raise ValueError("browser context invocation path differs")
    signed = store.unseal(context_path)
    if signed is None or canonical(signed) != canonical(context):
        raise ValueError("browser parent context is unauthenticated or differs from its signed payload")
    if environment_identity(environment) != context["invocation_environment"]:
        raise ValueError("browser invocation environment differs from signed parent")
    if type(context.get("fresh")) is not bool or fresh != context["fresh"]:
        raise ValueError("browser fresh mode differs from signed parent")
    commands = context["stage_contract"]["commands"]
    if len(commands) != 1:
        raise ValueError("browser parent command is ambiguous")
    command = commands[0]
    if (command[command.index("--run-browser") + 1] != str(manifest)
            or int(command[command.index("--jobs") + 1]) != jobs):
        raise ValueError("browser invocation differs from signed parent command")
    if source_snapshot(root) != context["snapshot"]:
        raise ValueError("browser source differs from parent stage inputs")
    if artifact_identity(manifest) != context["artifact"]:
        raise ValueError("browser artifact differs from the signed prepared artifact")
    for name, expected in context["consumed_artifacts"].items():
        if hash_output(root / name) != expected:
            raise ValueError("browser consumed artifact changed")
    for value in context["tools"].values():
        if isinstance(value, dict) and {"path", "sha256"} <= value.keys():
            if file_hash(value["path"]) != value["sha256"]:
                raise ValueError("browser tool executable changed")
    browser_receipt = None
    for item in context["preparations"]:
        if file_hash(private_path(store, item["path"])) != item["sha256"]:
            raise ValueError("browser preparation provenance changed")
        receipt = preparation_receipt(store, item["path"], item["stage"], item["key"])
        if item["stage"] == "prepare.browser":
            browser_receipt = receipt
    if not browser_receipt or str(Path(context["artifact"]["manifest"]).parent) not in browser_receipt["outputs"]:
        raise ValueError("browser context lacks authenticated artifact build provenance")
    return {str(context_path): file_hash(context_path),
            **{item["path"]: item["sha256"] for item in context["preparations"]}}
