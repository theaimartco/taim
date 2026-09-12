#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOK_DIR="${ROOT_DIR}/.git/hooks"
HOOK_FILE="${HOOK_DIR}/post-commit"

mkdir -p "${HOOK_DIR}"
cat << 'HOOK_EOF' > "${HOOK_FILE}"
#!/usr/bin/env bash
# Automatic post-commit background disk cleaner
ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
if [ -f "${ROOT_DIR}/scripts/clean-target.sh" ]; then
    bash "${ROOT_DIR}/scripts/clean-target.sh" > /dev/null 2>&1 &
fi
HOOK_EOF

chmod +x "${HOOK_FILE}"
echo "Git post-commit disk cleaner hook installed successfully."
