#!/usr/bin/env python3
"""Build sandboxed Taim .app bundles and signed Mac App Store installers.

Requires Python 3.11+, Xcode, and macOS. No Python packages are needed.
Signing credentials are read only from the environment / existing keychain.
"""

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib


class PackagingError(Exception):
    pass


TARGETS = {"arm64": "aarch64-apple-darwin", "x86_64": "x86_64-apple-darwin"}
SIGNING_ENV = ("TAIM_MACOS_APP_IDENTITY", "TAIM_MACOS_INSTALLER_IDENTITY",
               "TAIM_MACOS_TEAM_ID", "TAIM_MACOS_PROFILE")


def require(condition, message):
    if not condition:
        raise PackagingError(message)


def run(args, *, env=None):
    result = subprocess.run([str(arg) for arg in args], env=env, capture_output=True)
    if result.returncode:
        raise PackagingError(f"{args[0]} failed ({result.returncode}):\n"
                             + result.stderr.decode(errors="replace")[-12000:])
    return result.stdout


def sha256(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def config_at(path):
    path = Path(path).resolve()
    with path.open("rb") as source:
        config = tomllib.load(source)
    app = config.get("app", {})
    for field in ("name", "bundle_id", "version", "build", "minimum_system_version", "category", "copyright"):
        require(isinstance(app.get(field), str) and app[field].strip(), f"app.{field} must be a nonempty string")
    require(not any(c in app["name"] for c in '/\\:\x00\r\n') and app["name"] not in (".", ".."), "Invalid app name")
    require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9-]*(\.[A-Za-z0-9][A-Za-z0-9-]*)+", app["bundle_id"]), "Invalid bundle ID")
    for field in ("version", "build"):
        require(re.fullmatch(r"[0-9]+(\.[0-9]+){0,2}", app[field]), f"app.{field} must have one to three numeric components")
    require(re.fullmatch(r"[0-9]+\.[0-9]+(\.[0-9]+)?", app["minimum_system_version"]), "Invalid minimum macOS version")
    require(app["category"].startswith("public.app-category."), "Invalid app category")
    require(set(config) <= {"app", "entitlements", "usage_descriptions"}, "Unknown configuration table")
    require(set(app) <= {"name", "bundle_id", "version", "build", "minimum_system_version", "category", "copyright", "icon", "privacy_manifest"}, "Unknown app configuration field")
    config["root"] = path.parent
    return config


def entitlements_for(config, team=None):
    entitlements = {"com.apple.security.app-sandbox": True}
    entitlements.update(config.get("entitlements", {}))
    require(entitlements["com.apple.security.app-sandbox"] is True, "App Sandbox cannot be disabled")
    # This tool packages an app, never a helper or a debugging executable.
    for name in ("com.apple.security.get-task-allow", "get-task-allow", "com.apple.security.inherit",
                 "com.apple.security.cs.disable-library-validation", "com.apple.security.cs.allow-unsigned-executable-memory",
                 "com.apple.security.cs.allow-jit"):
        require(not entitlements.get(name), f"Unsupported distribution entitlement: {name}")
    for name in ("com.apple.application-identifier", "application-identifier", "com.apple.developer.team-identifier"):
        require(name not in entitlements, f"{name} is derived from the provisioning profile, not configuration")
    if team:
        entitlements["com.apple.application-identifier"] = team + "." + config["app"]["bundle_id"]
        entitlements["com.apple.developer.team-identifier"] = team
    return entitlements


def profile_for(path, bundle_id, team, now=None):
    """Check profile identity and distribution policy before using its contents."""
    profile = plistlib.loads(run(["security", "cms", "-D", "-i", path]))
    return check_profile(profile, bundle_id, team, now)


def check_profile(profile, bundle_id, team, now=None):
    now = now or dt.datetime.now(dt.timezone.utc)
    expires = profile.get("ExpirationDate")
    require(isinstance(expires, dt.datetime), "Provisioning profile has no expiration date")
    require(expires.replace(tzinfo=dt.timezone.utc) > now, "Provisioning profile has expired")
    require(team in profile.get("TeamIdentifier", []), "Provisioning profile belongs to a different team")
    require("OSX" in profile.get("Platform", []), "Use a macOS provisioning profile, not an iOS profile")
    allowed = profile.get("Entitlements", {})
    identifier = allowed.get("com.apple.application-identifier", allowed.get("application-identifier"))
    require(identifier == team + "." + bundle_id, "Use an explicit App Store profile matching this bundle ID and team")
    require(not allowed.get("get-task-allow") and not allowed.get("com.apple.security.get-task-allow"), "Development provisioning profile cannot be used for the App Store")
    require(not profile.get("ProvisionedDevices") and not profile.get("ProvisionsAllDevices"), "Use a Mac App Store distribution profile")
    require(profile.get("DeveloperCertificates"), "Provisioning profile has no distribution certificate")
    return profile


