#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Bounded golden evaluation; captured observations are not fresh test executions."""
from __future__ import annotations

import argparse
import concurrent.futures
import contextlib
import datetime as dt
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time

import release_gate_native as native

# Observation validation is integrity/provenance checking inside the supervising
# runner's trusted local receipt store. It is not signature verification for
# arbitrary external receipts or evidence of a fresh execution on today's source.
ROOT = Path(__file__).resolve().parent.parent
GOLDEN = ROOT / 'crates/geosolve-constraint-editor/tests/fixtures/golden_authoring_scene_oracle.golden.tsv'
HEADER = 'case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint'
SCHEMA = 'geosolve-golden-observation-v1'
NATIVE_TIMEOUT = 30
PARITY_TIMEOUT = 60
STATUSES = {'PASS', 'DEFECT', 'PANIC', 'TIMEOUT', 'HARNESS_ERROR'}
FAMILIES = '''constraint.fixed-point constraint.coincident-points constraint.point-on-curve
constraint.curve-contact constraint.horizontal-line constraint.vertical-line constraint.parallel-lines
constraint.perpendicular-lines constraint.radial-line constraint.equal-length constraint.equal-radius
constraint.equal-curvature constraint.midpoint constraint.symmetric-about-line constraint.curve-tangency
constraint.endpoint-continuity dimension.point-distance dimension.segment-length dimension.radius
 dimension.diameter dimension.oriented-angle constraint.horizontal-points constraint.vertical-points
constraint.concentric-curves constraint.collinear-supports constraint.coincident-with-origin
constraint.point-on-datum-axis constraint.collinear-with-datum-axis constraint.symmetric-about-datum-axis'''.split()
AUTHORING_CASES = ['deterministic', *[f'seed-{n:02}' for n in range(8)]]
FILLET_CASES = '''feature.fillet.authoring.coincident-closure.curve-pair
feature.fillet.authoring.coincident-closure.point
feature.fillet.authoring.native-profile.line-line
feature.fillet.evaluation.line-circle.same-cell-lower
feature.fillet.evaluation.line-circle.same-cell-seam
feature.fillet.evaluation.line-circle.source-rotation.retained-start'''.split()
SCENE_CASES = '''scene.current-computed.empty scene.current-native.withheld
scene.current-computed.fillet scene.rejected-historical.detached'''.split()
SCENE_EXPORT = 'workbench::tests::golden_scene_backend_parity::golden_scene_backend_parity_export'
SCENE_VALIDATE = 'workbench::tests::golden_scene_backend_parity::golden_scene_backend_parity_validate'
DENO_FLAGS = ['run', '--no-config', '--no-lock', '--no-prompt', '--cached-only',
              '--no-remote', '--node-modules-dir=manual', '--ignore-env',
              'scripts/compile-managed-batch-deno.mjs']


