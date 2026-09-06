#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Prepare and execute Cargo-owned native test stages without recompiling each test.

The parent gate owns scheduling, input/reuse policy and final coverage. This adapter
preserves Cargo's actual runtime environment (including dynamic-library paths) by
reading its verbose --list launch records, never by evaluating shell text. Unknown
Cargo launch formats or test harnesses fail closed. Doctests remain Cargo-owned.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import signal
import subprocess
import threading
import time
from typing import Any

SCHEMA = "geosolve-native-stages-v2"
HEADLESS_BINARY_ENV = "GEOSOLVE_RELEASE_HEADLESS_BINARY"
WORKSPACE_ARGS = ["--workspace", "--all-features"]
HEADLESS_TARGETS = ["m87_headless", "m92_atlas_scale_intent", "m92_product_design_intent",
                    "m92_mechanism_design_edits", "m92_product_reference_intent"]
HEADLESS_EXACT = [
    "inspect_edit_solve_and_static_render_share_one_exact_control_authority",
    "all_bundled_samples_apply_declared_edit_undo_redo_and_reload",
    "cli_inspect_render_and_edit_are_browser_free_and_never_overwrite_outputs",
]
MEMORY_HEAVY = {("geosolve-demo-web", "geosolve_demo_web"),
                ("geosolve-headless", "m87_headless"),
                ("geosolve-headless", "m92_atlas_scale_intent")}


