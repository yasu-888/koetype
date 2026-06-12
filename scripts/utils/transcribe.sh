#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
Usage: ./scripts/utils/transcribe.sh <audio-file> [<lang>] [<model>] [<variant>]

  audio-file  Path to audio file (WAV recommended)
  lang        Language code: auto, ja, en  (default: auto)
  model       Whisper model name            (default: large-v3-turbo)
              e.g. tiny, base, small, medium, large-v3, large-v3-turbo
  variant     Quantization variant          (default: q5_0)
              e.g. full, q5_0, q5_1, q8_0

Environment variables:
  KOETYPE_HOME                base dir (default: ~/.config/koetype)
  WHISPER_MODELS_DIR            モデル保存先ディレクトリを上書き
  KOETYPE_WHISPER_MODELS_DIR  alias for WHISPER_MODELS_DIR
  WHISPER_OUTPUT_DIR            文字起こし出力ディレクトリを上書き
  KOETYPE_WHISPER_OUTPUT_DIR  alias for WHISPER_OUTPUT_DIR
  KOETYPE_LOG_DIR             ログ保存先ディレクトリを上書き
  KOETYPE_WHISPER_CLI_PATH    whisper-cli の実行ファイルパスを明示指定
EOF
}

if [[ $# -lt 1 || "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 2
fi

audio_file="$1"
lang="${2:-auto}"
model="${3:-large-v3-turbo}"
variant="${4:-q5_0}"

if [[ ! -f "$audio_file" ]]; then
  echo "Audio file not found: $audio_file" >&2
  exit 2
fi

app_home="${KOETYPE_HOME:-$HOME/.config/koetype}"
models_dir="${WHISPER_MODELS_DIR:-${KOETYPE_WHISPER_MODELS_DIR:-$app_home/models/whisper}}"
output_dir="${WHISPER_OUTPUT_DIR:-${KOETYPE_WHISPER_OUTPUT_DIR:-$app_home/outputs/transcriptions}}"
log_dir="${KOETYPE_LOG_DIR:-$app_home/logs}"

if [[ "$variant" == "full" ]]; then
  model_file="ggml-${model}.bin"
else
  model_file="ggml-${model}-${variant}.bin"
fi

model_path="$models_dir/$model_file"
if [[ ! -f "$model_path" ]]; then
  echo "モデルが見つかりません: $model_path" >&2
  echo "先に実行してください: ./scripts/setup/whisper.sh $model $variant" >&2
  exit 2
fi

threads="$(sysctl -n hw.logicalcpu 2>/dev/null || echo 8)"
mkdir -p "$output_dir" "$log_dir"
stamp="$(date +%Y%m%d-%H%M%S)"
audio_stem="$(basename "${audio_file%.*}")"
output_base="$output_dir/${audio_stem}_${stamp}"
log_file="$log_dir/whisper-cli.log"

resolve_whisper_cli_path() {
  if [[ -n "${KOETYPE_WHISPER_CLI_PATH:-}" && -x "${KOETYPE_WHISPER_CLI_PATH}" ]]; then
    echo "${KOETYPE_WHISPER_CLI_PATH}"
    return 0
  fi
  local local_cli="$app_home/bin/whisper-cli"
  if [[ -x "$local_cli" ]]; then
    echo "$local_cli"
    return 0
  fi
  if [[ -x "/opt/homebrew/bin/whisper-cli" ]]; then
    echo "/opt/homebrew/bin/whisper-cli"
    return 0
  fi
  if [[ -x "/usr/local/bin/whisper-cli" ]]; then
    echo "/usr/local/bin/whisper-cli"
    return 0
  fi
  if command -v whisper-cli >/dev/null 2>&1; then
    command -v whisper-cli
    return 0
  fi
  echo "whisper-cli が見つかりません。KOETYPE_WHISPER_CLI_PATH か ~/.config/koetype/bin/whisper-cli を確認してください。" >&2
  return 1
}

cli_path="$(resolve_whisper_cli_path)"

echo "文字起こしを開始します..."
echo "cli=$cli_path"
echo "model=$model_path"
echo "audio=$audio_file"
echo "language=$lang"

"$cli_path" \
  -m "$model_path" \
  -f "$audio_file" \
  -l "$lang" \
  -t "$threads" \
  -otxt -ovtt -osrt -oj \
  -of "$output_base" \
  -pp \
  2>&1 | tee -a "$log_file"

echo ""
echo "完了しました。"
echo "txt:  ${output_base}.txt"
echo "srt:  ${output_base}.srt"
echo "vtt:  ${output_base}.vtt"
echo "json: ${output_base}.json"


