# SPDX-License-Identifier: GPL-3.0-or-later
"""M98 obligations, captured dependencies and honest Node coverage admission."""
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
import release_gate_m98 as m98


class M98ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        (self.root / '.gitignore').write_text('target/\n')
        self.policy = {'prose': [], 'global': [], 'owned': ['**']}
        for package in ('geosolve-demo-web', 'geosolve-sketch-engine-wasm', 'geosolve-collaboration', 'geosolve-collaboration-wasm'):
            self.write(f'crates/{package}/Cargo.toml', f'[package]\nname="{package}"\nversion="0.1.0"\n')

    def tearDown(self):
        self.temp.cleanup()

    def write(self, name, contents='// source\n'):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents)
        return path

    def runner(self):
        return gate.Runner(self.root, gate.Store(self.root / 'target/store'),
                           gate.source_snapshot(self.root), self.policy, {})

    def fixtures(self, base='target/capture/repository'):
        for name in m98.REQUIRED['engine.node']:
            self.write(f'{base}/packages/geosolve-engine/test/{name}.test.mjs')
        for name in m98.REQUIRED['folder.node']:
            self.write(f'{base}/scripts/{name}.test.mjs')
        for name in m98.REQUIRED['collaboration.node']:
            self.write(f'{base}/scripts/{name}.test.mjs')
        for name in m98.REQUIRED['collaboration.package']:
            self.write(f'{base}/packages/geosolve-collaboration/test/{name}.test.mjs')
        for name in m98.REQUIRED['collaboration.frontend']:
            self.write(f'{base}/{m98.FRONTEND}/src/lib/{name}.test.ts')
        for name in ('scripts/file-workspace.test.mjs','scripts/workspace-browser.test.mjs','scripts/package-m98.test.mjs',
                     'examples/generator-website/scripts/generator.test.mjs',
                     'examples/generator-website/scripts/browser.test.mjs', 'scripts/collaboration-browser.test.mjs'):
            self.write(f'{base}/{name}')
        return self.root / base

    def captured_runtime(self, test_body='', empty_file=False):
        self.fixtures(base='.')
        self.write('.gitignore', 'target/\n**/dist/\n**/node_modules/\n' + m98.FRONTEND + '/src/generated/\n')
        for name in m98.REQUIRED['engine.node']:
            body = test_body if name == 'engine' else ''
            self.write(f'packages/geosolve-engine/test/{name}.test.mjs',
                       'import test from "node:test"; import assert from "node:assert/strict";\n'
                       'import {mkdirSync,writeFileSync} from "node:fs";\n'
                       f'test({json.dumps(name)},()=>{{{body}}});\n')
        if empty_file:
            self.write('packages/geosolve-engine/test/engine.test.mjs', '// no declared tests\n')
        for name in m98.GENERATED:
            self.write(name + '/runtime.js', '// captured generated runtime\n')
        for name in m98.DEPENDENCIES:
            self.write(name + '/installed.js', '// installed dependency\n')
        self.write('target/demo-wasm/bindings.js')
        self.write('target/m98/workspace-runtime.mjs')
        fixture = self.write('target/custom-cargo-output/examples/text_fixture', '#!/bin/sh\n')
        captured = self.root / 'target/prepared'
        # Build commands are a separate Cargo/frontend preparation obligation;
        # exercise the real capture and Node consumer without compiling products.
        real_run = subprocess.run
        builds = []
        def only_build(command, **kwargs):
            if command[0] in ('node', 'cargo'):
                builds.append(command)
                if command[0] == 'cargo':
                    output = json.dumps({'reason': 'compiler-artifact', 'target': {'name': 'text_fixture', 'kind': ['example']}, 'executable': str(fixture)})
                    return subprocess.CompletedProcess(command, 0, stdout=output)
                return subprocess.CompletedProcess(command, 0)
            return real_run(command, **kwargs)
        with patch.object(m98.subprocess, 'run', side_effect=only_build):
            m98.prepare(self.root, self.root / 'target/demo-wasm', captured)
        self.assertEqual(len(builds), 6)
        self.assertIn(['cargo', 'build', '--locked', '-p', 'geosolve-collaboration', '--example', 'text_fixture', '--message-format=json'], builds)
        self.assertEqual((captured / 'repository' / m98.NATIVE_FIXTURE).read_bytes(), fixture.read_bytes())
        return captured

    def test_native_fixture_capture_requires_one_real_reported_artifact(self):
        fixture = self.write('custom-target/examples/text_fixture')
        row = {'reason': 'compiler-artifact', 'target': {'name': 'text_fixture', 'kind': ['example']}, 'executable': str(fixture)}
        valid = json.dumps(row)
        self.assertEqual(m98.fixture_executable(valid), fixture)
        for invalid in ('', valid + '\n' + valid,
                        json.dumps(row | {'target': {'name': 'other', 'kind': ['example']}}),
                        json.dumps(row | {'executable': 'relative/fixture'}),
                        json.dumps(row | {'executable': str(fixture.parent / 'missing')})):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                m98.fixture_executable(invalid)

    def test_modes_include_engine_workbench_and_browser_without_omitting_existing_wasm(self):
        self.assertEqual(gate.preparation_modes(['engine.node']), {'wasm','m98'})
        self.assertEqual(gate.preparation_modes(['folder.node']), {'wasm','m98'})
        self.assertEqual(gate.preparation_modes(['folder.browser']), {'wasm','browser','m98'})
        self.assertEqual(gate.preparation_modes(['package.*']), {'wasm','browser','m98'})
        self.assertIn('m98', gate.preparation_modes([]))
        self.assertIn('lifecycle', gate.preparation_modes(['wasm.*']))
        self.assertEqual(gate.preparation_modes(['collaboration.package']), {'wasm','m98'})
        self.assertEqual(gate.preparation_modes(['collaboration.node']), {'wasm','m98'})
        self.assertEqual(gate.preparation_modes(['collaboration.frontend']), {'wasm','m98'})
        self.assertEqual(gate.preparation_modes(['collaboration.browser']), {'wasm','browser','m98'})

    def test_preparation_binds_compiler_actor_and_native_sources_and_installed_bytes(self):
        for name in ('crates/geosolve-sketch-engine-wasm/Cargo.toml',
                     'crates/geosolve-demo-web/Cargo.toml'):
            self.write(name, '[package]\nname="example"\nversion="0.1.0"\n')
        worker = self.write('scripts/workspace-workbench-worker.mjs')
        self.write('packages/geosolve-sketch-code/dist/src/managed.js')
        first = self.runner()
        stages, paths = gate.preparation_stages(first, {'wasm','m98'})
        by_id = {stage.id: stage for stage in stages}
        stage = by_id['prepare.m98']
        self.assertEqual(stage.dependencies, ('preflight.clippy','prepare.wasm'))
        self.assertTrue(stage.build_lock)
        self.assertIn(paths['wasm'], stage.artifacts)
        for dep in (*m98.DEPENDENCIES, *m98.GENERATED[:2]):
            self.assertIn(dep, stage.artifacts)
        old_key = first.key(stage)
        worker.write_text('// changed actor semantics\n')
        changed = self.runner()
        new_stages, new_paths = gate.preparation_stages(changed, {'wasm','m98'})
        self.assertNotEqual(paths['m98'], new_paths['m98'])
        self.assertNotEqual(old_key, changed.key(next(s for s in new_stages if s.id == 'prepare.m98')))

    def test_all_current_test_files_are_obligations_and_missing_required_family_rejects(self):
        repo = self.fixtures()
        extra = repo / 'scripts/workspace-extra.test.mjs'
        extra.write_text('// newly introduced case\n')
        self.assertIn('scripts/workspace-extra.test.mjs', m98.inventory(repo, 'folder.node'))
        self.assertNotIn('scripts/workspace-browser.test.mjs', m98.inventory(repo, 'folder.node'))
        self.assertIn('scripts/workspace-browser.test.mjs', m98.inventory(repo, 'folder.browser'))
        self.assertNotIn('scripts/file-workspace.test.mjs', m98.inventory(repo, 'folder.browser'))
        (repo / 'scripts/workspace-storage.test.mjs').unlink()
        with self.assertRaisesRegex(ValueError, 'incomplete'):
            m98.inventory(repo, 'folder.node')

    def test_stage_group_carries_exact_file_inventory_runtime_bytes_and_browser_dependencies(self):
        repo = self.fixtures()
        metadata = {'schema':'geosolve-m98-runtime-v1','tests':{name:m98.inventory(repo,name) for name in m98.GROUPS}}
        self.write('target/capture/prepared.json', json.dumps(metadata))
        stages = gate.m98_stages(self.root, {'m98':'target/capture','browser':'target/browser'})
        self.assertEqual({stage.id for stage in stages}, set(m98.GROUPS))
        for stage in stages:
            self.assertTrue(stage.build_lock)
            self.assertEqual(list(stage.cases), metadata['tests'][stage.id])
            self.assertIn(str(self.root/'target/capture'),stage.artifacts)
            self.assertIn('prepare.m98',stage.dependencies)
        browser = next(s for s in stages if s.id == 'folder.browser')
        self.assertIn('prepare.browser', browser.dependencies)
        self.assertIn('--browser-package', browser.commands[0])
        collaboration_browser = next(s for s in stages if s.id == 'collaboration.browser')
        self.assertIn('prepare.browser', collaboration_browser.dependencies)
        self.assertNotIn('scripts/collaboration-browser.test.mjs', m98.inventory(repo, 'collaboration.node'))
        (repo/'packages/geosolve-engine/test/new.test.mjs').write_text('// added after prep')
        with self.assertRaisesRegex(ValueError,'inventory changed'):
            gate.m98_stages(self.root, {'m98':'target/capture','browser':'target/browser'})

    def test_deferred_group_keeps_all_m98_obligations_and_exact_target_selection(self):
        repo = self.fixtures()
        self.write('target/capture/prepared.json', json.dumps({
            'schema':'geosolve-m98-runtime-v1',
            'tests':{name:m98.inventory(repo,name) for name in m98.GROUPS}}))
        with patch.object(gate, 'qualification_stages', return_value=[]):
            _, groups = gate.pipeline_stages(self.root, {'m98':'target/capture','browser':'target/browser'}, 2, [])
            owning = next(group for group in groups if group[0] == 'm98')
            self.assertEqual(owning[1], ('prepare.m98',))
            self.assertEqual({stage.id for stage in owning[2]()}, set(m98.GROUPS))
            _, targeted = gate.pipeline_stages(self.root, {'m98':'target/capture'}, 2, ['engine.node'])
            self.assertEqual([stage.id for stage in targeted[0][2]()], ['engine.node'])

    def test_real_node_runs_in_private_copy_and_preserves_coverage_and_generated_evidence(self):
        captured = self.captured_runtime('''
          assert.equal(process.env.NODE_OPTIONS,undefined);
          assert.equal(process.env.GEOSOLVE_DIST,undefined);
          assert.equal(process.env.GEOSOLVE_LOADER_TEST_SDK_DIRECTORY,undefined);
          assert.equal(process.env.GEOSOLVE_M98_PACKAGES,undefined);
          assert.equal(process.env.GEOSOLVE_M98_PACKAGE_OUT,undefined);
          assert.equal(process.env.GEOSOLVE_M98_DIST,undefined);
          writeFileSync("packages/geosolve-engine/dist/runtime.js","private changed bytes");
          mkdirSync("target/m98/bake",{recursive:true});
          writeFileSync("target/m98/bake/shape.json",'{"regions":[]}');
        ''')
        before = gate.hash_output(captured)
        output = self.root / 'target/execution'
        with contextlib.redirect_stdout(io.StringIO()), patch.dict(os.environ, {
                'NODE_OPTIONS':'--not-a-real-node-flag', 'GEOSOLVE_DIST':'/foreign',
                'GEOSOLVE_LOADER_TEST_SDK_DIRECTORY':'/foreign', 'GEOSOLVE_M98_PACKAGES':'/foreign',
                'GEOSOLVE_M98_PACKAGE_OUT':'/foreign', 'GEOSOLVE_M98_DIST':'/foreign'}):
            m98.run(self.root, captured, 'engine.node', output)
        self.assertEqual(before, gate.hash_output(captured))
        self.assertFalse((output / 'repository').exists())
        self.assertTrue((output / 'artifacts/bake/shape.json').is_file())
        self.assertEqual(gate.read_json(output / 'coverage.json')['summary']['tests'],4)

    def test_changed_captured_bytes_or_installed_dependency_refuse_execution(self):
        captured = self.captured_runtime()
        runtime = captured / 'repository/packages/geosolve-engine/dist/runtime.js'
        original = runtime.read_bytes()
        runtime.write_text('// tampered\n')
        with self.assertRaisesRegex(ValueError, 'runtime bytes changed'):
            m98.run(self.root, captured, 'engine.node', self.root / 'target/tampered')
        runtime.write_bytes(original)
        self.write(m98.DEPENDENCIES[0] + '/installed.js', '// changed dependency\n')
        with self.assertRaisesRegex(ValueError, 'runtime bytes changed|dependency changed'):
            m98.run(self.root, captured, 'engine.node', self.root / 'target/changed-dependency')

    def test_real_node_mutating_shared_dependency_cannot_donate_success(self):
        captured = self.captured_runtime('''
          writeFileSync("crates/geosolve-demo-web/frontend/node_modules/installed.js","bad dependency write");
        ''')
        output = self.root / 'target/mutating'
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaisesRegex(ValueError, 'changed captured|changed installed'):
            m98.run(self.root, captured, 'engine.node', output)
        self.assertFalse((output / 'coverage.json').exists())

    def test_real_node_empty_owning_file_is_not_a_passing_file_level_test(self):
        captured = self.captured_runtime(empty_file=True)
        output = self.root / 'target/empty-owning-file'
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaisesRegex(ValueError, 'declares no tests'):
            m98.run(self.root, captured, 'engine.node', output)
        self.assertFalse((output / 'coverage.json').exists())

    def test_node_coverage_refuses_empty_partial_skipped_cancelled_and_failed_output(self):
        valid = '\n'.join(f'# {key} {value}' for key,value in {'tests':2,'pass':2,'fail':0,'cancelled':0,'skipped':0,'todo':0}.items())
        self.assertEqual(m98.validate_tap(valid)['tests'],2)
        for invalid in ('', valid.replace('# tests 2','# tests 0'),valid.replace('# pass 2','# pass 1'),
                        valid.replace('# skipped 0','# skipped 1'),valid.replace('# cancelled 0','# cancelled 1'),
                        valid.replace('# fail 0','# fail 1'),valid+'\n# tests 2'):
            with self.subTest(invalid=invalid),self.assertRaises(ValueError):
                m98.validate_tap(invalid)

    def test_native_engine_inputs_include_real_compiler_and_manifold_fixtures(self):
        real = Path(__file__).resolve().parents[2]
        inputs = gate.crate_inputs(real,'geosolve-sketch-engine')
        for name in ('crates/geosolve-sketch-engine/tests/fixtures/manifold.json',
                     'crates/geosolve-sketch-engine/tests/fixtures/session-radius-2.json'):
            self.assertTrue(gate.matches(name,inputs),name)

    def test_missing_collaboration_browser_and_frontend_owners_fail_closed(self):
        repo = self.fixtures()
        (repo / 'scripts/collaboration-browser.test.mjs').unlink()
        with self.assertRaisesRegex(ValueError, 'missing'):
            m98.inventory(repo, 'collaboration.browser')
        (repo / m98.FRONTEND / 'src/lib/collaboration-storage.test.ts').unlink()
        with self.assertRaisesRegex(ValueError, 'incomplete'):
            m98.inventory(repo, 'collaboration.frontend')

    def test_frontend_coverage_requires_exact_files_and_nonempty_passing_assertions(self):
        files = ['frontend/a.test.ts']
        result = {'name': str(self.root / files[0]), 'status': 'passed', 'assertionResults': [{'status':'passed'}]}
        report = {'success':True, 'numTotalTests':1, 'numPassedTests':1, 'testResults':[result]}
        self.assertEqual(m98.validate_frontend_report(report, files, self.root)['tests'],1)
        for invalid in ({**report,'numPendingTests':1}, {**report,'testResults':[]},
                        {**report,'testResults':[{**result,'assertionResults':[]}]},
                        {**report,'testResults':[{**result,'assertionResults':[{'status':'skipped'}]}]}):
            with self.assertRaises(ValueError):
                m98.validate_frontend_report(invalid, files, self.root)

    def test_browser_preparation_orders_and_authenticates_all_three_native_packages(self):
        self.fixtures(base='.')
        stages, prepared = gate.preparation_stages(self.runner(), {'wasm','browser'})
        by_id = {stage.id: stage for stage in stages}
        self.assertIn('prepare.m98', by_id['prepare.browser'].dependencies)
        self.assertIn('packages/geosolve-collaboration/dist', by_id['prepare.browser'].artifacts)
        self.assertIn(prepared['m98'], by_id['prepare.browser'].artifacts)
        self.assertIn('prepare.wasm', by_id['prepare.m98'].dependencies)

    def test_collaboration_rust_and_native_fixture_sources_change_preparation_identity(self):
        real = Path(__file__).resolve().parents[2]
        policy = gate.read_json(real / gate.POLICY_PATH)
        snapshot = gate.source_snapshot(real)
        def prepared(source):
            runner = gate.Runner(real, gate.Store(self.root / 'target/real-store'), source, policy, {})
            return gate.preparation_stages(runner, {'wasm','m98','browser'})[1]
        original = prepared(snapshot)
        for name in ('crates/geosolve-collaboration/src/text/history.rs',
                     'crates/geosolve-collaboration/examples/text_fixture.rs',
                     'crates/geosolve-collaboration-wasm/src/source.rs'):
            changed = prepared(snapshot | {name:{'sha256':'changed','executable':False}})
            self.assertNotEqual(original['m98'],changed['m98'])
            self.assertNotEqual(original['browser'],changed['browser'])


if __name__ == '__main__':
    unittest.main()
