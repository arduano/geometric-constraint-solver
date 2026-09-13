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
                "packages/geosolve-intent/node_modules", "packages/geosolve-cli/node_modules/esbuild",
                "packages/geosolve-cli/node_modules/@esbuild")
GENERATED = ("packages/geosolve-sketch-code/dist", "packages/geosolve-intent/dist",
             "packages/geosolve-engine/dist", "packages/geosolve-collaboration/dist", "packages/geosolve-cli/dist")
NATIVE_FIXTURE = "target/debug/examples/text_fixture"
FRONTEND_RUNTIME_TESTS = ("src/lib/collaboration-*.test.ts", "src/lib/local-interaction-worker.test.ts",
                          "src/lib/worker-workbench-adapter.test.ts", "src/lib/browsing-initialize.test.ts")
SOURCE_INPUTS = ("scripts/*.mjs", "scripts/release_gate_m98.py", "packages/**",
                 "examples/**", FRONTEND + "/**", "crates/*/tests/fixtures/**",
                 "crates/geosolve-sketch-code/assets/**", "LICENSE", "THIRD_PARTY_LICENSES.md")
GROUPS = {
    "engine.node": ("packages/geosolve-engine/test/*.test.mjs",),
    "folder.node": ("scripts/workspace-*.test.mjs", "scripts/file-workspace-bake.test.mjs", "packages/geosolve-cli/test/*.test.mjs"),
    # The current suite replaces the prototype's pre-session HTTP contract.
    "folder.browser": ("scripts/workspace-browser.test.mjs",),
    "example.generator": ("examples/generator-website/scripts/generator.test.mjs",),
    "example.browser": ("examples/generator-website/scripts/browser.test.mjs",),
    "package.m98": ("scripts/package-m98.test.mjs",),
    "collaboration.node": ("scripts/collaboration-*.test.mjs",),
    "collaboration.package": ("packages/geosolve-collaboration/test/*.test.mjs",),
    "collaboration.frontend": tuple(FRONTEND + "/" + pattern for pattern in FRONTEND_RUNTIME_TESTS),
    "collaboration.browser": ("scripts/collaboration-browser.test.mjs", "scripts/collaboration-browser-recovery.test.mjs"),
}
BROWSER_GROUPS = {"folder.browser", "package.m98", "collaboration.browser"}
REQUIRED = {
    "engine.node": ("engine", "native", "computed", "session"),
    "folder.node": ("workspace-storage", "workspace-session", "workspace-cache", "workspace-loader",
                    "workspace-bridge", "workspace-project", "workspace-cli", "workspace-generator",
                    "workspace-navigation", "workspace-transaction-review", "workspace-engine-runtime",
                    "workspace-workbench", "workspace-http", "workspace-actor-review", "workspace-recovery",
                    "workspace-manifold", "file-workspace-bake", "runtime-layout", "worker-lifetime"),
    "collaboration.node": ("collaboration-host", "collaboration-http", "collaboration-storage", "collaboration-runtime",
                           "collaboration-domain", "collaboration-domain-extraction", "collaboration-preview", "collaboration-source-integration", "collaboration-mirror", "collaboration-mirror-worker", "collaboration-cli"),
    "collaboration.package": ("authority", "semantic", "shared-text", "source", "client"),
    "collaboration.frontend": ("collaboration-adapter", "collaboration-authoring-controller", "collaboration-authoring-worker", "collaboration-browsing-worker",
                               "collaboration-remote-authoring", "collaboration-source-projection", "collaboration-storage", "collaboration-tab-identity",
                               "local-interaction-worker", "worker-workbench-adapter", "browsing-initialize"),
}


