"""iOS artifact boundaries: Simulator, device and macOS binaries cannot be
substituted, and device signing only accepts a profile matching its mode."""
import datetime as dt
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("ios_package", ROOT / "scripts/ios/package.py")
ios = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ios)


class SimulatorPackagingTests(unittest.TestCase):
    def test_bundle_metadata_is_explicitly_simulator_only(self):
        info = ios.info_plist("Counter", "counter", "com.theaimartco.taim.counter", "15.0")
        self.assertEqual(info["CFBundleSupportedPlatforms"], ["iPhoneSimulator"])
        self.assertEqual(info["CFBundleExecutable"], "counter")
        for name, binary, bundle_id, minimum in [("../escape", "counter", "com.example.app", "15.0"),
                ("Counter", "../counter", "com.example.app", "15.0"),
                ("Counter", "counter", "com..app", "15.0"),
                ("Counter", "counter", "com.example.app", "invalid")]:
            with self.subTest(name=name, binary=binary, bundle_id=bundle_id), self.assertRaises(ios.common.PackagingError):
                ios.info_plist(name, binary, bundle_id, minimum)

    def test_promotion_phones_are_not_held_to_sixty_hertz(self):
        for device in (False, True):
            with self.subTest(device=device):
                info = ios.info_plist("Counter", "counter", "com.example.app", "15.0", device=device)
                self.assertIs(info["CADisableMinimumFrameDurationOnPhone"], True)

    def test_device_and_macos_binaries_are_rejected(self):
        for platform in ("IOS", "MACOS"):
            with self.subTest(platform=platform), patch.object(ios.common, "run", side_effect=[b"arm64", f"platform {platform}\nminos 15.0\n".encode()]):
                with self.assertRaisesRegex(ios.common.PackagingError, "Simulator binary"):
                    ios.check_binary("counter", "15.0", "arm64")

    def test_architecture_minimum_and_non_system_dylibs_are_verified(self):
        cases = [[b"x86_64"], [b"arm64", b"platform IOSSIMULATOR\nminos 16.0\n"],
                 [b"arm64", b"platform IOSSIMULATOR\nminos 15.0\n", b"counter:\n\t/opt/local/lib/extra.dylib (compatibility version 1.0)\n"]]
        for outputs in cases:
            with self.subTest(outputs=outputs), patch.object(ios.common, "run", side_effect=outputs):
                with self.assertRaises(ios.common.PackagingError):
                    ios.check_binary("counter", "15.0", "arm64")


def profile(mode="app-store", app_id="ABCDE12345.com.example.app", expires=None):
    entitlements = {"application-identifier": app_id,
                    "keychain-access-groups": ["ABCDE12345.*"],
                    "get-task-allow": mode == "development"}
    result = {"ExpirationDate": expires or dt.datetime(2099, 1, 1), "TeamIdentifier": ["ABCDE12345"],
              "Platform": ["iOS"], "Entitlements": entitlements, "DeveloperCertificates": [b"cert"],
              "Name": "Example", "UUID": "0000"}
    if mode in ("development", "ad-hoc"):
        result["ProvisionedDevices"] = ["00008110-000000000000001E"]
    return result


