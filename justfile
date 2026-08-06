# Default target - list all recipes
default:
    @just --list

# Build and install the global binary with balanced resources (2 jobs, release-balanced profile)
build:
    bash localupdate.sh --balanced

# Compile a specific package/crate (defaults to "openz") without installing globally (uses 2 jobs)
compile package="openz":
    cargo build -p {{package}} -j 2

# List all available packages/crates in this workspace
list-packages:
    @echo "Crates in this workspace:"
    @echo " - openz (root binary crate)"
    @echo " - searchxyz"
    @echo " - openmedia-core"
    @echo " - openmedia-image"
    @echo " - openmedia-video"
    @echo " - openmedia-svg"
    @echo " - openmedia-animate"
    @echo " - openmedia-process"
    @echo " - openmedia-improve"
    @echo " - openmedia-mcp"
    @echo " - opendoc-mcp"
    @echo " - openz-github-mcp"
    @echo " - openz-docs-mcp"

# Run cargo check on a specific package (defaults to "openz") using at most 2 parallel jobs
check package="openz":
    cargo check -p {{package}} -j 2

# Run cargo check on all workspace crates with capped concurrency (2 parallel jobs)
check-all:
    cargo check --workspace -j 2

# Run cargo test on a specific package (defaults to "openz") using at most 2 parallel jobs
test package="openz":
    cargo test -p {{package}} -j 2

# Run cargo test on all workspace crates with capped concurrency (2 parallel jobs)
test-all:
    cargo test --workspace -j 2

# Run a specific test by name in a package/crate (uses 2 jobs)
# Usage: just test-one <test_name> [package_name]
test-one name package="openz":
    cargo test -p {{package}} {{name}} -j 2

# Run cargo clippy on a specific package (defaults to "openz") using at most 2 parallel jobs
clippy package="openz":
    cargo clippy -p {{package}} -j 2

# Run cargo fmt to format all files in the workspace
format:
    cargo fmt

# Clean cargo build artifacts to free disk space
clean:
    cargo clean
