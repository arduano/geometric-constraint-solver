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
import tempfile
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
    excluded_inputs: tuple = ()
    input_equivalence: str | None = None
    input_equivalence_contract: object = None
    dependencies: tuple = ()
    cwd: str = "."
    env: tuple = ()
    resource: str = "normal"
    build_lock: bool = False
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
    if stage.excluded_inputs:
        import release_equivalence
        # Unrecognized source includes and changed program boundaries retain the
        # entire input set. A fresh baseline alone never authorizes exclusions.
        allowed = ("**" not in stage.inputs and stage.input_equivalence and
                   release_equivalence.reviewed(stage.input_equivalence, selected, policy,
                                                 stage.input_equivalence_contract))
        if allowed:
            selected = {name: value for name, value in selected.items()
                        if not (matches(name, stage.excluded_inputs) and
                                release_equivalence.catalog_data(name, value, policy))}
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
                            ("cargo-deny", ["--version"]), ("python3", ["--version"]), ("readelf", ["--version"])):
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


def native_output_hash(directory, frozen_tree, executable):
    """Compare captured native bytes using the output snapshot just read once.

    Preparation holds the build lock while freezing its private output tree.
    Original Cargo fallback paths and paths through symlinks still require their
    own observation. Consumer keys and native execution independently revalidate.
    """
    executable = Path(executable)
    try:
        parts = executable.relative_to(directory).parts
    except ValueError:
        return hash_output(executable)
    observed = frozen_tree
    if parts and ".." not in parts:
        for part in parts:
            if not isinstance(observed, dict) or set(observed) != {"files"}:
                break
            observed = observed["files"].get(part)
        else:
            if isinstance(observed, dict) and set(observed) == {"sha256"}:
                return dict(observed)
    return hash_output(executable)


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
        self.memory_workers = min(jobs, policy.get("memory_heavy_workers", 1))
        if self.memory_workers < 1:
            raise ValueError("memory worker limit must be positive")
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
        self.input_maps = {}
        self.deferred_groups = {}
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

    def inputs(self, stage):
        # The snapshot and policy belong to one frozen run. Stages sharing an
        # input boundary need one scan; distinct exclusions/contracts remain
        # distinct and a new Runner always starts with an empty map.
        identity = canonical((stage.inputs, stage.excluded_inputs, stage.input_equivalence,
                              stage.input_equivalence_contract))
        if identity not in self.input_maps:
            self.input_maps[identity] = stage_inputs(stage, self.snapshot, self.policy)
        return self.input_maps[identity]

    def key(self, stage):
        # Preparation receipts may move when an unrelated input changes. Their
        # location is provenance; actual consumed bytes and selected cases are
        # the execution inputs. Normalize only our own content-addressed paths.
        raw_contract = stage.contract()
        if stage.kind == "native":
            # Only the prepared native adapter separates build admission from
            # test semantics. An unreviewed concurrent writer retains the lock
            # for fresh execution; it cannot invalidate an already completed
            # test of identical code, runtime, inputs and selected cases. Keep
            # the actual lock in the scheduler and full observation contract.
            raw_contract["build_lock"] = False
        if stage.execution_key is not None:
            raw_contract["commands"] = stage.execution_key
        contract = raw_contract if stage.kind == "build" else json.loads(re.sub(
            r"target/release-gate/prepared/[0-9a-f]{64}", "target/release-gate/prepared/{identity}", canonical(raw_contract).decode()))
        return digest({"schema": SCHEMA, "root": str(self.root.resolve()), "contract": contract,
                       "inputs": self.inputs(stage), "policy": self.policy,
                       "tools": self.tools, "environment": self.environment,
                       "artifacts": {re.sub(r"target/release-gate/prepared/[0-9a-f]{64}", "target/release-gate/prepared/{identity}", name):
                                     self.artifact_hash(name)
                                     for name in stage.artifacts}})

    def prepare(self, stages):
        validate_inventory(stages)
        self.artifact_hashes.clear()
        self.input_maps.clear()
        self.inventory = stages
        remaining = list(stages)
        while remaining:
            ready = [stage for stage in remaining if all(name in self.keys for name in stage.dependencies)]
            for stage in ready:
                self.keys[stage.id] = self.key(stage)
                remaining.remove(stage)

    def register(self, stages):
        """Append discovered obligations; active and completed contracts never change."""
        stages = list(stages)
        validate_inventory([*self.inventory, *stages])
        known = set(self.keys)
        for stage in stages:
            if not set(stage.dependencies) <= known:
                raise ValueError("deferred group is not ordered after registered prerequisites")
            known.add(stage.id)
        # A previously absent output must never donate a cached None after its
        # preparation completes. Preserve hashes only for already consumed paths.
        for stage in stages:
            for name in stage.artifacts:
                self.artifact_hashes.pop(name, None)
        keys = {stage.id: self.key(stage) for stage in stages}
        self.inventory.extend(stages)
        self.keys.update(keys)

    def decision(self, stage):
        if (self.fresh and stage.kind != "build") or not stage.reusable:
            return "run", "fresh execution requested" if self.fresh else "fresh verification required", None
        prior = self.store.find(stage.id, self.keys[stage.id])
        if prior:
            return "reuse", ("authenticated prepared build; no test execution claimed" if stage.kind == "build" else
                             "authenticated successful result with identical complete inputs"), prior
        return "run", "no verifiable successful result for these inputs", None

    def execute(self, stage, ready_at):
        decision, reason, previous = self.decision(stage)
        if previous:
            return {"stage": stage.id, "key": self.keys[stage.id], "status": "passed", "decision": decision,
                    "reason": reason, "origin_run": previous["run"], "receipt_path": previous["receipt_path"],
                    "duration_seconds": 0, "avoided_seconds": previous["duration_seconds"]}
        # Chrome's SingletonSocket must fit the OS Unix-socket path limit. Keep
        # runtime temporary files short and private; durable evidence stays in
        # the run directory. Cleanup also applies to timeout and harness errors.
        with tempfile.TemporaryDirectory(prefix="gs-", dir="/tmp") as runtime:
            return self.execute_fresh(stage, ready_at, reason, runtime)

    def execute_fresh(self, stage, ready_at, reason, runtime):
        key = self.keys[stage.id]
        directory = self.run_dir / "stages" / stage.id
        directory.mkdir(parents=True, exist_ok=False)
        scratch = directory / "scratch"
        scratch.mkdir()
        log = directory / "output.log"
        env = os.environ.copy()
        env.update(dict(stage.env))
        for name in ("TMPDIR", "TMP", "TEMP", "TEMPDIR", "NIX_BUILD_TOP"):
            env[name] = runtime
        env["CARGO_BUILD_JOBS"] = env.get("CARGO_BUILD_JOBS", str(self.policy.get("cargo_build_jobs", 2)))
        env["M92_AUDIT_OUTPUT"] = str(directory / "sample-audit")
        env["M92_PRODUCT_EVIDENCE"] = str(directory / "product-audit")
        if stage.id.startswith(("workspace.", "headless.")):
            env["M92_MECHANISM_EVIDENCE"] = str(directory / "mechanism-audit")
            env["DENO_DIR"] = str(Path(runtime) / "deno")
            env = {name: value for name, value in env.items() if not name.startswith("GEOSOLVE_GOLDEN_")}
        if stage.id == "browser":
            from release_browser_context import parent_context
            context = parent_context(self, stage, scratch / "browser", env)
            context_path = directory / "browser-context.json"
            write_json(context_path, self.store.seal(context))
            env["GEOSOLVE_RELEASE_BROWSER_CONTEXT"] = str(context_path)
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
                    prepared_directory = self.root / stage.outputs[0]
                    frozen_tree = outputs[str(prepared_directory)]
                    prepared = read_json(prepared_directory / "prepared.json")
                    for artifact in prepared["artifacts"]:
                        observed = native_output_hash(prepared_directory, frozen_tree, artifact["executable"])
                        if observed != {"sha256": artifact["sha256"]}:
                            raise ValueError("prepared Cargo executable changed")
                        outputs[artifact["executable"]] = observed
                    for auxiliary in prepared.get("auxiliary_executables", []):
                        observed = native_output_hash(prepared_directory, frozen_tree, auxiliary["path"])
                        if observed != {"sha256": auxiliary["sha256"]}:
                            raise ValueError("prepared auxiliary executable changed")
                        outputs[auxiliary["path"]] = observed
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
                   "runtime_temporary_policy": "private /tmp/gs-*; removed after process-tree completion",
                   "duration_seconds": time.monotonic() - began, "wait_seconds": began - ready_at,
                   "evidence": evidence}
        write_json(directory / "receipt.json", self.store.seal(receipt))
        return receipt

    def run(self, stages, stop_on_failure=True, deferred=()):
        # Already executed prerequisite results may be supplied by a preceding phase.
        pending = list(stages)
        active = {}
        ready_times = {}
        failed = False
        factories = {}
        for identity, dependencies, factory in deferred:
            if identity in self.deferred_groups or not set(dependencies) <= set(self.keys):
                raise ValueError("duplicate deferred group or missing preparation prerequisite")
            self.deferred_groups[identity] = {"dependencies": list(dependencies), "status": "pending", "stages": []}
            factories[identity] = factory
        with concurrent.futures.ThreadPoolExecutor(max_workers=self.jobs) as pool:
            while pending or active or factories:
                for identity, factory in list(factories.items()):
                    group = self.deferred_groups[identity]
                    dependencies = [self.results.get(name) for name in group["dependencies"]]
                    if self.stop.is_set() or (failed and stop_on_failure) or any(
                            result and result["status"] != "passed" for result in dependencies):
                        group["status"] = "blocked"
                        del factories[identity]
                        failed = True
                    elif all(dependencies):
                        try:
                            expanded = list(factory())
                            self.register(expanded)
                            pending[0:0] = expanded
                            group.update(status="expanded", stages=[stage.id for stage in expanded])
                        except Exception as error:
                            group.update(status="failed", error=str(error))
                            failed = True
                        del factories[identity]
                # The sole prepared-package frontend writer should start as soon
                # as its WASM dependency and memory slot are available. It may
                # overlap the next Cargo builder under the reviewed program guard.
                pending.sort(key=lambda stage: 0 if stage.id == "prepare.browser" else 1 if stage.build_lock else 2)
                for stage in list(pending):
                    deps = [self.results.get(name) for name in stage.dependencies]
                    if any(result and result["status"] != "passed" for result in deps):
                        self.results[stage.id] = {"stage": stage.id, "status": "blocked", "reason": "prerequisite failed"}
                        pending.remove(stage)
                        continue
                    if self.stop.is_set() or (failed and stop_on_failure):
                        self.results[stage.id] = {"stage": stage.id, "status": "not_run", "reason": "run stopped after failure/interruption"}
                        pending.remove(stage)
                        continue
                    if not all(deps) and stage.dependencies:
                        continue
                    ready_times.setdefault(stage.id, time.monotonic())
                    if len(active) >= self.jobs:
                        break
                    resources = [entry.resource for entry in active.values()]
                    if "exclusive" in resources or (stage.resource == "exclusive" and active):
                        continue
                    if stage.resource == "memory" and resources.count("memory") >= self.memory_workers:
                        continue
                    if stage.build_lock and any(entry.build_lock for entry in active.values()):
                        continue
                    future = pool.submit(self.execute, stage, ready_times[stage.id])
                    active[future] = stage
                    pending.remove(stage)
                    print(f"queue {stage.id}", flush=True)
                if not active:
                    if pending or factories:
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
            result["status"] == "passed" for result in self.results.values()) and all(
            group["status"] == "expanded" for group in self.deferred_groups.values())
        dirty = bool(capture(["git", "status", "--porcelain"], self.root))
        value = {"schema": SCHEMA, "run": self.run_id, "scope": scope, "complete": bool(complete),
                 "clean_source": not dirty, "qualified_release": bool(complete and scope == "release" and not dirty),
                 "status": "passed" if complete else ("running" if not final else "failed"),
                 "source_unchanged": unchanged, "source": self.snapshot,
                 "revision": self.revision,
                 "deferred_groups": self.deferred_groups,
                 "tools": self.tools, "environment_sha256": digest(self.environment),
                 "wall_seconds": time.monotonic() - self.started,
                 "worker_limits": {"stages": self.jobs, "memory_stages": self.memory_workers,
                                   "native_threads_per_stage": self.policy.get("test_threads", 2),
                                   "cargo_build_jobs": os.environ.get("CARGO_BUILD_JOBS", str(self.policy.get("cargo_build_jobs", 2)))},
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


def rust_embedded_inputs(root, directory, source_patterns):
    """Track literal Rust includes outside Cargo's ordinary crate directory.

    Cargo includes are compile inputs even when the referenced file is prose or
    belongs to another package's fixtures. These two forms cover the reviewed
    repository sources; generated OUT_DIR includes remain covered by the owning
    build script and assets. This is deliberately not a Rust expression evaluator.
    """
    literal = r'"(?:\\.|[^"\\])*"'
    direct = re.compile(r'include_(?:str|bytes)!\s*\(\s*(' + literal + r')\s*[,)]')
    manifest = re.compile(
        r'include_(?:str|bytes)!\s*\(\s*concat!\s*\(\s*'
        r'env!\s*\(\s*"CARGO_MANIFEST_DIR"\s*\)\s*,\s*'
        r'((?:' + literal + r'\s*,?\s*)+)\)\s*\)')
    result = set()
    pending = sorted({source for pattern in source_patterns for source in directory.glob(pattern)})
    seen = set()
    path_attribute = re.compile(r'#\[\s*path\s*=\s*(' + literal + r')\s*\]')
    for source in pending:
        if source in seen:
            continue
        seen.add(source)
        if source.suffix != ".rs" or not source.is_file():
            continue
        text = source.read_text()
        attributes = list(path_attribute.finditer(text))
        if len(attributes) != len(re.findall(r'#\[\s*path\s*=', text)):
            result.add("**")
        for attribute in attributes:
            target = (source.parent / json.loads(attribute[1])).resolve()
            if not target.is_relative_to(root.resolve()):
                raise ValueError(f"Rust module input escapes repository: {source}")
            result.add(target.relative_to(root.resolve()).as_posix())
            if target.is_file():
                pending.append(target)
            else:
                # Unresolved module path semantics require a broad closure.
                result.add("**")
        direct_matches, manifest_matches = list(direct.finditer(text)), list(manifest.finditer(text))
        recognized = [match.span() for match in direct_matches + manifest_matches]
        # Macro text emitted inside build.rs string literals is generated Rust,
        # owned by that reviewed build script and its asset directory. The one
        # source-level generated manifest include has the same owning contract.
        strings = [match.span() for match in re.finditer(literal, text, re.DOTALL)]
        generated = r'include_str!\s*\(\s*env!\s*\(\s*"GEOSOLVE_BUNDLED_SAMPLE_FRONTEND_MANIFEST"\s*\)\s*\)'
        if source.relative_to(root).as_posix() == "crates/geosolve-sketch-code/examples/generate_frontend_samples.rs":
            recognized.extend(match.span() for match in re.finditer(generated, text))
        plain_includes = {
            "crates/geosolve-sketch-code/src/bundled_samples.rs":
                r'include!\s*\(\s*concat!\s*\(\s*env!\s*\(\s*"OUT_DIR"\s*\)\s*,\s*'
                r'"/bundled-compiler-envelopes/bundled-sample-registry\.rs"\s*\)\s*\)',
            "crates/geosolve-demo-web/src/workbench/mod.rs":
                r'include!\s*\(\s*"golden_scene_backend_parity\.rs"\s*\)',
        }
        # The second existing plain include is an ordinary adjacent src file,
        # already scanned by the crate source glob. New plain includes may
        # contain nested includes and therefore use the all-source fallback.
        reviewed_plain = plain_includes.get(source.relative_to(root).as_posix())
        if reviewed_plain:
            recognized.extend(match.span() for match in re.finditer(reviewed_plain, text))
        for macro in re.finditer(r'include(?:_(?:str|bytes))?!', text):
            if any(start <= macro.start() < end for start, end in recognized):
                continue
            if (source.relative_to(root).as_posix() == "crates/geosolve-sketch-code/build.rs"
                    and any(start <= macro.start() < end for start, end in strings)):
                continue
            # Never guess at a new raw-string/concat/env/expression form. All
            # source, including prose, remains an input until it is reviewed.
            result.add("**")
        targets = [(source.parent, json.loads(match[1])) for match in direct_matches]
        targets += [(directory, "".join(json.loads(value) for value in re.findall(literal, match[1])).lstrip("/"))
                    for match in manifest_matches]
        for base, relative in targets:
            target = (base / relative).resolve()
            if not target.is_relative_to(root.resolve()):
                raise ValueError(f"Rust embedded input escapes repository: {source}: {relative}")
            result.add(target.relative_to(root.resolve()).as_posix())
    return result


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
        source_patterns = ["src/**/*.rs", "build.rs"]
        if owner and include_tests:
            result.update((f"{rel}/tests/**", f"{rel}/examples/**", f"{rel}/benches/**"))
            source_patterns.extend(("tests/**/*.rs", "examples/**/*.rs", "benches/**/*.rs"))
        result.update(rust_embedded_inputs(root, directory, source_patterns))
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


def preflight_stages(include_clippy=True):
    import release_gate_m98
    import release_equivalence
    try:
        equivalence = read_json(ROOT / release_equivalence.CONTRACT_PATH)
    except (OSError, ValueError):
        equivalence = None
    frontend_inputs = (FRONTEND + "/**", "packages/**", "crates/geosolve-sketch-code/**",
                       "crates/geosolve-constraint-editor/src/**", "crates/geosolve-demo-web/src/**",
                       "LICENSE", "THIRD_PARTY_LICENSES.md", "docs/API_COMPATIBILITY.md")
    stages = [
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
                                     npm(FRONTEND, "run", "check:licenses"),
                                     npm(FRONTEND, "run", "check:language-sdk"),
                                     npm(FRONTEND, "test", "--", "--no-cache",
                                         *(argument for pattern in release_gate_m98.FRONTEND_RUNTIME_TESTS for argument in ("--exclude", pattern)))),
              inputs=frontend_inputs, input_equivalence="frontend_static", input_equivalence_contract=equivalence,
              # These tests use UI/catalog metadata and private mocked geometry.
              # Collaboration's real WASM tests run after prepare.m98 in their
              # mandatory captured-tree group, never against stale live outputs.
              # Catalog-sensitive Playwright discovery stays fresh below.
              excluded_inputs=(release_equivalence.SAMPLE_ROOT + "/**",),
              dependencies=("preflight.managed",), resource="exclusive", timeout=600,
              outputs=(FRONTEND + "/node_modules",)),
        Stage("preflight.catalog", (npm(FRONTEND, "run", "check:manifest"),
                                    npm(FRONTEND, "run", "test:build")),
              inputs=frontend_inputs, dependencies=("preflight.frontend", "preflight.metadata-format"),
              resource="exclusive", timeout=600),
        Stage("preflight.clippy", (cmd("cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"),),
              inputs=rust_inputs(ROOT), dependencies=("preflight.catalog", "preflight.frontend", "preflight.metadata-format"),
              resource="exclusive", timeout=2400),
    ]
    return stages if include_clippy else [stage for stage in stages if stage.id != "preflight.clippy"]


