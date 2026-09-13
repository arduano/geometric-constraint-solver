# SPDX-License-Identifier: GPL-3.0-or-later
"""Real isolated receipt stores exercise eviction, provenance and destructive boundaries."""
import contextlib
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate as gate
import release_storage as storage


class StorageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        (self.root / '.gitignore').write_text('target/\n')
        self.store = gate.Store(self.root / 'target/release-gate')
        self.policy = {'prose':['docs/**'], 'global':[], 'owned':['src/**']}
        self.limits = dict(storage.POLICY, orphan_hours=1)

    def stage(self, name, code="print('ok')", **kw):
        return gate.Stage(name, ((sys.executable, '-c', code),), **kw)

    def execute(self, run, stages):
        runner = gate.Runner(self.root, self.store, gate.source_snapshot(self.root), self.policy, {}, run_id=run)
        runner.prepare(stages)
        with contextlib.redirect_stdout(io.StringIO()):
            runner.run(stages)
            report = runner.report(final=True)
        return runner, report

    def plan(self, **kw):
        return storage.plan(self.root, self.store, self.limits, live=(), **kw)

    def test_pin_keeps_reused_donor_scratch_and_original_prepared_inputs(self):
        native = self.store.path / 'prepared' / ('a'*64) / 'workspace'
        native.mkdir(parents=True)
        (native/'binary').write_text('original')
        stage = self.stage('native', "print('proof')", artifacts=(str(native/'binary'),))
        first, _ = self.execute('a-old', [stage])
        second, _ = self.execute('b-pinned', [stage])
        storage.pin(self.store, 'accepted', 'b-pinned')
        self.assertEqual(second.results['native']['origin_run'], 'a-old')
        obsolete = self.store.path/'prepared'/('b'*64)/'workspace'
        obsolete.mkdir(parents=True); (obsolete/'junk').write_bytes(b'x'*8192)
        report, closure = self.plan()
        self.assertIn(native, closure.keep)
        self.assertIn(first.run_dir/'stages/native', closure.keep)
        self.assertTrue(any(x['path']==str(obsolete.relative_to(self.root)) for x in report['actions']))
        self.assertGreater(storage.verify(closure)['evidence_files'],0)
        with storage.locked(self.store):
            storage.apply(self.root,self.store,report)
        self.assertFalse(obsolete.exists())
        self.assertIsNotNone(self.store.find('native',first.keys['native']))
        self.assertTrue(native.exists())

    def test_evicted_unpinned_result_requires_fresh_execution(self):
        stage = self.stage('old')
        old,_ = self.execute('a-old',[stage])
        self.execute('z-recent',[self.stage('recent')])
        report,_=self.plan()
        with storage.locked(self.store):storage.apply(self.root,self.store,report)
        self.assertFalse((old.run_dir/'stages/old').exists())
        self.assertTrue((old.run_dir/'qualification.json').is_file())
        runner=gate.Runner(self.root,self.store,gate.source_snapshot(self.root),self.policy,{})
        runner.prepare([stage]);self.assertEqual(runner.decision(stage)[0],'run')
        # Resuming old metadata preserves available donors and reruns missing caches.
        self.plan(extra_runs=('a-old',))

    def test_successful_donor_from_failed_run_is_preserved(self):
        stage=self.stage('good')
        old,_=self.execute('a-failed',[stage,self.stage('bad','raise SystemExit(2)')])
        self.execute('z-accepted',[stage]);storage.pin(self.store,'accepted','z-accepted')
        report,closure=self.plan()
        self.assertIn(old.run_dir/'stages/good',closure.keep)
        self.assertNotIn(old.run_dir/'stages/bad',closure.keep)
        storage.verify(closure)

    def test_tampered_or_missing_pinned_receipt_refuses_cleanup(self):
        runner,_=self.execute('z-accepted',[self.stage('good')]);storage.pin(self.store,'accepted','z-accepted')
        path=runner.run_dir/'stages/good/receipt.json';original=path.read_bytes()
        path.write_text('{}')
        with self.assertRaisesRegex(ValueError,'unauthenticated'):self.plan()
        path.write_bytes(original);path.unlink()
        with self.assertRaisesRegex(ValueError,'missing'):self.plan()

    def test_tampered_evidence_refuses_verification(self):
        runner,_=self.execute('z-accepted',[self.stage('good')]);storage.pin(self.store,'accepted','z-accepted')
        _,closure=self.plan();(runner.run_dir/'stages/good/output.log').write_text('forged')
        with self.assertRaisesRegex(ValueError,'evidence changed'):storage.verify(closure)

    def test_browser_reused_leaf_follows_signed_batch_and_preparation(self):
        binary=self.store.path/'prepared'/('c'*64)/'browser'
        binary.mkdir(parents=True);(binary/'wasm').write_text('actual wasm placeholder')
        old,_=self.execute('a-browser',[self.stage('browser-old')])
        stage=old.run_dir/'stages/browser-old'
        batch=stage/'batch.json'
        gate.write_json(batch,self.store.seal({'schema':'geosolve-browser-batch-v1','artifact':{'directory':str(binary)},'evidence':{str(stage/'output.log'):gate.file_hash(stage/'output.log')}}))
        leaf=self.store.path/'browser-leaves'/('d'*64+'.json')
        gate.write_json(leaf,self.store.seal({'batch':{'path':str(batch),'sha256':gate.file_hash(batch)},'artifact':{'directory':str(binary)}}))
        current,_=self.execute('z-current',[self.stage('later')])
        proof=current.run_dir/'stages/later/coverage.json'
        gate.write_json(proof,{'leaves':[{'decision':'reuse','receipt_key':'d'*64}]})
        receipt=current.run_dir/'stages/later/receipt.json'
        value=self.store.unseal(receipt);value['evidence'][str(proof)]=gate.file_hash(proof)
        gate.write_json(receipt,self.store.seal(value))
        storage.pin(self.store,'accepted','z-current')
        _,closure=self.plan()
        self.assertIn(stage,closure.keep);self.assertIn(binary,closure.keep)
        storage.verify(closure)

    def test_symlink_roots_and_external_targets_are_never_candidates(self):
        outside=self.root/'outside';outside.mkdir();(outside/'data').write_text('keep')
        parent=self.store.path/'prepared'/('e'*64);parent.mkdir(parents=True)
        (parent/'workspace').symlink_to(outside,target_is_directory=True)
        with self.assertRaisesRegex(ValueError,'symlink'):self.plan()
        self.assertEqual((outside/'data').read_text(),'keep')
        (parent/'workspace').unlink();(self.root/'target/debug').symlink_to(outside,target_is_directory=True)
        self.limits['target_gib']=1
        with self.assertRaisesRegex(ValueError,'symlink'):
            storage.regular_tree(self.root/'target/debug/child',self.root/'target')

    def test_prune_lock_is_exclusive_and_retains_inode(self):
        with storage.locked(self.store):
            inode=(self.store.path/'runner.lock').stat().st_ino
            with self.assertRaisesRegex(ValueError,'active'):
                with storage.locked(self.store):pass
        self.assertEqual((self.store.path/'runner.lock').stat().st_ino,inode)

    def test_live_stage_and_unknown_project_directory_are_preserved(self):
        old,_=self.execute('a-old',[self.stage('old')])
        self.execute('z-new',[self.stage('new')])
        project=self.root/'target/user-project';project.mkdir();(project/'journal').write_text('history')
        report,closure=storage.plan(self.root,self.store,self.limits,live={old.run_dir/'stages/old/output.log'})
        self.assertIn(old.run_dir/'stages/old',closure.keep)
        self.assertFalse(any('user-project' in x['path'] for x in report['actions']))

    def test_pin_metadata_cannot_be_replaced_silently(self):
        self.execute('a-one',[self.stage('a')]);self.execute('z-two',[self.stage('b')])
        storage.pin(self.store,'accepted','a-one')
        with self.assertRaisesRegex(ValueError,'different run'):storage.pin(self.store,'accepted','z-two')
        (self.store.path/'retention.json').write_text('{}')
        with self.assertRaisesRegex(ValueError,'unauthenticated'):self.plan()

    def test_reserve_guard_prevents_success_and_stops_running_child(self):
        runner=gate.Runner(self.root,self.store,gate.source_snapshot(self.root),self.policy,{},run_id='z-full')
        stage=self.stage('writer',"import time; print('started',flush=True); time.sleep(20)")
        runner.prepare([stage]);runner.storage_limits=self.limits
        # First guard permits launch; the next check observes the reserve threshold.
        with patch.object(storage,'free_guard',side_effect=[None,ValueError('disk reserve reached')]):
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertFalse(runner.run([stage]))
        self.assertEqual(runner.results['writer']['status'],'storage_exhausted')
        self.assertFalse(runner.results['writer'].get('complete', False))
        self.assertTrue(runner.stop.is_set())

    def test_over_budget_protected_evidence_is_not_automatically_unpinned(self):
        self.execute('z-protected',[self.stage('good')]);storage.pin(self.store,'accepted','z-protected')
        report,closure=self.plan()
        self.assertTrue(closure.keep)
        with patch.object(storage,'allocated',return_value=999*storage.GIB):
            with self.assertRaisesRegex(ValueError,'budget'):storage.guard(self.root,self.limits)
        self.assertEqual(storage.pins(self.store),{'accepted':'z-protected'})

    def test_stage_pin_preserves_delivery_without_pinning_unrelated_native_build(self):
        old, _ = self.execute('a-delivery', [self.stage('package'), self.stage('native')])
        self.execute('z-current', [self.stage('current')])
        storage.pin(self.store, 'installed', 'a-delivery', 'package')
        report, closure = self.plan()
        self.assertIn(old.run_dir / 'stages/package', closure.keep)
        self.assertNotIn(old.run_dir / 'stages/native', closure.keep)
        with storage.locked(self.store):
            storage.apply(self.root, self.store, report)
        storage.verify(self.plan()[1])
        (old.run_dir / 'stages/package/receipt.json').unlink()
        with self.assertRaisesRegex(ValueError, 'missing'):
            self.plan()

    def test_relative_output_symlink_preserves_transitive_preparation(self):
        a = self.store.path / 'prepared' / ('a' * 64) / 'workspace'
        b = self.store.path / 'prepared' / ('b' * 64) / 'headless'
        a.mkdir(parents=True); b.mkdir(parents=True)
        (b / 'binary').write_text('shared immutable runtime')
        (a / 'dependency').symlink_to('../../' + 'b' * 64 + '/headless')
        self.execute('z-accepted', [self.stage('prepare', outputs=(str(a),))])
        storage.pin(self.store, 'accepted', 'z-accepted')
        report, closure = self.plan()
        self.assertIn(b, closure.keep)
        storage.verify(closure)
        with storage.locked(self.store):
            storage.apply(self.root, self.store, report)
        storage.verify(self.plan()[1])

    def test_real_live_process_configured_and_relative_assets_are_protected(self):
        configured = self.store.path / 'prepared' / ('a' * 64) / 'browser'
        relative = self.store.path / 'prepared' / ('b' * 64) / 'browser'
        configured.mkdir(parents=True); relative.mkdir(parents=True)
        process = subprocess.Popen([sys.executable, '-c',
            "import time; print('ready', flush=True); time.sleep(30)",
            str(relative.relative_to(self.root))], cwd=self.root,
            env={**os.environ, 'GEOSOLVE_DIST': str(configured)}, stdout=subprocess.PIPE)
        try:
            self.assertEqual(process.stdout.readline(), b'ready\n')
            live = storage.active_paths(self.root)
            self.assertIn(configured, live); self.assertIn(relative, live)
            report, closure = storage.plan(self.root, self.store, self.limits, live=live)
            self.assertIn(configured, closure.keep); self.assertIn(relative, closure.keep)
            self.assertEqual(report['actions'], [])
        finally:
            process.terminate(); process.wait(timeout=5); process.stdout.close()

    def test_preparation_lock_inheritance_rejects_unrelated_descriptor(self):
        with storage.locked(self.store) as handle:
            with patch.dict(os.environ, {'GEOSOLVE_RELEASE_LOCK_FD': str(handle.fileno())}):
                with storage.preparation_lock(self.root):
                    with self.assertRaisesRegex(ValueError, 'active'):
                        with storage.locked(self.store):
                            pass
            with tempfile.TemporaryFile() as unrelated:
                with patch.dict(os.environ, {'GEOSOLVE_RELEASE_LOCK_FD': str(unrelated.fileno())}):
                    with self.assertRaisesRegex(ValueError, 'invalid inherited'):
                        with storage.preparation_lock(self.root):
                            pass

    def test_apply_refuses_unknown_directory_and_newly_active_payload(self):
        old, _ = self.execute('a-old', [self.stage('old')])
        self.execute('z-new', [self.stage('new')])
        report, _ = self.plan()
        with patch.object(storage, 'active_paths', return_value={old.run_dir / 'stages/old'}):
            with self.assertRaisesRegex(ValueError, 'became active'):
                storage.apply(self.root, self.store, report)
        unknown = self.root / 'target/project'; unknown.mkdir()
        report['actions'] = [{'path': 'target/project', 'kind': 'build-cache'}]
        with self.assertRaisesRegex(ValueError, 'known Cargo cache'):
            storage.apply(self.root, self.store, report)
        self.assertTrue(unknown.exists())

    def test_reserve_failure_before_launch_never_creates_child(self):
        runner = gate.Runner(self.root, self.store, gate.source_snapshot(self.root), self.policy, {})
        marker = self.root / 'launched'
        stage = self.stage('writer', f"from pathlib import Path; Path({str(marker)!r}).touch()")
        runner.prepare([stage]); runner.storage_limits = self.limits
        with patch.object(storage, 'free_guard', side_effect=ValueError('disk reserve reached')):
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertFalse(runner.run([stage]))
        self.assertFalse(marker.exists())
        self.assertFalse(runner.results['writer'].get('complete', False))

    def test_nested_output_expectation_cannot_hide_behind_valid_parent(self):
        output = self.store.path / 'prepared' / ('a' * 64) / 'workspace'
        output.mkdir(parents=True); (output / 'binary').write_text('actual')
        runner, _ = self.execute('z-current', [self.stage('prepare', outputs=(str(output),))])
        receipt = runner.run_dir / 'stages/prepare/receipt.json'
        value = self.store.unseal(receipt)
        value['outputs'][str(output / 'binary')] = {'sha256': '0' * 64}
        gate.write_json(receipt, self.store.seal(value))
        with self.assertRaisesRegex(ValueError, 'prepared output changed'):
            storage.verify(self.plan()[1])


if __name__=='__main__':unittest.main()