def utc():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def stamp(path):
    path = Path(path)
    if path.is_symlink() or not path.is_file():
        raise ValueError(f'expected a regular evidence file: {path}')
    data = path.read_bytes()
    return {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def write_json(path, value):
    with Path(path).open('x') as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write('\n')


def inventory():
    return sorted([(f'{family}.{case}', family) for family in FAMILIES for case in AUTHORING_CASES]
                  + [(case, 'feature.fillet') for case in FILLET_CASES]
                  + [(case, 'scene-authority') for case in SCENE_CASES])


def parse_rows(data, expected=None, reviewed=False, fingerprints=True):
    lines = data.splitlines()
    if not lines or lines[0] != HEADER:
        raise ValueError('oracle TSV header differs')
    rows = [line.split('\t') for line in lines[1:]]
    seen = set()
    for row in rows:
        if len(row) != 6 or row[0] in seen or row[2] not in STATUSES:
            raise ValueError('malformed, duplicate or unknown oracle row')
        case, family, status, finding, failure, fingerprint = row
        seen.add(case)
        inferred = ('scene-authority' if case.startswith('scene.') else
                    'feature.fillet' if case.startswith('feature.fillet.') else
                    re.sub(r'\.(deterministic|seed-\d\d)$', '', case))
        if inferred != family:
            raise ValueError('oracle family differs from case identity')
        if reviewed:
            if status == 'PASS':
                if finding != '-' or failure != '-':
                    raise ValueError('reviewed PASS has failure metadata')
                if fingerprints and not (family == 'scene-authority' and fingerprint == 'ok' or
                                         family != 'scene-authority' and re.fullmatch(r'input-[a-fA-F0-9]{16}', fingerprint)):
                    raise ValueError('reviewed PASS lacks exact input fingerprint')
            elif (not re.fullmatch(r'M\d+[A-Z]*-F\d{3,}', finding) or
                  failure in ('-', '') or fingerprint in ('', 'ok')):
                raise ValueError('golden contains an unclassified row')
    if expected is not None and sorted((row[0], row[1]) for row in rows) != sorted(expected):
        raise ValueError('oracle did not classify the exact inventory')
    return rows


def tsv(rows):
    return HEADER + '\n' + ''.join('\t'.join(row) + '\n' for row in sorted(rows))


def classify(rows, golden_rows):
    recorded = {row[0]: row for row in golden_rows}
    classified = []
    for row in rows:
        row = list(row)
        old = recorded.get(row[0])
        if old and [row[i] for i in (1, 2, 4, 5)] == [old[i] for i in (1, 2, 4, 5)]:
            row[3] = old[3]
        classified.append(row)
    return classified


def harness_row(case, family, status, failure, fingerprint):
    return [case, family, status, '-', failure, fingerprint]


def classify_process(case, family, result, log, timeout):
    code = result['exit_code']
    if code in (124, 137):
        return harness_row(case, family, 'TIMEOUT', 'case-timeout', f'{timeout}s')
    text = Path(log).read_text(errors='replace') if Path(log).exists() else ''
    status = 'PANIC' if re.search(r'panicked at|test result: FAILED', text) else 'HARNESS_ERROR'
    return harness_row(case, family, status, 'test-process', f'exit-{code}')


def parity_row(case, family, path, stage):
    try:
        lines = Path(path).read_text().splitlines()
        if len(lines) != 1:
            raise ValueError('parity result must contain exactly one row')
        status, failure, fingerprint = lines[0].split('\t')
        if status == 'PASS' and failure == '-' and fingerprint == 'ok':
            return None
        if status in STATUSES - {'PASS'} and failure not in ('', '-') and fingerprint:
            return harness_row(case, family, status, failure, fingerprint)
    except (OSError, ValueError):
        pass
    return harness_row(case, family, 'HARNESS_ERROR', stage, 'malformed-result')


class ProcessRunner:
    """Each command owns a process group; timeout/cancellation always reaps descendants."""
    def __init__(self, cancel=None, grace=5):
        self.cancel = cancel or threading.Event()
        self.grace = grace

    @staticmethod
    def signal_group(process, sig):
        with contextlib.suppress(ProcessLookupError):
            os.killpg(process.pid, sig)

    def terminate(self, process):
        self.signal_group(process, signal.SIGTERM)
        deadline = time.monotonic() + self.grace
        # A parent can exit before its children. The group still receives SIGKILL.
        while process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.02)
        self.signal_group(process, signal.SIGKILL)
        process.wait()

    def run(self, command, cwd, environment, directory, label, timeout, stdin=None, stdout=None):
        directory = Path(directory)
        log = directory / f'{label}.log'
        output = Path(stdout) if stdout else log
        start, monotonic = utc(), time.monotonic()
        timed_out = interrupted = False
        process = None
        code = 125
        try:
            with contextlib.ExitStack() as stack:
                out = stack.enter_context(output.open('wb'))
                err = out if output == log else stack.enter_context(log.open('wb'))
                inp = stack.enter_context(Path(stdin).open('rb')) if stdin else subprocess.DEVNULL
                if self.cancel.is_set():
                    interrupted = True
                else:
                    process = subprocess.Popen(command, cwd=cwd, env=environment, stdin=inp,
                                               stdout=out, stderr=err, start_new_session=True)
                    while process.poll() is None:
                        interrupted = self.cancel.is_set()
                        timed_out = time.monotonic() - monotonic >= timeout
                        if interrupted or timed_out:
                            self.terminate(process)
                            break
                        time.sleep(0.02)
                    code = 130 if interrupted else 124 if timed_out else process.returncode
                    if code < 0:
                        code = 128 - code
        except OSError as error:
            with log.open('ab') as handle:
                handle.write(f'{type(error).__name__}: {error}\n'.encode())
        finally:
            if process is not None:
                self.signal_group(process, signal.SIGKILL)
                process.wait()
        result = {'command': list(map(str, command)), 'cwd': str(cwd), 'start_utc': start,
                  'end_utc': utc(), 'duration_seconds': time.monotonic() - monotonic,
                  'timeout_seconds': timeout, 'timed_out': timed_out, 'interrupted': interrupted,
                  'exit_code': code, 'log': stamp(log), 'stdout': stamp(output),
                  'environment': {key: value for key, value in environment.items()
                                  if key.startswith(('GEOSOLVE_', 'CARGO_', 'RUST')) or
                                  key in ('LD_LIBRARY_PATH', 'DYLD_FALLBACK_LIBRARY_PATH', 'TMPDIR', 'DENO_DIR')}}
        write_json(directory / f'{label}.receipt.json', result)
        return result


