"""Distribution regressions: identity, entitlement and artifact ownership boundaries."""
import datetime as dt
import importlib.util
import hashlib
import json
from pathlib import Path
import plistlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("macos_package", ROOT / "scripts/macos/package.py")
macos = importlib.util.module_from_spec(spec)
spec.loader.exec_module(macos)


class MacDistributionTests(unittest.TestCase):
    def config(self):
        return macos.config_at(ROOT / "scripts/macos/counter.toml")

    def profile(self):
        return {"ExpirationDate": dt.datetime(2030, 1, 1), "TeamIdentifier": ["ABCDEFGHIJ"], "Platform": ["OSX"],
                "Entitlements": {"com.apple.application-identifier": "ABCDEFGHIJ.dev.example.notes"},
                "DeveloperCertificates": [b"fixture certificate"]}

    def test_rejects_wrong_bundle_team_expired_and_development_profiles(self):
        valid = self.profile()
        now = dt.datetime(2026, 1, 1, tzinfo=dt.timezone.utc)
        macos.check_profile(valid, "dev.example.notes", "ABCDEFGHIJ", now)
        for field, replacement in [
            ("ExpirationDate", dt.datetime(2020, 1, 1)),
            ("TeamIdentifier", ["WRONGTEAM0"]),
            ("Platform", ["iOS"]),
            ("Entitlements", {"com.apple.application-identifier": "ABCDEFGHIJ.*"}),
            ("Entitlements", {**valid["Entitlements"], "get-task-allow": True}),
            ("ProvisionedDevices", ["test-device"]),
            ("ProvisionsAllDevices", True),
            ("DeveloperCertificates", []),
        ]:
            with self.subTest(field=field, value=replacement), self.assertRaises(macos.PackagingError):
                macos.check_profile({**valid, field: replacement}, "dev.example.notes", "ABCDEFGHIJ", now)

    def test_local_sandbox_cannot_be_disabled_or_inherit_debug_capabilities(self):
        config = self.config()
        self.assertEqual(macos.entitlements_for(config), {"com.apple.security.app-sandbox": True})
        for key in ["com.apple.security.app-sandbox", "com.apple.security.get-task-allow",
                    "get-task-allow", "com.apple.security.inherit", "com.apple.security.cs.allow-jit",
                    "com.apple.security.cs.disable-library-validation", "com.apple.application-identifier"]:
            config["entitlements"] = {key: False if key.endswith("app-sandbox") else True}
            with self.subTest(key=key), self.assertRaises(macos.PackagingError):
                macos.entitlements_for(config)

    def test_macos_minimum_cannot_be_lower_than_compiled_executable(self):
        with patch.object(macos, "run", side_effect=[b"arm64", b"platform MACOS\n minos 14.0\n"]):
            with self.assertRaisesRegex(macos.PackagingError, "newer macOS"):
                macos.check_binary(Path("fixture"), "13.0")
        with patch.object(macos, "run", side_effect=[b"arm64 x86_64", b"platform MACOS\nminos 13.0\nplatform MACOS\nminos 14.0\n"]):
            with self.assertRaises(macos.PackagingError):
                macos.check_binary(Path("fixture"), "13.0", ["arm64", "x86_64"])

    def test_refuses_unbundled_local_dynamic_libraries(self):
        with patch.object(macos, "run", side_effect=[b"arm64", b"platform MACOS\nminos 13.0\n", b"app:\n\t/opt/homebrew/lib/libextra.dylib (compatibility version 1.0)\n"]):
            with self.assertRaisesRegex(macos.PackagingError, "Non-system"):
                macos.check_binary(Path("fixture"), "13.0")

    def test_developer_id_and_development_certificates_cannot_sign_store_artifacts(self):
        for name in ["Developer ID Application: Example (ABCDEFGHIJ)", "Apple Development: Example (ABCDEFGHIJ)"]:
            with patch.object(macos, "run", return_value=("1) " + "A" * 40 + f' "{name}"\n').encode()):
                with self.subTest(name=name), self.assertRaises(macos.PackagingError):
                    macos.identity(name)
        installer = "3rd Party Mac Developer Installer: Example (ABCDEFGHIJ)"
        with patch.object(macos, "run", return_value=("1) " + "A" * 40 + f' "{installer}"\n').encode()) as command:
            macos.identity(installer, installer=True)
            self.assertNotIn("-p", command.call_args.args[0])

    def test_failed_build_cannot_package_stale_executable(self):
        stale = {"reason": "compiler-artifact", "target": {"name": "notes", "kind": ["bin"]}, "executable": "/tmp/stale-notes"}
        result = subprocess.CompletedProcess([], 1, stdout=json.dumps(stale), stderr="build failed")
        with patch.object(macos.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(macos.PackagingError, "Cargo build failed"):
                macos.build_executable(Path("Cargo.toml"), "notes", "notes", "aarch64-apple-darwin", False, "13.0", "", True)

    def test_same_named_binary_from_another_package_cannot_be_substituted(self):
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "notes"
            executable.write_bytes(b"fixture")
            message = {"reason": "compiler-artifact", "package_id": "another-package",
                       "target": {"name": "notes", "kind": ["bin"]}, "executable": str(executable)}
            result = subprocess.CompletedProcess([], 0, stdout=json.dumps(message))
            with patch.object(macos.subprocess, "run", return_value=result):
                with self.assertRaisesRegex(macos.PackagingError, "exactly one"):
                    macos.build_executable(Path("Cargo.toml"), "notes", "notes", "aarch64-apple-darwin",
                                           False, "13.0", "", True, "the-selected-package")

    def test_store_signing_never_falls_back_and_binds_certificate_to_profile(self):
        with patch.dict(macos.os.environ, {}, clear=True):
            with self.assertRaisesRegex(macos.PackagingError, "App Store signing needs"):
                macos.signing_inputs(self.config())
        environment = dict(zip(macos.SIGNING_ENV, ["app", "installer", "ABCDEFGHIJ", "/tmp/test.provisionprofile"]))
        fingerprint = hashlib.sha1(b"fixture certificate").hexdigest().upper()
        identities = [("0" * 40, "Apple Distribution: Test (ABCDEFGHIJ)"),
                      ("1" * 40, "3rd Party Mac Developer Installer: Test (ABCDEFGHIJ)")]
        with patch.dict(macos.os.environ, environment, clear=True), \
                patch.object(macos, "profile_for", return_value=self.profile()), \
                patch.object(macos, "identity", side_effect=identities):
            with self.assertRaisesRegex(macos.PackagingError, "not authorized"):
                macos.signing_inputs(self.config())
        identities[0] = (fingerprint, identities[0][1])
        with patch.dict(macos.os.environ, environment, clear=True), \
                patch.object(macos, "profile_for", return_value=self.profile()), \
                patch.object(macos, "identity", side_effect=identities):
            self.assertEqual(macos.signing_inputs(self.config())[2], fingerprint)

    def test_ios_executable_cannot_masquerade_as_a_mac_app(self):
        with patch.object(macos, "run", side_effect=[b"arm64", b"platform IOS\nminos 13.0\n"]):
            with self.assertRaisesRegex(macos.PackagingError, "not built for macOS"):
                macos.check_binary(Path("fixture"), "13.0")

    def test_unsafe_bundle_names_and_misspelled_config_fields_are_rejected(self):
        for text in ['name = "../escape"', 'name = "Normal"\nminimum_os = "13.0"']:
            with tempfile.TemporaryDirectory() as directory:
                config = self.config()["app"]
                fields = "\n".join(f'{key} = {json.dumps(value)}' for key, value in config.items() if key != "name")
                path = Path(directory) / "app.toml"
                path.write_text("[app]\n" + fields + "\n" + text)
                with self.assertRaises(macos.PackagingError):
                    macos.config_at(path)

    def test_signed_entitlements_are_read_from_signature_not_an_untrusted_sidecar(self):
        actual = plistlib.dumps({"com.apple.security.app-sandbox": True})
        with patch.object(macos, "run", return_value=actual) as command:
            self.assertTrue(macos.signed_entitlements("Example.app")["com.apple.security.app-sandbox"])
            self.assertEqual(command.call_args.args[0][0], "codesign")


if __name__ == "__main__":
    unittest.main()
