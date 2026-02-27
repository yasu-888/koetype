/** @format */

import { useState, useEffect, useRef, useCallback } from "react";
import { BarnIcon, BrainIcon, CaretDownIcon, CheckIcon, CircleNotchIcon, TimerIcon } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { SttProvider } from "../types";
import { getErrorMessage } from "../utils/error";
import {
  inputClass,
  secondaryButtonClass,
  selectClass,
  selectContainerClass,
  selectIconClass,
  saveFeedbackButtonClass,
} from "../utils/styles";
import { MODELS, STT_PROVIDERS } from "./options";
import { Card, CardItem } from "../components/Card";

const DEFAULT_WHISPER_MODEL_PATH = "__whisper_model_auto__";

const Icons = {
  Barn: () => <BarnIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  Brain: () => <BrainIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  ChevronDown: () => <CaretDownIcon className="w-4 h-4 text-gray-500" weight="bold" />,
  Check: () => <CheckIcon className="w-5 h-5 text-emerald-500" weight="bold" />,
  Spinner: () => <CircleNotchIcon className="w-4 h-4 animate-spin" weight="bold" />,
  Timer: () => <TimerIcon className="w-5 h-5" weight="regular" />,
};

interface SttProviderSectionProps {
  onProviderChange: (provider: SttProvider) => void;
}

