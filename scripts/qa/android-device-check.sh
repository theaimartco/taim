#!/usr/bin/env bash
# Exercises a TAIM application on a connected Android device or emulator
# and collects evidence: screenshots, logs and a verdict.
#
#   bash scripts/qa/android-device-check.sh EVIDENCE_DIR [ANDROID_PROJECT_DIR]
#
# ANDROID_PROJECT_DIR defaults to examples/mobile_shell/android. Needs adb
# (one device connected, or ANDROID_SERIAL set), gradle (or ./gradlew in the
# project), cargo-ndk and the NDK.
#
# Automated: install, launch, scroll and fling, tap, back, background and
# foreground (the GPU surface is dropped and recreated), rotation, a
# dark-mode switch while running, and back on the first page finishing the
# activity. The run fails if the process dies when it should be alive, or
# logcat shows a panic or a native crash.
#
# Not automated, and listed in the report for a person to fill in: TalkBack,
# typing with the on-screen keyboard, and smoothness as seen by eye.
set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "Usage: bash scripts/qa/android-device-check.sh EVIDENCE_DIR [ANDROID_PROJECT_DIR]" >&2
  exit 64
fi

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
EVIDENCE="$1"
PROJECT="${2:-$ROOT/examples/mobile_shell/android}"

if [ -e "$EVIDENCE" ]; then
  echo "Refusing to overwrite existing evidence: $EVIDENCE" >&2
  exit 2
fi
for tool in adb cargo-ndk; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is required" >&2; exit 2; }
done
if [ ! -f "$PROJECT/app/build.gradle.kts" ]; then
  echo "Not a TAIM Android project: $PROJECT" >&2
  exit 2
fi

DEVICES="$(adb devices | awk 'NR > 1 && $2 == "device"' | wc -l)"
if [ -z "${ANDROID_SERIAL:-}" ] && [ "$DEVICES" != 1 ]; then
  echo "Connect exactly one device or emulator (found $DEVICES), or set ANDROID_SERIAL" >&2
  exit 2
fi

PACKAGE="$(sed -n 's/.*applicationId = "\(.*\)".*/\1/p' "$PROJECT/app/build.gradle.kts" | head -n 1)"
if [ -z "$PACKAGE" ]; then
  echo "No applicationId in $PROJECT/app/build.gradle.kts" >&2
  exit 2
fi

mkdir -p "$EVIDENCE"
EVIDENCE="$(cd "$EVIDENCE" && pwd)"
CHECKS=()
FAILED=0

record() {
  CHECKS+=("$1 | $2")
  printf '%-6s %s\n' "$1" "$2"
  if [ "$1" = FAIL ]; then FAILED=1; fi
}

shot() {
  adb exec-out screencap -p >"$EVIDENCE/$1.png" 2>/dev/null || true
}

alive() {
  adb shell pidof "$PACKAGE" >/dev/null 2>&1
}

expect_alive() {
  if alive; then record PASS "$1"; else record FAIL "$1 (the process is gone)"; fi
}

launch() {
  adb shell monkey -p "$PACKAGE" -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
  sleep "${1:-6}"
}

echo "Building and installing $PACKAGE from $PROJECT"
(
  cd "$PROJECT"
  if [ -x ./gradlew ]; then ./gradlew installDebug; else gradle installDebug; fi
) >"$EVIDENCE/build.log" 2>&1 || {
  echo "The build failed; see $EVIDENCE/build.log" >&2
  exit 1
}

adb logcat -c
adb logcat -b crash -c 2>/dev/null || true
adb shell am force-stop "$PACKAGE"

SIZE="$(adb shell wm size | sed -n 's/.*: \([0-9]*\)x\([0-9]*\).*/\1 \2/p' | tail -n 1)"
WIDTH="${SIZE% *}"
HEIGHT="${SIZE#* }"
CX=$((WIDTH / 2))

# Launch.
launch 8
shot 01-launch
expect_alive "launches and stays running"

# Scroll slowly, then fling.
adb shell input swipe "$CX" $((HEIGHT * 3 / 4)) "$CX" $((HEIGHT / 3)) 400
sleep 1
adb shell input swipe "$CX" $((HEIGHT * 3 / 4)) "$CX" $((HEIGHT / 4)) 60
sleep 2
shot 02-scrolled
expect_alive "scrolls and flings"

