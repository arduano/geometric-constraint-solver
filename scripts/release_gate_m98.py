#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Captured M98 Node/workspace runtime and private, coverage-checked executions."""
from pathlib import Path
import json
import os
import re
import shutil
import subprocess

FRONTEND = "crates/geosolve-demo-web/frontend"
DEPENDENCIES = (FRONTEND + "/node_modules", "packages/geosolve-sketch-code/node_modules",
                "packages/geosolve-intent/node_modules")
GENERATED = ("packages/geosolve-sketch-code/dist", "packages/geosolve-intent/dist",
             "packages/geosolve-engine/dist")
SOURCE_INPUTS = ("scripts/*.mjs", "scripts/release_gate_m98.py", "packages/**",
                 "examples/**", FRONTEND + "/**", "crates/*/tests/fixtures/**",
                 "crates/geosolve-sketch-code/assets/**", "LICENSE", "THIRD_PARTY_LICENSES.md")
GROUPS = {
    "engine.node": ("packages/geosolve-engine/test/*.test.mjs",),
    "folder.node": ("scripts/workspace-*.test.mjs", "scripts/file-workspace-bake.test.mjs"),
    # The current suite replaces the prototype's pre-session HTTP contract.
    "folder.browser": ("scripts/workspace-browser.test.mjs",),
    "example.generator": ("examples/generator-website/scripts/generator.test.mjs",),
    "example.browser": ("examples/generator-website/scripts/browser.test.mjs",),
    "package.m98": ("scripts/package-m98.test.mjs",),
}
REQUIRED = {
    "engine.node": ("engine", "native", "computed", "session"),
    "folder.node": ("workspace-storage", "workspace-session", "workspace-cache", "workspace-loader",
                    "workspace-bridge", "workspace-project", "workspace-cli", "workspace-generator",
                    "workspace-navigation", "workspace-transaction-review", "file-workspace-bake"),
}


def inventory(root, group):
    if group not in GROUPS:
        raise ValueError("unknown M98 qualification group")
    files = sorted({str(path.relative_to(root)) for pattern in GROUPS[group] for path in root.glob(pattern)
                    if group != "folder.node" or str(path.relative_to(root)) not in GROUPS["folder.browser"]})
    if not files:
        raise ValueError(f"missing M98 test obligation: {group}")
    for pattern in GROUPS[group]:
        if not any(root.glob(pattern)):
            raise ValueError(f"missing M98 test file or family: {pattern}")
    names = {Path(name).name.removesuffix(".test.mjs") for name in files}
    if set(REQUIRED.get(group, ())) - names:
        raise ValueError(f"incomplete M98 owning test inventory: {group}")
    return files


def copy_source(root, output):
    import release_gate as gate
    for name in gate.source_files(root):
        if not gate.matches(name, SOURCE_INPUTS):
            continue
        source, target = root / name, output / name
        if source.is_symlink():
            raise ValueError(f"source runtime symlink requires an explicit contract: {name}")
        if source.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)


def prepare(root, wasm_package, output):
    """Only this locked stage writes live engine/runtime outputs, then captures them."""
    import release_gate as gate
    for command in (["node", "packages/geosolve-engine/scripts/build-wasm.mjs"],
                    ["node", "packages/geosolve-engine/scripts/build.mjs"],
                    ["node", FRONTEND + "/scripts/build-workspace.mjs"]):
        subprocess.run(command, cwd=root, check=True)
    repo = output / "repository"
    repo.mkdir(parents=True)
    copy_source(root, repo)
    for name in GENERATED:
        shutil.copytree(root / name, repo / name, symlinks=False)
    shutil.copytree(wasm_package, repo / FRONTEND / "src/generated")
    runtime = repo / "target/m98/workspace-runtime.mjs"
    runtime.parent.mkdir(parents=True)
    shutil.copy2(root / "target/m98/workspace-runtime.mjs", runtime)
    # Preflight is the only installer. These separately hashed dependencies remain
    # source-owned, and every M98 consumer retains the build lock conservatively.
    for name in DEPENDENCIES:
        destination = repo / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        if not (root / name).is_dir():
            raise ValueError(f"missing installed dependency: {name}")
        destination.symlink_to((root / name).resolve(), target_is_directory=True)
    sdk = repo / "packages/geosolve-engine/node_modules/@geosolve/sketch-code"
    sdk.parent.mkdir(parents=True)
    sdk.symlink_to("../../../geosolve-sketch-code", target_is_directory=True)
    gate.write_json(output / "prepared.json", {
        "schema": "geosolve-m98-runtime-v1",
        "tests": {group: inventory(repo, group) for group in GROUPS},
        "dependencies": {name: gate.hash_output(root / name) for name in DEPENDENCIES},
        "tree": gate.hash_output(repo),
    })


