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
            {"name": name, "status": "PASS"} for name, _ in module.commands()
        ]
        self.assertEqual(module.classify(passing, False), "NO_GO")

    def test_partial_success_never_marks_release_ready(self):
        passing = [{"status": "PASS"}]
        self.assertEqual(module.classify(passing, True), "NO_GO")
        self.assertEqual(module.classify([], True), "NO_GO")

    def test_failed_step_blocks_success(self):
        passing = [{"name": name, "status": "PASS"} for name, _ in module.commands()]
        passing[-1]["status"] = "FAIL"
        self.assertEqual(module.classify(passing, True), "NO_GO")

    def test_complete_run_still_requires_human_approval(self):
        passing = [{"name": name, "status": "PASS"} for name, _ in module.commands()]
        status = module.classify(passing, True)
        self.assertIn("HUMAN_ORIGIN_AND_DEVICE_REVIEW_REQUIRED", status)
        self.assertNotIn("PRODUCTION_READY", status)

    def test_fabricated_gate_name_blocks_verification(self):
        passing = [
            {"name": name, "status": "PASS"} for name, _ in module.commands()
        ]
        self.assertIn("CODE_CHECKS_PASSED", module.classify(passing, True))
        passing[-1]["name"] = "invented_gate"
        self.assertEqual(module.classify(passing, True), "NO_GO")

    def test_release_requirements_are_in_local_gate(self):
        steps = dict(module.commands())
        self.assertIn("external_consumer", steps)
        self.assertIn("release_contract_tests", steps)
        self.assertIn("cpu_golden_tests", steps)
        self.assertEqual(steps["production_tests"], ["cargo", "test", "--workspace", "--locked"])
        self.assertIn("frame_time_contract", steps)
        self.assertIn("license_and_advisory_check", steps)
        self.assertIn("dependency_maintenance_policy", steps)

    def test_strict_source_audit_is_mandatory(self):
        name, cmd = module.commands()[0]
        self.assertEqual(name, "source_lineage")
        self.assertIn("--strict", cmd)


if __name__ == "__main__":
    unittest.main()
