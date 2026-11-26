#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="${SCRIPT_DIR}/dist"
mkdir -p "${DIST_DIR}"

echo "[1/4] Compiling version.dll (Windows x86_64 PE32+, zero external runtime DLLs)..."
x86_64-w64-mingw32-g++ \
    -O3 -march=x86-64-v3 -mtune=znver3 \
    -std=c++20 \
    -Wall -Wextra -Werror \
    -fno-exceptions -fno-rtti \
    -shared \
    -static -static-libgcc -static-libstdc++ \
    -Wl,--enable-stdcall-fixup,--gc-sections,-s \
    "${SCRIPT_DIR}/version_proxy.cpp" \
    "${SCRIPT_DIR}/version.def" \
    -o "${DIST_DIR}/version.dll" \
    -lkernel32

cp -f "${SCRIPT_DIR}/hoi4_accel.ini" "${DIST_DIR}/hoi4_accel.ini"

echo "[2/4] Checking PE format & imported DLLs (must ONLY be Windows system DLLs)..."
file "${DIST_DIR}/version.dll"
IMPORTS="$(x86_64-w64-mingw32-objdump -p "${DIST_DIR}/version.dll" | grep "DLL Name:" || true)"
echo "${IMPORTS}"
if echo "${IMPORTS}" | grep -Eqi "libgcc|libstdc|winpthread"; then
    echo "ERROR: Unexpected MinGW runtime DLL dependency detected!" >&2
    exit 1
fi

echo "[3/4] Checking exported version.dll symbols..."
x86_64-w64-mingw32-objdump -p "${DIST_DIR}/version.dll" | awk '/\[Ordinal\/Name Pointer\] Table/,/^$/'

echo "[4/4] Build artifacts ready:"
ls -lh "${DIST_DIR}/version.dll" "${DIST_DIR}/hoi4_accel.ini"
sha256sum "${DIST_DIR}/version.dll"
