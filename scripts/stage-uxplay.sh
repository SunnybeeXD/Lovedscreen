#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
prefix="${MSYSTEM_PREFIX:-/ucrt64}"
out="$root/stage/uxplay"

cmake -S "$root/vendor/UxPlay" -B "$root/stage/build" -G Ninja -DCMAKE_BUILD_TYPE=Release
ninja -C "$root/stage/build"

rm -rf "$out"
mkdir -p "$out/gstreamer-1.0"
cp "$root/stage/build/uxplay.exe" "$out/"
cp "$prefix"/lib/gstreamer-1.0/*.dll "$out/gstreamer-1.0/"

for bin in "$out/uxplay.exe" "$out"/gstreamer-1.0/*.dll; do
  ldd "$bin" | awk '/=> \//{print $3}' | grep -i "^$prefix/" || true
done | sort -u | while read -r dll; do
  cp -n "$dll" "$out/"
done
