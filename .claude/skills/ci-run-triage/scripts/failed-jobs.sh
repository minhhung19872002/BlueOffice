#!/bin/bash
# Summarise the failures of a GitHub Actions run of this repository.
# Usage: failed-jobs.sh [run-id]     (default: the latest run on the current branch)
# Prints the job table, then for each failed step the lines that usually name the cause.
set -u
run="${1:-$(gh run list --branch "$(git rev-parse --abbrev-ref HEAD)" --limit 1 --json databaseId -q '.[0].databaseId')}"
echo "== run $run"
gh run view "$run" 2>&1 | sed -n '/^JOBS/,/^ANNOTATIONS/p' | grep -E '^(X|✓|-|\*) ' || true
echo
echo "== failing lines"
gh run view "$run" --log-failed 2>&1 |
  grep -E "FAILED|panicked at|error(\[E[0-9]+\])?:|✘|^\S+\s+\S+\s+\S+\s+[0-9]+\) |is stale|over the|failing|Error:|timed out|exit code" |
  sed -E 's/^([^\t]*)\t([^\t]*)\t[0-9T:.Z-]+ /\1 | \2 | /' |
  cut -c1-260 | head -80
