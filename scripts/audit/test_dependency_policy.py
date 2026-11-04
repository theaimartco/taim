"""The maintenance exception must fail closed when its reviewed scope changes."""
import copy
from datetime import date
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("dependency_policy", Path(__file__).with_name("check-dependency-policy.py"))
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


class DependencyPolicyTests(unittest.TestCase):
    def setUp(self):
        self.today = date(2026, 10, 26)
        self.paste = {"id": "paste", "name": "paste", "version": "1.0.15", "source": policy.REGISTRY, "targets": [{"kind": ["proc-macro"]}]}
        self.parent = {"id": "pulp", "name": "pulp", "version": "0.22.3", "source": policy.REGISTRY}
        self.metadata = {"packages": [self.paste, self.parent], "resolve": {"nodes": [{"id": "pulp", "deps": [{"pkg": "paste"}]}]}}
        self.lock = {"package": [{"name": "paste", "version": "1.0.15", "source": policy.REGISTRY, "checksum": policy.CHECKSUM}]}

    def check(self):
        return policy.validate(self.metadata, self.lock, self.today)

    def test_reviewed_macro_is_permitted_only_before_review_deadline(self):
        self.assertEqual(self.check(), [])
        self.assertTrue(policy.validate(self.metadata, self.lock, policy.REVIEW_BY))

    def test_direct_workspace_or_upgraded_consumer_is_not_permitted(self):
        for changes in [{"name": "taim"}, {"version": "0.22.4"}, {"source": None}]:
            with self.subTest(changes=changes):
                original = copy.deepcopy(self.parent)
                self.parent.update(changes)
                self.assertTrue(self.check())
                self.parent.update(original)

    def test_altered_artifact_or_unexpected_macro_version_is_rejected(self):
        for field in ["checksum", "version", "source"]:
            lock = copy.deepcopy(self.lock)
            lock["package"][0][field] = "unreviewed"
            self.assertTrue(policy.validate(self.metadata, lock, self.today))
        self.paste["version"] = "1.0.16"
        self.assertTrue(self.check())

    def test_macro_cannot_become_a_runtime_library(self):
        for kind in ["lib", "rlib", "dylib", "cdylib", "staticlib"]:
            self.paste["targets"] = [{"kind": ["proc-macro"]}, {"kind": [kind]}]
            self.assertTrue(self.check())
        self.paste["targets"] = []
        self.assertTrue(self.check())

    def test_duplicate_macro_or_missing_resolved_edges_is_rejected(self):
        self.metadata["packages"].append(copy.deepcopy(self.paste))
        self.assertTrue(self.check())
        self.metadata["packages"].pop()
        self.metadata["resolve"]["nodes"] = []
        self.assertTrue(self.check())

    def test_exception_cannot_cover_a_security_advisory_amendment(self):
        advisory = {"id": "RUSTSEC-2024-0436", "package": "paste", "informational": "unmaintained"}
        self.assertEqual(policy.validate_advisory(advisory), [])
        for changes in [{"informational": None}, {"informational": "unsound"}, {"cvss": "CVSS:3.1/AV:N"}, {"id": "RUSTSEC-2027-0001"}]:
            changed = {**advisory, **changes}
            self.assertTrue(policy.validate_advisory(changed))

    def test_removing_macro_entirely_does_not_require_preserving_it(self):
        self.metadata["packages"] = [self.parent]
        self.assertEqual(self.check(), [])


if __name__ == "__main__":
    unittest.main()