class NativeError(RuntimeError):
    """Preparation or test evidence did not satisfy the native execution contract."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise NativeError(message)


def file_hash(path: str | Path) -> str:
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path: Path, value: Any) -> None:
    with path.open("x", encoding="utf8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def process(command: list[str], cwd: str | Path, env: dict[str, str], output: Path,
            timeout: float) -> dict[str, Any]:
    """Run a bounded process group; retain outputs and failure evidence on every exit."""
    require(timeout > 0, "execution timeout must be positive")
    output.mkdir(parents=True, exist_ok=False)
    started = time.time()
    monotonic = time.monotonic()
    state, code = "interrupted", None
    child = None
    previous_handler = None
    if threading.current_thread() is threading.main_thread():
        def interrupted(signum, _frame):
            raise InterruptedError(f"native execution interrupted by signal {signum}")
        previous_handler = signal.signal(signal.SIGTERM, interrupted)
    try:
        with (output / "stdout.log").open("xb") as stdout, (output / "stderr.log").open("xb") as stderr:
            child = subprocess.Popen(command, cwd=cwd, env=env, stdout=stdout,
                                     stderr=stderr, start_new_session=True)
            try:
                code = child.wait(timeout=timeout)
                state = "passed" if code == 0 else "failed"
            except subprocess.TimeoutExpired:
                state = "timeout"
            finally:
                if child.poll() is None:
                    try:
                        os.killpg(child.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                    try:
                        child.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        try:
                            os.killpg(child.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        child.wait(timeout=2)
                # Descendants can ignore TERM or outlive a failed test process.
                # A completed stage never retains its private process group.
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                code = child.returncode
    finally:
        if previous_handler is not None:
            signal.signal(signal.SIGTERM, previous_handler)
        receipt = dict(schema=SCHEMA, command=command, cwd=str(cwd), start_unix=started,
                       elapsed_seconds=time.monotonic() - monotonic, timeout_seconds=timeout,
                       status=state, exit_code=code)
        receipt["logs"] = {name: {"path": str(output / name), "sha256": file_hash(output / name)}
                           for name in ("stdout.log", "stderr.log") if (output / name).exists()}
        write_json(output / "process.json", receipt)
    return receipt


def checked_process(command: list[str], cwd: str | Path, env: dict[str, str], output: Path,
                    timeout: float) -> str:
    result = process(command, cwd, env, output, timeout)
    require(result["status"] == "passed", f"child {result['status']}: {command}; evidence: {output}")
    return (output / "stdout.log").read_text(encoding="utf8")


def cargo_artifacts(text: str, metadata: dict[str, Any]) -> list[dict[str, Any]]:
    packages = {p["id"]: p for p in metadata["packages"] if p["id"] in metadata["workspace_members"]}
    artifacts, finished = {}, []
    for line in text.splitlines():
        value = json.loads(line)
        if value.get("reason") == "build-finished":
            finished.append(value.get("success"))
        if value.get("reason") != "compiler-artifact" or not value.get("profile", {}).get("test"):
            continue
        if value["package_id"] not in packages or not value.get("executable"):
            continue
        package, target = packages[value["package_id"]], value["target"]
        require(not set(target["kind"]) & {"bench", "custom-build"}, "unexpected native test target kind")
        key = (package["name"], target["name"], tuple(target["kind"]))
        require(key not in artifacts, f"duplicate Cargo test executable: {key}")
        artifacts[key] = dict(package=package["name"], package_id=package["id"], target=target,
                              executable=value["executable"], features=value["features"],
                              profile=value["profile"], cwd=str(Path(package["manifest_path"]).parent))
    require(finished == [True], "Cargo JSON lacks one successful build-finished record")
    require(bool(artifacts), "Cargo produced no workspace test executables")
    return [artifacts[key] for key in sorted(artifacts)]


def auxiliary_executables(text: str, metadata: dict[str, Any]) -> list[dict[str, str]]:
    """Discover non-test binaries; launch environments decide which tests depend on them."""
    members = set(metadata["workspace_members"])
    result = {}
    for line in text.splitlines():
        value = json.loads(line)
        if (value.get("reason") == "compiler-artifact" and value.get("executable")
                and value.get("package_id") in members and not value.get("profile", {}).get("test")
                and set(value["target"]["kind"]) & {"bin", "example"}):
            path = value["executable"]
            result[path] = dict(path=path)
    return [result[path] for path in sorted(result)]


def preserve_auxiliary_executables(artifacts: list[dict[str, Any]], launches: dict[str, dict[str, str]],
                                   auxiliary: list[dict[str, str]], output: Path) -> list[dict[str, str]]:
    """Bind only applicable Cargo binary dependencies to immutable preparation copies.

    Cargo reuses target/debug/<binary> between feature selections. The copy must
    therefore be consumed at runtime: ordinary CARGO_BIN_EXE_* lookups are rebound,
    and the headless test's compile-time env! fallback has an explicit runtime override.
    Copies are private to this preparation, never hard links to Cargo's mutable output.
    """
    available = {item["path"] for item in auxiliary}
    copies = {}
    for artifact in artifacts:
        environment = dict(launches[artifact["executable"]])
        dependencies = []
        for key, source in sorted(environment.items()):
            if not key.startswith("CARGO_BIN_EXE_"):
                continue
            require(source in available, f"Cargo launch binary is missing from compiler artifacts: {key}")
            if source not in copies:
                source_hash = file_hash(source)
                directory = output / "auxiliary" / hashlib.sha256(source.encode()).hexdigest()[:16]
                directory.mkdir(parents=True, exist_ok=False)
                captured = directory / Path(source).name
                shutil.copyfile(source, captured)
                require(file_hash(captured) == source_hash, "Cargo auxiliary executable changed while copying")
                captured.chmod(Path(source).stat().st_mode & 0o555)
                copies[source] = dict(path=str(captured), sha256=source_hash, source_path=source)
            captured = copies[source]
            runtime_env = HEADLESS_BINARY_ENV if key == "CARGO_BIN_EXE_geosolve-headless" else key
            environment[key] = captured["path"]
            environment[runtime_env] = captured["path"]
            dependencies.append(captured | dict(cargo_env=key, runtime_env=runtime_env))
        artifact["env"] = environment
        artifact["auxiliary_executables"] = dependencies
    return [copies[source] for source in sorted(copies)]


def expected_targets(metadata: dict[str, Any], cargo_args: list[str]) -> set[tuple[str, str, tuple[str, ...]]]:
    """Reconcile compiler discovery with Cargo metadata instead of trusting a count."""
    packages, tests, bins, examples = set(), set(), set(), set()
    flags = set()
    arguments = iter(cargo_args)
    for argument in arguments:
        if argument in ("-p", "--package", "--test", "--bin", "--example"):
            try:
                value = next(arguments)
            except StopIteration as error:
                raise NativeError(f"missing Cargo selector value: {argument}") from error
            {"-p": packages, "--package": packages, "--test": tests,
             "--bin": bins, "--example": examples}[argument].add(value)
        else:
            require(argument in ("--workspace", "--all-features", "--lib", "--tests"),
                    f"unsupported native Cargo selector: {argument}")
            flags.add(argument)
    require(("--workspace" in flags) != bool(packages), "choose exactly workspace or explicit packages")
    workspace = [p for p in metadata["packages"] if p["id"] in metadata["workspace_members"]]
    require(packages <= {p["name"] for p in workspace}, "selected package is absent from workspace metadata")
    result = set()
    explicit = tests or bins or examples or flags & {"--lib", "--tests"}
    for package in workspace:
        if packages and package["name"] not in packages:
            continue
        for target in package["targets"]:
            kinds = set(target["kind"])
            library = bool(kinds & {"lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"})
            if not target["test"] and not ("example" in kinds and target["name"] in examples):
                continue
            selected = not explicit or (library and flags & {"--lib", "--tests"}) or ("--tests" in flags and kinds & {"bin", "test"})
            selected = selected or ("test" in kinds and target["name"] in tests) or ("bin" in kinds and target["name"] in bins) or ("example" in kinds and target["name"] in examples)
            if selected:
                require(not target.get("required-features"), "feature-gated target needs explicit selection support")
                result.add((package["name"], target["name"], tuple(target["kind"])))
    require(bool(result), "metadata selected no native test targets")
    return result


def cargo_launches(stderr: str, executables: set[str]) -> dict[str, dict[str, str]]:
    """Decode only known test launches, including Cargo's own environment overrides."""
    launches = {}
    for line in stderr.splitlines():
        match = re.fullmatch(r"\s*Running `(.+)`", line)
        if not match:
            continue
        tokens = shlex.split(match[1])
        overrides = {}
        while tokens and re.match(r"^(?:[A-Za-z_][A-Za-z_0-9]*|CARGO_BIN_EXE_[A-Za-z0-9_.-]+)=", tokens[0]):
            key, value = tokens.pop(0).split("=", 1)
            overrides[key] = value
        if not tokens or tokens[0] not in executables:
            continue
        executable = tokens.pop(0)
        require(tokens == ["--list", "--format", "terse"], "unsupported Cargo test launcher arguments")
        require(executable not in launches, f"duplicate Cargo test launch: {executable}")
        require("CARGO_MANIFEST_DIR" in overrides and "CARGO_MANIFEST_PATH" in overrides,
                "Cargo launch lacks package environment")
        launches[executable] = overrides
    require(set(launches) == executables, "Cargo launch inventory differs from compiler JSON artifacts")
    return launches


