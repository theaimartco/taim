# Android EGL framebuffer precision patch

This is upstream `wgpu-hal` 29.0.4, under its original MIT/Apache-2.0 licenses.
It is third-party source, not Taim-authored framework code. The only functional
change is in `src/gles/egl.rs`: Android requests at least eight bits per RGBA
channel when choosing an EGL configuration, even without sRGB-window support.
Upstream file hashes are recorded in `TAIM-UPSTREAM.json`.

The unpatched chooser can select RGB565 on an Android GLES driver lacking the
sRGB-window extension. Changing the Java SurfaceHolder format alone does not
change that EGL configuration. A process choosing Vulkan and later falling back
to GLES then changes the displayed palette despite identical theme values.

Cargo applies patches only at the consumer workspace root. Taim's workspace and
Android project generator include this patch. Manually assembled Android Cargo
workspaces must add the same `[patch.crates-io]` path entry to this directory.
Do not publish this source under the upstream crate's name or claim upstream
endorses the patch. Remove this vendor directory once an adopted upstream
release includes an equivalent precision guarantee and passes device tests.

Existing source, attribution and licenses are retained. A device without an
eight-bit EGL configuration will fail initialization rather than silently
display a lower-precision palette.
