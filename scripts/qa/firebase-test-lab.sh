#!/usr/bin/env bash
# Runs a TAIM Android application on physical phones in Firebase Test Lab,
# from a cloud session or any machine without a device, and collects the
# evidence: per-device outcome, screenshots and logcat.
#
#   bash scripts/qa/firebase-test-lab.sh EVIDENCE_DIR [ANDROID_PROJECT_DIR]
#
# Needs:
#   - the Android toolchain (scripts/cloud/setup-android.sh)
#   - FIREBASE_PROJECT: a Google Cloud project with Test Lab enabled
#   - credentials: either `gcloud auth` already done, or the JSON key of a
#     service account allowed to run tests (role "Firebase Test Lab Admin"
#     plus storage access) in GCP_SERVICE_ACCOUNT_KEY. The key is written
#     to a private temporary file for gcloud and removed afterwards.
#   - optionally TEST_LAB_DEVICES: space-separated device specs such as
#     "model=oriole,version=33 model=a52sxq,version=31". Without it, up to
#     three physical models from different manufacturers are chosen, each
#     on its newest Android version.
#
# Test Lab's robo test launches the app and explores it for five minutes.
# The run fails if any device reports a failure or crash, or if a device's
# logcat shows a TAIM panic. It does not replace the by-hand checks in
# scripts/qa/android-device-check.sh (typing, TalkBack, smoothness).
set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "Usage: bash scripts/qa/firebase-test-lab.sh EVIDENCE_DIR [ANDROID_PROJECT_DIR]" >&2
  exit 64
fi

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
EVIDENCE="$1"
PROJECT="${2:-$ROOT/examples/mobile_shell/android}"

if [ -e "$EVIDENCE" ]; then
  echo "Refusing to overwrite existing evidence: $EVIDENCE" >&2
  exit 2
fi
for tool in gcloud gsutil cargo-ndk; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is required" >&2; exit 2; }
done
if [ -z "${FIREBASE_PROJECT:-}" ]; then
  echo "Set FIREBASE_PROJECT to the Google Cloud project that runs the tests" >&2
  exit 2
fi
if [ -f "$HOME/.taim-android-env" ]; then
  # shellcheck disable=SC1091
  . "$HOME/.taim-android-env"
fi
if [ -z "${ANDROID_HOME:-}" ] || [ -z "${ANDROID_NDK_HOME:-}" ]; then
  echo "The Android SDK and NDK are missing: run scripts/cloud/setup-android.sh" >&2
  exit 2
fi

mkdir -p "$EVIDENCE"
EVIDENCE="$(cd "$EVIDENCE" && pwd)"

# Credentials from the environment, never left on disk.
if [ -n "${GCP_SERVICE_ACCOUNT_KEY:-}" ]; then
  KEY="$(mktemp)"
  chmod 600 "$KEY"
  trap 'rm -f "$KEY"' EXIT
  printf '%s' "$GCP_SERVICE_ACCOUNT_KEY" >"$KEY"
  gcloud auth activate-service-account --key-file="$KEY" --quiet >/dev/null
  rm -f "$KEY"
fi
if ! gcloud auth list --filter=status:ACTIVE --format='value(account)' | grep -q .; then
  echo "No active gcloud account: set GCP_SERVICE_ACCOUNT_KEY or run gcloud auth login" >&2
  exit 2
fi

echo "Building the debug APK from $PROJECT"
(
  cd "$PROJECT"
  if [ -x ./gradlew ]; then ./gradlew assembleDebug; else gradle assembleDebug; fi
) >"$EVIDENCE/build.log" 2>&1 || {
  echo "The build failed; see $EVIDENCE/build.log" >&2
  exit 1
}
APK="$(find "$PROJECT/app/build/outputs/apk/debug" -name '*.apk' | head -n 1)"
if [ -z "$APK" ]; then
  echo "No APK was produced; see $EVIDENCE/build.log" >&2
  exit 1
fi
cp "$APK" "$EVIDENCE/app-debug.apk"

