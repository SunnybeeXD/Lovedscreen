#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
prefix="${MSYSTEM_PREFIX:-/ucrt64}"
out="$root/stage/runtime"

rm -rf "$out"
mkdir -p "$out/gstreamer-1.0"
cp "$prefix"/lib/gstreamer-1.0/*.dll "$out/gstreamer-1.0/"

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
