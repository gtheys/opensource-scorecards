#!/usr/bin/env bash
# Loop `scorecards collect` until the category cache is complete, sleeping
# through rate-limit windows between batches. Resumable: cache is the state.
# Usage: ./scripts/collect-loop.sh [category]   (default neovim)
set -euo pipefail
cd "$(dirname "$0")/.."

CATEGORY="${1:-neovim}"
RAW_DIR="data/$CATEGORY/raw"
export GITHUB_TOKEN="${GITHUB_TOKEN:-$(gh auth token)}"

total=$(grep -c '^  - ' "data/$CATEGORY/repos.yaml")
for i in $(seq 1 12); do
  have=$(find "$RAW_DIR" -name '*.json' 2>/dev/null | wc -l)
  echo "=== batch $i: $have/$total cached ==="
  if [ "$have" -ge "$total" ]; then
    echo "complete"
    exit 0
  fi
  ./target/release/scorecards collect --category "$CATEGORY" || true
  # Sleep until the core rate window resets (+60s margin).
  reset=$(gh api rate_limit --jq '.resources.core.reset')
  now=$(date +%s)
  wait=$(( reset - now + 60 ))
  [ "$wait" -gt 0 ] && { echo "sleeping ${wait}s until rate reset…"; sleep "$wait"; }
done
echo "gave up after 12 batches (some repos may be permanently failing — check logs)"
exit 1
