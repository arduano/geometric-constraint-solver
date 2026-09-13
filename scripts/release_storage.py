#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Bounded checkout-local release storage. Dry-run by default; never prune project data."""
from __future__ import annotations

import argparse
import contextlib
import fcntl
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import time

import release_gate as gate

GIB = 1024 ** 3
POLICY = {"store_gib": 128, "target_gib": 160, "free_gib": 32,
          "recent_runs": 1, "qualified_runs": 1, "orphan_hours": 24}
RUN = re.compile(r"[A-Za-z0-9-]+\Z")
PREPARATION = re.compile(r"[a-f0-9]{64}\Z")
MODE = re.compile(r"(?:workspace|headless|wasm|lifecycle|m98|browser)(?:\.superseded-[a-f0-9]{8})?\Z")
META = {"receipt.json", "browser-context.json", "coverage.json", "batch.json"}
BUILD_CACHES = ("debug/incremental", "release/incremental", "wasm32-unknown-unknown/debug/incremental",
                "wasm32-unknown-unknown/release/incremental", "package-verification",
                "debug/deps", "debug/examples", "debug/.fingerprint",
                "debug", "release", "wasm32-unknown-unknown")


def policy(root):
    path = root / "scripts/release_storage_policy.json"
    value = gate.read_json(path) if path.exists() else dict(POLICY)
    if set(value) != set(POLICY) or any(type(v) is not int or v < 1 for v in value.values()):
        raise ValueError("invalid release storage limits")
    return value


def regular_tree(path, boundary):
    """Do not follow a replaced parent or a top-level link into another project."""
    path, boundary = Path(path).absolute(), Path(boundary).absolute()
    if not path.is_relative_to(boundary) or path == boundary:
        raise ValueError(f"storage path escapes managed boundary: {path}")
    for part in [*reversed(path.parents), path]:
        if part.is_symlink():
            raise ValueError(f"storage path traverses a symlink: {part}")
    if path.exists() and path.stat().st_uid != os.getuid():
        raise ValueError(f"storage path is not owned by this user: {path}")
    return path


def allocated(path):
    """Allocated bytes, no symlink traversal or double-counting hard links."""
    seen = set()
    def visit(p):
        info = p.lstat()
        identity = (info.st_dev, info.st_ino)
        if identity in seen:
            return 0
        seen.add(identity)
        size = info.st_blocks * 512
        if stat.S_ISDIR(info.st_mode):
            with os.scandir(p) as entries:
                size += sum(visit(Path(entry.path)) for entry in entries)
        return size
    return visit(Path(path)) if Path(path).exists() else 0


@contextlib.contextmanager
def locked(store):
    path = regular_tree(store.path / "runner.lock", store.path)
    with path.open("a") as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError("a release runner or storage cleanup is active") from error
        yield handle


@contextlib.contextmanager
def preparation_lock(root):
    """Internal preparation children inherit the parent's exact locked file description."""
    store = gate.Store(root / "target/release-gate")
    inherited = os.environ.get("GEOSOLVE_RELEASE_LOCK_FD")
    if inherited is None:
        with locked(store):
            yield
        return
    try:
        fd = int(inherited)
        info = os.fstat(fd)
        path = regular_tree(store.path / "runner.lock", store.path)
        expected = path.stat()
        if (info.st_dev, info.st_ino) != (expected.st_dev, expected.st_ino):
            raise ValueError("preparation did not inherit the runner lock")
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except (ValueError, OSError) as error:
        raise ValueError("invalid inherited preparation lock") from error
    yield


def retention(store):
    path = store.path / "retention.json"
    if not path.exists():
        return {"pins": {}, "stage_pins": {}}
    regular_tree(path, store.path)
    value = store.unseal(path)
    if not value or value.get("schema") != "geosolve-release-retention-v1" or not isinstance(value.get("pins"), dict):
        raise ValueError("retention pins are unauthenticated; cleanup refused")
    if not isinstance(value.get("stage_pins", {}), dict):
        raise ValueError("invalid stage retention pins")
    return value


