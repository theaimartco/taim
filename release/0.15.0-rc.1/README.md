# Evidence for 0.15.0-rc.1

Collected from commit `7f9b26685654e097460525011fda9138e649f387`, a clean
checkout, Rust 1.95.0, Linux x86-64.

| File | What it is |
|---|---|
| `release-check.md` | `scripts/release/check.sh` on that commit: every step passed except the Android link, skipped because no NDK was installed. GPU tests ran on Mesa's software Vulkan driver. |
| `taim-0.15.0-rc.1.cdx.json` | CycloneDX SBOM of the `taim` crate and its dependency tree (cargo-cyclonedx). |
| `SHA256SUMS` | Checksums of the release binaries built from that commit (`liquid`, `mobile_shell`, `taim-new`, `taim-showcase`, Linux x86-64) and of the SBOM. The binaries themselves are not stored in the repository. |
| `provenance.json` | `scripts/release/provenance.py` inventory. It records the source commit, toolchain and lockfile hash, and states that it is not an approval: no independent rebuild, signature or hardware acceptance has been verified. |

Still to collect before 0.15.0: an Android device check report from three
physical devices, a release check with no skipped step, and the owner's
sign-off (`docs/release/production-gates.md`).
