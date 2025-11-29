#!/bin/bash
# Build script for production release
# This script builds both the sidecar and the main Tauri app

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "Building Tasker release..."

# Detect target triple
case "$(uname -s)-$(uname -m)" in
    Linux-x86_64)   TARGET="x86_64-unknown-linux-gnu" ;;
    Linux-aarch64)  TARGET="aarch64-unknown-linux-gnu" ;;
    Darwin-x86_64)  TARGET="x86_64-apple-darwin" ;;
    Darwin-arm64)   TARGET="aarch64-apple-darwin" ;;
    MINGW*|MSYS*|CYGWIN*)
        TARGET="x86_64-pc-windows-msvc"
        ;;
    *)
        echo "Unsupported platform: $(uname -s)-$(uname -m)"
        exit 1
        ;;
esac

echo "Target: $TARGET"

# Build the sidecar in release mode
echo "Building tasker-sidecar..."
cd "$PROJECT_ROOT/tasker-sidecar"
cargo build --release

# Copy sidecar binary with target triple suffix for Tauri bundling
SIDECAR_SRC="target/release/tasker-sidecar"
if [[ "$TARGET" == *"windows"* ]]; then
    SIDECAR_SRC="${SIDECAR_SRC}.exe"
    SIDECAR_DST="target/release/tasker-sidecar-${TARGET}.exe"
else
    SIDECAR_DST="target/release/tasker-sidecar-${TARGET}"
fi

cp "$SIDECAR_SRC" "$SIDECAR_DST"
echo "Sidecar built: $SIDECAR_DST"

# Update tauri.conf.json to include externalBin for production
cd "$PROJECT_ROOT/src-tauri"
TAURI_CONF="tauri.conf.json"

# Check if externalBin already exists
if grep -q '"externalBin"' "$TAURI_CONF"; then
    echo "externalBin already configured in tauri.conf.json"
else
    # Add externalBin to bundle section
    sed -i 's/"icon": \[/"externalBin": [\n      "..\/tasker-sidecar\/target\/release\/tasker-sidecar"\n    ],\n    "icon": [/' "$TAURI_CONF"
    echo "Added externalBin to tauri.conf.json"
fi

# Build the Tauri app
echo "Building Tauri app..."
cd "$PROJECT_ROOT"
bun run tauri build

echo "Build complete!"
echo "Artifacts are in: $PROJECT_ROOT/src-tauri/target/release/bundle/"
