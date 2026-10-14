#!/usr/bin/env bash
set -eo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# OpenZ Update Manager — cargo-pretty TUI Edition
# Inspired by https://github.com/romancitodev/cargo-pretty
# ─────────────────────────────────────────────────────────────────────────────

# TrueColor Rust Ramp Palette (matching cargo-pretty's rust_ramp)
C_RUST_DEEP="\033[38;2;193;68;14m"     # #C1440E
C_RUST_MID="\033[38;2;232;131;78m"     # #E8834E
C_RUST_LIGHT="\033[38;2;246;201;160m"  # #F6C9A0
C_WHITE="\033[38;2;255;255;255m"       # #FFFFFF
C_TEXT="\033[38;2;230;234;242m"
C_MUTED="\033[38;2;138;146;166m"
C_DIM="\033[38;2;82;90;108m"
C_BORDER="\033[38;2;70;78;96m"
C_OK="\033[38;2;80;210;130m"
C_WARN="\033[38;2;245;185;65m"
C_ERR="\033[38;2;240;90;90m"
C_CYAN="\033[38;2;95;195;235m"
C_BOLD="\033[1m"
C_RESET="\033[0m"

# Fade ramp for recently compiled crates (newest -> oldest)
FADE_0="\033[38;2;246;201;160m"
FADE_1="\033[38;2;195;162;132m"
FADE_2="\033[38;2;145;122;102m"
FADE_3="\033[38;2;102;88;76m"

IS_TTY=false
if [ -t 1 ]; then
    IS_TTY=true
fi

IS_INTERACTIVE_TTY=false
if [ -t 0 ] && [ -t 1 ]; then
    IS_INTERACTIVE_TTY=true
fi

SCRIPT_START_EPOCH="$(date +%s)"

# Default options:
# Balanced mode is the default for localupdate.sh so updates don't stall on ThinLTO
# or compile the 600-crate graph twice (dev check + release install).
LOW_RESOURCE=false
BALANCED_RESOURCE=false
FULL_RELEASE=false
CLEAN_TARGET=false
SKIP_WEBUI_BUILD=false
RUN_CHECK=false
USE_CARGO_PRETTY=true
INSTALL_CARGO_PRETTY=false

for arg in "$@"; do
    case "$arg" in
        --low-resource|--low-mem|-l)
            LOW_RESOURCE=true
            ;;
        --balanced|--moderate|-b)
            BALANCED_RESOURCE=true
            ;;
        --release|--full-release|-r)
            FULL_RELEASE=true
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
        --skip-webui-build)
            SKIP_WEBUI_BUILD=true
            ;;
        --no-pretty|--builtin-tui)
            USE_CARGO_PRETTY=false
            ;;
        --pretty)
            USE_CARGO_PRETTY=true
            ;;
        --install-pretty)
            INSTALL_CARGO_PRETTY=true
            USE_CARGO_PRETTY=true
            ;;
        --help|-h)
            echo -e "${C_BOLD}Usage:${C_RESET} ./localupdate.sh [OPTIONS]"
            echo ""
            echo -e "${C_BOLD}Build Mode Options:${C_RESET}"
            echo "  --balanced, --moderate, -b     Balanced CPU/RAM mode (default: opt-level=2, cgu=4, -j 2)"
            echo "  --low-resource, --low-mem, -l  Minimum CPU/RAM mode for constrained machines (opt-level=1, -j 1)"
            echo "  --release, --full-release, -r  Full ThinLTO production release profile (opt-level=3, lto=thin)"
            echo ""
            echo -e "${C_BOLD}TUI & Pipeline Options:${C_RESET}"
            echo "  --pretty                       Use cargo-pretty live unit-graph TUI when available (default)"
            echo "  --no-pretty, --builtin-tui     Use OpenZ built-in cargo-pretty-style inline TUI renderer"
            echo "  --install-pretty               Auto-install cargo-pretty if missing"
            echo "  --clean-target                 Run cargo clean before building to reclaim target/ disk space"
            echo "  --check                        Run an explicit pre-build cargo check pass"
            echo "  --skip-check                   Skip pre-build cargo check (default)"
            echo "  --skip-webui-build             Skip npm WebUI build and static bundle sync"
            exit 0
            ;;
    esac