def pins(store):
    return retention(store)["pins"]


def pin(store, label, run, stage=None):
    if not RUN.fullmatch(run) or not RUN.fullmatch(label):
        raise ValueError("pin label and run ID must be simple local identifiers")
    path = regular_tree(store.path / "runs" / run / "qualification.json", store.path)
    record = store.unseal(path)
    if not record or record.get("run") != run:
        raise ValueError("pin requires an authenticated qualification record")
    retained = retention(store)
    existing = retained.setdefault("stage_pins", {}) if stage else retained["pins"]
    other = retained["pins"] if stage else retained.setdefault("stage_pins", {})
    identity = run
    if stage:
        results = [r for r in record.get("results", []) if r.get("stage") == stage]
        if len(results) != 1 or not results[0].get("receipt_path"):
            raise ValueError("stage pin requires one authenticated stage result")
        receipt = regular_tree(Path(results[0]["receipt_path"]), store.path)
        value = store.unseal(receipt)
        if not value or value.get("stage") != stage or not value.get("complete") or value.get("status") != "passed":
            raise ValueError("stage pin requires complete passing evidence")
        identity = str(receipt.relative_to(store.path))
    if label in other or (label in existing and existing[label] != identity):
        raise ValueError("pin label already identifies a different run or stage; unpin it explicitly")
    existing[label] = identity
    gate.write_json(store.path / "retention.json", store.seal({**retained, "schema": "geosolve-release-retention-v1"}))


def units(store):
    """Only runner-owned payload units are candidates; metadata is kept separately."""
    result = {}
    for name in ("prepared", "runs"):
        base = store.path / name
        if not base.exists():
            continue
        regular_tree(base, store.path)
        for parent in base.iterdir():
            regular_tree(parent, store.path)
            if not parent.is_dir():
                continue
            if name == "prepared" and PREPARATION.fullmatch(parent.name):
                for child in parent.iterdir():
                    regular_tree(child, store.path)
                    if child.is_dir() and MODE.fullmatch(child.name):
                        result[child] = "preparation"
            elif name == "runs" and RUN.fullmatch(parent.name):
                stages = parent / "stages"
                if stages.exists():
                    regular_tree(stages, store.path)
                    for child in stages.iterdir():
                        regular_tree(child, store.path)
                        if child.is_dir() and re.fullmatch(r"[A-Za-z0-9_.:-]+", child.name):
                            result[child] = "stage"
    return result


