/** @format */

import { WarningIcon } from "@phosphor-icons/react";

interface ConfirmModalProps {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmModal({
  title,
  message,
  confirmLabel = "削除する",
  cancelLabel = "キャンセル",
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-6">
      <div
        className="absolute inset-0 bg-gray-900/40 backdrop-blur-sm animate-fade-in"
        onClick={onCancel}
      />
      <div className="bg-white rounded-3xl p-8 max-w-md w-full shadow-2xl relative animate-in zoom-in slide-in-from-bottom-4 duration-300">
        <div className="w-16 h-16 flex items-center justify-center mb-6 mx-auto">
          <WarningIcon className="w-8 h-8 text-[#bc002d]" weight="fill" />
        </div>
        <h3 className="text-xl font-bold text-gray-900 text-center mb-3">{title}</h3>
        <p className="text-gray-500 text-center text-sm leading-relaxed mb-8">{message}</p>
        <div className="flex gap-4">
          <button
            onClick={onCancel}
            className="flex-1 px-6 h-12 rounded-xl text-sm font-semibold text-gray-600 hover:bg-gray-50 border border-gray-200 transition-colors cursor-pointer"
          >
            {cancelLabel}
          </button>
          <button
            onClick={onConfirm}
            className="flex-1 px-6 h-12 rounded-xl text-sm font-semibold text-white bg-[#bc002d] hover:bg-[#bc002d] shadow-lg shadow-[#bc002d]/30 transition-all hover:scale-[1.02] active:scale-95 cursor-pointer"
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
