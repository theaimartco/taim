#!/usr/bin/env python3
"""Validate all TAIM Premium icon SVG assets including Edition 03 with only the Python stdlib."""
from __future__ import annotations

import json
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
PACK = ROOT / "assets" / "icons" / "premium"
SVG_NS = "{http://www.w3.org/2000/svg}"
SAFE_TAGS = {
    "svg", "title", "desc", "defs", "linearGradient", "radialGradient",
    "stop", "filter", "feDropShadow", "style", "g", "path", "circle",
    "rect", "ellipse", "line", "polyline", "polygon",
}
FORBIDDEN = re.compile(r"(?:javascript:|data:|https?://)", re.IGNORECASE)
NAME = re.compile(r"^[a-z][a-z0-9_]*$")


def verify() -> tuple[int, list[str]]:
    errors: list[str] = []
    manifest = json.loads((PACK / "manifest.json").read_text(encoding="utf-8"))
    records = manifest["icons"]
    expected = [r["name"] for r in records]
    if len(expected) != 149 or len(set(expected)) != 149:
        errors.append(f"Expected 149 unique icons, got {len(expected)}")
    if manifest["count"] != len(expected):
        errors.append("Manifest count mismatch")
    discovered = {p.stem for p in PACK.glob("*.svg")}
    root_names = {p.stem for p in PACK.parent.glob("*.svg")}
    if root_names != set(expected):
        errors.append(
            f"Root SVG inventory must contain exactly the 149 TAIM premium names: "
            f"unexpected={root_names - set(expected)}; missing={set(expected) - root_names}"
        )
    if discovered != set(expected):
        errors.append(
            f"Manifest/files mismatch: missing={set(expected)-discovered}; extra={discovered-set(expected)}"
        )
    for record in records:
        name = record["name"]
        if not NAME.fullmatch(name) or record["source"] != name + ".svg":
            errors.append(f"Unsafe icon manifest entry: {record}")
            continue
        path = PACK / record["source"]
        if not path.exists():
            continue
        # Root exports must match premium sources byte-for-byte. The
        # generated icon-font script must not clobber these replacements.
        root_export = PACK.parent / (name + ".svg")
        if not root_export.is_file():
            errors.append(f"{name}: missing root assets/icons export")
        elif root_export.read_bytes() != path.read_bytes():
            errors.append(f"{name}: root SVG differs from premium source")
        raw = path.read_text(encoding="utf-8")
        try:
            xml = ET.fromstring(raw)
        except ET.ParseError as exc:
            errors.append(f"{name}: malformed XML: {exc}")
            continue
        if xml.tag != SVG_NS + "svg" or xml.get("viewBox") != "0 0 64 64":
            errors.append(f"{name}: invalid SVG root or grid")
        ids: set[str] = set()
        for node in xml.iter():
            tag = node.tag.split("}")[-1]
            if tag not in SAFE_TAGS:
                errors.append(f"{name}: unsupported/unsafe SVG tag: {tag}")
            node_id = node.get("id")
            if node_id is not None:
                if node_id in ids:
                    errors.append(f"{name}: duplicate SVG id: {node_id}")
                ids.add(node_id)
            for key, value in node.attrib.items():
                local_key = key.split("}")[-1]
                if local_key.startswith("on") or local_key in ("href", "src"):
                    errors.append(f"{name}: embedded event handler / href forbidden")
                elif FORBIDDEN.search(value):
                    errors.append(f"{name}: untrusted external/data reference")
        # External resources inside CSS text can bypass attribute-only checks.
        for style in xml.iter(SVG_NS + "style"):
            css = (style.text or "").lower()
            if "@import" in css or any(
                scheme in css for scheme in ("http:", "https:", "data:", "javascript:")
            ):
                errors.append(f"{name}: external CSS/resource import forbidden")
        if xml.find(SVG_NS + "title") is None:
            errors.append(f"{name}: accessible title missing")
        if xml.find(SVG_NS + "desc") is None:
            errors.append(f"{name}: accessible description missing")
        if not xml.findall(".//" + SVG_NS + "path") and not xml.findall(
            ".//" + SVG_NS + "circle"
        ):
            errors.append(f"{name}: no editable vector geometry")
    return len(records), errors


if __name__ == "__main__":
    count, errs = verify()
    for err in errs:
        print("ERROR:", err, file=sys.stderr)
    print(f"TAIM Premium SVG: {count} entries, {len(errs)} errors")
    raise SystemExit(1 if errs else 0)
