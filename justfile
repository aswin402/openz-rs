# Default target - list all recipes
default:
    @just --list

# Build and install the global binary with balanced resources under cgroup CPU/RAM cap (-j 1 default)
build:
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 bash -c 'if [ -f localupdate.sh ]; then bash localupdate.sh --balanced -j 1; else RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo install --path . --locked --profile release-balanced -j 1; fi'

# Alias for `just build` — update the installed binary under cgroup CPU/RAM cap (-j 1 default)
update:
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 bash -c 'if [ -f localupdate.sh ]; then bash localupdate.sh --balanced -j 1; else RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo install --path . --locked --profile release-balanced -j 1; fi'

# Compile a specific package/crate using cargo-pretty when available (uses 1 job under cgroup cap)
compile package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo build -p {{package}} -j 1

# List all available packages/crates in this workspace
list-packages:
    @echo "Crates in this workspace:"
    @echo " - openz (root binary crate)"

# Ultra-fast (<0.3s, <30MB RAM) standalone check for release version sync + 624-file zero-if/else & zero-regex guard
test-guard:
    mkdir -p target && env CARGO_MANIFEST_DIR="{{justfile_directory()}}" CARGO_PKG_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)" rustc --edition 2021 --test src/version_sync_tests.rs -o target/version_sync_tests_runner && ./target/version_sync_tests_runner --test-threads=1 && rm -f target/version_sync_tests_runner

# Run cargo check on a specific package (defaults to "openz") under cgroup CPU/RAM cap (1.5 cores max, 4.2-5.2GB RAM max)
check package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo check --lib -p {{package}} -j 1

# Run cargo check on all workspace crates with cgroup CPU/RAM cap
check-all:
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo check --workspace -j 1

# Internal helper: keep only the newest openz test binary/rlib in target/debug and strip C .o scratch
[private]
_prune-debug-scratch:
    @bash -c 'if [ -d target/debug ]; then rm -f target/debug/openz target/debug/openz.d target/debug/libopenz.rlib target/debug/libopenz.d target/debug/render_video target/debug/render_video.d target/debug/deps/render_video-*; ls -1dt target/debug/deps/openz-* 2>/dev/null | grep -v "\.d$" | tail -n +2 | xargs -r rm -f; ls -1dt target/debug/deps/libopenz-*.rlib 2>/dev/null | tail -n +2 | xargs -r rm -f; ls -1dt target/debug/deps/libopenz-*.rmeta 2>/dev/null | tail -n +2 | xargs -r rm -f; find target/debug/build -path "*/out/*" -type f \( -name "*.o" -o -name "*.pic.o" \) -delete 2>/dev/null || true; rm -f target/debug/build/tikv-jemalloc-sys-*/out/lib/libjemalloc*.a target/debug/build/tikv-jemalloc-sys-*/out/build/lib/libjemalloc.a 2>/dev/null || true; fi'

# Run cargo test on a specific package (defaults to "openz") under cgroup CPU/RAM cap (--lib only, 1 test thread)
test package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test --lib -p {{package}} -j 1 -- --test-threads=1
    @just _prune-debug-scratch

# Run cargo test on all workspace crates under cgroup CPU/RAM cap
test-all:
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test --workspace --lib -j 1 -- --test-threads=1
    @just _prune-debug-scratch

# Run a specific test by name in a package/crate under cgroup CPU/RAM cap (1.5 cores max, 4.2-5.2GB RAM max)
# Usage: just test-one <test_name> [package_name]
test-one name package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test --lib -p {{package}} {{name}} -j 1 -- --test-threads=1
    @just _prune-debug-scratch

# Run one library unit test only under cgroup CPU/RAM cap
# Usage: just test-lib-one <test_name> [package_name]
test-lib-one name package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test -p {{package}} --lib {{name}} -j 1 -- --test-threads=1
    @just _prune-debug-scratch

# Run cargo clippy on a specific package (defaults to "openz") under cgroup CPU/RAM cap
clippy package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo clippy --lib -p {{package}} -j 1

# Run cargo fmt to format all files in the workspace
format:
    cargo fmt

# Clean all cargo build artifacts (target/ -> 0B)
clean:
    cargo clean

# Remove only target/debug (~4.5G test/dev cache) while keeping target/release-balanced warm for fast localupdate.sh
clean-test:
    rm -rf target/debug target/version_sync_tests_runner

# Reclaim multi-GB stale openz binary/rlib artifacts, C object scratch, and inactive profile caches while preserving active upstream dependency caches
prune-target:
    bash -c 'if [ -f localupdate.sh ]; then bash localupdate.sh --prune-only; else rm -rf target/debug target/release target/release-balanced/openz target/release-balanced/libopenz.rlib target/release-balanced/render_video target/release-balanced/deps/openz-* target/release-balanced/deps/libopenz-* target/release-balanced/deps/render_video-* target/version_sync_tests_runner; fi'
