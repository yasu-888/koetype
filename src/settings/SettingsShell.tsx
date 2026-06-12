/** @format */

import { useState, useEffect, useCallback } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useTauriListen } from "../hooks/useTauriListen";
import { MegaphoneSimpleIcon } from "@phosphor-icons/react";
import { SettingsTab } from "./SettingsTab";
import { HistoryView } from "../components/HistoryView";
import { StatsView } from "../components/StatsView";
import { UserDictionaryView } from "../components/UserDictionaryView";
import { ErrorLogView } from "../components/ErrorLogView";
import { TroubleshootingSection } from "./TroubleshootingSection";
import { CreditsTab } from "./CreditsTab";
import { getErrorMessage } from "../utils/error";
import { getDisplayAppVersion } from "../utils/version";

type TabType = "settings" | "history" | "stats" | "dictionary" | "errorlog" | "onboarding" | "credits";
const FIRST_OPEN_ONBOARDING_KEY = "koetype_settings_first_opened";
const VALID_TABS: TabType[] = ["history", "stats", "settings", "dictionary", "errorlog", "onboarding", "credits"];

const NAV_ITEMS: { tab: TabType; label: string }[] = [
  { tab: "settings", label: "設定" },
  { tab: "history", label: "履歴" },
  { tab: "stats", label: "スタッツ" },
  { tab: "dictionary", label: "ユーザー辞書" },
  { tab: "errorlog", label: "エラーログ" },
  { tab: "onboarding", label: "トラブルシューティング" },
  { tab: "credits", label: "クレジット" },
];

export function SettingsShell({ defaultTab = "settings" }: { defaultTab?: TabType }) {
  const [activeTab, setActiveTab] = useState<TabType>(defaultTab);
  const appVersion = getDisplayAppVersion();

  const switchTab = useCallback((tab: TabType) => {
    setActiveTab((prev) => (prev === tab ? prev : tab));
    if (window.location.hash !== `#${tab}`) {
      window.history.replaceState(null, "", `#${tab}`);
    }
  }, []);

  useEffect(() => {
    const hash = window.location.hash.replace("#", "");
    const hashTab = VALID_TABS.includes(hash as TabType) ? (hash as TabType) : null;

    let initialTab: TabType = hashTab ?? defaultTab;
    if (!hashTab && defaultTab === "settings") {
      const opened = window.localStorage.getItem(FIRST_OPEN_ONBOARDING_KEY);
      if (!opened) {
        window.localStorage.setItem(FIRST_OPEN_ONBOARDING_KEY, "1");
        initialTab = "onboarding";
      }
    }

    switchTab(initialTab);
  }, [defaultTab, switchTab]);
  useTauriListen<TabType>("switch-tab", (event) => {
    switchTab(event.payload);
  });

  const handleOpenSupportLink = useCallback(async () => {
    try {
      await openUrl("https://www.buymeacoffee.com/koetype");
    } catch (error: unknown) {
      alert(`リンクを開けませんでした: ${getErrorMessage(error)}`);
    }
  }, []);

  return (
    <div className="flex h-screen bg-gray-50 font-sans text-gray-900 overflow-hidden">
      <aside className="w-64 bg-white border-r border-gray-200 flex flex-col flex-shrink-0 px-4">
        <div className="pl-4 pt-10 pb-8">
          <div className="flex items-center gap-3 font-bold text-[22px] tracking-tight text-gray-900">
            <img src="/icon.svg" className="w-9 h-9 object-contain" alt="KoeType Logo" />
            <span>KoeType</span>
          </div>
          <div className="text-[11px] text-gray-400 font-medium ml-12 mt-1">
            {appVersion ? `ver. ${appVersion}` : ""}
          </div>
        </div>
        <nav className="flex-1 space-y-2 px-2 pb-6">
          {NAV_ITEMS.map(({ tab, label }) => (
            <button
              key={tab}
              onClick={() => switchTab(tab)}
              className={`w-full flex items-center justify-start text-left pl-5 pr-4 h-12 rounded-xl text-[15px] font-semibold transition-colors duration-150 focus:outline-none ${
                activeTab === tab ? "bg-gray-100 text-gray-900" : "text-gray-500 hover:bg-gray-50 hover:text-gray-900"
              }`}
            >
              {label}
            </button>
          ))}
        </nav>
        <div className="px-2 pb-5">
          <button
            type="button"
            onClick={handleOpenSupportLink}
            className="w-full h-9 rounded-lg border border-[#f0d76b] bg-transparent text-[12px] font-medium text-gray-500 hover:text-gray-700 transition-colors duration-150 flex items-center justify-center gap-1.5 cursor-pointer"
          >
            <MegaphoneSimpleIcon className="w-3.5 h-3.5" weight="bold" />
            <span>KoeTypeを応援する</span>
          </button>
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto bg-gray-50">
        <div className="max-w-4xl mx-auto px-8 py-12">
          {activeTab === "settings" ? (
            <SettingsTab />
          ) : activeTab === "history" ? (
            <HistoryView />
          ) : activeTab === "stats" ? (
            <StatsView />
          ) : activeTab === "dictionary" ? (
            <UserDictionaryView />
          ) : activeTab === "onboarding" ? (
            <>
              <header className="mb-10">
                <h1 className="text-3xl font-bold text-gray-900 tracking-tight">トラブルシューティング</h1>
                <p className="text-sm text-gray-500 mt-2">権限状態の確認とペースト診断を実行できます。</p>
              </header>
              <TroubleshootingSection />
            </>
          ) : activeTab === "credits" ? (
            <CreditsTab />
          ) : (
            <ErrorLogView />
          )}
        </div>
      </main>
    </div>
  );
}
