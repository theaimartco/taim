"""Regression checks for the actual release screenshot evidence policy."""
import importlib.util
from pathlib import Path
import unittest
from PIL import Image

spec = importlib.util.spec_from_file_location("release_pixels", Path(__file__).resolve().parents[1] / "android/release_pixels.py")
pixels = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pixels)


class ReleasePixelsTests(unittest.TestCase):
    def image(self, background):
        image = Image.new("RGB", (8, 8), background)
        for x in range(7): image.putpixel((x, 0), (x * 10, x * 20, x * 30))
        return image

    def test_accepts_exact_palette_and_records_pixels_and_integrity(self):
        image = self.image((246, 248, 252))
        data, evidence = pixels.capture_signature(image, (0, 0, 8, 8), (246, 248, 252))
        self.assertEqual(data, image.tobytes())
        self.assertEqual(evidence["background_rgb"], [246, 248, 252])
        self.assertEqual(len(evidence["crop_sha256"]), 64)

    def test_stable_wrong_palette_is_not_mistaken_for_successful_restoration(self):
        with self.assertRaisesRegex(ValueError, "background mismatch"):
            pixels.capture_signature(self.image((255, 251, 255)), (0, 0, 8, 8), (246, 248, 252))

    def test_blank_or_offscreen_capture_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "blank"):
            pixels.capture_signature(Image.new("RGB", (8, 8)), (0, 0, 8, 8))
        for bounds in [(-1, 0, 8, 8), (0, 0, 9, 8), (1, 1, 1, 8)]:
            with self.subTest(bounds=bounds), self.assertRaises(ValueError):
                pixels.capture_signature(self.image((246, 248, 252)), bounds)

    def test_only_a_proven_backend_transition_can_use_explicit_one_level_rounding(self):
        before=bytes([20,30,40]);after=bytes([21,30,40])
        result=pixels.compare_restoration(before,after,"Vulkan","Gl",True)
        self.assertEqual(result["max_channel_delta"],1)
        for old,new,allow in [("Gl","Gl",True),(None,"Gl",True),("Vulkan","Gl",False)]:
            with self.subTest(old=old,new=new,allow=allow),self.assertRaises(ValueError):
                pixels.compare_restoration(before,after,old,new,allow)
        with self.assertRaises(ValueError):
            pixels.compare_restoration(before,bytes([22,30,40]),"Vulkan","Gl",True)
        with self.assertRaises(ValueError):
            pixels.compare_restoration(before,after+b'\0',"Vulkan","Gl",True)


if __name__ == "__main__": unittest.main()
