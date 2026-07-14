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

repair_corrupt_cargo_registry_sources() {
    local src_root="$HOME/.cargo/registry/src"
    [ -d "$src_root" ] || return 0

    local repaired=0
    while IFS= read -r crate_dir; do
        [ -d "$crate_dir" ] || continue
        # Cargo sometimes leaves a .cargo-ok marker after a failed unpack, but no Cargo.toml.
        # Removing only that unpacked crate dir lets Cargo re-extract it from registry/cache.
        if [ ! -f "$crate_dir/Cargo.toml" ]; then
            rm -rf "$crate_dir"
            repaired=$((repaired + 1))
        fi
    done < <(find "$src_root" -mindepth 2 -maxdepth 2 -type d 2>/dev/null)

    if [ "$repaired" -gt 0 ]; then
        echo "✅ Repaired $repaired corrupt Cargo registry source entr$( [ "$repaired" -eq 1 ] && echo y || echo ies )."
    fi
}



# 1. Back up global openz data if present (skipping heavy cache/log directories)
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

# 1. Relocate any stray runtime DB artifacts out of the working directory
echo "🧹 Checking for stray runtime databases in the working directory..."
cleanup_stray_runtime_dbs

# 2. Repair partial Cargo registry unpacks before compiling
echo "🧰 Checking Cargo registry cache health..."
repair_corrupt_cargo_registry_sources

# 3. Run pre-install validation
echo "🧪 Running compiler checks..."
cargo check $CARGO_FLAGS

# 4. Compile and install
echo "🔄 Re-compiling and installing new binary globally..."
if ! cargo install $CARGO_FLAGS --locked --path .; then
    echo "⚠️ Online install failed (possibly crates.io registry timeout). Retrying in offline mode..."
    cargo install $CARGO_FLAGS --locked --path . --offline
fi

echo "────────────────────────────────"
echo "✅ OpenZ updated successfully!"
if [ -f "$HOME/.cargo/bin/openz" ]; then
    "$HOME/.cargo/bin/openz" --version
fi

echo ""
echo "ℹ️ Database migration from file-based skills under ~/.openz/skills/ to SQLite (~/.openz/memory.db) will occur automatically on startup."
echo "ℹ️ Use the new '/audit' command inside the CLI chat loop to view the cryptographic Merkle Hash-Chain and verify session integrity."
echo "🩺 On startup OpenZ runs a doctor check; if it finds runtime DB files in your project root it archives them under ~/.openz/legacy-root-backup/ (data is preserved)."