def check_inventory(root):
    """Catch M92's stale count before any Cargo/compiler/native work."""
    directory = root / "crates/geosolve-sketch-code/assets/bundled-samples"
    contract = read_json(directory.parent / "bundled-sample-catalog.json")
    if contract.get("schema") != 1 or not contract.get("samples"):
        raise ValueError("catalog contract is missing, empty or unknown")
    manifests = []
    for path in sorted(directory.glob("*/manifest.json")):
        manifest = read_json(path)
        compiled = read_json(path.parent / "sketch.compiled.json")
        manifest["title"] = compiled["artifact"]["document"]["title"]
        manifests.append(manifest)
    count = len(manifests)
    if not count or sorted(m["ordinal"] for m in manifests) != list(range(1, count + 1)):
        raise ValueError("sample ordinals must be complete and contiguous")
    keys = [m["key"] for m in manifests]
    if len(keys) != len(set(keys)):
        raise ValueError("duplicate sample key")
    ordered = sorted(manifests, key=lambda m: m["ordinal"])
    if [{key: entry[key] for key in ("key", "title", "category")} for entry in ordered] != contract["samples"]:
        raise ValueError(f"catalog contract count/order/metadata is stale (catalog has {count})")
    retired = contract.get("retired_keys", [])
    if len(retired) != len(set(retired)) or set(retired) & set(keys):
        raise ValueError("retired catalog keys must be unique and absent")
    frontend = read_json(root / FRONTEND / "src/data/samples.json")
    rows = frontend if isinstance(frontend, list) else frontend.get("samples", [])
    if len(rows) != count or [row["key"] for row in rows] != [m["key"] for m in sorted(manifests, key=lambda m: m["ordinal"])]:
        raise ValueError("generated frontend inventory differs from catalog")
    print(f"sample preflight: {count} exact entries; reviewed catalog and frontend order match")