def parse_inventory(text: str) -> list[str]:
    cases = []
    for line in text.splitlines():
        if not line.strip():
            continue
        require(line.endswith(": test"), f"unsupported libtest inventory row: {line}")
        cases.append(line[:-6])
    require(len(cases) == len(set(cases)), "duplicate libtest case in inventory")
    return sorted(cases)


def validate_results(text: str, selected: list[str], ignored: list[str], filtered_out: int | None = None) -> None:
    results = {}
    for line in text.splitlines():
        match = re.fullmatch(r"test (.+) \.\.\. (ok|FAILED|ignored)(?:, .*)?", line)
        if match:
            name, status = match.groups()
            require(name not in results, f"duplicate completed test: {name}")
            results[name] = status
    expected = dict.fromkeys(selected, "ok") | dict.fromkeys(ignored, "ignored")
    require(results == expected, "completed libtest case inventory/status differs from the selected inventory")
    summaries = re.findall(r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; "
                           r"(\d+) measured; (\d+) filtered out; finished in .+$", text, re.MULTILINE)
    require(len(summaries) == 1, "libtest lacks one complete successful summary")
    passed, failed, skipped, measured, filtered = map(int, summaries[0])
    require((passed, failed, skipped, measured) == (len(selected), 0, len(ignored), 0),
            "libtest summary contradicts the selected inventory")
    require(filtered_out is None or filtered == filtered_out, "libtest filtered count differs from the selected inventory")


