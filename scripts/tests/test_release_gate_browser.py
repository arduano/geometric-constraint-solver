# SPDX-License-Identifier: GPL-3.0-or-later
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release_gate as gate


class BrowserCoverageTests(unittest.TestCase):
    def setUp(self):
        self.report = {"suites": [{"specs": [{"title": "sample history", "file": "sample.spec.ts", "tests": [{
            "projectName": "chromium", "expectedStatus": "passed", "status": "expected",
            "results": [{"status": "passed", "retry": 0, "errors": []}]}]}]}], "errors": []}

    def test_exact_fresh_execution_passes(self):
        self.assertEqual(len(gate.validate_browser_report(self.report, self.report)), 1)

    def test_missing_duplicate_renamed_and_empty_inventories_fail(self):
        for change in ("missing", "duplicate", "renamed", "empty"):
            altered = copy.deepcopy(self.report)
            if change == "missing":
                altered["suites"][0]["specs"][0]["tests"] = []
            elif change == "duplicate":
                altered["suites"].append(copy.deepcopy(altered["suites"][0]))
            elif change == "renamed":
                altered["suites"][0]["specs"][0]["title"] = "another test"
            else:
                altered["suites"] = []
            with self.subTest(change=change), self.assertRaises(ValueError):
                gate.validate_browser_report(self.report, altered)

    def test_skips_retries_expected_failures_and_flakes_never_supply_success(self):
        for key, value in (("expectedStatus", "failed"), ("status", "flaky"), ("results", []),
                           ("results", [{"status": "skipped"}]), ("results", [{"status": "passed", "retry": 1}]),
                           ("results", [{"status": "passed"}, {"status": "passed"}])):
            altered = copy.deepcopy(self.report)
            altered["suites"][0]["specs"][0]["tests"][0][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                gate.validate_browser_report(self.report, altered)

    def test_global_harness_error_rejects_green_test_rows(self):
        altered = copy.deepcopy(self.report)
        altered["errors"] = [{"message": "server cleanup failed"}]
        with self.assertRaises(ValueError):
            gate.validate_browser_report(self.report, altered)


if __name__ == "__main__":
    unittest.main()
