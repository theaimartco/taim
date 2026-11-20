#!/usr/bin/env python3
"""Build a signed iOS app from a TAIM Cargo binary.

Two kinds of product, never interchangeable:

* Simulator (`--target aarch64-apple-ios-sim` or `x86_64-apple-ios`, the
  default): an ad-hoc signed `.app` for `xcrun simctl`. It cannot be installed
  on a phone or uploaded.
* Device (`--target aarch64-apple-ios`): an `.app` signed with the owner's
  certificate and provisioning profile, embedded profile, entitlements derived
  from that profile, and an `.ipa`. `--mode development` and `ad-hoc` install
  on the profile's registered devices; `app-store` is for App Store Connect /
  TestFlight upload, which this tool never performs.

Credentials are never stored: pass `--team`, `--identity` and `--profile`, or
set TAIM_IOS_TEAM_ID, TAIM_IOS_IDENTITY and TAIM_IOS_PROFILE.
"""
import argparse
import datetime as dt
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("taim_macos_package", ROOT / "scripts/macos/package.py")
common = importlib.util.module_from_spec(spec)
spec.loader.exec_module(common)

SIMULATOR_TARGETS = {"aarch64-apple-ios-sim": "arm64", "x86_64-apple-ios": "x86_64"}
DEVICE_TARGETS = {"aarch64-apple-ios": "arm64"}
DEVICE_MODES = ("development", "ad-hoc", "app-store")
DEVELOPMENT_IDENTITIES = ("Apple Development:", "iPhone Developer:")
DISTRIBUTION_IDENTITIES = ("Apple Distribution:", "iPhone Distribution:")
# The usage descriptions TAIM's iOS services can need; iOS terminates an app
# that touches a protected service without its key.
USAGE_KEYS = {
    "NSCameraUsageDescription", "NSMicrophoneUsageDescription", "NSFaceIDUsageDescription",
    "NSLocationWhenInUseUsageDescription", "NSLocationAlwaysAndWhenInUseUsageDescription",
    "NSPhotoLibraryUsageDescription", "NSPhotoLibraryAddUsageDescription",
    "NSBluetoothAlwaysUsageDescription", "NSContactsUsageDescription", "NSCalendarsUsageDescription",
    "NSMotionUsageDescription", "NSSpeechRecognitionUsageDescription", "NSUserTrackingUsageDescription",
    "NSLocalNetworkUsageDescription", "NFCReaderUsageDescription",
}


def usage_descriptions(pairs):
    descriptions = {}
    for pair in pairs or []:
        key, separator, text = pair.partition("=")
        common.require(separator and key in USAGE_KEYS, f"Unknown usage description key: {key or pair}")
        common.require(text.strip() and len(text) <= 1000 and not any(c in text for c in "\x00\r"),
                       f"{key} needs a short, user-facing sentence")
        common.require(key not in descriptions, f"{key} given twice")
        descriptions[key] = text.strip()
    return descriptions


def info_plist(name, binary, bundle_id, minimum, *, device=False, version="0.1.0", build="1", usage=None):
    common.require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9-]*(\.[A-Za-z0-9][A-Za-z0-9-]*)+", bundle_id), "Invalid bundle ID")
    common.require(name and not any(c in name for c in '/\\:\x00\r\n') and name not in (".", ".."), "Invalid app name")
    common.require(binary and Path(binary).name == binary and binary not in (".", ".."), "Invalid executable name")
    common.require(re.fullmatch(r"\d+\.\d+(\.\d+)?", minimum), "Invalid iOS deployment target")
    common.require(re.fullmatch(r"[0-9]+(\.[0-9]+){0,2}", version), "Version must have one to three numeric components")
    common.require(re.fullmatch(r"[0-9]+(\.[0-9]+){0,2}", build), "Build number must have one to three numeric components")
    info = {"CFBundleName": name, "CFBundleDisplayName": name,
            "CFBundleExecutable": binary, "CFBundleIdentifier": bundle_id,
            "CFBundlePackageType": "APPL", "CFBundleInfoDictionaryVersion": "6.0",
            "CFBundleDevelopmentRegion": "en",
            "CFBundleShortVersionString": version, "CFBundleVersion": build,
            "CFBundleSupportedPlatforms": ["iPhoneOS" if device else "iPhoneSimulator"],
            "MinimumOSVersion": minimum, "LSRequiresIPhoneOS": True,
            "UIDeviceFamily": [1, 2], "UILaunchScreen": {},
            # Without it, iPhones with ProMotion hold the app to 60 Hz.
            "CADisableMinimumFrameDurationOnPhone": True,
            "UIRequiredDeviceCapabilities": ["arm64"],
            "UISupportedInterfaceOrientations": ["UIInterfaceOrientationPortrait",
                "UIInterfaceOrientationLandscapeLeft", "UIInterfaceOrientationLandscapeRight"],
            "UISupportedInterfaceOrientations~ipad": ["UIInterfaceOrientationPortrait",
                "UIInterfaceOrientationPortraitUpsideDown", "UIInterfaceOrientationLandscapeLeft",
                "UIInterfaceOrientationLandscapeRight"]}
    if not device:
        del info["UIRequiredDeviceCapabilities"]
    info.update(usage or {})
    return info


