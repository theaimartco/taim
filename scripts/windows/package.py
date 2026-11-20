#!/usr/bin/env python3
"""Build MSIX packages of Taim applications for the Microsoft Store.

  python scripts/windows/package.py layout --config scripts/windows/counter.toml \\
      --package counter --binary counter --output target/msix/counter
  python scripts/windows/package.py pack --config scripts/windows/counter.toml \\
      --package counter --binary counter --output target/msix/counter [--sign]

`layout` builds the executable and writes the package folder: the manifest,
the logos and the executable. It runs anywhere a Windows target can be built.
`pack` also runs `makeappx` (and `signtool` with --sign), so it needs Windows
and the Windows SDK. The Store signs what it publishes; --sign is for
sideloading and needs TAIM_WINDOWS_CERTIFICATE (a .pfx) and
TAIM_WINDOWS_CERTIFICATE_PASSWORD. Nothing is uploaded.

Logos are drawn from one square PNG with Pillow (`pip install Pillow`).
The C runtime is linked statically, so the package needs no VCLibs
framework dependency.
"""

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib
from xml.sax.saxutils import escape, quoteattr


class PackagingError(Exception):
    pass


TARGETS = {"x64": "x86_64-pc-windows-msvc", "arm64": "aarch64-pc-windows-msvc"}
# Windows 10 1809: the oldest release the Store still takes MSIX for.
MINIMUM = "10.0.17763.0"
TESTED = "10.0.26100.0"
# Declared capabilities a settings-and-network app may need; device access
# beyond these needs review of the application, not a config switch.
CAPABILITIES = {"internetClient", "internetClientServer", "privateNetworkClientServer"}
# Name and pixel size of each logo the manifest names.
LOGOS = {
    "StoreLogo.png": 50,
    "Square44x44Logo.png": 44,
    "Square150x150Logo.png": 150,
    "Square310x310Logo.png": 310,
}
WIDE = ("Wide310x150Logo.png", (310, 150))


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
    require(set(config) <= {"app"}, "Unknown configuration table")
    app = config.get("app", {})
    fields = ("name", "identity", "publisher", "publisher_display_name", "version", "description", "icon")
    for field in fields:
        require(isinstance(app.get(field), str) and app[field].strip(), f"app.{field} must be a nonempty string")
    require(set(app) <= {*fields, "background", "capabilities"}, "Unknown app configuration field")
    # Partner Center reserves the package identity name and publisher; both
    # must be copied from "Product identity" exactly.
    require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9.-]{1,48}[A-Za-z0-9]", app["identity"]), "Invalid app.identity (Package/Identity/Name)")
    require(app["publisher"].startswith("CN="), "app.publisher must be the Partner Center publisher, starting with CN=")
    version = app["version"].split(".")
    require(len(version) == 4 and all(re.fullmatch(r"[0-9]+", part) and int(part) <= 65535 for part in version),
            "app.version must have four numeric parts, each at most 65535")
    require(version[3] == "0", "The Store reserves the fourth version part; set it to 0")
    require(len(app["description"]) <= 2048, "app.description is longer than the manifest allows")
    background = app.get("background", "transparent")
    require(background == "transparent" or re.fullmatch(r"#[0-9A-Fa-f]{6}", background), "app.background must be transparent or #RRGGBB")
    capabilities = app.get("capabilities", [])
    require(isinstance(capabilities, list) and set(capabilities) <= CAPABILITIES,
            "app.capabilities may only name: " + ", ".join(sorted(CAPABILITIES)))
    config["root"] = path.parent
    return config


def manifest(config, executable, arch):
    """The AppxManifest.xml of a full-trust desktop application."""
    require(arch in TARGETS, f"Unknown architecture: {arch}")
    require(re.fullmatch(r"[A-Za-z0-9_.-]+\.exe", executable), "Invalid executable name")
    app = config["app"]
    capabilities = "".join(f'\n    <Capability Name="{name}" />' for name in app.get("capabilities", []))
    return f"""<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap rescap">
  <Identity Name={quoteattr(app["identity"])} Publisher={quoteattr(app["publisher"])} Version={quoteattr(app["version"])} ProcessorArchitecture="{arch}" />
  <Properties>
    <DisplayName>{escape(app["name"])}</DisplayName>
    <PublisherDisplayName>{escape(app["publisher_display_name"])}</PublisherDisplayName>
    <Logo>Assets\\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="{MINIMUM}" MaxVersionTested="{TESTED}" />
  </Dependencies>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Applications>
    <Application Id="App" Executable={quoteattr(executable)} EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName={quoteattr(app["name"])} Description={quoteattr(app["description"])}
        BackgroundColor={quoteattr(app.get("background", "transparent"))}
        Square150x150Logo="Assets\\Square150x150Logo.png" Square44x44Logo="Assets\\Square44x44Logo.png">
        <uap:DefaultTile Wide310x150Logo="Assets\\Wide310x150Logo.png" Square310x310Logo="Assets\\Square310x310Logo.png" />
      </uap:VisualElements>
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />{capabilities}
  </Capabilities>
</Package>
"""


