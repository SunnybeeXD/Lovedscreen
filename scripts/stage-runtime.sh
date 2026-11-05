#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
prefix="${MSYSTEM_PREFIX:-/ucrt64}"
out="$root/stage/runtime"

rm -rf "$out"
mkdir -p "$out/gstreamer-1.0"

required=(coreelements app audioconvert audioresample libav playback videoparsersbad videoconvertscale typefindfunctions d3d11 autodetect)
optional=(directsound wasapi2 videoscale videoconvert)

for name in "${required[@]}"; do
  f="$prefix/lib/gstreamer-1.0/libgst${name}.dll"
  [ -f "$f" ] || { echo "missing required plugin $name" >&2; exit 1; }
  cp "$f" "$out/gstreamer-1.0/"
done
for name in "${optional[@]}"; do
  f="$prefix/lib/gstreamer-1.0/libgst${name}.dll"
  [ -f "$f" ] && cp "$f" "$out/gstreamer-1.0/"
done

seeds=()
for pattern in libgstreamer-1.0 libgstbase-1.0 libgstapp-1.0 libgstvideo-1.0 libgstaudio-1.0 libplist-2.0 libssl libcrypto libwinpthread libgcc_s libstdc++; do
  for f in "$prefix"/bin/${pattern}*.dll; do
    [ -f "$f" ] && seeds+=("$f")
  done
done
[ "${#seeds[@]}" -gt 0 ] || { echo "no runtime seed DLLs found under $prefix/bin" >&2; exit 1; }

for bin in "${seeds[@]}" "$out"/gstreamer-1.0/*.dll; do
  echo "$bin"
  ldd "$bin" | awk '/=> \//{print $3}' | grep -i "^$prefix/" || true
done | grep -i '\.dll$' | grep -vi '/lib/gstreamer-1.0/' | sort -u | while read -r dll; do
  cp -n "$dll" "$out/"
done

ls "$out"/libgstreamer-1.0-*.dll "$out"/libplist-2.0-*.dll >/dev/null
