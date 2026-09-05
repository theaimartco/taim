"""Contract tests use synthetic children; they are not application benchmarks."""

import argparse
import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("measure_app", Path(__file__).with_name("measure-app.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class MeasurementTests(unittest.TestCase):
    def arguments(self, **overrides):
        values = dict(cache_state="uncontrolled", workload="synthetic test", startup_timeout=0.1,
                      warmup=0.01, duration=0.03, sample_interval=0.01, energy_counter=None)
        values.update(overrides)
        return argparse.Namespace(**values)

    def test_real_child_rss_and_marker(self):
        child = "import os,pathlib,time; pathlib.Path(os.environ['TAIM_FIRST_PRESENT_MARKER']).touch(); time.sleep(10)"
        result = module.measure(self.arguments(startup_timeout=2), [sys.executable, "-c", child])
        self.assertEqual(result["status"], "ok", result)
        self.assertGreater(result["steady_rss_bytes"]["minimum"], 0)
        self.assertGreaterEqual(result["steady_rss_bytes"]["samples"], 2)
        self.assertIsNone(result["energy"])
        self.assertIsNone(result["first_pixel_ms"])
        self.assertGreater(result["first_present_observed_upper_bound_ms"], 0)

    def test_missing_marker_fails_and_reaps_child(self):
        with tempfile.TemporaryDirectory() as directory:
            pid_file = Path(directory) / "pid"
            child = f"import os,pathlib,time; pathlib.Path({str(pid_file)!r}).write_text(str(os.getpid())); time.sleep(10)"
            result = module.measure(self.arguments(startup_timeout=0.5), [sys.executable, "-c", child])
            self.assertEqual(result["status"], "failed")
            self.assertIsNone(result["first_present_observed_upper_bound_ms"])
            with self.assertRaises(ProcessLookupError):
                os.kill(int(pid_file.read_text()), 0)

    def test_early_exit_is_not_a_measurement(self):
        result = module.measure(self.arguments(startup_timeout=2), [sys.executable, "-c", "pass"])
        self.assertEqual(result["status"], "failed")
        self.assertIn("exited before present", result["error"])
        self.assertIsNone(result["steady_rss_bytes"])

    def test_energy_wrap_and_invalid_reading(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "energy_uj"
            (path.parent / "max_energy_range_uj").write_text("100")
            path.write_text("90")
            counter = module.EnergyCounter(path)
            path.write_text("10")
            counter.sample()
            path.write_text("30")
            counter.sample()
            self.assertEqual(counter.total, 40)
            path.write_text("100")
            with self.assertRaises(ValueError):
                counter.sample()

    def test_exit_after_marker_does_not_report_steady_state(self):
        child = "import os,pathlib; pathlib.Path(os.environ['TAIM_FIRST_PRESENT_MARKER']).touch()"
        result = module.measure(self.arguments(startup_timeout=2), [sys.executable, "-c", child])
        self.assertEqual(result["status"], "failed")
        self.assertIsNone(result["steady_rss_bytes"])

    def test_requested_missing_energy_counter_fails(self):
        child = "import os,pathlib,time; pathlib.Path(os.environ['TAIM_FIRST_PRESENT_MARKER']).touch(); time.sleep(10)"
        with tempfile.TemporaryDirectory() as directory:
            args = self.arguments(startup_timeout=2, energy_counter=Path(directory) / "missing")
            result = module.measure(args, [sys.executable, "-c", child])
        self.assertEqual(result["status"], "failed")
        self.assertIsNone(result["energy"])

    def test_nonfinite_duration_is_rejected(self):
        for value in ["nan", "inf", "-1", "0"]:
            with self.assertRaises(argparse.ArgumentTypeError):
                module.positive(value)


if __name__ == "__main__":
    unittest.main()
