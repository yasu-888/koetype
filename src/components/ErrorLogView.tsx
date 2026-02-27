/** @format */

import { useState, useEffect } from "react";
import { WarningIcon, TrashIcon, CircleNotchIcon, CheckIcon, type IconProps } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";

import { ErrorLogEntry, ErrorType } from "../types";
import { getErrorMessage } from "../utils/error";
import { formatDate } from "../utils/format";
import { ConfirmModal } from "./ConfirmModal";

const Icons = {
  Warning: (props: IconProps) => <WarningIcon {...props} />,
  Trash: () => <TrashIcon className="w-4 h-4" weight="bold" />,
  Spinner: () => <CircleNotchIcon className="w-4 h-4 animate-spin" weight="bold" />,
  Check: () => <CheckIcon className="w-4 h-4" weight="bold" />,
};

const errorTypeLabels: Record<ErrorType, string> = {
  ApiError: "API",
  NetworkError: "Network",
  FileError: "File",
  AudioError: "Audio",
  ConfigError: "Config",
  UnknownError: "Unknown",
};

const errorTypeColors: Record<ErrorType, string> = {
  ApiError: "bg-[#bc002d]/15 text-[#bc002d]",
  NetworkError: "bg-orange-100 text-orange-700",
  FileError: "bg-yellow-100 text-yellow-700",
  AudioError: "bg-purple-100 text-purple-700",
  ConfigError: "bg-blue-100 text-blue-700",
  UnknownError: "bg-gray-100 text-gray-700",
};

export function ErrorLogView() {
  const [logs, setLogs] = useState<ErrorLogEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isClearingLogs, setIsClearingLogs] = useState(false);
  const [clearLogsState, setClearLogsState] = useState<"idle" | "success">("idle");
  const [showConfirmModal, setShowConfirmModal] = useState(false);

  useEffect(() => {
    loadErrorLog();
  }, []);

  const loadErrorLog = async () => {
    try {
      const entries = await invoke<ErrorLogEntry[]>("get_error_log");
      setLogs(entries);
    } catch (e) {
      console.error("エラーログ取得失敗:", e);
    } finally {
      setIsLoading(false);
    }
  };

  const handleClearLogs = () => {
    setShowConfirmModal(true);
  };

  const confirmClearLogs = async () => {
    setShowConfirmModal(false);
    setIsClearingLogs(true);
    try {
      await invoke("clear_error_log");
      setLogs([]);
      setClearLogsState("success");
      setTimeout(() => setClearLogsState("idle"), 1200);
    } catch (error: unknown) {
      console.error("エラーログ削除失敗:", getErrorMessage(error));
    } finally {
      setIsClearingLogs(false);
    }
  };

  const buttonClass =
    "bg-transparent hover:bg-[#bc002d]/10 border border-[#bc002d] text-[#bc002d] text-[14px] font-medium px-4 h-10 rounded-xl transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer whitespace-nowrap flex items-center justify-center gap-2";
  const clearButtonClass = `${buttonClass} ${
    clearLogsState === "success"
      ? "!bg-emerald-50 !border-emerald-200 !text-emerald-700 hover:!bg-emerald-100"
      : isClearingLogs
        ? "bg-[#bc002d]/10"
        : ""
  }`.trim();

  return (
    <>
      <header className="mb-12 flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900 tracking-tight">エラーログ</h1>
          <p className="text-sm text-gray-500 mt-2">アプリケーションで発生したエラーの履歴を確認できます。</p>
        </div>
        {logs.length > 0 && (
          <button onClick={handleClearLogs} className={clearButtonClass} disabled={isClearingLogs}>
            {isClearingLogs ? <Icons.Spinner /> : clearLogsState === "success" ? <Icons.Check /> : <Icons.Trash />}
            {isClearingLogs ? "" : clearLogsState === "success" ? "Done" : "すべて削除"}
          </button>
        )}
      </header>

      {isLoading ? (
        <div className="bg-white/80 rounded-3xl border border-gray-200/60 p-20 text-center">
          <p className="text-gray-500">読み込み中...</p>
        </div>
      ) : logs.length === 0 ? (
        <div className="bg-white/80 rounded-3xl border border-gray-200/60 p-20 text-center flex flex-col items-center justify-center">
          <Icons.Warning className="w-16 h-16 text-gray-300" weight="duotone" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2 mt-6">エラーログはありません</h3>
          <p className="text-gray-500">問題が発生した場合、ここにログが記録されます。</p>
        </div>
      ) : (
        <div className="space-y-4">
          {logs.map((log) => (
            <div
              key={log.id}
              className="bg-white/90 rounded-2xl border border-gray-200/70 shadow-[0_12px_30px_rgba(15,23,42,0.06)] p-6"
            >
              <div className="flex items-start justify-between mb-3">
                <div className="flex items-center gap-3">
                  <span className={`text-[11px] font-bold px-2 py-1 rounded ${errorTypeColors[log.error_type]}`}>
                    {errorTypeLabels[log.error_type]}
                  </span>
                  <span className="text-[12px] text-gray-400">{formatDate(log.timestamp, true)}</span>
                </div>
                {log.context && (
                  <span className="text-[11px] text-gray-400 bg-gray-100 px-2 py-1 rounded">{log.context}</span>
                )}
              </div>
              <p className="text-gray-700 text-sm leading-relaxed">{log.message}</p>
            </div>
          ))}
        </div>
      )}

      {showConfirmModal && (
        <ConfirmModal
          title="エラーログをすべて削除しますか？"
          message="削除すると、記録されたすべてのエラーログが失われます。この操作は取り消せません。"
          onConfirm={confirmClearLogs}
          onCancel={() => setShowConfirmModal(false)}
        />
      )}
    </>
  );
}