done

MODE_COUNT=0
[ "$LOW_RESOURCE" = true ] && MODE_COUNT=$((MODE_COUNT + 1))
[ "$BALANCED_RESOURCE" = true ] && MODE_COUNT=$((MODE_COUNT + 1))
[ "$FULL_RELEASE" = true ] && MODE_COUNT=$((MODE_COUNT + 1))

if [ "$MODE_COUNT" -gt 1 ]; then
    echo -e "${C_ERR}✗ Choose only one build mode: --balanced, --low-resource, or --release.${C_RESET}"
    exit 1
fi

# Default to balanced mode when no explicit mode flag was provided
if [ "$MODE_COUNT" -eq 0 ]; then
    BALANCED_RESOURCE=true
fi

export RUST_FONTCONFIG_DLOPEN="${RUST_FONTCONFIG_DLOPEN:-1}"

CARGO_FLAGS=()
CARGO_PROFILE_ARGS=()
PROFILE_DIR="release-balanced"
MODE_LABEL="Balanced (release-balanced)"

if [ "$LOW_RESOURCE" = true ]; then
    export CARGO_BUILD_JOBS="${OPENZ_BUILD_JOBS:-1}"
    CARGO_FLAGS=(-j "$CARGO_BUILD_JOBS")
    CARGO_PROFILE_ARGS=(--profile release-low-resource)
    PROFILE_DIR="release-low-resource"
    MODE_LABEL="Low-Resource (release-low-resource, -j ${CARGO_BUILD_JOBS})"
elif [ "$BALANCED_RESOURCE" = true ]; then
    export CARGO_BUILD_JOBS="${OPENZ_BUILD_JOBS:-2}"
    CARGO_FLAGS=(-j "$CARGO_BUILD_JOBS")
    CARGO_PROFILE_ARGS=(--profile release-balanced)
    PROFILE_DIR="release-balanced"
    MODE_LABEL="Balanced (release-balanced, -j ${CARGO_BUILD_JOBS})"
else
    if [ -n "${OPENZ_BUILD_JOBS:-}" ]; then
        export CARGO_BUILD_JOBS="$OPENZ_BUILD_JOBS"
        CARGO_FLAGS=(-j "$CARGO_BUILD_JOBS")
    fi
    CARGO_PROFILE_ARGS=(--release)
    PROFILE_DIR="release"
    MODE_LABEL="Full Release (ThinLTO, opt-level=3)"
fi

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

has_cargo_pretty() {
    command -v cargo-pretty >/dev/null 2>&1
}

detect_tui_engine_label() {
    if [ "$USE_CARGO_PRETTY" = true ] && has_cargo_pretty && [ "$IS_INTERACTIVE_TTY" = true ]; then
        echo "cargo-pretty (live unit-graph TUI)"
    else
        echo "openz-pretty (built-in live ANSI TUI)"
    fi
}

render_banner() {
    echo -e "${C_WHITE}     ██████╗ ██████╗ ███████╗███╗   ██╗${C_RUST_DEEP}███████╗${C_RESET}"
    echo -e "${C_WHITE}    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║${C_RUST_DEEP}╚══███╔╝${C_RESET}"
    echo -e "${C_WHITE}    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║${C_RUST_MID}  ███╔╝ ${C_RESET}"
    echo -e "${C_WHITE}    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║${C_RUST_MID} ███╔╝  ${C_RESET}"
    echo -e "${C_WHITE}    ╚██████╔╝██║     ███████╗██║ ╚████║${C_RUST_LIGHT}███████╗${C_RESET}"
    echo -e "${C_WHITE}     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝${C_RUST_LIGHT}╚══════╝${C_RESET}"
    echo -e "${C_BORDER}╭──────────────────────────────────────────────────────────────────────╮${C_RESET}"
    printf "${C_BORDER}│${C_RESET} 🦊 ${C_BOLD}${C_TEXT}%-65s${C_RESET} ${C_BORDER}│${C_RESET}\n" "OpenZ Update Manager"
    printf "${C_BORDER}│${C_RESET}    ${C_MUTED}Profile:${C_RESET}    ${C_RUST_LIGHT}%-53s${C_RESET} ${C_BORDER}│${C_RESET}\n" "$MODE_LABEL"
    printf "${C_BORDER}│${C_RESET}    ${C_MUTED}TUI Engine:${C_RESET} ${C_CYAN}%-53s${C_RESET} ${C_BORDER}│${C_RESET}\n" "$(detect_tui_engine_label)"
    printf "${C_BORDER}│${C_RESET}    ${C_MUTED}Target Dir:${C_RESET} ${C_TEXT}%-53s${C_RESET} ${C_BORDER}│${C_RESET}\n" "target/${PROFILE_DIR} (cache: $(target_size_human))"
    echo -e "${C_BORDER}╰──────────────────────────────────────────────────────────────────────╯${C_RESET}"
    echo ""
}

