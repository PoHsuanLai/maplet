#!/bin/bash

# Build script for WASM target
set -e

echo "Building maplet for WASM..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack is not installed. Installing..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Build the library for web target
echo "Building library..."
wasm-pack build \
    --target web \
    --out-dir pkg \
    --features "wasm,render" \
    --no-typescript \
    --no-pack

echo "Copying web assets..."
cp -r www/* pkg/ 2>/dev/null || true

echo "WASM build complete! Output in pkg/"
echo "To serve locally, run: python -m http.server 8000 --directory pkg" 