class Evaluator:
    def __init__(self, root, output, processes, artifacts, deno):
        self.root, self.output, self.processes = Path(root), Path(output), processes
        self.artifacts, self.deno = artifacts, deno
        self.environment = {key: value for key, value in os.environ.items()
                            if not key.startswith('GEOSOLVE_GOLDEN_')}

    def test(self, key, name, env, directory, label, timeout):
        artifact = self.artifacts[key]
        stage = native.execution_stage(artifact, exact=name, test_threads=1)
        command = [*stage['command'], '--nocapture']
        result = self.processes.run(command, stage['cwd'], env | stage['env'], directory, label, timeout)
        if result['exit_code'] == 0:
            try:
                native.validate_results((directory / f'{label}.log').read_text(), stage['selected'],
                                        stage['ignored'], stage['filtered_out'])
            except native.NativeError as error:
                write_json(directory / f'{label}-validation.json',
                           {'status': 'failed', 'process_exit_code': 0, 'error': str(error)})
                # Zero matched/executed cases is a harness failure, never a successful oracle.
                result = dict(result, exit_code=125)
        return result

    def case(self, item):
        case, family = item
        directory = self.output / 'cases' / case
        directory.mkdir(parents=True)
        # Runtime caches are disposable; only proof inputs, outputs and logs are sealed.
        with tempfile.TemporaryDirectory(prefix='gs-golden-') as runtime:
            temporary = Path(runtime)
            env = dict(self.environment, TMPDIR=str(temporary), DENO_DIR=str(temporary / 'deno'))
            native_output, manifest = directory / 'native.tsv', directory / 'manifest.json'
            parity_dir, result_path = directory / 'parity', directory / 'parity-result.tsv'
            env.update(GEOSOLVE_GOLDEN_ORACLE_CASE=case, GEOSOLVE_GOLDEN_ORACLE_OUTPUT=str(native_output),
                       GEOSOLVE_GOLDEN_ORACLE_PARITY_MANIFEST=str(manifest),
                       GEOSOLVE_GOLDEN_PARITY_MANIFEST=str(manifest),
                       GEOSOLVE_GOLDEN_PARITY_DIRECTORY=str(parity_dir),
                       GEOSOLVE_GOLDEN_PARITY_RESULT=str(result_path))
            if family == 'scene-authority':
                native_key, native_test = 'scene', 'workbench::tests::golden_scene_authority_oracle_survey'
                parity_key, export_test, validate_test = 'scene', SCENE_EXPORT, SCENE_VALIDATE
                stage_prefix = 'scene-parity'
            elif family == 'feature.fillet':
                native_key, native_test = 'fillet', 'golden_fillet_oracle_survey'
                parity_key, export_test, validate_test = ('parity', 'golden_fillet_backend_parity_export',
                                                         'golden_fillet_backend_parity_validate')
                stage_prefix = 'parity'
            else:
                native_key, native_test = 'authoring', 'golden_oracle_family_survey'
                parity_key, export_test, validate_test = 'parity', 'golden_backend_parity_export', 'golden_backend_parity_validate'
                stage_prefix = 'parity'
                env.update(GEOSOLVE_GOLDEN_ORACLE_FAMILY=family,
                           GEOSOLVE_GOLDEN_ORACLE_CASE=case[len(family) + 1:])
            outcome = self.test(native_key, native_test, env, directory, 'native', NATIVE_TIMEOUT)
            if outcome['exit_code'] or family != 'scene-authority' and not manifest.is_file():
                row = classify_process(case, family, outcome, directory / 'native.log', NATIVE_TIMEOUT)
            else:
                row = self.parity(case, family, parity_key, export_test, validate_test, stage_prefix,
                                  env, directory, parity_dir, result_path)
                if row is None:
                    try:
                        row = parse_rows(native_output.read_text(), [(case, family)])[0]
                    except (OSError, ValueError):
                        row = harness_row(case, family, 'HARNESS_ERROR', 'native-output', 'malformed-output')
            (directory / 'row.tsv').write_text(tsv([row]))
            write_json(directory / 'case.json', {'case_id': case, 'family': family, 'row': row,
                                               'interrupted': self.processes.cancel.is_set()})
            return row

    def parity(self, case, family, key, export, validate, prefix, env, directory, parity_dir, result_path):
        for phase, test in [('export', export), ('validate', validate)]:
            result_path.unlink(missing_ok=True)
            outcome = self.test(key, test, env, directory, phase, PARITY_TIMEOUT)
            if outcome['exit_code']:
                return classify_process(case, family, outcome, directory / f'{phase}.log', PARITY_TIMEOUT)
            row = parity_row(case, family, result_path, f'{prefix}-{phase}')
            if row is not None:
                return row
            if phase == 'export':
                request, compiled = parity_dir / 'compile-batch.request.json', parity_dir / 'compile-batch.output.json'
                if not request.is_file() or not request.stat().st_size:
                    return harness_row(case, family, 'HARNESS_ERROR', f'{prefix}-export', 'missing-compile-batch')
                outcome = self.processes.run([self.deno, *DENO_FLAGS],
                    self.root / 'packages/geosolve-sketch-code', env, directory, 'compile',
                    PARITY_TIMEOUT, stdin=request, stdout=compiled)
                if outcome['exit_code'] or not compiled.stat().st_size:
                    detail = f"batch-exit-{outcome['exit_code']}"
                    error = (directory / 'compile.log').read_text(errors='replace')
                    if error:
                        detail = re.sub(r'[\t\r\n]', ' ', error)[:512]
                    return harness_row(case, family, 'DEFECT', 'managed-compile', detail)
        return None


