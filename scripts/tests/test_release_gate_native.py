# SPDX-License-Identifier: GPL-3.0-or-later
"""Focused native gate regressions; fake libtests never run the solver suites."""
import importlib.util
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import sys
import tempfile
import time
import unittest

MODULE = Path(__file__).resolve().parents[1] / "release_gate_native.py"
spec = importlib.util.spec_from_file_location("release_gate_native", MODULE)
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


def metadata():
    return {"workspace_members": ["p1"], "packages": [
        {"id": "p1", "name": "example", "manifest_path": "/workspace/example/Cargo.toml",
         "targets": [{"name": "example", "kind": ["lib"], "test": True},
                     {"name": "integration", "kind": ["test"], "test": True},
                     {"name": "benchmark", "kind": ["bench"], "test": False}]},
        {"id": "external", "name": "external", "manifest_path": "/registry/external/Cargo.toml", "targets": []}]}


def artifact_message(name="example", kind="lib", package="p1", executable="/target/example"):
    return {"reason": "compiler-artifact", "package_id": package,
            "target": {"name": name, "kind": [kind]}, "executable": executable,
            "features": ["full"], "profile": {"test": True, "opt_level": "0"}}


def cargo_text(*messages):
    return "\n".join(json.dumps(message) for message in [*messages, {"reason": "build-finished", "success": True}])


class DiscoveryTests(unittest.TestCase):
    def test_cargo_json_retains_target_profile_features_and_excludes_dependencies(self):
        rows = native.cargo_artifacts(cargo_text(artifact_message(), artifact_message(package="external")), metadata())
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["features"], ["full"])
        self.assertEqual(rows[0]["cwd"], "/workspace/example")
        self.assertEqual(rows[0]["profile"]["opt_level"], "0")

    def test_cargo_missing_completion_duplicate_target_and_empty_inventory_fail(self):
        for data in [json.dumps(artifact_message()), cargo_text(artifact_message(), artifact_message()), cargo_text()]:
            with self.subTest(data=data), self.assertRaises(native.NativeError):
                native.cargo_artifacts(data, metadata())

    def test_metadata_selection_preserves_workspace_and_explicit_targets(self):
        self.assertEqual(native.expected_targets(metadata(), native.WORKSPACE_ARGS),
                         {("example", "example", ("lib",)), ("example", "integration", ("test",))})
        self.assertEqual(native.expected_targets(metadata(), ["-p", "example", "--test", "integration"]),
                         {("example", "integration", ("test",))})
        for args in [["-p", "absent"], ["--workspace", "--target", "wasm32-unknown-unknown"], ["-p", "example", "--test", "missing"]]:
            with self.subTest(args=args), self.assertRaises(native.NativeError):
                native.expected_targets(metadata(), args)

    def test_environment_is_decoded_as_data_including_spaces_and_literal_shell_text(self):
        env = {"CARGO_MANIFEST_DIR": "/workspace/a b", "CARGO_MANIFEST_PATH": "/workspace/a b/Cargo.toml",
               "LD_LIBRARY_PATH": "/target/debug:/lib", "CARGO_PKG_DESCRIPTION": "literal $(do-not-run) `nor-this`"}
        command = " ".join([*[shlex.quote(f"{k}={v}") for k, v in env.items()],
                            shlex.quote("/target/test binary"), "--list", "--format", "terse"])
        decoded = native.cargo_launches(f"     Running `{command}`", {"/target/test binary"})
        self.assertEqual(decoded["/target/test binary"], env)

    def test_missing_duplicate_or_wrapped_cargo_launch_fails_closed(self):
        launch = "Running `CARGO_MANIFEST_DIR=/work CARGO_MANIFEST_PATH=/work/Cargo.toml /test --list --format terse`"
        for text in ["", launch + "\n" + launch, launch.replace("/test --list", "wrapper /test --list")]:
            with self.subTest(text=text), self.assertRaises(native.NativeError):
                native.cargo_launches(text, {"/test"})

    def test_inventory_rejects_duplicates_and_custom_harness_output(self):
        self.assertEqual(native.parse_inventory("b: test\na: test\n\n"), ["a", "b"])
        for text in ["a: test\na: test\n", "a: benchmark\n", "Tests succeeded"]:
            with self.assertRaises(native.NativeError):
                native.parse_inventory(text)


class ExecutionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="geosolve-native-adapter-test-")
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def artifact(self):
        return dict(package="example", target={"name": "tests"}, executable=sys.executable,
                    sha256=native.file_hash(sys.executable), cwd=str(self.root),
                    env={"CARGO_MANIFEST_DIR": str(self.root)}, resource="native",
                    cases=["ordinary", "slow"], ignored=["slow"], features=[], profile={"test": True})

    def test_ordinary_and_ignored_selection_cannot_silently_select_zero_tests(self):
        normal = native.execution_stage(self.artifact())
        self.assertEqual(normal["selected"], ["ordinary"])
        self.assertEqual(normal["ignored"], ["slow"])
        ignored = native.execution_stage(self.artifact(), ignored=True, exact="slow", test_threads=1)
        self.assertEqual(ignored["selected"], ["slow"])
        self.assertEqual(ignored["ignored"], [])
        self.assertEqual(ignored["command"][-2:], ["--test-threads", "1"])
        for kwargs in [{"exact": "absent"}, {"exact": "slow"}, {"ignored": True, "exact": "ordinary"}]:
            with self.assertRaises(native.NativeError):
                native.execution_stage(self.artifact(), **kwargs)
        with self.assertRaises(native.NativeError):
            native.execution_stage(self.artifact(), env={"CARGO_MANIFEST_DIR": "/wrong"})

    def test_workspace_stack_env_and_doctest_obligation_are_explicit(self):
        stages = native.workspace_stages({"cargo_args": native.WORKSPACE_ARGS, "artifacts": [self.artifact()]})
        self.assertEqual(stages[0]["env"]["RUST_MIN_STACK"], "16777216")
        self.assertEqual(native.doctest_stage(self.root)["command"],
                         ["cargo", "test", "--locked", "--workspace", "--all-features", "--doc"])
        with self.assertRaises(native.NativeError):
            native.workspace_stages({"cargo_args": ["--workspace"], "artifacts": [self.artifact()]})

    def test_default_feature_ignored_headless_stage_inventory(self):
        artifacts = []
        for target in native.HEADLESS_TARGETS:
            a = self.artifact()
            a["package"], a["target"] = "geosolve-headless", {"name": target}
            a["cases"] = native.HEADLESS_EXACT if target == "m87_headless" else ["intent"]
            a["ignored"] = a["cases"]
            artifacts.append(a)
        args = ["-p", "geosolve-headless", *[v for n in native.HEADLESS_TARGETS for v in ("--test", n)]]
        result = native.ignored_headless_stages({"cargo_args": args, "artifacts": artifacts})
        self.assertEqual(len(result), 7)
        self.assertEqual(sum(len(s["selected"]) for s in result), 7)
        with self.assertRaises(native.NativeError):
            native.ignored_headless_stages({"cargo_args": [*args, "--all-features"], "artifacts": artifacts})

    def test_successful_exit_alone_cannot_replace_exact_completed_inventory(self):
        good = "test ordinary ... ok\ntest slow ... ignored, expensive\n\ntest result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"
        native.validate_results(good, ["ordinary"], ["slow"])
        with self.assertRaises(native.NativeError):
            native.validate_results(good, ["ordinary"], ["slow"], filtered_out=1)
        for bad in [good.replace("ordinary", "unrelated"), good.replace("1 passed", "0 passed"),
                    good + "test ordinary ... ok\n", "", good.replace(" ... ok", " ... FAILED")]:
            with self.assertRaises(native.NativeError):
                native.validate_results(bad, ["ordinary"], ["slow"])

    def run_fake(self, source, directory="execution", timeout=2):
        stage = native.execution_stage(self.artifact(), exact="ordinary")
        stage["command"] = [sys.executable, "-c", source]
        return native.run_stage(stage, self.root / directory, timeout=timeout,
                                env=os.environ | {"GEOSOLVE_FAKE_ENV": "retained"})

    def test_child_gets_package_cwd_and_environment_and_result_is_retained(self):
        receipt = self.run_fake("import os; assert os.getcwd()==os.environ['CARGO_MANIFEST_DIR']; assert os.environ['GEOSOLVE_FAKE_ENV']=='retained'; print('test ordinary ... ok'); print('test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s')")
        self.assertEqual(receipt["status"], "passed")
        self.assertTrue((self.root / "execution/result.json").is_file())

    def test_nonzero_child_missing_case_and_timeout_never_pass(self):
        for index, source in enumerate(["raise SystemExit(7)", "print('not a test result')", "import time; time.sleep(5)"]):
            with self.subTest(source=source), self.assertRaises(native.NativeError):
                self.run_fake(source, str(index), timeout=0.1)
            result = json.loads((self.root / str(index) / "result.json").read_text())
            self.assertEqual(result["status"], "failed")
            if index == 2:
                self.assertEqual(result["process"]["status"], "timeout")

    def test_changed_executable_is_rejected_before_execution(self):
        stage = native.execution_stage(self.artifact())
        stage["executable_sha256"] = "0" * 64
        with self.assertRaises(native.NativeError):
            native.run_stage(stage, self.root / "must-not-exist", timeout=1)
        self.assertFalse((self.root / "must-not-exist").exists())

    def test_changed_cli_executable_is_rejected_before_execution(self):
        auxiliary = self.root / "geosolve-headless"
        auxiliary.write_text("qualified CLI")
        stage = native.execution_stage(self.artifact())
        stage["auxiliary_executables"] = [{"path": str(auxiliary), "sha256": native.file_hash(auxiliary)}]
        auxiliary.write_text("different CLI")
        with self.assertRaisesRegex(native.NativeError, "auxiliary executable changed"):
            native.run_stage(stage, self.root / "must-not-exist", timeout=1)
        self.assertFalse((self.root / "must-not-exist").exists())

    @unittest.skipUnless(Path("/proc/self/status").exists(), "process-tree check uses Linux procfs")
    def test_timeout_reaps_a_descendant_that_ignores_term(self):
        pidfile = self.root / "descendant.pid"
        descendant = f"import os,signal,time; from pathlib import Path; signal.signal(signal.SIGTERM,signal.SIG_IGN); Path({str(pidfile)!r}).write_text(str(os.getpid())); time.sleep(60)"
        parent = f"import subprocess,sys,time; subprocess.Popen([sys.executable,'-c',{descendant!r}]); time.sleep(60)"
        with self.assertRaises(native.NativeError):
            self.run_fake(parent, "descendant-timeout", timeout=0.3)
        self.assertTrue(pidfile.exists())
        pid = int(pidfile.read_text())
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            status = Path(f"/proc/{pid}/stat")
            if not status.exists() or status.read_text().split()[2] == "Z":
                break
            time.sleep(0.01)
        else:
            os.kill(pid, signal.SIGKILL)
            self.fail("timed-out stage leaked a live descendant")

    def test_sigterm_to_adapter_records_interruption_and_stops_child(self):
        marker = self.root / "started"
        child = f"from pathlib import Path; import time; Path({str(marker)!r}).write_text('ready'); time.sleep(60)"
        wrapper = (f"import sys,os; from pathlib import Path; sys.path.insert(0,{str(MODULE.parent)!r}); "
                   f"import release_gate_native as n; n.process([sys.executable,'-c',{child!r}], "
                   f"{str(self.root)!r},dict(os.environ),Path({str(self.root / 'signal')!r}),60)")
        process = subprocess.Popen([sys.executable, "-c", wrapper], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            deadline = time.monotonic() + 3
            while not marker.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(marker.exists())
            process.send_signal(signal.SIGTERM)
            self.assertNotEqual(process.wait(timeout=3), 0)
            receipt = json.loads((self.root / "signal/process.json").read_text())
            self.assertEqual(receipt["status"], "interrupted")
            self.assertLess(receipt["exit_code"], 0)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()


if __name__ == "__main__":
    unittest.main()
