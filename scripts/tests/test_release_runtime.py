# SPDX-License-Identifier: GPL-3.0-or-later
"""Protected executable and loader fixtures; no product compiler or tests run."""
import errno
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate_native as native
import release_gate_runtime as runtime


class CaptureTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "cargo-test"
        self.source.write_bytes(b"original executable bytes\n")
        self.source.chmod(0o755)
        self.expected = native.file_hash(self.source)

    def test_copy_fallback_never_shares_cargo_inode_and_survives_replacement(self):
        for number, error in enumerate((errno.EXDEV, errno.EOPNOTSUPP, errno.EPERM)):
            with self.subTest(errno=error):
                destination = self.root / str(number) / "captured"
                with patch.object(native.fcntl, "ioctl", side_effect=OSError(error, "no clone")):
                    native.capture_executable(self.source, destination, self.expected)
                self.assertNotEqual(self.source.stat().st_ino, destination.stat().st_ino)
                self.assertEqual(destination.stat().st_mode & 0o777, 0o555)
                self.source.write_bytes(b"new Cargo profile\n")
                self.assertEqual(native.file_hash(destination), self.expected)
                self.source.write_bytes(b"original executable bytes\n")

    def test_partial_failed_clone_resets_offsets_before_copy(self):
        destination = self.root / "captured"

        def failed_clone(target, _operation, source):
            os.write(target, b"partial clone junk")
            os.read(source, 7)
            raise OSError(errno.EXDEV, "unsupported")

        with patch.object(native.fcntl, "ioctl", side_effect=failed_clone):
            native.capture_executable(self.source, destination, self.expected)
        self.assertEqual(destination.read_bytes(), self.source.read_bytes())

    def test_unexpected_io_error_and_existing_destination_fail_without_overwrite(self):
        destination = self.root / "captured"
        with patch.object(native.fcntl, "ioctl", side_effect=OSError(errno.EIO, "device failure")):
            with self.assertRaises(OSError):
                native.capture_executable(self.source, destination, self.expected)
        destination.write_bytes(b"retained previous evidence")
        with self.assertRaises(FileExistsError):
            native.capture_executable(self.source, destination, self.expected)
        self.assertEqual(destination.read_bytes(), b"retained previous evidence")

    def test_source_mutation_or_symbolic_link_is_rejected(self):
        self.source.write_bytes(b"mutated source")
        with self.assertRaisesRegex(native.NativeError, "changed while capturing"):
            native.capture_executable(self.source, self.root / "changed", self.expected)
        link = self.root / "symlink"
        link.symlink_to(self.source)
        with self.assertRaisesRegex(native.NativeError, "non-symlink"):
            native.capture_executable(link, self.root / "linked", native.file_hash(link))


