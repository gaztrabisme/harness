#!/usr/bin/env bash
# Publish the current HEAD tree to the public repo as one squashed commit on
# top of the remote's main. Local history stays private (older commits carry
# machine-specific values). Usage: dogfood/publish.sh [remote] [branch]
set -euo pipefail
remote="${1:-pub}"; branch="${2:-main}"
bash "$(dirname "$0")/publish-check.sh"
git fetch -q "$remote" "$branch"
parent="$(git rev-parse "$remote/$branch")"
msg="snapshot $(git rev-parse --short HEAD): $(git log -1 --format=%s)"
commit="$(git commit-tree "HEAD^{tree}" -p "$parent" -m "$msg")"
git push "$remote" "$commit:refs/heads/$branch"
echo "published $commit to $remote/$branch"