def rust_inputs(root):
    return tuple(sorted({pattern for manifest in (root / "crates").glob("*/Cargo.toml")
                         for pattern in crate_inputs(root, manifest.parent.name)}))


def preparation_modes(patterns):
    if not patterns:
        return {"workspace", "headless", "wasm", "browser", "lifecycle", "m98"}
    modes = set()
    import release_gate_m98
    for pattern in patterns:
        selected_m98 = {name for name in release_gate_m98.GROUPS if fnmatch.fnmatchcase(name, pattern)}
        if selected_m98:
            modes.update(("wasm", "m98"))
            if selected_m98 & release_gate_m98.BROWSER_GROUPS:
                modes.add("browser")
            # Wildcards can still select existing native/browser obligations.
            if pattern in selected_m98 or pattern.split(".")[0] in {"engine", "folder", "example", "collaboration"}:
                continue
        if pattern.startswith("workspace."):
            modes.add("workspace")
        elif pattern.startswith("headless."):
            modes.add("headless")
        elif pattern.startswith("wasm.") and fnmatch.fnmatchcase("wasm.lifecycle", pattern):
            modes.add("lifecycle")
        elif pattern in {"browser", "artifact.transport"}:
            modes.update(("wasm", "m98", "browser"))
        elif pattern in {"golden", "licenses", "performance"} or pattern.startswith(("rust.", "wasm.", "package.")):
            # These stages own their Cargo builds and managed preflight inputs.
            # No direct native executable or prepared browser is consumed.
            continue
        else:
            return {"workspace", "headless", "wasm", "browser", "lifecycle", "m98"}
    return modes


