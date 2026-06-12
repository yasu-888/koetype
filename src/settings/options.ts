/** @format */

import type { FeedbackMode, SttProvider } from "../types";

export const MODELS = ["gemini-2.5-flash-lite", "gemini-2.5-flash", "gemini-3-flash-preview"] as const;

export const STT_PROVIDERS: { value: SttProvider; label: string }[] = [
  { value: "gemini", label: "Gemini" },
  { value: "whisper", label: "Whisper" },
  { value: "hybrid", label: "Hybrid" },
  { value: "collaborate", label: "Collaborate" },
];

export const FEEDBACK_OPTIONS: { value: FeedbackMode; label: string }[] = [
  { value: "AlwaysOff", label: "常にOFF" },
  { value: "AlwaysOn", label: "常にON" },
  { value: "FullscreenOnly", label: "フルスクリーン時のみON" },
];

export const LANGUAGE_OPTIONS = [
  { value: "ja", label: "日本語" },
  { value: "en", label: "English" },
] as const;

