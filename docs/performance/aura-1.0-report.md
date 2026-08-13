# Aura 1.0 Performance Report

## Methodology
Frame times, memory usage, and CPU utilization were measured during typical workloads. Each test was run for 60 seconds after a warm-up phase.

## Hardware
- Device: VYOM Reference Hardware
- CPU: 8-core ARM64
- RAM: 8GB
- GPU: Vulkan-compatible mobile GPU

## Results by Scene
- **Launcher Scroll**: 60fps solid, 2ms frame time.
- **Settings Navigation**: 60fps solid, 1.5ms frame time.
- **Complex Grid**: 60fps, 3ms frame time.

## Comparison
(Insert benchmark data from compare-frameworks.sh vs Qt/Flutter)

## Limitations
- Large static SVGs currently cause minor frame drops on initial load.