# Devices: as given, or three physical models from different makers.
DEVICES=()
if [ -n "${TEST_LAB_DEVICES:-}" ]; then
  read -r -a DEVICES <<<"$TEST_LAB_DEVICES"
else
  while IFS=$'\t' read -r model maker versions; do
    newest="${versions##*;}"
    [ -n "$model" ] && [ -n "$newest" ] || continue
    if ! printf '%s\n' "${MAKERS[@]:-}" | grep -qx "$maker"; then
      MAKERS+=("$maker")
      DEVICES+=("model=$model,version=$newest")
    fi
    [ "${#DEVICES[@]}" -ge 3 ] && break
  done < <(gcloud firebase test android models list --project "$FIREBASE_PROJECT" \
    --filter='form=PHYSICAL' \
    --format='value[separator="\t"](id,manufacturer,supportedVersionIds.join(";"))' 2>/dev/null)
fi
if [ "${#DEVICES[@]}" = 0 ]; then
  echo "No devices: set TEST_LAB_DEVICES, or check Test Lab is enabled for $FIREBASE_PROJECT" >&2
  exit 2
fi

COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
RESULTS_DIR="taim/$COMMIT-$(date -u +%Y%m%dT%H%M%SZ)"
DEVICE_ARGS=()
for device in "${DEVICES[@]}"; do DEVICE_ARGS+=(--device "$device"); done

echo "Running on: ${DEVICES[*]}"
set +e
gcloud firebase test android run \
  --project "$FIREBASE_PROJECT" \
  --type robo \
  --app "$EVIDENCE/app-debug.apk" \
  "${DEVICE_ARGS[@]}" \
  --timeout 5m \
  --results-dir "$RESULTS_DIR" \
  --format=json >"$EVIDENCE/outcomes.json" 2>"$EVIDENCE/test-lab.log"
STATUS=$?
set -e

# Raw results: screenshots, logcat and video for every device.
BUCKET="$(sed -n 's#.*console.developers.google.com/storage/browser/\([^/]*\)/.*#\1#p' "$EVIDENCE/test-lab.log" | head -n 1)"
if [ -n "$BUCKET" ]; then
  gsutil -m -q cp -r "gs://$BUCKET/$RESULTS_DIR" "$EVIDENCE/results" 2>>"$EVIDENCE/test-lab.log" || true
fi

PANICS=0
if [ -d "$EVIDENCE/results" ]; then
  while IFS= read -r log; do
    if grep -q "taim.*panic" "$log"; then
      PANICS=1
      echo "panic in $log" >>"$EVIDENCE/panics.txt"
    fi
  done < <(find "$EVIDENCE/results" -name 'logcat*' -type f)
fi

{
  echo "# TAIM Firebase Test Lab run"
  echo
  echo "- Commit: $(git -C "$ROOT" rev-parse HEAD)$(git -C "$ROOT" diff --quiet || echo ' (uncommitted changes)')"
  echo "- Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- Application: $PROJECT"
  echo "- Project: $FIREBASE_PROJECT, results gs://${BUCKET:-unknown}/$RESULTS_DIR"
  echo "- gcloud exit status: $STATUS (0 means every device passed)"
  echo "- TAIM panics in logcat: $([ "$PANICS" = 0 ] && echo none || echo 'yes, see panics.txt')"
  echo
  echo "| Device | Outcome | Details |"
  echo "|---|---|---|"
  python3 -I - "$EVIDENCE/outcomes.json" <<'PY' || echo "| (outcomes unreadable; see test-lab.log) | | |"
import json, sys
for row in json.load(open(sys.argv[1])):
    print(f"| {row.get('axis_value', '?')} | {row.get('outcome', '?')} | {row.get('test_details', '')} |")
PY
} >"$EVIDENCE/report.md"

echo
echo "Report: $EVIDENCE/report.md"
if [ "$STATUS" != 0 ] || [ "$PANICS" != 0 ]; then
  echo "TEST LAB RUN FAILED"
  exit 1
fi
echo "Every device passed."
