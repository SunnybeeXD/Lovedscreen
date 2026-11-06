#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="${WORK:-$root/stage/libav-build}"
ffmpeg_prefix="${FFMPEG_PREFIX:-$root/stage/ffmpeg-min}"
plugin_prefix="${PLUGIN_PREFIX:-$root/stage/libav-plugin}"
gst_version="${GST_LIBAV_VERSION:-$(pkg-config --modversion gstreamer-1.0)}"
ffmpeg_url="${FFMPEG_URL:-https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION:?set FFMPEG_VERSION or FFMPEG_URL}.tar.xz}"
gst_url="${GST_LIBAV_URL:-https://gstreamer.freedesktop.org/src/gst-libav/gst-libav-${gst_version}.tar.xz}"
jobs="${JOBS:-$(nproc)}"

rm -rf "$work" "$ffmpeg_prefix" "$plugin_prefix"
mkdir -p "$work"
cd "$work"

curl -fsSL "$ffmpeg_url" -o ffmpeg.tar.xz
curl -fsSL "$gst_url" -o gst-libav.tar.xz
mkdir ffmpeg gst-libav
tar -xf ffmpeg.tar.xz -C ffmpeg --strip-components=1
tar -xf gst-libav.tar.xz -C gst-libav --strip-components=1

cd ffmpeg
./configure \
  --prefix="$ffmpeg_prefix" \
  --enable-shared --disable-static \
  --disable-programs --disable-doc --disable-debug \
  --disable-network --disable-autodetect \
  --disable-everything \
  --disable-avdevice --disable-swscale \
  --enable-avcodec --enable-avformat --enable-avfilter --enable-swresample \
  --enable-decoder=aac,alac,h264,hevc \
  --enable-parser=aac,h264,hevc \
  --enable-filter=buffer,buffersink,yadif
make -j"$jobs"
make install

cd ../gst-libav
PKG_CONFIG_PATH="$ffmpeg_prefix/lib/pkgconfig" PKG_CONFIG_LIBDIR="$ffmpeg_prefix/lib/pkgconfig:${PKG_CONFIG_LIBDIR:-}" \
  meson setup build --prefix="$plugin_prefix" --libdir=lib -Dtests=disabled -Ddoc=disabled
meson compile -C build
meson install -C build

ls -l "$ffmpeg_prefix"/lib/*avcodec* "$plugin_prefix"/lib/gstreamer-1.0/
