/** @format */

import { useState, useEffect, useRef, useCallback } from "react";
import { AppleLogoIcon, CaretDownIcon, CheckIcon, CircleNotchIcon, CommandIcon, CopyIcon } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { ShortcutSettings } from "../types";
import { getErrorMessage } from "../utils/error";
import { getPlatform } from "../utils/platform";
import { useTauriListen } from "../hooks/useTauriListen";
import { inputClass } from "../utils/styles";
import { buildShortcutFromEvent } from "./shortcuts";
import { Card, CardItem } from "../components/Card";
import { SttProvider } from "../types";

const Icons = {
  Command: () => <CommandIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  Copy: () => <CopyIcon className="w-4 h-4" weight="bold" />,
  OsStandard: () => <AppleLogoIcon className="w-4 h-4 text-gray-400" weight="regular" />,
  Spinner: () => <CircleNotchIcon className="w-4 h-4 animate-spin" weight="bold" />,
  Check: () => <CheckIcon className="w-5 h-5 text-emerald-500" weight="bold" />,
  ChevronDown: () => <CaretDownIcon className="w-4 h-4 text-gray-500" weight="bold" />,
};

const smallButtonClass =
  "px-3 h-10 min-w-[84px] flex-shrink-0 flex items-center justify-center whitespace-nowrap rounded-lg border border-gray-200/80 bg-white hover:bg-gray-50 text-[14px] text-gray-700 transition";
const saveButtonClass =
  "px-3 h-10 min-w-[84px] flex-shrink-0 flex items-center justify-center gap-1 whitespace-nowrap rounded-lg bg-gray-900 text-white hover:bg-black text-[14px] font-medium transition shadow-sm";

const saveFeedbackCompactButtonClass = (state: "idle" | "saving" | "success") =>
  state === "saving"
    ? "bg-transparent text-gray-500 h-10 px-3 min-w-[84px] rounded-lg flex items-center justify-center cursor-not-allowed text-sm flex-shrink-0 whitespace-nowrap transition"
    : `${saveButtonClass} ${state === "success" ? "bg-emerald-600 hover:bg-emerald-600 text-white shadow-emerald-200" : ""}`.trim();

interface ShortcutSectionProps {
  sttProvider: SttProvider;
}

