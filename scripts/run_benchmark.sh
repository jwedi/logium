#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
RESULTS_DIR="$ROOT_DIR/benchmark/results"

mkdir -p "$RESULTS_DIR"

TIMESTAMP=$(date +%Y-%m-%d_%H%M%S)
OUTPUT_FILE="$RESULTS_DIR/${TIMESTAMP}.txt"

echo "Running benchmarks..."
echo ""

cd "$ROOT_DIR"
RAW_OUTPUT=$(cargo bench --bench analysis_benchmark 2>&1)

# Parse criterion output into a clean table.
# Criterion prints either:
#   name\n                        time:   [low unit mean unit high unit]
# or:
#   name   time:   [low unit mean unit high unit]
echo "$RAW_OUTPUT" | awk '
BEGIN {
  name = ""
  n = 0
  printf "%-40s %12s %12s %12s\n", "Benchmark", "Low", "Mean", "High"
  printf "%-40s %12s %12s %12s\n", "----------------------------------------", "------------", "------------", "------------"
}
# Standalone name line
/^[a-zA-Z_]/ && !/time:/ && !/Benchmarking/ && !/Finished/ && !/Running/ && !/Gnuplot/ && !/Warning/ && !/Found/ {
  name = $1
  next
}
/time:.*\[/ {
  # If name+time on same line, extract name
  if ($0 ~ /^[a-zA-Z_]/) {
    name = $1
  }
  # Strip everything up to and including "["
  sub(/.*\[/, "")
  # Strip trailing "]"
  sub(/\].*/, "")
  # Now we have: "6.3062 ms 7.0653 ms 8.3996 ms"
  printf "%-40s %8s %-2s %8s %-2s %8s %-2s\n", name, $1, $2, $3, $4, $5, $6
  name = ""
}
' | tee "$OUTPUT_FILE"

echo ""
echo "Results saved to: $OUTPUT_FILE"