def check_binary(binary, minimum, arch, platform="IOSSIMULATOR"):
    architectures = common.run(["lipo", "-archs", binary]).decode().split()
    common.require(architectures == [arch], "Executable has the wrong architecture")
    metadata = common.run(["vtool", "-show-build", binary]).decode()
    kind = "iOS Simulator" if platform == "IOSSIMULATOR" else "iOS device"
    common.require(re.search(rf"\bplatform\s+{platform}\b", metadata), f"Executable is not an {kind} binary")
    versions = re.findall(r"\bminos\s+([0-9.]+)", metadata)
    common.require(len(versions) == 1 and common.version_tuple(versions[0]) <= common.version_tuple(minimum),
                   "Executable requires a newer iOS than MinimumOSVersion")
    libraries = common.run(["otool", "-L", binary]).decode().splitlines()
    for line in libraries:
        if line.startswith("\t"):
            library = line.strip().split(" (", 1)[0]
            common.require(library.startswith(("/System/Library/", "/usr/lib/")),
                           f"Non-system dylib needs explicit bundling/signing: {library}")


def identifier_allowed(pattern, identifier):
    """Whether a profile's application identifier (possibly `TEAM.*` or
    `TEAM.com.example.*`) covers `identifier`."""
    if pattern.endswith("*"):
        return identifier.startswith(pattern[:-1])
    return pattern == identifier


def check_profile(profile, bundle_id, team, mode, now=None):
    now = now or dt.datetime.now(dt.timezone.utc)
    common.require(mode in DEVICE_MODES, "Unknown signing mode")
    expires = profile.get("ExpirationDate")
    common.require(isinstance(expires, dt.datetime), "Provisioning profile has no expiration date")
    common.require(expires.replace(tzinfo=dt.timezone.utc) > now, "Provisioning profile has expired")
    common.require(team in profile.get("TeamIdentifier", []), "Provisioning profile belongs to a different team")
    common.require("iOS" in profile.get("Platform", []), "Use an iOS provisioning profile")
    allowed = profile.get("Entitlements", {})
    pattern = allowed.get("application-identifier", "")
    common.require(identifier_allowed(pattern, team + "." + bundle_id),
                   "Provisioning profile does not cover this bundle ID and team")
    debuggable = bool(allowed.get("get-task-allow"))
    devices = profile.get("ProvisionedDevices") or profile.get("ProvisionsAllDevices")
    if mode == "development":
        common.require(debuggable and devices, "Use an iOS App Development profile for --mode development")
    elif mode == "ad-hoc":
        common.require(not debuggable and devices, "Use an Ad Hoc distribution profile for --mode ad-hoc")
    else:
        common.require(not debuggable and not devices, "Use an App Store distribution profile for --mode app-store")
        common.require(pattern == team + "." + bundle_id, "App Store distribution needs an explicit App ID")
    common.require(profile.get("DeveloperCertificates"), "Provisioning profile has no certificates")
    return profile


def entitlements_for(profile, bundle_id, team):
    """Entitlements every TAIM iOS app needs, each one checked against the
    profile: the app identity, its own keychain group and, for development
    profiles only, debugging."""
    allowed = profile.get("Entitlements", {})
    application = team + "." + bundle_id
    entitlements = {"application-identifier": application,
                    "com.apple.developer.team-identifier": team,
                    "get-task-allow": bool(allowed.get("get-task-allow"))}
    groups = allowed.get("keychain-access-groups", [])
    if any(identifier_allowed(group, application) for group in groups):
        entitlements["keychain-access-groups"] = [application]
    return entitlements


