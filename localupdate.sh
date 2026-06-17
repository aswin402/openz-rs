#!/bin/bash
set -e

white="\033[38;2;240;240;240m"
orange="\033[38;2;255;95;0m"
reset="\033[0m"

echo -e "${white}     ██████╗ ██████╗ ███████╗███╗   ██╗${orange}███████╗"
echo -e "${white}    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║${orange}╚══███╔╝"
echo -e "${white}    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║${orange}  ███╔╝"
echo -e "${white}    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║${orange} ███╔╝"
echo -e "${white}    ╚██████╔╝██║     ███████╗██║ ╚████║${orange}███████╗"
echo -e "${white}     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝${orange}╚══════╝${reset}"
echo "🦊 OpenZ Update Manager"
echo "────────────────────────────────"

# Parse arguments for resource limits
LOW_RESOURCE=false
for arg in "$@"; do
    if [ "$arg" == "--low-resource" ] || [ "$arg" == "--low-mem" ] || [ "$arg" == "-l" ]; then
        LOW_RESOURCE=true
    fi
done

if [ "$LOW_RESOURCE" = true ]; then
    echo "⚡ Low-resource build mode active (restricting CPU cores & RAM consumption)..."
    export CARGO_BUILD_JOBS=1
    export RUSTFLAGS="-C codegen-units=1"
    CARGO_FLAGS="-j 1"
else
    echo "💡 Tip: If compilation consumes too much RAM or CPU, run: ./localupdate.sh --low-resource"
    CARGO_FLAGS=""
fi
echo ""

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
echo "🧪 Running compiler checks..."
cargo check $CARGO_FLAGS

# 3. Compile and install
echo "🔄 Re-compiling and installing new binary globally..."
cargo install $CARGO_FLAGS --locked --path .

echo "────────────────────────────────"
echo "✅ OpenZ updated successfully!"
if [ -f "$HOME/.cargo/bin/openz" ]; then
    "$HOME/.cargo/bin/openz" --version
fi

echo ""
echo "ℹ️ Database migration from file-based skills under ~/.openz/skills/ to SQLite (~/.openz/memory.db) will occur automatically on startup."
echo "ℹ️ Use the new '/audit' command inside the CLI chat loop to view the cryptographic Merkle Hash-Chain and verify session integrity."
