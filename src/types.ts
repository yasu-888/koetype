/** @format */

// Shared primitive types used across UI components

export type FeedbackMode = "AlwaysOff" | "AlwaysOn" | "FullscreenOnly";

export type InputDeliveryMode = "clipboard" | "type";
export type SttProvider = "gemini" | "whisper" | "hybrid" | "collaborate";

export interface FeedbackSettings {
  notification_mode: FeedbackMode;
  sound_mode: FeedbackMode;
}

export interface HistoryItem {
  id: number;
  text: string;
  dictation_text?: string | null;
  whisper_text?: string | null;
  stt_provider?: SttProvider | null;
  timestamp: number;
  duration_ms: number;
}

export interface StatsSnapshot {
  total_entries: number;
  total_characters: number;
  total_words: number;
  total_duration_ms: number;
  avg_wpm: number;
  updated_at: number;
}

export interface DailyUsagePoint {
  date: string;
  characters: number;
  entries: number;
}

export interface ShortcutSettings {
  input_shortcut: string;
  os_paste_shortcut: string;
}

// ユーザー辞書エントリ
export interface DictionaryEntry {
  id: number;
  word: string;
  created_at: number;
}

// エラーログエントリ
export interface ErrorLogEntry {
  id: number;
  timestamp: number;
  error_type: ErrorType;
  message: string;
  context?: string;
}

export type ErrorType =
  | "ApiError"
  | "NetworkError"
  | "FileError"
  | "AudioError"
  | "ConfigError"
  | "UnknownError";

export interface AudioDevice {
  name: string;
  id: string;
  is_default: boolean;
}

export interface AutomationProbeResult {
  ok: boolean;
  status_code: number | null;
  stdout: string;
  stderr: string;
}

export interface EnigoProbeResult {
  ok: boolean;
  message: string;
}

export interface PasteRuntimeDiag {
  executable_path: string;
  bundle_identifier: string;
  codesign_cdhash: string | null;
  ax_trusted: boolean;
  frontmost_app_bundle_id: string | null;
  frontmost_before_paste: string | null;
  frontmost_after_paste: string | null;
  last_paste_method: string | null;
  last_paste_error: string | null;
  last_restore_policy: string | null;
  last_paste_elapsed_ms: number | null;
  last_applescript_status_code: number | null;
  last_applescript_stderr: string | null;
  last_enigo_error: string | null;
  last_used_fallback: boolean | null;
  delivery_mode: string | null;
  snapshot_taken: boolean | null;
  restore_attempted: boolean | null;
  restore_ok: boolean | null;
  transient_markers_applied: boolean | null;
  type_error: string | null;
  automation_probe: AutomationProbeResult;
}

export interface PasteSelfTestResult {
  applescript_result: AutomationProbeResult;
  enigo_result: EnigoProbeResult;
  selected_path: string;
  final_status: "success" | "permission_denied" | "indeterminate";
}

export interface OnboardingStatus {
  is_macos: boolean;
  ack_unnotarized: boolean;
  completed_at: number | null;
  ax_trusted: boolean;
  permission_probe_ok: boolean | null;
  whisper_cli_path: string | null;
  whisper_model_path: string | null;
  whisper_bundled_cli: boolean;
  whisper_bundled_model: boolean;
  whisper_ready: boolean;
  all_ready: boolean;
}

export interface PermissionProbeResult {
  success: boolean;
  detail: string;
}
