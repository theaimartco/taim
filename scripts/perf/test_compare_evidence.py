"""Only synthetic contract tests. These numbers are not benchmark results."""

import importlib.util
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

source = Path(__file__).with_name("compare-evidence.py")
spec = importlib.util.spec_from_file_location("compare_evidence", source)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def fixture(name, count=1000):
    return {
        "framework": name,
        "hardware": {
            "machine_id": "synthetic", "kernel": "synthetic", "cpu": "synthetic",
            "gpu": "synthetic", "gpu_driver": "synthetic", "compositor": "synthetic",
            "display": "synthetic", "resolution": "1920x1080", "scale": 1,
            "refresh_hz": 60, "power_mode": "AC", "governor": "performance",
        },
        "workload": {
            "scene": "synthetic", "scene_revision": "synthetic",
            "interaction_replay_sha256": "synthetic", "duration_seconds": 60,
        },
        "build": {
            "release": True, "source_sha": "synthetic", "compiler": "synthetic",
        },
        "evidence": {"frame_source": "synthetic", "raw_trace_path": "synthetic.csv"},
        "metrics": {
            "presented_frame_interval_ms": [16.0] * count,
            "input_to_photon_ms": None, "rss_bytes": None,
            "gpu_memory_bytes": None, "idle_power_watts": None,
        },
    }


class EvidenceTests(unittest.TestCase):
    def compare(self, reports):
        with tempfile.TemporaryDirectory() as directory:
            paths = []
            for i, original in enumerate(reports):
                report = json.loads(json.dumps(original))
                path = Path(directory) / f"{i}.json"
                trace = Path(directory) / f"{i}.frames.csv"
                frames = report["metrics"]["presented_frame_interval_ms"]
                data = "".join(f"{n}\n" for n in frames).encode("utf-8")
                trace.write_bytes(data)
                report["evidence"]["raw_trace_path"] = trace.name
                report["evidence"]["raw_trace_sha256"] = hashlib.sha256(data).hexdigest()
                path.write_text(json.dumps(report))
                paths.append(path)
            return module.compare(paths)

    def reports(self):
        return [fixture(name) for name in module.FRAMEWORKS]

    def test_summary_and_missing_not_zero(self):
        out = self.compare(self.reports())
        self.assertEqual(out["frameworks"]["taim"]["frame_ms"]["p99"], 16.0)
        self.assertIsNone(out["frameworks"]["taim"]["gpu_memory_bytes"])
        self.assertIsNone(out["frameworks"]["taim"]["input_to_photon_ms"])

    def test_different_driver_rejected(self):
        reports = self.reports()
        reports[1]["hardware"]["gpu_driver"] = "other"
        with self.assertRaisesRegex(ValueError, "Different hardware"):
            self.compare(reports)

    def test_different_replay_rejected(self):
        reports = self.reports()
        reports[1]["workload"]["interaction_replay_sha256"] = "other"
        with self.assertRaisesRegex(ValueError, "Different scene"):
            self.compare(reports)

    def test_short_samples_rejected(self):
        reports = self.reports()
        reports[0]["metrics"]["presented_frame_interval_ms"] = [10] * 12
        with self.assertRaisesRegex(ValueError, "1000"):
            self.compare(reports)

    def test_latency_requires_sensor(self):
        reports = self.reports()
        reports[0]["metrics"]["input_to_photon_ms"] = [15.2] * 30
        with self.assertRaisesRegex(ValueError, "sensor"):
            self.compare(reports)

    def test_nan_rejected(self):
        reports = self.reports()
        reports[0]["metrics"]["presented_frame_interval_ms"][1] = float("nan")
        with self.assertRaisesRegex(ValueError, "finite"):
            self.compare(reports)

    def test_duplicate_rejected(self):
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            self.compare([fixture("taim")] * 4)

    def test_real_trace_missing_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            report = fixture("taim")
            path = Path(directory) / "taim.json"
            report["evidence"]["raw_trace_sha256"] = "0" * 64
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, "trace missing"):
                module.read_report(path, 1000)

    def test_trace_hash_tampering_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            report = fixture("taim")
            path = Path(directory) / "taim.json"
            trace = Path(directory) / "test.csv"
            trace.write_text("16.0\n" * 1000)
            report["evidence"]["raw_trace_path"] = trace.name
            report["evidence"]["raw_trace_sha256"] = "0" * 64
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, "hash mismatch"):
                module.read_report(path, 1000)

    def test_unproven_memory_rejected(self):
        reports = self.reports()
        reports[0]["metrics"]["gpu_memory_bytes"] = 1024
        with self.assertRaisesRegex(ValueError, "gpu_memory_bytes_source"):
            self.compare(reports)

    def test_nearest_rank(self):
        self.assertEqual(module.nearest_rank(list(range(1, 101)), 95), 95)
        self.assertEqual(module.nearest_rank(list(range(1, 101)), 99), 99)


if __name__ == "__main__":
    unittest.main()