def validate_tap(text, files=()):
    """A successful process must also finish a nonempty, unskipped Node inventory."""
    summaries = {}
    for name in ("tests", "pass", "fail", "cancelled", "skipped", "todo"):
        values = re.findall(rf"^# {name} (\d+)\s*$", text, re.MULTILINE)
        if len(values) != 1:
            raise ValueError(f"Node test output lacks one complete {name} summary")
        summaries[name] = int(values[0])
    if (not summaries["tests"] or summaries["pass"] != summaries["tests"]
            or any(summaries[name] for name in ("fail", "cancelled", "skipped", "todo"))):
        raise ValueError("Node test inventory is empty, failed, cancelled or skipped")
    # Node treats an empty test file as an implicit passing file-level case.
    # Every owning file must declare tests rather than donate that fallback.
    names = re.findall(r"^# Subtest: (.+)$", text, re.MULTILINE)
    if any(name == file or name.endswith("/" + file) for name in names for file in files):
        raise ValueError("owning Node test file declares no tests")
    return summaries


def run(root, prepared, group, output, browser_package=None):
    import release_gate as gate
    metadata = gate.read_json(prepared / "prepared.json")
    if metadata.get("schema") != "geosolve-m98-runtime-v1":
        raise ValueError("unsupported M98 runtime preparation")
    if gate.hash_output(prepared / "repository") != metadata["tree"]:
        raise ValueError("captured M98 runtime bytes changed")
    for name, expected in metadata["dependencies"].items():
        if gate.hash_output(root / name) != expected:
            raise ValueError(f"installed M98 runtime dependency changed: {name}")
    output.mkdir(parents=True, exist_ok=False)
    repository = output / "repository"
    shutil.copytree(prepared / "repository", repository, symlinks=True)
    if browser_package is not None:
        shutil.copytree(browser_package, repository / "crates/geosolve-demo-web/dist")
    files = inventory(repository, group)
    if files != metadata["tests"].get(group):
        raise ValueError("M98 discovered test inventory differs from preparation")
    environment = os.environ.copy()
    for name in ("NODE_OPTIONS", "NODE_PATH", "GEOSOLVE_DIST", "GEOSOLVE_LOADER_TEST_SDK_DIRECTORY",
                 "GEOSOLVE_M98_PACKAGES", "GEOSOLVE_M98_PACKAGE_OUT", "GEOSOLVE_M98_DIST"):
        environment.pop(name, None)
    environment["GEOSOLVE_BROWSER_EVIDENCE"] = str(output / "browser")
    environment["npm_config_cache"] = str(output / "npm-cache")
    if group == "package.m98":
        environment["GEOSOLVE_M98_PACKAGE_OUT"] = str(output / "packages")
        environment["GEOSOLVE_M98_DIST"] = str(repository / "crates/geosolve-demo-web/dist")
    if group == "example.generator":
        subprocess.run(["node", "examples/generator-website/scripts/check-types.mjs"],
                       cwd=repository, env=environment, check=True, timeout=120)
    command = ["node", "--test", "--test-concurrency=1", "--test-reporter=tap", *files]
    result = subprocess.run(command, cwd=repository, env=environment, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=1800)
    print(result.stdout, end="", flush=True)
    (output / "node.tap").write_text(result.stdout)
    if result.returncode:
        raise subprocess.CalledProcessError(result.returncode, command)
    counts = validate_tap(result.stdout, files)
    if gate.hash_output(prepared / "repository") != metadata["tree"]:
        raise ValueError("test changed captured M98 runtime bytes")
    for name, expected in metadata["dependencies"].items():
        if gate.hash_output(root / name) != expected:
            raise ValueError(f"test changed installed runtime dependency: {name}")
    gate.write_json(output / "coverage.json", {"status": "passed", "files": files, "summary": counts})
    # Keep screenshots, generated profiles and package evidence after disposing
    # the mutable source/build copy. The prepared runtime remains an input.
    for source, destination in ((repository / "target/m98", output / "artifacts"),
                                (repository / "examples/generator-website/test-output", output / "example-browser")):
        if source.is_dir():
            destination.mkdir(parents=True, exist_ok=True)
            for path in source.iterdir():
                if path.name != "workspace-runtime.mjs":
                    shutil.move(str(path), destination / path.name)
    # Output evidence survives; private mutable source/build copies are disposable.
    shutil.rmtree(repository)
    shutil.rmtree(output / "npm-cache", ignore_errors=True)