def active_paths(root):
    """Protect checkout paths opened/mapped by live processes, without logging arguments."""
    result = set()
    proc = Path('/proc')
    if not proc.is_dir():
        raise ValueError("automatic cleanup requires Linux /proc process protection")
    for process in proc.iterdir():
        if not process.name.isdigit() or int(process.name) == os.getpid():
            continue
        try:
            if process.stat().st_uid != os.getuid():
                continue
            links = [process / "cwd", process / "exe", *list((process / "fd").iterdir())]
            arguments = (process / "cmdline").read_bytes().decode(errors="replace").split("\0")
            values = [arg.split("=", 1)[-1] for arg in arguments]
            # A preview may open assets only on demand. Its configured roots remain
            # live even between requests; never print environment values or tokens.
            environment = (process / "environ").read_bytes().decode(errors="replace").split("\0")
            values.extend(value.split("=", 1)[-1] for value in environment)
            cwd = None
            try:
                cwd = Path(os.readlink(process / "cwd"))
                if cwd.is_relative_to(root) and any(Path(arg).name in {"cargo", "rustc", "rustdoc", "wasm-opt"}
                                                   for arg in arguments):
                    result.add(root / "target")
            except FileNotFoundError:
                pass
            for link in links:
                try:
                    values.append(os.readlink(link))
                except FileNotFoundError:
                    pass
            for line in (process / "maps").read_text().splitlines():
                fields = line.split(maxsplit=5)
                if len(fields) == 6:
                    values.append(fields[5])
            candidates = set()
            for value in set(values):
                # Search paths can name several roots. Resolve relative command
                # arguments against the process cwd, never the cleanup tool's cwd.
                value = value.removesuffix(" (deleted)")
                for component in {value, *value.split(os.pathsep)}:
                    if not component or len(component) > 4096 or "\n" in component or "\0" in component:
                        continue
                    if not component.startswith("/"):
                        if cwd is None:
                            continue
                        component = os.path.join(str(cwd), component)
                    component = os.path.normpath(component)
                    if component.startswith(str(root) + "/"):
                        candidates.add(component)
            for component in candidates:
                path = Path(component)
                if path.is_relative_to(root / "target"):
                    result.add(path)
                # A configured source-tree symlink may lead into target.
                try:
                    resolved = path.resolve()
                    if resolved.is_relative_to(root / "target"):
                        result.add(resolved)
                except OSError:
                    pass
        except (FileNotFoundError, ProcessLookupError):
            continue
        except PermissionError as error:
            # Privileged services can share a UID but deny ptrace (e.g. backups).
            # Never ignore an opaque process that identifies this checkout or a build.
            try:
                command = (process / "cmdline").read_bytes().decode(errors="replace")
            except (OSError, ProcessLookupError):
                raise ValueError(f"cannot identify same-user process {process.name}") from error
            if str(root) in command or any(Path(arg).name in {"cargo", "rustc", "rustdoc", "wasm-opt"}
                                           for arg in command.split("\0")):
                raise ValueError(f"cannot inspect active checkout/build process {process.name}") from error
    return result


class Closure:
    def __init__(self, root, store, available):
        self.root, self.store, self.available = root, store, available
        self.keep, self.external, self.receipts, self.evidence = set(), set(), {}, {}
        self.visited, self.queue = set(), []
        self.parsed_metadata = set()

    def reference(self, value):
        if not isinstance(value, str):
            return
        if value.startswith("target/release-gate/"):
            path = self.root / value
        elif value.startswith(str(self.store.path) + "/"):
            path = Path(value)
        else:
            if value.startswith(str(self.root / "target") + "/"):
                self.external.add(Path(value))
            return
        if ".." in path.parts:
            raise ValueError("retained evidence contains a noncanonical store path")
        regular_tree(path, self.store.path)
        matched = [parent for parent in (path, *path.parents) if parent in self.available]
        if not matched and path.is_dir():
            matched = [unit for unit in self.available if unit.is_relative_to(path)]
        for unit in matched:
            if unit in self.keep:
                continue
            self.keep.add(unit)
            receipt = unit / "receipt.json"
            if self.available[unit] == "stage" and receipt.exists():
                self.queue.append(receipt)
        if path.name in {"receipt.json", "batch.json", "browser-context.json"} or path.parent.name == "browser-leaves":
            self.queue.append(path)

    def references(self, value):
        if isinstance(value, dict):
            if isinstance(value.get("path"), str) and isinstance(value.get("sha256"), str):
                path = Path(value["path"])
                if path.is_relative_to(self.store.path):
                    regular_tree(path, self.store.path)
                    if gate.file_hash(path) != value["sha256"]:
                        raise ValueError("retained provenance reference changed")
                    self.evidence[path] = value["sha256"]
            for key, child in value.items():
                self.reference(key)
                if key not in {"source", "snapshot", "tools", "inputs", "run_dir", "context_path"}:
                    self.references(child)
            if value.get("decision") == "reuse" and value.get("receipt_key"):
                self.reference(str(self.store.path / "browser-leaves" / (value["receipt_key"] + ".json")))
        elif isinstance(value, list):
            for child in value:
                self.references(child)
        else:
            self.reference(value)

    def output_links(self, path, tree):
        """Resolve captured output symlinks relative to their owning file tree."""
        if not isinstance(tree, dict):
            return
        if "link" in tree:
            target = (path.parent / tree["link"]).resolve()
            self.reference(str(target))
            self.output_links(target, tree.get("target"))
        for name, child in tree.get("files", {}).items():
            if Path(name).name != name or name in {".", ".."}:
                raise ValueError("noncanonical captured output entry")
            self.output_links(path / name, child)

    def add_report(self, report, strict=True):
        for result in report.get("results", []):
            if result.get("receipt_path"):
                path = Path(result["receipt_path"])
                if not strict and not path.exists():
                    continue  # Evicted optional evidence causes ordinary cache misses.
                if strict and not path.exists():
                    raise ValueError("pinned run has missing stage evidence; cleanup refused")
                self.reference(result["receipt_path"])
        # Include unfinished stages of the most recent run, without treating them as passes.
        run_dir = self.store.path / "runs" / report["run"] / "stages"
        if not report.get("complete"):
            for unit in self.available:
                if unit.parent == run_dir:
                    self.reference(str(unit))

    def finish(self):
        while self.queue:
            path = self.queue.pop()
            if path in self.visited:
                continue
            self.visited.add(path)
            regular_tree(path, self.store.path)
            record = self.store.unseal(path)
            if not record:
                raise ValueError(f"retained receipt is missing or unauthenticated: {path}")
            self.receipts[path] = record
            self.references(record)
            for name, tree in record.get("outputs", {}).items():
                self.output_links(Path(name), tree)
            for name, expected in record.get("evidence", {}).items():
                p = Path(name)
                regular_tree(p, self.store.path)
                self.evidence[p] = expected
                if p.name in META and p not in self.parsed_metadata:
                    self.parsed_metadata.add(p)
                    if gate.file_hash(p) != expected:
                        raise ValueError(f"retained provenance changed: {p}")
                    content = gate.read_json(p)
                    if "hmac_sha256" in content:
                        content = self.store.unseal(p)
                        if content is None:
                            raise ValueError("unauthenticated nested provenance")
                    self.references(content)
        return self


