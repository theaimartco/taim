"""Microsoft Store packaging: configuration, manifest, logos and executables."""
import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("windows_package", ROOT / "scripts/windows/package.py")
windows = importlib.util.module_from_spec(spec)
spec.loader.exec_module(windows)

FOUNDATION = "{http://schemas.microsoft.com/appx/manifest/foundation/windows10}"
UAP = "{http://schemas.microsoft.com/appx/manifest/uap/windows10}"
RESCAP = "{http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities}"


def executable(subsystem=2, machine=0x8664):
    """The smallest PE file the packager reads: DOS stub, PE header, optional header."""
    data = bytearray(0x200)
    data[:2] = b"MZ"
    data[0x3C:0x40] = (0x80).to_bytes(4, "little")
    data[0x80:0x84] = b"PE\0\0"
    data[0x84:0x86] = machine.to_bytes(2, "little")
    data[0x80 + 24 + 68:0x80 + 24 + 70] = subsystem.to_bytes(2, "little")
    return bytes(data)


class WindowsPackagingTests(unittest.TestCase):
    def config(self, **app):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "app.toml"
            source = (ROOT / "scripts/windows/counter.toml").read_text()
            for key, value in app.items():
                source = "\n".join(line for line in source.splitlines() if not line.startswith(key + " "))
                source += f"\n{key} = {value}\n" if not source.endswith("\n") else f"{key} = {value}\n"
            path.write_text(source.replace('"../macos/AppIcon.png"', f'"{ROOT / "scripts/macos/AppIcon.png"}"'))
            return windows.config_at(path)

    def test_the_example_configuration_is_valid(self):
        config = self.config()
        self.assertEqual(config["app"]["version"], "0.1.0.0")

    def test_rejects_store_incompatible_versions_identities_and_capabilities(self):
        for field, value in [("version", '"1.0.0"'), ("version", '"1.0.0.3"'), ("version", '"1.0.0.70000"'),
                             ("publisher", '"TheAimArtCo"'), ("identity", '"has space"'),
                             ("capabilities", '["webcam"]'), ("background", '"red"')]:
            with self.subTest(field=field, value=value), self.assertRaises(windows.PackagingError):
                self.config(**{field: value})

    def test_the_manifest_is_a_full_trust_desktop_package(self):
        config = self.config(capabilities='["internetClient"]', description='"Counts <things> & more"')
        root = ET.fromstring(windows.manifest(config, "counter.exe", "arm64"))
        identity = root.find(FOUNDATION + "Identity")
        self.assertEqual(identity.get("ProcessorArchitecture"), "arm64")
        self.assertEqual(identity.get("Publisher"), "CN=TheAimArtCo")
        application = root.find(f"{FOUNDATION}Applications/{FOUNDATION}Application")
        self.assertEqual(application.get("EntryPoint"), "Windows.FullTrustApplication")
        self.assertEqual(application.get("Executable"), "counter.exe")
        visuals = application.find(UAP + "VisualElements")
        self.assertEqual(visuals.get("Description"), "Counts <things> & more")
        capabilities = root.find(FOUNDATION + "Capabilities")
        self.assertEqual(capabilities[0].tag, RESCAP + "Capability")
        self.assertEqual(capabilities[0].get("Name"), "runFullTrust")
        self.assertEqual([c.get("Name") for c in capabilities[1:]], ["internetClient"])
        family = root.find(f"{FOUNDATION}Dependencies/{FOUNDATION}TargetDeviceFamily")
        self.assertEqual(family.get("Name"), "Windows.Desktop")

    def test_console_executables_are_refused(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "app.exe"
            path.write_bytes(executable(subsystem=3))
            with self.assertRaisesRegex(windows.PackagingError, "console window"):
                windows.check_executable(path)
            path.write_bytes(executable(machine=0xAA64))
            self.assertEqual(windows.check_executable(path), "arm64")

    def test_layout_writes_manifest_logos_and_executable(self):
        try:
            from PIL import Image
        except ImportError:
            self.skipTest("Pillow is not installed")
        config = self.config()
        with tempfile.TemporaryDirectory() as scratch:
            built = Path(scratch) / "built.exe"
            built.write_bytes(executable())
            args = SimpleNamespace(output=Path(scratch) / "out", manifest=ROOT / "Cargo.toml",
                                   executable=built, arch="x64", binary="counter")
            _, folder, name = windows.layout(args, config)
            self.assertEqual(name, "counter.exe")
            self.assertEqual((folder / name).read_bytes(), built.read_bytes())
            ET.parse(folder / "AppxManifest.xml")
            for logo, size in windows.LOGOS.items():
                with Image.open(folder / "Assets" / logo) as image:
                    self.assertEqual(image.size, (size, size))
            with Image.open(folder / "Assets" / windows.WIDE[0]) as image:
                self.assertEqual(image.size, windows.WIDE[1])
            with self.assertRaisesRegex(windows.PackagingError, "not arm64"):
                windows.layout(SimpleNamespace(**{**vars(args), "output": Path(scratch) / "other",
                                                  "arch": "arm64"}), config)


if __name__ == "__main__":
    unittest.main()
