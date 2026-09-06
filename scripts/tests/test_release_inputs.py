# SPDX-License-Identifier: GPL-3.0-or-later
"""Regression coverage for compile inputs outside normal Cargo source ownership."""
import contextlib
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

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


if __name__ == "__main__":
    unittest.main()
