#!/usr/bin/env bash
# hoi4-rust-accel v1.3 launcher wrapper for Steam or standalone launch.
# Reads settings from hoi4_accel.ini (in this folder) or HOI4_ACCEL_* env vars.
#
# Steam Launch Options (recommended):
#   /path/to/hoi4-accel/launch_hoi4.sh %command%
#
# Optional presets (place before the script path in Steam Launch Options):
#   HOI4_ACCEL_PRESET=smooth    (default: steady 24-30+ FPS + 60 FPS camera + ~+20-25% speed + anti-freeze)
#   HOI4_ACCEL_PRESET=balanced  (Speed-5 @ ~14 FPS + ~+30% speed)
#   HOI4_ACCEL_PRESET=turbo     (max Speed-5 @ ~6 FPS + ~+40% speed)
#   HOI4_ACCEL_PRESET=pure      (zero frame batching, 50+ FPS & MP-safe, 30 FPS anti-freeze)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Prefer release-archive layout ($SCRIPT_DIR/libhoi4_rust_accel.so), fallback to dev build layout
if [ -f "$SCRIPT_DIR/libhoi4_rust_accel.so" ]; then
  ACCEL_SO="$SCRIPT_DIR/libhoi4_rust_accel.so"
else
  ACCEL_SO="$SCRIPT_DIR/target/release/libhoi4_rust_accel.so"
fi
JEMALLOC_SO="/usr/lib/libjemalloc.so.2"

if [ ! -f "$ACCEL_SO" ]; then
  if [ -f "$SCRIPT_DIR/Cargo.toml" ]; then
    echo "[hoi4-rust-accel] Missing $ACCEL_SO — rebuilding release binary..." >&2
    (cd "$SCRIPT_DIR" && RUSTFLAGS="-C target-cpu=native" cargo build --release)
  else
    echo "[hoi4-rust-accel] ERROR: libhoi4_rust_accel.so not found in $SCRIPT_DIR" >&2
    exit 1
  fi
fi

PRELOAD="$ACCEL_SO"
if [ -f "$JEMALLOC_SO" ]; then
  PRELOAD="$JEMALLOC_SO:$ACCEL_SO"
fi
if [ -n "${LD_PRELOAD:-}" ]; then
  PRELOAD="$PRELOAD:$LD_PRELOAD"
fi

export LD_PRELOAD="$PRELOAD"
export HOI4_ACCEL_PRESET="${HOI4_ACCEL_PRESET:-smooth}"

if [ "$#" -gt 0 ]; then
  exec "$@"
else
  HOI4_DIR="/run/media/gleb/Диск/SteamLibrary/steamapps/common/Hearts of Iron IV"
  cd "$HOI4_DIR"
  exec "./hoi4"
fi
