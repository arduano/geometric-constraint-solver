# SPDX-License-Identifier: GPL-3.0-or-later
"""Program edits never implicitly reapprove a catalog non-interference contract."""
import copy
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate as gate
import release_equivalence as eq


class ReviewedEquivalenceTests(unittest.TestCase):
    def setUp(self):
        self.policy = {"global": [eq.CONTRACT_PATH], "owned": ["crates/**"], "prose": ["docs/**"]}
        self.sample = eq.SAMPLE_ROOT + "/one/sketch.ts"
        self.program = "crates/example/src/lib.rs"
        self.inputs = {self.sample: {"sha256": "a", "executable": False},
                       self.program: {"sha256": "b", "executable": False}}
        self.contract = {"schema": 1, "browser": {"program_sha256": gate.digest(eq.partition(self.inputs, self.policy))}}

    def test_data_edit_and_prune_preserve_only_explicitly_reviewed_program(self):
        for changed in ({"sha256": "new", "executable": False}, None):
            inputs = dict(self.inputs, **{self.sample: changed})
            self.assertTrue(eq.reviewed("browser", inputs, self.policy, self.contract))
        inputs = dict(self.inputs)
        inputs.pop(self.sample)
        self.assertTrue(eq.reviewed("browser", inputs, self.policy, self.contract))
        inputs[self.program] = {"sha256": "unreviewed program", "executable": False}
        self.assertFalse(eq.reviewed("browser", inputs, self.policy, self.contract))

    def test_new_program_global_unknown_and_symlink_data_are_retained(self):
        changed = dict(self.inputs)
        changed[eq.SAMPLE_ROOT + "/two/shared.rs"] = {"sha256": "program", "executable": False}
        self.assertFalse(eq.reviewed("browser", changed, self.policy, self.contract))
        for value in ({"link": "../../code", "target": "a"}, {"sha256": "a", "executable": True}):
            changed = dict(self.inputs, **{self.sample: value})
            self.assertFalse(eq.reviewed("browser", changed, self.policy, self.contract))
        policy = copy.deepcopy(self.policy)
        policy["global"].append(self.sample)
        self.assertFalse(eq.reviewed("browser", self.inputs, policy, self.contract))
        policy["global"].remove(self.sample)
        policy["owned"] = ["crates/example/**"]
        self.assertFalse(eq.reviewed("browser", self.inputs, policy, self.contract))

    def test_missing_invalid_and_stale_contracts_fail_closed(self):
        for value in (None, [], {}, {"schema": 2}, {"schema": 1, "browser": {"program_sha256": "stale"}}):
            self.assertFalse(eq.reviewed("browser", self.inputs, self.policy, value))

    def test_contract_has_no_digest_self_reference(self):
        changed = dict(self.inputs, **{eq.CONTRACT_PATH: {"sha256": "reviewed approval", "executable": False}})
        self.assertTrue(eq.reviewed("browser", changed, self.policy, self.contract))

    def test_golden_exclusion_requires_review_and_resolved_input_closure(self):
        contract = {"schema": 1, "golden": self.contract["browser"]}
        stage = gate.Stage("golden", (("true",),), inputs=("crates/**",),
                           excluded_inputs=(eq.SAMPLE_ROOT + "/**",), input_equivalence="golden",
                           input_equivalence_contract=contract)
        self.assertNotIn(self.sample, gate.stage_inputs(stage, self.inputs, self.policy))
        import dataclasses
        unresolved = dataclasses.replace(stage, inputs=("**",))
        self.assertIn(self.sample, gate.stage_inputs(unresolved, self.inputs, self.policy))
        stale = dataclasses.replace(stage, input_equivalence_contract=None)
        self.assertIn(self.sample, gate.stage_inputs(stale, self.inputs, self.policy))


if __name__ == "__main__":
    unittest.main()