def build_overlap_inputs(root, snapshot, policy):
    """One reviewed body/writer closure protects every permitted build overlap."""
    patterns = (*rust_inputs(root), "packages/**", FRONTEND + "/**",
                "scripts/verify-geosolve-sketch-code-package.sh", "scripts/golden*")
    if "**" in patterns:
        return None
    return stage_inputs(Stage("build-overlap", (cmd("identity"),), inputs=patterns), snapshot, policy)


def build_overlap_reviewed(root, scope, snapshot, policy):
    import release_equivalence
    inputs = build_overlap_inputs(root, snapshot, policy)
    if inputs is None:
        return False
    try:
        contract = read_json(root / release_equivalence.CONTRACT_PATH)
    except (OSError, ValueError):
        return False
    return release_equivalence.reviewed(scope, inputs, policy, contract)


def preparation_stages(runner, modes=None):
    modes = {"workspace", "headless", "wasm", "browser", "lifecycle", "m98"} if modes is None else modes
    if "browser" in modes:
        modes = set(modes) | {"m98", "wasm"}
    stages, prepared = [], {}
    for mode in ("workspace", "headless", "wasm", "lifecycle", "m98", "browser"):
        if mode not in modes:
            continue
        if mode in {"workspace", "headless"}:
            inputs = rust_inputs(runner.root) if mode == "workspace" else crate_inputs(runner.root, "geosolve-headless")
            inputs = (*inputs, "scripts/release_gate_native.py")
        elif mode == "lifecycle":
            inputs = (*rust_inputs(runner.root), "scripts/release_gate_wasm.py", "scripts/release_gate_native.py")
        elif mode == "wasm":
            inputs = (*crate_inputs(runner.root, "geosolve-demo-web", include_tests=False),
                      FRONTEND + "/scripts/build-wasm.mjs")
        elif mode == "m98":
            import release_gate_m98
            inputs = (*crate_inputs(runner.root, "geosolve-sketch-engine-wasm", include_tests=False),
                      *crate_inputs(runner.root, "geosolve-collaboration-wasm", include_tests=False),
                      *crate_inputs(runner.root, "geosolve-collaboration"),
                      *release_gate_m98.SOURCE_INPUTS)
        else:
            inputs = (FRONTEND + "/**", "packages/**", "LICENSE", "THIRD_PARTY_LICENSES.md", "docs/API_COMPATIBILITY.md")
        identity = digest({"inputs": stage_inputs(Stage("identity", (cmd("identity"),), inputs=inputs),
                                                 runner.snapshot, runner.policy),
                           "tools": runner.tools, "environment": runner.environment,
                           "wasm": prepared.get("wasm") if mode in {"browser", "m98"} else None,
                           "runtime": prepared.get("m98") if mode == "browser" else None})
        out = f"target/release-gate/prepared/{identity}/{mode}"
        prepared[mode] = out
        if mode in {"workspace", "headless"}:
            command = cmd(sys.executable, "scripts/release_gate.py", "--prepare-native", mode, "--output", out)
        elif mode == "lifecycle":
            command = cmd(sys.executable, "scripts/release_gate.py", "--prepare-lifecycle", "--output", out)
        elif mode == "wasm":
            command = cmd(sys.executable, "scripts/release_gate.py", "--prepare-wasm", "--output", out)
        elif mode == "m98":
            command = cmd(sys.executable, "scripts/release_gate.py", "--prepare-m98", "--wasm-package", prepared["wasm"], "--output", out)
        else:
            command = cmd(sys.executable, "scripts/release_gate.py", "--prepare-browser", "--wasm-package", prepared["wasm"], "--output", out)
        dependencies = ("preflight.clippy",) + (("prepare.wasm",) if mode in {"browser", "m98"} else ())
        if mode == "browser":
            dependencies += ("prepare.m98",)
        # Only this prepared-package path is frontend-only. Its sole writer is
        # ordered after all installs and prepare.wasm; standalone WASM fallback
        # retains its compiler work. An unreviewed concurrent body/writer restores
        # the shared lock, even when that body's own native overlap guard failed.
        frontend_overlap = mode == "browser" and build_overlap_reviewed(
            runner.root, "frontend_build_overlap", runner.snapshot, runner.policy)
        stages.append(Stage(f"prepare.{mode}", (command,), inputs=inputs,
                            dependencies=dependencies, build_lock=not frontend_overlap,
                            resource="memory" if mode == "browser" else "normal", timeout=2400,
                            outputs=(out,), kind="build",
                            artifacts=(*release_gate_m98.DEPENDENCIES, *release_gate_m98.GENERATED[:2], prepared["wasm"])
                                if mode == "m98" else (*release_gate_m98.GENERATED, prepared["m98"])
                                if mode == "browser" else ()))
    return stages, prepared


