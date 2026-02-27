#!/usr/bin/env bash
# scripts/mac/build-sidecars.sh
# macOS 用の Swift sidecar (floating-overlay, speech-recognizer) をビルドします。

set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bin_dir="$root_dir/src-tauri/bin"
mkdir -p "$bin_dir"

build_sidecar() {
  local name="$1"
  local sources=("${@:2}")
  local plist="$bin_dir/${name}.plist"
  local app_bundle_name="${name}.app"
  if [[ "$name" == "speech-recognizer" ]]; then
    app_bundle_name="KoeType Speech Recognizer.app"
  fi
  local app_dir="$bin_dir/${app_bundle_name}/Contents"
  local app_exec="$app_dir/MacOS/${name}"
  local app_plist="$app_dir/Info.plist"
  local app_resources="$app_dir/Resources"
  local pkginfo="$app_dir/PkgInfo"
  local tmp_plist="$(mktemp)"

  echo "Building sidecar: $name..."

  if [[ ! -f "$plist" ]]; then
    echo "Error: $plist not found" >&2
    exit 1
  fi

  plutil -convert binary1 -o "$tmp_plist" "$plist"

  mkdir -p "$app_dir/MacOS" "$app_resources"
  cp "$plist" "$app_plist"
  printf "APPL????" > "$pkginfo"

  if [[ "$name" == "speech-recognizer" ]]; then
    cp "$root_dir/src-tauri/icons/icon.icns" "$app_resources/icon.icns"
  fi

  local swiftc_flags=()
  if [[ "$name" == "floating-overlay" ]]; then
    swiftc_flags=(
      -framework Cocoa -framework SwiftUI -framework Foundation
      -DNO_IDLE_OFFSET
      -DINSTANT_HIDE_AFTER_PASTE
    )
  elif [[ "$name" == "speech-recognizer" ]]; then
    swiftc_flags=(
      -framework Speech -framework AVFoundation -framework Foundation
    )
  fi

  xcrun swiftc \
    "${swiftc_flags[@]}" \
    -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist -Xlinker "$tmp_plist" \
    -o "$app_exec" \
    "${sources[@]}"

  # Debug 用にバイナリと plist を直下にもコピー
  cp "$app_exec" "$bin_dir/$name"
  cp "$plist" "$bin_dir/Info.plist"

  # Ad-hoc サイン
  codesign --force --deep --sign - "$bin_dir/${app_bundle_name}"

  rm -f "$tmp_plist"
  echo "Done: $name"
}

# 1. floating-overlay
build_sidecar "floating-overlay" \
  "$bin_dir/floating-overlay.swift" \
  "$bin_dir/floating-overlay-main.swift"

# 2. speech-recognizer
build_sidecar "speech-recognizer" \
  "$bin_dir/speech-recognizer.swift"

echo "All sidecars built successfully in $bin_dir"
