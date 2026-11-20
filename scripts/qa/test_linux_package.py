"""Snap Store packaging: configuration, snapcraft.yaml and desktop entry."""
import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("linux_package", ROOT / "scripts/linux/package.py")
linux = importlib.util.module_from_spec(spec)
spec.loader.exec_module(linux)


class SnapPackagingTests(unittest.TestCase):
    def config(self, **app):
        source = (ROOT / "scripts/linux/counter.toml").read_text()
        source = source.replace('"../macos/AppIcon.png"', f'"{ROOT / "scripts/macos/AppIcon.png"}"')
        for key, value in app.items():
            source = "\n".join(l for l in source.splitlines() if not l.startswith(key + " ")) + f"\n{key} = {value}\n"
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "app.toml"
            path.write_text(source)
            return linux.config_at(path)

    def test_the_example_configuration_is_valid(self):
        self.assertEqual(self.config()["app"]["name"], "taim-counter")

    def test_rejects_names_and_metadata_the_store_refuses(self):
        for field, value in [("name", '"Taim"'), ("name", '"a"'), ("name", '"taim--counter"'), ("name", '"-taim"'),
                             ("summary", '"' + "x" * 79 + '"'), ("grade", '"beta"'),
                             ("categories", '["Games"]'), ("plugs", '["camera"]'), ("version", '"1 0"')]:
            with self.subTest(field=field, value=value), self.assertRaises(linux.PackagingError):
                self.config(**{field: value})

    def test_snapcraft_project_is_strict_core24_and_builds_the_package(self):
        text = linux.snapcraft(self.config(plugs='["network"]', title='"Taim: \\"Counter\\""'),
                               "examples/counter", "counter", "https://example.com/taim.git", ["webgl"])
        try:
            import yaml
        except ImportError:
            self.skipTest("PyYAML is not installed")
        project = yaml.safe_load(text)
        self.assertEqual(project["base"], "core24")
        self.assertEqual(project["confinement"], "strict")
        self.assertEqual(project["title"], 'Taim: "Counter"')
        app = project["apps"]["taim-counter"]
        self.assertEqual(app["command"], "bin/counter")
        self.assertIn("network", app["plugs"])
        self.assertIn("wayland", app["plugs"])
        part = project["parts"]["taim-counter"]
        self.assertEqual(part["plugin"], "rust")
        self.assertEqual(part["rust-path"], ["examples/counter"])
        self.assertEqual(part["rust-features"], ["webgl"])
        self.assertIn("libxkbcommon0", part["stage-packages"])
        self.assertEqual(project["parts"]["gpu-2404"]["after"], ["taim-counter"])

    def test_writes_a_project_with_its_desktop_entry_and_icon(self):
        with tempfile.TemporaryDirectory() as scratch:
            args = SimpleNamespace(output=Path(scratch) / "out", manifest=ROOT / "Cargo.toml", package="counter",
                                   binary="counter", source=None, features="")
            output = linux.snap(args, self.config())
            gui = output / "snap" / "gui"
            entry = (gui / "taim-counter.desktop").read_text()
            self.assertIn("Exec=taim-counter\n", entry)
            self.assertIn("Icon=${SNAP}/meta/gui/taim-counter.png\n", entry)
            self.assertTrue((gui / "taim-counter.png").is_file())
            self.assertIn('source: "' + str(ROOT), (output / "snap/snapcraft.yaml").read_text())
            with self.assertRaisesRegex(linux.PackagingError, "No package"):
                linux.snap(SimpleNamespace(**{**vars(args), "output": Path(scratch) / "x", "package": "nope"}),
                           self.config())

    def test_small_or_non_png_icons_are_refused(self):
        with tempfile.TemporaryDirectory() as scratch:
            fake = Path(scratch) / "icon.png"
            fake.write_bytes(b"not a png")
            config = {"root": Path(scratch), "app": {"icon": "icon.png"}}
            with self.assertRaisesRegex(linux.PackagingError, "not a PNG"):
                linux.write_icon(config, Path(scratch) / "out.png")


if __name__ == "__main__":
    unittest.main()
