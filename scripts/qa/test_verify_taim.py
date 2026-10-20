"""Unit tests for TAIM's manual verification runner; no Cargo required."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "taim_verifier", Path(__file__).with_name("verify-taim.py"),
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class LocalVerificationTests(unittest.TestCase):
    def test_missing_tool_is_always_no_go(self):
        passing = [
            {"status": "PASS"} for _ in module.commands()
        ]
        self.assertEqual(module.classify(passing, False), "NO_GO")

    def test_partial_success_never_marks_release_ready(self):
        passing = [{"status": "PASS"}]
        self.assertEqual(module.classify(passing, True), "NO_GO")
        self.assertEqual(module.classify([], True), "NO_GO")

    def test_failed_step_blocks_success(self):
        passing = [{"status": "PASS"} for _ in module.commands()]
        passing[-1]["status"] = "FAIL"
        self.assertEqual(module.classify(passing, True), "NO_GO")

    def test_complete_run_still_requires_human_approval(self):
        passing = [{"status": "PASS"} for _ in module.commands()]
        status = module.classify(passing, True)
        self.assertIn("HUMAN_ORIGIN_AND_DEVICE_REVIEW_REQUIRED", status)
        self.assertNotIn("PRODUCTION_READY", status)

    def test_strict_source_audit_is_mandatory(self):
        name, cmd = module.commands()[0]
        self.assertEqual(name, "source_lineage")
        self.assertIn("--strict", cmd)


if __name__ == "__main__":
    unittest.main()
