# SPDX-License-Identifier: GPL-3.0-or-later
"""Regression coverage for compile inputs outside normal Cargo source ownership."""
import contextlib
import dataclasses
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate as gate


class EmbeddedInputTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.write(".gitignore", "target/\n")
        self.write("crates/example/Cargo.toml", '[package]\nname="example"\nversion="0.1.0"\n')
        self.policy = {"prose": ["docs/**"], "global": ["**/Cargo.toml"], "owned": ["crates/**", "packages/**"]}
        self.store = gate.Store(self.root / "target/store")

    def tearDown(self):
        self.temporary.cleanup()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def runner(self, stages, fresh=False):
        runner = gate.Runner(self.root, self.store, gate.source_snapshot(self.root), self.policy, {}, fresh=fresh)
        runner.prepare(stages)
        return runner

    def execute(self, runner, stages):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertTrue(runner.run(stages))
            self.assertTrue(runner.report(final=True)["complete"])

    def test_consumed_markdown_rebuilds_even_when_fresh_retains_build_receipts(self):
        self.write("docs/input.md", "first")
        self.write("crates/example/src/main.rs", 'fn main() { print!("{}", include_str!("../../../docs/input.md")); }')
        stage = gate.Stage("build.example", (("rustc", "crates/example/src/main.rs", "-o", "target/example"),),
                           inputs=gate.crate_inputs(self.root, "example"), outputs=("target/example",), kind="build")
        self.execute(self.runner([stage]), [stage])
        self.assertEqual(subprocess.check_output([self.root / "target/example"], text=True), "first")
        self.assertEqual(self.runner([stage], fresh=True).decision(stage)[0], "reuse")
        self.write("docs/input.md", "second")
        changed = self.runner([stage], fresh=True)
        self.assertEqual(changed.decision(stage)[0], "run")
        self.execute(changed, [stage])
        self.assertEqual(subprocess.check_output([self.root / "target/example"], text=True), "second")

    def test_workspace_preparation_identity_tracks_real_m33_consumed_markdown(self):
        real_root = Path(__file__).resolve().parents[2]
        self.assertIn("docs/M33_CAD_CAPABILITY_MATRIX.md", gate.crate_inputs(real_root, "geosolve-sketch"))
        self.write("docs/input.md", "first")
        self.write("crates/example/tests/capability.rs", 'const INPUT: &str = include_str!("../../../docs/input.md");')
        first = self.runner([])
        a, _ = gate.preparation_stages(first, {"workspace"})
        self.write("docs/input.md", "second")
        second = self.runner([])
        b, _ = gate.preparation_stages(second, {"workspace"})
        self.assertNotEqual(first.key(a[0]), second.key(b[0]))
        self.assertNotEqual(a[0].outputs, b[0].outputs)
        consumer = gate.Stage("test.example", (("true",),), inputs=gate.crate_inputs(self.root, "example"))
        self.assertNotEqual(first.key(consumer), second.key(consumer))

    def test_unconsumed_prose_keeps_preparation_identity(self):
        self.write("crates/example/src/lib.rs", "pub fn example() {}")
        self.write("docs/signoff.md", "first")
        first = self.runner([])
        a, _ = gate.preparation_stages(first, {"workspace"})
        self.write("docs/signoff.md", "second")
        second = self.runner([])
        b, _ = gate.preparation_stages(second, {"workspace"})
        self.assertEqual(first.key(a[0]), second.key(b[0]))

    def test_cross_package_fixture_manifest_concat_and_missing_inputs_are_declared(self):
        self.write("crates/example/src/lib.rs", '''
const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/fixtures/", "compiled.json"
));
const REMOVED: &[u8] = include_bytes!("../../../docs/missing.md");
''')
        inputs = gate.crate_inputs(self.root, "example")
        self.assertIn("packages/fixtures/compiled.json", inputs)
        self.assertIn("docs/missing.md", inputs)
        selected = gate.stage_inputs(gate.Stage("consumer", (("true",),), inputs=inputs), {}, self.policy)
        self.assertIsNone(selected["docs/missing.md"])

    def test_dependency_test_only_document_is_not_a_production_input(self):
        self.write("crates/example/Cargo.toml", '[package]\nname="example"\nversion="0.1.0"\n[dependencies]\ndep={path="../dep"}\n')
        self.write("crates/dep/Cargo.toml", '[package]\nname="dep"\nversion="0.1.0"\n')
        self.write("crates/dep/src/lib.rs", 'const INPUT: &str = include_str!("../../../docs/production.md");')
        self.write("crates/dep/tests/capability.rs", 'const INPUT: &str = include_str!("../../../docs/tests.md");')
        inputs = gate.crate_inputs(self.root, "example")
        self.assertIn("docs/production.md", inputs)
        self.assertNotIn("docs/tests.md", inputs)

    def test_unknown_include_form_invalidates_prose_and_rejects_docs_only(self):
        self.write("crates/example/src/lib.rs", 'const INPUT: &str = include_str!(env!("OTHER_INPUT"));')
        self.write("docs/input.md", "first")
        inputs = gate.crate_inputs(self.root, "example")
        self.assertIn("**", inputs)
        subprocess.run(["git", "add", "."], cwd=self.root, check=True)
        subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                        "commit", "-qm", "fixture"], cwd=self.root, check=True)
        self.write("docs/input.md", "second")
        with self.assertRaisesRegex(ValueError, "unresolved Rust input"):
            gate.documentation_delta(self.root, "HEAD", self.policy)

    def test_new_plain_include_with_nested_document_uses_all_source_fallback(self):
        self.write("crates/example/src/lib.rs", 'include!("../../../packages/fixtures/external.rs");')
        self.write("packages/fixtures/external.rs", 'const INPUT: &str = include_str!("../../docs/input.md");')
        self.assertIn("**", gate.crate_inputs(self.root, "example"))
        selected = gate.stage_inputs(gate.Stage("consumer", (("true",),), inputs=gate.crate_inputs(self.root, "example")),
                                     {"docs/input.md": {"sha256": "changed"}}, self.policy)
        self.assertIn("docs/input.md", selected)

    def test_multiline_manifest_include_rejects_documentation_only(self):
        self.write("crates/example/src/lib.rs", '\n'.join([
            'const INPUT: &str = include_str!(concat!(',
            'env!("CARGO_MANIFEST_DIR"),',
            '"/../../docs/input.md"));']))
        self.write("docs/input.md", "first")
        subprocess.run(["git", "add", "."], cwd=self.root, check=True)
        subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                        "commit", "-qm", "fixture"], cwd=self.root, check=True)
        self.write("docs/input.md", "second")
        with self.assertRaisesRegex(ValueError, "declared or unresolved Rust input"):
            gate.documentation_delta(self.root, "HEAD", self.policy)

    def test_frontend_preflight_tracks_all_read_rust_registries(self):
        policy = gate.read_json(Path(__file__).resolve().parents[2] / gate.POLICY_PATH)
        stage = next(stage for stage in gate.preflight_stages() if stage.id == "preflight.frontend")
        for name in ("crates/geosolve-constraint-editor/src/geometry_tools.rs",
                     "crates/geosolve-demo-web/src/workbench/geometry_palette.rs",
                     "crates/geosolve-demo-web/src/workbench/action_surface.rs",
                     "crates/geosolve-demo-web/src/workbench/command_manifest.rs"):
            with self.subTest(input=name):
                self.assertIn(name, gate.stage_inputs(stage, {name: {"sha256": "changed"}}, policy))

    def test_external_path_module_and_nested_fixture_join_the_owner_closure(self):
        self.write("crates/example/tests/owner.rs", '#[path = "../../../packages/helpers/fixture.rs"] mod fixture;')
        self.write("packages/helpers/fixture.rs", 'const FIXTURE: &str = include_str!("../data/fixture.json");')
        self.write("packages/data/fixture.json", "actual fixture")
        inputs = gate.crate_inputs(self.root, "example")
        self.assertIn("packages/helpers/fixture.rs", inputs)
        self.assertIn("packages/data/fixture.json", inputs)
        self.assertNotIn("**", inputs)

    def test_real_retained_headless_fixture_follows_cross_package_module(self):
        root = Path(__file__).resolve().parents[2]
        inputs = gate.crate_inputs(root, "geosolve-headless")
        self.assertIn("crates/geosolve-sketch-code/tests/support/retained_sample.rs", inputs)
        self.assertIn("crates/geosolve-sketch-code/tests/fixtures/retained-bondtech-indx-link/sketch.compiled.json", inputs)

    def test_embedded_input_outside_repository_fails_closed(self):
        self.write("crates/example/src/lib.rs", 'const INPUT: &str = include_str!("../../../../external.md");')
        with self.assertRaisesRegex(ValueError, "escapes repository"):
            gate.crate_inputs(self.root, "example")


