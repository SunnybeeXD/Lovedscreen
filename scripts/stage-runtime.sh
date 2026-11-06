#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
prefix="${MSYSTEM_PREFIX:-/ucrt64}"
out="$root/stage/runtime"

rm -rf "$out"
mkdir -p "$out/gstreamer-1.0"

required=(coreelements app audioconvert audioresample playback videoparsersbad videoconvertscale typefindfunctions d3d11 autodetect)
optional=(directsound wasapi2 videoscale videoconvert d3d12 opengl)

for name in "${required[@]}"; do
  f="$prefix/lib/gstreamer-1.0/libgst${name}.dll"
  [ -f "$f" ] || { echo "missing required plugin $name" >&2; exit 1; }
  cp "$f" "$out/gstreamer-1.0/"
done
for name in "${optional[@]}"; do
  f="$prefix/lib/gstreamer-1.0/libgst${name}.dll"
  [ -f "$f" ] && cp "$f" "$out/gstreamer-1.0/"
done

libav_plugin="$root/stage/libav-plugin/lib/gstreamer-1.0/libgstlibav.dll"
[ -f "$libav_plugin" ] || { echo "missing minimal libav plugin, run scripts/build-minimal-libav.sh first" >&2; exit 1; }
cp "$libav_plugin" "$out/gstreamer-1.0/"
cp "$root"/stage/ffmpeg-min/bin/*.dll "$out/"
export PATH="$root/stage/ffmpeg-min/bin:$PATH"

seeds=()
for pattern in libgstreamer-1.0 libgstbase-1.0 libgstapp-1.0 libgstvideo-1.0 libgstaudio-1.0 libwinpthread libgcc_s libstdc++; do
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

ls "$out"/libgstreamer-1.0*.dll
ls "$out"/gstreamer-1.0

for fat in libx265 libaom libSvtAv1Enc librsvg libshaderc libjxl librav1e; do
  if ls "$out"/${fat}*.dll >/dev/null 2>&1; then
    echo "unexpected heavy dependency staged: $fat" >&2
    exit 1
  fi
done

echo "before strip: $(du -sk "$out" | cut -f1) KB"
find "$out" -name '*.dll' -exec strip --strip-unneeded {} + || true
echo "after strip: $(du -sk "$out" | cut -f1) KB"
echo "largest files:"
find "$out" -type f -printf '%s %P\n' | sort -rn | head -25
