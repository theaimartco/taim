# Android production hardening: 2026-10-31

**Production clearance: not granted.** The target is framework-wide Android
competition, not a narrow demo. Passing this audit is evidence for the fixes
below; it is not a zero-defect guarantee, physical-device certification or a
claim of Flutter performance superiority. The existing experimental-platform
status remains until the broader release evidence exists.

## Concrete fixes

| Finding | Change | Verification |
| --- | --- | --- |
| An async clipboard completion was scoped only to a window and could satisfy another editor's pending paste | Opaque Root-scoped, single-use paste tokens; capture at the originating response; reject changed focus/editor state/purpose, modals, superseded reads and closed/unfocused windows | Deterministic reordered-completion, supersession, modal, editing and cross-Root tests; real accessibility paste regression |
| Clipboard failures left widgets waiting for a later unsolicited result | Both single-line and multiline widgets terminate pending reads on errors | Failed-read regression for both widget types |
| Separate semantic queues could reorder Copy/Selection/SetText/Focus commands | One bounded FIFO with a shared validated decoder; preserve each response's original paste token | Interleaved decoder tests and actual same-batch Copy-before-Selection test |
| Queued text commands could outlive a target's role or autofill policy | Recheck the live semantic target before execution; retain explicit autofill intent separately from plain text edits | Live-role, hidden/disabled target and secure autofill intent tests |
| Secure composition finish committed an empty string without an active composition, potentially deleting selected password text | Empty finish is a no-op; close cancels composition, calls superclass cleanup and invalidates the connection | Real secure SelectAll + empty finish + close + stale-write rejection, while the fixture's private value remains unchanged |
| Secret widget editor state could be read when preparing async paste | Secure purpose bypasses editor-state reads even for a custom widget; purpose changes invalidate the lease | Deliberately nonconforming custom-editor privacy test; tokens contain no editor text |
| Persisted jobs could reach a permission-dependent API without a local explicit guard | Require RECEIVE_BOOT_COMPLETED only for persisted jobs; transient jobs do not require it | Missing-permission and transient-job tests plus scheduler/headless process regressions |
| API/permission compatibility errors were not part of the release success criterion | Fix/annotate existing version-isolated helpers, catch location revocation, use valid trim/grant constants; successful assembleRelease/bundleRelease now depend on full lintRelease | Full API26-baseline lint: 28 errors before audit, 0 errors afterward; release APK/AAB tasks exercise the gate |
| Release backup/device-transfer policy was only implicit | Explicit cloud, device-transfer and legacy backup exclusions, including device-protected storage | Generated resources compile/package and official bundletool validation passes |
| Unsupported clipboard data could be formatted into a diagnostic | Log only its type/unsupported status, never content | Source audit and strict host build |

The paste ticket is private metadata, not a container for clipboard or password
text. At most one pending destination context is retained per Root. A public
editor snapshot is used to reject stale edits; secure input never reads that
snapshot. Custom shells using Root directly must use begin_paste/accept_paste
around asynchronous reads, as the built-in shell now does.

## Validation and artifacts

The [audit evidence](production-hardening-acceptance-20261031.json) records exact
source/artifact hashes, test reports, release lint warnings and the tested scope.

A fresh generated project was built as optimized, non-debuggable ARM64/x86_64
APK and AAB. APK signatures/alignment and bundle structure are verified. Signing
uses the existing development test certificate, not an owner production key;
no artifact was uploaded, published or deployed.

The release-only smoke harness uses public UI/shell operations, real rendered
counter pixels, touch, suspend/resume and OS background process eviction. It uses
neither run-as nor debug instrumentation in the release app. Android's protected
previous-app slot is explicitly displaced through Settings before am kill; force
stop is not substituted for the saved-task process-death test.

**Open intermittent release regression:** an earlier release run passed five checks and
restored semantic `Count: 2` into a new PID, but failed exact rendered-pixel
equality after process death. The saved screenshots show a changed background
colour and button focus appearance. The cause is not established; the final run
is recorded as failed, even though an earlier artifact passed all seven checks.
This remains a release blocker until rendering/focus/theme restoration is
diagnosed. The same artifact subsequently passed all seven checks after a fresh
emulator boot; the later WebView/sensor artifact also passes exact-pixel checks,
including OS trim-memory. Those passes do not establish why the earlier run
changed colour, so the failed report remains retained and the intermittent
failure is not declared fixed by a reboot.

The [WebView and sensor follow-up](webview-sensors-acceptance-20261031.json)
adds built-in adapters, end-to-end typed sensor delivery and renderer lighting/
hinting restoration after memory-pressure recreation. See [WebView](webview.md)
and [sensor](sensors.md) contracts for their limits. These implementations do not
complete the remaining SDK ecosystem, physical-device acceptance or competitive
performance work.

```sh
python3 scripts/android/access-probe.py --production --serial emulator-5594 \
  --apk target/android/x86_64/debug/platform-probe/taim-platform-probe.apk
python3 scripts/android/release-smoke.py --serial emulator-5594 \
  --apk target/android/production-audit-project/app/build/outputs/apk/release/app-release.apk \
  --package com.theaimartco.taim.productionaudit
```

## Remaining evidence and coverage

| Area | What this pass does not certify |
| --- | --- |
| Devices/API/graphics | Physical ARM64 GPUs, API26 runtime, the wider Android/OEM/page-size matrix and long-run GPU recovery |
| Accessibility/input | Physical TalkBack/Switch Access, actual password-manager providers, OEM keyboards and complex language/gesture behavior |
| Reliability | Extended soak, real storage/network failures, reboot/Doze/OEM background policy and external SDK lifecycles |
| Framework breadth | External GPU texture import/arbitrary composition and the SDK-specific integrations still listed in capabilities.md |
| Competitive performance | Physical-device startup, large lists, animation, memory and energy comparisons; the existing counter harness is insufficient for a dominance claim |
| Distribution | Owner signing identity, app branding, current store/target-SDK acceptance and the application's own user-flow validation |

There are 19 release lint warnings, not a warning-free result:

- ApplySharedPref (3): small permission history writes intentionally commit before
  callback completion; process-restart tests caught lost async writes. Slow/disk
  failure behavior still needs device/fault testing.
- ClickableViewAccessibility (4), ViewConstructor (2): programmatic activity-owned
  routing hosts; actual virtual/native children implement actions. Physical AT
  acceptance remains required.
- UseRequiresApi (6): guarded helper classes use SDK TargetApi annotations without
  adding AndroidX solely for annotations.
- DiscouragedApi (1): explicit application notification icon resource lookup;
  release shrinking is disabled and missing resources fail validation.
- UnusedAttribute (1): a newer manifest attribute is intentionally ignored on the
  API26 baseline and used on supported releases.
- MissingApplicationIcon (1): generated starter branding is not a company's app.
- OldTargetApi (1): this project remains pinned to SDK36; current distribution
  requirements and any target upgrade must be validated separately.

A narrowly scoped MissingPermission annotation exists only around the optional
persisted-job helper, after an explicit OS permission check. The legacy
onBackPressed lint annotation documents that API33/34+ use the separate native
predictive-back callbacks. NewApi checks have not been globally disabled and no
lint baseline was introduced to hide the original errors.