class InputCacheTests(unittest.TestCase):
    """Memoization preserves reviewed input semantics without running build tools."""
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.policy = {"prose": ["docs/**"], "global": ["Cargo.lock"], "owned": ["crates/**", "packages/**"]}
        self.snapshot = {name: {"sha256": name, "executable": False} for name in (
            "Cargo.lock", "crates/example/src/lib.rs", "packages/helper.ts", "docs/note.md", "unknown-input")}

    def runner(self, *, snapshot=None, policy=None, root=None):
        root = self.root if root is None else root
        with patch.object(gate, "capture", return_value="fixture-revision"):
            return gate.Runner(root, gate.Store(root / "target/store", create=False),
                               self.snapshot if snapshot is None else snapshot,
                               self.policy if policy is None else policy, {})

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def reviewed_stage(self):
        import release_equivalence as equivalence
        sample = equivalence.SAMPLE_ROOT + "/fixture/sketch.compiled.json"
        self.snapshot[sample] = {"sha256": "catalog-v1", "executable": False}
        stage = gate.Stage("reviewed", (("true",),), inputs=("crates/**",),
                           excluded_inputs=(equivalence.SAMPLE_ROOT + "/**",), input_equivalence="fixture")
        full = gate.stage_inputs(stage, self.snapshot, self.policy)
        contract = {"schema": 1, "fixture": {"program_sha256": gate.digest(equivalence.partition(full, self.policy))}}
        return dataclasses.replace(stage, input_equivalence_contract=contract), sample

    def test_shared_boundaries_scan_once_and_distinct_boundaries_keep_their_inputs(self):
        native = gate.Stage("native.a", (("true",),), inputs=("crates/**", "docs/missing.md"))
        sibling = dataclasses.replace(native, id="native.b", cases=("different-case",))
        package = gate.Stage("package", (("true",),), inputs=("packages/**",))
        runner = self.runner()
        with patch.object(gate, "stage_inputs", wraps=gate.stage_inputs) as scan:
            runner.prepare([native, sibling, package])
            self.assertEqual(scan.call_count, 2)
            self.assertEqual(set(runner.inputs(native)),
                             {"Cargo.lock", "unknown-input", "crates/example/src/lib.rs", "docs/missing.md"})
            self.assertIsNone(runner.inputs(native)["docs/missing.md"])
            self.assertEqual(set(runner.inputs(package)), {"Cargo.lock", "unknown-input", "packages/helper.ts"})
            self.assertNotEqual(runner.key(native), runner.key(sibling))
            self.assertEqual(scan.call_count, 2)

    def test_exclusions_scope_and_contract_never_share_an_unreviewed_result(self):
        reviewed, sample = self.reviewed_stage()
        full = dataclasses.replace(reviewed, excluded_inputs=())
        no_contract = dataclasses.replace(reviewed, input_equivalence_contract=None)
        wrong_contract = dataclasses.replace(reviewed, input_equivalence_contract={"schema": 1, "fixture": {"program_sha256": "stale"}})
        other_scope = dataclasses.replace(reviewed, input_equivalence="other")
        unresolved = dataclasses.replace(reviewed, inputs=("**",))
        runner = self.runner()
        self.assertNotIn(sample, runner.inputs(reviewed))
        for stage in (full, no_contract, wrong_contract, other_scope, unresolved):
            with self.subTest(stage=stage):
                self.assertIn(sample, runner.inputs(stage))
        # A mutable contract value is fingerprinted by contents on every lookup.
        reviewed.input_equivalence_contract["fixture"]["program_sha256"] = "changed-review"
        self.assertIn(sample, runner.inputs(reviewed))

    def test_prepare_recomputes_cached_boundaries_after_snapshot_and_policy_reset(self):
        stage = gate.Stage("native", (("true",),), inputs=("crates/**",))
        sibling = dataclasses.replace(stage, id="sibling")
        runner = self.runner()
        with patch.object(gate, "stage_inputs", wraps=gate.stage_inputs) as scan:
            runner.prepare([stage, sibling])
            before = runner.keys[stage.id]
            self.assertEqual(scan.call_count, 1)
            runner.snapshot = self.snapshot | {"crates/example/src/lib.rs": {"sha256": "changed", "executable": False}}
            runner.policy = self.policy | {"global": ["Cargo.lock", "docs/**"]}
            runner.prepare([stage, sibling])
            self.assertEqual(scan.call_count, 2)
            self.assertNotEqual(runner.keys[stage.id], before)
            self.assertEqual(runner.inputs(stage)["crates/example/src/lib.rs"]["sha256"], "changed")
            self.assertIn("docs/note.md", runner.inputs(stage))

    def test_new_runner_source_policy_and_root_are_isolated(self):
        stage = gate.Stage("native", (("true",),), inputs=("crates/**",))
        original = self.runner()
        original_key = original.key(stage)
        changed_source = self.snapshot | {"crates/example/src/lib.rs": {"sha256": "changed", "executable": False}}
        source_runner = self.runner(snapshot=changed_source)
        policy_runner = self.runner(policy=self.policy | {"global": ["Cargo.lock", "packages/**"]})
        root_runner = self.runner(root=self.root / "another-worktree")
        self.assertEqual(source_runner.inputs(stage)["crates/example/src/lib.rs"]["sha256"], "changed")
        self.assertIn("packages/helper.ts", policy_runner.inputs(stage))
        self.assertNotIn("packages/helper.ts", original.inputs(stage))
        for changed in (source_runner, policy_runner, root_runner):
            self.assertNotEqual(original_key, changed.key(stage))
        self.assertEqual(original.key(stage), original_key)

    def test_cached_catalog_equivalence_survives_data_only_changes_and_rejects_program_changes(self):
        stage, sample = self.reviewed_stage()
        original = self.runner()
        original_key = original.key(stage)
        data = self.snapshot | {sample: {"sha256": "catalog-v2", "executable": False}}
        data_runner = self.runner(snapshot=data)
        self.assertEqual(original_key, data_runner.key(stage))
        changed = data | {"crates/example/src/lib.rs": {"sha256": "new-reader", "executable": False}}
        changed_runner = self.runner(snapshot=changed)
        self.assertIn(sample, changed_runner.inputs(stage))
        self.assertNotEqual(original_key, changed_runner.key(stage))
        for value in ({"sha256": "catalog-v2", "executable": True}, {"link": "other", "target": "catalog-v2"}):
            guarded = self.runner(snapshot=data | {sample: value})
            self.assertIn(sample, guarded.inputs(stage))
            self.assertNotEqual(original_key, guarded.key(stage))

    def test_native_package_closures_scan_once_per_call_and_keep_embedded_dependency_inputs(self):
        import release_gate_native as native
        self.write("crates/example/Cargo.toml", '[package]\nname="example"\nversion="0.1.0"\n[dependencies]\ndep={path="../dep"}\n')
        self.write("crates/dep/Cargo.toml", '[package]\nname="dep"\nversion="0.1.0"\n')
        self.write("crates/geosolve-headless/Cargo.toml", '[package]\nname="geosolve-headless"\nversion="0.1.0"\n[dependencies]\nexample={path="../example"}\n')
        self.write("crates/example/tests/case.rs", '#[path="../../../packages/fixture.rs"] mod fixture;')
        self.write("packages/fixture.rs", 'const INPUT: &str = include_str!("../docs/case.md");')
        self.write("crates/dep/src/lib.rs", 'const INPUT: &str = include_str!("../../../docs/production.md");')
        self.write("crates/dep/tests/case.rs", 'const INPUT: &str = include_str!("../../../docs/dependency-test.md");')
        expected = {package: gate.crate_inputs(self.root, package) for package in ("example", "geosolve-headless")}
        self.assertIn("packages/fixture.rs", expected["example"])
        self.assertIn("docs/case.md", expected["example"])
        self.assertIn("docs/production.md", expected["example"])
        self.assertNotIn("docs/dependency-test.md", expected["example"])

        def description(package, name):
            return dict(id=f"{package}::{name}::normal", command=["/protected/test"], env={}, features=[], profile={},
                        resource="native", selected=[name], build_overlap_safe=False, runtime_closures=[])

        workspace = [description("example", "a"), description("example", "b"), description("geosolve-headless", "normal")]
        ignored = [description("geosolve-headless", "ignored")]
        managed = ("packages/geosolve-sketch-code/src/**", "packages/geosolve-sketch-code/scripts/compile*",
                   "packages/geosolve-sketch-code/scripts/mutate*", "packages/geosolve-intent/src/**")
        with patch.object(gate, "read_json", return_value=self.policy), \
                patch.object(gate, "rust_inputs", return_value=("crates/**",)), \
                patch.object(gate, "source_snapshot", return_value=self.snapshot), \
                patch.object(native, "workspace_stages", return_value=workspace), \
                patch.object(native, "ignored_headless_stages", return_value=ignored), \
                patch.object(gate, "crate_inputs", wraps=gate.crate_inputs) as scan:
            stages = gate.native_stages(self.root, {"workspace": "target/workspace", "headless": "target/headless"})
            self.assertEqual([call.args[1] for call in scan.call_args_list], ["example", "geosolve-headless"])
            for stage in stages:
                package = stage.id.split(".", 1)[1].split("::")[0]
                suffix = managed if package == "geosolve-headless" else ()
                self.assertEqual(stage.inputs, (*expected[package], *suffix, "scripts/release_gate_native.py"))
            self.write("packages/fixture.rs", 'const INPUT: &str = include_str!("../docs/new-case.md");')
            refreshed = gate.native_stages(self.root, {"workspace": "target/workspace", "headless": "target/headless"})
            self.assertEqual(scan.call_count, 4)
            self.assertIn("docs/new-case.md", refreshed[0].inputs)
            self.assertNotIn("docs/case.md", refreshed[0].inputs)


if __name__ == "__main__":
    unittest.main()
