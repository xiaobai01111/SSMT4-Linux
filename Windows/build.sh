#!/bin/bash
# Build ssmt4-bridge.exe using MinGW-w64 cross-compiler
# Usage: ./build.sh [Release|Debug]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_TYPE="${1:-Release}"
BUILD_DIR="${SCRIPT_DIR}/build"

echo "Building ssmt4-bridge.exe (${BUILD_TYPE})..."
mkdir -p "${BUILD_DIR}"
cd "${BUILD_DIR}"

cmake "${SCRIPT_DIR}" \
    -DCMAKE_TOOLCHAIN_FILE="${SCRIPT_DIR}/toolchain-mingw64.cmake" \
    -DCMAKE_BUILD_TYPE="${BUILD_TYPE}"

make -j"$(nproc)"

echo ""
echo "Build complete: ${BUILD_DIR}/ssmt4-bridge.exe"
ls -lh "${BUILD_DIR}/ssmt4-bridge.exe"
