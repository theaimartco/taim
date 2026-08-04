#!/usr/bin/env bash
# Runs every registered aura_bench scene through a release build with
# governor/warm-up checks, and writes one JSON report per scene into
# benchmarks/baseline/. See docs/performance/methodology.md for the full
# reproducibility contract this script exists to enforce.

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

SCENES="${SCENES:-}"
DEVICE="${DEVICE:-}"
WARM_UP_SECS="${WARM_UP_SECS:-30}"
RUNS="${RUNS:-5}"
REFRESH_RATE="${REFRESH_RATE:-60}"
OUTPUT_DIR="${OUTPUT_DIR:-benchmarks/baseline}"

log() {
    echo "[run-baseline] $*" >&2
}

check_governor() {
    local governor_files
    governor_files=(/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor)

    if [[ ! -e "${governor_files[0]}" ]]; then
        log "warning: could not read CPU frequency governor (not Linux, or cpufreq unavailable); proceeding without a check."
        return
    fi

    local governors
    governors=$(cat "${governor_files[@]}" 2>/dev/null | sort -u)

    if [[ "${governors}" != "performance" ]]; then
        log "warning: CPU governor is not pinned to 'performance' (found: ${governors//$'\n'/, })."
        log "warning: results may not be comparable to a prior 'performance'-governor run."
        log "warning: set it with: for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo performance | sudo tee \"\$g\"; done"
    fi
}

require_clean_worktree_or_warn() {
    if [[ -n "$(git status --porcelain)" ]]; then
        log "warning: working tree has uncommitted changes; the report's git_commit field will not fully describe what was measured."
    fi
}

build_release() {
    log "building aura_bench in release mode..."
    cargo build --release --package aura_bench
}

resolve_scenes() {
    if [[ -n "${SCENES}" ]]; then
        echo "${SCENES}"
        return
    fi

    ./target/release/aura_bench --list
}

run_scene() {
    local scene="$1"
    local output_file="${OUTPUT_DIR}/${scene}.json"

    log "running scene '${scene}' (warm-up ${WARM_UP_SECS}s, ${RUNS} measured runs)..."

    local args=(
        --scene "${scene}"
        --refresh-rate "${REFRESH_RATE}"
        --warm-up-secs "${WARM_UP_SECS}"
        --runs "${RUNS}"
        --device "${DEVICE}"
        --save "${output_file}"
    )

    # `--save` refuses to write a report whose git commit, compiler, or
    # device could not be determined, so the human summary on stdout is
    # all that survives a run that cannot be attributed.
    ./target/release/aura_bench "${args[@]}"
}

require_device() {
    if [[ -z "${DEVICE}" ]]; then
        log "error: DEVICE is required."
        log "error: a report in ${OUTPUT_DIR} that does not name the machine it ran on"
        log "error: cannot be compared against anything, so this script will not produce one."
        log "error: set DEVICE to a [[device]] name from benchmarks/devices.toml, e.g."
        log "error:     DEVICE=dev-workstation-01 $0"
        exit 1
    fi

    if ! grep -q "name = \"${DEVICE}\"" benchmarks/devices.toml; then
        log "error: '${DEVICE}' has no [[device]] entry in benchmarks/devices.toml."
        log "error: add one describing the CPU, GPU, OS, compositor, refresh rate, and"
        log "error: power mode before recording reports against that name."
        exit 1
    fi
}

main() {
    require_device
    check_governor
    require_clean_worktree_or_warn
    build_release

    mkdir -p "${OUTPUT_DIR}"

    local scenes
    scenes=$(resolve_scenes)

    while IFS= read -r scene; do
        [[ -z "${scene}" ]] && continue
        run_scene "${scene}"
    done <<< "${scenes}"

    log "done. Compare two runs' P95 with: jq '.median.frame_time.p95' benchmarks/baseline/<scene>.json"
}

main "$@"
