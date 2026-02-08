#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
FIXTURES_DIR="$ROOT_DIR/crates/logium-core/tests/fixtures"

echo "Setting up test data..."

# Zookeeper logs
echo "Downloading Zookeeper logs..."
curl -sL "https://raw.githubusercontent.com/logpai/loghub/master/Zookeeper/Zookeeper_2k.log" \
  -o "$FIXTURES_DIR/zookeeper/full.log"

# Nginx logs (take first 2000 lines for tests, keep full for benchmark)
echo "Downloading Nginx logs..."
curl -sL "https://raw.githubusercontent.com/elastic/examples/master/Common%20Data%20Formats/nginx_logs/nginx_logs" \
  -o "$FIXTURES_DIR/nginx/full_large.log"
head -n 2000 "$FIXTURES_DIR/nginx/full_large.log" > "$FIXTURES_DIR/nginx/full.log"

# Linux/Syslog logs
echo "Downloading Linux/Syslog logs..."
curl -sL "https://raw.githubusercontent.com/logpai/loghub/master/Linux/Linux_2k.log" \
  -o "$FIXTURES_DIR/syslog/full.log"

# Split each into A (odd lines) and B (even lines)
for dir in zookeeper nginx syslog; do
  echo "Splitting $dir logs..."
  awk 'NR % 2 == 1' "$FIXTURES_DIR/$dir/full.log" > "$FIXTURES_DIR/$dir/source_a.log"
  awk 'NR % 2 == 0' "$FIXTURES_DIR/$dir/full.log" > "$FIXTURES_DIR/$dir/source_b.log"
done

echo "Done! Test data is ready in $FIXTURES_DIR"
echo ""
echo "File counts:"
for dir in zookeeper nginx syslog; do
  echo "  $dir/full.log: $(wc -l < "$FIXTURES_DIR/$dir/full.log") lines"
  echo "  $dir/source_a.log: $(wc -l < "$FIXTURES_DIR/$dir/source_a.log") lines"
  echo "  $dir/source_b.log: $(wc -l < "$FIXTURES_DIR/$dir/source_b.log") lines"
done
if [ -f "$FIXTURES_DIR/nginx/full_large.log" ]; then
  echo "  nginx/full_large.log: $(wc -l < "$FIXTURES_DIR/nginx/full_large.log") lines"
fi
