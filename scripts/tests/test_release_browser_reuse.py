# SPDX-License-Identifier: GPL-3.0-or-later
import copy
import json
import os
import re
import subprocess
from types import SimpleNamespace
from unittest.mock import patch
from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate as gate
import release_browser as browser


def witness(key="one"):
    return {"format": "geosolve-browser-sample-prefix-v1", "key": key, "title": "One",
            "project": "geosolve-sample-" + key, "workspaceBytes": 123,
            **{name: "a" * 64 for name in ("workspaceSha256", "acceptedSourceSha256",
               "fittedGeometrySha256", "authoritativeFrameSha256")}}


def report(statuses):
    return {"errors": [], "suites": [{"specs": [
        {"file": "m92-sample-audit.spec.ts", "title": browser.SAMPLE_PREFIX + key,
         "tests": [{"projectName": "chromium", "expectedStatus": "passed",
                    "status": "expected" if status == "passed" else "unexpected",
                    "results": [{"status": status, "retry": 0, "errors": []}]}]}
        for key, status in statuses]}]}


class BrowserReuseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.store = gate.Store(self.root / "store")
        self.row = gate.browser_rows(report([("one", "passed")]))[0]

    def tearDown(self):
        self.temp.cleanup()

    def test_program_excludes_only_explicit_catalog_data(self):
        inputs = {browser.SAMPLE_ROOT + "/one/sketch.ts": "sample",
                  browser.CATALOG: "membership", browser.CATALOG_FRONTEND: "projection",
                  "crates/geosolve-sketch-code/build.rs": "template",
                  "crates/geosolve-core/src/solver.rs": "program", "unreviewed-input.dat": "unknown"}
        policy = {"global": [], "owned": [browser.SAMPLE_ROOT + "/**", browser.CATALOG, browser.CATALOG_FRONTEND]}
        inputs = {name: {"sha256": value, "executable": False} for name, value in inputs.items()}
        selected = browser.program_inputs(inputs, policy)
        self.assertEqual(set(selected), {"crates/geosolve-sketch-code/build.rs",
                                       "crates/geosolve-core/src/solver.rs", "unreviewed-input.dat"})
        self.assertNotEqual(gate.digest(selected), gate.digest(dict(selected, **{"unreviewed-input.dat": "changed"})))

    def test_sample_data_is_independent_but_manifest_and_witness_changes_invalidate(self):
        snapshot = {}
        for key in ("one", "two"):
            prefix = browser.SAMPLE_ROOT + "/" + key + "/"
            directory = self.root / prefix
            directory.mkdir(parents=True)
            (directory / "manifest.json").write_text(json.dumps({"key": key, "title": key, "ordinal": 1}))
            for filename in ("manifest.json", "sketch.ts", "witnesses.json"):
                snapshot[prefix + filename] = {"sha256": filename}
        original = browser.sample_inputs(self.root, snapshot, "one")
        snapshot[browser.SAMPLE_ROOT + "/two/sketch.ts"] = {"sha256": "different"}
        self.assertEqual(original, browser.sample_inputs(self.root, snapshot, "one"))
        path = self.root / browser.SAMPLE_ROOT / "one/manifest.json"
        value = json.loads(path.read_text())
        value["ordinal"] = 2
        path.write_text(json.dumps(value))
        self.assertEqual(original, browser.sample_inputs(self.root, snapshot, "one"))
        value["title"] = "Different"
        path.write_text(json.dumps(value))
        self.assertNotEqual(original, browser.sample_inputs(self.root, snapshot, "one"))
        snapshot[browser.SAMPLE_ROOT + "/one/witnesses.json"] = {"sha256": "different"}
        self.assertNotEqual(original, browser.sample_inputs(self.root, snapshot, "one"))

    def test_complete_initial_state_and_exact_case_identity_are_reuse_inputs(self):
        original = browser.leaf_key("program", {"data": "same"}, self.row, witness())
        for field in ("workspaceSha256", "acceptedSourceSha256", "fittedGeometrySha256", "authoritativeFrameSha256"):
            changed = witness()
            changed[field] = "b" * 64
            self.assertNotEqual(original, browser.leaf_key("program", {"data": "same"}, self.row, changed))
        changed = copy.deepcopy(self.row)
        changed["project"] = "chromium-memory"
        self.assertNotEqual(original, browser.leaf_key("program", {"data": "same"}, changed, witness()))
        self.assertNotEqual(original, browser.leaf_key("new compiler", {"data": "same"}, self.row, witness()))

    def test_incomplete_or_wrong_witness_cannot_authorize_reuse(self):
        for field in ("workspaceSha256", "workspaceBytes", "project", "key", "authoritativeFrameSha256"):
            value = witness()
            value.pop(field)
            with self.assertRaises(ValueError):
                browser.validate_witness(value, "one")

    def test_leaf_requires_authentic_complete_receipt_and_intact_original_logs(self):
        log = self.store.path / "run/output.json"
        log.parent.mkdir()
        log.write_text("actual completed report")
        key = browser.leaf_key("program", {}, self.row, witness())
        provenance = {str(log): gate.file_hash(log)}
        artifact = {"sha256": "original"}
        batch_path = self.store.path / "run/batch.json"
        gate.write_json(batch_path, self.store.seal({"schema": "geosolve-browser-batch-v1", "program": "program",
                        "origin_run": "original-run", "artifact": artifact, "provenance": provenance,
                        "passed_cases": [browser.identity(self.row)], "evidence": provenance}))
        value = {"schema": browser.SCHEMA, "key": key, "case": browser.identity(self.row),
                 "status": "passed", "complete": True, "artifact": artifact, "program": "program",
                 "sample": {}, "witness": witness(), "origin_run": "original-run", "provenance": provenance,
                 "batch": {"path": str(batch_path), "sha256": gate.file_hash(batch_path)}}
        path = self.store.path / "browser-leaves" / (key + ".json")
        gate.write_json(path, self.store.seal(value))
        self.assertEqual(browser.find_leaf(self.store, key, self.row)["origin_run"], "original-run")
        log.write_text("tampered")
        self.assertIsNone(browser.find_leaf(self.store, key, self.row))
        log.write_text("actual completed report")
        value["complete"] = False
        gate.write_json(path, self.store.seal(value))
        self.assertIsNone(browser.find_leaf(self.store, key, self.row))
        envelope = json.loads(path.read_text())
        envelope["payload"]["complete"] = True
        gate.write_json(path, envelope)
        self.assertIsNone(browser.find_leaf(self.store, key, self.row))

    def test_shared_batch_evidence_is_verified_once_per_run(self):
        self.test_leaf_requires_authentic_complete_receipt_and_intact_original_logs()
        # Establish a fresh valid batch after the deliberate tampering above.
        log = self.store.path / "run/output.json"
        provenance = {str(log): gate.file_hash(log)}
        artifact = {"sha256": "original"}
        key = browser.leaf_key("program", {}, self.row, witness())
        batch_path = self.store.path / "run/shared-batch.json"
        gate.write_json(batch_path, self.store.seal({"schema": "geosolve-browser-batch-v1", "program": "program",
                        "origin_run": "run", "artifact": artifact, "provenance": provenance,
                        "passed_cases": [browser.identity(self.row)], "evidence": provenance}))
        value = {"schema": browser.SCHEMA, "key": key, "case": browser.identity(self.row), "status": "passed",
                 "complete": True, "artifact": artifact, "program": "program", "sample": {}, "witness": witness(),
                 "origin_run": "run", "provenance": provenance,
                 "batch": {"path": str(batch_path), "sha256": gate.file_hash(batch_path)}}
        gate.write_json(self.store.path / "browser-leaves" / (key + ".json"), self.store.seal(value))
        verified = {}
        with patch.object(gate, "file_hash", wraps=gate.file_hash) as observed:
            self.assertIsNotNone(browser.find_leaf(self.store, key, self.row, verified))
            count = observed.call_count
            self.assertIsNotNone(browser.find_leaf(self.store, key, self.row, verified))
            self.assertEqual(count, observed.call_count)
        log.write_text("changed between invocations")
        self.assertIsNone(browser.find_leaf(self.store, key, self.row, {}))

    def test_failed_batch_retains_only_complete_single_attempt_rows(self):
        expected = report([("one", "passed"), ("two", "passed"), ("three", "passed")])
        actual = report([("one", "passed"), ("two", "failed"), ("three", "skipped")])
        self.assertEqual([browser.sample_key(row) for row in browser.validate_partial_rows(expected, actual)], ["one"])
        actual["suites"][0]["specs"][0]["tests"][0]["results"].append({"status": "passed", "retry": 1})
        self.assertEqual(browser.validate_partial_rows(expected, actual), [])
        actual["suites"][0]["specs"].pop()
        with self.assertRaises(ValueError):
            browser.validate_partial_rows(expected, actual)

    def test_case_filter_reconciles_project_and_title_without_duplicate_or_missing_rows(self):
        expected = report([("one", "passed"), ("two", "passed")])
        selected = {browser.identity(self.row)}
        filtered = browser.filtered_discovery(expected, selected)
        self.assertEqual([browser.identity(row) for row in gate.browser_rows(filtered)], [browser.identity(self.row)])
        broken = copy.deepcopy(expected)
        broken["suites"][0]["specs"].append(copy.deepcopy(broken["suites"][0]["specs"][0]))
        with self.assertRaises(ValueError):
            browser.validate_partial_rows(expected, broken)


