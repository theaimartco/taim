"""Exact release-fixture pixel evidence, independent of Android transport."""
import hashlib


def capture_signature(image, bounds, expected_background=None):
    left, top, right, bottom = bounds
    if not (0 <= left < right <= image.width and 0 <= top < bottom <= image.height):
        raise ValueError("Fixture bounds fall outside the captured screen")
    crop = image.convert("RGB").crop(bounds)
    colors = crop.getcolors(maxcolors=crop.width * crop.height) or []
    if len(colors) < 8:
        raise ValueError("Release counter pixels are blank")
    dominant = max(colors)[1]
    if expected_background is not None and dominant != expected_background:
        raise ValueError(f"Fixture background mismatch: expected {expected_background}, got {dominant}; check surface precision/backend")
    pixels = crop.tobytes()
    return pixels, {"bounds": list(bounds), "background_rgb": list(dominant),
                    "crop_sha256": hashlib.sha256(pixels).hexdigest()}


def compare_restoration(before, after, initial_backend=None, restored_backend=None, allow_backend_rounding=False):
    if not before or len(before) != len(after):
        raise ValueError("Restored counter pixel dimensions changed")
    transition = initial_backend in {"Vulkan", "Gl"} and restored_backend in {"Vulkan", "Gl"} and initial_backend != restored_backend
    limit = 1 if allow_backend_rounding and transition else 0
    delta = max(abs(a - b) for a, b in zip(before, after))
    if delta > limit:
        raise ValueError(f"Restored pixel delta {delta} exceeds {limit}; backends {initial_backend} -> {restored_backend}")
    return {"initial_backend": initial_backend, "restored_backend": restored_backend,
            "max_channel_delta": delta, "allowed_channel_delta": limit,
            "changed_channels": sum(a != b for a, b in zip(before, after)),
            "comparison": "exact" if delta == 0 else "verified_backend_rounding_1lsb"}
