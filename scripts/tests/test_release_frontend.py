# SPDX-License-Identifier: GPL-3.0-or-later
"""Frontend result reuse never drops catalog-sensitive preflight obligations."""
import copy
import dataclasses
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate as gate
import release_equivalence as eq


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
        self.assertEqual(commands.count(gate.npm(gate.FRONTEND, 'test', '--', '--no-cache')), 1)
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


if __name__ == '__main__':
    unittest.main()
