#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Repository-local release scheduling and authenticated, conservative result reuse.

The trusted store is private to the current OS user and checkout. Receipts are
HMAC authenticated, content addressed, and never accepted from external logs.
This is an integrity boundary against accidental edits/untrusted imported results,
not protection against the OS user who owns the checkout and signing key.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import dataclasses
import fnmatch
import fcntl
import hashlib
import hmac
import json
import os
from pathlib import Path
import platform
import re
import secrets
import shutil
import signal
import subprocess
import sys
import threading
import time
import tomllib
import uuid

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = 1
FRONTEND = "crates/geosolve-demo-web/frontend"
POLICY_PATH = "scripts/release_policy.json"


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def file_hash(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read_json(path):
    return json.loads(Path(path).read_text())


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + "." + uuid.uuid4().hex + ".tmp")
    temporary.write_bytes(canonical(value) + b"\n")
    os.replace(temporary, path)


def capture(command, root=ROOT):
    return subprocess.check_output(command, cwd=root, text=True, stderr=subprocess.PIPE).strip()


def source_files(root):
    paths = capture(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], root)
    return sorted(set(filter(None, paths.split("\0"))))


def source_snapshot(root):
    result = {}
    for name in source_files(root):
        path = root / name
        if path.is_symlink():
            result[name] = {"link": os.readlink(path), "target": file_hash(path) if path.is_file() else None}
        elif path.is_file():
            result[name] = {"sha256": file_hash(path), "executable": bool(path.stat().st_mode & 0o111)}
        else:
            result[name] = None
    return result


def matches(name, patterns):
    return any(fnmatch.fnmatchcase(name, pattern) for pattern in patterns)


@dataclasses.dataclass(frozen=True)
class Stage:
    id: str
    commands: tuple
    inputs: tuple = ()
    dependencies: tuple = ()
    cwd: str = "."
    env: tuple = ()
    resource: str = "normal"
    timeout: int = 3600
    outputs: tuple = ()
    reusable: bool = True
    cases: tuple = ()
    kind: str = "test"
    artifacts: tuple = ()
    execution_key: object = None
    test_inventories: tuple = ()

    def contract(self):
        return dataclasses.asdict(self)


def validate_inventory(stages):
    ids = [stage.id for stage in stages]
    if len(ids) != len(set(ids)):
        raise ValueError("duplicate release stage")
    all_ids = set(ids)
    for stage in stages:
        if not re.fullmatch(r"[A-Za-z0-9_.:-]+", stage.id):
            raise ValueError(f"unsafe stage ID: {stage.id}")
        if not stage.commands or stage.timeout <= 0:
            raise ValueError(f"incomplete stage: {stage.id}")
        if stage.resource not in {"normal", "memory", "exclusive"}:
            raise ValueError(f"unknown resource class: {stage.id}")
        if len(stage.cases) != len(set(stage.cases)):
            raise ValueError(f"duplicate selected case: {stage.id}")
        if not set(stage.dependencies) <= all_ids:
            raise ValueError(f"missing dependency: {stage.id}")
    pending = {stage.id: set(stage.dependencies) for stage in stages}
    while pending:
        ready = {name for name, deps in pending.items() if not deps}
        if not ready:
            raise ValueError("cyclic release inventory")
        pending = {name: deps - ready for name, deps in pending.items() if name not in ready}


def stage_inputs(stage, snapshot, policy):
    selected = {}
    for name, value in snapshot.items():
        known = matches(name, policy["prose"] + policy["global"] + policy["owned"])
        if not known or matches(name, policy["global"]) or matches(name, stage.inputs):
            selected[name] = value
    # Explicit missing paths are inputs too. Adding a previously absent file invalidates.
    for name in stage.inputs:
        if not any(c in name for c in "*?["):
            selected.setdefault(name, None)
    return selected


def effective_environment():
    # Do not persist arbitrary user environment (which may contain credentials).
    # Hash all inherited values conservatively, excluding process/UI bookkeeping.
    ignored = {"_", "PWD", "OLDPWD", "SHLVL", "TERM", "COLORTERM", "TERM_PROGRAM", "TERM_PROGRAM_VERSION",
               "TMPDIR", "TMP", "TEMP", "TEMPDIR", "NIX_BUILD_TOP", "GEOSOLVE_ALLOW_DIRTY"}
    return {name: hashlib.sha256(value.encode()).hexdigest() for name, value in sorted(os.environ.items())
            if name not in ignored and not name.startswith(("VSCODE_", "ELECTRON_"))}


def tool_identity(root):
    tools = {}
    for tool, arguments in (("rustc", ["-vV"]), ("cargo", ["--version"]),
                            ("node", ["--version"]), ("npm", ["--version"]),
                            ("deno", ["--version"]), ("wasm-bindgen", ["--version"]),
                            ("wasm-opt", ["--version"]), ("wasm-bindgen-test-runner", ["--version"]),
                            ("cargo-deny", ["--version"]), ("python3", ["--version"])):
        executable = shutil.which(tool)
        if not executable:
            tools[tool] = None
            continue
        try:
            version = capture([executable, *arguments], root)
        except subprocess.CalledProcessError:
            version = "version-command-failed"
        tools[tool] = {"path": str(Path(executable).resolve()), "sha256": file_hash(executable), "version": version}
    browser = os.environ.get("GEOSOLVE_CHROMIUM_PATH") or shutil.which("google-chrome")
    if browser:
        tools["browser"] = {"path": str(Path(browser).resolve()), "sha256": file_hash(browser),
                            "version": capture([browser, "--version"], root)}
    # Cargo user configuration affects builds even when repository sources match.
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    tools["cargo_config"] = {str(p): file_hash(p) for p in (cargo_home / "config", cargo_home / "config.toml") if p.is_file()}
    tools["host"] = {"node": platform.node(), "platform": platform.platform(), "cpu": os.cpu_count()}
    return tools


