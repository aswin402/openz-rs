# Default target - list all recipes
default:
    @just --list

# Build and install the global binary with balanced resources and cargo-pretty TUI (2 jobs, release-balanced profile)
build:
    bash localupdate.sh --balanced

# Alias for `just build` — run the cargo-pretty TUI update manager
update:
    bash localupdate.sh --balanced

# Compile a specific package/crate using cargo-pretty when available (uses 2 jobs)
compile package="openz":
    if command -v cargo-pretty >/dev/null 2>&1 && [ -t 0 ] && [ -t 1 ]; then cargo pretty build -p {{package}} -j 2; else cargo build -p {{package}} -j 2; fi

# List all available packages/crates in this workspace
list-packages:
    @echo "Crates in this workspace:"
    @echo " - openz (root binary crate)"

# Run cargo check on a specific package (defaults to "openz") using 1 job and idle CPU/IO priority
check package="openz":
    nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo check --lib -p {{package}} -j 1

# Run cargo check on all workspace crates with capped concurrency (1 job) and idle CPU/IO priority
check-all:
    nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo check --workspace -j 1

# Run cargo test on a specific package (defaults to "openz") using 1 build job, --lib only, and 1 test thread
test package="openz":
    nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test --lib -p {{package}} -j 1 -- --test-threads=1

# Run cargo test on all workspace crates with capped concurrency (1 build job, 1 test thread)
test-all:
    nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test --workspace --lib -j 1 -- --test-threads=1

# Run a specific test by name in a package/crate (uses 1 build job, --lib only, and 1 test thread)
# Usage: just test-one <test_name> [package_name]
test-one name package="openz":
    nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test --lib -p {{package}} {{name}} -j 1 -- --test-threads=1

# Run one library unit test only, avoiding binary/integration test target compilation
# Usage: just test-lib-one <test_name> [package_name]
test-lib-one name package="openz":
    nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo test -p {{package}} --lib {{name}} -j 1 -- --test-threads=1

# Run cargo clippy on a specific package (defaults to "openz") using 1 parallel job and idle CPU/IO priority
clippy package="openz":
    nice -n 19 ionice -c 3 env RUST_FONTCONFIG_DLOPEN=1 CARGO_BUILD_JOBS=1 cargo clippy --lib -p {{package}} -j 1

# Run cargo fmt to format all files in the workspace
format:
    cargo fmt

# Clean cargo build artifacts to free disk space
clean:
    cargo clean