class RuntimeInspectionTests(unittest.TestCase):
    executable = "/private/prepared/test"
    loader = "/nix/store/glibc/lib/ld-linux-x86-64.so.2"
    library = "/nix/store/glibc/lib/libc.so.6"
    readelf = "/nix/store/binutils/bin/readelf"

    def inspect(self, *, environment=None, original_library=None, library=None, dynamic=None, header=None):
        def capture(command, *, env, **_kwargs):
            if command[1] == "-lW":
                return header if header is not None else f"[Requesting program interpreter: {self.loader}]"
            if command[1] == "--list":
                selected = original_library if "/workspace" in env.get("LD_LIBRARY_PATH", "") else library
                selected = selected or self.library
                return f"linux-vdso.so.1 (0x1)\nlibc.so.6 => {selected} (0x2)\n{self.loader} (0x3)\n"
            return dynamic or "0x0 (NULL) 0x0\n"

        def immutable(path):
            return str(path).startswith("/nix/store/")

        with patch.object(runtime.shutil, "which", return_value=self.readelf), \
                patch.object(runtime, "immutable", side_effect=immutable), \
                patch.object(runtime.subprocess, "check_output", side_effect=capture), \
                patch.object(native, "file_hash", side_effect=lambda p: hashlib.sha256(str(p).encode()).hexdigest()):
            return runtime.inspect_runtime(self.executable, environment or {})

    def test_stable_store_closure_filters_mutable_cargo_search_paths(self):
        closure = self.inspect(environment={"LD_LIBRARY_PATH": "/workspace/target/deps:/nix/store/lib/lib"})
        self.assertTrue(closure["safe"], closure)
        self.assertEqual(closure["schema"], runtime.SCHEMA)
        self.assertEqual(closure["executable"], self.executable)
        self.assertEqual(closure["ld_library_path"], "/nix/store/lib/lib")
        self.assertEqual(set(closure["libraries"]), {self.loader, self.library})

    def test_loader_resolution_changes_and_mutable_libraries_fall_back(self):
        closure = self.inspect(environment={"LD_LIBRARY_PATH": "/workspace/target/deps"},
                               original_library="/workspace/target/deps/libc.so.6")
        self.assertFalse(closure["safe"])
        self.assertIn("mutable build outputs", closure["reason"])
        self.assertFalse(self.inspect(library="/tmp/replaceable-lib.so")["safe"])

    def test_loader_preload_relative_origin_and_optional_modules_fall_back(self):
        self.assertFalse(self.inspect(environment={"LD_PRELOAD": "/nix/store/preload/lib.so"})["safe"])
        for value in ("$ORIGIN", "/nix/store/lib/lib:", "/tmp/link", "/nix/store/../tmp"):
            with self.subTest(search_path=value):
                self.assertFalse(self.inspect(dynamic=f"0x1 (RUNPATH) Library runpath: [{value}]")["safe"])
        for tag in ("AUDIT", "DEPAUDIT", "FILTER", "AUXILIARY"):
            with self.subTest(tag=tag):
                self.assertFalse(self.inspect(dynamic=f"0x1 ({tag}) [optional.so]")["safe"])

    def test_missing_interpreter_or_inspector_fails_closed(self):
        self.assertFalse(self.inspect(header="static/unknown executable")["safe"])
        with patch.object(runtime.shutil, "which", return_value=None):
            self.assertFalse(runtime.inspect_runtime(self.executable, {})["safe"])
        with patch.object(runtime.shutil, "which", return_value=self.readelf), \
                patch.object(runtime, "immutable", return_value=True), \
                patch.object(runtime.subprocess, "check_output", side_effect=subprocess.TimeoutExpired("readelf", 20)):
            self.assertFalse(runtime.inspect_runtime(self.executable, {})["safe"])

    def test_loader_parser_rejects_missing_unresolved_or_duplicate_dependencies(self):
        for value in ("", "libc.so.6 => not found", "nonsense", "/lib/libc.so (0x1)\n/lib/libc.so (0x1)"):
            with self.subTest(output=value), self.assertRaises(ValueError):
                runtime.loader_lines(value)

    def test_external_symlink_into_store_is_not_immutable(self):
        with patch.object(Path, "resolve", return_value=Path("/nix/store/lib/libc.so.6")), \
                patch.object(Path, "is_file", return_value=True):
            self.assertFalse(runtime.immutable("/tmp/replaceable-symlink"))
            self.assertFalse(runtime.immutable("/nix/store/../outside/symlink"))
            self.assertTrue(runtime.immutable("/nix/store/lib/libc.so.6"))


class RuntimeExecutionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.library = self.root / "fake-loader"
        self.library.write_text("immutable loader fixture")
        self.environment = {key: value for key, value in os.environ.items()
                            if not key.startswith("LD_") and key != "GLIBC_TUNABLES"}
        self.environment["LD_LIBRARY_PATH"] = ""
        self.closure = {"schema": runtime.SCHEMA, "safe": True, "executable": sys.executable,
                        "loader": str(self.library), "ld_library_path": "", "loader_environment": {"LD_LIBRARY_PATH": ""},
                        "libraries": {str(self.library): native.file_hash(self.library)}}
        self.artifact = {"package": "fixture", "target": {"name": "tests"}, "executable": sys.executable,
                         "sha256": native.file_hash(sys.executable), "cwd": str(self.root), "env": {"LD_LIBRARY_PATH": ""},
                         "resource": "native", "cases": ["ordinary"], "ignored": [], "features": ["full"],
                         "profile": {"test": True, "opt_level": "1"}, "build_overlap_safe": True,
                         "runtime_closure": self.closure}

    def stage(self, prefix=""):
        stage = native.execution_stage(self.artifact)
        source = prefix + "print('test ordinary ... ok'); print('test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s')"
        stage["command"] = [sys.executable, "-c", source]
        return stage

    def test_legacy_metadata_remains_locked_and_retains_profile(self):
        artifact = {key: value for key, value in self.artifact.items() if key not in ("build_overlap_safe", "runtime_closure")}
        stage = native.execution_stage(artifact)
        self.assertFalse(stage["build_overlap_safe"])
        self.assertEqual(stage["features"], ["full"])
        self.assertEqual(stage["profile"], {"test": True, "opt_level": "1"})

    def test_protected_child_runs_with_its_bound_runtime(self):
        with patch.object(runtime, "immutable", return_value=True):
            receipt = native.run_stage(self.stage(), self.root / "run", timeout=2, env=self.environment)
        self.assertEqual(receipt["status"], "passed")

    def test_changed_loader_environment_closure_owner_or_missing_auxiliary_fails(self):
        for change in ("preload", "owner", "missing_auxiliary", "empty_libraries"):
            with self.subTest(change=change):
                stage = self.stage()
                stage["runtime_closures"] = [dict(self.closure)]
                environment = dict(self.environment)
                if change == "preload":
                    environment["LD_PRELOAD"] = "/unexpected.so"
                elif change == "owner":
                    stage["runtime_closures"][0]["executable"] = "/other/test"
                elif change == "missing_auxiliary":
                    stage["runtime_closures"] = []
                else:
                    stage["runtime_closures"][0]["libraries"] = {}
                with patch.object(runtime, "immutable", return_value=True), self.assertRaises(native.NativeError):
                    native.run_stage(stage, self.root / change, timeout=2, env=environment)
                self.assertFalse((self.root / change).exists())

    def test_postexecution_library_mutation_cannot_produce_passing_receipt(self):
        prefix = f"from pathlib import Path; Path({str(self.library)!r}).write_text('changed'); "
        with patch.object(runtime, "immutable", return_value=True), \
                self.assertRaisesRegex(native.NativeError, "runtime library changed"):
            native.run_stage(self.stage(prefix), self.root / "mutated", timeout=2, env=self.environment)
        result = json.loads((self.root / "mutated/result.json").read_text())
        self.assertEqual(result["status"], "failed")

    def test_executable_replacement_during_child_execution_cannot_pass(self):
        for owner in ("main", "auxiliary"):
            with self.subTest(owner=owner):
                executable = self.root / f"{owner}-child"
                auxiliary = self.root / f"{owner}-cli"
                auxiliary.write_text("original CLI bytes")
                auxiliary.chmod(0o555)
                replaced = executable if owner == "main" else auxiliary
                prefix = (f"from pathlib import Path; original = Path({str(replaced)!r}); "
                          "replacement = original.with_suffix('.next'); "
                          "replacement.write_text('replacement bytes'); replacement.replace(original); ")
                executable.write_text(f"#!{sys.executable}\n" + self.stage(prefix)["command"][2] + "\n")
                executable.chmod(0o555)
                stage = self.stage()
                stage["command"] = [str(executable)]
                stage["executable_sha256"] = native.file_hash(executable)
                stage["runtime_closures"] = [self.closure | {"executable": str(executable)}]
                if owner == "auxiliary":
                    stage["auxiliary_executables"] = [{"path": str(auxiliary), "sha256": native.file_hash(auxiliary),
                                                       "cargo_env": "CARGO_BIN_EXE_fixture", "runtime_env": "FIXTURE_CLI"}]
                    stage["env"].update(CARGO_BIN_EXE_fixture=str(auxiliary), FIXTURE_CLI=str(auxiliary))
                    stage["runtime_closures"].append(self.closure | {"executable": str(auxiliary)})
                output = self.root / f"{owner}-replaced"
                with patch.object(runtime, "immutable", return_value=True), \
                        self.assertRaisesRegex(native.NativeError, "changed during execution"):
                    native.run_stage(stage, output, timeout=2, env=self.environment)
                result = json.loads((output / "result.json").read_text())
                self.assertEqual(result["status"], "failed")
                self.assertEqual(result["process"]["status"], "passed")


class PreparationFallbackTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.original = self.root / "target/test-executable"
        self.original.parent.mkdir()
        self.original.write_text("test executable bytes")
        self.original.chmod(0o755)
        self.cli = self.root / "target/fixture-cli"
        self.cli.write_text("CLI bytes")
        self.cli.chmod(0o755)
        self.manifest = self.root / "Cargo.toml"
        self.metadata = {"workspace_members": ["fixture"], "packages": [{
            "id": "fixture", "name": "fixture", "manifest_path": str(self.manifest),
            "targets": [{"name": "fixture", "kind": ["lib"], "test": True}],
        }]}
        self.profile = {"test": True, "opt_level": "1", "debug_assertions": True, "overflow_checks": True}
        self.artifact = {"reason": "compiler-artifact", "package_id": "fixture",
                         "target": {"name": "fixture", "kind": ["lib"]}, "features": ["full"],
                         "profile": self.profile, "executable": str(self.original)}
        self.listed = []

    def prepare(self, name, *, safe, auxiliary=False):
        import shlex
        messages = [self.artifact]
        if auxiliary:
            messages.append({"reason": "compiler-artifact", "package_id": "fixture",
                             "target": {"name": "fixture-cli", "kind": ["bin"]},
                             "profile": {"test": False}, "executable": str(self.cli)})
        compiler = "\n".join(json.dumps(row) for row in [*messages, {"reason": "build-finished", "success": True}])
        launch = {"CARGO_MANIFEST_DIR": str(self.root), "CARGO_MANIFEST_PATH": str(self.manifest),
                  "LD_LIBRARY_PATH": str(self.root / "target")}
        if auxiliary:
            launch["CARGO_BIN_EXE_fixture-cli"] = str(self.cli)

        def checked(command, _cwd, _environment, output, _timeout):
            output.mkdir(parents=True)
            if command[:2] == ["cargo", "metadata"]:
                return json.dumps(self.metadata)
            if command[:2] == ["cargo", "test"]:
                if "-vv" in command:
                    tokens = [*(shlex.quote(f"{key}={value}") for key, value in launch.items()),
                              shlex.quote(str(self.original)), "--list", "--format", "terse"]
                    (output / "stderr.log").write_text("Running `" + " ".join(tokens) + "`\n")
                return compiler
            self.listed.append(command[0])
            return "" if "--ignored" in command else "ordinary: test\n"

        def inspect(executable, _environment):
            eligible = safe if not auxiliary or Path(executable).name == self.original.name else False
            return {"schema": runtime.SCHEMA, "safe": eligible, "executable": str(executable),
                    "reason": "unknown relative loader fixture", "ld_library_path": ""}

        with patch.object(native, "checked_process", side_effect=checked), \
                patch.object(runtime, "inspect_runtime", side_effect=inspect):
            return native.prepare(self.root, ["--workspace", "--all-features", "--lib"], self.root / name, env={})

    def test_unknown_runtime_keeps_original_launch_path_and_cargo_environment(self):
        prepared = self.prepare("fallback", safe=False)
        artifact = prepared["artifacts"][0]
        self.assertFalse(artifact["build_overlap_safe"])
        self.assertEqual(artifact["executable"], str(self.original))
        self.assertEqual(self.listed, [str(self.original)] * 2)
        self.assertEqual(artifact["env"]["LD_LIBRARY_PATH"], str(self.root / "target"))
        self.assertEqual(Path(artifact["captured_executable"]).read_bytes(), self.original.read_bytes())
        self.assertEqual(artifact["profile"], self.profile)
        self.assertEqual(artifact["features"], ["full"])

    def test_protected_runtime_discovers_and_executes_private_copy(self):
        prepared = self.prepare("protected", safe=True)
        artifact = prepared["artifacts"][0]
        self.assertTrue(artifact["build_overlap_safe"])
        self.assertEqual(artifact["executable"], artifact["captured_executable"])
        self.assertNotEqual(artifact["executable"], str(self.original))
        self.assertEqual(self.listed, [artifact["executable"]] * 2)
        self.assertEqual(artifact["env"]["LD_LIBRARY_PATH"], "")
        self.original.write_text("later Cargo build")
        self.assertEqual(native.file_hash(artifact["executable"]), artifact["sha256"])

    def test_unknown_auxiliary_loader_keeps_original_cli_and_records_both_paths(self):
        prepared = self.prepare("auxiliary-fallback", safe=True, auxiliary=True)
        artifact = prepared["artifacts"][0]
        self.assertFalse(artifact["build_overlap_safe"])
        dependency = artifact["auxiliary_executables"][0]
        self.assertEqual(dependency["path"], str(self.cli))
        self.assertEqual(artifact["env"]["CARGO_BIN_EXE_fixture-cli"], str(self.cli))
        self.assertEqual({item["path"] for item in prepared["auxiliary_executables"]},
                         {str(self.cli), dependency["captured_path"]})


if __name__ == "__main__":
    unittest.main()
