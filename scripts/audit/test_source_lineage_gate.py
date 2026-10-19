"""Synthetic source gate tests; passing does not certify independent authorship."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "source_gate", Path(__file__).with_name("source-lineage-gate.py"))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)

def report(overrides=None):
    values = dict(total_files=1, total_loc=200, byte_identical_files=0,
                  token_identical_files=0, high_similarity_files=0,
                  disguised_copies=0, inherited_loc_token=0)
    values.update(overrides or {})
    return {"summary": values, "files": [{"path": "custom.rs"}]}

class SourceGateTests(unittest.TestCase):
    def test_clean_means_review_not_originality_certification(self):
        result = gate.assess(report())
        self.assertIn("REVIEW_REQUIRED", result["status"])
        self.assertTrue(result["manual_review_required"])

    def test_inherited_files_block(self):
        self.assertEqual(gate.assess(report({"byte_identical_files": 1}))["status"], "NO_GO")

    def test_high_similarity_blocks(self):
        self.assertEqual(gate.assess(report({"high_similarity_files": 2}))["status"], "NO_GO")

    def test_inherited_lines_block(self):
        self.assertEqual(gate.assess(report({"inherited_loc_token": 24}))["status"], "NO_GO")

    def test_missing_inventory_blocks(self):
        x = report()
        x["files"] = []
        with self.assertRaisesRegex(ValueError, "incomplete"):
            gate.assess(x)

    def test_missing_metric_blocks(self):
        x = report()
        del x["summary"]["disguised_copies"]
        with self.assertRaisesRegex(ValueError, "metric"):
            gate.assess(x)

if __name__ == "__main__":
    unittest.main()
