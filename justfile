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

# Run cargo check on a specific package (defaults to "openz") using 1 job
check package="openz":
    cargo check -p {{package}} -j 1

# Run cargo check on all workspace crates with capped concurrency (1 job)
check-all:
    cargo check --workspace -j 1

# Run cargo test on a specific package (defaults to "openz") using 1 job
test package="openz":
    cargo test -p {{package}} -j 1

# Run cargo test on all workspace crates with capped concurrency (1 job)
test-all:
    cargo test --workspace -j 1

# Run a specific test by name in a package/crate (uses 1 job)
# Usage: just test-one <test_name> [package_name]
test-one name package="openz":
    cargo test -p {{package}} {{name}} -j 1

# Run one library unit test only, avoiding binary/integration test target compilation
# Usage: just test-lib-one <test_name> [package_name]
test-lib-one name package="openz":
    cargo test -p {{package}} --lib {{name}} -j 1

# Run cargo clippy on a specific package (defaults to "openz") using at most 2 parallel jobs
clippy package="openz":
    cargo clippy -p {{package}} -j 2

# Run cargo fmt to format all files in the workspace
format:
    cargo fmt

# Clean cargo build artifacts to free disk space
clean:
    cargo clean
