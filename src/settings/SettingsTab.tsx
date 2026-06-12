/** @format */

import { useState, useCallback } from "react";
import { CheckIcon, CircleNotchIcon } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { SttProvider } from "../types";
import { getErrorMessage } from "../utils/error";
import { secondaryButtonClass } from "../utils/styles";
import { SttProviderSection } from "./SttProviderSection";
import { GeminiSection } from "./GeminiSection";
import { ShortcutSection } from "./ShortcutSection";
import { OptionsSection } from "./OptionsSection";

const FIXED_MIC_SENSITIVITY = 1;
const FIXED_WAVE_MOTION_SCALE = 1;

const Icons = {
  Check: () => <CheckIcon className="w-5 h-5 text-emerald-500" weight="bold" />,
  Spinner: () => <CircleNotchIcon className="w-4 h-4 animate-spin" weight="bold" />,
};

const exportFeedbackButtonClass = (state: "idle" | "saving" | "success") =>
  state === "saving"
    ? "bg-transparent text-gray-500 h-12 flex-shrink-0 w-24 flex items-center justify-center cursor-not-allowed"
    : `${secondaryButtonClass} ${
        state === "success"
          ? "bg-emerald-50 border-emerald-200 text-emerald-700 hover:bg-emerald-50 hover:text-emerald-700"
          : ""
      }`.trim();

export function SettingsTab() {
  const [exportState, setExportState] = useState<"idle" | "saving" | "success">("idle");
  const [sttProvider, setSttProvider] = useState<SttProvider>("gemini");

  const handleExportSettingsToFile = useCallback(async () => {
    setExportState("saving");
    await new Promise((resolve) => setTimeout(resolve, 500));
    try {
      const [provider, model, aiModel, whisperPaths, whisperPath, soundMode, deliveryMode, device, lang, minInputMs, hybridMs, inputShortcut, osPasteShortcut] = await Promise.allSettled([
        invoke<string>("get_stt_provider"),
        invoke<string>("get_model"),
        invoke<string>("get_ai_model"),
        invoke<string[]>("get_whisper_model_paths"),
        invoke<string | null>("get_whisper_model_path"),
        invoke<{ sound_mode: string }>("get_feedback_settings"),
        invoke<string>("get_input_delivery_mode"),
        invoke<string | null>("get_selected_audio_device"),
        invoke<string>("get_language"),
        invoke<number>("get_min_input_duration_ms"),
        invoke<number>("get_hybrid_threshold_ms"),
        invoke<{ input_shortcut: string; os_paste_shortcut: string }>("get_shortcut_settings").then((s) => s.input_shortcut),
        invoke<{ input_shortcut: string; os_paste_shortcut: string }>("get_shortcut_settings").then((s) => s.os_paste_shortcut),
      ]);

      const saves: Promise<unknown>[] = [];
      if (provider.status === "fulfilled") saves.push(invoke("set_stt_provider", { provider: provider.value }));
      if (model.status === "fulfilled") saves.push(invoke("set_model", { model: model.value }));
      if (aiModel.status === "fulfilled") saves.push(invoke("set_ai_model", { model: aiModel.value }));
      if (whisperPaths.status === "fulfilled") saves.push(invoke("set_whisper_model_paths", { paths: whisperPaths.value }));
      if (whisperPath.status === "fulfilled") saves.push(invoke("set_whisper_model_path", { modelPath: whisperPath.value }));
      if (soundMode.status === "fulfilled") saves.push(invoke("set_sound_setting", { mode: soundMode.value.sound_mode }));
      if (deliveryMode.status === "fulfilled") saves.push(invoke("set_input_delivery_mode", { mode: deliveryMode.value }));
      if (device.status === "fulfilled") saves.push(invoke("set_audio_device", { deviceName: device.value }));
      if (lang.status === "fulfilled") saves.push(invoke("set_language", { lang: lang.value }));
      if (minInputMs.status === "fulfilled") saves.push(invoke("set_min_input_duration_ms", { durationMs: minInputMs.value }));
      if (hybridMs.status === "fulfilled") saves.push(invoke("set_hybrid_threshold_ms", { durationMs: hybridMs.value }));
      saves.push(invoke("set_mic_sensitivity", { value: FIXED_MIC_SENSITIVITY }));
      saves.push(invoke("set_wave_motion_scale", { value: FIXED_WAVE_MOTION_SCALE }));
      if (inputShortcut.status === "fulfilled" && osPasteShortcut.status === "fulfilled") {
        saves.push(invoke("set_shortcut_settings", {
          shortcuts: {
            input_shortcut: inputShortcut.value,
            os_paste_shortcut: osPasteShortcut.value,
          },
        }));
      }

      await Promise.all(saves);
      setExportState("success");
      setTimeout(() => setExportState("idle"), 1200);
    } catch (error: unknown) {
      setExportState("idle");
      alert(`設定ファイル出力失敗: ${getErrorMessage(error)}`);
    }
  }, []);

  return (
    <>
      <header className="mb-12 flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900 tracking-tight">設定</h1>
          <p className="text-sm text-gray-500 mt-2">モデル、オプションなどを自由に設定できます。</p>
        </div>
        <button
          onClick={handleExportSettingsToFile}
          disabled={exportState === "saving"}
          className={exportFeedbackButtonClass(exportState)}
        >
          {exportState === "saving" ? (
            <Icons.Spinner />
          ) : exportState === "success" ? (
            <>
              <Icons.Check />
              Saved
            </>
          ) : (
            "設定を出力"
          )}
        </button>
      </header>

      <SttProviderSection onProviderChange={setSttProvider} />
      <GeminiSection />
      <ShortcutSection sttProvider={sttProvider} />
      <OptionsSection />
    </>
  );
}