def runtime_identity(evaluator):
    """Authenticate ignored/generated compiler inputs as well as Cargo executables."""
    root = evaluator.root
    files = {}
    for key, artifact in evaluator.artifacts.items():
        files[f'binary:{key}'] = stamp(artifact['executable'])
    files['deno'] = stamp(evaluator.deno)
    for package in ('geosolve-intent', 'geosolve-sketch-code'):
        directory = root / 'packages' / package
        for subpath in ('dist', 'node_modules'):
            for path in sorted((directory / subpath).rglob('*')):
                if path.is_symlink():
                    files[str(path.relative_to(root))] = {'symlink': os.readlink(path)}
                elif path.is_file():
                    files[str(path.relative_to(root))] = stamp(path)
    return files


def source_identity(root):
    def git(*args):
        return subprocess.check_output(['git', *args], cwd=root).decode().strip()
    paths = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=root)
    digest = hashlib.sha256()
    for name in sorted(set(paths.decode().split('\0')) - {''}):
        path = root / name
        digest.update(name.encode() + b'\0')
        if path.is_symlink():
            digest.update(b'link:' + os.readlink(path).encode())
        elif path.is_file():
            digest.update(hashlib.sha256(path.read_bytes()).digest())
        else:
            digest.update(b'missing')
    return {'source': git('rev-parse', 'HEAD'), 'tree': git('rev-parse', 'HEAD^{tree}'),
            'status': git('status', '--porcelain=v1', '--untracked-files=all'),
            'working_input_sha256': digest.hexdigest()}


