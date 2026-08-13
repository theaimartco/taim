# Competitive Benchmark Rules

For fair framework comparisons (e.g., against Flutter, Qt, Slint), the following methodology must be adhered to:

1. **Same Hardware**: All benchmarks must be run on the exact same physical device, ideally a standard VYOM OS reference device or a controlled environment.
2. **Same Scenes**: Implement the exact same UI layout, element count, and visual appearance in all frameworks. Use equivalent components where possible.
3. **Same Methodology**: 
   - Measure frame times accurately from input to pixels on screen.
   - Run tests for the same duration.
   - Disable background tasks or interference during runs.
4. **Tooling**: Use the provided `compare-frameworks.sh` script to collect and report data consistently.