def prepare(root: Path, cargo_args: list[str], output: Path, *, timeout: float = 1800,
            env: dict[str, str] | None = None) -> dict[str, Any]:
    """Compile once, discover test environments/cases; no test bodies are executed.

    cargo_args are Cargo package/target/feature options, e.g. WORKSPACE_ARGS or
    ['-p', 'geosolve-sketch-code', '--test', 'golden_backend_parity'].
    """
    root, output = root.resolve(), output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    environment = dict(os.environ if env is None else env)
    require("--" not in cargo_args and not any(a in cargo_args for a in ("--doc", "--no-run", "--message-format")),
            "prepare accepts Cargo selectors only")
    metadata_text = checked_process(["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"],
                                    root, environment, output / "metadata", timeout)
    metadata = json.loads(metadata_text)
    expected = expected_targets(metadata, cargo_args)
    command = ["cargo", "test", "--locked", *cargo_args, "--no-run", "--message-format=json"]
    compiled = checked_process(command, root, environment, output / "build", timeout)
    auxiliary = auxiliary_executables(compiled, metadata)
    artifacts = cargo_artifacts(compiled, metadata)
    require({(a["package"], a["target"]["name"], tuple(a["target"]["kind"])) for a in artifacts} == expected,
            "Cargo executable inventory differs from selected workspace metadata targets")
    # A single Cargo discovery pass supplies its real dynamic-library and package
    # environment. --list runs no test bodies; doctests remain a separate stage.
    discovery_args = cargo_args if any(a in cargo_args for a in ("--lib", "--test", "--bin", "--example")) else [*cargo_args, "--lib", "--tests"]
    discovery = checked_process(["cargo", "test", "--locked", *discovery_args, "-vv", "--message-format=json",
                     "--", "--list", "--format", "terse"], root, environment, output / "cargo-list", timeout)
    rediscovered = cargo_artifacts("\n".join(line for line in discovery.splitlines() if line.startswith("{")), metadata)
    require(artifacts == rediscovered, "Cargo listing changed the prepared executable/profile/feature inventory")
    launches = cargo_launches((output / "cargo-list/stderr.log").read_text(), {a["executable"] for a in artifacts})
    auxiliary = preserve_auxiliary_executables(artifacts, launches, auxiliary, output)
    for index, artifact in enumerate(artifacts):
        require(artifact["env"]["CARGO_MANIFEST_DIR"] == artifact["cwd"], "Cargo environment has the wrong package cwd")
        artifact["sha256"] = file_hash(artifact["executable"])
        artifact["resource"] = "memory-heavy" if (artifact["package"], artifact["target"]["name"]) in MEMORY_HEAVY else "native"
        for label, flags in (("cases", []), ("ignored", ["--ignored"])):
            text = checked_process([artifact["executable"], *flags, "--list", "--format", "terse"],
                                   artifact["cwd"], environment | artifact["env"], output / f"list-{index}-{label}", 30)
            artifact[label] = parse_inventory(text)
        require(set(artifact["ignored"]) <= set(artifact["cases"]), "ignored inventory not contained in complete inventory")
    result = dict(schema=SCHEMA, root=str(root), cargo_args=cargo_args, artifacts=artifacts, auxiliary_executables=auxiliary,
                  build_command=command, metadata_sha256=hashlib.sha256(metadata_text.encode()).hexdigest())
    write_json(output / "prepared.json", result)
    return result


def execution_stage(artifact: dict[str, Any], *, ignored: bool = False, exact: str | None = None,
                    test_threads: int | None = None, env: dict[str, str] | None = None) -> dict[str, Any]:
    available = artifact["ignored"] if ignored else artifact["cases"]
    require(exact is None or exact in available, f"requested test is absent from selected inventory: {exact}")
    considered = [exact] if exact else available
    skipped = [] if ignored else sorted(set(considered) & set(artifact["ignored"]))
    selected = sorted(set(considered) - set(skipped))
    require(exact is None or bool(selected), "exact test is ignored; select ignored execution explicitly")
    command = [artifact["executable"], "--format", "pretty", "--color", "never"]
    if ignored:
        command.append("--ignored")
    if exact:
        command += [exact, "--exact"]
    if test_threads is not None:
        require(test_threads > 0, "test thread count must be positive")
        command += ["--test-threads", str(test_threads)]
    require(not (set(env or {}) & set(artifact["env"])), "test overrides cannot replace Cargo-owned runtime environment")
    overrides = artifact["env"] | dict(env or {})
    return dict(schema=SCHEMA, id=f"{artifact['package']}::{artifact['target']['name']}::{exact or ('ignored' if ignored else 'normal')}",
                command=command, cwd=artifact["cwd"], env=overrides, resource=artifact["resource"],
                selected=selected, ignored=skipped, executable_sha256=artifact["sha256"],
                filtered_out=len(artifact["cases"]) - len(selected) - len(skipped),
                features=artifact["features"], profile=artifact["profile"], auxiliary_executables=artifact.get("auxiliary_executables", []))


