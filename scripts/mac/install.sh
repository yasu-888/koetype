#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
src_app="$root_dir/src-tauri/target/release/bundle/macos/KoeType.app"
dst_app="/Applications/KoeType.app"
launch_agent="$HOME/Library/LaunchAgents/com.koetype.desktop.plist"
bundle_id="com.koetype.desktop"
sidecar_bundle_id="com.koetype.desktop.speech-recognizer"
sidecar_app="$dst_app/Contents/Resources/bin/KoeType Speech Recognizer.app"

if [[ ! -d "$src_app" ]]; then
  echo "error: build output not found: $src_app" >&2
  echo "run: pnpm tauri build" >&2
  exit 1
fi

echo "[1/5] stop running app"
pkill -f "$dst_app/Contents/MacOS/koetype" || true
pkill -f "$dst_app/Contents/Resources/bin/floating-overlay.app/Contents/MacOS/floating-overlay" || true

echo "[2/5] install app to /Applications"
rm -rf "$dst_app"
cp -R "$src_app" "$dst_app"

echo "[3/5] refresh launch agent"
mkdir -p "$HOME/Library/LaunchAgents"
cat > "$launch_agent" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key>
    <string>com.koetype.desktop</string>
    <key>ProgramArguments</key>
    <array>
      <string>/Applications/KoeType.app/Contents/MacOS/koetype</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>StandardOutPath</key>
    <string>/tmp/koetype.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/koetype.err.log</string>
  </dict>
</plist>
PLIST

launchctl unload "$launch_agent" 2>/dev/null || true
launchctl load "$launch_agent"

echo "[4/5] permission handling"
# Reset Accessibility by default to recover from stale authorization entries.
# Use KOETYPE_RESET_ACCESSIBILITY=0 to skip.
if [[ "${KOETYPE_RESET_ACCESSIBILITY:-1}" == "1" ]]; then
  echo "resetting Accessibility TCC grants"
  tccutil reset Accessibility "$bundle_id" || true
  tccutil reset Accessibility "$sidecar_bundle_id" || true
else
  echo "keeping Accessibility TCC grants (set KOETYPE_RESET_ACCESSIBILITY=1 to reset)"
fi

# Keep audio/speech grants by default to avoid repeated prompts.
# Use KOETYPE_RESET_TCC=1 for full reset.
if [[ "${KOETYPE_RESET_TCC:-0}" == "1" ]]; then
  echo "resetting Microphone / SpeechRecognition TCC grants"
  tccutil reset Microphone "$bundle_id" || true
  tccutil reset SpeechRecognition "$bundle_id" || true
  tccutil reset Microphone "$sidecar_bundle_id" || true
  tccutil reset SpeechRecognition "$sidecar_bundle_id" || true
else
  echo "keeping Microphone / SpeechRecognition TCC grants (set KOETYPE_RESET_TCC=1 to reset)"
fi

echo "[5/6] audio permission probe (install time)"
if [[ "${KOETYPE_RUN_AUDIO_PERMISSION_PROBE:-1}" == "1" ]]; then
  if [[ -d "$sidecar_app" ]]; then
    probe_output="/tmp/koetype_install_permission_probe.wav"
    probe_log="/tmp/koetype-permission-probe.log"
    probe_locale="${KOETYPE_PERMISSION_PROBE_LOCALE:-ja-JP}"
    probe_timeout_secs="${KOETYPE_PERMISSION_PROBE_TIMEOUT:-180}"

    open -n -W "$sidecar_app" --args "$probe_output" "$probe_locale" --speech --permissions-only >"$probe_log" 2>&1 &
    probe_pid=$!
    started_at="$(date +%s)"
    while kill -0 "$probe_pid" 2>/dev/null; do
      now="$(date +%s)"
      elapsed=$((now - started_at))
      if [[ "$elapsed" -ge "$probe_timeout_secs" ]]; then
        kill "$probe_pid" 2>/dev/null || true
        echo "audio permission probe timed out after ${probe_timeout_secs}s (killed)"
        break
      fi
      sleep 1
    done
    wait "$probe_pid" 2>/dev/null || true
    echo "audio permission probe finished. log: $probe_log"
  else
    echo "audio permission probe skipped: sidecar not found"
  fi
else
  echo "audio permission probe skipped (set KOETYPE_RUN_AUDIO_PERMISSION_PROBE=1 to enable)"
fi

echo "[6/6] open Privacy settings and start app"
if [[ "${KOETYPE_OPEN_PRIVACY:-1}" == "1" ]]; then
  open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility" || true
  open "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone" || true
  open "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition" || true
fi

if ! pgrep -f "$dst_app/Contents/MacOS/koetype" >/dev/null 2>&1; then
  open -a "$dst_app"
fi

process_count="$(pgrep -f "$dst_app/Contents/MacOS/koetype" | wc -l | tr -d ' ' || true)"
echo "running koetype processes: $process_count"

echo ""
echo "installed: $dst_app"
echo "next:"
echo "  1) enable KoeType / KoeType Speech Recognizer in Accessibility if prompted"
echo "  2) enable KoeType Speech Recognizer in Microphone / Speech Recognition if prompted"
echo "  3) rerun paste diagnostics from Settings"


