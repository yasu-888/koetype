/** @format */

import { useState, useEffect, useRef, useCallback } from "react";
import {
  CaretDownIcon,
  ClipboardTextIcon,
  GlobeIcon,
  MicrophoneStageIcon,
  SpeakerHighIcon,
  TimerIcon,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { AudioDevice, FeedbackMode, InputDeliveryMode } from "../types";
import { getErrorMessage } from "../utils/error";
import { getPlatform } from "../utils/platform";
import { inputClass, selectClass, selectContainerClass, selectIconClass } from "../utils/styles";
import { FEEDBACK_OPTIONS, LANGUAGE_OPTIONS } from "./options";
import { Card, CardItem } from "../components/Card";

const AUTO_SAVE_DEBOUNCE_MS = 700;
const DEFAULT_MIN_INPUT_DURATION_MS = 2000;

const parseDurationMsFromSecondsInput = (value: string): number | null => {
  const seconds = Number(value);
  if (!Number.isFinite(seconds) || seconds < 0) return null;
  return Math.round(seconds * 1000);
};

const Icons = {
  ChevronDown: () => <CaretDownIcon className="w-4 h-4 text-gray-500" weight="bold" />,
  ClipboardText: () => <ClipboardTextIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  Globe: () => <GlobeIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  Mic: () => <MicrophoneStageIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  Timer: () => <TimerIcon className="w-5 h-5" weight="regular" />,
  Volume: () => <SpeakerHighIcon className="w-5 h-5 text-gray-400" weight="regular" />,
};

export function OptionsSection() {
  const [soundMode, setSoundMode] = useState<FeedbackMode>("FullscreenOnly");
  const [inputDeliveryMode, setInputDeliveryMode] = useState<InputDeliveryMode>("type");
  const [audioDevices, setAudioDevices] = useState<AudioDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);
  const [language, setLanguage] = useState<string>("ja");
  const [minInputSeconds, setMinInputSeconds] = useState<string>("2");
  const [minInputError, setMinInputError] = useState<string | null>(null);

  const platform = getPlatform();
  const isWindows = platform === "windows";

  const settingsReadyRef = useRef(false);
  const lastSavedMinInputDurationRef = useRef<number>(DEFAULT_MIN_INPUT_DURATION_MS);
  const minInputDebounceTimerRef = useRef<number | null>(null);

  const alertActionError = useCallback((prefix: string, error: unknown) => {
    alert(`${prefix}: ${getErrorMessage(error)}`);
  }, []);

  useEffect(() => {
    const load = async () => {
      settingsReadyRef.current = false;
      try {
        const settings = await invoke<{ sound_mode: FeedbackMode }>("get_feedback_settings");
        setSoundMode(settings.sound_mode);
      } catch {
        // ignore
      }
      try {
        const deliveryMode = await invoke<InputDeliveryMode>("get_input_delivery_mode");
        setInputDeliveryMode(deliveryMode);
      } catch {
        // ignore
      }
      try {
        const devices = await invoke<AudioDevice[]>("list_audio_devices");
        setAudioDevices(devices);
        const selected = await invoke<string | null>("get_selected_audio_device");
        setSelectedDevice(selected);
      } catch {
        // ignore
      }
      try {
        const lang = await invoke<string>("get_language");
        setLanguage(lang);
      } catch {
        // ignore
      }
      try {
        const durationMs = await invoke<number>("get_min_input_duration_ms");
        const seconds = durationMs / 1000;
        setMinInputSeconds(Number.isInteger(seconds) ? String(seconds) : seconds.toFixed(1));
        setMinInputError(null);
        lastSavedMinInputDurationRef.current = durationMs;
      } catch {
        lastSavedMinInputDurationRef.current = DEFAULT_MIN_INPUT_DURATION_MS;
      }
      settingsReadyRef.current = true;
    };
    load();
    return () => {
      if (minInputDebounceTimerRef.current !== null) window.clearTimeout(minInputDebounceTimerRef.current);
    };
  }, []);

  const saveMinInputSeconds = useCallback(
    async (nextSecondsInput: string) => {
      if (!settingsReadyRef.current) return;
      const durationMs = parseDurationMsFromSecondsInput(nextSecondsInput);
      if (durationMs === null) {
        setMinInputError("0以上の数値を入力してください");
        return;
      }
      setMinInputError(null);
      if (durationMs === lastSavedMinInputDurationRef.current) return;
      try {
        await invoke("set_min_input_duration_ms", { durationMs });
        lastSavedMinInputDurationRef.current = durationMs;
      } catch (error: unknown) {
        alertActionError("設定失敗", error);
      }
    },
    [alertActionError],
  );

  useEffect(() => {
    if (!settingsReadyRef.current) return;
    if (minInputDebounceTimerRef.current !== null) window.clearTimeout(minInputDebounceTimerRef.current);
    minInputDebounceTimerRef.current = window.setTimeout(() => {
      minInputDebounceTimerRef.current = null;
      void saveMinInputSeconds(minInputSeconds);
    }, AUTO_SAVE_DEBOUNCE_MS);
    return () => {
      if (minInputDebounceTimerRef.current !== null) {
        window.clearTimeout(minInputDebounceTimerRef.current);
        minInputDebounceTimerRef.current = null;
      }
    };
  }, [minInputSeconds, saveMinInputSeconds]);

  const handleSaveSoundMode = async (mode: FeedbackMode) => {
    try {
      setSoundMode(mode);
      await invoke("set_sound_setting", { mode });
    } catch (error: unknown) {
      alertActionError("設定失敗", error);
    }
  };

  const handleSaveInputDeliveryMode = async (mode: InputDeliveryMode) => {
    try {
      await invoke("set_input_delivery_mode", { mode });
      setInputDeliveryMode(mode);
    } catch (error: unknown) {
      alert(`入力方式の保存に失敗: ${getErrorMessage(error)}`);
    }
  };

  const handleDeviceSelect = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    const value = e.target.value;
    const device = value === "default" ? null : value;
    setSelectedDevice(device);
    try {
      await invoke("set_audio_device", { deviceName: device });
    } catch (error: unknown) {
      alertActionError("マイク設定失敗", error);
    }
  };

  const handleLanguageSelect = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    const value = e.target.value;
    setLanguage(value);
    try {
      await invoke("set_language", { lang: value });
    } catch (error: unknown) {
      alertActionError("言語設定失敗", error);
    }
  };

  return (
    <div className="mb-12">
      <h3 className="text-xs font-bold text-gray-400 tracking-wider mb-8 ml-1">オプション設定</h3>
      <Card>
        <CardItem icon={<Icons.Timer />} label="最短入力時間">
          <div className="flex flex-col items-end gap-2">
            <div className="flex gap-3 items-center">
              <div className="w-full max-w-[160px]">
                <input
                  type="number"
                  min={0}
                  step={0.1}
                  value={minInputSeconds}
                  onChange={(e) => {
                    setMinInputSeconds(e.target.value);
                    if (minInputError) setMinInputError(null);
                  }}
                  onBlur={() => {
                    if (minInputDebounceTimerRef.current !== null) {
                      window.clearTimeout(minInputDebounceTimerRef.current);
                      minInputDebounceTimerRef.current = null;
                    }
                    void saveMinInputSeconds(minInputSeconds);
                  }}
                  className={inputClass}
                  aria-invalid={minInputError ? "true" : undefined}
                />
              </div>
              <span className="text-sm text-gray-500">秒</span>
            </div>
            {minInputError ? <div className="text-xs text-[#bc002d]">{minInputError}</div> : null}
          </div>
        </CardItem>

        <CardItem icon={<Icons.Volume />} label="効果音">
          <div className={selectContainerClass}>
            <select
              value={soundMode}
              onChange={(e) => handleSaveSoundMode(e.target.value as FeedbackMode)}
              className={selectClass}
            >
              {FEEDBACK_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
            <div className={selectIconClass}>
              <Icons.ChevronDown />
            </div>
          </div>
        </CardItem>

        {!isWindows ? (
          <CardItem icon={<Icons.ClipboardText />} label="自動入力方式">
            <div className={selectContainerClass}>
              <select
                value={inputDeliveryMode}
                onChange={(e) => handleSaveInputDeliveryMode(e.target.value as InputDeliveryMode)}
                className={selectClass}
              >
                <option value="clipboard">クリップボード経由（高速）</option>
                <option value="type">キー注入（履歴汚染を回避）</option>
              </select>
              <div className={selectIconClass}>
                <Icons.ChevronDown />
              </div>
            </div>
          </CardItem>
        ) : null}

        <CardItem icon={<Icons.Mic />} label="マイク">
          <div className={selectContainerClass}>
            <select value={selectedDevice || "default"} onChange={handleDeviceSelect} className={selectClass}>
              <option value="default">システムデフォルト</option>
              {audioDevices.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name} {d.is_default ? "(Default)" : ""}
                </option>
              ))}
            </select>
            <div className={selectIconClass}>
              <Icons.ChevronDown />
            </div>
          </div>
        </CardItem>

        <CardItem icon={<Icons.Globe />} label="言語">
          <div className={selectContainerClass}>
            <select value={language} onChange={handleLanguageSelect} className={selectClass}>
              {LANGUAGE_OPTIONS.map((l) => (
                <option key={l.value} value={l.value}>
                  {l.label}
                </option>
              ))}
            </select>
            <div className={selectIconClass}>
              <Icons.ChevronDown />
            </div>
          </div>
        </CardItem>
      </Card>
    </div>
  );
}