class DevicePackagingTests(unittest.TestCase):
    def test_device_metadata_and_usage_descriptions(self):
        usage = ios.usage_descriptions(["NSCameraUsageDescription=Scan receipts",
                                        "NSFaceIDUsageDescription=Unlock your notes"])
        info = ios.info_plist("Notes", "notes", "com.example.app", "15.0", device=True,
                              version="1.2.0", build="7", usage=usage)
        self.assertEqual(info["CFBundleSupportedPlatforms"], ["iPhoneOS"])
        self.assertEqual(info["UIRequiredDeviceCapabilities"], ["arm64"])
        self.assertEqual(info["NSCameraUsageDescription"], "Scan receipts")
        self.assertEqual(info["CFBundleVersion"], "7")
        for bad in (["NSCameraUsage=x"], ["NSCameraUsageDescription="], ["NSCameraUsageDescription"],
                    ["NSCameraUsageDescription=a", "NSCameraUsageDescription=b"]):
            with self.subTest(bad=bad), self.assertRaises(ios.common.PackagingError):
                ios.usage_descriptions(bad)
        with self.assertRaises(ios.common.PackagingError):
            ios.info_plist("Notes", "notes", "com.example.app", "15.0", version="one")

    def test_profiles_must_match_the_signing_mode(self):
        for mode in ios.DEVICE_MODES:
            with self.subTest(mode=mode):
                ios.check_profile(profile(mode), "com.example.app", "ABCDE12345", mode)
        mismatches = [("development", "app-store"), ("app-store", "ad-hoc"), ("ad-hoc", "development")]
        for given, wanted in mismatches:
            with self.subTest(given=given, wanted=wanted), self.assertRaises(ios.common.PackagingError):
                ios.check_profile(profile(given), "com.example.app", "ABCDE12345", wanted)

    def test_profiles_are_checked_for_identity_expiry_and_platform(self):
        with self.assertRaisesRegex(ios.common.PackagingError, "expired"):
            ios.check_profile(profile(expires=dt.datetime(2000, 1, 1)), "com.example.app", "ABCDE12345", "app-store")
        with self.assertRaisesRegex(ios.common.PackagingError, "different team"):
            ios.check_profile(profile(), "com.example.app", "ZZZZZ99999", "app-store")
        with self.assertRaisesRegex(ios.common.PackagingError, "does not cover"):
            ios.check_profile(profile(), "com.example.other", "ABCDE12345", "app-store")
        mac = profile()
        mac["Platform"] = ["OSX"]
        with self.assertRaisesRegex(ios.common.PackagingError, "iOS provisioning"):
            ios.check_profile(mac, "com.example.app", "ABCDE12345", "app-store")
        wildcard = profile("development", app_id="ABCDE12345.*")
        ios.check_profile(wildcard, "com.example.any", "ABCDE12345", "development")
        with self.assertRaisesRegex(ios.common.PackagingError, "explicit App ID"):
            ios.check_profile(profile(app_id="ABCDE12345.*"), "com.example.app", "ABCDE12345", "app-store")

    def test_entitlements_come_from_the_profile(self):
        entitlements = ios.entitlements_for(profile("development"), "com.example.app", "ABCDE12345")
        self.assertEqual(entitlements["application-identifier"], "ABCDE12345.com.example.app")
        self.assertEqual(entitlements["keychain-access-groups"], ["ABCDE12345.com.example.app"])
        self.assertTrue(entitlements["get-task-allow"])
        self.assertFalse(ios.entitlements_for(profile(), "com.example.app", "ABCDE12345")["get-task-allow"])
        self.assertTrue(ios.identifier_allowed("ABCDE12345.com.example.*", "ABCDE12345.com.example.app"))
        self.assertFalse(ios.identifier_allowed("ABCDE12345.com.example.app", "ABCDE12345.com.example.app2"))

    def test_identities_must_match_the_mode(self):
        listing = (b'  1) 1111111111111111111111111111111111111111 "Apple Development: Dev (ABCDE12345)"\n'
                   b'  2) 2222222222222222222222222222222222222222 "Apple Distribution: Org (ABCDE12345)"\n')
        with patch.object(ios.common, "run", return_value=listing):
            self.assertEqual(ios.identity("Apple Distribution: Org (ABCDE12345)", "app-store")[0], "2" * 40)
            self.assertEqual(ios.identity("1" * 40, "development")[0], "1" * 40)
            with self.assertRaises(ios.common.PackagingError):
                ios.identity("Apple Development: Dev (ABCDE12345)", "app-store")
            with self.assertRaises(ios.common.PackagingError):
                ios.identity("Missing", "development")

    def test_simulator_entitlements_are_linked_not_signed(self):
        import os, tempfile
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {}, clear=False):
            path = ios.embed_simulator_entitlements(Path(directory), "com.example.app", "aarch64-apple-ios-sim")
            self.assertEqual(ios.plistlib.loads(path.read_bytes())["keychain-access-groups"], ["com.example.app"])
            self.assertIn(f"__TEXT,__entitlements,{path}", os.environ["CARGO_TARGET_AARCH64_APPLE_IOS_SIM_RUSTFLAGS"])

    def test_device_binaries_must_be_device_builds(self):
        with patch.object(ios.common, "run", side_effect=[b"arm64", b"platform IOSSIMULATOR\nminos 15.0\n"]):
            with self.assertRaisesRegex(ios.common.PackagingError, "iOS device binary"):
                ios.check_binary("app", "15.0", "arm64", "IOS")


if __name__ == "__main__":
    unittest.main()
