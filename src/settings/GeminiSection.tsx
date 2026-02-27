/** @format */

import { useState, useEffect, useRef, useCallback } from "react";
import { BrainIcon, CaretDownIcon, KeyIcon } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { getErrorMessage } from "../utils/error";
import { inputClass, secondaryButtonClass, selectClass, selectContainerClass, selectIconClass } from "../utils/styles";
import { MODELS } from "./options";
import { Card, CardItem } from "../components/Card";

const AUTO_SAVE_DEBOUNCE_MS = 700;

const normalizeApiKey = (value: string): string => value.trim();

const Icons = {
  Brain: () => <BrainIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  ChevronDown: () => <CaretDownIcon className="w-4 h-4 text-gray-500" weight="bold" />,
  Key: () => <KeyIcon className="w-5 h-5 text-gray-400" weight="regular" />,
};

export function GeminiSection() {
  const [apiKey, setApiKey] = useState("");
  const [aiModel, setAiModel] = useState("gemini-2.5-flash-lite");

  const [isAiTesting, setIsAiTesting] = useState(false);
  const [aiTestResult, setAiTestResult] = useState<{ success: boolean; message: string } | null>(null);

  const settingsReadyRef = useRef(false);
  const lastSavedApiKeyRef = useRef<string>("");
  const apiKeyDebounceTimerRef = useRef<number | null>(null);
  const aiTestResultTimerRef = useRef<number | null>(null);

  const alertActionError = useCallback((prefix: string, error: unknown) => {
    alert(`${prefix}: ${getErrorMessage(error)}`);
  }, []);

  useEffect(() => {
    const load = async () => {
      settingsReadyRef.current = false;
      try {
        const key = await invoke<string>("get_api_key");
        setApiKey(key);
        lastSavedApiKeyRef.current = normalizeApiKey(key);
      } catch {
        setApiKey("");
        lastSavedApiKeyRef.current = "";
      }
      try {
        const model = await invoke<string>("get_ai_model");
        setAiModel(model);
      } catch {
        // ignore
      }
      settingsReadyRef.current = true;
    };
    load();
    return () => {
      if (apiKeyDebounceTimerRef.current !== null) window.clearTimeout(apiKeyDebounceTimerRef.current);
      if (aiTestResultTimerRef.current !== null) window.clearTimeout(aiTestResultTimerRef.current);
    };
  }, []);

  const saveApiKey = useCallback(
    async (nextApiKey: string) => {
      if (!settingsReadyRef.current) return;
      const normalized = normalizeApiKey(nextApiKey);
      if (normalized === lastSavedApiKeyRef.current) return;
      try {
        await invoke("set_api_key", { apiKey: nextApiKey });
        lastSavedApiKeyRef.current = normalized;
      } catch (error: unknown) {
        alertActionError("保存失敗", error);
      }
    },
    [alertActionError],
  );

  useEffect(() => {
    if (!settingsReadyRef.current) return;
    if (apiKeyDebounceTimerRef.current !== null) window.clearTimeout(apiKeyDebounceTimerRef.current);
    apiKeyDebounceTimerRef.current = window.setTimeout(() => {
      apiKeyDebounceTimerRef.current = null;
      void saveApiKey(apiKey);
    }, AUTO_SAVE_DEBOUNCE_MS);
    return () => {
      if (apiKeyDebounceTimerRef.current !== null) {
        window.clearTimeout(apiKeyDebounceTimerRef.current);
        apiKeyDebounceTimerRef.current = null;
      }
    };
  }, [apiKey, saveApiKey]);

  const handleSaveAiModel = async (model: string) => {
    try {
      setAiModel(model);
      await invoke("set_ai_model", { model });
    } catch (error: unknown) {
      alertActionError("AI処理用モデル設定失敗", error);
    }
  };

  const showTimedAiTestResult = (result: { success: boolean; message: string } | null) => {
    if (aiTestResultTimerRef.current !== null) {
      window.clearTimeout(aiTestResultTimerRef.current);
      aiTestResultTimerRef.current = null;
    }
    setAiTestResult(result);
    if (!result) return;
    aiTestResultTimerRef.current = window.setTimeout(() => {
      setAiTestResult(null);
      aiTestResultTimerRef.current = null;
    }, 3000);
  };

  const handleTestAiConnection = async () => {
    if (!apiKey) {
      showTimedAiTestResult({ success: false, message: "先にAPIキーを入力してください" });
      return;
    }
    setIsAiTesting(true);
    showTimedAiTestResult(null);
    try {
      const result = await invoke<string>("test_gemini_connection", { apiKey, model: aiModel });
      showTimedAiTestResult({ success: true, message: `接続成功!\n${result}` });
    } catch (error: unknown) {
      showTimedAiTestResult({ success: false, message: `接続失敗: ${getErrorMessage(error)}` });
    } finally {
      setIsAiTesting(false);
    }
  };

  const aiTestButtonStateClass = aiTestResult
    ? aiTestResult.success
      ? "bg-emerald-50 border-emerald-200 text-emerald-700 hover:bg-emerald-50 hover:border-emerald-200 hover:text-emerald-700"
      : "bg-[#bc002d]/10 border-[#bc002d]/35 text-[#bc002d] hover:bg-[#bc002d]/10 hover:border-[#bc002d]/35 hover:text-[#bc002d]"
    : "";
  const aiTestButtonLabel = isAiTesting
    ? "テスト中"
    : aiTestResult
      ? aiTestResult.success
        ? "成功"
        : "失敗"
      : "テスト";

  return (
    <div className="mb-12">
      <h3 className="text-xs font-bold text-gray-400 tracking-wider mb-8 ml-1">Gemini設定</h3>
      <Card>
        <CardItem icon={<Icons.Key />} label="APIキー">
          <div className="w-full max-w-[520px]">
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              onBlur={() => {
                if (apiKeyDebounceTimerRef.current !== null) {
                  window.clearTimeout(apiKeyDebounceTimerRef.current);
                  apiKeyDebounceTimerRef.current = null;
                }
                void saveApiKey(apiKey);
              }}
              placeholder="AIza..."
              className={inputClass}
            />
          </div>
        </CardItem>
        <CardItem icon={<Icons.Brain />} label="AI処理用Geminiモデル">
          <div className="flex flex-col items-end gap-3 w-auto">
            <div className="flex gap-3 w-auto">
              <div className={selectContainerClass}>
                <select value={aiModel} onChange={(e) => handleSaveAiModel(e.target.value)} className={selectClass}>
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
                onClick={handleTestAiConnection}
                disabled={isAiTesting}
                className={`${secondaryButtonClass} ${aiTestButtonStateClass}`.trim()}
                title={aiTestResult?.message || ""}
              >
                {aiTestButtonLabel}
              </button>
            </div>
          </div>
        </CardItem>
      </Card>
    </div>
  );
}