def native_stages(root, prepared):
    if not ({"workspace", "headless"} & prepared.keys()):
        return []
    import release_gate_native as native
    # Loader closure protects binaries and libraries; this separate reviewed
    # program boundary confirms that test bodies do not mutate Cargo/npm outputs.
    policy = read_json(root / POLICY_PATH)
    overlap_reviewed = build_overlap_reviewed(root, "native_build_overlap", source_snapshot(root), policy)
    # These proptest suites replay tracked seeds and append new failures there.
    # Preserve that coverage and keep their source-tree writes away from Cargo.
    body_build_lock = {"geosolve-sketch::m22_properties::normal", "geosolve-linkage::m23_properties::normal"}
    result = []
    package_input_maps = {}
    for mode in ("workspace", "headless"):
        if mode not in prepared:
            continue
        source = root / prepared[mode] / "prepared.json"
        value = read_json(source)
        descriptions = native.workspace_stages(value, test_threads=2) if mode == "workspace" else native.ignored_headless_stages(value)
        for description in descriptions:
            package = description["id"].split("::")[0]
            if package not in package_input_maps:
                package_input_maps[package] = crate_inputs(root, package)
            managed = ("packages/geosolve-sketch-code/src/**", "packages/geosolve-sketch-code/scripts/compile*",
                       "packages/geosolve-sketch-code/scripts/mutate*", "packages/geosolve-intent/src/**") if package in {
                           "geosolve-headless", "geosolve-demo-web", "geosolve-sketch-code"} else ()
            managed_artifacts = ("packages/geosolve-sketch-code/dist", "packages/geosolve-sketch-code/node_modules",
                                 "packages/geosolve-intent/dist") if managed else ()
            auxiliary = tuple(item["path"] for item in description.get("auxiliary_executables", []))
            stage_id = mode + "." + description["id"]
            result.append(Stage(stage_id,
                (cmd(sys.executable, "scripts/release_gate.py", "--native-run", str(source), "--native-id", description["id"], "--output", "{scratch}/native"),),
                inputs=(*package_input_maps[package], *managed, "scripts/release_gate_native.py"),
                dependencies=(f"prepare.{mode}",), resource="memory" if description["resource"] == "memory-heavy" else "normal",
                build_lock=description["id"] in body_build_lock or not (overlap_reviewed and description.get("build_overlap_safe", False)),
                timeout=2400, kind="native", cases=tuple(description["selected"]), artifacts=(description["command"][0], *managed_artifacts, *auxiliary),
                execution_key={key: description[key] for key in ("command", "env", "features", "profile",
                               "build_overlap_safe", "runtime_closures")}))
    return result