def inventory(root, group):
    if group not in GROUPS:
        raise ValueError("unknown M98 qualification group")
    exclude = set(GROUPS["folder.browser"] if group == "folder.node" else
                  GROUPS["collaboration.browser"] if group == "collaboration.node" else ())
    files = sorted({str(path.relative_to(root)) for pattern in GROUPS[group] for path in root.glob(pattern)
                    if str(path.relative_to(root)) not in exclude})
    if not files:
        raise ValueError(f"missing M98 test obligation: {group}")
    for pattern in GROUPS[group]:
        if not any(root.glob(pattern)):
            raise ValueError(f"missing M98 test file or family: {pattern}")
    names = {re.sub(r"\.test\.(?:mjs|ts)$", "", Path(name).name) for name in files}
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
                    ["node", "packages/geosolve-collaboration/scripts/build-wasm.mjs"],
                    ["node", "packages/geosolve-collaboration/scripts/build.mjs"]):
        subprocess.run(command, cwd=root, check=True)
    built_fixture = subprocess.run(
        ["cargo", "build", "--locked", "-p", "geosolve-collaboration", "--example", "text_fixture", "--message-format=json"],
        cwd=root, check=True, text=True, stdout=subprocess.PIPE)
    native_fixture = fixture_executable(built_fixture.stdout)
    subprocess.run(["node", "packages/geosolve-cli/scripts/build.mjs"], cwd=root, check=True)
    repo = output / "repository"
    repo.mkdir(parents=True)
    copy_source(root, repo)
    for name in GENERATED:
        shutil.copytree(root / name, repo / name, symlinks=False)
    shutil.copytree(wasm_package, repo / FRONTEND / "src/generated")
    fixture = repo / NATIVE_FIXTURE
    fixture.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(native_fixture, fixture)
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
    # Resolve host dependencies through package exports in the captured tree.
    # Installed file: links point into the live checkout and must not be captured.
    for name in ("sketch-code", "engine", "collaboration"):
        dependency = repo / "packages/geosolve-cli/node_modules/@geosolve" / name
        dependency.parent.mkdir(parents=True, exist_ok=True)
        dependency.symlink_to("../../../geosolve-" + name, target_is_directory=True)
    gate.write_json(output / "prepared.json", {
        "schema": "geosolve-m98-runtime-v1",
        "tests": {group: inventory(repo, group) for group in GROUPS},
        "dependencies": {name: gate.hash_output(root / name) for name in DEPENDENCIES},
        "tree": gate.hash_output(repo),
    })


def fixture_executable(messages):
    """Capture Cargo's actual output, including an explicitly configured target directory."""
    artifacts = [row for line in messages.splitlines() if line.strip() for row in [json.loads(line)]
                 if row.get("reason") == "compiler-artifact" and row.get("target", {}).get("name") == "text_fixture"
                 and row["target"].get("kind") == ["example"] and row.get("executable")]
    if len(artifacts) != 1:
        raise ValueError("Cargo did not report exactly one native text fixture executable")
    executable = Path(artifacts[0]["executable"])
    if not executable.is_absolute() or executable.is_symlink() or not executable.is_file():
        raise ValueError("native text fixture must be a reported ordinary executable file")
    return executable


def validate_frontend_report(report, files, repository):
    """Every registered Vitest file must finish with actual passing assertions."""
    if not report.get("success") or report.get("numTotalTests", 0) <= 0:
        raise ValueError("frontend collaboration run did not pass a nonempty inventory")
    if report.get("numPassedTests") != report["numTotalTests"] or any(
            report.get(name, 0) for name in ("numFailedTests", "numPendingTests", "numTodoTests", "numFailedTestSuites")):
        raise ValueError("frontend collaboration inventory failed or skipped")
    completed = []
    for result in report.get("testResults", []):
        path = Path(result["name"])
        completed.append(str(path.relative_to(repository)))
        assertions = result.get("assertionResults", [])
        if result.get("status") != "passed" or not assertions or any(row.get("status") != "passed" for row in assertions):
            raise ValueError("frontend collaboration file lacks passing assertions")
    if sorted(completed) != sorted(files):
        raise ValueError("frontend collaboration completed file inventory differs from discovery")
    return {"tests": report["numTotalTests"], "pass": report["numPassedTests"]}


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


def cleanup_runtime(output):
    """Retain selected diagnostics, then dispose only this run's mutable copies.

    The parent may repeat this after terminating a stage process tree, before
    sealing its evidence, when interruption prevented the child's finally block.
    """
    output = Path(output)
    if output.is_symlink():
        raise ValueError("M98 execution output cannot be a symlink")
    repository = output / "repository"

    def remove_private(path):
        if path.is_symlink():
            path.unlink()
        elif path.exists():
            shutil.rmtree(path)

    try:
        for source, destination in ((repository / "target/m98", output / "artifacts"),
                                    (repository / "examples/generator-website/test-output", output / "example-browser")):
            if not source.exists() and not source.is_symlink():
                continue
            if repository.is_symlink() or source.is_symlink() or not source.resolve().is_relative_to(repository.resolve()):
                raise ValueError("M98 diagnostic directory escapes its private runtime")
            if not source.is_dir():
                raise ValueError("M98 diagnostic output must be a directory")
            if destination.is_symlink():
                raise ValueError("M98 diagnostic destination cannot be a symlink")
            destination.mkdir(parents=True, exist_ok=True)
            for path in source.iterdir():
                target = destination / path.name
                if target.exists() or target.is_symlink():
                    raise ValueError("M98 diagnostic destination already exists")
                shutil.move(str(path), target)
    finally:
        try:
            remove_private(repository)
        finally:
            remove_private(output / "npm-cache")


