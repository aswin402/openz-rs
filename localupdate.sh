#!/bin/bash
set -e

echo "🦊 OpenZ Update Manager"
echo "────────────────────────────────"

echo "🔄 Re-compiling and installing new binary globally..."
cargo install --path .

echo "────────────────────────────────"
echo "✅ OpenZ updated successfully!"
if [ -f "$HOME/.cargo/bin/openz" ]; then
    "$HOME/.cargo/bin/openz" --version
fi
