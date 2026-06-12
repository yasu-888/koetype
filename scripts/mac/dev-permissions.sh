#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
debug_bin="$root_dir/src-tauri/target/debug/koetype"
bundle_id="com.koetype.desktop"
sidecar_bundle_id="com.koetype.desktop.speech-recognizer"
overlay_bundle_id="com.koetype.desktop.floating-overlay"

echo "=== KoeType dev permission setup ==="
echo ""
echo "このスクリプトは dev ビルド向けの TCC 権限をリセットし、"
echo "System Preferences の Privacy 設定を開きます。"
echo ""
echo "権限を付与するバイナリ:"
echo "  - $debug_bin"
echo "  - speech-recognizer sidecar ($sidecar_bundle_id)"
echo "  - floating-overlay sidecar ($overlay_bundle_id)"
echo "  - このターミナルアプリ自体 (グローバルショートカット取得に必要)"
echo ""

echo "[1/3] Accessibility TCC をリセット..."
tccutil reset Accessibility "$bundle_id" || true
tccutil reset Accessibility "$sidecar_bundle_id" || true
tccutil reset Accessibility "$overlay_bundle_id" || true

echo "[2/3] Microphone / SpeechRecognition TCC をリセット..."
tccutil reset Microphone "$bundle_id" || true
tccutil reset SpeechRecognition "$bundle_id" || true
tccutil reset Microphone "$sidecar_bundle_id" || true
tccutil reset SpeechRecognition "$sidecar_bundle_id" || true

echo "[3/3] Privacy 設定を開く..."
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility" || true
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone" || true
open "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition" || true

echo ""
echo "次のステップ:"
echo "  1) Accessibility: このターミナルアプリ と KoeType dev バイナリを有効化"
echo "     バイナリパス: $debug_bin"
echo "  2) Microphone: KoeType dev バイナリ と speech-recognizer を有効化"
echo "  3) Speech Recognition: speech-recognizer を有効化"
echo "  4) pnpm tauri dev を実行"
