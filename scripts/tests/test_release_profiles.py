# SPDX-License-Identifier: GPL-3.0-or-later
"""Profile configuration and authenticated reuse checks without compiling products."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate as gate


class ReleaseProfileTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        (self.root / ".gitignore").write_text("target/\n")
        self.policy = gate.read_json(Path(__file__).resolve().parents[2] / gate.POLICY_PATH)
        policy_path = self.root / gate.POLICY_PATH
        policy_path.parent.mkdir(parents=True)
        policy_path.write_text(json.dumps(self.policy))
        for package in ("geosolve-headless", "geosolve-demo-web"):
            crate = self.root / "crates" / package
            (crate / "src").mkdir(parents=True)
            (crate / "Cargo.toml").write_text(f'[package]\nname="{package}"\nversion="0.1.0"\n')
            (crate / "src/lib.rs").write_text("pub fn fixture() {}\n")
        self.store = gate.Store(self.root / "target/store")
        # Ignore user profile choices only inside each isolated test. The CLI
        # override tests supply their own values and verify they are retained.
        self.environment = {key: value for key, value in os.environ.items()
                            if not key.startswith("CARGO_PROFILE_") and key != "CARGO_BUILD_JOBS"}

    def configured_environment(self, overrides=None, arguments=()):
        observed = {}

        def observe_tools(_root):
            observed.update(os.environ)
            return {}

        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.dict(os.environ, {**self.environment, **(overrides or {})}, clear=True))
            stack.enter_context(patch.object(sys, "argv", ["release_gate.py", "--plan", "--preflight", *arguments]))
            stack.enter_context(patch.object(gate, "ROOT", self.root))
            stack.enter_context(patch.object(gate, "tool_identity", side_effect=observe_tools))
            # Exercise the real CLI and Runner construction, but never run a
            # compiler, package manager, browser, or repository qualification.
            stack.enter_context(patch.object(gate, "preflight_stages", return_value=[]))
            stack.enter_context(patch.object(gate, "preparation_stages", return_value=([], {})))
            stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            self.assertEqual(gate.main(), 0)
        return observed

    def runner(self, environment, stages=(), fresh=False):
        with patch.dict(os.environ, environment, clear=True):
            runner = gate.Runner(self.root, self.store, gate.source_snapshot(self.root),
                                 self.policy, {"compiler": "fixed-test-toolchain"}, fresh=fresh)
            runner.prepare(list(stages))
        return runner

    def execute(self, runner, stages, environment):
        with patch.dict(os.environ, environment, clear=True), contextlib.redirect_stdout(io.StringIO()):
            self.assertTrue(runner.run(stages))
            report = runner.report(final=True)
        self.assertTrue(report["complete"])
        return report

    def test_cli_defaults_keep_checked_tests_and_original_release_codegen_count(self):
        environment = self.configured_environment()
        self.assertEqual(environment["CARGO_PROFILE_TEST_OPT_LEVEL"], "1")
        self.assertEqual(environment["CARGO_PROFILE_TEST_DEBUG_ASSERTIONS"], "true")
        self.assertEqual(environment["CARGO_PROFILE_TEST_OVERFLOW_CHECKS"], "true")
        self.assertEqual(environment["CARGO_PROFILE_TEST_DEBUG"], "line-tables-only")
        for profile in ("RELEASE", "BENCH"):
            self.assertEqual(environment[f"CARGO_PROFILE_{profile}_INCREMENTAL"], "true")
            self.assertEqual(environment[f"CARGO_PROFILE_{profile}_CODEGEN_UNITS"], "16")
            for unchanged in ("OPT_LEVEL", "LTO", "PANIC", "DEBUG_ASSERTIONS", "OVERFLOW_CHECKS"):
                self.assertNotIn(f"CARGO_PROFILE_{profile}_{unchanged}", environment)

    def test_unoptimized_option_still_forces_assertions_and_overflow_checks(self):
        environment = self.configured_environment({
            "CARGO_PROFILE_TEST_OPT_LEVEL": "3",
            "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS": "false",
            "CARGO_PROFILE_TEST_OVERFLOW_CHECKS": "false",
        }, ("--test-opt-level", "0"))
        self.assertEqual(environment["CARGO_PROFILE_TEST_OPT_LEVEL"], "0")
        self.assertEqual(environment["CARGO_PROFILE_TEST_DEBUG_ASSERTIONS"], "true")
        self.assertEqual(environment["CARGO_PROFILE_TEST_OVERFLOW_CHECKS"], "true")

    def test_explicit_diagnostic_and_release_profile_overrides_remain_inputs(self):
        overrides = {
            "CARGO_PROFILE_TEST_DEBUG": "2",
            "CARGO_PROFILE_RELEASE_INCREMENTAL": "false",
            "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "8",
            "CARGO_PROFILE_BENCH_INCREMENTAL": "false",
            "CARGO_PROFILE_BENCH_CODEGEN_UNITS": "1",
            "CARGO_BUILD_JOBS": "2",
        }
        environment = self.configured_environment(overrides)
        runner = self.runner(environment)
        for key, value in overrides.items():
            with self.subTest(variable=key):
                self.assertEqual(environment[key], value)
                self.assertIn(key, runner.environment)
                self.assertNotEqual(runner.environment[key], value)

    def test_profile_change_invalidates_all_prepared_paths_and_stage_keys(self):
        environment = self.configured_environment()
        baseline = self.runner(environment)
        stages, prepared = gate.preparation_stages(baseline)
        changes = {
            "CARGO_PROFILE_TEST_DEBUG": "2",
            "CARGO_PROFILE_TEST_OPT_LEVEL": "0",
            "CARGO_PROFILE_RELEASE_INCREMENTAL": "false",
            "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "8",
            "CARGO_PROFILE_BENCH_INCREMENTAL": "false",
            "CARGO_PROFILE_BENCH_CODEGEN_UNITS": "8",
            "CARGO_BUILD_JOBS": "2",
        }
        for name, replacement in changes.items():
            changed = self.runner({**environment, name: replacement})
            changed_stages, changed_prepared = gate.preparation_stages(changed)
            with self.subTest(variable=name):
                for mode, output in prepared.items():
                    self.assertNotEqual(output, changed_prepared[mode], mode)
                for original, updated in zip(stages, changed_stages, strict=True):
                    self.assertEqual(original.id, updated.id)
                    self.assertNotEqual(baseline.key(original), changed.key(updated), original.id)

    def test_changed_profiles_cannot_reuse_an_authenticated_success(self):
        environment = self.configured_environment()
        stage = gate.Stage("profile-probe", ((sys.executable, "-c", "print('profile probe passed')"),))
        baseline = self.runner(environment, [stage])
        self.execute(baseline, [stage], environment)
        self.assertEqual(self.runner(environment, [stage]).decision(stage)[0], "reuse")
        for name, value in (
            ("CARGO_PROFILE_TEST_DEBUG", "2"),
            ("CARGO_PROFILE_RELEASE_INCREMENTAL", "false"),
            ("CARGO_PROFILE_BENCH_INCREMENTAL", "false"),
            ("CARGO_BUILD_JOBS", "2"),
        ):
            with self.subTest(variable=name):
                altered = self.runner({**environment, name: value}, [stage])
                self.assertEqual(altered.decision(stage)[0], "run")
        self.assertEqual(self.runner(environment, [stage], fresh=True).decision(stage)[0], "run")

    def test_cargo_job_limit_reaches_child_and_matches_report(self):
        for override, expected in ((None, "4"), ("2", "2")):
            with self.subTest(caller_override=override):
                environment = self.configured_environment({} if override is None else {"CARGO_BUILD_JOBS": override})
                code = "import os; print('CARGO_BUILD_JOBS=' + os.environ['CARGO_BUILD_JOBS'])"
                stage = gate.Stage("cargo-jobs-probe", ((sys.executable, "-c", code),), reusable=False)
                runner = self.runner(environment, [stage])
                report = self.execute(runner, [stage], environment)
                output = (runner.run_dir / "stages" / stage.id / "output.log").read_text()
                self.assertIn(f"\nCARGO_BUILD_JOBS={expected}\n", output)
                self.assertEqual(str(report["worker_limits"]["cargo_build_jobs"]), expected)

    def test_frontend_preflight_disables_mutable_test_cache(self):
        frontend = next(stage for stage in gate.preflight_stages() if stage.id == "preflight.frontend")
        self.assertIn(gate.npm(gate.FRONTEND, "test", "--", "--no-cache"), frontend.commands)
        self.assertIn(gate.npm(gate.FRONTEND, "run", "check:static"), frontend.commands)


if __name__ == "__main__":
    unittest.main()
