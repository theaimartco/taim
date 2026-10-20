"""Synthetic exact Git-tree coverage; tests cannot certify originality."""
import importlib.util
from pathlib import Path
import unittest
spec = importlib.util.spec_from_file_location("fork", Path(__file__).with_name("exact-fork-blobs.py"))
fork = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fork)

class ExactForkBlobTests(unittest.TestCase):
    def test_parse_binary_tree_and_relocated_copy(self):
        data = b"100644 blob deadbeef\torigin.rs\0" + b"100644 blob cafe1234\treadme.md\0"
        upstream = fork.parse_tree(data)
        self.assertEqual(upstream["origin.rs"], "deadbeef")
        current = {"new/engine.rs": "deadbeef", "docs/readme.md": "cafe1234"}
        result = fork.exact_matches(upstream, current)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["identity"], "relocated_exact_copy")

    def test_same_path_inherited_blob_detected(self):
        xs = fork.exact_matches({"kernel.wgsl": "abc"}, {"kernel.wgsl": "abc"})
        self.assertEqual(len(xs), 1)
        self.assertEqual(xs[0]["identity"], "same_path")

    def test_modified_content_not_mislabeled_identical(self):
        self.assertEqual(fork.exact_matches({"core.rs": "abc"}, {"core.rs": "def"}), [])

    def test_non_source_file_excluded(self):
        self.assertEqual(fork.exact_matches({"theme.png": "abc"}, {"theme.png": "abc"}), [])

    def test_duplicate_origin_paths_included(self):
        xs = fork.exact_matches(
            {"alpha.rs": "abc", "beta.rs": "abc"}, {"gamma.rs": "abc"}
        )
        self.assertEqual(xs[0]["upstream_paths"], ["alpha.rs", "beta.rs"])

if __name__ == "__main__":
    unittest.main()