export function ShortcutSection({ sttProvider }: ShortcutSectionProps) {
  const [inputShortcut, setInputShortcut] = useState<string>("");
  const [osPasteShortcut, setOsPasteShortcut] = useState<string>("");
  const [editingTarget, setEditingTarget] = useState<"input" | "os" | null>(null);
  const [saveShortcutsState, setSaveShortcutsState] = useState<"idle" | "saving" | "success">("idle");
  const platform = getPlatform();
  const isMac = platform === "macos";

  const pausedShortcutsRef = useRef(false);
  const inputShortcutRef = useRef<HTMLInputElement | null>(null);
  const osShortcutRef = useRef<HTMLInputElement | null>(null);

  const loadShortcuts = useCallback(async () => {
    try {
      const shortcuts = await invoke<ShortcutSettings>("get_shortcut_settings");
      setInputShortcut(shortcuts.input_shortcut);
      setOsPasteShortcut(shortcuts.os_paste_shortcut);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadShortcuts();
    return () => {
      // 編集中（ショートカット一時停止中）にアンマウントされた場合の復帰保証
      if (pausedShortcutsRef.current) {
        invoke("resume_shortcuts").catch(() => {});
      }
    };
  }, [loadShortcuts]);
  useTauriListen("shortcuts-updated", loadShortcuts);

  const startEditShortcut = async (target: "input" | "os") => {
    try {
      if (!pausedShortcutsRef.current) {
        await invoke("pause_shortcuts");
        pausedShortcutsRef.current = true;
      }
    } catch {
      // ignore
    }
    setEditingTarget(target);
    setTimeout(() => {
      if (target === "input") {
        inputShortcutRef.current?.focus();
        inputShortcutRef.current?.select();
      } else {
        osShortcutRef.current?.focus();
        osShortcutRef.current?.select();
      }
    }, 0);
  };

  const handleSaveShortcuts = async () => {
    setSaveShortcutsState("saving");
    await new Promise((resolve) => setTimeout(resolve, 500));
    try {
      if (!inputShortcut.trim()) {
        setSaveShortcutsState("idle");
        alert("入力ショートカットを指定してください");
        return;
      }
      if (!osPasteShortcut.trim()) {
        setSaveShortcutsState("idle");
        alert("OS標準音声入力ペーストのショートカットを指定してください");
        return;
      }
      const payload: ShortcutSettings = {
        input_shortcut: inputShortcut,
        os_paste_shortcut: osPasteShortcut,
      };
      await invoke("set_shortcut_settings", { shortcuts: payload });
      await invoke("resume_shortcuts");
      pausedShortcutsRef.current = false;
      setSaveShortcutsState("success");
      setTimeout(() => {
        setSaveShortcutsState("idle");
        setEditingTarget(null);
      }, 1200);
    } catch (error: unknown) {
      setSaveShortcutsState("idle");
      alert(`ショートカット設定失敗: ${getErrorMessage(error)}`);
    }
  };

  const shortcutInputProps = (
    setter: React.Dispatch<React.SetStateAction<string>>,
    value: string,
    target: "input" | "os",
  ) => ({
    value,
    readOnly: true,
    onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (editingTarget !== target) return;
      const combo = buildShortcutFromEvent(e, isMac);
      if (combo !== null) setter(combo);
    },
    onFocus: (e: React.FocusEvent<HTMLInputElement>) => {
      if (editingTarget === target) e.target.select();
    },
  });

  return (
    <div className="mb-12">
      <h3 className="text-xs font-bold text-gray-400 tracking-wider mb-8 ml-1">入力とショートカット</h3>
      <Card>
        <CardItem
          icon={<Icons.Command />}
          label="入力ショートカット（長押し録音 / 短押しペースト）"
          className="pr-4"
        >
          <div className="flex flex-col gap-2 max-w-full">
            <div className="inline-flex items-center gap-2 flex-wrap">
              <div className="w-[200px]">
                <input
                  ref={inputShortcutRef}
                  type="text"
                  {...shortcutInputProps(setInputShortcut, inputShortcut, "input")}
                  placeholder={isMac ? "Option+Space" : "Control+Shift+X"}
                  className={`${inputClass} ${editingTarget === "input" ? "ring-2 ring-blue-200" : ""}`}
                />
              </div>
              <button
                onClick={() =>
                  editingTarget === "input" ? handleSaveShortcuts() : startEditShortcut("input")
                }
                disabled={saveShortcutsState === "saving"}
                className={
                  editingTarget === "input"
                    ? saveFeedbackCompactButtonClass(saveShortcutsState)
                    : smallButtonClass
                }
              >
                {editingTarget === "input" && saveShortcutsState === "saving" ? <Icons.Spinner /> : null}
                {editingTarget === "input" && saveShortcutsState === "success" ? <Icons.Check /> : null}
                {editingTarget === "input"
                  ? saveShortcutsState === "saving"
                    ? ""
                    : saveShortcutsState === "success"
                      ? "Saved"
                      : "Save"
                  : "変更"}
              </button>
            </div>
          </div>
        </CardItem>

        {(sttProvider === "collaborate" || isMac) && (
          <CardItem
            icon={sttProvider === "collaborate" ? <Icons.Copy /> : <Icons.OsStandard />}
            label={sttProvider === "collaborate" ? "Whisper文字起こしをペースト" : "OS標準音声入力をペースト"}
            className="pr-4"
          >
            <div className="flex items-center gap-2 w-full">
              <div className="w-[200px]">
                <input
                  ref={osShortcutRef}
                  type="text"
                  {...shortcutInputProps(setOsPasteShortcut, osPasteShortcut, "os")}
                  placeholder={isMac ? "Option+Control+V" : "Control+Super+V"}
                  className={`${inputClass} ${editingTarget === "os" ? "ring-2 ring-blue-200" : ""}`}
                />
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => (editingTarget === "os" ? handleSaveShortcuts() : startEditShortcut("os"))}
                  disabled={saveShortcutsState === "saving"}
                  className={
                    editingTarget === "os"
                      ? saveFeedbackCompactButtonClass(saveShortcutsState)
                      : smallButtonClass
                  }
                >
                  {editingTarget === "os" && saveShortcutsState === "saving" ? <Icons.Spinner /> : null}
                  {editingTarget === "os" && saveShortcutsState === "success" ? <Icons.Check /> : null}
                  {editingTarget === "os"
                    ? saveShortcutsState === "saving"
                      ? ""
                      : saveShortcutsState === "success"
                        ? "Saved"
                        : "Save"
                    : "変更"}
                </button>
              </div>
            </div>
          </CardItem>
        )}
      </Card>
    </div>
  );
}
