# SPDX-License-Identifier: GPL-3.0-or-later
"""Regression coverage for compile inputs outside normal Cargo source ownership."""
import contextlib
import dataclasses
import io
import json
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
                     "crates/geosolve-constraint-editor/src/authoring_catalog.rs",
                     "crates/geosolve-constraint-editor/examples/generate_tool_catalog.rs",
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


class CountRepairReceiptTests(unittest.TestCase):
    """Actual input factories and signed Python-child receipts; no product build."""
    checker = gate.FRONTEND + "/scripts/check-sample-manifest.mjs"

    def setUp(self):
        import release_equivalence as equivalence
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.write(".gitignore", "target/\n**/dist/\n**/node_modules/\n")
        self.policy = gate.read_json(gate.ROOT / gate.POLICY_PATH)
        self.write(gate.POLICY_PATH, json.dumps(self.policy))
        self.write("scripts/release_test_inventory.json", (gate.ROOT / "scripts/release_test_inventory.json").read_text())
        for manifest in (gate.ROOT / "crates").glob("*/Cargo.toml"):
            package = manifest.parent.name
            self.write(f"crates/{package}/Cargo.toml", f'[package]\nname="{package}"\nversion="0.1.0"\n')
            self.write(f"crates/{package}/src/lib.rs", "// fixture semantic source\n")
        self.write(self.checker, "stale count assertion\n")
        self.probe = self.root / "target/native-probe"
        self.write("target/native-probe", f"#!{sys.executable}\n" + '''
import json, sys, time
from pathlib import Path
name = sys.argv[1] if len(sys.argv) > 1 else 'native'
active = Path('target/builder-active')
if name == 'native':
    assert not active.exists(), 'native overlapped a locked builder'
if name == 'builder':
    active.write_text('active')
start = time.monotonic()
time.sleep(.15)
if name == 'builder':
    active.unlink()
path = Path('target/observed-' + name + '.json')
previous = json.loads(path.read_text()) if path.exists() else []
path.write_text(json.dumps([*previous, [start, time.monotonic()]]))
print('fixture child passed')
''')
        self.probe.chmod(0o755)
        self.description = dict(id="geosolve-core::fixture::normal", command=[str(self.probe)], env={},
                                features=["fixture"], profile={"test": True, "opt_level": "1"}, resource="native",
                                selected=["fixture"], build_overlap_safe=True, runtime_closures=[{"fixture": "runtime-v1"}])
        self.write("target/prepared/prepared.json", "{}")
        for package in ("geosolve-intent", "geosolve-sketch-code"):
            for output in ("dist", "node_modules"):
                self.write(f"packages/{package}/{output}/fixture", "unchanged managed artifact")
        patterns = (*gate.rust_inputs(self.root), "packages/**", gate.FRONTEND + "/**",
                    "scripts/verify-geosolve-sketch-code-package.sh", "scripts/golden*")
        selected = gate.stage_inputs(gate.Stage("review", (("identity",),), inputs=patterns),
                                     gate.source_snapshot(self.root), self.policy)
        contract = {"schema": 1, "native_build_overlap": {"program_sha256": gate.digest(equivalence.partition(selected, self.policy))}}
        self.write(equivalence.CONTRACT_PATH, json.dumps(contract))
        self.store = gate.Store(self.root / "target/store")

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def runner(self, stages, *, fresh=False):
        runner = gate.Runner(self.root, self.store, gate.source_snapshot(self.root), self.policy, {}, jobs=2, fresh=fresh)
        runner.prepare(stages)
        return runner

    def execute(self, runner, stages):
        with contextlib.redirect_stdout(io.StringIO()):
            success = runner.run(stages)
            report = runner.report(final=True)
        return success, report

    def native(self):
        import release_gate_native as native
        with patch.object(native, "workspace_stages", return_value=[self.description]):
            stage, = gate.native_stages(self.root, {"workspace": "target/prepared"})
        # The real adapter factory supplies mapping, runtime/coverage identity
        # and admission. A small actual child substitutes for product execution.
        return dataclasses.replace(stage, commands=(tuple(self.description["command"]),), dependencies=())

    def stages(self):
        native = self.native()
        inventory = {stage.id: stage for stage in gate.qualification_stages(self.root, {}, 2)}
        peers = [dataclasses.replace(inventory[name], commands=((str(self.probe), name),),
                                     dependencies=(), cases=("fixture",), test_inventories=())
                 for name in ("golden", "wasm.m70_transition_parity")]
        with patch.object(gate, "ROOT", self.root):
            catalog = next(stage for stage in gate.preflight_stages() if stage.id == "preflight.catalog")
        check = f"from pathlib import Path; assert Path({self.checker!r}).read_text() == 'correct count assertion\\n'"
        catalog = dataclasses.replace(catalog, commands=((sys.executable, "-c", check),),
                                      dependencies=tuple(stage.id for stage in (native, *peers)))
        return [native, *peers, catalog]

    def test_checker_repair_reuses_native_golden_and_wasm_from_finalized_failed_run(self):
        stages = self.stages()
        self.assertFalse(stages[0].build_lock)
        first = self.runner(stages)
        success, report = self.execute(first, stages)
        self.assertFalse(success)
        self.assertEqual(report["status"], "failed")
        self.assertEqual(first.results[stages[-1].id]["status"], "failed")
        first_path = first.run_dir / "qualification.json"
        failed_bytes = first_path.read_bytes()
        for stage in stages[:-1]:
            self.assertIsNotNone(self.store.find(stage.id, first.keys[stage.id]))
            self.assertNotIn(self.checker, first.inputs(stage))
        self.assertIsNone(self.store.find(stages[-1].id, first.keys[stages[-1].id]))
        self.assertIn(self.checker, first.inputs(stages[-1]))

        self.write(self.checker, "correct count assertion\n")
        repaired = self.stages()
        self.assertTrue(repaired[0].build_lock)  # Review remains invalid; no repin.
        second = self.runner(repaired)
        for stage in repaired[:-1]:
            self.assertEqual(second.keys[stage.id], first.keys[stage.id])
            self.assertEqual(second.decision(stage)[0], "reuse")
        self.assertEqual(second.decision(repaired[-1])[0], "run")
        success, report = self.execute(second, repaired)
        self.assertTrue(success)
        self.assertTrue(report["complete"])
        for stage in repaired[:-1]:
            self.assertEqual(second.results[stage.id]["origin_run"], first.run_id)
        self.assertTrue(next(stage for stage in report["inventory"] if stage["id"] == repaired[0].id)["build_lock"])
        self.assertFalse(self.store.unseal(Path(second.results[repaired[0].id]["receipt_path"]))["contract"]["build_lock"])
        for name in ("native", "golden", "wasm.m70_transition_parity"):
            self.assertEqual(len(json.loads((self.root / f"target/observed-{name}.json").read_text())), 1)
        self.assertEqual(first_path.read_bytes(), failed_bytes)

    def test_checker_fallback_still_locks_fresh_execution_and_records_actual_contract(self):
        self.write(self.checker, "correct count assertion\n")
        native = self.native()
        self.assertTrue(native.build_lock)
        first = self.runner([native])
        self.assertTrue(self.execute(first, [native])[0])
        builder = gate.Stage("builder", ((str(self.probe), "builder"),), build_lock=True, kind="build")
        stages = [builder, native]
        second = self.runner(stages, fresh=True)
        self.assertEqual(second.decision(native)[0], "run")
        self.assertTrue(self.execute(second, stages)[0])
        native_intervals = json.loads((self.root / "target/observed-native.json").read_text())
        builder_interval, = json.loads((self.root / "target/observed-builder.json").read_text())
        self.assertEqual(len(native_intervals), 2)
        self.assertGreaterEqual(native_intervals[-1][0], builder_interval[1])
        receipt = self.store.unseal(Path(second.results[native.id]["receipt_path"]))
        self.assertTrue(receipt["contract"]["build_lock"])

    def test_real_execution_inputs_still_invalidate_native_receipts(self):
        native = self.native()
        first = self.runner([native])
        self.assertTrue(self.execute(first, [native])[0])
        self.write(self.checker, "correct count assertion\n")
        native = self.native()
        runner = self.runner([native])
        self.assertEqual(runner.decision(native)[0], "reuse")
        variants = [dataclasses.replace(native, cases=("changed-case",)),
                    dataclasses.replace(native, resource="memory"),
                    dataclasses.replace(native, env=(("FIXTURE_MODE", "changed"),)),
                    dataclasses.replace(native, timeout=60)]
        for key, value in (("command", [str(self.probe), "changed-argument"]), ("env", {"FIXTURE_MODE": "changed"}),
                           ("features", ["changed"]), ("profile", {"test": True, "opt_level": "2"}),
                           ("runtime_closures", [{"fixture": "runtime-v2"}]), ("build_overlap_safe", False)):
            variants.append(dataclasses.replace(native, execution_key=native.execution_key | {key: value}))
        for changed in variants:
            with self.subTest(contract=changed.contract()):
                self.assertEqual(self.runner([changed]).decision(changed)[0], "run")
        original_probe = self.probe.read_text()
        self.probe.write_text(original_probe + "# different actual executable bytes\n")
        self.assertEqual(self.runner([native]).decision(native)[0], "run")
        self.probe.write_text(original_probe)
        self.assertEqual(self.runner([native]).decision(native)[0], "reuse")
        self.write("crates/geosolve-core/src/lib.rs", "// changed semantic source\n")
        self.assertEqual(self.runner([native]).decision(native)[0], "run")

    def test_only_explicit_native_kind_normalizes_build_admission(self):
        native = self.native()
        runner = self.runner([])
        self.assertEqual(native.kind, "native")
        for kind, name in (("test", "ordinary"), ("test", "performance"), ("build", "builder")):
            ordinary = dataclasses.replace(native, kind=kind, id=name)
            self.assertNotEqual(runner.key(ordinary), runner.key(dataclasses.replace(ordinary, build_lock=True)))
        self.assertNotEqual(runner.key(native), runner.key(dataclasses.replace(native, kind="test")))
        self.assertEqual(runner.key(native), runner.key(dataclasses.replace(native, build_lock=True)))


if __name__ == "__main__":
    unittest.main()