export function SttProviderSection({ onProviderChange }: SttProviderSectionProps) {
  const [sttProvider, setSttProvider] = useState<SttProvider>("gemini");
  const [selectedModel, setSelectedModel] = useState("gemini-2.5-flash-lite");
  const [whisperModelPath, setWhisperModelPath] = useState<string>(DEFAULT_WHISPER_MODEL_PATH);
  const [whisperModelOptions, setWhisperModelOptions] = useState<string[]>([]);
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [saveHybridThresholdState, setSaveHybridThresholdState] = useState<"idle" | "saving" | "success">("idle");
  const [hybridThresholdSeconds, setHybridThresholdSeconds] = useState<string>("10");
  const testResultTimerRef = useRef<number | null>(null);

  const alertActionError = useCallback((prefix: string, error: unknown) => {
    alert(`${prefix}: ${getErrorMessage(error)}`);
  }, []);

  useEffect(() => {
    const load = async () => {
      try {
        const provider = await invoke<string>("get_stt_provider");
        const normalized =
          provider === "gemini" || provider === "hybrid" || provider === "collaborate"
            ? (provider as SttProvider)
            : "whisper";
        setSttProvider(normalized);
        onProviderChange(normalized);
      } catch {
        // ignore
      }
      try {
        const model = await invoke<string>("get_model");
        setSelectedModel(model);
      } catch {
        // ignore
      }
      try {
        const [paths, currentPath] = await Promise.all([
          invoke<string[]>("get_whisper_model_paths"),
          invoke<string | null>("get_whisper_model_path"),
        ]);
        const merged = new Set(paths.filter((p) => p.trim().length > 0));
        const normalizedCurrent = currentPath?.trim() ? currentPath.trim() : null;
        if (normalizedCurrent) merged.add(normalizedCurrent);
        setWhisperModelOptions(Array.from(merged).sort((a, b) => a.localeCompare(b)));
        setWhisperModelPath(normalizedCurrent ?? DEFAULT_WHISPER_MODEL_PATH);
      } catch {
        setWhisperModelOptions([]);
        setWhisperModelPath(DEFAULT_WHISPER_MODEL_PATH);
      }
      try {
        const durationMs = await invoke<number>("get_hybrid_threshold_ms");
        const seconds = durationMs / 1000;
        setHybridThresholdSeconds(Number.isInteger(seconds) ? String(seconds) : seconds.toFixed(1));
      } catch {
        // ignore
      }
    };
    load();
  }, [onProviderChange]);

  useEffect(() => {
    return () => {
      if (testResultTimerRef.current !== null) {
        window.clearTimeout(testResultTimerRef.current);
      }
    };
  }, []);

  const showTimedTestResult = (result: { success: boolean; message: string } | null) => {
    if (testResultTimerRef.current !== null) {
      window.clearTimeout(testResultTimerRef.current);
      testResultTimerRef.current = null;
    }
    setTestResult(result);
    if (!result) return;
    testResultTimerRef.current = window.setTimeout(() => {
      setTestResult(null);
      testResultTimerRef.current = null;
    }, 3000);
  };

  const handleSaveSttProvider = async (provider: SttProvider) => {
    try {
      setSttProvider(provider);
      onProviderChange(provider);
      await invoke("set_stt_provider", { provider });
    } catch (error: unknown) {
      alertActionError("文字起こしモードの設定失敗", error);
    }
  };

  const handleSaveModel = async (model: string) => {
    try {
      setSelectedModel(model);
      await invoke("set_model", { model });
    } catch (error: unknown) {
      alertActionError("モデル設定失敗", error);
    }
  };

  const handleSaveWhisperModelPath = async (path: string) => {
    const targetPath = path === DEFAULT_WHISPER_MODEL_PATH ? null : path;
    try {
      await invoke("set_whisper_model_path", { modelPath: targetPath });
      setWhisperModelPath(path);
      const merged = targetPath
        ? Array.from(new Set([...whisperModelOptions, targetPath])).sort((a, b) => a.localeCompare(b))
        : whisperModelOptions;
      if (targetPath) setWhisperModelOptions(merged);
      await invoke("set_whisper_model_paths", { paths: merged });
    } catch (error: unknown) {
      alertActionError("Whisperモデル設定失敗", error);
    }
  };

  const handlePickWhisperModelPath = async () => {
    try {
      const picked = await invoke<string | null>("pick_whisper_model_path");
      if (!picked) return;
      await handleSaveWhisperModelPath(picked);
    } catch (error: unknown) {
      alertActionError("Whisperモデル選択失敗", error);
    }
  };

  const handleTestConnection = async () => {
    if (sttProvider === "whisper") {
      showTimedTestResult({ success: false, message: "Whisper選択時はAPI接続テストは不要です（ローカル実行）" });
      return;
    }
    let apiKey = "";
    try {
      apiKey = await invoke<string>("get_api_key");
    } catch {
      // ignore
    }
    if (!apiKey) {
      showTimedTestResult({ success: false, message: "先にAPIキーを入力してください" });
      return;
    }
    setIsTesting(true);
    showTimedTestResult(null);
    try {
      const result = await invoke<string>("test_gemini_connection", { apiKey, model: selectedModel });
      showTimedTestResult({ success: true, message: `接続成功!\n${result}` });
    } catch (error: unknown) {
      showTimedTestResult({ success: false, message: `接続失敗: ${getErrorMessage(error)}` });
    } finally {
      setIsTesting(false);
    }
  };

  const handleSaveHybridThresholdSeconds = async () => {
    const seconds = Number(hybridThresholdSeconds);
    if (!Number.isFinite(seconds) || seconds < 0) {
      alert("0以上の数値を入力してください");
      return;
    }
    setSaveHybridThresholdState("saving");
    await new Promise((resolve) => setTimeout(resolve, 500));
    const durationMs = Math.round(seconds * 1000);
    try {
      await invoke("set_hybrid_threshold_ms", { durationMs });
      setSaveHybridThresholdState("success");
      setTimeout(() => setSaveHybridThresholdState("idle"), 1200);
    } catch (error: unknown) {
      setSaveHybridThresholdState("idle");
      alertActionError("設定失敗", error);
    }
  };

  const testButtonStateClass = testResult
    ? testResult.success
      ? "bg-emerald-50 border-emerald-200 text-emerald-700 hover:bg-emerald-50 hover:border-emerald-200 hover:text-emerald-700"
      : "bg-[#bc002d]/10 border-[#bc002d]/35 text-[#bc002d] hover:bg-[#bc002d]/10 hover:border-[#bc002d]/35 hover:text-[#bc002d]"
    : "";
  const testButtonLabel = isTesting ? "テスト中" : testResult ? (testResult.success ? "成功" : "失敗") : "テスト";

  const renderWhisperModelOptionLabel = (path: string) => path.split(/[/\\]/).pop() || path;

  return (
    <div className="mb-12">
      <h3 className="text-xs font-bold text-gray-400 tracking-wider mb-8 ml-1">文字起こしモード</h3>
      <Card>
        <CardItem icon={<Icons.Barn />} label="プロバイダー">
          <div className="flex flex-col items-end gap-2 w-full">
            <div className={selectContainerClass}>
              <select
                value={sttProvider}
                onChange={(e) => handleSaveSttProvider(e.target.value as SttProvider)}
                className={selectClass}
              >
                {STT_PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value}>
                    {p.label}
                  </option>
                ))}
              </select>
              <div className={selectIconClass}>
                <Icons.ChevronDown />
              </div>
            </div>
            {sttProvider === "hybrid" && (
              <div className="text-xs text-gray-500 max-w-[520px] text-right leading-relaxed">
                Hybrid閾値以下ならWhisper、超えたらGeminiで処理します
              </div>
            )}
            {sttProvider === "collaborate" && (
              <div className="text-xs text-gray-500 max-w-[520px] text-right leading-relaxed">
                Whisperで文字起こしした後、Geminiで文章を整形します
              </div>
            )}
          </div>
        </CardItem>

        {sttProvider === "hybrid" && (
          <CardItem icon={<Icons.Timer />} label="Hybrid閾値">
            <div className="flex gap-3 items-center">
              <div className="w-full max-w-[160px]">
                <input
                  type="number"
                  min={0}
                  step={0.1}
                  value={hybridThresholdSeconds}
                  onChange={(e) => setHybridThresholdSeconds(e.target.value)}
                  className={inputClass}
                />
              </div>
              <span className="text-sm text-gray-500">秒</span>
              <button
                onClick={handleSaveHybridThresholdSeconds}
                disabled={saveHybridThresholdState === "saving"}
                className={saveFeedbackButtonClass(saveHybridThresholdState)}
              >
                {saveHybridThresholdState === "saving" ? (
                  <Icons.Spinner />
                ) : saveHybridThresholdState === "success" ? (
                  <Icons.Check />
                ) : null}
                {saveHybridThresholdState === "saving" ? "" : saveHybridThresholdState === "success" ? "Saved" : "Save"}
              </button>
            </div>
          </CardItem>
        )}

        {(sttProvider === "gemini" || sttProvider === "hybrid" || sttProvider === "collaborate") && (
          <CardItem icon={<Icons.Brain />} label="文字起こし用Geminiモデル">
            <div className="flex flex-col items-end gap-3 w-auto">
              <div className="flex gap-3 w-auto">
                <div className={selectContainerClass}>
                  <select
                    value={selectedModel}
                    onChange={(e) => handleSaveModel(e.target.value)}
                    className={selectClass}
                  >
                    {MODELS.map((m) => (
                      <option key={m} value={m}>
                        {m}
                      </option>
                    ))}
                  </select>
                  <div className={selectIconClass}>
                    <Icons.ChevronDown />
                  </div>
                </div>
                <button
                  onClick={handleTestConnection}
                  disabled={isTesting}
                  className={`${secondaryButtonClass} ${testButtonStateClass}`.trim()}
                  title={testResult?.message || ""}
                >
                  {testButtonLabel}
                </button>
              </div>
            </div>
          </CardItem>
        )}

        {(sttProvider === "whisper" || sttProvider === "hybrid" || sttProvider === "collaborate") && (
          <CardItem icon={<Icons.Brain />} label="Whisperモデル">
            <div className="flex flex-col items-end gap-3 w-auto">
              <div className="flex gap-3 w-auto">
                <div className={selectContainerClass}>
                  <select
                    value={whisperModelPath}
                    onChange={(e) => handleSaveWhisperModelPath(e.target.value)}
                    className={selectClass}
                  >
                    <option value={DEFAULT_WHISPER_MODEL_PATH}>自動選択（既定）</option>
                    {whisperModelOptions.map((path) => (
                      <option key={path} value={path}>
                        {renderWhisperModelOptionLabel(path)}
                      </option>
                    ))}
                  </select>
                  <div className={selectIconClass}>
                    <Icons.ChevronDown />
                  </div>
                </div>
                <button onClick={handlePickWhisperModelPath} className={secondaryButtonClass}>
                  追加
                </button>
              </div>
              <div className="text-xs text-gray-500 max-w-[520px] text-right leading-relaxed">
                `ggml-*.bin` を `~/.config/koetype/models/whisper/` に直接配置、または「追加」
                <br />
              </div>
            </div>
          </CardItem>
        )}
      </Card>
    </div>
  );
}