# Tap something in the middle, then back out of whatever it opened.
adb shell input tap "$CX" $((HEIGHT / 2))
sleep 1
shot 03-tapped
adb shell input keyevent KEYCODE_BACK
sleep 1
expect_alive "tap, then back"

# Background and foreground: the native window goes away and comes back.
adb shell input keyevent KEYCODE_HOME
sleep 3
launch 4
shot 04-resumed
expect_alive "returns from the background"

# Rotation, with auto-rotate off so the request sticks; restored after.
AUTO_ROTATE="$(adb shell settings get system accelerometer_rotation | tr -d '\r')"
ROTATION="$(adb shell settings get system user_rotation | tr -d '\r')"
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 1
sleep 3
shot 05-landscape
expect_alive "rotates to landscape"
adb shell settings put system user_rotation "${ROTATION:-0}"
adb shell settings put system accelerometer_rotation "${AUTO_ROTATE:-1}"
sleep 2

# Dark mode while running; restored after.
NIGHT="$(adb shell cmd uimode night | sed -n 's/.*: \(.*\)/\1/p' | tr -d '\r')"
adb shell cmd uimode night yes >/dev/null
sleep 3
shot 06-dark
expect_alive "follows a switch to dark mode"
adb shell cmd uimode night "${NIGHT:-no}" >/dev/null
sleep 2

# Back with nothing open finishes the activity.
for _ in 1 2 3; do
  adb shell input keyevent KEYCODE_BACK
  sleep 1
done
sleep 2
if alive; then
  record FAIL "back on the first page finishes the activity (still running)"
else
  record PASS "back on the first page finishes the activity"
fi

# Logs.
adb logcat -d -s taim:V AndroidRuntime:E DEBUG:V libc:F >"$EVIDENCE/logcat.txt" 2>&1 || true
adb logcat -d -b crash >"$EVIDENCE/crash.txt" 2>&1 || true
if grep -q "panic" "$EVIDENCE/logcat.txt"; then
  record FAIL "no panics in the log (see logcat.txt)"
else
  record PASS "no panics in the log"
fi
if grep -q "$PACKAGE" "$EVIDENCE/crash.txt"; then
  record FAIL "no crashes in the crash buffer (see crash.txt)"
else
  record PASS "no crashes in the crash buffer"
fi

prop() { adb shell getprop "$1" | tr -d '\r'; }
GPU="$(adb shell dumpsys SurfaceFlinger 2>/dev/null | grep -m 1 'GLES:' | tr -d '\r' || true)"

{
  echo "# TAIM Android device check"
  echo
  echo "- Commit: $(git -C "$ROOT" rev-parse HEAD)$(git -C "$ROOT" diff --quiet || echo ' (uncommitted changes)')"
  echo "- Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- Application: $PACKAGE ($PROJECT)"
  echo "- Device: $(prop ro.product.manufacturer) $(prop ro.product.model), Android $(prop ro.build.version.release) (API $(prop ro.build.version.sdk)), $(prop ro.product.cpu.abi)"
  echo "- GPU: ${GPU:-unknown}"
  echo "- Screen: ${WIDTH}x${HEIGHT}"
  echo
  echo "## Automated"
  echo
  echo "| Result | Check |"
  echo "|---|---|"
  for check in "${CHECKS[@]}"; do echo "| $check |"; done
  echo
  echo "Screenshots: 01-launch.png to 06-dark.png. Logs: logcat.txt, crash.txt, build.log."
  echo
  echo "## By hand (fill in PASS or FAIL, and who checked)"
  echo
  echo "| Result | Check |"
  echo "|---|---|"
  echo "| | Typing in a text field with the on-screen keyboard, including deleting |"
  echo "| | The keyboard does not cover the focused field |"
  echo "| | TalkBack reads buttons and list items; double-tap activates; two-finger scroll scrolls |"
  echo "| | Scrolling and transitions look smooth (no visible stutter) |"
  echo "| | Copy and paste between the app and another app |"
  echo "| | Text and layout are readable in landscape and with the largest font size |"
} >"$EVIDENCE/report.md"

echo
echo "Report: $EVIDENCE/report.md"
if [ "$FAILED" != 0 ]; then
  echo "DEVICE CHECK FAILED"
  exit 1
fi
echo "Automated device checks passed; complete the by-hand section of the report."