def qualification_stages(root, prepared, jobs):
    import golden_oracle
    import release_equivalence
    try:
        equivalence = read_json(root / release_equivalence.CONTRACT_PATH)
    except (OSError, ValueError):
        equivalence = None
    exact_tests = read_json(root / "scripts/release_test_inventory.json")["stages"]
    all_rust = (*rust_inputs(root), "README.md", "LICENSE", "docs/API_COMPATIBILITY.md", "THIRD_PARTY_LICENSES.md")
    built = tuple("prepare." + mode for mode in prepared)
    result = native_stages(root, prepared)
    result.extend([
        Stage("rust.doctests", (cmd("cargo", "test", "--locked", "--workspace", "--all-features", "--doc"),),
              inputs=all_rust, dependencies=built, env=(("RUST_MIN_STACK", "16777216"),), resource="exclusive"),
        Stage("rust.documentation", (cmd("cargo", "doc", "--locked", "--workspace", "--all-features", "--no-deps"),),
              inputs=all_rust, dependencies=built, env=(("RUSTDOCFLAGS", "-D warnings"),), resource="exclusive"),
        Stage("rust.bench-build", (cmd("cargo", "bench", "--locked", "--workspace", "--all-features", "--no-run"),),
              inputs=all_rust, dependencies=built, resource="exclusive", kind="build"),
        Stage("golden", (cmd("bash", "scripts/golden-authoring-scene-oracle.sh", "--require-clean", "--jobs", str(jobs),
                             "--prepared-packages", "--output-dir", "{scratch}/golden"),),
              inputs=(*all_rust, "packages/*/src/**", "packages/*/scripts/compile*", "packages/*/tsconfig*",
                      "scripts/golden*", "scripts/release_gate_native.py"),
              # Audited adapters export their own fixtures and use CodeProject::managed;
              # they never read the bundled catalog. Keep executable hashes in the
              # original observation as provenance; all program/build inputs remain.
              input_equivalence="golden", input_equivalence_contract=equivalence,
              excluded_inputs=("crates/geosolve-sketch-code/assets/bundled-samples/**",
                               "crates/geosolve-sketch-code/assets/bundled-sample-catalog.json",
                               FRONTEND + "/src/data/samples.json"),
              artifacts=("packages/geosolve-sketch-code/dist", "packages/geosolve-sketch-code/node_modules",
                         "packages/geosolve-intent/dist", "packages/geosolve-intent/node_modules"),
              dependencies=built, resource="exclusive", timeout=3600,
              cases=tuple(case for case, _ in golden_oracle.inventory())),
        Stage("wasm.check", (cmd("cargo", "check", "--locked", "-p", "geosolve-demo-web", "--all-features", "--target", "wasm32-unknown-unknown"),),
              inputs=all_rust, dependencies=built, resource="exclusive"),
        Stage("wasm.engine", (cmd("cargo", "clippy", "--locked", "-p", "geosolve-sketch-engine-wasm", "--all-targets",
                                  "--target", "wasm32-unknown-unknown", "--", "-D", "warnings"),),
              inputs=crate_inputs(root, "geosolve-sketch-engine-wasm"), dependencies=built, resource="exclusive"),
        Stage("wasm.collaboration", (cmd("cargo", "clippy", "--locked", "-p", "geosolve-collaboration-wasm", "--all-targets",
                                        "--target", "wasm32-unknown-unknown", "--", "-D", "warnings"),),
              inputs=crate_inputs(root, "geosolve-collaboration-wasm"), dependencies=built, resource="exclusive"),
        Stage("package.contents", (cmd(sys.executable, "scripts/release_gate.py", "--check-packages"),),
              inputs=(*all_rust, "scripts/verify-geosolve-sketch-code-package.sh"), dependencies=built, resource="exclusive"),
        Stage("package.archive", (cmd("bash", "scripts/verify-geosolve-sketch-code-package.sh"),),
              inputs=(*all_rust, "scripts/verify-geosolve-sketch-code-package.sh"), dependencies=built, resource="exclusive"),
        Stage("licenses", (cmd("cargo", "deny", "check", "licenses") if shutil.which("cargo-deny") else
                           cmd("nix-shell", "-p", "cargo-deny", "--run", "cargo deny check licenses"),),
              inputs=("**/Cargo.toml", "deny.toml", "LICENSE", "THIRD_PARTY_LICENSES.md"), dependencies=built, resource="exclusive",
              reusable=bool(shutil.which("cargo-deny"))),
    ])
    for target in ("m70_transition_parity", "m71_transition_parity", "m74_reference_geometry", "m75_hover_pointer_parity",
                   "m76_annotation_parity", "m77_curve_control_parity", "m79_inference_lifecycle"):
        result.append(Stage("wasm." + target, (cmd("cargo", "test", "--locked", "-p", "geosolve-constraint-editor", "--test", target,
                                                  "--target", "wasm32-unknown-unknown"),),
                            inputs=crate_inputs(root, "geosolve-constraint-editor"), dependencies=built, resource="exclusive",
                            cases=tuple(exact_tests["wasm." + target]), test_inventories=(tuple(exact_tests["wasm." + target]),),
                            env=(("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER", "wasm-bindgen-test-runner"),)))
    if "lifecycle" in prepared:
        lifecycle = read_json(root / prepared["lifecycle"] / "prepared.json")
        if lifecycle["schema"] != "geosolve-wasm-lifecycle-preparation-v1" or sorted(lifecycle["cases"]) != sorted(exact_tests["wasm.lifecycle"]):
            raise ValueError("prepared WASM lifecycle inventory differs")
        if (Path(lifecycle["executable"]) != (root / prepared["lifecycle"] / "lifecycle.wasm").resolve()
                or lifecycle["command"] != [shutil.which("wasm-bindgen-test-runner"), lifecycle["executable"], "actual_wasm_"]
                or file_hash(lifecycle["executable"]) != lifecycle["sha256"]
                or file_hash(lifecycle["command"][0]) != lifecycle["runner_sha256"]):
            raise ValueError("prepared WASM lifecycle artifact or runner changed")
        result.append(Stage("wasm.lifecycle", (tuple(lifecycle["command"]),),
                            inputs=(*all_rust, "scripts/release_gate_wasm.py"), dependencies=built,
                            resource="memory", cases=tuple(exact_tests["wasm.lifecycle"]),
                            test_inventories=(tuple(exact_tests["wasm.lifecycle"]),),
                            artifacts=(lifecycle["executable"], lifecycle["command"][0]),
                            execution_key={key: lifecycle[key] for key in ("command", "env", "features", "profile")},
                            env=tuple(sorted((lifecycle["env"] | {"CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER": lifecycle["command"][0]}).items()))))
    performance = [cmd("cargo", "run", "--locked", "--release", "-p", package, "--example", example)
                   for package, example in (("geosolve-sketch", "m14_performance"), ("geosolve-sketch", "m32_performance"),
                                            ("geosolve-constraint-editor", "m83_performance"))]
    performance += [cmd("cargo", "test", "--locked", "--release", "-p", "geosolve-constraint-editor", "--test", "m83_interaction_performance", "--", "--ignored", "--nocapture", "--test-threads=1"),
                    cmd("cargo", "test", "--locked", "--release", "-p", "geosolve-linkage", "--test", "m23_performance",
                        "exact_auto_sparse_crossover_solves_and_validates_256_moving_body_chain", "--", "--exact", "--ignored", "--nocapture")]
    performance_cases = tuple(exact_tests["performance.interaction"] + exact_tests["performance.linkage"])
    performance_inputs = tuple(sorted({item for package in (
        "geosolve-sketch", "geosolve-constraint-editor", "geosolve-linkage")
        for item in crate_inputs(root, package)}))
    result.append(Stage("performance", tuple(performance), inputs=performance_inputs, dependencies=built, resource="exclusive",
                        cases=performance_cases, test_inventories=((), (), (), tuple(exact_tests["performance.interaction"]), tuple(exact_tests["performance.linkage"]))))
    browser_path = root / prepared.get("browser", "target/unprepared-browser")
    manifest = str(browser_path / "harness.json")
    production = str(browser_path / "production.json")
    result.append(Stage("browser", (cmd(sys.executable, "scripts/release_gate.py", "--run-browser", manifest,
                                       "--jobs", str(min(jobs, 2)), "--output", "{scratch}/browser"),),
                        inputs=(FRONTEND + "/**", *crate_inputs(root, "geosolve-demo-web", include_tests=False),
                                "packages/**", "docs/API_COMPATIBILITY.md"), dependencies=built,
                        resource="memory", env=(("GEOSOLVE_E2E_ARTIFACT_MANIFEST", manifest),),
                        artifacts=(str(browser_path / "geosolve-harness"), FRONTEND + "/node_modules",
                                   "packages/geosolve-sketch-code/dist",
                                   "packages/geosolve-sketch-code/node_modules", "packages/geosolve-intent/dist",
                                   "packages/geosolve-intent/node_modules"), timeout=2400))
    result.append(Stage("artifact.transport", (cmd(sys.executable, "scripts/release_gate.py", "--verify-production", production,
                                                     "--output", "{scratch}/transport.json"),),
                        inputs=(FRONTEND + "/scripts/**",), dependencies=built, resource="memory", reusable=False, timeout=180,
                        artifacts=(str(browser_path / "geosolve-production"),)))
    if "m98" in prepared:
        result.extend(m98_stages(root, prepared))
    # Start browser and prepared lifecycle work alongside native suites;
    # Resource admission preserves the configured total and memory-stage caps.
    return sorted(result, key=lambda stage: 0 if stage.id in {"browser", "wasm.lifecycle"} else 1)


