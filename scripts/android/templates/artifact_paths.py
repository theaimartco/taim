"""Select only this Android project's actual Cargo cdylib output."""
from pathlib import Path
import re


def cdylib_output(metadata, manifest, target, profile):
    manifest = Path(manifest).resolve()
    packages = [p for p in metadata["packages"] if Path(p["manifest_path"]).resolve() == manifest]
    if len(packages) != 1:
        raise ValueError("Cargo metadata does not identify exactly one application manifest")
    libraries = [t for t in packages[0]["targets"] if "cdylib" in t["crate_types"]]
    if len(libraries) != 1 or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", libraries[0]["name"]):
        raise ValueError("Expected exactly one named Android cdylib")
    library = Path(metadata["target_directory"]) / target / profile / ("lib" + libraries[0]["name"] + ".so")
    if not library.is_file():
        raise ValueError(f"Application's own cdylib is missing: {library}")
    return library
