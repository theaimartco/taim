"""Self-contained dependency boundary fixtures (no source ownership claims)."""

import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("boundary", Path(__file__).with_name("check-upstream-boundary.py"))
boundary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(boundary)


class BoundaryTests(unittest.TestCase):
    def test_iced_package_prefix(self):
        self.assertTrue(boundary.iced_package("iced_widget"))
        self.assertTrue(boundary.iced_package("iced-wgpu"))
        self.assertFalse(boundary.iced_package("iceberg"))
        self.assertFalse(boundary.iced_package("taim_widget"))

    def test_lock_rejects_iced(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "Cargo.lock"
            path.write_text('version = 4\n[[package]]\nname = "iced_winit"\nversion = "1.0.0"\n')
            self.assertIn("Iced crate", boundary.check_lock(path)[0])

    def test_lock_rejects_iced_git_source(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "Cargo.lock"
            path.write_text('version = 4\n[[package]]\nname = "neutral"\nversion = "1.0.0"\nsource = "git+https://github.com/iced-rs/neutral"\n')
            self.assertIn("iced-rs Git", boundary.check_lock(path)[0])

    def test_clean_manifest_permitted(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "Cargo.toml"
            path.write_text('[dependencies]\ntaim_core = "0.15"\n')
            self.assertEqual(boundary.check_manifest(path), [])

    def test_renamed_iced_dependency_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "Cargo.toml"
            path.write_text('[dependencies]\nmy_frontend = { package = "iced_widget", version = "0.14" }\n')
            self.assertIn("Iced crate", boundary.check_manifest(path)[0])

    def test_workspace_iced_dependency_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "Cargo.toml"
            path.write_text('[workspace.dependencies]\ngui = { package = "iced_widget", version = "0.14" }\n')
            self.assertIn("Iced crate", boundary.check_manifest(path)[0])

    def test_patched_iced_source_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "Cargo.toml"
            path.write_text('[patch.crates-io]\niced_runtime = { git = "https://github.com/iced-rs/iced" }\n')
            messages = boundary.check_manifest(path)
            self.assertGreaterEqual(len(messages), 1)
            self.assertTrue(any("Iced" in message or "iced-rs Git" in message for message in messages))

    def test_replacement_iced_source_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "Cargo.toml"
            path.write_text('[replace]\n"iced_core:0.14.0" = { path = "src" }\n')
            self.assertIn("Iced crate", boundary.check_manifest(path)[0])

    def test_target_specific_iced_dependency_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "Cargo.toml"
            path.write_text('[target."cfg(unix)".dependencies]\ngui = { git = "https://github.com/iced-rs/gui.git" }\n')
            self.assertIn("iced-rs Git", boundary.check_manifest(path)[0])


if __name__ == "__main__":
    unittest.main()