def _run_logged(command, cwd, environment, path, timeout):
    """Write output as it arrives, including partial output before termination."""
    import release_gate_native as native
    process_output = path.with_name(path.stem + "-process")
    try:
        result = native.process(command, cwd, environment, process_output, timeout)
    finally:
        # The existing bounded process owner stops every descendant before
        # diagnostics move or mutable runtime directories are removed.
        text = "".join(log.read_text(errors="replace") for name in ("stdout.log", "stderr.log")
                       if (log := process_output / name).is_file())
        path.write_text(text)
        print(text, end="", flush=True)
    if result["status"] == "timeout":
        raise subprocess.TimeoutExpired(command, timeout, output=text)
    if result["status"] != "passed":
        raise subprocess.CalledProcessError(result["exit_code"], command, output=text)
    return text


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
    try:
        _run_private(root, prepared, group, output, browser_package, metadata, repository)
    finally:
        cleanup_runtime(output)


def _run_private(root, prepared, group, output, browser_package, metadata, repository):
    import release_gate as gate
    shutil.copytree(prepared / "repository", repository, symlinks=True)
    if browser_package is not None:
        shutil.copytree(browser_package, repository / "crates/geosolve-demo-web/dist")
    files = inventory(repository, group)
    if files != metadata["tests"].get(group):
        raise ValueError("M98 discovered test inventory differs from preparation")
    environment = os.environ.copy()
    for name in ("NODE_OPTIONS", "NODE_PATH", "GEOSOLVE_DIST", "GEOSOLVE_LOADER_TEST_SDK_DIRECTORY",
                 "GEOSOLVE_M98_PACKAGES", "GEOSOLVE_M98_PACKAGE_OUT", "GEOSOLVE_M98_DIST",
                 "GEOSOLVE_BROWSER_DIAGNOSTIC_NO_BACKDROP", "GEOSOLVE_BROWSER_DIAGNOSTIC_NO_TOOLBAR_BACKDROP"):
        environment.pop(name, None)
    environment["GEOSOLVE_BROWSER_EVIDENCE"] = str(output / "browser")
    environment["npm_config_cache"] = str(output / "npm-cache")
    environment.pop("GEOSOLVE_COLLABORATION_NATIVE_FIXTURE", None)
    if group == "collaboration.package":
        environment["GEOSOLVE_COLLABORATION_NATIVE_FIXTURE"] = str(repository / NATIVE_FIXTURE)
    if group in BROWSER_GROUPS:
        environment["GEOSOLVE_DIST"] = str(repository / "crates/geosolve-demo-web/dist")
    if group == "package.m98":
        environment["GEOSOLVE_M98_PACKAGE_OUT"] = str(output / "packages")
        environment["GEOSOLVE_M98_DIST"] = str(repository / "crates/geosolve-demo-web/dist")
    if group == "example.generator":
        _run_logged(["node", "examples/generator-website/scripts/check-types.mjs"],
                   repository, environment, output / "typecheck.log", 120)
    if group == "collaboration.frontend":
        report_path = output / "frontend.json"
        command = ["node", "node_modules/vitest/vitest.mjs", "run", "--no-cache", "--reporter=json",
                   "--outputFile", str(report_path), *[str(Path(file).relative_to(FRONTEND)) for file in files]]
        cwd = repository / FRONTEND
    else:
        command = ["node", "--test", "--test-concurrency=1", "--test-reporter=tap", *files]
        cwd = repository
    transcript = _run_logged(command, cwd, environment,
                            output / ("frontend.log" if group == "collaboration.frontend" else "node.tap"), 1800)
    counts = validate_frontend_report(gate.read_json(report_path), files, repository) if group == "collaboration.frontend" else validate_tap(transcript, files)
    if gate.hash_output(prepared / "repository") != metadata["tree"]:
        raise ValueError("test changed captured M98 runtime bytes")
    for name, expected in metadata["dependencies"].items():
        if gate.hash_output(root / name) != expected:
            raise ValueError(f"test changed installed runtime dependency: {name}")
    gate.write_json(output / "coverage.json", {"status": "passed", "files": files, "summary": counts})