# Render a 28-cell Rust-ramp progress bar (matching cargo-pretty BAR_WIDTH = 28)
render_rust_progress_bar() {
    local current="$1"
    local total="$2"
    local width=28
    if [ "$total" -le 0 ]; then
        total=1
    fi
    if [ "$current" -gt "$total" ]; then
        current="$total"
    fi
    local filled=$(( (current * width) / total ))
    local empty=$(( width - filled ))
    local bar=""
    local i=0
    while [ "$i" -lt "$filled" ]; do
        if [ "$i" -lt 7 ]; then
            bar="${bar}${C_RUST_DEEP}█"
        elif [ "$i" -lt 15 ]; then
            bar="${bar}${C_RUST_MID}█"
        elif [ "$i" -lt 23 ]; then
            bar="${bar}${C_RUST_LIGHT}█"
        else
            bar="${bar}${C_WHITE}█"
        fi
        i=$((i + 1))
    done
    i=0
    while [ "$i" -lt "$empty" ]; do
        bar="${bar}${C_DIM}░"
        i=$((i + 1))
    done
    printf "%b%b" "$bar" "$C_RESET"
}

format_elapsed() {
    local secs="$1"
    local mins=$((secs / 60))
    local rem=$((secs % 60))
    if [ "$mins" -gt 0 ]; then
        printf "%dm %02ds" "$mins" "$rem"
    else
        printf "%ds" "$rem"
    fi
}

# Run a quick pipeline step with an animated spinner and status badge
run_pipeline_step() {
    local step_id="$1"
    local label="$2"
    shift 2
    local start_ts
    start_ts="$(date +%s)"
    local log_file
    log_file="$(mktemp)"

    if [ "$IS_TTY" = true ]; then
        "$@" >"$log_file" 2>&1 &
        local pid=$!
        local frames=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏')
        local idx=0
        while kill -0 "$pid" 2>/dev/null; do
            local now_ts
            now_ts="$(date +%s)"
            local elapsed=$((now_ts - start_ts))
            printf "\r  ${C_RUST_MID}%s${C_RESET} ${C_DIM}[%s]${C_RESET} ${C_TEXT}%-44s${C_RESET} ${C_MUTED}%6s${C_RESET}" \
                "${frames[$idx]}" "$step_id" "$label" "$(format_elapsed "$elapsed")"
            idx=$(( (idx + 1) % 10 ))
            sleep 0.08
        done
        local rc=0
        wait "$pid" || rc=$?
        local end_ts
        end_ts="$(date +%s)"
        local elapsed=$((end_ts - start_ts))
        local summary_note=""
        if [ -s "$log_file" ]; then
            summary_note="$(tail -n 1 "$log_file" | tr -d '\r' | cut -c1-36)"
        fi
        if [ "$rc" -eq 0 ]; then
            printf "\r\033[2K  ${C_OK}✓${C_RESET} ${C_DIM}[%s]${C_RESET} ${C_TEXT}%-44s${C_RESET} ${C_MUTED}%-18s %5s${C_RESET}\n" \
                "$step_id" "$label" "$summary_note" "$(format_elapsed "$elapsed")"
            rm -f "$log_file"
            return 0
        else
            printf "\r\033[2K  ${C_ERR}✗${C_RESET} ${C_DIM}[%s]${C_RESET} ${C_TEXT}%-44s${C_RESET} ${C_ERR}FAILED (%s)${C_RESET}\n" \
                "$step_id" "$label" "$(format_elapsed "$elapsed")"
            cat "$log_file" >&2
            rm -f "$log_file"
            return "$rc"
        fi
    else
        printf "  • [%s] %s...\n" "$step_id" "$label"
        if "$@" >"$log_file" 2>&1; then
            local end_ts
            end_ts="$(date +%s)"
            printf "  ✓ [%s] %s (%s)\n" "$step_id" "$label" "$(format_elapsed $((end_ts - start_ts)))"
            rm -f "$log_file"
            return 0
        else
            local rc=$?
            cat "$log_file" >&2
            rm -f "$log_file"
            return "$rc"
        fi
    fi
}

