# SPDX-License-Identifier: GPL-3.0-or-later
"""Adversarial release-policy tests use real isolated child processes and receipts."""
import contextlib
import dataclasses
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

SPEC = importlib.util.spec_from_file_location("release_gate", Path(__file__).resolve().parents[1] / "release_gate.py")
gate = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = gate
SPEC.loader.exec_module(gate)


class ReleasePolicyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        (self.root / ".gitignore").write_text("target/\n")
        self.policy = {"prose": ["docs/**"], "global": ["Cargo.lock", "policy.json"], "owned": ["src/**", "tests/**", "build/**"]}
        self.store = gate.Store(self.root / "target/store")
        self.tools = {"tool": "v1"}

    def tearDown(self):
        self.temporary.cleanup()

    def runner(self, stages, fresh=False, jobs=2):
        runner = gate.Runner(self.root, self.store, gate.source_snapshot(self.root), self.policy, self.tools, jobs, fresh)
        runner.prepare(stages)
        return runner

    def stage(self, name, code="print('ok')", **kwargs):
        return gate.Stage(name, ((sys.executable, "-c", code),), **kwargs)

    def execute(self, runner, stages):
        with contextlib.redirect_stdout(io.StringIO()):
            result = runner.run(stages)
            report = runner.report(final=True)
        return result, report

    def write(self, name, value="x"):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def test_unrelated_prose_and_test_repair_preserve_independent_evidence(self):
        stages = [self.stage("solver", inputs=("src/**",)), self.stage("harness", inputs=("tests/**",))]
        first = self.runner(stages)
        self.assertTrue(self.execute(first, stages)[0])
        self.write("docs/signoff.md")
        self.write("tests/count.py", "16")
        next_run = self.runner(stages)
        self.assertEqual(next_run.decision(stages[0])[0], "reuse")
        self.assertEqual(next_run.decision(stages[1])[0], "run")
        self.execute(next_run, stages)
        self.assertEqual(next_run.results["solver"]["origin_run"], first.run_id)

    def test_unknown_file_toolchain_lockfile_and_policy_invalidate(self):
        stages = [self.stage("solver", inputs=("src/**",))]
        self.execute(self.runner(stages), stages)
        for name in ("mystery.dat", "Cargo.lock", "policy.json"):
            with self.subTest(name=name):
                self.write(name)
                self.assertEqual(self.runner(stages).decision(stages[0])[0], "run")
                (self.root / name).unlink()
        self.tools["tool"] = "v2"
        self.assertEqual(self.runner(stages).decision(stages[0])[0], "run")

    def test_fixture_deleted_or_added_and_changed_selection_invalidate(self):
        self.write("src/fixture.json")
        stage = self.stage("solver", inputs=("src/**",), cases=("a", "b"))
        self.execute(self.runner([stage]), [stage])
        (self.root / "src/fixture.json").unlink()
        self.assertEqual(self.runner([stage]).decision(stage)[0], "run")
        self.write("src/fixture.json")
        changed = dataclasses.replace(stage, cases=("a",))
        self.assertEqual(self.runner([changed]).decision(changed)[0], "run")

    def test_failed_overall_run_keeps_only_successful_independent_stages(self):
        ok = self.stage("ok")
        bad = self.stage("bad", "raise SystemExit(2)", dependencies=("ok",))
        stages = [ok, bad]
        first = self.runner(stages)
        success, report = self.execute(first, stages)
        self.assertFalse(success)
        self.assertFalse(report["complete"])
        second = self.runner(stages)
        self.assertEqual(second.decision(ok)[0], "reuse")
        self.assertEqual(second.decision(bad)[0], "run")
        self.assertEqual(self.store.unseal(first.run_dir / "qualification.json")["status"], "failed")

    def test_tampered_receipt_log_and_output_never_reuse(self):
        stage = self.stage("result", "from pathlib import Path; Path('target/result').write_text('yes')", outputs=("target/result",))
        first = self.runner([stage])
        self.execute(first, [stage])
        receipt_path = self.store.path / "results/result" / (first.keys[stage.id] + ".json")
        original = receipt_path.read_bytes()
        value = json.loads(original)
        value["payload"]["status"] = "failed"
        receipt_path.write_text(json.dumps(value))
        self.assertEqual(self.runner([stage]).decision(stage)[0], "run")
        receipt_path.write_bytes(original)
        output = self.root / "target/result"
        output.write_text("tampered")
        self.assertEqual(self.runner([stage]).decision(stage)[0], "run")
        output.write_text("yes")
        log = next(iter(first.results[stage.id]["evidence"]))
        Path(log).write_text("invented success")
        self.assertEqual(self.runner([stage]).decision(stage)[0], "run")

    def test_timeout_and_blocked_dependents_do_not_pass(self):
        timeout = self.stage("timeout", "import time; time.sleep(10)", timeout=1)
        dependent = self.stage("consumer", dependencies=("timeout",))
        runner = self.runner([timeout, dependent])
        start = time.monotonic()
        success, report = self.execute(runner, [timeout, dependent])
        self.assertLess(time.monotonic() - start, 4)
        self.assertFalse(success)
        self.assertFalse(report["complete"])
        self.assertEqual(runner.results["timeout"]["status"], "timed_out")
        self.assertEqual(runner.results["consumer"]["status"], "blocked")

    def test_resource_constraints_exclude_overlapping_memory_work(self):
        code = "import time; from pathlib import Path; p=Path('target/memory.lock'); p.open('x').close(); time.sleep(.15); p.unlink()"
        stages = [self.stage(f"memory{i}", code, resource="memory") for i in range(3)]
        stages.append(self.stage("exclusive", "from pathlib import Path; assert not Path('target/memory.lock').exists()", resource="exclusive"))
        self.assertTrue(self.execute(self.runner(stages, jobs=3), stages)[0])

    def test_source_mutation_rejects_all_new_cache_evidence(self):
        stage = self.stage("mutator", "from pathlib import Path; Path('unmapped.txt').write_text('changed')")
        runner = self.runner([stage])
        _, report = self.execute(runner, [stage])
        self.assertFalse(report["complete"])
        self.assertFalse(report["source_unchanged"])
        self.assertIsNone(self.store.find(stage.id, runner.keys[stage.id]))

    def test_fresh_bypasses_result_reuse(self):
        stage = self.stage("result")
        self.execute(self.runner([stage]), [stage])
        self.assertEqual(self.runner([stage], fresh=True).decision(stage)[0], "run")

    def test_duplicate_inventory_missing_dependencies_and_cycles_rejected(self):
        with self.assertRaises(ValueError):
            gate.validate_inventory([self.stage("a"), self.stage("a")])
        with self.assertRaises(ValueError):
            gate.validate_inventory([self.stage("a", dependencies=("missing",))])
        with self.assertRaises(ValueError):
            gate.validate_inventory([self.stage("a", dependencies=("b",)), self.stage("b", dependencies=("a",))])

    def test_build_cache_is_not_test_evidence(self):
        self.write("target/build/result.log", "everything passed")
        stage = self.stage("solver")
        self.assertEqual(self.runner([stage]).decision(stage)[0], "run")

    def test_changed_transitive_artifact_invalidates_consumer(self):
        self.write("target/compiler.js", "original")
        stage = self.stage("consumer", artifacts=("target/compiler.js",))
        self.execute(self.runner([stage]), [stage])
        self.write("target/compiler.js", "changed")
        self.assertEqual(self.runner([stage]).decision(stage)[0], "run")

    def test_shared_ordering_dependency_does_not_invalidate_unaffected_work(self):
        dependency = self.stage("preflight", inputs=("tests/**",))
        stage = self.stage("solver", inputs=("src/**",), dependencies=("preflight",))
        self.execute(self.runner([dependency, stage]), [dependency, stage])
        self.write("tests/preflight.py", "correct count")
        runner = self.runner([dependency, stage])
        self.assertEqual(runner.decision(dependency)[0], "run")
        self.assertEqual(runner.decision(stage)[0], "reuse")

    def test_artifact_symlinks_hash_actual_targets_and_reject_cycles(self):
        self.write("target/dependency/file", "original")
        (self.root / "target/link").symlink_to("dependency", target_is_directory=True)
        first = gate.hash_output(self.root / "target/link")
        self.write("target/dependency/file", "modified")
        self.assertNotEqual(first, gate.hash_output(self.root / "target/link"))
        (self.root / "target/dependency/cycle").symlink_to(".", target_is_directory=True)
        with self.assertRaises(ValueError):
            gate.hash_output(self.root / "target/link")

    def test_prose_mode_rejects_embedded_markdown_and_nonprose_changes(self):
        self.write("docs/a.md", "original")
        subprocess.run(["git", "add", "."], cwd=self.root, check=True)
        subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "fixture"], cwd=self.root, check=True)
        self.write("docs/a.md", "changed")
        self.assertEqual(gate.documentation_delta(self.root, "HEAD", self.policy), ["docs/a.md"])
        self.write("src/lib.rs", 'include_str!("../docs/a.md");')
        with self.assertRaises(ValueError):
            gate.documentation_delta(self.root, "HEAD", self.policy)

    def test_serial_and_parallel_inventory_and_verdicts_agree(self):
        stages = [self.stage(f"case{i}", cases=(str(i),)) for i in range(8)]
        serial = self.runner(stages, fresh=True, jobs=1)
        parallel = self.runner(stages, fresh=True, jobs=2)
        self.execute(serial, stages)
        self.execute(parallel, stages)
        self.assertEqual({name: value["status"] for name, value in serial.results.items()},
                         {name: value["status"] for name, value in parallel.results.items()})

    def test_nix_temporary_paths_do_not_discard_identical_evidence(self):
        original = os.environ.get("TMPDIR")
        try:
            os.environ["TMPDIR"] = "/tmp/nix-shell-first"
            stage = self.stage("solver")
            self.execute(self.runner([stage]), [stage])
            os.environ["TMPDIR"] = "/tmp/nix-shell-second"
            self.assertEqual(self.runner([stage]).decision(stage)[0], "reuse")
        finally:
            if original is None:
                os.environ.pop("TMPDIR", None)
            else:
                os.environ["TMPDIR"] = original

    def test_stale_real_catalog_count_fails_without_launching_compilers(self):
        from unittest.mock import patch
        real_root = Path(__file__).resolve().parents[2]
        sample_root = Path("crates/geosolve-sketch-code/assets/bundled-samples")
        for source in (real_root / sample_root).glob("*/manifest.json"):
            self.write(str(source.relative_to(real_root)), source.read_text())
        generator = "packages/geosolve-sketch-code/scripts/generate-bundled-samples.mjs"
        self.write(generator, (real_root / generator).read_text().replace("directories.length, 16", "directories.length, 20"))
        self.write(gate.FRONTEND + "/src/data/samples.json", (real_root / gate.FRONTEND / "src/data/samples.json").read_text())
        with patch.object(subprocess, "Popen", side_effect=AssertionError("preflight launched a subprocess")):
            with self.assertRaisesRegex(ValueError, "count is stale"):
                gate.check_inventory(self.root)

    def test_wasm_and_performance_filters_cannot_pass_with_zero_missing_or_duplicate_tests(self):
        good = "test selected ... timing=1ms\nok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"
        gate.validate_test_output(good, ["selected"])
        for bad in (good.replace("selected", "renamed"), good.replace("1 passed", "0 passed"),
                    good + "test selected ... ok\n", good.replace("0 ignored", "1 ignored"), ""):
            with self.assertRaises(ValueError):
                gate.validate_test_output(bad, ["selected"])

    def test_unmapped_root_script_invalidates_every_semantic_stage(self):
        policy = gate.read_json(Path(__file__).resolve().parents[1] / "release_policy.json")
        stage = self.stage("solver", inputs=("crates/geosolve-core/src/**",))
        snapshot = {"scripts/unknown_input.py": {"sha256": "a"}}
        self.assertIn("scripts/unknown_input.py", gate.stage_inputs(stage, snapshot, policy))


if __name__ == "__main__":
    unittest.main()
