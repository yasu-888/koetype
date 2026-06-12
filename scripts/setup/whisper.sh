#!/usr/bin/env bash
set -euo pipefail

HF_BASE="https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
app_home="${KOETYPE_HOME:-$HOME/.config/koetype}"
models_dir="${WHISPER_MODELS_DIR:-${KOETYPE_WHISPER_MODELS_DIR:-$app_home/models/whisper}}"

MODELS=(
  tiny
  tiny.en
  base
  base.en
  small
  small.en
  medium
  medium.en
  large-v1
  large-v2
  large-v3
  large-v3-turbo
)

get_variants() {
  case "$1" in
    tiny|tiny.en|base|base.en|small|small.en)
      echo "full q5_1 q8_0" ;;
    medium|medium.en|large-v2|large-v3|large-v3-turbo)
      echo "full q5_0 q8_0" ;;
    large-v1)
      echo "full" ;;
    *)
      echo "Unknown model: $1" >&2
      return 1 ;;
  esac
}

model_file_name() {
  local model="$1" variant="$2"
  if [[ "$variant" == "full" ]]; then
    echo "ggml-${model}.bin"
  else
    echo "ggml-${model}-${variant}.bin"
  fi
}

usage() {
  cat >&2 <<'EOF'
Usage:
  ./scripts/setup/whisper.sh                   # interactive model/variant selection
  ./scripts/setup/whisper.sh <model>           # interactive variant selection
  ./scripts/setup/whisper.sh <model> <variant> # non-interactive

Models:
  tiny, tiny.en, base, base.en, small, small.en
  medium, medium.en, large-v1, large-v2, large-v3, large-v3-turbo

Variants (by model family):
  tiny / base / small  : full, q5_1, q8_0
  medium               : full, q5_0, q8_0
  large-v1             : full
  large-v2 / v3 / turbo: full, q5_0, q8_0

Environment variables:
  KOETYPE_HOME              base dir (default: ~/.config/koetype)
  WHISPER_MODELS_DIR          override model storage directory
  KOETYPE_WHISPER_MODELS_DIR  alias for WHISPER_MODELS_DIR
EOF
}

select_from_list() {
  local prompt="$1"; shift
  local -a items=("$@")
  local i=1
  for item in "${items[@]}"; do
    printf "  %2d) %s\n" "$i" "$item" >&2
    ((i++))
  done
  local choice
  printf "%s [1-%d]: " "$prompt" "${#items[@]}" >&2
  read -r choice
  if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= ${#items[@]} )); then
    echo "${items[$((choice - 1))]}"
  else
    echo "Invalid selection." >&2
    exit 2
  fi
}

# --- Argument parsing ---
if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

model="${1:-}"
variant="${2:-}"

# Select model interactively if not provided
if [[ -z "$model" ]]; then
  echo "Select a Whisper model:" >&2
  model="$(select_from_list "Model" "${MODELS[@]}")"
fi

# Validate model and collect available variants
if ! variants_str="$(get_variants "$model")"; then
  echo "Unknown model: $model" >&2
  echo "Run '$0 --help' for available models." >&2
  exit 2
fi
read -ra valid_variants <<< "$variants_str"

# Select variant interactively if not provided
if [[ -z "$variant" ]]; then
  if [[ ${#valid_variants[@]} -eq 1 ]]; then
    variant="${valid_variants[0]}"
    echo "Only one variant available for '$model': $variant" >&2
  else
    echo "" >&2
    echo "Select a variant for '$model':" >&2
    variant="$(select_from_list "Variant" "${valid_variants[@]}")"
  fi
else
  # Validate provided variant
  is_valid=false
  for v in "${valid_variants[@]}"; do
    if [[ "$v" == "$variant" ]]; then
      is_valid=true
      break
    fi
  done
  if [[ "$is_valid" != "true" ]]; then
    echo "Variant '$variant' is not available for model '$model'." >&2
    echo "Available: ${valid_variants[*]}" >&2
    exit 2
  fi
fi

# --- Download ---
mkdir -p "$models_dir"

file_name="$(model_file_name "$model" "$variant")"
model_path="$models_dir/$file_name"
model_url="$HF_BASE/$file_name"

echo ""
echo "model:   $model"
echo "variant: $variant"
echo "file:    $file_name"
echo "dest:    $model_path"
echo ""

if [[ -f "$model_path" ]]; then
  echo "Already exists: $model_path"
else
  echo "Downloading: $model_url"
  curl -L --fail --progress-bar "$model_url" -o "$model_path"
  echo "Downloaded: $model_path"
fi

echo ""
echo "Done."
echo ""
echo "Next:"
echo "  ./scripts/utils/transcribe.sh <audio-file> [auto|ja|en] $model $variant"