def prepare(root, output, processes, prepared_packages):
    preflight = output / 'preflight'
    preflight.mkdir()
    env = dict(os.environ)
    env['TMPDIR'] = str(output / 'tmp')
    Path(env['TMPDIR']).mkdir()
    def run(label, command, cwd=root, stdout=None, timeout=300):
        result = processes.run(command, cwd, env, preflight, label, timeout, stdout=stdout)
        if result['exit_code']:
            raise ValueError(f'{label} preflight failed; see {preflight / (label + ".log")}')
    metadata_file = preflight / 'metadata.json'
    run('metadata', ['cargo', 'metadata', '--locked', '--offline', '--no-deps',
                     '--format-version', '1'], stdout=metadata_file)
    metadata = json.loads(metadata_file.read_text())
    artifacts = {}
    targets = [('authoring', 'geosolve-constraint-editor', 'golden_authoring_oracle', 'test'),
               ('fillet', 'geosolve-constraint-editor', 'golden_fillet_oracle', 'test'),
               ('scene', 'geosolve-demo-web', 'geosolve_demo_web', 'lib'),
               ('parity', 'geosolve-sketch-code', 'golden_backend_parity', 'test')]
    for key, package, target, kind in targets:
        selector = ['-p', package, '--lib'] if kind == 'lib' else ['-p', package, '--test', target]
        stream = preflight / f'{key}-artifacts.jsonl'
        run(f'build-{key}', ['cargo', 'test', '--locked', *selector,
                             '--no-run', '--message-format=json'], stdout=stream, timeout=1800)
        rows = native.cargo_artifacts(stream.read_text(), metadata)
        expected = native.expected_targets(metadata, selector)
        actual = {(a['package'], a['target']['name'], tuple(a['target']['kind'])) for a in rows}
        if actual != expected or len(rows) != 1:
            raise ValueError(f'{key}: Cargo artifact inventory differs from metadata')
        artifact = rows[0]
        listing = preflight / f'{key}-listing.jsonl'
        run(f'list-{key}', ['cargo', 'test', '--locked', *selector, '-vv', '--message-format=json',
                            '--', '--list', '--format', 'terse'], stdout=listing)
        listed_json = '\n'.join(line for line in listing.read_text().splitlines() if line.startswith('{'))
        if native.cargo_artifacts(listed_json, metadata) != rows:
            raise ValueError(f'{key}: listing changed the prepared artifacts')
        launches = native.cargo_launches((preflight / f'list-{key}.log').read_text(), {artifact['executable']})
        artifact['env'] = launches[artifact['executable']]
        if artifact['env'].get('CARGO_MANIFEST_DIR') != artifact['cwd']:
            raise ValueError(f'{key}: Cargo launch package environment differs')
        artifact['sha256'] = native.file_hash(artifact['executable'])
        artifact['resource'] = 'native'
        for label, flags in [('cases', []), ('ignored', ['--ignored'])]:
            path = preflight / f'{key}-{label}.txt'
            outcome = processes.run([artifact['executable'], *flags, '--list', '--format', 'terse'],
                                    artifact['cwd'], env | artifact['env'], preflight,
                                    f'{key}-{label}', 30, stdout=path)
            if outcome['exit_code']:
                raise ValueError(f'{key}: failed direct test discovery')
            artifact[label] = native.parse_inventory(path.read_text())
        artifacts[key] = artifact
    deno = shutil.which('deno')
    if not deno:
        raise ValueError('pinned managed compiler requires deno on PATH')
    deno = str(Path(deno).resolve())
    evaluator = Evaluator(root, output, processes, artifacts, deno)
    for key, name in [('authoring', 'golden_oracle_inventory_and_tsv_schema_are_exhaustive'),
                      ('fillet', 'golden_fillet_oracle_inventory_and_tsv_schema_are_exhaustive'),
                      ('parity', 'golden_backend_parity_exclusion_ledger_is_explicit_and_reviewable')]:
        result = evaluator.test(key, name, env, preflight, f'inventory-{key}', 300)
        if result['exit_code']:
            raise ValueError(f'{key} inventory preflight failed')
    if not prepared_packages:
        for package in ('geosolve-intent', 'geosolve-sketch-code'):
            for label, args in [('install', ['ci', '--ignore-scripts']), ('build', ['run', 'build'])]:
                run(f'{package}-{label}', ['npm', *args], root / 'packages' / package)
    # The sidecar itself verifies exact Deno 2.9.4 and TypeScript 5.9.2.
    request = preflight / 'compiler.request.json'
    request.write_text('{"entries":[]}\n')
    compiled = preflight / 'compiler.output.json'
    result = processes.run([deno, *DENO_FLAGS], root / 'packages/geosolve-sketch-code', env,
                           preflight, 'compiler', 60, stdin=request, stdout=compiled)
    if result['exit_code'] or json.loads(compiled.read_text()) != {'entries': []}:
        raise ValueError('pinned managed compiler preflight failed')
    write_json(preflight / 'artifacts.json', artifacts)
    return evaluator, {'artifacts': artifacts, 'deno': {'path': deno, **stamp(deno)},
                       'prepared_packages': prepared_packages}