def identity(value, installer=False):
    args = ["security", "find-identity", "-v"]
    if not installer:
        args += ["-p", "codesigning"]
    found = re.findall(r'\b([0-9A-Fa-f]{40})\s+"([^"]+)"', run(args).decode())
    matches = [(digest, name) for digest, name in found if value in (digest, name)]
    require(len(matches) == 1, "Signing identity not found or ambiguous in the current keychain")
    digest, name = matches[0]
    prefixes = ("3rd Party Mac Developer Installer:", "Mac Installer Distribution:") if installer else ("Apple Distribution:", "3rd Party Mac Developer Application:", "Mac App Distribution:")
    require(name.startswith(prefixes), "Use a Mac App Store distribution identity, not Apple Development or Developer ID")
    if installer:
        require(sum(candidate == name for _, candidate in found) == 1,
                "Installer certificate name is ambiguous; remove/disable the duplicate identity before packaging")
    return digest.upper(), name


def signing_inputs(config):
    missing = [name for name in SIGNING_ENV if not os.environ.get(name)]
    require(not missing, "App Store signing needs: " + ", ".join(missing))
    team = os.environ["TAIM_MACOS_TEAM_ID"]
    require(re.fullmatch(r"[A-Z0-9]{10}", team), "Invalid TAIM_MACOS_TEAM_ID")
    require(not config["app"]["bundle_id"].startswith(("com.example.", "org.example.")), "Replace the example bundle ID with your registered App ID")
    profile_path = Path(os.environ["TAIM_MACOS_PROFILE"]).expanduser().resolve()
    profile = profile_for(profile_path, config["app"]["bundle_id"], team)
    app_digest, app_name = identity(os.environ["TAIM_MACOS_APP_IDENTITY"])
    _, installer_name = identity(os.environ["TAIM_MACOS_INSTALLER_IDENTITY"], installer=True)
    require(app_name.endswith(f"({team})") and installer_name.endswith(f"({team})"), "App and installer identities must belong to TAIM_MACOS_TEAM_ID")
    certificate_digests = {hashlib.sha1(cert).hexdigest().upper() for cert in profile["DeveloperCertificates"]}
    require(app_digest in certificate_digests, "App signing certificate is not authorized by the provisioning profile")
    # productbuild's documented --sign argument is the certificate Common Name.
    return team, profile_path, app_digest, installer_name


def info_plist(config, executable):
    app = config["app"]
    info = {
        "CFBundleName": app["name"], "CFBundleDisplayName": app["name"],
        "CFBundleIdentifier": app["bundle_id"], "CFBundleExecutable": executable,
        "CFBundlePackageType": "APPL", "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleSupportedPlatforms": ["MacOSX"], "CFBundleDevelopmentRegion": "en",
        "CFBundleShortVersionString": app["version"], "CFBundleVersion": app["build"],
        "LSMinimumSystemVersion": app["minimum_system_version"],
        "LSApplicationCategoryType": app["category"], "NSHumanReadableCopyright": app["copyright"],
        "NSHighResolutionCapable": True,
    }
    for key, value in config.get("usage_descriptions", {}).items():
        require(re.fullmatch(r"NS[A-Za-z]+UsageDescription", key) and isinstance(value, str) and value.strip(), "Invalid usage description")
        info[key] = value
    return info


def compile_icon(config, resources, scratch):
    app = config["app"]
    require(app.get("icon"), "Set app.icon to a 1024 x 1024 PNG or an .icns file")
    source = (config["root"] / app["icon"]).resolve()
    require(source.is_file(), f"App icon missing: {source}")
    destination = resources / "AppIcon.icns"
    if source.suffix.lower() == ".icns":
        shutil.copy2(source, destination)
        # iconutil validates the input, including the high-resolution representation.
        iconset = scratch / "Validate.iconset"
        run(["iconutil", "--convert", "iconset", "--output", iconset, destination])
        require((iconset / "icon_512x512@2x.png").is_file(), "Icon must include a 1024px representation")
        return
    require(source.suffix.lower() == ".png", "Icon must be PNG or ICNS")
    dimensions = run(["sips", "-g", "pixelWidth", "-g", "pixelHeight", source]).decode()
    require(re.search(r"pixelWidth:\s+1024\b", dimensions) and re.search(r"pixelHeight:\s+1024\b", dimensions), "PNG icon must be exactly 1024 x 1024")
    iconset = scratch / "AppIcon.iconset"
    iconset.mkdir()
    for size in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            name = f"icon_{size}x{size}" + ("@2x" if scale == 2 else "") + ".png"
            run(["sips", "-z", str(size * scale), str(size * scale), source, "--out", iconset / name])
    run(["iconutil", "--convert", "icns", "--output", destination, iconset])