def identity(value, mode):
    found = re.findall(r'\b([0-9A-Fa-f]{40})\s+"([^"]+)"',
                       common.run(["security", "find-identity", "-v", "-p", "codesigning"]).decode())
    matches = [(digest, name) for digest, name in found if value in (digest, name)]
    common.require(len(matches) == 1, "Signing identity not found or ambiguous in the current keychain")
    digest, name = matches[0]
    prefixes = DEVELOPMENT_IDENTITIES if mode == "development" else DISTRIBUTION_IDENTITIES
    common.require(name.startswith(prefixes),
                   f"--mode {mode} needs an identity starting with {' or '.join(prefixes)}")
    return digest.upper(), name


def compile_icon(icon, bundle, minimum, device, scratch):
    """Compiles a 1024 × 1024 PNG into the bundle's asset catalog with actool
    and returns the Info.plist keys it produces."""
    source = Path(icon).resolve()
    common.require(source.is_file() and source.suffix.lower() == ".png", f"App icon must be a PNG: {source}")
    dimensions = common.run(["sips", "-g", "pixelWidth", "-g", "pixelHeight", "-g", "hasAlpha", source]).decode()
    common.require(re.search(r"pixelWidth:\s+1024\b", dimensions) and re.search(r"pixelHeight:\s+1024\b", dimensions),
                   "App icon must be exactly 1024 x 1024")
    common.require(not re.search(r"hasAlpha:\s+yes\b", dimensions), "App Store icons must not have an alpha channel")
    catalog = scratch / "Assets.xcassets"
    iconset = catalog / "AppIcon.appiconset"
    iconset.mkdir(parents=True)
    shutil.copy2(source, iconset / "AppIcon.png")
    (catalog / "Contents.json").write_text(json.dumps({"info": {"author": "taim", "version": 1}}))
    (iconset / "Contents.json").write_text(json.dumps({
        "images": [{"filename": "AppIcon.png", "idiom": "universal", "platform": "ios", "size": "1024x1024"}],
        "info": {"author": "taim", "version": 1}}))
    partial = scratch / "icon-info.plist"
    common.run(["xcrun", "actool", "--compile", bundle, "--platform", "iphoneos" if device else "iphonesimulator",
                "--minimum-deployment-target", minimum, "--app-icon", "AppIcon",
                "--target-device", "iphone", "--target-device", "ipad",
                "--output-partial-info-plist", partial, "--errors", "--warnings", catalog])
    return plistlib.loads(partial.read_bytes())


def platform_keys(device):
    """The DT* build-environment keys App Store validation expects."""
    sdk = "iphoneos" if device else "iphonesimulator"
    version = common.run(["xcrun", "--sdk", sdk, "--show-sdk-version"]).decode().strip()
    build = common.run(["xcrun", "--sdk", sdk, "--show-sdk-build-version"]).decode().strip()
    xcode = common.run(["xcodebuild", "-version"]).decode().split()
    xcode_version = xcode[1] if len(xcode) > 1 else "0"
    parts = (xcode_version.split(".") + ["0", "0"])[:3]
    return {"DTPlatformName": sdk, "DTPlatformVersion": version, "DTPlatformBuild": build,
            "DTSDKName": sdk + version, "DTSDKBuild": build, "DTCompiler": "com.apple.compilers.llvm.clang.1_0",
            "DTXcode": f"{int(parts[0]):02d}{int(parts[1])}{int(parts[2])}", "DTXcodeBuild": xcode[-1],
            "BuildMachineOSBuild": common.run(["sw_vers", "-buildVersion"]).decode().strip()}


def simulator_entitlements(bundle_id):
    """What Xcode gives a Simulator app: its identity and its own keychain group."""
    return {"application-identifier": bundle_id, "keychain-access-groups": [bundle_id]}


def embed_simulator_entitlements(target_directory, bundle_id, target):
    """The Simulator reads an app's entitlements from the executable's
    `__TEXT,__entitlements` section, as Xcode links them; an ad-hoc signature
    carrying entitlements is refused at launch. The file has a stable path and
    content so repeated builds reuse Cargo's cache."""
    path = target_directory / "taim-ios" / (bundle_id + ".entitlements")
    path.parent.mkdir(parents=True, exist_ok=True)
    data = plistlib.dumps(simulator_entitlements(bundle_id))
    if not path.is_file() or path.read_bytes() != data:
        path.write_bytes(data)
    variable = "CARGO_TARGET_" + target.upper().replace("-", "_") + "_RUSTFLAGS"
    common.require("," not in str(path), "Target directory path must not contain commas")
    os.environ[variable] = (os.environ.get(variable, "") +
                            f" -C link-arg=-Wl,-sectcreate,__TEXT,__entitlements,{path}").strip()
    return path