def m98_stages(root, prepared):
    import release_gate_m98
    captured = root / prepared["m98"]
    metadata = read_json(captured / "prepared.json")
    if metadata.get("schema") != "geosolve-m98-runtime-v1" or set(metadata.get("tests", {})) != set(release_gate_m98.GROUPS):
        raise ValueError("M98 preparation lacks complete owning test inventory")
    stages = []
    for name in release_gate_m98.GROUPS:
        needs_browser = name in release_gate_m98.BROWSER_GROUPS
        if needs_browser and "browser" not in prepared:
            continue
        cases = tuple(release_gate_m98.inventory(captured / "repository", name))
        if list(cases) != metadata["tests"][name]:
            raise ValueError("M98 prepared owning test inventory changed")
        command = cmd(sys.executable, "scripts/release_gate.py", "--run-m98", str(captured),
                      "--m98-group", name, "--output", "{scratch}/m98")
        artifacts = (str(captured), *release_gate_m98.DEPENDENCIES)
        dependencies = ("prepare.m98",)
        if needs_browser:
            flavor = "geosolve-production" if name == "package.m98" else "geosolve-harness"
            browser = str(root / prepared["browser"] / flavor)
            command += ("--browser-package", browser)
            artifacts += (browser,)
            dependencies += ("prepare.browser",)
        stages.append(Stage(name, (command,), inputs=release_gate_m98.SOURCE_INPUTS,
                            dependencies=dependencies, artifacts=artifacts, cases=cases,
                            # This suite measures mandatory paint/navigation latency.
                            # Competing browser or build stages invalidate that measurement.
                            build_lock=True,
                            resource="exclusive" if name == "collaboration.browser" else "memory",
                            timeout=2100))
    return stages


