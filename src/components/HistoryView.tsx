/** @format */

import { useEffect, useState, useCallback } from "react";
import { flushSync } from "react-dom";
import { ClockCounterClockwiseIcon, CopyIcon, WarningIcon, TrashIcon, type IconProps } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { HistoryItem } from "../types";
import { useTauriListen } from "../hooks/useTauriListen";
import { formatDate, formatDuration } from "../utils/format";
import { ConfirmModal } from "./ConfirmModal";

interface ParsedAiPrompt {
  voiceInstruction: string;
  selectedText: string;
}

const parseAiPrompt = (raw: string | null | undefined): ParsedAiPrompt | null => {
  const text = raw?.trim();
  if (!text || !text.startsWith("[AI_PROMPT]")) return null;
  const marker = "\n[SELECTED_TEXT]\n";
  const idx = text.indexOf(marker);
  if (idx < 0) return null;
  const voiceInstruction = text.slice("[AI_PROMPT]\n".length, idx).trim();
  const selectedText = text.slice(idx + marker.length).trim();
  if (!voiceInstruction || !selectedText) return null;
  return { voiceInstruction, selectedText };
};

const Icons = {
  History: () => <ClockCounterClockwiseIcon className="w-5 h-5" weight="regular" />,
  EmptyHistory: () => <ClockCounterClockwiseIcon className="w-16 h-16 text-gray-300" weight="duotone" />,
  Copy: () => <CopyIcon className="w-4 h-4" weight="bold" />,
  Warning: (props: IconProps) => <WarningIcon {...props} />,
  Trash: () => <TrashIcon className="w-4 h-4" weight="bold" />,
};

