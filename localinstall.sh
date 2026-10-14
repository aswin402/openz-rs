#!/usr/bin/env bash
set -eo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# OpenZ Global Installer — cargo-pretty TUI Edition
# Inspired by https://github.com/romancitodev/cargo-pretty
# ─────────────────────────────────────────────────────────────────────────────

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

mkdir -p ~/.openz/workspace ~/.openz/sessions ~/.openz/skills ~/.openz/traces ~/.openz/web

exec bash "$(dirname "$0")/localupdate.sh" "$@"