class Store:
    def __init__(self, path, create=True):
        self.path = Path(path)
        if create:
            self.path.mkdir(mode=0o700, parents=True, exist_ok=True)
        if self.path.is_symlink():
            raise ValueError("receipt store cannot be a symlink")
        if self.path.exists() and (self.path.stat().st_uid != os.getuid() or self.path.stat().st_mode & 0o022):
            raise ValueError("receipt store must be owned by this user and not writable by others")
        key = self.path / "authentication.key"
        if create and not key.exists():
            fd = os.open(key, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, "wb") as stream:
                stream.write(secrets.token_bytes(32))
        self.key = key.read_bytes() if key.exists() else None
        if key.exists() and (key.is_symlink() or key.stat().st_mode & 0o077 or len(self.key) != 32):
            raise ValueError("invalid receipt authentication key")

    def seal(self, value):
        if not self.key:
            raise ValueError("read-only store has no authentication key")
        return {"payload": value, "hmac_sha256": hmac.new(self.key, canonical(value), hashlib.sha256).hexdigest()}

    def unseal(self, path):
        if not self.key:
            return None
        try:
            envelope = read_json(path)
            payload = envelope["payload"]
            actual = hmac.new(self.key, canonical(payload), hashlib.sha256).hexdigest()
            return payload if hmac.compare_digest(actual, envelope["hmac_sha256"]) else None
        except (OSError, ValueError, KeyError, TypeError):
            return None

    def find(self, stage_id, key):
        receipt = self.unseal(self.path / "results" / stage_id / (key + ".json"))
        if not receipt or receipt.get("schema") != SCHEMA or receipt.get("key") != key:
            return None
        if receipt.get("status") != "passed" or receipt.get("stage") != stage_id or not receipt.get("complete"):
            return None
        try:
            for name, expected in receipt["evidence"].items():
                path = Path(name)
                if not path.resolve().is_relative_to(self.path.resolve()) or file_hash(path) != expected:
                    return None
            for name, expected in receipt["outputs"].items():
                if hash_output(Path(name)) != expected:
                    return None
        except (OSError, ValueError, KeyError):
            return None
        return receipt

    def save(self, receipt):
        if receipt["status"] == "passed" and receipt["complete"]:
            write_json(self.path / "results" / receipt["stage"] / (receipt["key"] + ".json"), self.seal(receipt))


def hash_output(path):
    return _hash_output(path, set())


def _hash_output(path, ancestors):
    path = Path(path)
    resolved = path.resolve()
    if resolved in ancestors:
        raise ValueError(f"cyclic artifact symlink: {path}")
    if path.is_symlink():
        return {"link": os.readlink(path), "target": _hash_output(resolved, ancestors)}
    if path.is_file():
        return {"sha256": file_hash(path)}
    if path.is_dir():
        entries = {}
        for child in sorted(path.iterdir()):
            entries[child.name] = _hash_output(child, ancestors | {resolved})
        return {"files": entries}
    raise ValueError(f"missing stage output: {path}")