def cargo_profile(debug, size=False):
    """The Cargo profile to build: `release-small` trades about a quarter of
    software-rendering speed for a ~40% smaller executable."""
    require(not (debug and size), "--size builds are optimized; drop --debug")
    return "dev" if debug else "release-small" if size else "release"


def build_executable(manifest, package, binary, target, debug, minimum, features, locked, package_id=None, profile=None):
    args = ["cargo", "build", "--manifest-path", manifest, "-p", package, "--bin", binary,
            "--target", target, "--message-format=json-render-diagnostics"]
    if locked:
        args.append("--locked")
    profile = profile or cargo_profile(debug)
    if profile != "dev":
        args += ["--profile", profile]
    if features:
        args += ["--features", features]
    env = dict(os.environ, MACOSX_DEPLOYMENT_TARGET=minimum)
    print(f"Building {package}/{binary} for {target} ({profile})…", flush=True)
    # Cargo's artifact messages identify the selected application's executable,
    # including shared target directories and custom binary names.
    result = subprocess.run([str(a) for a in args], env=env, stdout=subprocess.PIPE, text=True)
    outputs = []
    for line in result.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") == "compiler-message":
            rendered = message.get("message", {}).get("rendered")
            if rendered:
                print(rendered, file=sys.stderr, end="")
        if (message.get("reason") == "compiler-artifact" and message.get("executable")
                and message.get("target", {}).get("name") == binary
                and (package_id is None or message.get("package_id") == package_id)
                and "bin" in message.get("target", {}).get("kind", [])):
            outputs.append(Path(message["executable"]))
    require(result.returncode == 0, "Cargo build failed; no bundle was produced")
    require(len(set(outputs)) == 1 and outputs[0].is_file(), "Cargo did not report exactly one application executable")
    return outputs[0]


def version_tuple(value):
    return tuple((list(map(int, value.split("."))) + [0, 0])[:3])


def check_binary(binary, minimum, expected_arches=None):
    arches = run(["lipo", "-archs", binary]).decode().split()
    require(set(arches) <= {"arm64", "x86_64"} and arches, "Expected a macOS arm64/x86_64 Mach-O executable")
    if expected_arches:
        require(set(arches) == set(expected_arches), "Executable architecture does not match the requested architecture")
    builds = run(["vtool", "-show-build", binary]).decode()
    require(len(re.findall(r"\bplatform\s+MACOS\b", builds)) == len(arches), "Executable is not built for macOS")
    minimums = re.findall(r"\bminos\s+([0-9.]+)", builds)
    require(len(minimums) == len(arches), "Missing Mach-O deployment target metadata")
    require(all(version_tuple(v) <= version_tuple(minimum) for v in minimums), "Executable requires a newer macOS than LSMinimumSystemVersion")
    libraries = run(["otool", "-L", binary]).decode().splitlines()
    for line in libraries:
        if not line.startswith("\t"):
            continue
        library = line.strip().split(" (", 1)[0]
        require(library.startswith(("/System/Library/", "/usr/lib/")), f"Non-system dynamic library needs explicit bundling/signing support: {library}")
    return arches


def signed_entitlements(bundle):
    data = run(["codesign", "--display", "--entitlements", "-", "--xml", bundle])
    start = data.find(b"<?xml")
    require(start >= 0, "App signature has no XML entitlements")
    return plistlib.loads(data[start:])


