#!/usr/bin/env bash
# run_tests_safely.sh
# Run OpenZ tests in safe sequential batches with restricted CPU/RAM usage to prevent crashes.

set -e

echo "=== Starting OpenZ Safe Test Suite Run ==="
echo "Restricting compilation to 1 parallel job (-j 1)"
echo "Running tests sequentially (--test-threads=1)"
echo "=========================================="

# 1. compile first with restricted cores
cargo test --offline --no-run -j 1

# 2. Run test modules sequentially
echo ""
echo "👉 Batch 1: Self-Management & Configuration Tools"
cargo test --offline -j 1 tools::self_management -- --test-threads=1

echo ""
echo "👉 Batch 2: Headroom (Context Compression) Tools"
cargo test --offline -j 1 tools::headroom -- --test-threads=1

echo ""
echo "👉 Batch 3: Extended & Shared Memory Systems"
cargo test --offline -j 1 tools::memory_extra -- --test-threads=1
cargo test --offline -j 1 tools::shared_memory -- --test-threads=1

echo ""
echo "👉 Batch 4: Sequential Thinking & Reasonings"
cargo test --offline -j 1 tools::sequential_thinking -- --test-threads=1

echo ""
echo "👉 Batch 5: Database & File System Tools"
cargo test --offline -j 1 tools::db_inspector -- --test-threads=1
cargo test --offline -j 1 tools::filesystem -- --test-threads=1

echo ""
echo "👉 Batch 6: SearchXyz & Web Crawlers"
cargo test --offline -j 1 tools::searchxyz -- --test-threads=1
cargo test --offline -j 1 tools::web -- --test-threads=1
cargo test --offline -j 1 tools::web_search -- --test-threads=1

echo ""
echo "👉 Batch 7: All other tools and CLI integrations"
# We can exclude the already tested ones if we want, but since they are fast/cached it is ok. 
# Alternatively, to run remaining tests, we can just run cargo test.
cargo test --offline -j 1 -- --test-threads=1

echo "=========================================="
echo "✅ All safe test batches completed successfully!"