def signing(args):
    team = args.team or os.environ.get("TAIM_IOS_TEAM_ID")
    identity_name = args.identity or os.environ.get("TAIM_IOS_IDENTITY")
    profile_path = args.profile or os.environ.get("TAIM_IOS_PROFILE")
    common.require(team and identity_name and profile_path,
                   "Device builds need --team, --identity and --profile (or TAIM_IOS_TEAM_ID, TAIM_IOS_IDENTITY, TAIM_IOS_PROFILE)")
    common.require(re.fullmatch(r"[A-Z0-9]{10}", team), "Team ID must be ten uppercase letters or digits")
    profile_path = Path(profile_path).resolve()
    common.require(profile_path.is_file(), "Provisioning profile not found")
    profile = plistlib.loads(common.run(["security", "cms", "-D", "-i", profile_path]))
    check_profile(profile, args.bundle_id, team, args.mode)
    digest, name = identity(identity_name, args.mode)
    return team, digest, name, profile_path, profile


def build(args):
    device = args.target in DEVICE_TARGETS
    common.require(not args.size or args.release, "--size needs --release")
    if device:
        common.require(args.mode in DEVICE_MODES, "Device builds need --mode development, ad-hoc or app-store")
        common.require(args.mode != "app-store" or args.release, "App Store builds must be --release")
        common.require(args.mode != "app-store" or args.icon, "App Store builds need --icon (1024 x 1024 PNG)")
    else:
        common.require(args.mode in (None, "simulator"), "Simulator targets are always ad-hoc signed; drop --mode")
    usage = usage_descriptions(args.usage)
    info = info_plist(args.name, args.binary, args.bundle_id, args.minimum_ios, device=device,
                      version=args.version, build=args.build_number, usage=usage)
    signer = signing(args) if device else None
    manifest = Path(args.manifest).resolve()
    output = Path(args.output).resolve()
    common.require(not output.exists(), "Use a new output directory")
    metadata = json.loads(common.run(["cargo", "metadata", "--locked", "--no-deps",
                                     "--format-version", "1", "--manifest-path", manifest]))
    packages = [p for p in metadata["packages"] if p["name"] == args.package and p["id"] in metadata["workspace_members"]]
    common.require(len(packages) == 1 and any(t["name"] == args.binary and "bin" in t["kind"] for t in packages[0]["targets"]),
                   "Selected Cargo package does not own this executable")
    arch = (DEVICE_TARGETS if device else SIMULATOR_TARGETS)[args.target]
    os.environ["IPHONEOS_DEPLOYMENT_TARGET"] = args.minimum_ios
    if not device:
        embed_simulator_entitlements(Path(metadata["target_directory"]), args.bundle_id, args.target)
    executable = common.build_executable(manifest, args.package, args.binary, args.target,
                                         not args.release, args.minimum_ios, args.features, True, packages[0]["id"],
                                         common.cargo_profile(not args.release, args.size))
    check_binary(executable, args.minimum_ios, arch, "IOS" if device else "IOSSIMULATOR")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".taim-ios-", dir=output.parent) as temporary:
        scratch = Path(temporary)
        product = scratch / "product"
        bundle = product / (args.name + ".app")
        bundle.mkdir(parents=True)
        shutil.copy2(executable, bundle / args.binary)
        (bundle / args.binary).chmod(0o755)
        if args.icon:
            info.update(compile_icon(args.icon, bundle, args.minimum_ios, device, scratch))
        if device:
            info.update(platform_keys(True))
        (bundle / "Info.plist").write_bytes(plistlib.dumps(info, fmt=plistlib.FMT_BINARY if device else plistlib.FMT_XML))
        common.run(["plutil", "-lint", bundle / "Info.plist"])
        if args.privacy_manifest:
            privacy = Path(args.privacy_manifest).resolve()
            manifest_data = plistlib.loads(privacy.read_bytes())
            common.require(isinstance(manifest_data.get("NSPrivacyTracking"), bool),
                           "Privacy manifest must declare NSPrivacyTracking")
            shutil.copy2(privacy, bundle / "PrivacyInfo.xcprivacy")
        report = {"bundle": str(output / bundle.name), "bundle_id": args.bundle_id,
                  "target": args.target, "architecture": arch, "simulator_only": not device,
                  "profile": common.cargo_profile(not args.release, args.size),
                  "executable_bytes": executable.stat().st_size,
                  "usage_descriptions": sorted(usage), "icon": bool(args.icon)}
        if device:
            team, digest, name, profile_path, profile = signer
            shutil.copy2(profile_path, bundle / "embedded.mobileprovision")
            entitlements = entitlements_for(profile, args.bundle_id, team)
            entitlements_path = scratch / "entitlements.plist"
            entitlements_path.write_bytes(plistlib.dumps(entitlements))
            common.run(["codesign", "--force", "--sign", digest, "--entitlements", entitlements_path,
                        "--generate-entitlement-der", "--timestamp=none" if args.mode == "development" else "--timestamp",
                        bundle])
            common.run(["codesign", "--verify", "--strict", "--deep", bundle])
            payload = scratch / "Payload"
            payload.mkdir()
            shutil.copytree(bundle, payload / bundle.name, symlinks=True)
            ipa = product / (args.name + ".ipa")
            common.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", payload, ipa])
            report.update({"signature": name, "mode": args.mode, "team": team,
                           "profile_name": profile.get("Name"), "profile_uuid": profile.get("UUID"),
                           "profile_expires": profile["ExpirationDate"].isoformat(),
                           "entitlements": entitlements, "ipa": str(output / ipa.name),
                           "ipa_sha256": common.sha256(ipa)})
        else:
            common.run(["codesign", "--force", "--sign", "-", bundle])
            common.run(["codesign", "--verify", "--strict", bundle])
            report["signature"] = "ad-hoc"
        source = subprocess.run(["git", "-C", metadata["workspace_root"], "rev-parse", "HEAD"], capture_output=True, text=True)
        status = subprocess.run(["git", "-C", metadata["workspace_root"], "status", "--porcelain"], capture_output=True, text=True)
        report.update({"signature_verified": True, "executable_sha256": common.sha256(bundle / args.binary),
                       "source_commit": source.stdout.strip() if source.returncode == 0 else None,
                       "source_dirty": bool(status.stdout) if status.returncode == 0 else None,
                       "cargo_lock_sha256": common.sha256(Path(metadata["workspace_root"]) / "Cargo.lock"),
                       "rustc": common.run(["rustc", "--version"]).decode().strip(),
                       "sdk": common.run(["xcrun", "--sdk", "iphoneos" if device else "iphonesimulator",
                                          "--show-sdk-version"]).decode().strip(),
                       "built_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
                       "upload_performed": False,
                       "runtime_acceptance": "not performed by packaging", "production_clearance": False})
        (product / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        shutil.move(str(product), output)
    print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--manifest", default="Cargo.toml")
    parser.add_argument("--package", required=True)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--bundle-id", required=True)
    parser.add_argument("--minimum-ios", default="15.0")
    parser.add_argument("--version", default="0.1.0", help="CFBundleShortVersionString")
    parser.add_argument("--build-number", default="1", help="CFBundleVersion; must increase for each upload")
    parser.add_argument("--target", choices=[*SIMULATOR_TARGETS, *DEVICE_TARGETS], default="aarch64-apple-ios-sim")
    parser.add_argument("--mode", choices=DEVICE_MODES, help="Device signing mode")
    parser.add_argument("--team", help="Apple Developer Team ID (or TAIM_IOS_TEAM_ID)")
    parser.add_argument("--identity", help="Signing identity name or SHA-1 (or TAIM_IOS_IDENTITY)")
    parser.add_argument("--profile", help="Path to the .mobileprovision (or TAIM_IOS_PROFILE)")
    parser.add_argument("--usage", action="append", metavar="KEY=TEXT",
                        help="Info.plist usage description, e.g. NSCameraUsageDescription='Scan receipts'")
    parser.add_argument("--icon", help="1024 x 1024 PNG app icon without alpha")
    parser.add_argument("--features", default="")
    parser.add_argument("--release", action="store_true")
    parser.add_argument("--size", action="store_true",
                        help="With --release: the release-small Cargo profile, ~40%% smaller, slower software rendering")
    parser.add_argument("--privacy-manifest")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    try:
        common.require(sys.platform == "darwin", "Run on macOS with Xcode installed")
        build(args)
    except (common.PackagingError, OSError, ValueError, KeyError) as error:
        print(f"iOS packaging: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