def workspace_stages(prepared: dict[str, Any], *, test_threads: int | None = None) -> list[dict[str, Any]]:
    require(prepared["cargo_args"] == WORKSPACE_ARGS, "workspace qualification requires --workspace --all-features")
    return [execution_stage(a, test_threads=test_threads, env={"RUST_MIN_STACK": "16777216"}) for a in prepared["artifacts"]]


def ignored_headless_stages(prepared: dict[str, Any]) -> list[dict[str, Any]]:
    expected_args = ["-p", "geosolve-headless", *[value for name in HEADLESS_TARGETS for value in ("--test", name)]]
    require(prepared["cargo_args"] == expected_args, "ignored qualification requires the exact default-feature headless target build")
    by_target = {a["target"]["name"]: a for a in prepared["artifacts"]}
    require(set(by_target) == set(HEADLESS_TARGETS), "headless executable inventory changed")
    stages = [execution_stage(by_target["m87_headless"], ignored=True, exact=name,
                              test_threads=1 if "all_bundled" in name else None) for name in HEADLESS_EXACT]
    stages += [execution_stage(by_target[name], ignored=True, test_threads=1) for name in HEADLESS_TARGETS[1:]]
    return stages


def doctest_stage(root: Path) -> dict[str, Any]:
    return dict(schema=SCHEMA, id="native-doctests", command=["cargo", "test", "--locked", "--workspace", "--all-features", "--doc"],
                cwd=str(root.resolve()), env={"RUST_MIN_STACK": "16777216"}, resource="cargo-build")


def run_stage(stage: dict[str, Any], output: Path, *, timeout: float,
              env: dict[str, str] | None = None) -> dict[str, Any]:
    require(stage.get("schema") == SCHEMA, "incompatible native stage schema")
    require(file_hash(stage["command"][0]) == stage["executable_sha256"], "prepared native executable changed")
    for auxiliary in stage.get("auxiliary_executables", []):
        require(file_hash(auxiliary["path"]) == auxiliary["sha256"], "prepared auxiliary executable changed")
        require(stage["env"].get(auxiliary["cargo_env"]) == auxiliary["path"]
                and stage["env"].get(auxiliary["runtime_env"]) == auxiliary["path"],
                "native stage does not consume its prepared auxiliary executable")
    environment = dict(os.environ if env is None else env) | stage["env"]
    result = process(stage["command"], stage["cwd"], environment, output, timeout)
    try:
        require(result["status"] == "passed", f"native child {result['status']}")
        validate_results((output / "stdout.log").read_text(), stage["selected"], stage["ignored"], stage["filtered_out"])
    except NativeError as error:
        write_json(output / "result.json", dict(schema=SCHEMA, status="failed", stage=stage, error=str(error), process=result))
        raise
    receipt = dict(schema=SCHEMA, status="passed", stage=stage, process=result)
    write_json(output / "result.json", receipt)
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--mode", choices=("workspace", "headless"), default="workspace")
    parser.add_argument("--stage-file", type=Path, help="execute one stage from a prepared stages.json")
    parser.add_argument("--stage-id", help="exact ID from --stage-file")
    parser.add_argument("--timeout", type=float, default=1800)
    args = parser.parse_args()
    if args.stage_file:
        require(bool(args.stage_id), "--stage-file requires --stage-id")
        stages = json.loads(args.stage_file.read_text())
        selected = [stage for stage in stages if stage.get("id") == args.stage_id]
        require(len(selected) == 1 and selected[0].get("schema") == SCHEMA, "stage selection is missing, duplicate or incompatible")
        receipt = run_stage(selected[0], args.output, timeout=args.timeout)
        print(json.dumps(dict(status=receipt["status"], stage=args.stage_id)))
        return
    require(args.stage_id is None, "--stage-id requires --stage-file")
    cargo_args = WORKSPACE_ARGS if args.mode == "workspace" else ["-p", "geosolve-headless", *[value for name in HEADLESS_TARGETS for value in ("--test", name)]]
    prepared = prepare(args.root, cargo_args, args.output, timeout=args.timeout)
    stages = workspace_stages(prepared) if args.mode == "workspace" else ignored_headless_stages(prepared)
    write_json(args.output / "stages.json", stages + ([doctest_stage(args.root)] if args.mode == "workspace" else []))
    print(json.dumps(dict(prepared=str(args.output / "prepared.json"), stages=len(stages))))


if __name__ == "__main__":
    main()