def validate_bundle(bundle, app_store=False):
    bundle = Path(bundle).resolve()
    require(bundle.suffix == ".app" and bundle.is_dir(), "Expected an existing .app bundle")
    info = plistlib.loads((bundle / "Contents/Info.plist").read_bytes())
    binary = bundle / "Contents/MacOS" / info["CFBundleExecutable"]
    require(binary.parent == bundle / "Contents/MacOS", "Invalid executable path in Info.plist")
    require(info["CFBundlePackageType"] == "APPL", "Not an application bundle")
    require((bundle / "Contents/Resources" / info["CFBundleIconFile"]).is_file(), "Missing bundled icon")
    privacy = plistlib.loads((bundle / "Contents/Resources/PrivacyInfo.xcprivacy").read_bytes())
    require(isinstance(privacy.get("NSPrivacyTracking"), bool), "Privacy manifest must declare tracking policy")
    run(["plutil", "-lint", bundle / "Contents/Info.plist"])
    run(["codesign", "--verify", "--strict", "--verbose=2", bundle])
    entitlements = signed_entitlements(bundle)
    require(entitlements.get("com.apple.security.app-sandbox") is True, "Signed app is missing App Sandbox")
    for key in ("com.apple.security.get-task-allow", "get-task-allow", "com.apple.security.inherit",
                "com.apple.security.cs.allow-jit", "com.apple.security.cs.allow-unsigned-executable-memory",
                "com.apple.security.cs.disable-library-validation"):
        require(not entitlements.get(key), f"Unsafe app entitlement: {key}")
    arches = check_binary(binary, info["LSMinimumSystemVersion"])
    if app_store:
        details = subprocess.run(["codesign", "--display", "--verbose=4", str(bundle)], capture_output=True, text=True)
        require(details.returncode == 0, "Cannot inspect signing identity")
        require("Signature=adhoc" not in details.stderr, "Ad-hoc signed apps cannot be uploaded")
        require(re.search(r"Authority=(Apple Distribution:|3rd Party Mac Developer Application:|Mac App Distribution:)", details.stderr), "App must use an App Store distribution signature")
        team = entitlements.get("com.apple.developer.team-identifier")
        require(team, "Distribution app has no signing team")
        require(f"TeamIdentifier={team}" in details.stderr, "Signature and entitlement teams differ")
        require(entitlements.get("com.apple.application-identifier") == team + "." + info["CFBundleIdentifier"], "Distribution app identifier does not match bundle")
        profile = profile_for(bundle / "Contents/embedded.provisionprofile", info["CFBundleIdentifier"], team)
        with tempfile.TemporaryDirectory(prefix="taim-signature-") as temporary:
            prefix = Path(temporary) / "certificate"
            run(["codesign", "--display", "--extract-certificates", prefix, bundle])
            certificate = prefix.with_name(prefix.name + "0").read_bytes()
        require(certificate in profile["DeveloperCertificates"], "Signature certificate is not authorized by embedded profile")
    return {"bundle": str(bundle), "bundle_id": info["CFBundleIdentifier"], "architectures": arches,
            "sandbox": True, "signature_verified": True, "app_store_local_checks": app_store,
            "executable_sha256": sha256(binary)}