class BrowserInvocationTests(unittest.TestCase):
    """Exercise parent/child orchestration using bounded synthetic Playwright reports."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.write(".gitignore", "target/\nnode_modules/\n")
        self.write("src/program.rs", "program")
        self.store = gate.Store(self.root / "target/release-gate")
        self.manifest = self.store.path / "prepared/build/browser/harness.json"
        self.artifact_dir = self.manifest.parent / "geosolve-harness"
        self.write(str(self.artifact_dir.relative_to(self.root) / "index.html"), "prepared artifact")
        gate.write_json(self.manifest, {"format": "geosolve-release-artifact-v1", "kind": "harness",
                        "directory": str(self.artifact_dir), "filesSha256": "a" * 64})
        self.write(gate.FRONTEND + "/node_modules/playwright.js", "playwright")
        for key in ("one", "two"):
            self.write(browser.SAMPLE_ROOT + "/" + key + "/manifest.json", json.dumps({"key": key, "ordinal": 1, "title": key}))
            self.write(browser.SAMPLE_ROOT + "/" + key + "/sketch.ts", key)
        self.write(browser.CATALOG, json.dumps({"schema": 1, "samples": [{"key": "one"}, {"key": "two"}]}))
        self.catalog_case = ["release-catalog.spec.ts", "reviewed catalog", "chromium"]
        self.other_case = ["workbench.spec.ts", "reviewed workbench", "chromium"]
        self.write("scripts/release_test_inventory.json", json.dumps({"browser_non_sample_cases": [self.catalog_case, self.other_case]}))
        self.policy = {"global": ["scripts/release_equivalence.json"], "owned": ["src/**", "crates/**"], "prose": []}
        self.calls = []
        self.statuses = {}
        self.omit = set()
        self.side_effect = None
        self.counter = 0

    def tearDown(self):
        self.temp.cleanup()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def prepare_context(self, fresh=False, reviewed=True):
        import release_equivalence
        self.counter += 1
        run_id = "run-" + str(self.counter)
        run_dir = self.store.path / "runs" / run_id
        stage = gate.Stage("browser", ((sys.executable, "scripts/release_gate.py", "--run-browser", str(self.manifest),
                           "--jobs", "2", "--output", "{scratch}/browser"),), inputs=("src/**", "crates/**"),
                           env=(("GEOSOLVE_E2E_ARTIFACT_MANIFEST", str(self.manifest)),),
                           artifacts=(str(self.artifact_dir),))
        snapshot = gate.source_snapshot(self.root)
        inputs = gate.stage_inputs(stage, snapshot, self.policy)
        contract = {"schema": 1, "browser": {"program_sha256": gate.digest(release_equivalence.partition(inputs, self.policy))}}
        if not reviewed:
            contract["browser"]["program_sha256"] = "not reviewed"
        self.write(release_equivalence.CONTRACT_PATH, json.dumps(contract))
        snapshot = gate.source_snapshot(self.root)
        prep_path = run_dir / "preparation.json"
        prep_log = run_dir / "preparation.log"
        prep_log.parent.mkdir(parents=True)
        prep_log.write_text("actual preparation")
        prep = {"stage": "prepare.browser", "key": "prepared-key", "status": "passed", "complete": True,
                "evidence": {str(prep_log): gate.file_hash(prep_log)},
                "outputs": {str(self.manifest.parent): gate.hash_output(self.manifest.parent)}}
        gate.write_json(prep_path, self.store.seal(prep))
        runner = SimpleNamespace(root=self.root, store=self.store, run_id=run_id, run_dir=run_dir, fresh=fresh,
                  snapshot=snapshot, policy=self.policy, tools={}, environment={},
                  keys={"prepare.browser": "prepared-key", "browser": "browser-key"},
                  results={"prepare.browser": {"status": "passed", "key": "prepared-key", "receipt_path": str(prep_path)}},
                  artifact_hash=lambda name: gate.hash_output(self.root / name))
        env = dict(os.environ, GEOSOLVE_E2E_ARTIFACT_MANIFEST=str(self.manifest))
        output = run_dir / "stages/browser/scratch/browser"
        context = browser.parent_context(runner, stage, output, env)
        gate.write_json(context["context_path"], self.store.seal(context))
        env["GEOSOLVE_RELEASE_BROWSER_CONTEXT"] = context["context_path"]
        return context, output, env

    def fake_playwright(self, arguments, *, cwd, env, stdout, stderr):
        discovered = "--list" in arguments
        prefix = env.get("GEOSOLVE_BROWSER_PREFIX_ONLY") == "1"
        selected = re.compile(arguments[arguments.index("--grep") + 1]) if "--grep" in arguments else None
        identities = [["m92-sample-audit.spec.ts", browser.SAMPLE_PREFIX + key, "chromium"] for key in ("one", "two")]
        identities += [self.catalog_case, self.other_case]
        identities = [row for row in identities if row[1] not in self.omit and (selected is None or selected.search(row[1]))]
        self.calls.append({"discovery": discovered, "prefix": prefix, "cases": identities})
        result = {"errors": [], "suites": [{"specs": []}]}
        failed = False
        for file, title, project in identities:
            status = "passed" if prefix or discovered else self.statuses.get(title, "passed")
            failed |= status != "passed"
            result["suites"][0]["specs"].append({"file": file, "title": title, "tests": [{
                "projectName": project, "expectedStatus": "passed", "status": "expected" if status == "passed" else "unexpected",
                "results": [{"status": status, "retry": 0, "errors": []}]}]})
            if file == "m92-sample-audit.spec.ts" and not discovered:
                key = title[len(browser.SAMPLE_PREFIX):]
                gate.write_json(Path(env["GEOSOLVE_BROWSER_WITNESS_OUTPUT"]) / (key + ".json"), witness(key))
        stdout.write(json.dumps(result))
        if not discovered and not prefix and self.side_effect:
            self.side_effect()
        return SimpleNamespace(returncode=1 if failed else 0)

    def execute(self, fresh=False, reviewed=True):
        context, output, env = self.prepare_context(fresh, reviewed)
        with patch.dict(os.environ, env, clear=True), patch.object(browser, "invoke_playwright", self.fake_playwright):
            browser.run(self.root, self.store, self.manifest, output, 2, fresh, context)
        return gate.read_json(output / "coverage.json"), context, output

    def test_fresh_runs_full_workflows_even_with_authenticated_leaves(self):
        baseline, _, _ = self.execute()
        self.assertEqual(len(baseline["leaves"]), 2)
        reused, _, _ = self.execute()
        self.assertEqual(len(reused["reused_cases"]), 2)
        self.calls.clear()
        fresh, _, _ = self.execute(fresh=True)
        self.assertEqual(fresh["reused_cases"], [])
        full = [call for call in self.calls if not call["prefix"] and not call["discovery"]]
        self.assertEqual(len(full), 1)
        self.assertEqual(len(full[0]["cases"]), 3)

    def test_stale_program_guard_runs_every_leaf_without_donating(self):
        actual, _, _ = self.execute(reviewed=False)
        self.assertFalse(actual["reviewed_narrow_reuse"])
        self.assertEqual(len(actual["fresh_cases"]), 3)
        self.assertEqual(actual["leaves"], [])
        self.assertEqual(list((self.store.path / "browser-leaves").glob("*.json")), [])

    def test_missing_non_sample_discovery_fails_before_any_test_body(self):
        self.omit.add(self.other_case[1])
        with self.assertRaisesRegex(ValueError, "reviewed non-sample inventory"):
            self.execute()
        self.assertTrue(all(call["discovery"] for call in self.calls))

    def test_failed_full_batch_salvages_one_independent_leaf_and_resume_runs_missing(self):
        self.statuses[browser.SAMPLE_PREFIX + "two"] = "failed"
        with self.assertRaisesRegex(ValueError, "independent successful leaves retained"):
            self.execute()
        self.assertEqual(len(list((self.store.path / "browser-leaves").glob("*.json"))), 1)
        self.statuses.clear()
        actual, _, _ = self.execute()
        self.assertEqual([row[1] for row in actual["reused_cases"]], [browser.SAMPLE_PREFIX + "one"])
        self.assertEqual(len(actual["fresh_cases"]), 2)

    def test_source_or_artifact_mutation_rejects_all_donations(self):
        for change in (lambda: self.write("src/program.rs", "changed"),
                       lambda: (self.artifact_dir / "index.html").write_text("changed artifact")):
            self.side_effect = change
            with self.assertRaisesRegex(ValueError, "source differs|artifact differs"):
                self.execute()
            self.assertEqual(list((self.store.path / "browser-leaves").glob("*.json")), [])

    def test_signed_context_rejects_wrong_manifest_output_environment_and_fresh(self):
        context, output, env = self.prepare_context()
        other = self.manifest.with_name("other-harness.json")
        other.write_bytes(self.manifest.read_bytes())
        for manifest, destination, fresh, environment in (
            (other, output, False, env), (self.manifest, output.parent / "wrong", False, env),
            (self.manifest, output, True, env), (self.manifest, output, False, dict(env, NODE_OPTIONS="changed"))):
            with self.subTest(manifest=manifest, output=destination, fresh=fresh), patch.dict(os.environ, environment, clear=True):
                with self.assertRaises(ValueError):
                    browser.validate_context(self.root, self.store, manifest, destination, 2, fresh, context)

    def test_tampered_or_changed_profile_build_context_rejects(self):
        context, output, env = self.prepare_context()
        with patch.dict(os.environ, env, clear=True):
            changed = copy.deepcopy(context)
            changed["tools"] = {"rustc": "different profile"}
            with self.assertRaisesRegex(ValueError, "unauthenticated"):
                browser.validate_context(self.root, self.store, self.manifest, output, 2, False, changed)
            (self.artifact_dir / "index.html").write_text("another legitimate build")
            with self.assertRaisesRegex(ValueError, "artifact differs"):
                browser.validate_context(self.root, self.store, self.manifest, output, 2, False, context)


if __name__ == "__main__":
    unittest.main()
