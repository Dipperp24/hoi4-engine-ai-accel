#!/usr/bin/env bash
# hoi4-rust-accel v1.1 launcher wrapper for Steam or standalone launch.
# Reads settings from hoi4_accel.ini (in this folder) or HOI4_ACCEL_* env vars.
#
# Steam Launch Options (recommended):
#   /home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/launch_hoi4.sh %command%
#
# Optional presets (place before the script path in Steam Launch Options):
#   HOI4_ACCEL_PRESET=smooth    (default: steady 24-30+ FPS + 60 FPS camera + ~+20-25% speed + anti-freeze)
#   HOI4_ACCEL_PRESET=balanced  (Speed-5 @ ~14 FPS + ~+30% speed)
#   HOI4_ACCEL_PRESET=turbo     (max Speed-5 @ ~6 FPS + ~+40% speed)
#   HOI4_ACCEL_PRESET=pure      (zero frame batching, 50+ FPS & MP-safe, 30 FPS anti-freeze)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACCEL_SO="$SCRIPT_DIR/target/release/libhoi4_rust_accel.so"
JEMALLOC_SO="/usr/lib/libjemalloc.so.2"

if [ ! -f "$ACCEL_SO" ]; then
  echo "[hoi4-rust-accel] Missing $ACCEL_SO — rebuilding release binary..." >&2
  (cd "$SCRIPT_DIR" && RUSTFLAGS="-C target-cpu=native" cargo build --release)
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
