/** @format */

import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { MegaphoneSimpleIcon, GlobeIcon, GithubLogoIcon, LinkSimpleIcon, XLogoIcon } from "@phosphor-icons/react";
import { getErrorMessage } from "../utils/error";
import { getDisplayAppVersion } from "../utils/version";
import appIcon from "../../assets/icon.svg";

const CREDIT_LINKS = [
  {
    id: "buymeacoffee",
    label: "KoeTypeを応援する",
    value: "buymeacoffee.com/koetype",
    url: "https://www.buymeacoffee.com/koetype",
    icon: <MegaphoneSimpleIcon className="w-4 h-4" weight="bold" />,
  },
  {
    id: "x",
    label: "Twitter(X)",
    value: "x.com/_yasu888",
    url: "https://x.com/_yasu888",
    icon: <XLogoIcon className="w-4 h-4" weight="bold" />,
  },
  {
    id: "website",
    label: "ポートフォリオ",
    value: "yasu.lifemate.dev",
    url: "https://yasu.lifemate.dev/",
    icon: <GlobeIcon className="w-4 h-4" weight="bold" />,
  },
  {
    id: "github",
    label: "GitHub",
    value: "github.com/yasu-888/koetype",
    url: "https://github.com/yasu-888/koetype",
    icon: <GithubLogoIcon className="w-4 h-4" weight="bold" />,
  },
] as const;

export function CreditsTab() {
  const appVersion = getDisplayAppVersion();

  const handleOpenLink = async (url: string) => {
    try {
      await openUrl(url);
    } catch (error: unknown) {
      alert(`リンクを開けませんでした: ${getErrorMessage(error)}`);
    }
  };

  const checkUpdates = async () => {
    try {
      await invoke("check_for_updates");
    } catch (error: unknown) {
      alert(`更新ページを開けませんでした: ${getErrorMessage(error)}`);
    }
  };

  return (
    <>
      <header className="mb-10">
        <h1 className="text-3xl font-bold text-gray-900 tracking-tight">クレジット</h1>
        <p className="text-sm text-gray-500 mt-2">
          応援いただける方はこちらから寄付をお願いします。今後の開発継続に役立てさせていただきます。
        </p>
      </header>

      <div className="space-y-5">
        <section className="bg-white/90 rounded-2xl border border-gray-200/70 shadow-[0_12px_30px_rgba(15,23,42,0.06)] p-6">
          <div className="flex items-center gap-4">
            <img src={appIcon} alt="KoeType icon" className="w-14 h-14 object-contain" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <p className="text-xl font-semibold text-gray-900 tracking-tight">KoeType</p>
                <p className="text-xs text-gray-400 shrink-0">ver. {appVersion}</p>
                <button
                  type="button"
                  onClick={checkUpdates}
                  className="ml-1 text-xs text-gray-600 border border-gray-200 rounded-lg px-2 py-1 hover:bg-gray-50 cursor-pointer"
                >
                  更新確認
                </button>
              </div>
              <p className="text-sm text-gray-600 mt-1">
                声でタイピングをもっと手軽に。Mac/Win対応の完全無料で使える音声入力デスクトップアプリです。
              </p>
            </div>
          </div>
        </section>

        <section className="bg-white/90 rounded-2xl border border-gray-200/70 shadow-[0_12px_30px_rgba(15,23,42,0.06)] p-6">
          <h2 className="text-sm font-semibold text-gray-700 mb-4">開発者</h2>
          <div className="flex items-center gap-4">
            <img
              src="/credits/yasu.webp"
              alt="Yasu profile"
              className="w-20 h-20 rounded-full object-cover border border-gray-200"
            />
            <div className="min-w-0">
              <p className="text-lg font-semibold text-gray-900">yasu</p>
              <p className="text-sm text-gray-600 mt-1">生活の役に立つアプリやツールを開発・運営しています。</p>
            </div>
          </div>
        </section>

        <section className="bg-white/90 rounded-2xl border border-gray-200/70 shadow-[0_12px_30px_rgba(15,23,42,0.06)] p-6">
          <h2 className="text-sm font-semibold text-gray-700 mb-4">リンク</h2>
          <div className="space-y-3">
            {CREDIT_LINKS.map((link) => (
              <button
                key={link.id}
                type="button"
                onClick={() => handleOpenLink(link.url)}
                className={`w-full border rounded-xl px-4 py-3 text-left transition-colors duration-150 cursor-pointer ${
                  link.id === "buymeacoffee"
                    ? "bg-[#ffdd00]/45 border-[#f1c40f] hover:bg-[#ffdd00]/60"
                    : "bg-white border-gray-200 hover:border-gray-300 hover:bg-gray-50"
                }`}
              >
                <div className="flex items-center gap-3">
                  <span className={link.id === "buymeacoffee" ? "text-gray-900" : "text-gray-600"}>{link.icon}</span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <p className="text-sm font-semibold text-gray-900">{link.label}</p>
                      {link.id === "buymeacoffee" && (
                        <span className="text-[10px] font-bold bg-gray-900 text-white px-2 py-0.5 rounded-full">
                          Support
                        </span>
                      )}
                    </div>
                    <p
                      className={`text-xs break-all ${link.id === "buymeacoffee" ? "text-gray-700" : "text-gray-500"}`}
                    >
                      {link.value}
                    </p>
                  </div>
                  <LinkSimpleIcon
                    className={`w-4 h-4 ${link.id === "buymeacoffee" ? "text-gray-700" : "text-gray-400"}`}
                    weight="bold"
                  />
                </div>
              </button>
            ))}
          </div>
        </section>
      </div>
    </>
  );
}
