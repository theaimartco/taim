# TAIM vs Iced vs Slint vs egui — physical benchmark protocol

**Status: no completed same-hardware measurements.** Existing TAIM internal
scene measurements are useful engineering data but do not measure actual
compositor-present frame intervals or physical input-to-photon latency.

## Fair and reproducible experiment

Run all four frameworks on the same physical Linux device with the same
CPU/GPU/governor, power mode, driver, compositor, panel Hz/resolution/scale,
window size, text/font/image data, UI specification and replayed input.
Pin each revision, Rust toolchain and compiler flags. Use release builds.
Define equivalent scenes: idle shell; 10k and 100k virtualized row scroll;
text editor with IME; image atlas; overlays/effects; window resize;
navigation; and accessibility keyboard traversal.

Alternate engine order, use >=5 independent runs, warm up before each run,
and retain >=1000 compositor-present intervals per framework/scene (a
frame-present interval is not a CPU/UI frame cost). Record RAM as process RSS
time series; GPU memory as vendor/driver-reported resident allocations with
process attribution; idle energy as a nonoverlapping energy domain sampled
with background-workload controls. Physical key-to-photon requires an actual
calibrated actuator/sensor, not a Rust timestamp.

Raw JSON per framework should contain:
- framework: taim, iced, slint or egui
- hardware: machine_id, kernel, cpu, gpu, gpu_driver, compositor, display,
  resolution, scale, refresh_hz, power_mode, governor
- workload: scene, scene_revision, interaction_replay_sha256, duration_seconds
- build: release (true), source_sha, compiler
- evidence: frame_source, raw_trace_path, optional latency_sensor
- metrics: presented_frame_interval_ms (1000+ observed numeric samples),
  input_to_photon_ms (null or 30+ calibrated samples), rss_bytes,
  gpu_memory_bytes, idle_power_watts (all optional nullable measurements)

Unavailable values must stay null. Save raw traces and their hashes alongside
each JSON. The evidence manifest is NOT a substitute for the raw source.

## Validate declared comparability and calculate p50/p95/p99

    python3 scripts/perf/compare-evidence.py \
      --output evidence/comparison.json \
      evidence/taim.json evidence/iced.json \
      evidence/slint.json evidence/egui.json

This produces a report only when the declared hardware and scenario match,
release/source/compiler metadata exist, all four engines are present, raw
presentation samples are positive, and optional physical latency has an
identified sensor. It uses nearest-rank percentiles. Validation cannot
independently authenticate raw traces or assess visual quality; reviewers
must examine both. Use --allow-partial only for exploratory 2–3 engine runs,
not public superiority claims.

No benchmark winner should be announced before physical artifacts and
uncertainty across repeated sessions are reviewed.
