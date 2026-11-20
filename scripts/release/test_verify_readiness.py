"""Synthetic release evidence tests, not actual production acceptance."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("release_verify", Path(__file__).with_name("verify-readiness.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
COMMIT = "a" * 40


class ReleaseContractTests(unittest.TestCase):
    def fixture(self, directory):
        root = Path(directory)
        evidence = root / "synthetic-evidence.txt"
        evidence.write_text("Synthetic contract test: NOT PHYSICAL TEST DATA.\n")
        digest = hashlib.sha256(evidence.read_bytes()).hexdigest()
        manifest = {
            "schema_version": 1, "operator": "synthetic-operator", "candidate_commit": COMMIT,
            "supported_targets": ["VYOM-native-Wayland"],
            "gates": {name: {
                "status": "PASS", "reviewer": "synthetic-independent-reviewer",
                "reviewed_at": "2026-10-19T17:00:00Z",
                "evidence": [{"path": evidence.name, "sha256": digest}],
            } for name in module.REQUIRED_GATES}
        }
        p = root / "manifest.json"
        p.write_text(json.dumps(manifest))
        return p, manifest, evidence

    def test_complete_contract_passes_not_certifies(self):
        with tempfile.TemporaryDirectory() as d:
            path, _, _ = self.fixture(d)
            self.assertIn("not_independent", module.verify(path, COMMIT)["status"])

    def test_missing_gate_is_no_go(self):
        with tempfile.TemporaryDirectory() as d:
            p, m, _ = self.fixture(d)
            del m["gates"][module.REQUIRED_GATES[0]]
            p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError, "missing"):
                module.verify(p, COMMIT)

    def test_unreviewed_gate_is_no_go(self):
        with tempfile.TemporaryDirectory() as d:
            p, m, _ = self.fixture(d)
            m["gates"]["real_screen_reader"]["reviewer"] = "synthetic-operator"
            p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError, "independent"):
                module.verify(p, COMMIT)

    def test_evidence_tampering_is_no_go(self):
        with tempfile.TemporaryDirectory() as d:
            p, _, evidence = self.fixture(d)
            evidence.write_text("Changed")
            with self.assertRaisesRegex(ValueError, "mismatched evidence"):
                module.verify(p, COMMIT)

    def test_operator_cannot_self_review_using_case_or_whitespace(self):
        with tempfile.TemporaryDirectory() as d:
            p, m, _ = self.fixture(d)
            m["operator"] = " synthetic-operator "
            m["gates"]["real_screen_reader"]["reviewer"] = "SYNTHETIC-OPERATOR"
            p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError, "independent"):
                module.verify(p, COMMIT)

    def test_commit_mismatch_is_no_go(self):
        with tempfile.TemporaryDirectory() as d:
            p, _, _ = self.fixture(d)
            with self.assertRaisesRegex(ValueError, "differs"):
                module.verify(p, "b" * 40)

    def test_missing_actual_target_is_no_go(self):
        with tempfile.TemporaryDirectory() as d:
            p, m, _ = self.fixture(d)
            m["supported_targets"] = ["generic-winit"]
            p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError, "VYOM"):
                module.verify(p, COMMIT)


if __name__ == "__main__":
    unittest.main()
