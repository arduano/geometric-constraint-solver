#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Prepare the existing optimized-WASM lifecycle tests for independent execution."""
from __future__ import annotations
import json
import os
from pathlib import Path
import shutil
import re
import shlex

SCHEMA = 'geosolve-wasm-lifecycle-preparation-v1'


def select_artifact(text, metadata):
    """Accept only the one requested Cargo lib-test and its explicit release profile."""
    from release_gate_native import require
    packages = [item for item in metadata['packages'] if item['name'] == 'geosolve-demo-web'
                and item['id'] in metadata['workspace_members']]
    require(len(packages) == 1, 'WASM lifecycle owner missing or duplicated')
    records = [json.loads(line) for line in text.splitlines()]
    require([r.get('success') for r in records if r.get('reason') == 'build-finished'] == [True],
            'WASM preparation lacks one successful build completion')
    tests = [r for r in records if r.get('reason') == 'compiler-artifact'
             and r.get('profile', {}).get('test') and r.get('executable')]
    require(len(tests) == 1, 'WASM lifecycle preparation produced unexpected test executables')
    artifact = tests[0]
    require(artifact['package_id'] == packages[0]['id']
            and artifact['target']['name'] == 'geosolve_demo_web'
            and set(artifact['target']['kind']) == {'cdylib', 'rlib'}
            and Path(artifact['executable']).suffix == '.wasm', 'wrong WASM lifecycle executable')
    require(artifact['features'] == [], 'WASM lifecycle must retain the default-feature selection')
    require(artifact['profile']['opt_level'] == '3' and artifact['profile']['debug_assertions'] is False
            and artifact['profile']['overflow_checks'] is False,
            'WASM lifecycle must retain the original optimized release profile')
    # Cargo freshness is build-cache bookkeeping, not an executable/profile input.
    return {key: value for key, value in artifact.items() if key != "fresh"}


def launch_environment(stderr, executable, runner):
    """Capture Cargo-owned package/runtime variables without interpreting shell text."""
    from release_gate_native import require
    launches = []
    for line in stderr.splitlines():
        match = re.fullmatch(r"\s*Running `(.+)`", line)
        if not match:
            continue
        tokens = shlex.split(match[1])
        environment = {}
        while tokens and re.match(r"^[A-Za-z_][A-Za-z_0-9]*=", tokens[0]):
            name, value = tokens.pop(0).split("=", 1)
            environment[name] = value
        if executable not in tokens:
            continue
        require(len(tokens) == 4 and tokens[1:] == [executable, "actual_wasm_", "--list"]
                and shutil.which(tokens[0]) == runner, "unexpected Cargo WASM test launch")
        require("CARGO_MANIFEST_DIR" in environment and "CARGO_MANIFEST_PATH" in environment,
                "Cargo WASM launch lacks package environment")
        launches.append(environment)
    require(len(launches) == 1, "Cargo WASM test launch missing or duplicated")
    return launches[0]


def prepare(root, output):
    from release_gate_native import checked_process, file_hash, parse_inventory, require, write_json
    root, output = Path(root).resolve(), Path(output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    environment = dict(os.environ)
    metadata_text = checked_process(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1'],
                                    root, environment, output / 'metadata', 120)
    command = ['cargo', 'test', '--locked', '--release', '-p', 'geosolve-demo-web', '--lib', 'actual_wasm_',
               '--target', 'wasm32-unknown-unknown', '--no-run', '--message-format=json']
    compiled = checked_process(command, root, environment, output / 'build', 2300)
    artifact = select_artifact(compiled, json.loads(metadata_text))
    original = Path(artifact['executable'])
    before = file_hash(original)
    executable = output / 'lifecycle.wasm'
    shutil.copyfile(original, executable)
    require(file_hash(executable) == before and file_hash(original) == before,
            'WASM lifecycle executable changed during capture')
    runner = shutil.which('wasm-bindgen-test-runner')
    require(bool(runner), 'missing pinned wasm-bindgen-test-runner')
    discovery_command = ['cargo', 'test', '--locked', '--release', '-p', 'geosolve-demo-web', '--lib',
                         'actual_wasm_', '--target', 'wasm32-unknown-unknown', '-vv', '--message-format=json',
                         '--', '--list']
    environment['CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER'] = runner
    discovered = checked_process(discovery_command, root, environment, output / 'cargo-list', 2300)
    rediscovered = select_artifact('\n'.join(line for line in discovered.splitlines() if line.startswith('{')),
                                   json.loads(metadata_text))
    require(rediscovered == artifact, 'WASM Cargo discovery changed the executable/profile/features')
    require(file_hash(original) == before and file_hash(executable) == before,
            'WASM lifecycle executable changed during discovery')
    overrides = launch_environment((output / 'cargo-list/stderr.log').read_text(), str(original), runner)
    command = [runner, str(executable), 'actual_wasm_']
    listing = checked_process([*command, '--list'], root, environment | overrides, output / 'list', 60)
    cases = parse_inventory(listing)
    expected = json.loads((root / 'scripts/release_test_inventory.json').read_text())['stages']['wasm.lifecycle']
    require(sorted(cases) == sorted(expected), 'WASM lifecycle discovery differs from reviewed inventory')
    write_json(output / 'prepared.json', {'schema': SCHEMA, 'command': command, 'cases': sorted(cases),
               'profile': artifact['profile'], 'features': artifact['features'], 'env': overrides,
               'sha256': before, 'original': str(original), 'executable': str(executable),
               'runner_sha256': file_hash(runner), 'cargo_artifact': artifact})
