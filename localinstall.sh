#!/bin/bash
set -e

echo "🦊 OpenZ Installer: Global Setup"
echo "────────────────────────────────"

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
