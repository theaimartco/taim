import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("sbom", Path(__file__).with_name("sbom.py"))
sbom = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sbom)


class SbomTests(unittest.TestCase):
    def test_distinct_sources_edges_and_locked_registry_checksums_are_preserved(self):
        registry = "registry+https://github.com/rust-lang/crates.io-index"
        metadata = {"packages": [
            {"id": "app", "name": "app", "version": "1", "source": None, "license": "MIT"},
            {"id": "remote", "name": "dep", "version": "1", "source": registry, "license": "MIT OR Apache-2.0"},
            {"id": "local", "name": "dep", "version": "1", "source": None, "license": None}],
            "workspace_members": ["app"], "resolve": {"nodes": [
                {"id": "app", "dependencies": ["local", "remote", "remote"]}]}}
        lock = {"package": [{"name": "dep", "version": "1", "source": registry, "checksum": "a" * 64}]}
        result = sbom.build(metadata, lock, "2026-11-01T00:00:00Z")
        self.assertEqual(result["spdxVersion"], "SPDX-2.3")
        packages = result["packages"]
        self.assertEqual(len({p["SPDXID"] for p in packages}), 3)
        self.assertEqual(sum("checksums" in p for p in packages), 1)
        self.assertEqual(sum("externalRefs" in p for p in packages), 1)
        self.assertEqual(len(result["relationships"]), 3)
        self.assertTrue(all(not p["filesAnalyzed"] for p in packages))
        known = {p["SPDXID"] for p in packages} | {"SPDXRef-DOCUMENT"}
        for edge in result["relationships"]:
            self.assertIn(edge["spdxElementId"], known)
            self.assertIn(edge["relatedSpdxElement"], known)

    def test_empty_graph_does_not_invent_dependencies_or_release_attestation(self):
        result = sbom.build({"packages": [], "resolve": None}, {}, "2026-11-01T00:00:00Z")
        self.assertEqual(result["packages"], [])
        self.assertEqual(result["relationships"], [])
        self.assertIn("Not an APK byte inventory", result["documentComment"])


if __name__ == "__main__": unittest.main()