def pipeline_stages(root, prepared, jobs, patterns):
    """Declare fixed obligations and exact groups deferred until preparation passes."""
    def selected(stages):
        return [stage for stage in stages if not patterns or any(
            fnmatch.fnmatchcase(stage.id, pattern) for pattern in patterns)]

    # These stages own Cargo work but never mutate installed JS packages. The
    # complete performance stage retains exclusive CPU/memory admission.
    common = []
    for stage in qualification_stages(root, {}, jobs):
        if stage.id in {"browser", "artifact.transport"}:
            continue
        common.append(dataclasses.replace(stage, dependencies=("preflight.clippy",),
                      build_lock=stage.id != "performance",
                      resource="exclusive" if stage.id == "performance" else "memory" if stage.id == "golden" else "normal"))

    deferred = []
    for mode in ("workspace", "headless", "browser", "lifecycle", "m98"):
        if mode not in prepared:
            continue
        dependencies = ("prepare." + mode,) + (("prepare.wasm",) if mode == "browser" else ())
        def factory(mode=mode):
            if mode in {"workspace", "headless"}:
                stages = native_stages(root, {mode: prepared[mode]})
            elif mode == "m98":
                stages = m98_stages(root, prepared)
            else:
                own = {mode: prepared[mode]}
                if mode == "browser":
                    own["wasm"] = prepared["wasm"]
                wanted = {"browser", "artifact.transport"} if mode == "browser" else {"wasm.lifecycle"}
                stages = [stage for stage in qualification_stages(root, own, jobs) if stage.id in wanted]
                if {stage.id for stage in stages} != wanted:
                    raise ValueError("prepared group did not expand its complete stage inventory")
            if not stages:
                raise ValueError("prepared group is unexpectedly empty")
            return selected(stages)
        deferred.append((mode, dependencies, factory))
    return selected(common), deferred


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
    if os.environ.get("GEOSOLVE_RELEASE_BROWSER_CONTEXT"):
        import release_browser
        store = Store(ROOT / "target/release-gate")
        context = store.unseal(Path(os.environ["GEOSOLVE_RELEASE_BROWSER_CONTEXT"]))
        if not context:
            raise ValueError("browser parent context is unauthenticated")
        release_browser.run(ROOT, store, manifest, output, jobs, context["fresh"], context)
        return
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
                "geosolve-sketch-code", "geosolve-sketch-render", "geosolve-headless",
                "geosolve-sketch-engine", "geosolve-sketch-engine-wasm"]
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
    compiler_inputs = rust_inputs(root)
    if any(matches(name, compiler_inputs) for name in changes):
        raise ValueError("changed Markdown is a declared or unresolved Rust input; use affected qualification")
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
    parser.add_argument("--jobs", type=int)
    parser.add_argument("--preflight", action="store_true", help="run only cheap preparation checks; no release claim")
    parser.add_argument("--prepare", action="store_true", help="prepare and discover all executable inventories without running test bodies")
    parser.add_argument("--stage", action="append", default=[], help="run matching stage IDs and prerequisites; reports targeted evidence")
    parser.add_argument("--resume", help="resume a trusted run ID, reusing only independently verified successful work")
    parser.add_argument("--docs-only", action="store_true", help="verify a prose-only diff; does not nominate new product bytes")
    parser.add_argument("--since", help="Git baseline for documentation-only verification")
    parser.add_argument("--test-opt-level", choices=("0", "1"), default="1",
                        help="non-release test optimization (native and WASM); debug assertions/overflow checks stay enabled (default: 1)")
    parser.add_argument("--check-inventory", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--check-packages", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--prepare-native", choices=("workspace", "headless"), help=argparse.SUPPRESS)
    parser.add_argument("--prepare-wasm", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--prepare-m98", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--run-m98", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--m98-group", help=argparse.SUPPRESS)
    parser.add_argument("--browser-package", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--prepare-lifecycle", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--wasm-package", type=Path, help=argparse.SUPPRESS)
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
    if args.prepare_lifecycle:
        import release_gate_wasm
        prepare_output(args.output, lambda out: release_gate_wasm.prepare(ROOT, out))
        return 0
    if args.prepare_wasm:
        def build_wasm(out):
            subprocess.run(["npm", "run", "wasm:release"], cwd=ROOT / FRONTEND, check=True)
            shutil.copytree(ROOT / FRONTEND / "src/generated", out)
        prepare_output(args.output, build_wasm)
        return 0
    if args.prepare_m98:
        import release_gate_m98
        prepare_output(args.output, lambda out: release_gate_m98.prepare(ROOT, args.wasm_package.resolve(), out))
        return 0
    if args.run_m98:
        import release_gate_m98
        release_gate_m98.run(ROOT, args.run_m98, args.m98_group, args.output, args.browser_package)
        return 0
    if args.prepare_browser:
        wasm_args = ["--wasm-package", str(args.wasm_package.resolve())] if args.wasm_package else []
        prepare_output(args.output, lambda out: subprocess.run(["node", "scripts/build-release-artifacts.mjs", "--out", str(out), *wasm_args],
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
    policy = read_json(ROOT / POLICY_PATH)
    if args.jobs is None:
        args.jobs = policy["workers"]
    if args.jobs < 1 or args.jobs > 8:
        parser.error("--jobs must be between 1 and 8")
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
    # Keep source-line backtraces without retaining tens of GiB of variable
    # debug data in every test executable. Caller overrides remain explicit inputs.
    os.environ.setdefault("CARGO_PROFILE_TEST_DEBUG", policy["test_debug"])
    # Catalog data edits otherwise repeat optimization of unchanged release code.
    # Preserve the original non-incremental release/bench codegen-unit count;
    # Cargo would default incremental profiles to 256 units if it were omitted.
    for profile in ("RELEASE", "BENCH"):
        os.environ.setdefault(f"CARGO_PROFILE_{profile}_INCREMENTAL", str(policy["release_incremental"]).lower())
        os.environ.setdefault(f"CARGO_PROFILE_{profile}_CODEGEN_UNITS", str(policy["release_codegen_units"]))
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
    runner = Runner(ROOT, store, source_snapshot(ROOT), policy, tool_identity(ROOT), args.jobs, args.fresh)
    stages = preflight_stages(include_clippy=not args.preflight)
    preparations, prepared = preparation_stages(runner, preparation_modes(args.stage))
    if not args.preflight:
        stages += preparations
    runner.prepare(stages)
    if args.plan:
        for stage in stages:
            action, reason, _ = runner.decision(stage)
            print(f"{action:5} {stage.id}: {reason}")
        if not args.preflight:
            common, groups = pipeline_stages(ROOT, prepared, args.jobs, args.stage)
            runner.register(common)
            for stage in common:
                action, reason, _ = runner.decision(stage)
                print(f"{action:5} {stage.id}: {reason}")
            for identity, dependencies, factory in groups:
                # Reuse requires authenticated current preparation outputs. A
                # stale metadata file is not authority for a read-only plan.
                if all(runner.decision(next(stage for stage in preparations if stage.id == name))[0] == "reuse"
                       for name in dependencies):
                    expanded = factory()
                    runner.register(expanded)
                    for stage in expanded:
                        action, reason, _ = runner.decision(stage)
                        print(f"{action:5} {stage.id}: {reason}")
                else:
                    print(f"run   {identity}: exact case inventory pending authenticated preparation")
        return 0
    lock_file = (store.path / "runner.lock").open("a")
    try:
        fcntl.flock(lock_file, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as error:
        lock_file.close()
        raise ValueError("another gate owns this checkout's mutable build/install state") from error
    previous = {sig: signal.signal(sig, lambda *_: runner.stop.set()) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        if args.resume and original["source"] == runner.snapshot:
            for result in original["results"]:
                if result.get("decision") == "run" and result.get("complete"):
                    receipt = store.unseal(result.get("receipt_path", ""))
                    if receipt and receipt.get("source_sha256") == digest(original["source"]):
                        store.save(receipt)
        if args.preflight or args.prepare:
            result = runner.run(stages)
        else:
            preflight = [stage for stage in stages if stage.id.startswith("preflight.")]
            result = runner.run(preflight)
            if result:
                common, deferred = pipeline_stages(ROOT, prepared, args.jobs, args.stage)
                runner.register(common)
                result = runner.run([*preparations, *common], deferred=deferred)
                if args.stage and not any(not stage.id.startswith(("prepare.", "preflight."))
                                          for stage in runner.inventory):
                    result = False
                    runner.deferred_groups["selection"] = {"status": "failed", "error": "--stage matched no qualification stage"}
        if result and args.prepare:
            expanded = qualification_stages(ROOT, prepared, args.jobs)
            if args.stage:
                selected = {stage.id for stage in expanded if any(fnmatch.fnmatchcase(stage.id, pattern) for pattern in args.stage)}
                if not selected:
                    raise ValueError("--stage matched no qualification stage")
                expanded = [stage for stage in expanded if stage.id in selected]
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