def build(args):
    config = config_at(args.config)
    entitlements_for(config)  # Reject invalid policies before building or signing.
    require(not (args.mode == "app-store" and args.debug), "App Store mode requires an optimized release build")
    cargo_profile(args.debug, args.size)  # Reject --debug --size before building.
    signing = signing_inputs(config) if args.mode == "app-store" else None
    manifest = Path(args.manifest).resolve()
    require(manifest.is_file(), "Cargo manifest not found")
    output = Path(args.output).resolve()
    require(not output.exists(), "Output directory already exists; use a new directory to retain release evidence")
    output.parent.mkdir(parents=True, exist_ok=True)
    arches = list(TARGETS) if args.arch == "universal" else [args.arch or platform.machine()]
    require(all(a in TARGETS for a in arches), "Unsupported Mac architecture")
    metadata_command = ["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", manifest]
    if args.mode == "app-store":
        metadata_command.append("--locked")
    metadata = json.loads(run(metadata_command))
    lock = Path(metadata["workspace_root"]) / "Cargo.lock"
    require(args.mode != "app-store" or lock.is_file(), "App Store builds require Cargo.lock; run cargo generate-lockfile first")
    packages = [p for p in metadata["packages"] if p["name"] == args.package and p["id"] in metadata["workspace_members"]]
    require(len(packages) == 1, "Select exactly one Cargo workspace package")
    require(any(t["name"] == args.binary and "bin" in t["kind"] for t in packages[0]["targets"]), "Selected package does not own this binary")
    app = config["app"]
    with tempfile.TemporaryDirectory(prefix=".taim-macos-", dir=output.parent) as temporary:
        scratch = Path(temporary)
        product = scratch / "product"
        product.mkdir()
        bundle = product / (app["name"] + ".app")
        contents = bundle / "Contents"
        resources = contents / "Resources"
        resources.mkdir(parents=True)
        (contents / "MacOS").mkdir()
        info = info_plist(config, args.binary)
        compile_icon(config, resources, scratch)
        info["CFBundleIconFile"] = "AppIcon.icns"
        require(app.get("privacy_manifest"), "Set app.privacy_manifest to the application's reviewed PrivacyInfo.xcprivacy")
        privacy_source = config["root"] / app["privacy_manifest"]
        plistlib.loads(privacy_source.read_bytes())
        shutil.copy2(privacy_source, resources / "PrivacyInfo.xcprivacy")
        (contents / "Info.plist").write_bytes(plistlib.dumps(info))
        (contents / "PkgInfo").write_bytes(b"APPL????")
        executables = [build_executable(manifest, args.package, args.binary, TARGETS[arch], args.debug,
                                      app["minimum_system_version"], args.features, lock.is_file(), packages[0]["id"],
                                      cargo_profile(args.debug, args.size)) for arch in arches]
        executable = contents / "MacOS" / args.binary
        if len(executables) == 1:
            shutil.copy2(executables[0], executable)
        else:
            run(["lipo", "-create", *executables, "-output", executable])
        executable.chmod(0o755)
        check_binary(executable, app["minimum_system_version"], arches)
        team, profile, app_identity, installer_identity = signing or (None, None, "-", None)
        entitlements = entitlements_for(config, team)
        entitlement_path = scratch / "entitlements.plist"
        entitlement_path.write_bytes(plistlib.dumps(entitlements))
        if profile:
            shutil.copy2(profile, contents / "embedded.provisionprofile")
        command = ["codesign", "--force", "--sign", app_identity, "--entitlements", entitlement_path]
        if signing:
            command.append("--timestamp")
        command.append(bundle)
        run(command)
        report = validate_bundle(bundle, app_store=bool(signing))
        report.update({"mode": args.mode, "profile": cargo_profile(args.debug, args.size),
                       "executable_bytes": executable.stat().st_size,
                       "package": args.package, "binary": args.binary,
                       "built_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
                       "cargo_lock_sha256": sha256(lock), "config_sha256": sha256(args.config),
                       "apple_upload": "not attempted", "runtime_acceptance": "not performed by packaging tool"})
        report["rustc"] = run(["rustc", "--version"]).decode().strip()
        report["sdk_version"] = run(["xcrun", "--sdk", "macosx", "--show-sdk-version"]).decode().strip()
        source = subprocess.run(["git", "-C", metadata["workspace_root"], "rev-parse", "HEAD"], capture_output=True, text=True)
        report["source_commit"] = source.stdout.strip() if source.returncode == 0 else None
        dirty = subprocess.run(["git", "-C", metadata["workspace_root"], "status", "--porcelain"], capture_output=True, text=True)
        report["source_dirty"] = bool(dirty.stdout) if dirty.returncode == 0 else None
        if signing:
            installer = product / (app["name"] + ".pkg")
            run(["productbuild", "--sign", installer_identity, "--timestamp", "--component", bundle, "/Applications", installer])
            run(["pkgutil", "--check-signature", installer])
            report["installer_sha256"] = sha256(installer)
        # Publish the whole completed product only after every local check passes.
        report["bundle"] = str(output / bundle.name)
        (product / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        shutil.move(str(product), output)
    print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    package = commands.add_parser("build", help="Build, sandbox, sign and validate a .app; app-store mode also signs a .pkg")
    package.add_argument("--config", required=True)
    package.add_argument("--manifest", default="Cargo.toml")
    package.add_argument("--package", required=True)
    package.add_argument("--binary", required=True)
    package.add_argument("--output", required=True, help="New output directory (never overwritten)")
    package.add_argument("--arch", choices=[*TARGETS, "universal"])
    package.add_argument("--mode", choices=["development", "app-store"], default="development")
    package.add_argument("--debug", action="store_true", help="Use dev profile for local smoke only")
    package.add_argument("--size", action="store_true",
                         help="Build with the release-small Cargo profile: ~40%% smaller, slower software rendering")
    package.add_argument("--features", default="")
    validate = commands.add_parser("validate", help="Verify a packaged app locally without uploading")
    validate.add_argument("bundle")
    validate.add_argument("--app-store", action="store_true")
    args = parser.parse_args()
    try:
        require(sys.platform == "darwin", "Run this tool on macOS with Xcode installed")
        if args.command == "build":
            build(args)
        else:
            print(json.dumps(validate_bundle(args.bundle, args.app_store), indent=2))
    except (PackagingError, OSError, ValueError, KeyError, plistlib.InvalidFileException) as error:
        print(f"macOS packaging: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
