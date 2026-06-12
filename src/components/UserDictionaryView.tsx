/** @format */

import { useState, useEffect } from "react";
import { PlusIcon, XIcon, BookIcon, CircleNotchIcon, CheckIcon } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { DictionaryEntry } from "../types";
import { getErrorMessage } from "../utils/error";

const Icons = {
  Plus: () => <PlusIcon className="w-4 h-4" weight="bold" />,
  Remove: () => <XIcon className="w-4 h-4" weight="bold" />,
  Book: () => <BookIcon className="w-16 h-16 text-gray-300" weight="duotone" />,
  Spinner: () => <CircleNotchIcon className="w-4 h-4 animate-spin" weight="bold" />,
  Check: () => <CheckIcon className="w-4 h-4" weight="bold" />,
};

export function UserDictionaryView() {
  const [entries, setEntries] = useState<DictionaryEntry[]>([]);
  const [newWordsText, setNewWordsText] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const [addWordState, setAddWordState] = useState<"idle" | "saving" | "success">("idle");
  const MAX_ENTRIES = 1000;

  useEffect(() => {
    loadDictionary();
  }, []);

  const loadDictionary = async () => {
    try {
      const dict = await invoke<DictionaryEntry[]>("get_user_dictionary");
      setEntries(dict);
    } catch (e) {
      console.error("辞書取得失敗:", e);
    } finally {
      setIsLoading(false);
    }
  };

  const handleAddWords = async () => {
    const parsedWords = newWordsText
      .split("\n")
      .map((word) => word.trim())
      .filter(Boolean);

    if (parsedWords.length === 0) return;

    const uniqueWords = [...new Set(parsedWords)];
    if (entries.length + uniqueWords.length > MAX_ENTRIES) {
      alert(`最大${MAX_ENTRIES}件までです`);
      return;
    }

    setAddWordState("saving");
    const addedEntries: DictionaryEntry[] = [];
    const failedWords: string[] = [];

    for (const word of uniqueWords) {
      try {
        const entry = await invoke<DictionaryEntry>("add_dictionary_entry", { word });
        addedEntries.push(entry);
      } catch {
        failedWords.push(word);
      }
    }

    if (addedEntries.length > 0) {
      setEntries((prev) => [...addedEntries, ...prev]);
      setAddWordState("success");
      setTimeout(() => {
        setAddWordState("idle");
      }, 900);
    } else {
      setAddWordState("idle");
    }

    if (failedWords.length === 0) {
      setNewWordsText("");
      return;
    }

    setNewWordsText(failedWords.join("\n"));
    alert(`追加失敗: ${failedWords.length}件。未追加の単語を入力欄に残しました。`);
  };

  const handleRemoveWord = async (id: number) => {
    try {
      await invoke("remove_dictionary_entry", { id });
      setEntries((prev) => prev.filter((e) => e.id !== id));
    } catch (error: unknown) {
      alert(`削除失敗: ${getErrorMessage(error)}`);
    }
  };

  const inputClass =
    "w-full bg-white/85 border border-gray-200/70 rounded-xl px-4 py-3 text-[15px] text-gray-900 placeholder-gray-400 focus:outline-none focus:border-gray-300 focus:ring-4 focus:ring-gray-200/70 transition-all duration-200 shadow-sm resize-y min-h-[120px]";

  const buttonClass =
    "bg-gray-900 hover:bg-black active:bg-gray-900 text-white text-[14px] font-medium px-5 h-10 rounded-xl transition-all duration-200 shadow-sm disabled:opacity-50 disabled:cursor-not-allowed hover:shadow-md cursor-pointer whitespace-nowrap flex items-center justify-center gap-2";
  const addWordButtonClass = `${buttonClass} ${
    addWordState === "success"
      ? "bg-emerald-600 hover:bg-emerald-600 shadow-emerald-200"
      : addWordState === "saving"
        ? "bg-gray-800 hover:bg-gray-800"
        : ""
  }`.trim();

  return (
    <>
      <header className="mb-8">
        <h1 className="text-3xl font-bold text-gray-900 tracking-tight">ユーザー辞書</h1>
        <p className="text-sm text-gray-500 mt-2">
          固有名詞や専門用語を登録すると、Gemini文字起こしの精度が向上します。
        </p>
      </header>

      <div className="bg-white/90 rounded-2xl border border-gray-200/70 shadow-[0_12px_30px_rgba(15,23,42,0.06)] p-5 mb-8">
        <textarea
          value={newWordsText}
          onChange={(e) => setNewWordsText(e.target.value)}
          placeholder={"例:\nKoeType\n音声入力\n楽しい"}
          className={inputClass}
        />
        <div className="flex items-center justify-between mt-3 gap-4">
          <p className="text-sm text-gray-600 text-left">1行に1単語。改行しながら複数登録できます。</p>
          <button onClick={handleAddWords} className={addWordButtonClass} disabled={addWordState === "saving"}>
            {addWordState === "saving" ? (
              <Icons.Spinner />
            ) : addWordState === "success" ? (
              <Icons.Check />
            ) : (
              <Icons.Plus />
            )}
            {addWordState === "saving" ? "追加中" : addWordState === "success" ? "追加完了" : "追加"}
          </button>
        </div>
      </div>

      <div className="bg-white/90 rounded-2xl border border-gray-200/70 shadow-[0_12px_30px_rgba(15,23,42,0.06)]">
        <div className="px-8 py-3 flex justify-end border-b border-gray-100/70">
          <p className="text-xs text-gray-500">
            {entries.length}/{MAX_ENTRIES} 件
          </p>
        </div>
        <div className="divide-y divide-gray-100/70">
          {isLoading ? (
            <div className="p-12 text-center text-gray-400">読み込み中...</div>
          ) : entries.length === 0 ? (
            <div className="p-12 text-center text-gray-400 flex flex-col items-center justify-center">
              <Icons.Book />
              <p className="mt-4">登録された単語はありません</p>
            </div>
          ) : (
            entries.map((entry) => (
              <div key={entry.id} className="px-8 py-4 flex items-center justify-between">
                <span className="text-gray-800 text-[15px]">{entry.word}</span>
                <button
                  onClick={() => handleRemoveWord(entry.id)}
                  className="text-gray-400 hover:text-red-700 transition-colors duration-150 p-2 rounded-lg hover:bg-red-50"
                  title="削除"
                >
                  <Icons.Remove />
                </button>
              </div>
            ))
          )}
        </div>
      </div>
    </>
  );
}