def evaluate_cases(evaluator, items, jobs):
    results = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as pool:
        futures = {pool.submit(evaluator.case, item): item for item in items}
        for future in concurrent.futures.as_completed(futures):
            item = futures[future]
            try:
                row = future.result()
            except Exception as error:
                # A harness failure in one case must not suppress remaining classifications.
                row = harness_row(*item, 'HARNESS_ERROR', 'runner', type(error).__name__)
                if hasattr(evaluator, 'output'):
                    directory = evaluator.output / 'cases' / item[0]
                    directory.mkdir(parents=True, exist_ok=True)
                    (directory / 'row.tsv').write_text(tsv([row]))
                    path = directory / 'case.json'
                    if not path.exists():
                        write_json(path, {'case_id': item[0], 'family': item[1], 'row': row,
                                          'harness_error': str(error)})
            results[item[0]] = row
    return [results[case] for case, _ in items]


def seal(output):
    return {str(path.relative_to(output)): stamp(path) for path in sorted(output.rglob('*')) if path.is_file()}


def validate_observation(output, fingerprints=True):
    output = Path(output).resolve()
    path = output / 'observation.json'
    receipt = json.loads(path.read_text())
    if receipt.get('schema') != SCHEMA or receipt.get('state') != 'complete':
        raise ValueError('observation lacks complete execution provenance')
    if receipt.get('inventory') != [list(item) for item in inventory()]:
        raise ValueError('captured observation inventory differs')
    if (not receipt.get('source_start') or receipt.get('source_start') != receipt.get('source_end') or
            not receipt.get('runtime_start') or receipt.get('runtime_start') != receipt.get('runtime_end')):
        raise ValueError('source or runtime changed during observation')
    if not {'actual.tsv', 'classified.tsv', 'golden.tsv'} <= set(receipt.get('files', {})):
        raise ValueError('observation lacks authenticated TSV files')
    required_cases = {f'cases/{case}/case.json' for case, _ in inventory()}
    if not required_cases <= set(receipt['files']):
        raise ValueError('observation lacks exact per-case completion receipts')
    for name, expected in receipt['files'].items():
        relative = Path(name)
        if relative.is_absolute() or '..' in relative.parts or stamp(output / relative) != expected:
            raise ValueError(f'captured evidence changed: {name}')
    rows = parse_rows((output / 'actual.tsv').read_text(), inventory())
    for row in rows:
        case = json.loads((output / 'cases' / row[0] / 'case.json').read_text())
        if case.get('case_id') != row[0] or case.get('family') != row[1] or case.get('row') != row:
            raise ValueError(f'case completion differs from aggregated observation: {row[0]}')
    golden = (output / 'golden.tsv').read_text()
    golden_rows = parse_rows(golden, inventory(), reviewed=True, fingerprints=fingerprints)
    classified = tsv(classify(rows, golden_rows))
    if classified != (output / 'classified.tsv').read_text():
        raise ValueError('captured classification differs from its authenticated raw observation')
    return receipt, classified, golden


