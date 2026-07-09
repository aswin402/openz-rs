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
echo "🦊 OpenZ Installer: Global Setup"
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
    echo "💡 Tip: If compilation consumes too much RAM or CPU, run: ./localinstall.sh --low-resource"
    CARGO_FLAGS=""
fi
echo ""

# Clean up any stray runtime database files left in the working directory
# (e.g. an older build dropping ./memory.db). Mirrors the in-app startup
# doctor check: migrate into ~/.openz when the global DB is missing, otherwise
# archive into ~/.openz/legacy-root-backup/<timestamp>/ so nothing is destroyed.
cleanup_stray_runtime_dbs() {
    local root
    root="$(pwd)"
    local global_dir="$HOME/.openz"
    local stamp
    stamp="$(date +%Y%m%dT%H%M%S)"
    local archive_dir="$global_dir/legacy-root-backup/$stamp"
    local found=0
    for f in "$root"/*; do
        [ -e "$f" ] || continue
        local name
        name="$(basename "$f")"
        case "$name" in
            *.db|*.db-shm|*.db-wal|embeddings_cache.json)
                found=1
                local dst
                if [ -e "$global_dir/$name" ]; then
                    mkdir -p "$archive_dir"
                    dst="$archive_dir/$name"
                else
                    mkdir -p "$global_dir"
                    dst="$global_dir/$name"
                fi
                if mv -f "$f" "$dst" 2>/dev/null; then
                    echo "   ↳ moved stray '$name' -> $dst"
                else
                    echo "   ↳ could not move stray '$name'"
                fi
                ;;
        esac
    done
    if [ "$found" -eq 1 ]; then
        echo "✅ Relocated stray runtime DB artifacts out of the working directory."
    fi
}


# Back up global openz data if present (skipping heavy cache/log directories)
if [ -d "$HOME/.openz" ]; then
    echo "💾 Backing up existing global OpenZ data (excluding heavy worktrees and logs)..."
    rm -rf "$HOME/.openz.bak"
    mkdir -p "$HOME/.openz.bak"
    for item in "$HOME"/.openz/*; do
        if [ -e "$item" ]; then
            name=$(basename "$item")
            if [ "$name" != "worktrees" ] && [ "$name" != "tool_outputs" ] && [ "$name" != "traces" ] && [ "$name" != "cron_logs" ] && [ "$name" != "legacy-root-backup" ]; then
                cp -r "$item" "$HOME/.openz.bak/" 2>/dev/null || true
            fi
        fi
    done
    echo "✅ Backup created at ~/.openz.bak"
fi

# 0. Relocate any stray runtime DB artifacts out of the working directory
echo "🧹 Checking for stray runtime databases in the working directory..."
cleanup_stray_runtime_dbs

# 1. Compile and install globally via Cargo
echo "📦 Compiling and installing openz globally via Cargo..."
cargo install $CARGO_FLAGS --locked --path .

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
echo "🩺 On startup OpenZ runs a doctor check; if it finds runtime DB files in your"
echo "   project root it archives them under ~/.openz/legacy-root-backup/ (data is preserved)."