def plan(root, store, limits=None, extra_runs=(), live=None, build_cache=False):
    root = root.resolve()
    limits = limits or policy(root)
    available = units(store)
    reports = {}
    for path in sorted((store.path / "runs").glob("*/qualification.json")):
        regular_tree(path, store.path)
        report = store.unseal(path)
        if not report or report.get("run") != path.parent.name:
            raise ValueError(f"run metadata is unauthenticated; cleanup refused: {path.parent.name}")
        reports[path.parent.name] = report
    retained = set(pins(store).values()) | set(extra_runs)
    retained.update(sorted(reports, reverse=True)[:limits["recent_runs"]])
    qualified = sorted((run for run, report in reports.items() if report.get("qualified_release")), reverse=True)
    retained.update(qualified[:limits["qualified_runs"]])
    if retained - reports.keys():
        raise ValueError("retained run is missing; cleanup refused")
    closure = Closure(root, store, available)
    for name in retention(store).get("stage_pins", {}).values():
        path = store.path / name
        if not path.is_file():
            raise ValueError("pinned stage evidence is missing; cleanup refused")
        closure.reference(str(path))
    for run in retained:
        closure.add_report(reports[run], strict=run in pins(store).values())
    live = active_paths(root) if live is None else set(live)
    for path in live:
        closure.reference(str(path))
        for unit in available:
            if unit.is_relative_to(path):
                closure.reference(str(unit))
    now = time.time()
    for unit in available:
        if available[unit] == "stage" and unit.parent.parent.name not in reports:
            if now - unit.stat().st_mtime < limits["orphan_hours"] * 3600:
                closure.reference(str(unit))
    closure.finish()
    entries = [{"path": str(unit.relative_to(root)), "kind": kind,
                "bytes": allocated(unit), "retain": unit in closure.keep}
               for unit, kind in sorted(available.items())]
    actions = [entry for entry in entries if not entry["retain"]]
    target_bytes = allocated(root / "target")
    store_bytes = allocated(store.path)
    projected = target_bytes - sum(item["bytes"] for item in actions)
    protections = closure.external | live
    if build_cache:
        for name in BUILD_CACHES:
            path = root / "target" / name
            if projected <= limits["target_gib"] * GIB:
                break
            if not path.exists():
                continue
            regular_tree(path, root / "target")
            if any(p == path or p.is_relative_to(path) or path.is_relative_to(p) for p in protections):
                continue
            size = allocated(path)
            children = [entry for entry in actions if (root / entry["path"]).is_relative_to(path)]
            actions = [entry for entry in actions if entry not in children]
            increment = size - sum(entry["bytes"] for entry in children)
            projected -= increment
            actions.append({"path": str(path.relative_to(root)), "kind": "build-cache", "bytes": size, "retain": False})
    return {"schema": "geosolve-release-storage-plan-v1", "limits": limits,
            "pins": pins(store), "stage_pins": retention(store).get("stage_pins", {}),
            "retained_runs": sorted(retained),
            "retained_stages": sum(available[p] == "stage" for p in closure.keep),
            "retained_preparations": sum(available[p] == "preparation" for p in closure.keep),
            "target_bytes": target_bytes, "store_bytes": store_bytes,
            "projected_target_bytes": projected,
            "projected_store_bytes": store_bytes - sum(e["bytes"] for e in entries if not e["retain"]),
            "reclaimable_bytes": sum(entry["bytes"] for entry in actions),
            "free_bytes": shutil.disk_usage(root / "target").free,
            "actions": actions, "protected_active_paths": len(live)}, closure