def disposition(mode, output, golden_path=GOLDEN):
    receipt, classified, captured_golden = validate_observation(output, fingerprints=mode != 'survey')
    if mode == 'survey':
        print(classified, end='')
        return 0
    golden = golden_path.read_text()
    parse_rows(golden, inventory(), reviewed=True)
    if golden != captured_golden:
        raise ValueError('reviewed golden changed since observation; create a fresh observation')
    if golden != classified:
        print('oracle result differs from the recorded checklist:', file=sys.stderr)
        print(''.join(difflib.unified_diff(golden.splitlines(True), classified.splitlines(True),
                                        fromfile=str(golden_path), tofile=str(output / 'classified.tsv'))), file=sys.stderr)
        return 1
    if mode == 'require-clean' and any(row[2] != 'PASS' for row in parse_rows(classified)):
        print('oracle contains known defects or harness failures', file=sys.stderr)
        return 1
    print(f'oracle checklist matches: {golden_path}')
    print(f'captured observation: {output / "observation.json"}; original start {receipt["start_utc"]}', file=sys.stderr)
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    for mode in ('survey', 'check', 'require-clean'):
        modes.add_argument('--' + mode, dest='mode', action='store_const', const=mode)
    parser.add_argument('--jobs', type=int, default=2)
    parser.add_argument('--output-dir', type=Path)
    parser.add_argument('--prepared-packages', action='store_true')
    parser.add_argument('--from-observation', type=Path,
                        help='validate a complete captured execution; does not execute tests')
    args = parser.parse_args(argv)
    if not 1 <= args.jobs <= 32:
        parser.error('--jobs must be between 1 and 32')
    if args.from_observation:
        if args.output_dir or args.prepared_packages:
            parser.error('--from-observation cannot prepare packages or create an execution directory')
        try:
            return disposition(args.mode, args.from_observation.resolve())
        except (OSError, ValueError, KeyError) as error:
            print(f'oracle capture validation failed: {error}', file=sys.stderr)
            return 1
    if args.output_dir:
        output = args.output_dir.resolve()
        output.mkdir(parents=True, exist_ok=False)
    else:
        parent = ROOT / 'target/golden-authoring-scene-oracle'
        parent.mkdir(parents=True, exist_ok=True)
        output = Path(tempfile.mkdtemp(prefix='run.', dir=parent))
    cancel = threading.Event()
    processes = ProcessRunner(cancel)
    previous = {}
    for sig in (signal.SIGTERM, signal.SIGINT):
        previous[sig] = signal.signal(sig, lambda _sig, _frame: cancel.set())
    start, identity = utc(), source_identity(ROOT)
    print(f'oracle evidence: {output}', file=sys.stderr)
    try:
        golden = GOLDEN.read_text()
        golden_rows = parse_rows(golden, inventory(), reviewed=True, fingerprints=args.mode != 'survey')
        (output / 'golden.tsv').write_text(golden)
        evaluator, preparation = prepare(ROOT, output, processes, args.prepared_packages)
        runtime_start = runtime_identity(evaluator)
        rows = evaluate_cases(evaluator, inventory(), args.jobs)
        (output / 'actual.tsv').write_text(tsv(rows))
        (output / 'classified.tsv').write_text(tsv(classify(rows, golden_rows)))
        end_identity, runtime_end = source_identity(ROOT), runtime_identity(evaluator)
        state = ('interrupted' if cancel.is_set() else 'complete' if
                 identity == end_identity and runtime_start == runtime_end else 'source-changed')
        write_json(output / 'observation.json', {'schema': SCHEMA, 'state': state,
            'start_utc': start, 'end_utc': utc(), 'source_start': identity, 'source_end': end_identity,
            'inventory': inventory(), 'jobs': args.jobs, 'preparation': preparation,
            'runtime_start': runtime_start, 'runtime_end': runtime_end, 'files': seal(output)})
        if state != 'complete':
            raise ValueError(f'no complete observation: {state}')
        return disposition(args.mode, output)
    except (OSError, ValueError, KeyError, native.NativeError) as error:
        write_json(output / 'failure.json', {'state': 'interrupted' if cancel.is_set() else 'failed',
                   'start_utc': start, 'end_utc': utc(), 'error': str(error)})
        print(f'oracle evaluation failed: {error}; evidence retained at {output}', file=sys.stderr)
        return 130 if cancel.is_set() else 1
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


if __name__ == '__main__':
    raise SystemExit(main())