def draw_logos(config, assets):
    try:
        from PIL import Image
    except ImportError as error:
        raise PackagingError("Logos need Pillow: pip install Pillow") from error
    source = (config["root"] / config["app"]["icon"]).resolve()
    require(source.is_file() and source.suffix.lower() == ".png", f"app.icon must be a PNG: {source}")
    with Image.open(source) as icon:
        require(icon.width == icon.height and icon.width >= 310, "app.icon must be square and at least 310 px")
        icon = icon.convert("RGBA")
        assets.mkdir(parents=True)
        for name, size in LOGOS.items():
            icon.resize((size, size), Image.LANCZOS).save(assets / name)
        # The wide tile centres the icon on a transparent banner.
        name, (width, height) = WIDE
        wide = Image.new("RGBA", (width, height))
        mark = icon.resize((height, height), Image.LANCZOS)
        wide.paste(mark, ((width - height) // 2, 0), mark)
        wide.save(assets / name)


def build_executable(manifest_path, package, binary, target, features, locked):
    args = ["cargo", "build", "--manifest-path", manifest_path, "-p", package, "--bin", binary,
            "--target", target, "--release", "--message-format=json-render-diagnostics"]
    if locked:
        args.append("--locked")
    if features:
        args += ["--features", features]
    # A static C runtime: the package then runs without the VCLibs framework.
    flags = " ".join(filter(None, [os.environ.get("RUSTFLAGS", ""), "-C target-feature=+crt-static"]))
    env = dict(os.environ, RUSTFLAGS=flags)
    print(f"Building {package}/{binary} for {target}…", flush=True)
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
                and "bin" in message.get("target", {}).get("kind", [])):
            outputs.append(Path(message["executable"]))
    require(result.returncode == 0, "Cargo build failed; no package was produced")
    require(len(set(outputs)) == 1 and outputs[0].is_file(), "Cargo did not report exactly one application executable")
    return outputs[0]


def check_executable(path):
    """A Windows GUI executable for the requested machine, not a console one."""
    data = Path(path).read_bytes()
    require(data[:2] == b"MZ", "Not a Windows executable")
    header = int.from_bytes(data[0x3C:0x40], "little")
    require(data[header:header + 4] == b"PE\0\0", "Not a PE executable")
    machine = int.from_bytes(data[header + 4:header + 6], "little")
    optional = header + 24
    subsystem = int.from_bytes(data[optional + 68:optional + 70], "little")
    require(subsystem == 2, "The executable opens a console window: add "
            '#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")] to main.rs')
    return {0x8664: "x64", 0xAA64: "arm64"}.get(machine)


def layout(args, config):
    output = Path(args.output).resolve()
    require(not output.exists(), f"Output exists; choose a new directory: {output}")
    lock = Path(args.manifest).resolve().parent / "Cargo.lock"
    if args.executable:
        executable = Path(args.executable).resolve()
        require(executable.is_file(), f"Executable missing: {executable}")
    else:
        executable = build_executable(args.manifest, args.package, args.binary, TARGETS[args.arch],
                                      args.features, lock.is_file())
    machine = check_executable(executable)
    require(machine == args.arch, f"Executable is built for {machine}, not {args.arch}")
    folder = output / "layout"
    draw_logos(config, folder / "Assets")
    name = args.binary + ".exe"
    shutil.copy2(executable, folder / name)
    (folder / "AppxManifest.xml").write_text(manifest(config, name, args.arch), encoding="utf-8")
    return output, folder, name


def pack(args, config):
    require(sys.platform == "win32", "makeappx and signtool need Windows and the Windows SDK")
    output, folder, _ = layout(args, config)
    app = config["app"]
    package = output / f"{app['identity']}_{app['version']}_{args.arch}.msix"
    run(["makeappx", "pack", "/d", folder, "/p", package, "/o"])
    signed = False
    if args.sign:
        certificate = os.environ.get("TAIM_WINDOWS_CERTIFICATE")
        password = os.environ.get("TAIM_WINDOWS_CERTIFICATE_PASSWORD")
        require(certificate and password, "--sign needs TAIM_WINDOWS_CERTIFICATE and TAIM_WINDOWS_CERTIFICATE_PASSWORD")
        run(["signtool", "sign", "/fd", "SHA256", "/f", certificate, "/p", password,
             "/tr", "http://timestamp.digicert.com", "/td", "SHA256", package])
        signed = True
    return output, package, signed


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    for command, text in [("layout", "Write the package folder (any OS)"),
                          ("pack", "Write the folder and pack an .msix (Windows)")]:
        sub = commands.add_parser(command, help=text)
        sub.add_argument("--config", required=True)
        sub.add_argument("--manifest", default="Cargo.toml")
        sub.add_argument("--package", required=True)
        sub.add_argument("--binary", required=True)
        sub.add_argument("--output", required=True, help="New output directory (never overwritten)")
        sub.add_argument("--arch", choices=list(TARGETS), default="x64")
        sub.add_argument("--features", default="")
        sub.add_argument("--executable", help="Package this already built executable instead of building")
        if command == "pack":
            sub.add_argument("--sign", action="store_true", help="Sign for sideloading; the Store signs uploads itself")
    args = parser.parse_args()
    try:
        config = config_at(args.config)
        report = {"identity": config["app"]["identity"], "version": config["app"]["version"], "arch": args.arch,
                  "built_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
                  "store_upload": "not attempted", "runtime_acceptance": "not performed by packaging tool"}
        if args.command == "layout":
            output, folder, _ = layout(args, config)
            report["layout"] = str(folder)
        else:
            output, package, signed = pack(args, config)
            report.update({"msix": str(package), "msix_sha256": sha256(package), "signed": signed})
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
    except (PackagingError, OSError, ValueError, KeyError) as error:
        print(f"Windows packaging: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
