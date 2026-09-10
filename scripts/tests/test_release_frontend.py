# SPDX-License-Identifier: GPL-3.0-or-later
"""Frontend result reuse never drops catalog-sensitive preflight obligations."""
import copy
import contextlib
import dataclasses
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate as gate
import release_equivalence as eq
import release_gate_m98 as m98


def stamp(value):
    return {'sha256': value, 'executable': False}


class FrontendPreflightTests(unittest.TestCase):
    def setUp(self):
        with patch.object(gate, 'rust_inputs', return_value=('crates/**',)):
            self.stages = {stage.id: stage for stage in gate.preflight_stages()}
        self.policy = gate.read_json(gate.ROOT / gate.POLICY_PATH)
        self.sample = eq.SAMPLE_ROOT + '/one/sketch.ts'
        self.program = gate.FRONTEND + '/src/App.tsx'
        self.snapshot = {
            self.sample: stamp('sample source'),
            eq.SAMPLE_ROOT + '/one/sketch.compiled.json': stamp('compiled sample'),
            eq.SAMPLE_ROOT + '/one/manifest.json': stamp('sample metadata'),
            eq.CATALOG: stamp('reviewed catalog'),
            eq.FRONTEND_CATALOG: stamp('frontend catalog'),
            self.program: stamp('frontend program'),
            'packages/geosolve-sketch-code/src/index.ts': stamp('SDK program'),
        }
        unreviewed = dataclasses.replace(self.stages['preflight.frontend'], input_equivalence_contract=None)
        inputs = gate.stage_inputs(unreviewed, self.snapshot, self.policy)
        contract = {'schema': 1, 'frontend_static': {'program_sha256': gate.digest(eq.partition(inputs, self.policy))}}
        self.frontend = dataclasses.replace(unreviewed, input_equivalence_contract=contract)
        self.catalog = self.stages['preflight.catalog']

    def inputs(self, stage, snapshot=None, policy=None):
        return gate.stage_inputs(stage, self.snapshot if snapshot is None else snapshot,
                                 self.policy if policy is None else policy)

    def test_split_preserves_every_static_command_once_and_vitest_cache_policy(self):
        manifest = json.loads((gate.ROOT / gate.FRONTEND / 'package.json').read_text())
        scripts = [command.removeprefix('npm run ') for command in manifest['scripts']['check:static'].split(' && ')]
        self.assertEqual(set(scripts), {'check:manifest', 'check:licenses', 'test:build', 'check:language-sdk'})
        commands = [*self.frontend.commands, *self.catalog.commands]
        for name in scripts:
            self.assertEqual(commands.count(gate.npm(gate.FRONTEND, 'run', name)), 1)
        self.assertEqual(commands.count(gate.npm(gate.FRONTEND, 'ci', '--ignore-scripts')), 1)
        unit = next(command for command in commands if command[:4] == gate.npm(gate.FRONTEND, 'test'))
        self.assertEqual(unit[4:6], ('--', '--no-cache'))
        self.assertTrue(all(arg == '--exclude' for arg in unit[6::2]))
        self.assertEqual(tuple(gate.FRONTEND + '/' + pattern for pattern in unit[7::2]), m98.GROUPS['collaboration.frontend'])
        self.assertEqual(len(m98.FRONTEND_RUNTIME_TESTS), 3)
        self.assertEqual(len(commands), len(scripts) + 2)
        self.assertNotIn(gate.npm(gate.FRONTEND, 'run', 'test:build'), self.frontend.commands)
        self.assertIn(gate.npm(gate.FRONTEND, 'run', 'test:build'), self.catalog.commands)
        self.assertIn('preflight.catalog', self.stages['preflight.clippy'].dependencies)
        self.assertIn('preflight.frontend', self.catalog.dependencies)
        self.assertIn('preflight.metadata-format', self.catalog.dependencies)

    def test_numeric_edit_sample_addition_and_prune_invalidate_catalog_only(self):
        baseline_frontend, baseline_catalog = self.inputs(self.frontend), self.inputs(self.catalog)
        variants = [self.snapshot | {self.sample: stamp('numeric edit')},
                    self.snapshot | {eq.SAMPLE_ROOT + '/two/sketch.ts': stamp('new sample')},
                    {name: value for name, value in self.snapshot.items() if not name.startswith(eq.SAMPLE_ROOT + '/one/')}]
        for changed in variants:
            with self.subTest(changed=sorted(set(changed) ^ set(self.snapshot))):
                self.assertEqual(self.inputs(self.frontend, changed), baseline_frontend)
                self.assertNotEqual(self.inputs(self.catalog, changed), baseline_catalog)
        self.assertEqual(self.frontend.excluded_inputs, (eq.SAMPLE_ROOT + '/**',))
        self.assertEqual(self.catalog.excluded_inputs, ())
        self.assertIsNone(self.catalog.input_equivalence)

    def test_catalog_metadata_remains_frontend_input_even_when_program_guard_matches(self):
        for path in (eq.CATALOG, eq.FRONTEND_CATALOG):
            changed = self.snapshot | {path: stamp('new catalog title/order/key')}
            self.assertTrue(eq.reviewed('frontend_static', self.inputs(self.catalog, changed), self.policy,
                                        self.frontend.input_equivalence_contract))
            self.assertNotEqual(self.inputs(self.frontend, changed), self.inputs(self.frontend))
            self.assertIn(path, self.inputs(self.frontend, changed))

    def test_vitest_sdk_config_license_global_and_unknown_program_changes_invalidate(self):
        paths = [self.program, gate.FRONTEND + '/src/App.test.tsx', gate.FRONTEND + '/vite.config.ts',
                 gate.FRONTEND + '/src/language/generated/language-service-declarations.ts',
                 gate.FRONTEND + '/scripts/check-runtime-licenses.mjs',
                 gate.FRONTEND + '/package-lock.json', 'packages/geosolve-sketch-code/src/index.ts',
                 'scripts/release_gate.py', 'unowned-new-input.js']
        for path in paths:
            changed = self.snapshot | {path: stamp('changed dependency')}
            with self.subTest(path=path):
                self.assertNotEqual(self.inputs(self.frontend, changed), self.inputs(self.frontend))
                self.assertIn(self.sample, self.inputs(self.frontend, changed))

    def test_unrecognized_sample_files_symlinks_executables_and_global_overrides_fail_closed(self):
        changes = [self.snapshot | {eq.SAMPLE_ROOT + '/one/shared-program.js': stamp('new program')},
                   self.snapshot | {eq.SAMPLE_ROOT + '/one/patches/nested.patch.ts': stamp('nested unknown')},
                   self.snapshot | {self.sample: {'link': '../../../program', 'target': 'changed'}},
                   self.snapshot | {self.sample: {'sha256': 'script', 'executable': True}}]
        for changed in changes:
            self.assertIn(self.sample, self.inputs(self.frontend, changed))
            self.assertNotEqual(self.inputs(self.frontend, changed), self.inputs(self.frontend))
        policy = copy.deepcopy(self.policy)
        policy['global'].append(self.sample)
        self.assertIn(self.sample, self.inputs(self.frontend, policy=policy))

    def test_missing_stale_wrong_scope_or_unresolved_review_never_excludes_samples(self):
        for contract in (None, {}, {'schema': 1, 'frontend_static': {'program_sha256': 'stale'}},
                         {'schema': 1, 'browser': self.frontend.input_equivalence_contract['frontend_static']}):
            stage = dataclasses.replace(self.frontend, input_equivalence_contract=contract)
            self.assertIn(self.sample, self.inputs(stage))
        unresolved = dataclasses.replace(self.frontend, inputs=('**',))
        self.assertIn(self.sample, self.inputs(unresolved))

    def test_input_cache_keeps_frontend_exclusion_separate_from_catalog_boundary(self):
        runner = object.__new__(gate.Runner)
        runner.snapshot, runner.policy, runner.input_maps = self.snapshot, self.policy, {}
        self.assertNotIn(self.sample, runner.inputs(self.frontend))
        self.assertIn(self.sample, runner.inputs(self.catalog))
        stale = dataclasses.replace(self.frontend, input_equivalence_contract=None)
        self.assertIn(self.sample, runner.inputs(stale))
        self.assertNotIn(self.sample, runner.inputs(self.frontend))


class FrontendOverlapInputTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.policy = gate.read_json(gate.ROOT / gate.POLICY_PATH)
        (self.root / 'scripts').mkdir()
        (self.root / gate.POLICY_PATH).write_text(json.dumps(self.policy))
        (self.root / 'target/native').mkdir(parents=True)
        (self.root / 'target/native/prepared.json').write_text('{}')
        self.sample = eq.SAMPLE_ROOT + '/one/sketch.ts'
        self.snapshot = {
            self.sample: stamp('sample'), eq.CATALOG: stamp('catalog'),
            eq.FRONTEND_CATALOG: stamp('frontend catalog'),
            'crates/geosolve-core/src/lib.rs': stamp('core program'),
            gate.FRONTEND + '/scripts/build-release-artifacts.mjs': stamp('frontend build'),
            'packages/geosolve-sketch-code/src/index.ts': stamp('managed program'),
        }
        for name in ('rust_inputs', 'crate_inputs'):
            mock = patch.object(gate, name, return_value=('crates/**',))
            mock.start()
            self.addCleanup(mock.stop)
        inputs = gate.build_overlap_inputs(self.root, self.snapshot, self.policy)
        entry = {'program_sha256': gate.digest(eq.partition(inputs, self.policy))}
        self.contract = {'schema': 1, 'frontend_build_overlap': entry, 'native_build_overlap': entry}
        self.save_contract(self.contract)

    def save_contract(self, value):
        (self.root / eq.CONTRACT_PATH).write_text(json.dumps(value))

    def planned(self, snapshot=None):
        import release_gate_native as native
        snapshot = self.snapshot if snapshot is None else snapshot
        runner = SimpleNamespace(root=self.root, snapshot=snapshot, policy=self.policy, tools={}, environment={})
        stages, prepared = gate.preparation_stages(runner)
        description = {'id': 'geosolve-core::fixture::normal', 'command': ['target/native/test'],
                       'env': {}, 'features': [], 'profile': {}, 'resource': 'native',
                       'selected': ['exact_case'], 'build_overlap_safe': True, 'runtime_closures': {}}
        with patch.object(gate, 'source_snapshot', return_value=snapshot), patch.object(
                native, 'workspace_stages', return_value=[description]):
            body = gate.native_stages(self.root, {'workspace': 'target/native'})[0]
        return {stage.id: stage for stage in stages}, prepared, body

    def test_reviewed_prepared_package_keeps_writer_dependencies_and_exact_command(self):
        stages, prepared, body = self.planned()
        browser = stages['prepare.browser']
        self.assertFalse(browser.build_lock)
        self.assertFalse(body.build_lock)
        self.assertEqual(browser.resource, 'memory')
        self.assertEqual(browser.kind, 'build')
        self.assertEqual(browser.dependencies, ('preflight.clippy', 'prepare.wasm', 'prepare.m98'))
        self.assertEqual(browser.commands, (gate.cmd(sys.executable, 'scripts/release_gate.py',
            '--prepare-browser', '--wasm-package', prepared['wasm'], '--output', prepared['browser']),))
        self.assertEqual(browser.outputs, (prepared['browser'],))
        self.assertTrue(all(stage.build_lock for name, stage in stages.items() if name != 'prepare.browser'))

    def test_all_concurrent_program_changes_lock_both_frontend_and_native(self):
        paths = [gate.FRONTEND + '/package.json', gate.FRONTEND + '/package-lock.json',
                 gate.FRONTEND + '/vite.config.ts', gate.FRONTEND + '/tsconfig.app.json',
                 gate.FRONTEND + '/postcss.config.js', gate.FRONTEND + '/tailwind.config.ts',
                 gate.FRONTEND + '/scripts/build-release-artifacts.mjs',
                 'crates/geosolve-core/build.rs', 'crates/geosolve-core/tests/helper.rs',
                 'scripts/golden-authoring-scene-oracle.sh', 'scripts/verify-geosolve-sketch-code-package.sh',
                 'packages/geosolve-sketch-code/scripts/compile-managed-batch-deno.mjs',
                 'Cargo.lock', 'scripts/release_gate.py', 'unowned-new-writer.js']
        for path in paths:
            with self.subTest(path=path):
                stages, _, body = self.planned(self.snapshot | {path: stamp('changed program')})
                self.assertTrue(stages['prepare.browser'].build_lock)
                self.assertTrue(body.build_lock)

    def test_reviewed_catalog_numeric_edit_and_prune_keep_overlap(self):
        variants = [self.snapshot | {self.sample: stamp('numeric edit')},
                    {name: value for name, value in self.snapshot.items() if name != self.sample},
                    self.snapshot | {eq.CATALOG: stamp('retired key'), eq.FRONTEND_CATALOG: stamp('pruned catalog')}]
        for snapshot in variants:
            stages, _, body = self.planned(snapshot)
            self.assertFalse(stages['prepare.browser'].build_lock)
            self.assertFalse(body.build_lock)

    def test_unrecognized_data_missing_input_or_unresolved_include_locks_both(self):
        variants = [self.snapshot | {self.sample: {'link': '../program', 'target': 'changed'}},
                    self.snapshot | {self.sample: {'sha256': 'executable', 'executable': True}},
                    self.snapshot | {eq.SAMPLE_ROOT + '/one/new-program.js': stamp('new program')},
                    {name: value for name, value in self.snapshot.items() if not name.startswith('packages/')}]
        for snapshot in variants:
            stages, _, body = self.planned(snapshot)
            self.assertTrue(stages['prepare.browser'].build_lock)
            self.assertTrue(body.build_lock)
        with patch.object(gate, 'rust_inputs', return_value=('**',)):
            stages, _, body = self.planned()
            self.assertTrue(stages['prepare.browser'].build_lock)
            self.assertTrue(body.build_lock)

    def test_missing_corrupt_stale_or_wrong_scope_never_authorizes_frontend_overlap(self):
        for contract in (None, {}, {'schema': 1, 'frontend_build_overlap': {'program_sha256': 'stale'}},
                         {'schema': 1, 'frontend_static': self.contract['frontend_build_overlap']},
                         {'schema': 1, 'native_build_overlap': self.contract['native_build_overlap']}):
            self.save_contract(contract)
            self.assertTrue(self.planned()[0]['prepare.browser'].build_lock)
        path = self.root / eq.CONTRACT_PATH
        path.write_text('{invalid')
        self.assertTrue(self.planned()[0]['prepare.browser'].build_lock)
        path.unlink()
        stages, _, body = self.planned()
        self.assertTrue(stages['prepare.browser'].build_lock)
        self.assertTrue(body.build_lock)


class FrontendOverlapSchedulingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        (self.root / '.gitignore').write_text('target/\n')
        self.store = gate.Store(self.root / 'target/store')
        self.policy = {'prose': [], 'global': [], 'owned': ['**'], 'memory_heavy_workers': 2}

    def stage(self, name, duration=0.03, **kwargs):
        path = self.root / 'target' / (name + '.json')
        code = ('import json,time;from pathlib import Path;start=time.monotonic();'
                f'time.sleep({duration});Path({str(path)!r}).write_text(json.dumps([start,time.monotonic()]))')
        return gate.Stage(name, ((sys.executable, '-c', code),), **kwargs)

    def runner(self, stages, jobs=3, fresh=True):
        runner = gate.Runner(self.root, self.store, gate.source_snapshot(self.root), self.policy, {}, jobs, fresh)
        runner.prepare(stages)
        return runner

    def execute(self, runner, stages, deferred=(), stop_on_failure=True):
        with contextlib.redirect_stdout(io.StringIO()):
            passed = runner.run(stages, deferred=deferred, stop_on_failure=stop_on_failure)
            return passed, runner.report(final=True)

    def interval(self, name):
        return json.loads((self.root / 'target' / (name + '.json')).read_text())

    def test_frontend_starts_before_queued_native_work_and_overlaps_one_cargo_writer(self):
        wasm = self.stage('prepare.wasm', .05, build_lock=True)
        native = [self.stage('native.' + str(i), .6, dependencies=(wasm.id,), resource='memory') for i in range(2)]
        browser = self.stage('prepare.browser', .4, dependencies=(wasm.id,), resource='memory', kind='build')
        builders = [self.stage('cargo.' + str(i), .7, dependencies=(wasm.id,), build_lock=True) for i in range(2)]
        consumer = self.stage('browser.consumer', dependencies=(browser.id,), resource='memory')
        performance = self.stage('performance', resource='exclusive')
        stages = [wasm, *native, *builders, browser, consumer, performance]
        self.assertTrue(self.execute(self.runner(stages), stages)[0])
        w, b, c0, c1 = [self.interval(name) for name in (wasm.id, browser.id, builders[0].id, builders[1].id)]
        self.assertGreaterEqual(b[0], w[1])
        self.assertLess(b[0], c0[1])
        self.assertLess(c0[0], b[1])
        self.assertGreaterEqual(c1[0], c0[1])
        self.assertLess(b[0], self.interval(native[1].id)[0])
        self.assertGreaterEqual(self.interval(consumer.id)[0], b[1])
        p = self.interval(performance.id)
        self.assertTrue(all(self.interval(stage.id)[1] <= p[0] for stage in stages if stage != performance))
        events = sorted((moment, delta, stage.resource) for stage in stages
                        for moment, delta in zip(self.interval(stage.id), (1, -1)))
        active = memory = 0
        for _, delta, resource in events:
            active += delta
            memory += delta if resource == 'memory' else 0
            self.assertLessEqual(active, 3)
            self.assertLessEqual(memory, 2)

    def test_stale_review_fallback_and_one_worker_keep_serial_exclusion(self):
        for jobs, locked in ((3, True), (1, False)):
            with self.subTest(jobs=jobs, frontend_locked=locked):
                browser = self.stage('prepare.browser', .15, build_lock=locked, resource='memory', kind='build')
                cargo = self.stage('cargo', .15, build_lock=True)
                stages = [cargo, browser]
                self.assertTrue(self.execute(self.runner(stages, jobs), stages)[0])
                b, c = self.interval(browser.id), self.interval(cargo.id)
                self.assertTrue(b[1] <= c[0] or c[1] <= b[0])

    def test_frontend_priority_waits_for_an_available_memory_slot(self):
        memory = [self.stage('memory.' + str(i), .4 + i * .2, resource='memory') for i in range(2)]
        wasm = self.stage('prepare.wasm', .1, build_lock=True)
        browser = self.stage('prepare.browser', .1, dependencies=(wasm.id,), resource='memory', kind='build')
        stages = [*memory, wasm, browser]
        self.assertTrue(self.execute(self.runner(stages), stages)[0])
        b = self.interval(browser.id)
        intervals = [self.interval(stage.id) for stage in memory]
        self.assertGreaterEqual(b[0], min(interval[1] for interval in intervals))
        self.assertTrue(all(interval[0] < self.interval(wasm.id)[1] for interval in intervals))

    def test_failed_frontend_blocks_browser_inventory_but_retains_independent_cargo_for_resume(self):
        browser = gate.Stage('prepare.browser', ((sys.executable, '-c', 'raise SystemExit(7)'),),
                             resource='memory', kind='build')
        cargo = self.stage('cargo', .15, build_lock=True, kind='build')
        calls = []
        deferred = [('browser', (browser.id,), lambda: calls.append(True))]
        first = self.runner([browser, cargo], fresh=False)
        passed, report = self.execute(first, [browser, cargo], deferred, stop_on_failure=False)
        self.assertFalse(passed)
        self.assertFalse(report['complete'])
        self.assertEqual(report['deferred_groups']['browser']['status'], 'blocked')
        self.assertFalse(calls)
        fixed = self.stage('prepare.browser', resource='memory', kind='build')
        next_run = self.runner([fixed, cargo], fresh=False)
        self.assertEqual(next_run.decision(fixed)[0], 'run')
        self.assertEqual(next_run.decision(cargo)[0], 'reuse')
        consumer = self.stage('browser.consumer', dependencies=(fixed.id,), cases=('exact_browser_case',))
        passed, report = self.execute(next_run, [fixed, cargo], [('browser', (fixed.id,), lambda: [consumer])])
        self.assertTrue(passed)
        self.assertTrue(report['complete'])
        self.assertEqual(next_run.results[cargo.id]['origin_run'], first.run_id)
        self.assertEqual(report['deferred_groups']['browser']['stages'], [consumer.id])


if __name__ == '__main__':
    unittest.main()
