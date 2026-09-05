# Application measurements

`scripts/perf/measure-app.py` launches the actual Linux application and samples
its `/proc/<pid>/status` RSS after warmup. Python 3.11 or newer is required.
Build the optional first-present probe into the application:

```sh
cargo build -p mobile_shell --release --features taim/measurement
python3 scripts/perf/measure-app.py --output /tmp/mobile-shell.json \
  --warmup 10 --duration 60 --workload 'idle mobile shell, default window' \
  -- target/release/mobile_shell
```

The runner terminates the application and its process group after measurement.
Use a dedicated instance with no unsaved work. Reports identify the executable
by SHA-256, command, platform, workload, and declared cache state. Repeat runs
on the same device and preserve every report, including failures.

The marker is emitted only after the first successful compositor `present()`.
The runner's monotonic observation is an upper bound, including process launch,
file creation and polling (5 ms requested interval, potentially longer under
scheduler load). It is **not first pixel or presentation feedback**. The latter
remains null until measured externally. The probe is absent from default builds.

`--cache-state cold` is a declaration about externally prepared conditions,
not an operation: this script neither drops kernel caches nor reboots the device.
The executable is hashed after measurement and must remain unchanged during
the run. Default reports use `uncontrolled`. A `cold` declaration by itself
does not prove a storage-cold launch; preserve the external preparation protocol.

RSS includes only the launched process, excluding helper processes and GPU
allocations. Warmup is a chosen delay, not automatic proof of equilibrium.
Record application state and inspect repeated runs before calling it steady state.

Optional energy capture accepts one explicit Linux powercap `energy_uj` file:

```sh
python3 scripts/perf/measure-app.py --output /tmp/mobile-shell-energy.json \
  --duration 3600 --workload 'idle mobile shell, fixed panel brightness' \
  --energy-counter /sys/class/powercap/intel-rapl:0/energy_uj \
  -- target/release/mobile_shell
```

The sibling `max_energy_range_uj` provides wraparound accounting. Sample often
enough that the counter wraps fewer than once per interval. Counter resets cannot
be distinguished from wraps. Missing or unreadable requested counters fail the
measurement; omitting the option records null, never zero energy. The selected
domain includes other processes and may exclude the panel/GPU. No baseline is
subtracted. Do not label package energy as whole-device or app-only energy.
Projected Wh/hour equals average watts; it is an extrapolation when the run is
shorter than one hour. Whole-device claims require an external power meter.

The runner does not automate interactions: arrange the stated workload externally.
It does not establish a VYOM hardware proof or a competitor comparison. Follow
`benchmarks/competitors/README.md` for those experiments.

Validate runner contracts with:

```sh
python3 -m unittest discover -s scripts/perf -p 'test_measure_app.py'
cargo check -p taim_winit --features measurement
```
