# Default target - list all recipes
default:
    @just --list

# Build and install the global binary with balanced resources and cargo-pretty TUI under cgroup CPU/RAM cap
build:
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env OPENZ_BUILD_JOBS=1 bash localupdate.sh --balanced

# Alias for `just build` — run the cargo-pretty TUI update manager under cgroup CPU/RAM cap
update:
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env OPENZ_BUILD_JOBS=1 bash localupdate.sh --balanced

# Compile a specific package/crate using cargo-pretty when available (uses 1 job under cgroup cap)
compile package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo build -p {{package}} -j 1

# List all available packages/crates in this workspace
list-packages:
    @echo "Crates in this workspace:"
    @echo " - openz (root binary crate)"

# Ultra-fast (<0.3s, <30MB RAM) standalone check for release version sync + 624-file zero-if/else & zero-regex guard
test-guard:
    env CARGO_MANIFEST_DIR="{{justfile_directory()}}" CARGO_PKG_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)" rustc --edition 2021 --test src/version_sync_tests.rs -o target/version_sync_tests_runner && ./target/version_sync_tests_runner --test-threads=1

# Run cargo check on a specific package (defaults to "openz") under cgroup CPU/RAM cap (1.5 cores max, 4.2-5.2GB RAM max)
check package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo check --lib -p {{package}} -j 1

# Run cargo check on all workspace crates with cgroup CPU/RAM cap
check-all:
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo check --workspace -j 1

# Run cargo test on a specific package (defaults to "openz") under cgroup CPU/RAM cap (--lib only, 1 test thread)
test package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test --lib -p {{package}} -j 1 -- --test-threads=1

# Run cargo test on all workspace crates under cgroup CPU/RAM cap
test-all:
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test --workspace --lib -j 1 -- --test-threads=1

# Run a specific test by name in a package/crate under cgroup CPU/RAM cap (1.5 cores max, 4.2-5.2GB RAM max)
# Usage: just test-one <test_name> [package_name]
test-one name package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test --lib -p {{package}} {{name}} -j 1 -- --test-threads=1

# Run one library unit test only under cgroup CPU/RAM cap
# Usage: just test-lib-one <test_name> [package_name]
test-lib-one name package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test -p {{package}} --lib {{name}} -j 1 -- --test-threads=1

# Run cargo clippy on a specific package (defaults to "openz") under cgroup CPU/RAM cap
clippy package="openz":
    systemd-run --user --scope -q -p CPUQuota=150% -p MemoryHigh=4200M -p MemoryMax=5200M nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo clippy --lib -p {{package}} -j 1

# Run cargo fmt to format all files in the workspace
format:
    cargo fmt

# Clean cargo build artifacts to free disk space
clean:
    cargo clean

# Reclaim multi-GB stale openz binary/rlib artifacts while preserving upstream dependency caches
prune-target:
    rm -rf target/release target/debug/deps/openz-* target/debug/deps/libopenz-* target/release-balanced/deps/openz-* target/release-balanced/deps/libopenz-* target/version_sync_tests_runner
