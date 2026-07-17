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

# Parse arguments for resource limits and cache cleanup
LOW_RESOURCE=false
BALANCED_RESOURCE=false
CLEAN_TARGET=false
RUN_CHECK=true
for arg in "$@"; do
    case "$arg" in
        --low-resource|--low-mem|-l)
            LOW_RESOURCE=true
            ;;
        --balanced|--moderate|-b)
            BALANCED_RESOURCE=true
            ;;
        --skip-check)
            RUN_CHECK=false
            ;;
        --check)
            RUN_CHECK=true
            ;;
        --clean-target)
            CLEAN_TARGET=true
            ;;
        --help|-h)
            echo "Usage: ./localupdate.sh [--balanced] [--low-resource] [--clean-target] [--skip-check]"
            echo "  --balanced, --moderate, -b     Moderate CPU/RAM mode: faster than low-resource, lighter than full release."
            echo "  --low-resource, --low-mem, -l  Minimum CPU/RAM mode for weak machines."
            echo "  --clean-target                 Run cargo clean before building to reclaim target/ disk space."
            echo "  --skip-check                   Skip pre-install cargo check (install still compiles)."
            echo "  --check                        Force pre-install cargo check."
            exit 0
            ;;
    esac
done

CARGO_FLAGS=""
CARGO_PROFILE_FLAG=""

if [ "$LOW_RESOURCE" = true ] && [ "$BALANCED_RESOURCE" = true ]; then
    echo "❌ Choose only one resource mode: --balanced or --low-resource."
    exit 1
fi

if [ "$LOW_RESOURCE" = true ]; then
    echo "⚡ Low-resource build mode active (minimum CPU/RAM, slower build)..."
    export CARGO_BUILD_JOBS="${OPENZ_BUILD_JOBS:-1}"
    CARGO_FLAGS="-j $CARGO_BUILD_JOBS"
    CARGO_PROFILE_FLAG="--profile release-low-resource"
elif [ "$BALANCED_RESOURCE" = true ]; then
    echo "⚖️ Balanced build mode active (moderate speed with capped CPU/RAM)..."
    export CARGO_BUILD_JOBS="${OPENZ_BUILD_JOBS:-2}"
    CARGO_FLAGS="-j $CARGO_BUILD_JOBS"
    CARGO_PROFILE_FLAG="--profile release-balanced"
    RUN_CHECK=false
else
    echo "💡 Tip: For moderate speed with less RAM/CPU, run: ./localupdate.sh --balanced"
    echo "💡 Tip: For minimum RAM/CPU, run: ./localupdate.sh --low-resource"
    echo "💡 Tip: If disk space is low, run: ./localupdate.sh --clean-target"
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


target_size_kib() {
    if [ ! -d target ]; then
        echo 0
        return 0
    fi
    du -sk target 2>/dev/null | awk '{print $1}'
}

target_size_human() {
    if [ ! -d target ]; then
        echo "0B"
        return 0
    fi
    du -sh target 2>/dev/null | awk '{print $1}'
}

check_target_disk_usage() {
    local threshold_kib=$((20 * 1024 * 1024))
    local size_kib
    size_kib="$(target_size_kib)"
    case "$size_kib" in
        ''|*[!0-9]*) size_kib=0 ;;
    esac

    if [ "$size_kib" -ge "$threshold_kib" ]; then
        echo "⚠️ Cargo build cache target/ is $(target_size_human)."
        echo "   This is rebuildable compiler output. Reclaim it with: $0 --clean-target"
    fi
}

clean_target_if_requested() {
    if [ "$CLEAN_TARGET" != true ]; then
        return 0
    fi
    if [ ! -d target ]; then
        echo "🧹 --clean-target requested, but target/ does not exist."
        return 0
    fi

    echo "🧹 Cleaning Cargo build cache target/ ($(target_size_human))..."
    cargo clean
}


openz_version_line() {
    local bin="$1"
    local output line
    output="$($bin --version 2>/dev/null || true)"
    output="$(printf '%s\n' "$output" | tr -d '\r' | sed 's/\x1b\[[0-9;]*[A-Za-z]//g')"
    line="$(printf '%s\n' "$output" | grep -E 'openz v[0-9]+\.[0-9]+\.[0-9]+' | tail -n 1 || true)"
    if [ -z "$line" ]; then
        line="$(printf '%s\n' "$output" | sed -n '/[^[:space:]]/p' | tail -n 1)"
    fi
    printf '%s' "${line:-unknown}"
}

report_installed_binary() {
    local bin="$HOME/.cargo/bin/openz"
    if [ ! -x "$bin" ]; then
        return 0
    fi

    local version
    version="$(openz_version_line "$bin")"
    local size_human
    size_human="$(du -h "$bin" 2>/dev/null | awk '{print $1}')"
    local size_bytes
    size_bytes="$(stat -c%s "$bin" 2>/dev/null || wc -c < "$bin")"
    local start_ns end_ns smoke_ms
    start_ns="$(date +%s%N)"
    "$bin" --version >/dev/null 2>&1 || true
    end_ns="$(date +%s%N)"
    smoke_ms=$(( (end_ns - start_ns) / 1000000 ))

    echo "📦 Installed binary: $bin"
    echo "   version: ${version:-unknown}"
    echo "   size: ${size_human:-unknown} (${size_bytes:-unknown} bytes)"
    echo "   version-command smoke time: ${smoke_ms}ms"
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

# 3. Clean or warn about Cargo target cache growth
clean_target_if_requested
check_target_disk_usage

# 4. Run pre-install validation when requested. Balanced mode skips this by default
# because cargo install compiles the same graph again.
if [ "$RUN_CHECK" = true ]; then
    echo "🧪 Running compiler checks..."
    cargo check $CARGO_FLAGS
else
    echo "⏭️ Skipping pre-install cargo check (install still compiles the project)."
fi

# 5. Compile and install
echo "🔄 Re-compiling and installing new binary globally..."
if ! cargo install $CARGO_FLAGS $CARGO_PROFILE_FLAG --locked --path .; then
    echo "⚠️ Online install failed (possibly crates.io registry timeout). Retrying in offline mode..."
    cargo install $CARGO_FLAGS $CARGO_PROFILE_FLAG --locked --path . --offline
fi

echo "────────────────────────────────"
echo "✅ OpenZ updated successfully!"
report_installed_binary

echo ""
echo "ℹ️ Database migration from file-based skills under ~/.openz/skills/ to SQLite (~/.openz/memory.db) will occur automatically on startup."
echo "ℹ️ Use the new '/audit' command inside the CLI chat loop to view the cryptographic Merkle Hash-Chain and verify session integrity."
echo "🩺 On startup OpenZ runs a doctor check; if it finds runtime DB files in your project root it archives them under ~/.openz/legacy-root-backup/ (data is preserved)."