def verify(closure):
    """Check retained store bytes, not mutable external build outputs or a new release."""
    for path, expected in closure.evidence.items():
        if gate.file_hash(path) != expected:
            raise ValueError(f"retained evidence changed: {path}")
    outputs = {}
    for receipt in closure.receipts.values():
        for name, expected in receipt.get("outputs", {}).items():
            path = Path(name)
            if path.is_relative_to(closure.store.path):
                if path in outputs and outputs[path] != expected:
                    raise ValueError("retained outputs disagree")
                outputs[path] = expected
    checked = {}
    for path in sorted(outputs, key=lambda p: len(p.parts)):
        parent = next((parent for parent in checked if path.is_relative_to(parent)), None)
        observed = (gate.native_output_hash(parent, checked[parent], path) if parent else gate.hash_output(path))
        if observed != outputs[path]:
            raise ValueError(f"retained prepared output changed: {path}")
        if parent is None:
            checked[path] = observed
    return {"receipts": len(closure.receipts), "evidence_files": len(closure.evidence), "output_trees": len(checked)}


def apply(root, store, report):
    """Called under runner.lock, immediately after planning; never accepts a saved plan."""
    current_units = units(store)
    live = active_paths(root)
    for entry in report["actions"]:
        path = root / entry["path"]
        if entry["kind"] == "build-cache":
            if entry["path"] not in {"target/" + name for name in BUILD_CACHES}:
                raise ValueError("cleanup candidate is not a known Cargo cache")
        elif path not in current_units or entry["kind"] != current_units[path]:
            raise ValueError("cleanup candidate is not a known runner payload")
        if any(p == path or p.is_relative_to(path) or path.is_relative_to(p) for p in live):
            raise ValueError("cleanup candidate became active; rerun the audit")
    for entry in report["actions"]:
        path = root / entry["path"]
        boundary = root / "target" if entry["kind"] == "build-cache" else store.path
        regular_tree(path, boundary)
        shutil.rmtree(path)
    # Indexed receipts are caches, not evidence. Remove only entries whose owner was evicted.
    for directory in (store.path / "results", store.path / "browser-leaves"):
        if directory.exists():
            regular_tree(directory, store.path)
            for path in directory.rglob("*.json"):
                regular_tree(path, store.path)
                receipt = store.unseal(path)
                origin = receipt.get("receipt_path") if receipt else None
                batch = receipt.get("batch", {}).get("path") if receipt else None
                if (origin and not Path(origin).exists()) or (batch and not Path(batch).exists()):
                    path.unlink()
    # Retain tiny authenticated run reports, key and pins; remove empty preparation containers.
    for path in (store.path / "prepared").glob("*"):
        if path.is_dir() and not path.is_symlink() and not any(path.iterdir()):
            path.rmdir()
    result = {**report, "applied": True, "finished": time.time(),
              "actual_target_bytes": allocated(root / "target"), "actual_store_bytes": allocated(store.path),
              "free_bytes_after": shutil.disk_usage(root / "target").free}
    gate.write_json(store.path / "storage-last.json", store.seal({k: v for k, v in result.items() if k != "actions"}))
    return result