class Runner:
    def __init__(self, root, store, snapshot, policy, tools, jobs=2, fresh=False, run_id=None):
        self.root, self.store, self.snapshot, self.policy, self.tools = root, store, snapshot, policy, tools
        self.jobs, self.fresh = jobs, fresh
        self.run_id = run_id or time.strftime("%Y%m%dT%H%M%S") + "-" + uuid.uuid4().hex[:8]
        self.run_dir = store.path / "runs" / self.run_id
        self.environment = effective_environment()
        self.stop = threading.Event()
        self.results = {}
        self.keys = {}
        self.inventory = []
        self.lock = threading.Lock()
        self.started = time.monotonic()
        self.artifact_hashes = {}
        try:
            self.revision = {"commit": capture(["git", "rev-parse", "HEAD"], root),
                             "tree": capture(["git", "rev-parse", "HEAD^{tree}"], root)}
        except subprocess.CalledProcessError:
            self.revision = {"commit": None, "tree": None}

    def artifact_hash(self, name):
        if name not in self.artifact_hashes:
            path = self.root / name
            self.artifact_hashes[name] = hash_output(path) if path.exists() else None
        return self.artifact_hashes[name]

    def key(self, stage):
        # Preparation receipts may move when an unrelated input changes. Their
        # location is provenance; actual consumed bytes and selected cases are
        # the execution inputs. Normalize only our own content-addressed paths.
        raw_contract = stage.contract()
        if stage.execution_key is not None:
            raw_contract["commands"] = stage.execution_key
        contract = raw_contract if stage.kind == "build" else json.loads(re.sub(
            r"target/release-gate/prepared/[0-9a-f]{64}", "target/release-gate/prepared/{identity}", canonical(raw_contract).decode()))
        return digest({"schema": SCHEMA, "root": str(self.root.resolve()), "contract": contract,
                       "inputs": stage_inputs(stage, self.snapshot, self.policy), "policy": self.policy,
                       "tools": self.tools, "environment": self.environment,
                       "artifacts": {re.sub(r"target/release-gate/prepared/[0-9a-f]{64}", "target/release-gate/prepared/{identity}", name):
                                     self.artifact_hash(name)
                                     for name in stage.artifacts}})

    def prepare(self, stages):
        validate_inventory(stages)
        self.artifact_hashes.clear()
        self.inventory = stages
        remaining = list(stages)
        while remaining:
            ready = [stage for stage in remaining if all(name in self.keys for name in stage.dependencies)]
            for stage in ready:
                self.keys[stage.id] = self.key(stage)
                remaining.remove(stage)

    def decision(self, stage):
        if (self.fresh and stage.kind != "build") or not stage.reusable:
            return "run", "fresh execution requested" if self.fresh else "fresh verification required", None
        prior = self.store.find(stage.id, self.keys[stage.id])
        if prior:
            return "reuse", ("authenticated prepared build; no test execution claimed" if stage.kind == "build" else
                             "authenticated successful result with identical complete inputs"), prior
        return "run", "no verifiable successful result for these inputs", None

    def execute(self, stage, ready_at):
        key = self.keys[stage.id]
        decision, reason, previous = self.decision(stage)
        if previous:
            return {"stage": stage.id, "key": key, "status": "passed", "decision": decision,
                    "reason": reason, "origin_run": previous["run"], "receipt_path": previous["receipt_path"],
                    "duration_seconds": 0, "avoided_seconds": previous["duration_seconds"]}
        directory = self.run_dir / "stages" / stage.id
        directory.mkdir(parents=True, exist_ok=False)
        scratch = directory / "scratch"
        scratch.mkdir()
        log = directory / "output.log"
        env = os.environ.copy()
        env.update(dict(stage.env))
        for name in ("TMPDIR", "TMP", "TEMP", "TEMPDIR", "NIX_BUILD_TOP"):
            env[name] = str(scratch)
        env["CARGO_BUILD_JOBS"] = env.get("CARGO_BUILD_JOBS", "2")
        env["M92_AUDIT_OUTPUT"] = str(directory / "sample-audit")
        env["M92_PRODUCT_EVIDENCE"] = str(directory / "product-audit")
        began = time.monotonic()
        status, codes = "passed", []
        resources = []
        with log.open("wb") as output:
            for index, command in enumerate(stage.commands):
                log_start = output.tell()
                command = [argument.replace("{scratch}", str(scratch)).replace("{stage_dir}", str(directory)) for argument in command]
                output.write((json.dumps(command) + "\n").encode())
                output.flush()
                resource_file = directory / f"resource-{index}.json"
                timed = [shutil.which("time"), "-f", '{"user_seconds":%U,"system_seconds":%S,"peak_rss_kib":%M}',
                         "-o", str(resource_file), *command] if shutil.which("time") else command
                process = subprocess.Popen(timed, cwd=self.root / stage.cwd, env=env,
                                           stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
                while process.poll() is None:
                    if self.stop.wait(0.1) or time.monotonic() - began > stage.timeout:
                        status = "interrupted" if self.stop.is_set() else "timed_out"
                        try:
                            os.killpg(process.pid, signal.SIGTERM)
                            process.wait(timeout=7)
                        except subprocess.TimeoutExpired:
                            os.killpg(process.pid, signal.SIGKILL)
                            process.wait()
                        except ProcessLookupError:
                            pass
                        break
                codes.append(process.wait())
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                if resource_file.is_file():
                    try:
                        resources.append(json.loads(resource_file.read_text().splitlines()[-1]))
                    except (ValueError, IndexError):
                        resources.append({"unavailable": True})
                if status != "passed" or codes[-1] != 0:
                    status = status if status != "passed" else "failed"
                    break
                if stage.test_inventories and stage.test_inventories[index]:
                    output.flush()
                    with log.open("rb") as stream:
                        stream.seek(log_start)
                        transcript = stream.read().decode(errors="replace")
                    try:
                        validate_test_output(transcript, stage.test_inventories[index])
                    except ValueError as error:
                        output.write((str(error) + "\n").encode())
                        status = "failed"
                        break
        outputs = {}
        if source_snapshot(self.root) != self.snapshot:
            status = "source_changed"
        if status == "passed":
            try:
                outputs = {str(self.root / name): hash_output(self.root / name) for name in stage.outputs}
                if stage.id in {"prepare.workspace", "prepare.headless"}:
                    prepared = read_json(self.root / stage.outputs[0] / "prepared.json")
                    for artifact in prepared["artifacts"]:
                        if file_hash(artifact["executable"]) != artifact["sha256"]:
                            raise ValueError("prepared Cargo executable changed")
                        outputs[artifact["executable"]] = hash_output(Path(artifact["executable"]))
                    for auxiliary in prepared.get("auxiliary_executables", []):
                        outputs[auxiliary["path"]] = hash_output(Path(auxiliary["path"]))
            except (OSError, ValueError) as error:
                with log.open("a") as output:
                    output.write(str(error) + "\n")
                status = "failed"
        evidence = {str(path): file_hash(path) for path in directory.rglob("*") if path.is_file()}
        receipt = {"schema": SCHEMA, "run": self.run_id, "stage": stage.id, "key": key,
                   "receipt_path": str(directory / "receipt.json"),
                   "status": status, "complete": status == "passed", "decision": "run", "reason": reason,
                   "source_sha256": digest(self.snapshot), "contract": stage.contract(), "tools_sha256": digest(self.tools),
                   "revision": self.revision,
                   "environment_sha256": digest(self.environment), "exit_codes": codes, "outputs": outputs,
                   "resources": resources,
                   "duration_seconds": time.monotonic() - began, "wait_seconds": began - ready_at,
                   "evidence": evidence}
        write_json(directory / "receipt.json", self.store.seal(receipt))
        return receipt

    def run(self, stages, stop_on_failure=True):
        # Already executed prerequisite results may be supplied by a preceding phase.
        pending = list(stages)
        active = {}
        ready_times = {}
        failed = False
        with concurrent.futures.ThreadPoolExecutor(max_workers=self.jobs) as pool:
            while pending or active:
                for stage in list(pending):
                    deps = [self.results.get(name) for name in stage.dependencies]
                    if any(result and result["status"] != "passed" for result in deps):
                        self.results[stage.id] = {"stage": stage.id, "status": "blocked", "reason": "prerequisite failed"}
                        pending.remove(stage)
                        continue
                    if not all(deps) and stage.dependencies:
                        continue
                    ready_times.setdefault(stage.id, time.monotonic())
                    if self.stop.is_set() or (failed and stop_on_failure):
                        self.results[stage.id] = {"stage": stage.id, "status": "not_run", "reason": "run stopped after failure/interruption"}
                        pending.remove(stage)
                        continue
                    if len(active) >= self.jobs:
                        break
                    resources = [entry.resource for entry in active.values()]
                    if "exclusive" in resources or (stage.resource == "exclusive" and active):
                        continue
                    if stage.resource == "memory" and "memory" in resources:
                        continue
                    future = pool.submit(self.execute, stage, ready_times[stage.id])
                    active[future] = stage
                    pending.remove(stage)
                    print(f"queue {stage.id}", flush=True)
                if not active:
                    if pending:
                        raise ValueError("unresolved scheduling prerequisites")
                    break
                done, _ = concurrent.futures.wait(active, timeout=0.2, return_when=concurrent.futures.FIRST_COMPLETED)
                for future in done:
                    stage = active.pop(future)
                    try:
                        result = future.result()
                    except Exception as error:
                        result = {"stage": stage.id, "status": "harness_error", "reason": str(error)}
                    self.results[stage.id] = result
                    failed |= result["status"] != "passed"
                    print(f"{result['status']:11} {stage.id} ({result.get('duration_seconds', 0):.1f}s)", flush=True)
                    self.report()
        return not failed and all(result["status"] == "passed" for result in self.results.values())

    def report(self, final=False, scope="development"):
        unchanged = source_snapshot(self.root) == self.snapshot if final else None
        complete = final and unchanged and len(self.results) == len(self.inventory) and all(
            result["status"] == "passed" for result in self.results.values())
        dirty = bool(capture(["git", "status", "--porcelain"], self.root))
        value = {"schema": SCHEMA, "run": self.run_id, "scope": scope, "complete": bool(complete),
                 "clean_source": not dirty, "qualified_release": bool(complete and scope == "release" and not dirty),
                 "status": "passed" if complete else ("running" if not final else "failed"),
                 "source_unchanged": unchanged, "source": self.snapshot,
                 "revision": self.revision,
                 "tools": self.tools, "environment_sha256": digest(self.environment),
                 "wall_seconds": time.monotonic() - self.started,
                 "inventory": [stage.contract() for stage in self.inventory],
                 "results": [{key: result[key] for key in ("stage", "key", "status", "complete", "decision", "reason",
                               "origin_run", "receipt_path", "duration_seconds", "wait_seconds", "resources", "avoided_seconds")
                              if key in result} for _, result in sorted(self.results.items())]}
        self.run_dir.mkdir(parents=True, exist_ok=True)
        write_json(self.run_dir / "qualification.json", self.store.seal(value))
        if final and unchanged:
            # A failed overall run can contribute independently completed successes.
            for result in self.results.values():
                if result.get("decision") == "run" and result.get("complete"):
                    self.store.save(result)
        return value


def crate_inputs(root, package, include_tests=True):
    """Cargo path-dependency closure; dependency tests are not production inputs."""
    result, seen = set(), set()
    def visit(directory, owner=False):
        if directory in seen:
            return
        seen.add(directory)
        rel = directory.relative_to(root).as_posix()
        manifest = tomllib.loads((directory / "Cargo.toml").read_text())
        result.update((f"{rel}/src/**", f"{rel}/assets/**", f"{rel}/build.rs", f"{rel}/Cargo.toml", f"{rel}/README.md"))
        if owner and include_tests:
            result.update((f"{rel}/tests/**", f"{rel}/examples/**", f"{rel}/benches/**"))
        sections = [manifest.get(section, {}) for section in ("dependencies", "build-dependencies", "dev-dependencies")]
        for target in manifest.get("target", {}).values():
            sections.extend(target.get(section, {}) for section in ("dependencies", "build-dependencies", "dev-dependencies"))
        for section in sections:
            for dependency in section.values():
                if isinstance(dependency, dict) and "path" in dependency:
                    visit((directory / dependency["path"]).resolve())
    visit(root / "crates" / package, True)
    return tuple(sorted(result))


def cmd(*arguments):
    return tuple(arguments)


def npm(prefix, *arguments):
    return cmd("npm", "--prefix", prefix, *arguments)


def preflight_stages():
    return [
        Stage("preflight.inventory", (cmd(sys.executable, "scripts/release_gate.py", "--check-inventory"),
                                      cmd("git", "diff", "--check"),
                                      cmd(sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests", "-p", "test_*.py"),
                                      cmd(sys.executable, "-m", "unittest", "discover", "-s", "scripts", "-p", "golden_oracle_test.py")),
              inputs=("**",), reusable=False, timeout=120),
        Stage("preflight.metadata-format", (cmd("cargo", "metadata", "--locked", "--offline", "--format-version", "1"),
                                           cmd("cargo", "fmt", "--all", "--", "--check")),
              inputs=("crates/**",), dependencies=("preflight.inventory",), timeout=120),
        Stage("preflight.managed", (npm("packages/geosolve-intent", "ci", "--ignore-scripts"),
                                    npm("packages/geosolve-intent", "test"),
                                    npm("packages/geosolve-sketch-code", "ci", "--ignore-scripts"),
                                    npm("packages/geosolve-sketch-code", "test")),
              inputs=("packages/**", "crates/geosolve-sketch-code/assets/**", "crates/geosolve-demo-web/tests/fixtures/**"),
              dependencies=("preflight.inventory",), resource="exclusive", timeout=600,
              outputs=("packages/geosolve-intent/dist", "packages/geosolve-sketch-code/dist",
                       "packages/geosolve-intent/node_modules", "packages/geosolve-sketch-code/node_modules")),
        Stage("preflight.frontend", (npm(FRONTEND, "ci", "--ignore-scripts"),
                                     npm(FRONTEND, "run", "check:static"), npm(FRONTEND, "test")),
              inputs=(FRONTEND + "/**", "packages/**", "crates/geosolve-sketch-code/**"),
              dependencies=("preflight.managed",), resource="exclusive", timeout=600,
              outputs=(FRONTEND + "/node_modules",)),
    ]


def check_inventory(root):
    """Catch M92's stale count before any Cargo/compiler/native work."""
    directory = root / "crates/geosolve-sketch-code/assets/bundled-samples"
    manifests = [read_json(p) for p in sorted(directory.glob("*/manifest.json"))]
    count = len(manifests)
    if not count or sorted(m["ordinal"] for m in manifests) != list(range(1, count + 1)):
        raise ValueError("sample ordinals must be complete and contiguous")
    keys = [m["key"] for m in manifests]
    if len(keys) != len(set(keys)):
        raise ValueError("duplicate sample key")
    generator = (root / "packages/geosolve-sketch-code/scripts/generate-bundled-samples.mjs").read_text()
    expected = re.search(r"assert\.equal\(directories\.length,\s*(\d+)\)", generator)
    if expected is None or int(expected[1]) != count:
        raise ValueError(f"bundled generator count is stale or unknown (catalog has {count})")
    frontend = read_json(root / FRONTEND / "src/data/samples.json")
    rows = frontend if isinstance(frontend, list) else frontend.get("samples", [])
    if len(rows) != count or [row["key"] for row in rows] != [m["key"] for m in sorted(manifests, key=lambda m: m["ordinal"])]:
        raise ValueError("generated frontend inventory differs from catalog")
    print(f"sample preflight: {count} exact entries; generator count and frontend order match")


def preparation_stages(runner):
    # Immutable output paths are tied to source/tool/environment identity. A failed
    # preparation is retried in a new run directory; successful outputs stay put.
    artifact_key = digest({"source": {name: value for name, value in runner.snapshot.items()
                                     if not matches(name, runner.policy["prose"])},
                           "tools": runner.tools, "environment": runner.environment})
    prepared = f"target/release-gate/prepared/{artifact_key}"
    stages = []
    for mode in ("workspace", "headless"):
        out = f"{prepared}/{mode}"
        stages.append(Stage(f"prepare.{mode}", (cmd(sys.executable, "scripts/release_gate.py", "--prepare-native", mode, "--output", out),),
                            inputs=("crates/**", "scripts/release_gate_native.py"),
                            dependencies=("preflight.frontend", "preflight.metadata-format"), resource="exclusive", timeout=2400,
                            outputs=(out,), kind="build"))
    browser_out = f"{prepared}/browser"
    stages.append(Stage("prepare.browser", (cmd(sys.executable, "scripts/release_gate.py", "--prepare-browser", "--output", browser_out),),
                        inputs=("crates/**", "packages/**", "LICENSE", "THIRD_PARTY_LICENSES.md", "API_COMPATIBILITY.md"),
                        dependencies=("preflight.frontend", "preflight.metadata-format"), resource="exclusive", timeout=1200,
                        outputs=(browser_out,), kind="build"))
    return stages, prepared


def native_stages(root, prepared):
    import release_gate_native as native
    result = []
    for mode in ("workspace", "headless"):
        source = root / prepared / mode / "prepared.json"
        value = read_json(source)
        descriptions = native.workspace_stages(value, test_threads=2) if mode == "workspace" else native.ignored_headless_stages(value)
        for description in descriptions:
            package = description["id"].split("::")[0]
            managed = ("packages/geosolve-sketch-code/src/**", "packages/geosolve-sketch-code/scripts/compile*",
                       "packages/geosolve-sketch-code/scripts/mutate*", "packages/geosolve-intent/src/**") if package in {
                           "geosolve-headless", "geosolve-demo-web", "geosolve-sketch-code"} else ()
            managed_artifacts = ("packages/geosolve-sketch-code/dist", "packages/geosolve-sketch-code/node_modules",
                                 "packages/geosolve-intent/dist") if managed else ()
            auxiliary = tuple(item["path"] for item in value.get("auxiliary_executables", []))
            stage_id = mode + "." + description["id"]
            result.append(Stage(stage_id,
                (cmd(sys.executable, "scripts/release_gate.py", "--native-run", str(source), "--native-id", description["id"], "--output", "{scratch}/native"),),
                inputs=(*crate_inputs(root, package), *managed, "scripts/release_gate_native.py"),
                dependencies=(f"prepare.{mode}",), resource="memory" if description["resource"] == "memory-heavy" else "normal",
                timeout=2400, cases=tuple(description["selected"]), artifacts=(description["command"][0], *managed_artifacts, *auxiliary)))
    return result


def qualification_stages(root, prepared, jobs):
    import golden_oracle
    exact_tests = read_json(root / "scripts/release_test_inventory.json")["stages"]
    all_rust = ("crates/**", "README.md", "LICENSE", "API_COMPATIBILITY.md", "THIRD_PARTY_LICENSES.md")
    built = ("prepare.workspace", "prepare.headless", "prepare.browser")
    result = native_stages(root, prepared)
    result.extend([
        Stage("rust.clippy", (cmd("cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"),),
              inputs=all_rust, dependencies=built, resource="exclusive"),
        Stage("rust.doctests", (cmd("cargo", "test", "--locked", "--workspace", "--all-features", "--doc"),),
              inputs=all_rust, dependencies=built, env=(("RUST_MIN_STACK", "16777216"),), resource="exclusive"),
        Stage("rust.documentation", (cmd("cargo", "doc", "--locked", "--workspace", "--all-features", "--no-deps"),),
              inputs=all_rust, dependencies=built, env=(("RUSTDOCFLAGS", "-D warnings"),), resource="exclusive"),
        Stage("rust.bench-build", (cmd("cargo", "bench", "--locked", "--workspace", "--all-features", "--no-run"),),
              inputs=all_rust, dependencies=built, resource="exclusive", kind="build"),
        Stage("golden", (cmd("bash", "scripts/golden-authoring-scene-oracle.sh", "--require-clean", "--jobs", str(jobs),
                             "--prepared-packages", "--output-dir", "{scratch}/golden"),),
              inputs=(*all_rust, "packages/**", "scripts/golden*", "scripts/release_gate_native.py"),
              dependencies=built, resource="exclusive", timeout=3600,
              cases=tuple(case for case, _ in golden_oracle.inventory())),
        Stage("wasm.check", (cmd("cargo", "check", "--locked", "-p", "geosolve-demo-web", "--all-features", "--target", "wasm32-unknown-unknown"),),
              inputs=all_rust, dependencies=built, resource="exclusive"),
        Stage("package.contents", (cmd(sys.executable, "scripts/release_gate.py", "--check-packages"),),
              inputs=(*all_rust, "scripts/verify-geosolve-sketch-code-package.sh"), dependencies=built, resource="exclusive"),
        Stage("package.archive", (cmd("bash", "scripts/verify-geosolve-sketch-code-package.sh"),),
              inputs=(*all_rust, "scripts/verify-geosolve-sketch-code-package.sh"), dependencies=built, resource="exclusive"),
        Stage("licenses", (cmd("cargo", "deny", "check", "licenses") if shutil.which("cargo-deny") else
                           cmd("nix-shell", "-p", "cargo-deny", "--run", "cargo deny check licenses"),),
              inputs=("**/Cargo.toml", "deny.toml", "LICENSE", "THIRD_PARTY_LICENSES.md"), dependencies=built, resource="exclusive"),
    ])
    for target in ("m70_transition_parity", "m71_transition_parity", "m74_reference_geometry", "m75_hover_pointer_parity",
                   "m76_annotation_parity", "m77_curve_control_parity", "m79_inference_lifecycle"):
        result.append(Stage("wasm." + target, (cmd("cargo", "test", "--locked", "-p", "geosolve-constraint-editor", "--test", target,
                                                  "--target", "wasm32-unknown-unknown"),),
                            inputs=crate_inputs(root, "geosolve-constraint-editor"), dependencies=built, resource="exclusive",
                            cases=tuple(exact_tests["wasm." + target]), test_inventories=(tuple(exact_tests["wasm." + target]),),
                            env=(("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER", "wasm-bindgen-test-runner"),)))
    result.append(Stage("wasm.lifecycle", (cmd("cargo", "test", "--locked", "--release", "-p", "geosolve-demo-web", "--lib", "actual_wasm_",
                                                "--target", "wasm32-unknown-unknown"),), inputs=all_rust, dependencies=built,
                        resource="exclusive", cases=tuple(exact_tests["wasm.lifecycle"]), test_inventories=(tuple(exact_tests["wasm.lifecycle"]),),
                        env=(("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER", "wasm-bindgen-test-runner"),)))
    performance = [cmd("cargo", "run", "--locked", "--release", "-p", package, "--example", example)
                   for package, example in (("geosolve-sketch", "m14_performance"), ("geosolve-sketch", "m32_performance"),
                                            ("geosolve-constraint-editor", "m83_performance"))]
    performance += [cmd("cargo", "test", "--locked", "--release", "-p", "geosolve-constraint-editor", "--test", "m83_interaction_performance", "--", "--ignored", "--nocapture", "--test-threads=1"),
                    cmd("cargo", "test", "--locked", "--release", "-p", "geosolve-linkage", "--test", "m23_performance",
                        "exact_auto_sparse_crossover_solves_and_validates_256_moving_body_chain", "--", "--exact", "--ignored", "--nocapture")]
    performance_cases = tuple(exact_tests["performance.interaction"] + exact_tests["performance.linkage"])
    result.append(Stage("performance", tuple(performance), inputs=all_rust, dependencies=built, resource="exclusive",
                        cases=performance_cases, test_inventories=((), (), (), tuple(exact_tests["performance.interaction"]), tuple(exact_tests["performance.linkage"]))))
    manifest = str(root / prepared / "browser/harness.json")
    production = str(root / prepared / "browser/production.json")
    result.append(Stage("browser", (cmd(sys.executable, "scripts/release_gate.py", "--run-browser", manifest,
                                       "--jobs", str(min(jobs, 2)), "--output", "{scratch}/browser"),),
                        inputs=(FRONTEND + "/**", "crates/**", "packages/**"), dependencies=built,
                        resource="memory", env=(("GEOSOLVE_E2E_ARTIFACT_MANIFEST", manifest),),
                        artifacts=(str(root / prepared / "browser/geosolve-harness"),), timeout=2400))
    result.append(Stage("artifact.transport", (cmd(sys.executable, "scripts/release_gate.py", "--verify-production", production,
                                                     "--output", "{scratch}/transport.json"),),
                        inputs=(FRONTEND + "/scripts/**",), dependencies=built, resource="memory", reusable=False, timeout=180,
                        artifacts=(str(root / prepared / "browser/geosolve-production"),)))
    # Start the longest independent browser workload alongside native suites;
    # resource admission still permits only one memory-heavy stage at a time.
    return sorted(result, key=lambda stage: 0 if stage.id == "browser" else 1)


def validate_test_output(text, expected):
    names = re.findall(r"^test (\S+) \.\.\. ", text, re.MULTILINE)
    if not expected or sorted(names) != sorted(expected) or len(names) != len(set(names)):
        raise ValueError("test execution did not cover the exact reviewed case inventory")
    summaries = re.findall(r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (?:\d+ measured; )?(\d+) filtered out;", text, re.MULTILINE)
    if len(summaries) != 1 or tuple(map(int, summaries[0][:3])) != (len(expected), 0, 0):
        raise ValueError("test execution lacks a complete successful nonempty reviewed summary")


def browser_rows(report):
    rows = []
    def visit(suite):
        for spec in suite.get("specs", []):
            for test in spec.get("tests", []):
                rows.append({"title": spec["title"], "file": spec["file"], "project": test["projectName"],
                             "test": test})
        for child in suite.get("suites", []):
            visit(child)
    visit(report)
    return rows


def validate_browser_report(discovery, actual):
    expected_rows, rows = browser_rows(discovery), browser_rows(actual)
    identity = lambda row: (row["file"], row["title"], row["project"])
    expected, completed = list(map(identity, expected_rows)), list(map(identity, rows))
    if not expected or len(expected) != len(set(expected)) or sorted(expected) != sorted(completed):
        raise ValueError("browser completed inventory differs from discovery")
    if actual.get("errors"):
        raise ValueError("browser harness emitted errors")
    for row in rows:
        test = row["test"]
        results = test.get("results", [])
        if test.get("expectedStatus") != "passed" or test.get("status") != "expected" or len(results) != 1:
            raise ValueError(f"browser case skipped, flaky, retried or unexpected: {row['title']}")
        if results[0].get("status") != "passed" or results[0].get("retry", 0) != 0 or results[0].get("errors"):
            raise ValueError(f"browser case did not freshly pass: {row['title']}")
    return completed


def run_browser(manifest, output, jobs):
    output.mkdir(parents=True, exist_ok=False)
    environment = dict(os.environ, GEOSOLVE_E2E_ARTIFACT_MANIFEST=str(manifest), M92_BROWSER_AUDIT_OUTPUT=str(output / "audit"))
    command = ["./node_modules/.bin/playwright", "test", "tests/e2e/language-service.spec.ts",
               "tests/e2e/workbench.spec.ts", "tests/e2e/m92-sample-audit.spec.ts", "--workers", str(min(jobs, 2)),
               "--output", str(output / "results"), "--reporter=json"]
    for filename, suffix in (("inventory.json", ["--list"]), ("results.json", [])):
        with (output / filename).open("w") as stream:
            subprocess.run(command + suffix, cwd=ROOT / FRONTEND, env=environment, stdout=stream, check=True)
    completed = validate_browser_report(read_json(output / "inventory.json"), read_json(output / "results.json"))
    write_json(output / "coverage.json", {"status": "passed", "cases": completed})


def prepare_output(output, action):
    # Never overwrite failed or previously captured preparation evidence.
    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists():
        os.rename(output, output.with_name(output.name + ".superseded-" + uuid.uuid4().hex[:8]))
    action(output)


def verify_production(manifest, output):
    import socket
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    environment = dict(os.environ, GEOSOLVE_E2E_ARTIFACT_MANIFEST=str(manifest), GEOSOLVE_E2E_PORT=str(port))
    server = subprocess.Popen(["node", "scripts/serve-artifact.mjs"], cwd=ROOT / FRONTEND,
                              env=environment, start_new_session=True)
    def interrupted(*_):
        raise InterruptedError("production verification interrupted")
    previous = signal.signal(signal.SIGTERM, interrupted)
    try:
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if server.poll() is not None:
                raise ValueError("artifact server exited during startup")
            with socket.socket() as probe:
                if probe.connect_ex(("127.0.0.1", port)) == 0:
                    break
            time.sleep(0.1)
        else:
            raise ValueError("artifact server readiness timeout")
        subprocess.run(["node", "scripts/verify-artifact.mjs", "--manifest", str(manifest), "--url", f"http://127.0.0.1:{port}/",
                        "--receipt", str(output)], cwd=ROOT / FRONTEND, check=True, timeout=120)
    finally:
        if server.poll() is None:
            os.killpg(server.pid, signal.SIGTERM)
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(server.pid, signal.SIGKILL)
                server.wait()
        signal.signal(signal.SIGTERM, previous)


def check_packages():
    packages = ["geosolve-geometry", "geosolve-core", "geosolve-sketch", "geosolve-linkage", "geosolve-sketch-features",
                "geosolve-sketch-intent", "geosolve-sketch-ops", "geosolve-sketch-topology", "geosolve-constraint-editor",
                "geosolve-sketch-code", "geosolve-sketch-render", "geosolve-headless"]
    for package in packages:
        contents = capture(["cargo", "package", "--locked", "--allow-dirty", "--list", "-p", package]).splitlines()
        if not {"LICENSE", "README.md"} <= set(contents):
            raise ValueError(f"package lacks required release metadata: {package}")
        print(f"package metadata: {package}")


def documentation_delta(root, since, policy):
    capture(["git", "rev-parse", "--verify", since + "^{commit}"], root)
    changes = capture(["git", "diff", "--name-only", since, "--"], root).splitlines()
    changes += capture(["git", "ls-files", "--others", "--exclude-standard"], root).splitlines()
    changes = sorted(set(changes))
    if not changes or any(not matches(name, policy["prose"]) for name in changes):
        raise ValueError("documentation mode requires a nonempty, exclusively reviewed prose diff")
    # The mode checks prose, not newly packaged products (README is package data).
    # Also reject explicit embedded Markdown inputs even under a prose directory.
    for name in source_files(root):
        path = root / name
        if path.suffix not in {".rs", ".py", ".js", ".mjs", ".ts"} or not path.is_file():
            continue
        for line in path.read_text(errors="replace").splitlines():
            if re.search(r"include_(?:str|bytes)!|readFile|\.read_text\(|open\(", line):
                if any(Path(changed).name in line for changed in changes):
                    raise ValueError(f"changed Markdown may be consumed by {name}; use affected qualification")
    return changes


def verify_docs(root, since, policy):
    import unicodedata
    from urllib.parse import unquote
    started = time.monotonic()
    changed = documentation_delta(root, since, policy)
    subprocess.run(["git", "diff", "--check", since], cwd=root, check=True)
    def anchors(path):
        result, counts = set(), {}
        for line in path.read_text().splitlines():
            match = re.match(r"^#{1,6}\s+(.+?)\s*#*$", line)
            if not match:
                continue
            text = re.sub(r"<[^>]+>", "", match[1]).lower()
            slug = "".join(c for c in text if c in "-_ " or not unicodedata.category(c).startswith(("P", "S"))).replace(" ", "-")
            number = counts.get(slug, 0)
            counts[slug] = number + 1
            result.add(slug if not number else f"{slug}-{number}")
        return result
    checked = 0
    for name in changed:
        path = root / name
        if not path.is_file():
            continue
        delta = capture(["git", "diff", "--unified=0", since, "--", name], root)
        lines = [line[1:] for line in delta.splitlines() if line.startswith("+") and not line.startswith("+++")]
        if not delta:
            lines = path.read_text().splitlines()
        for line in lines:
            for link in re.findall(r"\[[^\]]*\]\(([^)]+)\)", line):
                if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", link):
                    continue
                destination, _, anchor = unquote(link).partition("#")
                target = path.parent / destination if destination else path
                if not target.exists() or (anchor and target.suffix == ".md" and anchor not in anchors(target)):
                    raise ValueError(f"broken added link in {name}: {link}")
                checked += 1
    store = Store(root / "target/release-gate")
    report = {"schema": SCHEMA, "scope": "documentation", "status": "passed", "qualified_product": False,
              "since": capture(["git", "rev-parse", since], root), "source": source_snapshot(root),
              "changed": changed, "checked_links": checked, "wall_seconds": time.monotonic() - started,
              "reason": "prose verification only; existing qualified product evidence and bytes retain their original identities"}
    path = store.path / "docs" / (uuid.uuid4().hex + ".json")
    write_json(path, store.seal(report))
    print(f"documentation passed: {len(changed)} files, {checked} added links, {report['wall_seconds']:.2f}s; {path}")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", action="store_true", help="explain stage execution/reuse without running tests")
    parser.add_argument("--fresh", action="store_true", help="bypass test-result reuse (ordinary build caches remain)")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--preflight", action="store_true", help="run only cheap preparation checks; no release claim")
    parser.add_argument("--prepare", action="store_true", help="prepare and discover all executable inventories without running test bodies")
    parser.add_argument("--stage", action="append", default=[], help="run matching stage IDs and prerequisites; reports targeted evidence")
    parser.add_argument("--resume", help="resume a trusted run ID, reusing only independently verified successful work")
    parser.add_argument("--docs-only", action="store_true", help="verify a prose-only diff; does not nominate new product bytes")
    parser.add_argument("--since", help="Git baseline for documentation-only verification")
    parser.add_argument("--test-opt-level", choices=("0", "1"), default="1",
                        help="native test optimization; debug assertions/overflow checks stay enabled (default: 1)")
    parser.add_argument("--check-inventory", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--check-packages", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--prepare-native", choices=("workspace", "headless"), help=argparse.SUPPRESS)
    parser.add_argument("--prepare-browser", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--native-run", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--native-id", help=argparse.SUPPRESS)
    parser.add_argument("--verify-production", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--run-browser", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--output", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.check_inventory:
        check_inventory(ROOT)
        return 0
    if args.check_packages:
        check_packages()
        return 0
    if args.prepare_native:
        import release_gate_native as native
        selection = native.WORKSPACE_ARGS if args.prepare_native == "workspace" else ["-p", "geosolve-headless", *[v for n in native.HEADLESS_TARGETS for v in ("--test", n)]]
        prepare_output(args.output, lambda out: native.prepare(ROOT, selection, out))
        return 0
    if args.prepare_browser:
        prepare_output(args.output, lambda out: subprocess.run(["node", "scripts/build-release-artifacts.mjs", "--out", str(out)],
                                                              cwd=ROOT / FRONTEND, check=True))
        return 0
    if args.native_run:
        import release_gate_native as native
        value = read_json(args.native_run)
        stages = native.workspace_stages(value, test_threads=2) if value["cargo_args"] == native.WORKSPACE_ARGS else native.ignored_headless_stages(value)
        selected = [stage for stage in stages if stage["id"] == args.native_id]
        if len(selected) != 1:
            raise ValueError("native stage selection is missing or duplicated")
        native.run_stage(selected[0], args.output, timeout=2300)
        return 0
    if args.verify_production:
        verify_production(args.verify_production, args.output)
        return 0
    if args.run_browser:
        run_browser(args.run_browser, args.output, args.jobs)
        return 0
    if args.jobs < 1 or args.jobs > 8:
        parser.error("--jobs must be between 1 and 8")
    policy = read_json(ROOT / POLICY_PATH)
    if args.docs_only:
        return verify_docs(ROOT, args.since or "HEAD^", policy)
    if not (args.fresh or args.plan or args.preflight or args.prepare or args.stage or args.resume):
        try:
            documentation_delta(ROOT, args.since or "HEAD^", policy)
        except (ValueError, subprocess.CalledProcessError):
            pass
        else:
            return verify_docs(ROOT, args.since or "HEAD^", policy)
    os.environ["CARGO_PROFILE_TEST_OPT_LEVEL"] = args.test_opt_level
    os.environ["CARGO_PROFILE_TEST_DEBUG_ASSERTIONS"] = "true"
    os.environ["CARGO_PROFILE_TEST_OVERFLOW_CHECKS"] = "true"
    if "GEOSOLVE_CHROMIUM_PATH" not in os.environ and shutil.which("google-chrome"):
        os.environ["GEOSOLVE_CHROMIUM_PATH"] = shutil.which("google-chrome")
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
    if not args.plan and not args.preflight and os.environ.get("GEOSOLVE_ALLOW_DIRTY") != "1" and capture(["git", "status", "--porcelain"], ROOT):
        raise ValueError("release qualification requires a clean tree; GEOSOLVE_ALLOW_DIRTY=1 permits provisional development evidence")
    store = Store(ROOT / "target/release-gate", create=not args.plan)
    if args.resume:
        if not re.fullmatch(r"[A-Za-z0-9-]+", args.resume):
            raise ValueError("resume requires a local trusted run ID")
        original = store.unseal(store.path / "runs" / args.resume / "qualification.json")
        if not original:
            raise ValueError("resume run is missing or unauthenticated")
        if original["source"] == source_snapshot(ROOT):
            for result in original["results"]:
                if result.get("decision") == "run" and result.get("complete"):
                    receipt = store.unseal(result.get("receipt_path", ""))
                    if receipt and receipt.get("source_sha256") == digest(original["source"]):
                        store.save(receipt)
    runner = Runner(ROOT, store, source_snapshot(ROOT), policy, tool_identity(ROOT), args.jobs, args.fresh)
    stages = preflight_stages()
    preparations, prepared = preparation_stages(runner)
    if not args.preflight:
        stages += preparations
    runner.prepare(stages)
    if args.plan:
        for stage in stages:
            action, reason, _ = runner.decision(stage)
            print(f"{action:5} {stage.id}: {reason}")
        if not args.preflight:
            if all((ROOT / prepared / mode / "prepared.json").is_file() for mode in ("workspace", "headless")):
                expanded = qualification_stages(ROOT, prepared, args.jobs)
                runner.prepare(stages + expanded)
                for stage in expanded:
                    action, reason, _ = runner.decision(stage)
                    print(f"{action:5} {stage.id}: {reason}")
            else:
                print("run   qualification: native case inventory will be discovered by the declared preparation stages")
        return 0
    lock_file = (store.path / "runner.lock").open("a")
    try:
        fcntl.flock(lock_file, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as error:
        raise ValueError("another gate owns this checkout's mutable build/install state") from error
    previous = {sig: signal.signal(sig, lambda *_: runner.stop.set()) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        result = runner.run(stages)
        if result and not args.preflight:
            expanded = qualification_stages(ROOT, prepared, args.jobs)
            if args.stage:
                selected = {stage.id for stage in expanded if any(fnmatch.fnmatchcase(stage.id, pattern) for pattern in args.stage)}
                if not selected:
                    raise ValueError("--stage matched no qualification stage")
                expanded = [stage for stage in expanded if stage.id in selected]
            if not args.prepare:
                runner.prepare(stages + expanded)
                result = runner.run(expanded)
        scope = "preflight" if args.preflight else "preparation" if args.prepare else "targeted" if args.stage else "release"
        report = runner.report(final=True, scope=scope)
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        lock_file.close()
    print(f"receipt: {runner.run_dir / 'qualification.json'}")
    return 0 if result and report["complete"] else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"release gate: {error}", file=sys.stderr)
        raise SystemExit(1)
