#!/usr/bin/env bash
# TAIM native identity software verification for Linux developers.
# A pass is neither a guarantee of original authorship nor hardware readiness.
set -Eeuo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
EVIDENCE_DIR="$ROOT/artifacts/native-identity-local"
mkdir -p "$EVIDENCE_DIR"

log() { printf '\n[TAIM verify] %s\n' "$*"; }
fail() { printf '[TAIM verify] NO-GO: %s\n' "$*" >&2; exit 2; }

for tool in git python3 cargo rustc rustfmt; do
    command -v "$tool" >/dev/null 2>&1 || fail "Missing required executable: $tool"
done

git rev-parse --verify 'HEAD^{commit}' >/dev/null || fail "No verified HEAD"
git cat-file -e '56b706fc^{commit}' 2>/dev/null ||
    fail "Missing original fork point: fetch complete Git history"
git diff --quiet && git diff --cached --quiet ||
    fail "Tracked source differs from HEAD; commit before verification"
if test -n "$(git ls-files --others --exclude-standard -- '*.rs' '*.wgsl' '*.toml')"; then
    fail "Untracked source prevents trustworthy HEAD-based verification"
fi

git rev-parse HEAD > "$EVIDENCE_DIR/commit.txt"
rustc --version > "$EVIDENCE_DIR/rustc.txt"
cargo --version > "$EVIDENCE_DIR/cargo.txt"

log "Python audit contract tests"
python3 -m unittest discover -s scripts/audit -p 'test_*.py' -v \
    2>&1 | tee "$EVIDENCE_DIR/audit-tool-tests.log"

log "Upstream dependency guard"
python3 scripts/audit/check-upstream-boundary.py \
    2>&1 | tee "$EVIDENCE_DIR/upstream-boundary.log"

log "Collect source lineage evidence (non-strict diagnostic)"
python3 scripts/audit/source-lineage-gate.py \
    --output "$EVIDENCE_DIR/source-lineage.json" \
    2>&1 | tee "$EVIDENCE_DIR/source-lineage.log"

log "Rust formatting"
cargo fmt --all -- --check 2>&1 | tee "$EVIDENCE_DIR/format.log"

log "Workspace compile"
cargo check --workspace --all-targets 2>&1 | tee "$EVIDENCE_DIR/cargo-check.log"

log "Clippy warnings are errors"
cargo clippy --workspace --all-targets -- -D warnings \
    2>&1 | tee "$EVIDENCE_DIR/clippy.log"

log "Workspace Rust tests"
cargo test --workspace 2>&1 | tee "$EVIDENCE_DIR/workspace-tests.log"

log "External Cargo consumer checks (native and legacy)"
bash scripts/release/external-consumer-smoke.sh \
    2>&1 | tee "$EVIDENCE_DIR/external-consumer.log"

log "Fail closed until all inherited source is independently rewritten"
if python3 scripts/audit/source-lineage-gate.py \
    --strict --output "$EVIDENCE_DIR/source-lineage.json" \
    2>&1 | tee "$EVIDENCE_DIR/source-lineage-strict.log"; then
    log "Automated similarity checks cleared; independent manual provenance review still mandatory."
else
    fail "Inherited source remains or source scan failed; 100% original TAIM is NO-GO"
fi

log "Software verification completed; not hardware performance or legal-origin certification."
