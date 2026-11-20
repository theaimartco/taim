#!/usr/bin/env python3
"""Write Snapcraft projects of Taim applications for the Snap Store.

  python3 scripts/linux/package.py snap --config scripts/linux/counter.toml \\
      --package counter --binary counter --output target/snap/counter [--source URL]

The project builds the application from source with Snapcraft's Rust plugin,
on the core24 base, strictly confined. Build it on Ubuntu with
`cd OUTPUT && snapcraft pack`, try it with `sudo snap install --dangerous
*.snap`, and publish with `snapcraft upload --release=edge *.snap` after
`snapcraft register NAME`. Nothing is built or uploaded by this tool.

`--source` is the git URL the store's builders fetch (for `snapcraft
remote-build` or Launchpad); without it, the local checkout is used.
GPU drivers come from Canonical's mesa-2404 content snap.
"""

import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib


class PackagingError(Exception):
    pass


# Interfaces a windowed application uses; more must be asked for by name.
PLUGS = ["desktop", "desktop-legacy", "wayland", "x11", "gpu-2404"]
OPTIONAL_PLUGS = {"network", "home", "audio-playback"}
# winit and wgpu load these at run time rather than linking them.
RUNTIME = ["libxkbcommon0", "libxkbcommon-x11-0", "libwayland-client0", "libwayland-cursor0",
           "libwayland-egl1", "libx11-6", "libx11-xcb1", "libxcursor1", "libxi6", "libxrandr2",
           "libvulkan1", "xkb-data"]
BUILD = ["pkg-config", "libx11-dev", "libxkbcommon-dev", "libwayland-dev"]
CATEGORIES = {"AudioVideo", "Development", "Education", "Game", "Graphics", "Network",
              "Office", "Science", "Settings", "System", "Utility"}


def require(condition, message):
    if not condition:
        raise PackagingError(message)


def config_at(path):
    path = Path(path).resolve()
    with path.open("rb") as source:
        config = tomllib.load(source)
    require(set(config) <= {"app"}, "Unknown configuration table")
    app = config.get("app", {})
    fields = ("name", "title", "summary", "description", "version", "license", "icon")
    for field in fields:
        require(isinstance(app.get(field), str) and app[field].strip(), f"app.{field} must be a nonempty string")
    require(set(app) <= {*fields, "categories", "grade", "plugs"}, "Unknown app configuration field")
    # The Snap Store's naming rules.
    require(re.fullmatch(r"(?=.*[a-z])[a-z0-9](?:-?[a-z0-9])*", app["name"]) and 2 <= len(app["name"]) <= 40,
            "app.name must be 2-40 lowercase letters, digits and single hyphens")
    require(len(app["summary"]) <= 78, "app.summary must be at most 78 characters")
    require(len(app["title"]) <= 40, "app.title must be at most 40 characters")
    require(len(app["version"]) <= 32 and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9.+~-]*", app["version"]),
            "Invalid app.version")
    require(app.get("grade", "stable") in ("stable", "devel"), "app.grade must be stable or devel")
    categories = app.get("categories", ["Utility"])
    require(isinstance(categories, list) and categories and set(categories) <= CATEGORIES,
            "app.categories must name freedesktop main categories: " + ", ".join(sorted(CATEGORIES)))
    plugs = app.get("plugs", [])
    require(isinstance(plugs, list) and set(plugs) <= OPTIONAL_PLUGS,
            "app.plugs may only add: " + ", ".join(sorted(OPTIONAL_PLUGS)))
    config["root"] = path.parent
    return config


def quoted(text):
    """A YAML double-quoted scalar; JSON strings are valid ones."""
    return json.dumps(text, ensure_ascii=False)


