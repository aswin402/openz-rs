#!/bin/bash
set -e

echo "🦊 OpenZ Installer: Global Setup"
echo "────────────────────────────────"

# Back up global openz data if present (skipping heavy cache/log directories)
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

# 1. Compile and install globally via Cargo
echo "📦 Compiling and installing openz globally via Cargo..."
cargo install --path .

# 2. Setup folder architecture
echo "📁 Setting up directory structures at ~/.openz..."
mkdir -p ~/.openz/workspace
mkdir -p ~/.openz/sessions
mkdir -p ~/.openz/skills
mkdir -p ~/.openz/traces

# 3. Initialize config if missing by running the version command once
echo "⚙️  Verifying configuration..."
if [ -f "$HOME/.cargo/bin/openz" ]; then
    "$HOME/.cargo/bin/openz" --version
fi

echo "────────────────────────────────"
echo "🎉 OpenZ successfully installed globally!"
echo "💡 You can now run 'openz' from anywhere."
echo "👉 Run 'openz configure' to set up LLM providers."