backup_openz_data() {
    if [ ! -d "$HOME/.openz" ]; then
        echo "no ~/.openz"
        return 0
    fi
    rm -rf "$HOME/.openz.bak"
    mkdir -p "$HOME/.openz.bak"
    for item in "$HOME"/.openz/*; do
        if [ -e "$item" ]; then
            local name
            name="$(basename "$item")"
            if [ "$name" != "worktrees" ] && [ "$name" != "tool_outputs" ] && [ "$name" != "traces" ] && [ "$name" != "cron_logs" ] && [ "$name" != "legacy-root-backup" ]; then
                cp -r "$item" "$HOME/.openz.bak/" 2>/dev/null || true
            fi
        fi
    done
    echo "~/.openz.bak"
}

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
                found=$((found + 1))
                local dst
                if [ -e "$global_dir/$name" ]; then
                    mkdir -p "$archive_dir"
                    dst="$archive_dir/$name"
                else
                    mkdir -p "$global_dir"
                    dst="$global_dir/$name"
                fi
                mv -f "$f" "$dst" 2>/dev/null || true
                ;;
        esac
    done
    if [ "$found" -gt 0 ]; then
        echo "relocated $found DB(s)"
    else
        echo "workspace clean"
    fi
}

repair_corrupt_cargo_registry_sources() {
    local src_root="$HOME/.cargo/registry/src"
    if [ ! -d "$src_root" ]; then
        echo "registry clean"
        return 0
    fi

    local repaired=0
    while IFS= read -r crate_dir; do
        [ -d "$crate_dir" ] || continue
        if [ ! -f "$crate_dir/Cargo.toml" ]; then
            rm -rf "$crate_dir"
            repaired=$((repaired + 1))
        fi
    done < <(find "$src_root" -mindepth 2 -maxdepth 2 -type d 2>/dev/null)

    if [ "$repaired" -gt 0 ]; then
        echo "repaired $repaired crate(s)"
    else
        echo "registry healthy"
    fi
}

manage_target_cache() {
    if [ "$CLEAN_TARGET" = true ] && [ -d target ]; then
        local before_h
        before_h="$(target_size_human)"
        cargo clean
        echo "cleaned ($before_h freed)"
        return 0
    fi
    local size_h
    size_h="$(target_size_human)"
    echo "target/ is $size_h"
}

# Estimate total units in workspace for fallback progress bar
estimate_unit_total() {
    if [ -f Cargo.lock ]; then
        local count
        count="$(grep -c '^name = ' Cargo.lock 2>/dev/null || echo 450)"
        if [ "$count" -gt 0 ]; then
            echo "$count"
            return 0
        fi
    fi
    echo 450
}

# Built-in cargo-pretty-style live animated status view for Cargo builds
# Displays:
#   - 28-cell Rust gradient progress bar + unit count + percentage + elapsed time
#   - Live "Compiling" section + fading "Compiled" history (up to 4 rows)
#   - Never swallows stderr silently; dumps full diagnostics on failure
run_builtin_cargo_pretty_tui() {
    local cargo_subcmd="$1"
    shift
    local start_ts
    start_ts="$(date +%s)"
    local total_est
    total_est="$(estimate_unit_total)"
    local seen=0
    local current_crate="resolving unit graph..."
    local done_0=""
    local done_1=""
    local done_2=""
    local done_3=""
    local err_log
    err_log="$(mktemp)"
    local drawn_lines=0

    if [ "$IS_TTY" = true ]; then
        # Reserve 7 lines for the inline cargo-pretty status panel
        printf "\n\n\n\n\n\n\n"
        drawn_lines=7
    fi

    draw_inline_panel() {
        local now_ts
        now_ts="$(date +%s)"
        local elapsed=$((now_ts - start_ts))
        local pct=0
        if [ "$total_est" -gt 0 ]; then
            pct=$(( (seen * 100) / total_est ))
            [ "$pct" -gt 99 ] && pct=99
        fi
        local bar
        bar="$(render_rust_progress_bar "$seen" "$total_est")"

        if [ "$IS_TTY" = true ]; then
            printf "\033[%dA\r\033[J" "$drawn_lines"
            printf "  ${C_BOLD}${C_RUST_MID}Compiling${C_RESET} ${C_TEXT}openz${C_RESET} %b ${C_BOLD}${C_WHITE}%3d%%${C_RESET} ${C_MUTED}(%d/%d units · %s)${C_RESET}\n" \
                "$bar" "$pct" "$seen" "$total_est" "$(format_elapsed "$elapsed")"
            printf "  ${C_BORDER}├─${C_RESET} ${C_RUST_MID}▸${C_RESET} ${C_WHITE}%-58s${C_RESET}\n" "${current_crate:0:58}"
            printf "  ${C_BORDER}├─${C_RESET} ${C_OK}✓${C_RESET} ${FADE_0}%-58s${C_RESET}\n" "${done_0:----}"
            printf "  ${C_BORDER}├─${C_RESET} ${C_OK}✓${C_RESET} ${FADE_1}%-58s${C_RESET}\n" "${done_1:----}"
            printf "  ${C_BORDER}├─${C_RESET} ${C_OK}✓${C_RESET} ${FADE_2}%-58s${C_RESET}\n" "${done_2:----}"
            printf "  ${C_BORDER}╰─${C_RESET} ${C_OK}✓${C_RESET} ${FADE_3}%-58s${C_RESET}\n" "${done_3:----}"
            printf "  ${C_DIM}   cargo %s %s${C_RESET}\n" "$cargo_subcmd" "$*"
        fi
    }

    draw_inline_panel

    local fifo
    fifo="$(mktemp -u)"
    mkfifo "$fifo"

    CARGO_TERM_COLOR=never cargo "$cargo_subcmd" "$@" >"$fifo" 2>&1 &
    local cargo_pid=$!

    while IFS= read -r raw_line; do
        echo "$raw_line" >>"$err_log"
        local trimmed
        trimmed="$(printf '%s' "$raw_line" | sed 's/^[[:space:]]*//')"
        case "$trimmed" in
            Compiling\ *|Checking\ *)
                local crate_info="${trimmed#Compiling }"
                crate_info="${crate_info#Checking }"
                crate_info="${crate_info%% (*}"
                if [ -n "$current_crate" ] && [ "$current_crate" != "resolving unit graph..." ]; then
                    done_3="$done_2"
                    done_2="$done_1"
                    done_1="$done_0"
                    done_0="$current_crate"
                fi
                current_crate="$crate_info"
                seen=$((seen + 1))
                if [ "$seen" -ge "$total_est" ]; then
                    total_est=$((seen + 10))
                fi
                if [ "$IS_TTY" = true ]; then
                    draw_inline_panel
                else
                    printf "  [%3d%% | %d/%d] %s\n" "$(( (seen * 100) / total_est ))" "$seen" "$total_est" "$trimmed"
                fi
                ;;
            Fresh\ *)
                seen=$((seen + 1))
                ;;
            Finished\ *)
                if [ -n "$current_crate" ] && [ "$current_crate" != "resolving unit graph..." ]; then
                    done_3="$done_2"
                    done_2="$done_1"
                    done_1="$done_0"
                    done_0="$current_crate"
                fi
                current_crate="linking final binary..."
                seen="$total_est"
                [ "$IS_TTY" = true ] && draw_inline_panel
                ;;
        esac
    done <"$fifo"

    rm -f "$fifo"
    local rc=0
    wait "$cargo_pid" || rc=$?
    local end_ts
    end_ts="$(date +%s)"
    local elapsed=$((end_ts - start_ts))

    if [ "$rc" -eq 0 ]; then
        if [ "$IS_TTY" = true ]; then
            printf "\033[%dA\r\033[J" "$drawn_lines"
            local full_bar
            full_bar="$(render_rust_progress_bar 28 28)"
            printf "  ${C_OK}✓${C_RESET} ${C_DIM}[5/6]${C_RESET} ${C_TEXT}%-24s${C_RESET} %b ${C_OK}100%%${C_RESET} ${C_MUTED}(%s)${C_RESET}\n" \
                "Cargo ${cargo_subcmd} (${PROFILE_DIR})" "$full_bar" "$(format_elapsed "$elapsed")"
        else
            printf "  ✓ [5/6] Cargo %s (%s) completed in %s\n" "$cargo_subcmd" "$PROFILE_DIR" "$(format_elapsed "$elapsed")"
        fi
        rm -f "$err_log"
        return 0
    else
        if [ "$IS_TTY" = true ]; then
            printf "\033[%dA\r\033[J" "$drawn_lines"
        fi
        echo -e "  ${C_ERR}✗ [5/6] Cargo ${cargo_subcmd} failed after $(format_elapsed "$elapsed"):${C_RESET}"
        echo -e "${C_BORDER}────────────────────────────────────────────────────────────────────────${C_RESET}"
        tail -n 60 "$err_log" >&2
        echo -e "${C_BORDER}────────────────────────────────────────────────────────────────────────${C_RESET}"
        rm -f "$err_log"
        return "$rc"
    fi
}

