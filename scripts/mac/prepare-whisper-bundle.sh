#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
resources_dir="$root_dir/src-tauri/resources"
bin_dir="$resources_dir/bin"
model_dir="$resources_dir/models/whisper"
cli_path="$bin_dir/whisper-cli"
model_name="${KOETYPE_BUNDLE_MODEL_NAME:-ggml-large-v3-turbo-q5_0.bin}"
model_path="$model_dir/$model_name"

mkdir -p "$bin_dir" "$model_dir"

detect_arch() {
  case "$(uname -m)" in
    arm64) echo "arm64" ;;
    x86_64) echo "x64" ;;
    *)
      echo "unsupported architecture: $(uname -m)" >&2
      exit 1
      ;;
  esac
}

install_whisper_cli_from_homebrew() {
  local src_cli
  src_cli="$(command -v whisper-cli || true)"
  if [[ -z "$src_cli" ]]; then
    if ! command -v brew >/dev/null 2>&1; then
      echo "brew が見つかりません。whisper-cli を取得できません。" >&2
      exit 1
    fi
    echo "Installing whisper-cpp via Homebrew..."
    brew install whisper-cpp
    src_cli="$(command -v whisper-cli || true)"
  fi

  if [[ -z "$src_cli" || ! -x "$src_cli" ]]; then
    echo "whisper-cli の取得に失敗しました。" >&2
    exit 1
  fi

  cp "$src_cli" "$cli_path"
  chmod +x "$cli_path"

  # whisper-cli が Homebrew の動的ライブラリへ依存している場合は同梱し、実行時に @loader_path を参照させる。
  local dylib
  while IFS= read -r dylib; do
    [[ -z "$dylib" ]] && continue
    local base
    base="$(basename "$dylib")"
    cp "$dylib" "$bin_dir/$base"
    install_name_tool -change "$dylib" "@loader_path/$base" "$cli_path"
  done < <(otool -L "$src_cli" | awk '/Cellar\/.*\.dylib/ {print $1}')
}

download_model() {
  local model_url="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${model_name}"
  echo "Downloading whisper model: $model_url"
  curl -L --fail --progress-bar "$model_url" -o "$model_path"
}

if [[ ! -x "$cli_path" ]]; then
  detect_arch >/dev/null
  install_whisper_cli_from_homebrew
else
  echo "whisper-cli already exists: $cli_path"
fi

if [[ ! -f "$model_path" ]]; then
  download_model
else
  echo "model already exists: $model_path"
fi

echo "Whisper bundle ready:"
echo "  CLI:   $cli_path"
echo "  Model: $model_path"
