# SPDX-License-Identifier: GPL-3.0-or-later
"""Real-process pipeline admission and deferred-inventory regressions."""
import contextlib
import fcntl
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


class PipelineTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        (self.root / '.gitignore').write_text('target/\n')
        self.store = gate.Store(self.root / 'target/store')
        self.policy = {'prose': [], 'global': [], 'owned': ['**'], 'memory_heavy_workers': 2}

    def tearDown(self):
        self.temp.cleanup()

    def stage(self, name, duration=0.05, **kwargs):
        path = self.root / 'target' / (name + '.json')
        code = ('import json,time;from pathlib import Path;'
                'start=time.monotonic();time.sleep(' + str(duration) + ');'
                'Path(' + repr(str(path)) + ').write_text(json.dumps([start,time.monotonic()]))')
        return gate.Stage(name, ((sys.executable, '-c', code),), **kwargs)

    def runner(self, stages, jobs=3):
        r = gate.Runner(self.root, self.store, gate.source_snapshot(self.root), self.policy, {}, jobs, True)
        r.prepare(stages)
        return r

    def interval(self, name):
        return json.loads((self.root / 'target' / (name + '.json')).read_text())

    def run_quiet(self, runner, stages, deferred=()):
        with contextlib.redirect_stdout(io.StringIO()):
            ok = runner.run(stages, deferred=deferred)
            report = runner.report(final=True)
        return ok, report

    def test_one_builder_overlaps_protected_execution_and_performance_drains(self):
        stages = [self.stage('build1', .5, build_lock=True), self.stage('build2', .4, build_lock=True),
                  self.stage('protected', .7, resource='memory'), self.stage('performance', .03, resource='exclusive')]
        r = self.runner(stages)
        self.assertTrue(self.run_quiet(r, stages)[0])
        a,b,t,p = [self.interval(name) for name in ('build1','build2','protected','performance')]
        self.assertGreaterEqual(b[0],a[1])
        self.assertLess(t[0],a[1])
        self.assertLess(b[0],t[1])
        self.assertGreaterEqual(p[0],max(a[1],b[1],t[1]))

    def native_preparation_fixture(self, bad=None):
        directory = self.root / 'target/prepared/native'
        directory.mkdir(parents=True)
        executable, auxiliary = directory / 'executable', directory / 'auxiliary'
        executable.write_bytes(b'captured main bytes')
        auxiliary.write_bytes(b'captured auxiliary bytes')
        expected = {p: gate.file_hash(p) for p in (executable, auxiliary)}
        metadata = {'artifacts': [{'executable': str(executable), 'sha256': expected[executable]}],
                    'auxiliary_executables': [{'path': str(auxiliary), 'sha256': expected[auxiliary]}]}
        if bad:
            metadata[bad][0]['sha256'] = 'wrong expected bytes'
        (directory / 'prepared.json').write_text(json.dumps(metadata))
        stage = self.stage('prepare.workspace', 0, outputs=('target/prepared/native',), build_lock=True, kind='build')
        return stage, expected

    def test_native_preparation_reconciles_inventory_with_one_fresh_captured_tree(self):
        stage, expected = self.native_preparation_fixture()
        runner = self.runner([stage])
        with patch.object(gate, 'file_hash', wraps=gate.file_hash) as observe:
            self.assertTrue(self.run_quiet(runner, [stage])[0])
        for path, sha in expected.items():
            self.assertEqual(sum(Path(call.args[0]) == path for call in observe.call_args_list), 1)
            self.assertEqual(runner.results[stage.id]['outputs'][str(path)], {'sha256': sha})
        # The stored preparation still checks actual output bytes before reuse.
        next(iter(expected)).write_bytes(b'changed after preparation')
        self.assertIsNone(self.store.find(stage.id, runner.keys[stage.id]))

    def test_native_preparation_rejects_wrong_main_digest(self):
        stage, _ = self.native_preparation_fixture('artifacts')
        runner = self.runner([stage])
        ok, report = self.run_quiet(runner, [stage])
        self.assertFalse(ok)
        self.assertFalse(report['complete'])
        self.assertEqual(runner.results[stage.id]['status'], 'failed')

    def test_native_preparation_rejects_wrong_auxiliary_digest(self):
        stage, _ = self.native_preparation_fixture('auxiliary_executables')
        runner = self.runner([stage])
        self.assertFalse(self.run_quiet(runner, [stage])[0])
        self.assertEqual(runner.results[stage.id]['status'], 'failed')

    def test_native_original_and_symlink_paths_observe_current_fallback_bytes(self):
        directory = self.root / 'target/prepared'
        directory.mkdir(parents=True)
        original = self.root / 'target/original'
        original.mkdir()
        binary = original / 'binary'
        binary.write_bytes(b'original')
        (directory / 'linked-parent').symlink_to(original, target_is_directory=True)
        (directory / 'linked-file').symlink_to(binary)
        frozen = gate.hash_output(directory)
        binary.write_bytes(b'changed original')
        for path in (binary, directory / 'linked-parent/binary', directory / 'linked-file',
                     directory / '../original/binary'):
            with self.subTest(path=path):
                self.assertEqual(gate.native_output_hash(directory, frozen, path), gate.hash_output(path))

    def test_consumer_key_rechecks_captured_bytes_after_preparation_snapshot(self):
        stage, expected = self.native_preparation_fixture()
        runner = self.runner([stage])
        self.assertTrue(self.run_quiet(runner, [stage])[0])
        binary = next(iter(expected))
        consumer = self.stage('native-test', 0, artifacts=(str(binary),), dependencies=(stage.id,))
        before = runner.key(consumer)
        binary.write_bytes(b'changed captured executable')
        runner.register([consumer])
        self.assertNotEqual(before, runner.keys[consumer.id])

    def test_group_expands_after_own_preparation_before_later_build_finishes(self):
        stages = [self.stage('prepare1', .04, build_lock=True), self.stage('prepare2', .5, build_lock=True)]
        r = self.runner(stages)
        def factory():
            self.assertEqual(r.results['prepare1']['status'],'passed')
            return [self.stage('discovered', .05, dependencies=('prepare1',))]
        ok,report = self.run_quiet(r,stages,[('native',('prepare1',),factory)])
        self.assertTrue(ok)
        self.assertTrue(report['complete'])
        self.assertEqual(report['deferred_groups']['native']['stages'],['discovered'])
        self.assertLess(self.interval('discovered')[1],self.interval('prepare2')[1])
        self.assertEqual(len(report['inventory']),3)

    def test_failed_preparation_cannot_hide_unexpanded_group(self):
        stage=gate.Stage('prepare',((sys.executable,'-c','raise SystemExit(3)'),),build_lock=True)
        r=self.runner([stage]);called=[]
        ok,report=self.run_quiet(r,[stage],[('native',('prepare',),lambda:called.append(True))])
        self.assertFalse(ok);self.assertFalse(report['complete']);self.assertFalse(called)
        self.assertEqual(report['deferred_groups']['native']['status'],'blocked')

    def test_failed_discovery_preserves_independent_pass_and_rejects_completion(self):
        stage=self.stage('prepare');r=self.runner([stage])
        def fail():raise ValueError('missing native inventory')
        ok,report=self.run_quiet(r,[stage],[('native',('prepare',),fail)])
        self.assertFalse(ok);self.assertFalse(report['complete'])
        self.assertEqual(report['deferred_groups']['native']['status'],'failed')
        self.assertIsNotNone(self.store.find('prepare',r.keys['prepare']))

    def test_duplicate_discovery_never_replaces_completed_stage(self):
        stage=self.stage('prepare');r=self.runner([stage])
        ok,report=self.run_quiet(r,[stage],[('native',('prepare',),lambda:[stage])])
        self.assertFalse(ok);self.assertFalse(report['complete'])
        self.assertEqual(len(report['inventory']),1)
        self.assertEqual(r.results['prepare']['status'],'passed')

    def test_registered_artifact_hash_does_not_reuse_prebuild_missing_value(self):
        stage=self.stage('prepare');r=self.runner([stage]);name='target/materialized'
        self.assertIsNone(r.artifact_hash(name))
        path=self.root/name;path.write_text('ready')
        consumer=self.stage('consumer',artifacts=(name,),dependencies=('prepare',))
        r.register([consumer])
        self.assertEqual(r.artifact_hash(name),{'sha256':gate.file_hash(path)})

    def test_pipeline_empty_native_inventory_is_failure(self):
        with patch.object(gate,'qualification_stages',return_value=[]),patch.object(gate,'native_stages',return_value=[]):
            _,groups=gate.pipeline_stages(self.root,{'workspace':'target/prepared'},3,[])
            with self.assertRaisesRegex(ValueError,'unexpectedly empty'):groups[0][2]()

    def test_serial_parallel_expand_identical_case_inventory(self):
        inventories=[]
        for jobs in (1,3):
            stages=[self.stage('prepare')];r=self.runner(stages,jobs)
            _,report=self.run_quiet(r,stages,[('native',('prepare',),lambda:[self.stage('case',dependencies=('prepare',),cases=('actual',))])])
            inventories.append([(s['id'],s['cases']) for s in report['inventory']])
        self.assertEqual(*inventories)

    def test_stopped_run_marks_reverse_order_dependents_without_throwing(self):
        preparation = self.stage('prepare')
        consumer = self.stage('consumer', dependencies=('prepare',))
        runner = self.runner([consumer, preparation])
        runner.stop.set()
        ok, report = self.run_quiet(runner, [consumer, preparation])
        self.assertFalse(ok)
        self.assertFalse(report['complete'])
        self.assertEqual(set(runner.results), {'prepare', 'consumer'})
        self.assertTrue(all(row['status'] in {'not_run', 'blocked'} for row in runner.results.values()))

    def test_stop_blocks_unexpanded_group_and_persists_authenticated_incomplete_report(self):
        stage = self.stage('prepare')
        runner = self.runner([stage])
        runner.stop.set()
        called = []
        ok, report = self.run_quiet(runner, [stage], [('native', ('prepare',), lambda: called.append(True))])
        self.assertFalse(ok)
        self.assertFalse(called)
        self.assertFalse(report['complete'])
        self.assertEqual(report['deferred_groups']['native']['status'], 'blocked')
        self.assertEqual(self.store.unseal(runner.run_dir / 'qualification.json'), json.loads(json.dumps(report)))

    def test_targeted_group_keeps_only_requested_cases_and_own_preparation(self):
        selected = self.stage('workspace.selected', dependencies=('prepare.workspace',), cases=('case',))
        unrelated = self.stage('workspace.unrelated', dependencies=('prepare.workspace',), cases=('other',))
        with patch.object(gate, 'qualification_stages', return_value=[]), patch.object(
                gate, 'native_stages', return_value=[selected, unrelated]) as discover:
            common, groups = gate.pipeline_stages(self.root, {'workspace': 'target/prepared'}, 3, ['workspace.selected'])
            self.assertEqual(common, [])
            self.assertEqual(groups[0][1], ('prepare.workspace',))
            self.assertEqual(groups[0][2](), [selected])
            discover.assert_called_once_with(self.root, {'workspace': 'target/prepared'})

    def test_failed_unrelated_preparation_does_not_block_independent_group_in_continue_mode(self):
        first = self.stage('prepare.ok')
        second = gate.Stage('prepare.fail', ((sys.executable, '-c', 'raise SystemExit(1)'),))
        runner = self.runner([first, second])
        with contextlib.redirect_stdout(io.StringIO()):
            ok = runner.run([first, second], stop_on_failure=False,
                            deferred=[('native', ('prepare.ok',), lambda: [self.stage('independent', dependencies=('prepare.ok',))])])
            report = runner.report(final=True)
        self.assertFalse(ok)
        self.assertFalse(report['complete'])
        self.assertEqual(report['deferred_groups']['native']['status'], 'expanded')
        self.assertEqual(runner.results['independent']['status'], 'passed')

    def test_resumed_authenticated_preparation_expands_without_executing_again(self):
        stage = self.stage('prepare', kind='build')
        first = self.runner([stage])
        self.assertTrue(self.run_quiet(first, [stage])[0])
        second = self.runner([stage])
        second.fresh = False
        ok, report = self.run_quiet(second, [stage],
                                    [('native', ('prepare',), lambda: [self.stage('new.case', dependencies=('prepare',))])])
        self.assertTrue(ok)
        self.assertTrue(report['complete'])
        self.assertEqual(second.results['prepare']['decision'], 'reuse')
        self.assertEqual(second.results['prepare']['origin_run'], first.run_id)
        self.assertEqual(second.results['new.case']['decision'], 'run')

    @contextlib.contextmanager
    def cli_fixture(self, arguments):
        policy = self.policy | {'workers': 3, 'test_debug': 'line-tables-only',
                                'release_incremental': True, 'release_codegen_units': 16}
        policy_path = self.root / gate.POLICY_PATH
        policy_path.parent.mkdir(parents=True, exist_ok=True)
        policy_path.write_text(json.dumps(policy))
        store = gate.Store(self.root / 'target/release-gate')
        preflight = self.stage('preflight.clippy')
        preparation = self.stage('prepare.workspace', kind='build', build_lock=True,
                                 dependencies=('preflight.clippy',), outputs=('target/prepared',))
        prepared = self.root / 'target/prepared'
        prepared.mkdir(parents=True, exist_ok=True)
        (prepared / 'prepared.json').write_text('{"stale":true}')
        resume_path = store.path / 'runs/prior/qualification.json'
        source = gate.source_snapshot(self.root)
        receipt_path = store.path / 'runs/prior/stages/passed/receipt.json'
        gate.write_json(receipt_path, store.seal({'source_sha256': gate.digest(source), 'stage': 'passed'}))
        gate.write_json(resume_path, store.seal({'source': source, 'results': [
            {'decision': 'run', 'complete': True, 'receipt_path': str(receipt_path)}]}))
        runners = []
        real_runner = gate.Runner
        def capture_runner(*args, **kwargs):
            runner = real_runner(*args, **kwargs)
            runners.append(runner)
            return runner
        output = io.StringIO()
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.dict(os.environ, {'GEOSOLVE_ALLOW_DIRTY': '1'}))
            stack.enter_context(patch.object(gate, 'ROOT', self.root))
            stack.enter_context(patch.object(gate, 'tool_identity', return_value={}))
            stack.enter_context(patch.object(gate, 'preflight_stages', side_effect=lambda **kwargs: [preflight]))
            stack.enter_context(patch.object(gate, 'preparation_stages', return_value=([preparation], {'workspace': 'target/prepared'})))
            stack.enter_context(patch.object(gate, 'Runner', side_effect=capture_runner))
            stack.enter_context(patch.object(sys, 'argv', ['release_gate.py', *arguments]))
            stack.enter_context(contextlib.redirect_stdout(output))
            yield store, runners, preparation, output

    def test_cli_plan_resume_does_not_save_receipts_or_write_store(self):
        with self.cli_fixture(['--plan', '--resume', 'prior']) as (store, runners, _, output):
            before = gate.hash_output(store.path)
            with patch.object(gate, 'pipeline_stages', return_value=([], [])), \
                    patch.object(gate.Store, 'save') as save, patch.object(gate, 'write_json') as write:
                self.assertEqual(gate.main(), 0)
            save.assert_not_called()
            write.assert_not_called()
            self.assertEqual(gate.hash_output(store.path), before)
            self.assertEqual(runners[0].results, {})
            self.assertIn('prepare.workspace', output.getvalue())

    def test_cli_resume_cannot_import_while_another_runner_holds_lock(self):
        with self.cli_fixture(['--resume', 'prior']) as (store, _, _, _):
            with (store.path / 'runner.lock').open('a') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                before = gate.hash_output(store.path)
                with patch.object(gate.Store, 'save') as save:
                    with self.assertRaisesRegex(ValueError, 'runner or storage cleanup is active'):
                        gate.main()
                save.assert_not_called()
                self.assertEqual(gate.hash_output(store.path), before)

    def test_cli_plan_uses_pipeline_contracts_and_stale_preparation_never_expands(self):
        raw_common = self.stage('rust.check', resource='exclusive')
        native = self.stage('workspace.selected', dependencies=('prepare.workspace',), cases=('selected',))
        with self.cli_fixture(['--plan']) as (_, runners, _, output), \
                patch.object(gate, 'qualification_stages', return_value=[raw_common]), \
                patch.object(gate, 'native_stages', return_value=[native]) as discovery:
            expected, _ = gate.pipeline_stages(self.root, {'workspace': 'target/prepared'}, 3, [])
            self.assertEqual(gate.main(), 0)
            recorded = {stage.id: stage for stage in runners[0].inventory}
            self.assertEqual(recorded['rust.check'].contract(), expected[0].contract())
            self.assertTrue(recorded['rust.check'].build_lock)
            self.assertEqual(recorded['rust.check'].resource, 'normal')
            discovery.assert_not_called()
            self.assertNotIn('workspace.selected', recorded)
            self.assertIn('exact case inventory pending authenticated preparation', output.getvalue())

    def test_cli_plan_authenticated_preparation_expands_exact_pipeline_group(self):
        raw_common = self.stage('rust.check', resource='exclusive')
        native = self.stage('workspace.selected', dependencies=('prepare.workspace',), cases=('selected',))
        with self.cli_fixture(['--plan']) as (_, runners, preparation, _), \
                patch.object(gate, 'qualification_stages', return_value=[raw_common]), \
                patch.object(gate, 'native_stages', return_value=[native]):
            real_find = gate.Store.find
            def find(store, identity, key):
                if identity == preparation.id:
                    return {'status': 'passed', 'complete': True, 'key': key, 'stage': identity}
                return real_find(store, identity, key)
            expected, groups = gate.pipeline_stages(self.root, {'workspace': 'target/prepared'}, 3, [])
            expected += groups[0][2]()
            with patch.object(gate.Store, 'find', autospec=True, side_effect=find):
                self.assertEqual(gate.main(), 0)
            recorded = {stage.id: stage.contract() for stage in runners[0].inventory}
            for stage in expected:
                self.assertEqual(recorded[stage.id], stage.contract())
            self.assertEqual(runners[0].results, {})

    def test_native_children_isolate_caches_evidence_and_ambient_golden_controls(self):
        code = ("import os; from pathlib import Path; "
                "assert not any(k.startswith('GEOSOLVE_GOLDEN_') for k in os.environ); "
                "assert os.environ['DENO_DIR'] == str(Path(os.environ['TMPDIR']) / 'deno'); "
                "assert os.environ['M92_MECHANISM_EVIDENCE'].endswith('/stages/workspace.fixture/mechanism-audit'); "
                "assert '/external/' not in os.environ['M92_MECHANISM_EVIDENCE']")
        stage = gate.Stage('workspace.fixture', ((sys.executable, '-c', code),))
        with patch.dict(os.environ, {'GEOSOLVE_GOLDEN_ORACLE_CASE': 'external-case',
                                    'DENO_DIR': '/external/cache', 'M92_MECHANISM_EVIDENCE': '/external/evidence'}):
            self.assertTrue(self.run_quiet(self.runner([stage]), [stage])[0])

    def test_native_overlap_requires_body_review_and_preserves_property_seed_writers(self):
        import release_equivalence as equivalence
        import release_gate_native as native
        descriptions = []
        for identity in ('geosolve-core::m1::normal', 'geosolve-sketch::m22_properties::normal',
                         'geosolve-linkage::m23_properties::normal'):
            descriptions.append(dict(id=identity, command=['/protected/test'], env={}, features=[], profile={},
                                     resource='native', selected=['actual'], build_overlap_safe=True, runtime_closures=[]))
        snapshot = {gate.FRONTEND + '/scripts/build-release-artifacts.mjs': {'sha256': 'writer'},
                    'scripts/verify-geosolve-sketch-code-package.sh': {'sha256': 'package-writer'}}
        with patch.object(gate, 'read_json', return_value=self.policy), \
                patch.object(gate, 'rust_inputs', return_value=('crates/**',)), \
                patch.object(gate, 'crate_inputs', return_value=()), \
                patch.object(gate, 'source_snapshot', return_value=snapshot), \
                patch.object(native, 'workspace_stages', return_value=descriptions), \
                patch.object(equivalence, 'reviewed', return_value=True) as review:
            stages = gate.native_stages(self.root, {'workspace': 'target/prepared'})
            self.assertEqual([stage.build_lock for stage in stages], [False, True, True])
            self.assertEqual(set(review.call_args.args[1]), set(snapshot))
            review.return_value = False
            self.assertTrue(all(stage.build_lock for stage in gate.native_stages(self.root, {'workspace': 'target/prepared'})))
            review.return_value = True
            with patch.object(gate, 'rust_inputs', return_value=('**',)):
                self.assertTrue(all(stage.build_lock for stage in gate.native_stages(self.root, {'workspace': 'target/prepared'})))


if __name__=='__main__':unittest.main()