# Compile OpenZ using cargo-pretty when available in an interactive TTY,
# or seamlessly fall back to the built-in cargo-pretty-style live TUI.
compile_and_install_openz() {
    if [ "$INSTALL_CARGO_PRETTY" = true ] && ! has_cargo_pretty; then
        run_pipeline_step "PRE" "Installing cargo-pretty TUI wrapper" \
            bash -c "cargo binstall -y cargo-pretty 2>/dev/null || cargo install cargo-pretty"
    fi

    if [ "$RUN_CHECK" = true ]; then
        echo -e "  ${C_CYAN}ℹ${C_RESET} ${C_MUTED}Running requested pre-build compiler check...${C_RESET}"
        run_builtin_cargo_pretty_tui check --locked "${CARGO_PROFILE_ARGS[@]}" "${CARGO_FLAGS[@]}"
    fi

    local build_start
    build_start="$(date +%s)"
    local built_with_pretty=false

    if [ "$USE_CARGO_PRETTY" = true ] && has_cargo_pretty && [ "$IS_INTERACTIVE_TTY" = true ]; then
        echo -e "  ${C_RUST_MID}▸${C_RESET} ${C_DIM}[5/6]${C_RESET} ${C_TEXT}Building with ${C_BOLD}cargo-pretty${C_RESET}${C_TEXT} live status view (${PROFILE_DIR})...${C_RESET}"
        if cargo pretty build --bin openz --locked "${CARGO_PROFILE_ARGS[@]}" "${CARGO_FLAGS[@]}"; then
            built_with_pretty=true
        else
            echo -e "  ${C_WARN}⚠ cargo-pretty exited early; switching to built-in live TUI renderer...${C_RESET}"
        fi
    fi

    if [ "$built_with_pretty" = false ]; then
        run_builtin_cargo_pretty_tui build --bin openz --locked "${CARGO_PROFILE_ARGS[@]}" "${CARGO_FLAGS[@]}"
    fi

    local built_bin="target/${PROFILE_DIR}/openz"
    if [ ! -f "$built_bin" ]; then
        echo -e "  ${C_ERR}✗ Expected compiled binary at ${built_bin} was not found.${C_RESET}"
        exit 1
    fi

    # Atomic installation into ~/.cargo/bin/openz (avoids ETXTBSY if openz is running)
    mkdir -p "$HOME/.cargo/bin"
    local dst_bin="$HOME/.cargo/bin/openz"
    local tmp_bin="$HOME/.cargo/bin/.openz.tmp.$$"
    install -m 755 "$built_bin" "$tmp_bin"
    mv -f "$tmp_bin" "$dst_bin"

    local build_end
    build_end="$(date +%s)"
    if [ "$built_with_pretty" = true ]; then
        local full_bar
        full_bar="$(render_rust_progress_bar 28 28)"
        printf "  ${C_OK}✓${C_RESET} ${C_DIM}[5/6]${C_RESET} ${C_TEXT}%-24s${C_RESET} %b ${C_OK}100%%${C_RESET} ${C_MUTED}(%s)${C_RESET}\n" \
            "Installed ~/.cargo/bin/openz" "$full_bar" "$(format_elapsed $((build_end - build_start)))"
    fi
}

