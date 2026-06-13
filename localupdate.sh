#!/bin/bash
set -e

echo "🦊 OpenZ Update Manager"
echo "────────────────────────────────"

# 1. Back up global openz data if present (skipping heavy cache/log directories)
if [ -d "$HOME/.openz" ]; then
    echo "💾 Backing up existing global OpenZ data (excluding heavy worktrees and logs)..."
    rm -rf "$HOME/.openz.bak"
    mkdir -p "$HOME/.openz.bak"
    for item in "$HOME"/.openz/*; do
        if [ -e "$item" ]; then
            name=$(basename "$item")
            if [ "$name" != "worktrees" ] && [ "$name" != "tool_outputs" ] && [ "$name" != "traces" ] && [ "$name" != "cron_logs" ]; then
                cp -r "$item" "$HOME/.openz.bak/" 2>/dev/null || true
            fi
        fi
    done
    echo "✅ Backup created at ~/.openz.bak"
fi

# 2. Run pre-install validation
echo "🧪 Running compiler checks and validation tests..."
cargo check
cargo test

# 3. Compile and install
echo "🔄 Re-compiling and installing new binary globally..."
cargo install --path .

echo "────────────────────────────────"
echo "✅ OpenZ updated successfully!"
if [ -f "$HOME/.cargo/bin/openz" ]; then
    "$HOME/.cargo/bin/openz" --version
fi

echo ""
echo "ℹ️ Database migration from file-based skills under ~/.openz/skills/ to SQLite (~/.openz/memory.db) will occur automatically on startup."
echo "ℹ️ Use the new '/audit' command inside the CLI chat loop to view the cryptographic Merkle Hash-Chain and verify session integrity."