export function HistoryView() {
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [isClearingHistory, setIsClearingHistory] = useState(false);
  const [showConfirmModal, setShowConfirmModal] = useState(false);
  const [deleteTargetId, setDeleteTargetId] = useState<number | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [hybridThresholdMs, setHybridThresholdMs] = useState<number>(10_000);

  const loadHistory = useCallback(async () => {
    try {
      const items = await invoke<HistoryItem[]>("get_history");
      setHistory(items);
    } catch (e) {
      console.error("履歴取得失敗:", e);
    }
  }, []);

  useEffect(() => {
    loadHistory();
  }, [loadHistory]);
  useTauriListen("transcription-completed", loadHistory);

  useEffect(() => {
    const loadHybridThreshold = async () => {
      try {
        const thresholdMs = await invoke<number>("get_hybrid_threshold_ms");
        setHybridThresholdMs(thresholdMs);
      } catch (e) {
        console.error("Hybrid閾値取得失敗:", e);
      }
    };
    loadHybridThreshold();
  }, []);

  const handleCopyHistory = async (text: string, id: string) => {
    try {
      await invoke("copy_text", { text });
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch (e) {
      console.error("コピー失敗:", e);
    }
  };

  const handleClearHistory = () => {
    setShowConfirmModal(true);
  };

  const confirmClearHistory = async () => {
    setShowConfirmModal(false);
    flushSync(() => {
      setIsClearingHistory(true);
      setHistory([]);
    });

    try {
      await invoke("clear_history");
      setIsClearingHistory(false);
    } catch (e) {
      console.error("履歴クリア失敗:", e);
      setIsClearingHistory(false);
      // 失敗した場合は再読み込み
      const items = await invoke<HistoryItem[]>("get_history");
      setHistory(items);
    }
  };

  const handleDeleteItem = (id: number) => {
    setDeleteTargetId(id);
  };

  const confirmDeleteItem = async () => {
    if (deleteTargetId === null) return;
    const id = deleteTargetId;
    setDeleteTargetId(null);

    try {
      await invoke("delete_history_item", { id });
      setHistory(history.filter((item) => item.id !== id));
    } catch (e) {
      console.error("削除失敗:", e);
    }
  };

  const deleteButtonClass =
    "bg-transparent hover:bg-[#bc002d]/10 border border-[#bc002d] text-[#bc002d] text-[13px] font-semibold px-4 h-10 rounded-xl transition-all duration-200 cursor-pointer whitespace-nowrap flex-shrink-0 flex items-center justify-center";

  const resolvePrimaryModeLabel = (item: HistoryItem): string => {
    if (parseAiPrompt(item.dictation_text)) {
      return "Response";
    }
    switch (item.stt_provider) {
      case "gemini":
        return "Gemini";
      case "whisper":
        return "Whisper";
      case "collaborate":
        return "Collaborate";
      case "hybrid":
        return item.duration_ms <= hybridThresholdMs ? "Whisper" : "Gemini";
      default:
        return "Gemini";
    }
  };

  const resolveSecondaryBlock = (
    item: HistoryItem
  ): { text: string; label: string; copyId: string; copyText: string; selectedText?: string } | null => {
    const aiPrompt = parseAiPrompt(item.dictation_text);
    if (aiPrompt) {
      return {
        text: aiPrompt.voiceInstruction,
        selectedText: aiPrompt.selectedText,
        label: "Prompt",
        copyId: `secondary-${item.id}`,
        copyText: `${aiPrompt.voiceInstruction}\n---selected_text---\n${aiPrompt.selectedText}\n-------------------`,
      };
    }

    if (item.stt_provider === "collaborate") {
      const whisperText = item.whisper_text?.trim();
      if (whisperText) {
        return {
          text: whisperText,
          label: "Whisper",
          copyId: `secondary-${item.id}`,
          copyText: whisperText,
        };
      }
      return null;
    }

    const osText = item.dictation_text?.trim();
    if (!osText) {
      return null;
    }

    return {
      text: osText,
      label: "OS標準",
      copyId: `secondary-${item.id}`,
      copyText: osText,
    };
  };

  return (
    <>
      <header className="mb-12 flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900 tracking-tight">履歴</h1>
          <p className="text-sm text-gray-500 mt-2">過去30日分が保存されます。</p>
        </div>
        {history.length > 0 && (
          <button onClick={handleClearHistory} disabled={isClearingHistory} className={deleteButtonClass}>
            {isClearingHistory ? "削除中..." : "履歴をすべて消去"}
          </button>
        )}
      </header>

      {history.length === 0 ? (
        <div className="bg-white/80 rounded-3xl border border-gray-200/60 p-20 text-center shadow-sm">
          <div className="mx-auto mb-6 flex items-center justify-center opacity-40">
            <Icons.EmptyHistory />
          </div>
          <h3 className="text-lg font-semibold text-gray-900 mb-2">履歴がありません</h3>
          <p className="text-gray-500 max-w-md mx-auto">音声入力をすると、 ここに文字起こし履歴が保存されます。</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-6">
          {history.map((item) => {
            const primaryText = item.text?.trim();
            const primaryModeLabel = resolvePrimaryModeLabel(item);
            const secondaryBlock = resolveSecondaryBlock(item);

            return (
              <div
                key={item.id}
                className="group bg-white/90 rounded-2xl border border-gray-200/70 p-6 shadow-[0_8px_20px_rgba(15,23,42,0.04)] hover:shadow-[0_12px_24px_rgba(15,23,42,0.08)] transition-all duration-300"
              >
                <div className="flex justify-between items-start mb-4">
                  <div className="flex items-center gap-3">
                    <span className="text-[12px] font-bold text-gray-400 tracking-wider">
                      {formatDate(item.timestamp)}
                    </span>
                  </div>
                  <div className="flex items-center gap-3">
                    <button
                      onClick={() => handleDeleteItem(item.id)}
                      className="p-1 px-2 hover:bg-[#bc002d]/10 text-gray-400 hover:text-[#bc002d] rounded-md transition-colors cursor-pointer group/del"
                      title="削除"
                    >
                      <Icons.Trash />
                    </button>
                    <span className="text-[11px] font-medium bg-gray-100 text-gray-500 px-2 py-1 rounded-md">
                      {formatDuration(item.duration_ms)}
                    </span>
                  </div>
                </div>

                <div className="space-y-3">
                  {primaryText && (
                    <div className="bg-gray-50/50 px-4 py-2 rounded-xl border border-gray-100/50">
                      <div className="h-5 flex justify-between items-center">
                        <span className="text-[10px] font-medium text-gray-400 tracking-widest">
                          {primaryModeLabel}
                        </span>
                        <div className="flex items-center gap-2">
                          {copiedId === `primary-${item.id}` && (
                            <span className="text-[10px] font-bold text-emerald-500 leading-none">コピーしました</span>
                          )}
                          <button
                            onClick={() => handleCopyHistory(primaryText, `primary-${item.id}`)}
                            className="p-px hover:bg-gray-100/70 rounded-lg text-gray-300 hover:text-gray-500 transition-colors cursor-pointer"
                            title="コピー"
                          >
                            <Icons.Copy />
                          </button>
                        </div>
                      </div>
                      <p className="text-gray-800 leading-relaxed text-[14px] line-clamp-5 my-2">{primaryText}</p>
                      <div className="h-5" aria-hidden="true" />
                    </div>
                  )}

                  {secondaryBlock && (
                    <div className="bg-gray-50/30 px-4 py-2 rounded-xl border border-gray-100/50">
                      <div className="h-5 flex justify-between items-center">
                        <span className="text-[10px] font-medium text-gray-400 tracking-widest">
                          {secondaryBlock.label}
                        </span>
                        <div className="flex items-center gap-2">
                          {copiedId === secondaryBlock.copyId && (
                            <span className="text-[10px] font-bold text-emerald-500 leading-none">コピーしました</span>
                          )}
                          <button
                            onClick={() => handleCopyHistory(secondaryBlock.copyText, secondaryBlock.copyId)}
                            className="p-px hover:bg-gray-100/70 rounded-lg text-gray-300 hover:text-gray-500 transition-colors cursor-pointer"
                            title="コピー"
                          >
                            <Icons.Copy />
                          </button>
                        </div>
                      </div>
                      {secondaryBlock.selectedText ? (
                        <div className="my-2 space-y-2">
                          <p className="text-gray-600 leading-relaxed text-[14px] line-clamp-5">
                            {secondaryBlock.text}
                          </p>
                          <div className="flex items-center text-[11px] text-gray-400 font-mono">
                            <span className="h-px bg-gray-300 flex-1" />
                            <span className="px-2">selected_text</span>
                            <span className="h-px bg-gray-300 flex-1" />
                          </div>
                          <p className="text-gray-700 leading-relaxed text-[14px] line-clamp-5">
                            {secondaryBlock.selectedText}
                          </p>
                          <div className="h-px bg-gray-300 w-full" />
                        </div>
                      ) : (
                        <p className="text-gray-600 leading-relaxed text-[14px] line-clamp-5 my-2">
                          {secondaryBlock.text}
                        </p>
                      )}
                      <div className="h-5" aria-hidden="true" />
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {showConfirmModal && (
        <ConfirmModal
          title="履歴をすべて削除しますか？"
          message="削除すると、これまでの文字起こし記録やアクティビティ、スタッツがすべて失われます。この操作は取り消せません。"
          onConfirm={confirmClearHistory}
          onCancel={() => setShowConfirmModal(false)}
        />
      )}

      {deleteTargetId !== null && (
        <ConfirmModal
          title="この項目を削除しますか？"
          message="この操作は取り消せません。"
          onConfirm={confirmDeleteItem}
          onCancel={() => setDeleteTargetId(null)}
        />
      )}
    </>
  );
}