build_and_install_webui() {
    if [ "$SKIP_WEBUI_BUILD" = true ]; then
        echo "skipped (--skip-webui-build)"
        return 0
    fi
    if [ ! -d "web" ] || [ ! -f "web/package.json" ]; then
        echo "skipped (no web/)"
        return 0
    fi
    if ! command -v npm >/dev/null 2>&1; then
        echo "skipped (no npm)"
        return 0
    fi

    (cd web && npm run build >/dev/null 2>&1)
    rm -rf "$HOME/.openz/web/dist"
    mkdir -p "$HOME/.openz/web/dist"
    cp -a web/dist/. "$HOME/.openz/web/dist/"
    echo "synced ~/.openz/web/dist"
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

render_summary_card() {
    local bin="$HOME/.cargo/bin/openz"
    local total_secs=$(( $(date +%s) - SCRIPT_START_EPOCH ))
    local version="unknown"
    local size_human="unknown"
    local smoke_ms="0"

    if [ -x "$bin" ]; then
        version="$(openz_version_line "$bin")"
        size_human="$(du -h "$bin" 2>/dev/null | awk '{print $1}')"
        local start_ns end_ns
        start_ns="$(date +%s%N)"
        "$bin" --version >/dev/null 2>&1 || true
        end_ns="$(date +%s%N)"
        smoke_ms=$(( (end_ns - start_ns) / 1000000 ))
    fi

    echo ""
    echo -e "${C_BORDER}╭─ ${C_OK}✓ OpenZ Updated Successfully${C_BORDER} ───────────────────────────────────────╮${C_RESET}"
    printf "${C_BORDER}│${C_RESET}  ${C_MUTED}Binary:${C_RESET}      ${C_WHITE}%-54s${C_RESET} ${C_BORDER}│${C_RESET}\n" "$bin"
    printf "${C_BORDER}│${C_RESET}  ${C_MUTED}Version:${C_RESET}     ${C_RUST_LIGHT}%-54s${C_RESET} ${C_BORDER}│${C_RESET}\n" "$version"
    printf "${C_BORDER}│${C_RESET}  ${C_MUTED}Footprint:${C_RESET}   ${C_TEXT}%-54s${C_RESET} ${C_BORDER}│${C_RESET}\n" "${size_human} (${PROFILE_DIR}) · smoke ${smoke_ms}ms"
    printf "${C_BORDER}│${C_RESET}  ${C_MUTED}Total Time:${C_RESET}  ${C_OK}%-54s${C_RESET} ${C_BORDER}│${C_RESET}\n" "$(format_elapsed "$total_secs")"
    echo -e "${C_BORDER}╰──────────────────────────────────────────────────────────────────────╯${C_RESET}"
}

# ─────────────────────────────────────────────────────────────────────────────
# Main Execution Pipeline
# ─────────────────────────────────────────────────────────────────────────────

render_banner

run_pipeline_step "1/6" "Backing up global ~/.openz state" backup_openz_data
run_pipeline_step "2/6" "Checking workspace for stray runtime DBs" cleanup_stray_runtime_dbs
run_pipeline_step "3/6" "Verifying Cargo registry source health" repair_corrupt_cargo_registry_sources
run_pipeline_step "4/6" "Inspecting Cargo target/ build cache" manage_target_cache

compile_and_install_openz

run_pipeline_step "6/6" "Building & syncing WebUI static bundle" build_and_install_webui

render_summary_card