def maintain(root, store, *, extra_runs=(), apply_changes=True):
    report, _ = plan(root, store, extra_runs=extra_runs, build_cache=True)
    if apply_changes:
        report = apply(root, store, report)
    return report


def free_guard(root, limits):
    free = shutil.disk_usage(root / "target").free
    if free < limits["free_gib"] * GIB:
        raise ValueError(f"disk reserve reached: less than {limits['free_gib']} GiB free; run release_storage.py prune --apply")


def guard(root, limits):
    free_guard(root, limits)
    target = allocated(root / "target")
    store = allocated(root / "target/release-gate")
    if target > limits["target_gib"] * GIB or store > limits["store_gib"] * GIB:
        raise ValueError("release storage budget reached; prune disposable caches or explicitly revise retained pins/budget")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("audit", "prune", "pin", "unpin", "verify"))
    parser.add_argument("--apply", action="store_true", help="execute a newly computed prune plan under the runner lock")
    parser.add_argument("--build-cache", action="store_true", help="include rebuildable Cargo caches when over the target budget")
    parser.add_argument("--run")
    parser.add_argument("--label")
    parser.add_argument("--stage", help="pin one passing stage and its dependencies instead of a whole run")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    store = gate.Store(root / "target/release-gate", create=False)
    if not store.key:
        raise ValueError("no initialized release store in this checkout")
    with locked(store):
        if args.command == "pin":
            pin(store, args.label or "", args.run or "", args.stage)
            print(json.dumps(retention(store), indent=2))
            return
        if args.command == "unpin":
            existing = retention(store)
            if not args.label or not any(args.label in existing.get(k, {}) for k in ("pins", "stage_pins")):
                raise ValueError("unknown pin label")
            for key in ("pins", "stage_pins"):
                existing.get(key, {}).pop(args.label, None)
            gate.write_json(store.path / "retention.json", store.seal(existing))
            print(json.dumps(existing, indent=2))
            return
        report, closure = plan(root, store, extra_runs=([args.run] if args.run else ()), build_cache=args.build_cache)
        if args.command == "verify":
            print(json.dumps(verify(closure), indent=2))
            return
        if args.command == "prune" and args.apply:
            report = apply(root, store, report)
        if args.json:
            print(json.dumps(report, indent=2))
        else:
            for key in ("target_bytes", "store_bytes", "reclaimable_bytes", "projected_target_bytes", "projected_store_bytes"):
                print(f"{key}: {report[key] / GIB:.2f} GiB")
            print(f"retained: {len(report['retained_runs'])} runs, {report['retained_stages']} stage payloads, {report['retained_preparations']} preparations")
            print(f"actions: {len(report['actions'])}; {'applied' if report.get('applied') else 'dry run'}")
            print("Use --json for the exact candidate paths. Pins and all run metadata are retained.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError) as error:
        print(f"release storage: {error}", file=sys.stderr)
        raise SystemExit(1)