def snapcraft(config, package_path, binary, source, features):
    app = config["app"]
    name = app["name"]
    plugs = PLUGS + list(app.get("plugs", []))
    description = "\n".join("  " + line if line else "" for line in app["description"].strip().splitlines())
    return f"""name: {name}
title: {quoted(app["title"])}
summary: {quoted(app["summary"])}
description: |
{description}
version: {quoted(app["version"])}
license: {quoted(app["license"])}
base: core24
grade: {app.get("grade", "stable")}
confinement: strict
platforms:
  amd64:
  arm64:

plugs:
  gpu-2404:
    interface: content
    target: $SNAP/gpu-2404
    default-provider: mesa-2404

apps:
  {name}:
    command: bin/{binary}
    command-chain:
      - bin/gpu-2404-wrapper
    desktop: meta/gui/{name}.desktop
    plugs:
{"".join(f"      - {plug}{chr(10)}" for plug in plugs)}    environment:
      # winit opens the staged libraries by name; the keyboard maps are
      # staged too, since the base snap has none.
      LD_LIBRARY_PATH: $SNAP/usr/lib/$CRAFT_ARCH_TRIPLET_BUILD_FOR:$LD_LIBRARY_PATH
      XKB_CONFIG_ROOT: $SNAP/usr/share/X11/xkb

parts:
  {name}:
    plugin: rust
    source: {quoted(source)}
    rust-path:
      - {quoted(package_path)}
    rust-features: {json.dumps(features)}
    build-packages: [{", ".join(BUILD)}]
    stage-packages: [{", ".join(RUNTIME)}]

  gpu-2404:
    after: [{name}]
    source: https://github.com/canonical/gpu-snap.git
    plugin: dump
    override-prime: |
      craftctl default
      ${{CRAFT_PART_SRC}}/bin/gpu-2404-cleanup mesa-2404
    prime:
      - bin/gpu-2404-wrapper
"""


def desktop_entry(config):
    app = config["app"]
    escape = lambda text: text.replace("\\", "\\\\").replace("\n", " ")
    return (
        "[Desktop Entry]\n"
        "Type=Application\n"
        f"Name={escape(app['title'])}\n"
        f"Comment={escape(app['summary'])}\n"
        f"Exec={app['name']}\n"
        f"Icon=${{SNAP}}/meta/gui/{app['name']}.png\n"
        f"Categories={';'.join(app.get('categories', ['Utility']))};\n"
        "Terminal=false\n"
        "StartupNotify=true\n"
    )


def write_icon(config, destination):
    source = (config["root"] / config["app"]["icon"]).resolve()
    require(source.is_file() and source.suffix.lower() == ".png", f"app.icon must be a PNG: {source}")
    data = source.read_bytes()
    require(data[:8] == b"\x89PNG\r\n\x1a\n", "app.icon is not a PNG file")
    width, height = int.from_bytes(data[16:20], "big"), int.from_bytes(data[20:24], "big")
    # The store shows icons from 256 px; anything else is scaled badly.
    require(width == height and width >= 256, "app.icon must be square and at least 256 px")
    shutil.copy2(source, destination)


def package_directory(manifest, package):
    metadata = json.loads(subprocess.run(
        ["cargo", "metadata", "--manifest-path", str(manifest), "--no-deps", "--format-version", "1"],
        capture_output=True, check=True, text=True).stdout)
    root = Path(metadata["workspace_root"])
    for candidate in metadata["packages"]:
        if candidate["name"] == package:
            return root, Path(candidate["manifest_path"]).parent.relative_to(root)
    raise PackagingError(f"No package named {package} in the workspace")


def snap(args, config):
    output = Path(args.output).resolve()
    require(not output.exists(), f"Output exists; choose a new directory: {output}")
    root, package_path = package_directory(Path(args.manifest).resolve(), args.package)
    source = args.source or str(root)
    features = [f for f in args.features.split(",") if f]
    gui = output / "snap" / "gui"
    gui.mkdir(parents=True)
    write_icon(config, gui / f"{config['app']['name']}.png")
    (gui / f"{config['app']['name']}.desktop").write_text(desktop_entry(config), encoding="utf-8")
    (output / "snap" / "snapcraft.yaml").write_text(
        snapcraft(config, package_path.as_posix(), args.binary, source, features), encoding="utf-8")
    validator = shutil.which("desktop-file-validate")
    if validator:
        result = subprocess.run([validator, gui / f"{config['app']['name']}.desktop"], capture_output=True, text=True)
        require(result.returncode == 0, "desktop-file-validate: " + result.stdout + result.stderr)
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    project = commands.add_parser("snap", help="Write a Snapcraft project")
    project.add_argument("--config", required=True)
    project.add_argument("--manifest", default="Cargo.toml")
    project.add_argument("--package", required=True)
    project.add_argument("--binary", required=True)
    project.add_argument("--output", required=True, help="New output directory (never overwritten)")
    project.add_argument("--source", help="Git URL the store builders fetch; default: this checkout")
    project.add_argument("--features", default="", help="Comma-separated Cargo features")
    args = parser.parse_args()
    try:
        output = snap(args, config_at(args.config))
        print(json.dumps({"project": str(output), "next": f"cd {output} && snapcraft pack",
                          "store_upload": "not attempted"}, indent=2))
    except (PackagingError, OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"Snap packaging: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
