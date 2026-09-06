# SPDX-License-Identifier: GPL-3.0-or-later
"""Reject WASM build outputs that would silently change selected release coverage."""
import copy
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate_wasm as wasm
from release_gate_native import NativeError


class LifecyclePreparationTests(unittest.TestCase):
    def setUp(self):
        self.metadata = {'packages': [{'name': 'geosolve-demo-web', 'id': 'owner'}], 'workspace_members': ['owner']}
        self.artifact = {'reason': 'compiler-artifact', 'package_id': 'owner',
                         'target': {'name': 'geosolve_demo_web', 'kind': ['cdylib', 'rlib']},
                         'features': [], 'executable': '/target/wasm32-unknown-unknown/release/deps/demo.wasm',
                         'profile': {'test': True, 'opt_level': '3', 'debug_assertions': False,
                                     'overflow_checks': False}}

    def records(self, *artifacts, success=True):
        return '\n'.join(json.dumps(v) for v in [*artifacts, {'reason': 'build-finished', 'success': success}])

    def test_only_exact_default_feature_release_lib_is_selected(self):
        self.assertEqual(wasm.select_artifact(self.records(self.artifact), self.metadata), self.artifact)
        for field, value in [('package_id', 'wrong-owner'), ('features', ['unexpected']), ('executable', '/native-test')]:
            with self.subTest(field=field):
                candidate = copy.deepcopy(self.artifact)
                candidate[field] = value
                with self.assertRaises(NativeError):
                    wasm.select_artifact(self.records(candidate), self.metadata)
        for field, value in [('opt_level', '1'), ('debug_assertions', True), ('overflow_checks', True)]:
            with self.subTest(profile=field):
                candidate = copy.deepcopy(self.artifact)
                candidate['profile'][field] = value
                with self.assertRaises(NativeError):
                    wasm.select_artifact(self.records(candidate), self.metadata)

    def test_cargo_freshness_does_not_change_captured_semantic_identity(self):
        cold = self.artifact | {'fresh': False}
        warm = self.artifact | {'fresh': True}
        self.assertEqual(wasm.select_artifact(self.records(cold), self.metadata),
                         wasm.select_artifact(self.records(warm), self.metadata))

    def test_cargo_launch_keeps_package_variables_and_rejects_argument_drift(self):
        text = "Running `CARGO_MANIFEST_DIR='/source with space' CARGO_MANIFEST_PATH='/source with space/Cargo.toml' LD_LIBRARY_PATH=/libs runner /test.wasm actual_wasm_ --list`"
        with patch.object(wasm.shutil, 'which', return_value='/bin/runner'):
            env = wasm.launch_environment(text, '/test.wasm', '/bin/runner')
            self.assertEqual(env['CARGO_MANIFEST_DIR'], '/source with space')
            self.assertEqual(env['LD_LIBRARY_PATH'], '/libs')
            for bad in (text + '\n' + text, text.replace('--list', '--ignored'),
                        text.replace(' actual_wasm_', ''), text.replace('CARGO_MANIFEST_DIR', 'UNEXPECTED')):
                with self.subTest(bad=bad), self.assertRaises(NativeError):
                    wasm.launch_environment(bad, '/test.wasm', '/bin/runner')

    def test_missing_duplicate_or_failed_build_never_prepares_tests(self):
        for text in (self.records(), self.records(self.artifact, self.artifact),
                     self.records(self.artifact, success=False), json.dumps(self.artifact)):
            with self.subTest(text=text):
                with self.assertRaises(NativeError):
                    wasm.select_artifact(text, self.metadata)


if __name__ == '__main__':
    unittest.main()
