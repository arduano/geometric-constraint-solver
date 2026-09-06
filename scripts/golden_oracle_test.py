#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Synthetic execution and classification regressions; never build or run product tests."""
import concurrent.futures
import contextlib
import io
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import threading
import time
import unittest

import golden_oracle as oracle


class ClassificationTests(unittest.TestCase):
    def test_inventory_and_pass_bytes_are_the_unchanged_reviewed_corpus(self):
        golden = oracle.GOLDEN.read_text()
        rows = oracle.parse_rows(golden, oracle.inventory(), reviewed=True)
        self.assertEqual(len(rows), 271)
        self.assertEqual(oracle.tsv(oracle.classify(rows, rows)), golden)
        self.assertEqual((oracle.NATIVE_TIMEOUT, oracle.PARITY_TIMEOUT), (30, 60))

    def test_findings_require_exact_signature_and_cannot_bless_a_changed_input(self):
        case, family = oracle.inventory()[0]
        old = [case, family, 'DEFECT', 'M93-F001', 'residual', 'original']
        observed = [case, family, 'DEFECT', '-', 'residual', 'original']
        self.assertEqual(oracle.classify([observed], [old])[0][3], 'M93-F001')
        observed[-1] = 'changed'
        self.assertEqual(oracle.classify([observed], [old])[0][3], '-')
        with self.assertRaises(ValueError):
            oracle.parse_rows(oracle.tsv([observed]), reviewed=True)

    def test_schema_rejects_missing_duplicate_and_wrong_family_rows(self):
        rows = oracle.parse_rows(oracle.GOLDEN.read_text())
        for bad in [rows[1:], rows + [rows[0]], [['not-a-case', *rows[0][1:]], *rows[1:]]]:
            with self.assertRaises(ValueError):
                oracle.parse_rows(oracle.tsv(bad), oracle.inventory())

    def test_parity_result_rejects_extra_rows_and_preserves_failure_classification(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'parity.tsv'
            case, family = oracle.inventory()[0]
            path.write_text('PASS\t-\tok\n')
            self.assertIsNone(oracle.parity_row(case, family, path, 'parity-export'))
            path.write_text('PASS\t-\tok\nPASS\t-\tok\n')
            self.assertEqual(oracle.parity_row(case, family, path, 'parity-export')[2:],
                             ['HARNESS_ERROR', '-', 'parity-export', 'malformed-result'])
            path.write_text('DEFECT\tresidual\toriginal\n')
            self.assertEqual(oracle.parity_row(case, family, path, 'parity-export')[2:],
                             ['DEFECT', '-', 'residual', 'original'])

    def test_native_failure_timeout_and_panic_classes_match_reference(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'log'
            path.write_text('test result: FAILED\n')
            args = oracle.inventory()[0]
            self.assertEqual(oracle.classify_process(*args, {'exit_code': 101}, path, 30)[2:],
                             ['PANIC', '-', 'test-process', 'exit-101'])
            for code in (124, 137):
                self.assertEqual(oracle.classify_process(*args, {'exit_code': code}, path, 60)[2:],
                                 ['TIMEOUT', '-', 'case-timeout', '60s'])
            path.write_text('unknown launcher error')
            self.assertEqual(oracle.classify_process(*args, {'exit_code': 0}, path, 30)[2:],
                             ['HARNESS_ERROR', '-', 'test-process', 'exit-0'])


class ProcessTests(unittest.TestCase):
    def test_timeout_kills_parent_and_descendants_and_retains_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            script = directory / 'fork.py'
            pidfile = directory / 'child.pid'
            script.write_text('''import os, signal, time
pid = os.fork()
if pid == 0:
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    while True: time.sleep(1)
open(os.environ['CHILD_PID'], 'w').write(str(pid))
while True: time.sleep(1)
''')
            runner = oracle.ProcessRunner(grace=0.1)
            result = runner.run([sys.executable, str(script)], directory,
                                dict(os.environ, CHILD_PID=str(pidfile)), directory, 'timeout', 0.25)
            self.assertEqual(result['exit_code'], 124)
            self.assertTrue(result['timed_out'])
            child = int(pidfile.read_text())
            self.assert_not_running(child)
            self.assertTrue((directory / 'timeout.receipt.json').is_file())
            self.assertLess(result['duration_seconds'], 2)

    def test_successful_parent_cannot_leave_background_child_running(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            pidfile = directory / 'child.pid'
            code = "import os,time; pid=os.fork(); open('child.pid','w').write(str(pid)) if pid else time.sleep(20)"
            result = oracle.ProcessRunner(grace=0.1).run([sys.executable, '-c', code], directory,
                                                        dict(os.environ), directory, 'parent', 2)
            self.assertEqual(result['exit_code'], 0)
            self.assert_not_running(int(pidfile.read_text()))

    def assert_not_running(self, pid):
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline:
            status = Path(f'/proc/{pid}/stat')
            if not status.exists() or status.read_text().split()[2] == 'Z':
                return
            time.sleep(0.01)
        self.fail(f'child {pid} survived process group cleanup')

    def test_cancellation_interrupts_active_work_and_prevents_new_processes(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            cancelled = threading.Event()
            runner = oracle.ProcessRunner(cancelled, grace=0.1)
            timer = threading.Timer(0.1, cancelled.set)
            timer.start()
            try:
                result = runner.run([sys.executable, '-c', 'import time; time.sleep(20)'], directory,
                                    dict(os.environ), directory, 'cancel', 10)
                self.assertEqual(result['exit_code'], 130)
                self.assertTrue(result['interrupted'])
                second = runner.run([sys.executable, '-c', "open('unexpected','w').write('ran')"],
                                    directory, dict(os.environ), directory, 'queued', 10)
                self.assertTrue(second['interrupted'])
                self.assertFalse((directory / 'unexpected').exists())
            finally:
                timer.cancel()


class SchedulerTests(unittest.TestCase):
    def test_bounded_parallel_and_serial_results_agree_and_continue_after_failure(self):
        class FakeEvaluator:
            def __init__(self):
                self.lock = threading.Lock()
                self.active = self.peak = 0
                self.visited = []
            def case(self, item):
                with self.lock:
                    self.active += 1
                    self.peak = max(self.peak, self.active)
                    self.visited.append(item[0])
                try:
                    time.sleep(0.01)
                    if item[0].endswith('seed-00'):
                        raise ValueError('synthetic bad case')
                    return [*item, 'PASS', '-', '-', 'input-0000000000000000']
                finally:
                    with self.lock:
                        self.active -= 1
        items = oracle.inventory()[:12]
        serial, parallel = FakeEvaluator(), FakeEvaluator()
        expected = oracle.evaluate_cases(serial, items, 1)
        actual = oracle.evaluate_cases(parallel, items, 2)
        self.assertEqual(actual, expected)
        self.assertEqual(len(parallel.visited), len(items))
        self.assertEqual(serial.peak, 1)
        self.assertEqual(parallel.peak, 2)
        self.assertTrue(any(row[2] == 'HARNESS_ERROR' for row in actual))

    def test_case_direct_execution_preserves_four_steps_and_isolated_temp_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / 'packages/geosolve-sketch-code'
            package.mkdir(parents=True)
            output = root / 'capture'
            output.mkdir()
            binary = root / 'fake-test'
            binary.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys
name = next(arg for arg in sys.argv[1:] if not arg.startswith('--') and arg not in ('pretty', 'never', '1'))
env = os.environ
assert pathlib.Path(env['TMPDIR']).is_dir()
assert env['CARGO_MANIFEST_DIR'] == str(pathlib.Path.cwd())
assert env['CARGO_PKG_NAME'] == 'synthetic-cargo-env'
case = env['GEOSOLVE_GOLDEN_ORACLE_FAMILY'] + '.' + env['GEOSOLVE_GOLDEN_ORACLE_CASE']
if name == 'golden_oracle_family_survey':
    pathlib.Path(env['GEOSOLVE_GOLDEN_ORACLE_OUTPUT']).write_text('case_id\\tfamily\\tstatus\\tfinding_id\\tfailure_class\\tfingerprint\\n' + case + '\\t' + env['GEOSOLVE_GOLDEN_ORACLE_FAMILY'] + '\\tPASS\\t-\\t-\\tinput-0000000000000000\\n')
    pathlib.Path(env['GEOSOLVE_GOLDEN_ORACLE_PARITY_MANIFEST']).write_text('{}')
else:
    directory = pathlib.Path(env['GEOSOLVE_GOLDEN_PARITY_DIRECTORY'])
    directory.mkdir(exist_ok=True)
    if name.endswith('_export'):
        (directory / 'compile-batch.request.json').write_text('{"entries":[]}')
    else:
        assert json.loads((directory / 'compile-batch.output.json').read_text()) == {'entries':[]}
    pathlib.Path(env['GEOSOLVE_GOLDEN_PARITY_RESULT']).write_text('PASS\\t-\\tok\\n')
print('test ' + name + ' ... ok')
print('test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s')
''')
            binary.chmod(0o755)
            deno = root / 'fake-deno'
            deno.write_text('#!/usr/bin/env python3\nimport sys,json\nassert json.load(sys.stdin)=={"entries":[]}\nprint("{\\"entries\\":[]}")\n')
            deno.chmod(0o755)
            artifact = dict(executable=str(binary), cwd=str(root), package='fake', target={'name':'fake'},
                            env={'CARGO_MANIFEST_DIR':str(root), 'CARGO_PKG_NAME':'synthetic-cargo-env'},
                            cases=['golden_oracle_family_survey', 'golden_backend_parity_export', 'golden_backend_parity_validate'],
                            ignored=[], sha256=oracle.native.file_hash(binary), resource='native', features=[], profile={})
            evaluator = oracle.Evaluator(root, output, oracle.ProcessRunner(grace=0.1),
                                         {'authoring':artifact, 'parity':artifact}, str(deno))
            items = oracle.inventory()[:2]
            rows = oracle.evaluate_cases(evaluator, items, 2)
            self.assertTrue(all(row[2] == 'PASS' for row in rows), rows)
            for case, _ in items:
                folder = output / 'cases' / case
                receipts = list(folder.glob('*.receipt.json'))
                self.assertEqual(len(receipts), 4)
                commands = [json.loads(path.read_text())['command'] for path in receipts]
                self.assertFalse(any('cargo' in command or 'npm' in command for command in commands))
                for path in receipts:
                    env = json.loads(path.read_text())['environment']
                    self.assertEqual(env['TMPDIR'], str(folder / 'tmp'))
                    self.assertEqual(env['DENO_DIR'], str(folder / 'tmp/deno'))


class ObservationTests(unittest.TestCase):
    def capture(self, directory):
        text = oracle.GOLDEN.read_text()
        for name in ('golden.tsv', 'actual.tsv', 'classified.tsv'):
            (directory / name).write_text(text)
        for row in oracle.parse_rows(text):
            case_dir = directory / 'cases' / row[0]
            case_dir.mkdir(parents=True)
            oracle.write_json(case_dir / 'case.json', {'case_id':row[0], 'family':row[1], 'row':row})
        receipt = dict(schema=oracle.SCHEMA, state='complete', source_start={'source':'original'},
                       runtime_start={'synthetic':'original'}, runtime_end={'synthetic':'original'},
                       source_end={'source':'original'}, start_utc='2026-09-05T00:00:00+00:00',
                       inventory=oracle.inventory(), files=oracle.seal(directory))
        oracle.write_json(directory / 'observation.json', receipt)

    def test_three_dispositions_share_original_observation_without_executing(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.capture(directory)
            original = (directory / 'observation.json').read_bytes()
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                for mode in ('survey', 'check', 'require-clean'):
                    self.assertEqual(oracle.disposition(mode, directory), 0)
            self.assertEqual((directory / 'observation.json').read_bytes(), original)
            self.assertEqual(len(list(directory.iterdir())), 5)

    def test_changed_capture_incomplete_state_and_inventory_fail_closed(self):
        for mutation in ('row', 'state', 'inventory'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                self.capture(directory)
                if mutation == 'row':
                    path = directory / 'actual.tsv'
                    path.write_text(path.read_text().replace('input-', 'altered-', 1))
                else:
                    path = directory / 'observation.json'
                    receipt = json.loads(path.read_text())
                    if mutation == 'state':
                        receipt['state'] = 'interrupted'
                    else:
                        receipt['inventory'] = receipt['inventory'][:-1]
                    path.write_text(json.dumps(receipt))
                with self.assertRaises(ValueError):
                    oracle.validate_observation(directory)

    def test_external_path_in_evidence_manifest_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.capture(directory)
            path = directory / 'observation.json'
            receipt = json.loads(path.read_text())
            receipt['files']['../outside'] = {'bytes':0,'sha256':'not-a-hash'}
            path.write_text(json.dumps(receipt))
            with self.assertRaises(ValueError):
                oracle.validate_observation(directory)


if __name__ == '__main__':
    unittest.main()
