# Taim

A Rust GUI framework built around persistent views and reactive state.

**Experimental, 0.15.0-dev.** Production acceptance is incomplete. See
[engineering status](docs/status.md), [release gates](docs/release/production-gates.md)
and [device acceptance](docs/testing/wayland-vyom-device-acceptance.md).

A Taim application builds its view tree once. Views read signal handles;
callbacks change state, and the runtime tracks which views need work.
The public application API uses `launch`, `Scope`, views and callbacks.

```rust
use taim::prelude::*;

fn main() -> taim::Result {
    taim::launch("Counter", |cx| {
        let count = cx.signal(0_i64);

        column(views![
            text(move || format!("Count: {}", count.get())).size(24),
            button("+1").on_press(move || count.update(|n| *n += 1)),
        ])
        .gap(12.0)
        .padding(24.0)
    })
}
```

From this checkout:

```bash
cargo run -p counter
cargo run -p trivial
```

For an application outside the repository, use a local checkout dependency
until a packaged release has passed the publication gates:

```toml
[dependencies]
taim = { path = "/path/to/taim" }

# Required for the Android GLES precision fix until it lands upstream.
# Cargo does not inherit patches from a dependency's workspace.
[patch.crates-io]
wgpu-hal = { path = "/path/to/taim/vendor/wgpu-hal" }
```

The workspace includes the native view tree (`view/`), reactive signals
(`taim-signal/`), a window shell (`winit/`), GPU and software renderer paths
(`wgpu/`, `tiny_skia/`), and text, gesture, accessibility and design modules.
The presence of a module does not certify its platform support. The full
workspace, including IDE and inspector tools, builds against the current
facade. Taim Lab uses native views; Virgo preserves its editor and terminal
models through an IDE-local retained-widget adapter on the native shell.
The removed `Program` application host has not been restored.

Taim began from Iced. Historical origin and licensed source are recorded in
[CREDITS.md](CREDITS.md) and [LICENSE](LICENSE). Taim's current view contract
and native implementations establish its engineering direction; source
similarity scores alone do not establish independent authorship. See the
[architecture acceptance contract](docs/architecture/native-identity.md).

Local verification:

```bash
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
bash scripts/release/external-consumer-smoke.sh
python3 scripts/qa/verify-taim.py
```

The manual verifier collects checks and reports missing evidence. Passing
software tests does not replace physical Wayland/VYOM, IME, screen-reader,
GPU recovery or performance measurements. Frame latency and refresh-rate
claims require [same-device evidence](docs/performance/four-framework-comparison.md).

MIT; see [LICENSE](LICENSE) and [CREDITS.md](CREDITS.md).

## Sponsor & Support

If you find Taim valuable or are using it in production, consider sponsoring the project to support continuous GPU compute-raster optimizations, native platform backends, and ecosystem development:

[![Sponsor Taim](https://img.shields.io/badge/Sponsor-Razorpay-0c2340?style=for-the-badge&logo=razorpay&logoColor=white)](https://rzp.io/rzp/theaimart-taimrs)

👉 **Support directly via Razorpay**: [https://rzp.io/rzp/theaimart-taimrs](https://rzp.io/rzp/theaimart-taimrs)


## Android development preview

An experimental NativeActivity host and ARM64 counter APK build workflow are
available. The tested API 35 x86_64/4 KiB emulator passes touch/repaint and
surface-resume smoke tests. The tested 16 KiB guest now passes via GLES compatibility;
native input, insets and system fonts are implemented. Accessibility/autofill,
back handling, opt-in saved-task restoration, native services/plugins and an
API 36 release APK/AAB project generator are also implemented. See
[Android setup](docs/android/README.md), [platform evidence](docs/android/platform-20261030.md)
and the [implementation ledger](docs/android/plan.md). Physical-device support
and Flutter performance superiority are not established.
