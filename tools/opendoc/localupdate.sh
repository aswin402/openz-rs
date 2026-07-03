#!/bin/bash
set -e

echo "=== Updating opendoc-mcp global installation ==="

# Check for cargo
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo is not installed. Please install Rust first."
    exit 1
fi

# Rebuild release binary
echo "Rebuilding release binary..."
cargo +stable build --release --all-features

# Resolve the binary path
BINARY_PATH=""
if [ -f "target/release/opendoc-mcp" ]; then
    BINARY_PATH="target/release/opendoc-mcp"
elif [ -f "../../target/release/opendoc-mcp" ]; then
    BINARY_PATH="../../target/release/opendoc-mcp"
elif [ -f "../target/release/opendoc-mcp" ]; then
    BINARY_PATH="../target/release/opendoc-mcp"
else
    BINARY_PATH=$(find ../../ -name "opendoc-mcp" -path "*/release/opendoc-mcp" -print -quit 2>/dev/null)
fi

if [ -z "$BINARY_PATH" ] || [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Could not locate the built release binary."
    exit 1
fi

# Copy binary to global location
INSTALL_DIR="$HOME/.local/bin"
if [ ! -d "$INSTALL_DIR" ]; then
    echo "Error: Installation directory $INSTALL_DIR does not exist. Please run localinstall.sh first."
    exit 1
fi

echo "Copying updated binary to $INSTALL_DIR/opendoc-mcp from $BINARY_PATH..."
cp "$BINARY_PATH" "$INSTALL_DIR/opendoc-mcp"

echo ""
echo "=== Update complete! ==="
