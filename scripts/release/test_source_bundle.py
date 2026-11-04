"""Verify source artifacts are deterministic, complete and cannot read outside the root."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("source_bundle", Path(__file__).with_name("build-source-bundle.py"))
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


class SourceBundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.names = ["Cargo.toml", "Cargo.lock", "LICENSE", "CREDITS.md", "rust-toolchain.toml", "src/lib.rs"]
        for name in self.names:
            path = self.root / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(name.encode())

    def write(self, output="source.tar.gz", names=None):
        return bundle.write_bundle(self.root, self.names if names is None else names,
                                   self.root / output, "0.15.0-dev", "a" * 40, True, 1)

    def test_archive_is_reproducible_despite_inventory_order_and_source_mtime(self):
        self.write()
        os.utime(self.root / "src/lib.rs", (9000, 9000))
        self.write("again.tar.gz", list(reversed(self.names)))
        self.assertEqual((self.root / "source.tar.gz").read_bytes(), (self.root / "again.tar.gz").read_bytes())

    def test_manifest_matches_the_exact_archived_bytes_and_preserves_attribution(self):
        manifest = self.write()
        with tarfile.open(self.root / "source.tar.gz") as archive:
            prefix = "taim-0.15.0-dev/"
            embedded = json.load(archive.extractfile(prefix + "TAIM-SOURCE.json"))
            self.assertEqual(embedded, manifest)
            self.assertTrue(embedded["working_tree_modified"])
            self.assertFalse(embedded["production_release_approved"])
            self.assertEqual({entry["path"] for entry in embedded["files"]}, set(self.names))
            for entry in embedded["files"]:
                data = archive.extractfile(prefix + entry["path"]).read()
                self.assertEqual(hashlib.sha256(data).hexdigest(), entry["sha256"])
                member = archive.getmember(prefix + entry["path"])
                self.assertEqual((member.uid, member.gid, member.mtime), (0, 0, 1))

    def test_local_state_secret_files_and_traversal_are_not_distributable(self):
        for name in ["../secret", "/etc/passwd", ".env", ".env.production", ".aws/credentials", ".codex/config.toml", "target/cache", "artifacts/log.json", "winit/.DS_Store", "scripts/__pycache__/a.pyc", "private.key"]:
            self.assertFalse(bundle.distributable(name), name)
        self.assertTrue(bundle.distributable("src/lib.rs"))

    def test_android_flutter_outputs_and_signing_files_never_enter_bundle(self):
        # Even tracked files bypass gitignore; the distribution policy must
        # independently reject machine state, build output and private keys.
        rejected = [
            "examples/app/build/app.apk", "examples/app/.gradle/cache.bin",
            "examples/app/.gradle-bootstrap/gradle.zip", "examples/app/.cxx/cache",
            "examples/app/.externalNativeBuild/cache", "examples/app/.dart_tool/package_config.json",
            "examples/app/local.properties", "examples/app/key.properties",
            "examples/app/.flutter-plugins-dependencies", "signing/release.jks",
            "signing/release.keystore", "release/app.aab", "release/app.apks",
            "examples/app/target/cache", "logs/debug.log", ".pytest_cache/state",
            "coverage/default.profraw", ".coverage.host", "src/lib.rs~",
        ]
        for name in rejected:
            with self.subTest(name=name):
                self.assertFalse(bundle.distributable(name))
        for name in ["Cargo.lock", "docs/android/production-readiness.md",
                     "tests/golden/images/button_enabled_light.rgba",
                     "scripts/android/templates/build.gradle.kts",
                     "examples/android_counter/src/lib.rs"]:
            with self.subTest(name=name):
                self.assertTrue(bundle.distributable(name))

    def test_signing_key_is_rejected_when_explicitly_in_the_inventory(self):
        (self.root / "release.jks").write_bytes(b"private signing key fixture")
        with self.assertRaises(ValueError):
            self.write(names=self.names + ["release.jks"])
        self.assertFalse((self.root / "source.tar.gz").exists())

    def test_symlink_cannot_copy_content_outside_source_root(self):
        (self.root / "src/link.rs").symlink_to("/etc/passwd")
        with self.assertRaises(ValueError):
            self.write(names=[*self.names, "src/link.rs"])
        self.assertFalse((self.root / "source.tar.gz").exists())
        self.assertEqual(list(self.root.glob("*.tmp")), [])

    def test_missing_attribution_or_toolchain_does_not_overwrite_existing_bundle(self):
        output = self.root / "source.tar.gz"
        output.write_bytes(b"previous artifact")
        with self.assertRaises(ValueError):
            self.write(names=[name for name in self.names if name != "LICENSE"])
        self.assertEqual(output.read_bytes(), b"previous artifact")


if __name__ == "__main__":
    unittest.main()
