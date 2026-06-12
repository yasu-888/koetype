#!/usr/bin/env bash
# scripts/manage.sh
# KoeType 開発・運用管理用メインスクリプト

set -euo pipefail

# スクリプトのディレクトリに移動
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# 色の定義
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

build_with_auto_clean() {
    local repo_root
    local target_dir
    local marker_path
    local current_root
    local previous_root
    local path_in_target

    repo_root="$(pwd -P)"
    target_dir="$repo_root/src-tauri/target"
    marker_path="$target_dir/.koetype-build-root"
    current_root="$repo_root"

    if [[ -f "$marker_path" ]]; then
        previous_root="$(cat "$marker_path" 2>/dev/null || true)"
        if [[ -n "$previous_root" && "$previous_root" != "$current_root" ]]; then
            echo -e "${BLUE}stale target を検知: 旧ルート=${previous_root}${NC}"
            echo -e "${BLUE}src-tauri/target を自動削除して再ビルドします${NC}"
            rm -rf "$target_dir"
        fi
    elif [[ -d "$target_dir" ]]; then
        path_in_target="$(
            rg -m1 -o '/[^ :]+/src-tauri' "$target_dir" 2>/dev/null \
                | sed -n '1p' \
                | sed 's#/src-tauri$##'
        )"
        if [[ -n "$path_in_target" && "$path_in_target" != "$current_root" ]]; then
            echo -e "${BLUE}target 内の絶対パス汚染を検知: 旧ルート=${path_in_target}${NC}"
            echo -e "${BLUE}src-tauri/target を自動削除して再ビルドします${NC}"
            rm -rf "$target_dir"
        fi
    fi

    pnpm tauri build
    mkdir -p "$target_dir"
    printf "%s\n" "$current_root" > "$marker_path"
}

usage() {
    echo "KoeType 管理用スクリプト"
    echo ""
    echo "Usage: ./scripts/manage.sh [command]"
    echo ""
    echo "Commands:"
    echo "  build:sidecar    (macOS) Swift sidecar をビルド"
    echo "  install          (macOS) アプリを /Applications にインストール"
    echo "  rebuild-install  (macOS) ビルドしてインストール"
    echo "  setup:whisper    Whisper モデルのセットアップ"
    echo "  transcribe       音声ファイルの文字起こしテスト"
    echo "  help             このヘルプを表示"
    echo ""
    if [[ $# -eq 0 ]]; then
        echo "コマンドを指定せずに実行すると、メニューが表示されます。"
    fi
}

menu() {
    echo -e "${BLUE}=== KoeType 管理メニュー ===${NC}"
    echo "1) Sidecar ビルド (macOS)"
    echo "2) アプリのインストール (macOS)"
    echo "3) ビルド & インストール (macOS)"
    echo "4) Whisper モデル セットアップ"
    echo "5) 文字起こしテスト"
    echo "q) 終了"
    echo ""
    printf "選択してください: "
    read -r choice

    case "$choice" in
        1) ./scripts/mac/build-sidecars.sh ;;
        2) ./scripts/mac/install.sh ;;
        3) build_with_auto_clean && ./scripts/mac/install.sh ;;
        4) ./scripts/setup/whisper.sh ;;
        5)
            echo "文字起こしする音声ファイルのパスを入力してください:"
            read -r file
            ./scripts/utils/transcribe.sh "$file"
            ;;
        q) exit 0 ;;
        *) echo "無効な選択です。" ;;
    esac
}

cmd="${1:-}"

case "$cmd" in
    build:sidecar)
        ./scripts/mac/build-sidecars.sh
        ;;
    install)
        ./scripts/mac/install.sh
        ;;
    rebuild-install)
        build_with_auto_clean && ./scripts/mac/install.sh
        ;;
    setup:whisper)
        ./scripts/setup/whisper.sh
        ;;
    transcribe)
        shift
        ./scripts/utils/transcribe.sh "$@"
        ;;
    help|--help|-h)
        usage
        ;;
    "")
        menu
        ;;
    *)
        echo -e "${RED}エラー: 不明なコマンド '$cmd'${NC}"
        usage
        exit 1
        ;;
esac

