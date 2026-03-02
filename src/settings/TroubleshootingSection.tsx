/** @format */

import { useCallback, useEffect, useState } from "react";
import {
  CheckIcon,
  CircleNotchIcon,
  DownloadSimpleIcon,
  RecordIcon,
  ShieldCheckIcon,
  TestTubeIcon,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { OnboardingStatus, PasteRuntimeDiag, PasteSelfTestResult, PermissionProbeResult } from "../types";
import { getErrorMessage } from "../utils/error";
import { buttonClass } from "../utils/styles";

const Icons = {
  Shield: () => <ShieldCheckIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  Record: () => <RecordIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  Download: () => <DownloadSimpleIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  TestTube: () => <TestTubeIcon className="w-5 h-5 text-gray-400" weight="regular" />,
  Spinner: () => <CircleNotchIcon className="w-4 h-4 animate-spin" weight="bold" />,
  Check: () => <CheckIcon className="w-5 h-5 text-emerald-500" weight="bold" />,
};

export function TroubleshootingSection() {
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [isProbing, setIsProbing] = useState(false);
  const [isForcingKill, setIsForcingKill] = useState(false);
  const [isInstallingWhisper, setIsInstallingWhisper] = useState(false);
  const [isCheckingWhisper, setIsCheckingWhisper] = useState(false);
  const [whisperMessage, setWhisperMessage] = useState<string | null>(null);
  const [whisperMessageSuccess, setWhisperMessageSuccess] = useState(false);
  const [probeMessage, setProbeMessage] = useState<string | null>(null);
  const [probeMessageSuccess, setProbeMessageSuccess] = useState(false);
  const [isRunningPasteDiag, setIsRunningPasteDiag] = useState(false);
  const [pasteDiag, setPasteDiag] = useState<PasteRuntimeDiag | null>(null);
  const [pasteSelfTest, setPasteSelfTest] = useState<PasteSelfTestResult | null>(null);
  const [diagMessage, setDiagMessage] = useState<string | null>(null);
  const [diagSuccess, setDiagSuccess] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<OnboardingStatus>("get_onboarding_status");
      setStatus(next);
    } catch (error: unknown) {
      alert(`状態の取得失敗: ${getErrorMessage(error)}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const alertActionError = useCallback((prefix: string, error: unknown) => {
    alert(`${prefix}: ${getErrorMessage(error)}`);
  }, []);

  const setAck = async () => {
    try {
      const next = await invoke<OnboardingStatus>("set_onboarding_ack_unnotarized", { ack: true });
      setStatus(next);
    } catch (error: unknown) {
      alert(`確認状態の保存失敗: ${getErrorMessage(error)}`);
    }
  };

  const runProbe = async () => {
    setIsProbing(true);
    setProbeMessage(null);
    setProbeMessageSuccess(false);
    try {
      const result = await invoke<PermissionProbeResult>("run_permissions_probe");
      if (result.success) {
        setProbeMessageSuccess(true);
        setProbeMessage("マイク/音声認識の権限確認に成功しました");
      } else {
        setProbeMessage(result.detail);
      }
      await refresh();
    } catch (error: unknown) {
      setProbeMessage(getErrorMessage(error));
    } finally {
      setIsProbing(false);
    }
  };

  const forceKillFloatingOverlay = async () => {
    setIsForcingKill(true);
    try {
      await invoke<string>("force_kill_floating_overlay");
    } catch (error: unknown) {
      alert(`強制終了に失敗: ${getErrorMessage(error)}`);
    } finally {
      setIsForcingKill(false);
    }
  };

  const checkWhisperStatus = async () => {
    setIsCheckingWhisper(true);
    setWhisperMessage(null);
    setWhisperMessageSuccess(false);
    try {
      const next = await invoke<OnboardingStatus>("get_onboarding_status");
      setStatus(next);
      if (next.whisper_ready) {
        const modelName = next.whisper_model_path?.split(/[/\\]/).pop() ?? "検出済み";
        setWhisperMessageSuccess(true);
        setWhisperMessage(`インストール済み: ${modelName} (100%)`);
      } else {
        const cliMissing = !next.whisper_cli_path;
        const modelMissing = !next.whisper_model_path;
        const detectedModel = next.whisper_model_path?.split(/[/\\]/).pop();
        if (cliMissing && modelMissing) {
          setWhisperMessage("Whisper CLI とモデルが見つかりません");
        } else if (cliMissing) {
          setWhisperMessage(
            detectedModel
              ? `Whisper CLI が見つかりません（検出モデル: ${detectedModel}）`
              : "Whisper CLI が見つかりません",
          );
        } else if (modelMissing) {
          setWhisperMessage("Whisperモデルが見つかりません");
        } else {
          setWhisperMessage("Whisperの状態確認に失敗しました");
        }
      }
    } catch (error: unknown) {
      setWhisperMessage(getErrorMessage(error));
    } finally {
      setIsCheckingWhisper(false);
    }
  };

  const installWhisper = async () => {
    setIsInstallingWhisper(true);
    setWhisperMessage(null);
    setWhisperMessageSuccess(false);
    try {
      // インストール開始時に、未導入ガイダンス付きフローティングを一旦閉じる
      await invoke<string>("hide_floating_ui").catch(() => null);
      await invoke<string>("open_terminal_install_whisper");
      setWhisperMessageSuccess(true);
      setWhisperMessage("ターミナルを開きました。進捗はターミナル画面で確認してください。");
    } catch (error: unknown) {
      const message = `Whisperインストール失敗: ${getErrorMessage(error)}`;
      await invoke("log_frontend_event", { message }).catch(() => null);
      setWhisperMessageSuccess(false);
      setWhisperMessage(getErrorMessage(error));
    } finally {
      setIsInstallingWhisper(false);
    }
  };

  const openWhisperManualSetup = async () => {
    try {
      await openUrl(
        "https://github.com/yasu-888/koetype?tab=readme-ov-file#Whisper%E3%81%AE%E3%82%BB%E3%83%83%E3%83%88%E3%82%A2%E3%83%83%E3%83%97",
      );
    } catch (error: unknown) {
      alert(`手動設定ガイドを開けませんでした: ${getErrorMessage(error)}`);
    }
  };

  const runPasteDiagnostics = async () => {
    setIsRunningPasteDiag(true);
    setDiagMessage(null);
    setDiagSuccess(false);
    await new Promise((resolve) => setTimeout(resolve, 50));
    try {
      const diag = await invoke<PasteRuntimeDiag>("diagnose_paste_runtime");
      const selfTest = await invoke<PasteSelfTestResult>("run_paste_self_test");
      setPasteDiag(diag);
      setPasteSelfTest(selfTest);
      setDiagMessage("診断完了");
      setDiagSuccess(true);
    } catch (error: unknown) {
      setDiagMessage(getErrorMessage(error));
      setDiagSuccess(false);
      alertActionError("ペースト診断失敗", error);
    } finally {
      setIsRunningPasteDiag(false);
    }
  };

  useEffect(() => {
    if (!probeMessageSuccess || !probeMessage) return;
    const timerId = window.setTimeout(() => {
      setProbeMessage(null);
      setProbeMessageSuccess(false);
    }, 3000);
    return () => window.clearTimeout(timerId);
  }, [probeMessageSuccess, probeMessage]);

  useEffect(() => {
    if (!diagSuccess || !diagMessage) return;
    const timerId = window.setTimeout(() => {
      setDiagMessage(null);
      setDiagSuccess(false);
    }, 3000);
    return () => window.clearTimeout(timerId);
  }, [diagSuccess, diagMessage]);

  if (loading || !status) {
    return (
      <div className="mb-12">
        <div className="text-sm text-gray-500">権限状態を確認中...</div>
      </div>
    );
  }

  return (
    <div className="mb-12 space-y-4">
      {/* Whisper セットアップ セクション */}
      <div className="rounded-2xl border border-gray-200 bg-white px-7 py-6 shadow-sm">
        <div className="mb-4 flex items-center gap-2">
          <Icons.Download />
          <h3 className="text-sm font-bold tracking-wide text-gray-700">Whisper セットアップ</h3>
        </div>
        <div className="mt-4 flex items-center justify-end gap-3">
          {whisperMessage ? (
            <span className={`text-sm ${whisperMessageSuccess ? "text-emerald-600" : "text-gray-600"}`}>
              {whisperMessageSuccess ? (
                <span className="inline-flex items-center gap-1">
                  <Icons.Check />
                  {whisperMessage}
                </span>
              ) : (
                whisperMessage
              )}
            </span>
          ) : null}
          <button onClick={checkWhisperStatus} disabled={isCheckingWhisper} className={buttonClass}>
            {isCheckingWhisper ? <Icons.Spinner /> : "状態を確認"}
          </button>
          <button onClick={openWhisperManualSetup} className={buttonClass}>
            手動で設定
          </button>
          <button onClick={installWhisper} disabled={isInstallingWhisper} className={buttonClass}>
            {isInstallingWhisper ? <Icons.Spinner /> : "インストール"}
          </button>
        </div>
      </div>

      {/* 権限設定セクション (macOS のみ) */}
      {status.is_macos ? (
        <div className="rounded-2xl border border-gray-200 bg-white px-7 py-6 shadow-sm">
          <div className="mb-4 flex items-center gap-2">
            <Icons.Shield />
            <h3 className="text-sm font-bold tracking-wide text-gray-700">権限設定 (macOS)</h3>
          </div>

          {!status.ack_unnotarized ? (
            <div className="mb-4 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
              この無料配布版は Apple 公証なしで配布されています。初回起動時に警告や許可確認が表示されることがあります。
              <div className="mt-3">
                <button onClick={setAck} className={buttonClass}>
                  了解した
                </button>
              </div>
            </div>
          ) : null}

          <div className="space-y-2 text-sm text-gray-700">
            <div>Whisper CLI: {status.whisper_cli_path ?? "(未検出)"}</div>
            <div>Whisper Model: {status.whisper_model_path ?? "(未検出)"}</div>
            <div>Whisper同梱検出: {status.whisper_bundled_cli && status.whisper_bundled_model ? "ON" : "OFF"}</div>
            <div>アクセシビリティ: {status.ax_trusted ? "許可済み" : "未許可"}</div>
            <div>
              マイク/音声認識:{" "}
              {status.permission_probe_ok === null
                ? "未確認"
                : status.permission_probe_ok
                  ? "許可済み"
                  : "未許可または要再確認"}
            </div>
          </div>

          <div className="mt-4 flex items-center justify-end gap-3">
            {probeMessage ? (
              <span className={`text-sm ${probeMessageSuccess ? "text-emerald-600" : "text-gray-600"}`}>
                {probeMessageSuccess ? (
                  <span className="inline-flex items-center gap-1">
                    <Icons.Check />
                    {probeMessage}
                  </span>
                ) : (
                  probeMessage
                )}
              </span>
            ) : null}
            <button onClick={runProbe} disabled={isProbing} className={buttonClass}>
              {isProbing ? <Icons.Spinner /> : "権限確認"}
            </button>
          </div>
        </div>
      ) : null}

      {/* フローティングUI セクション (macOS のみ) */}
      {status.is_macos ? (
        <div className="rounded-2xl border border-gray-200 bg-white px-7 py-6 shadow-sm">
          <div className="mb-4 flex items-center gap-2">
            <Icons.Record />
            <h3 className="text-sm font-bold tracking-wide text-gray-700">フローティングUI</h3>
          </div>
          <p className="text-xs text-gray-400">フローティングのUIが残ってしまって消えない場合に使用してください。</p>
          <div className="mt-4 flex justify-end">
            <button onClick={forceKillFloatingOverlay} disabled={isForcingKill} className={buttonClass}>
              {isForcingKill ? <Icons.Spinner /> : "強制終了"}
            </button>
          </div>
        </div>
      ) : null}

      <div className="rounded-2xl border border-gray-200 bg-white px-7 py-6 shadow-sm">
        <div className="mb-4 flex items-center gap-2">
          <Icons.TestTube />
          <h3 className="text-sm font-bold tracking-wide text-gray-700">実行環境と権限を検査</h3>
        </div>

        <div className="flex items-center justify-end gap-3">
          {diagMessage ? (
            <span className={`text-sm text-right ${diagSuccess ? "text-emerald-600" : "text-gray-600"}`}>
              {diagSuccess ? (
                <span className="inline-flex items-center gap-1">
                  <Icons.Check />
                  {diagMessage}
                </span>
              ) : (
                diagMessage
              )}
            </span>
          ) : null}
          <button onClick={runPasteDiagnostics} disabled={isRunningPasteDiag} className={buttonClass}>
            {isRunningPasteDiag ? <Icons.Spinner /> : "診断実行"}
          </button>
        </div>

        {!pasteDiag && !pasteSelfTest ? (
          <div className="mt-4 border-t border-gray-100 pt-4 text-sm text-gray-500">
            診断結果はまだありません。必要なときに「診断実行」を押してください。
          </div>
        ) : null}

        {pasteDiag ? (
          <div className="mt-4 border-t border-gray-100 pt-4 text-sm text-gray-700">
            <div>
              実行バイナリ: <span className="font-mono text-[12px]">{pasteDiag.executable_path}</span>
            </div>
            <div>
              Bundle ID: <span className="font-mono text-[12px]">{pasteDiag.bundle_identifier}</span>
            </div>
            <div>
              CDHash: <span className="font-mono text-[12px]">{pasteDiag.codesign_cdhash ?? "(unknown)"}</span>
            </div>
            <div>AX許可: {pasteDiag.ax_trusted ? "ON" : "OFF"}</div>
            <div>
              前面アプリ:{" "}
              <span className="font-mono text-[12px]">{pasteDiag.frontmost_app_bundle_id ?? "(unknown)"}</span>
            </div>
            <div>
              前回ペースト前の前面アプリ:{" "}
              <span className="font-mono text-[12px]">{pasteDiag.frontmost_before_paste ?? "(unknown)"}</span>
            </div>
            <div>
              前回ペースト後の前面アプリ:{" "}
              <span className="font-mono text-[12px]">{pasteDiag.frontmost_after_paste ?? "(unknown)"}</span>
            </div>
            <div>
              前回ペースト経路: <span className="font-mono text-[12px]">{pasteDiag.last_paste_method ?? "(none)"}</span>
            </div>
            <div>
              復元ポリシー: <span className="font-mono text-[12px]">{pasteDiag.last_restore_policy ?? "(unknown)"}</span>
            </div>
            <div>
              入力方式: <span className="font-mono text-[12px]">{pasteDiag.delivery_mode ?? "(unknown)"}</span>
            </div>
            <div>
              スナップショット取得:{" "}
              {pasteDiag.snapshot_taken === null ? "(unknown)" : pasteDiag.snapshot_taken ? "YES" : "NO"}
            </div>
            <div>
              復元実行:{" "}
              {pasteDiag.restore_attempted === null ? "(unknown)" : pasteDiag.restore_attempted ? "YES" : "NO"}
            </div>
            <div>復元成功: {pasteDiag.restore_ok === null ? "(unknown)" : pasteDiag.restore_ok ? "YES" : "NO"}</div>
            <div>
              Transientマーカー付与:{" "}
              {pasteDiag.transient_markers_applied === null
                ? "(unknown)"
                : pasteDiag.transient_markers_applied
                  ? "YES"
                  : "NO"}
            </div>
            <div>
              前回ペースト処理時間:{" "}
              <span className="font-mono text-[12px]">
                {pasteDiag.last_paste_elapsed_ms !== null ? `${pasteDiag.last_paste_elapsed_ms}ms` : "(unknown)"}
              </span>
            </div>
            <div>
              AppleScript status:{" "}
              <span className="font-mono text-[12px]">{pasteDiag.last_applescript_status_code ?? "(none)"}</span>
            </div>
            {pasteDiag.last_applescript_stderr ? (
              <div className="mt-2 text-[#bc002d]">前回 AppleScript stderr: {pasteDiag.last_applescript_stderr}</div>
            ) : null}
            {pasteDiag.last_enigo_error ? (
              <div className="mt-2 text-[#bc002d]">前回 Enigo エラー: {pasteDiag.last_enigo_error}</div>
            ) : null}
            {pasteDiag.type_error ? (
              <div className="mt-2 text-[#bc002d]">前回 Type 入力エラー: {pasteDiag.type_error}</div>
            ) : null}
            <div>
              Enigo フォールバック使用:{" "}
              {pasteDiag.last_used_fallback === null ? "(unknown)" : pasteDiag.last_used_fallback ? "YES" : "NO"}
            </div>
            {pasteDiag.last_paste_error ? (
              <div className="mt-2 text-[#bc002d]">前回ペーストエラー: {pasteDiag.last_paste_error}</div>
            ) : null}
            <div>
              AppleScript疎通: {pasteDiag.automation_probe.ok ? "OK" : "NG"}{" "}
              {pasteDiag.automation_probe.status_code !== null ? `(code=${pasteDiag.automation_probe.status_code})` : ""}
            </div>
            {pasteDiag.automation_probe.stderr ? (
              <div className="mt-2 text-[#bc002d]">AppleScript stderr: {pasteDiag.automation_probe.stderr}</div>
            ) : null}
          </div>
        ) : null}

        {pasteSelfTest ? (
          <div className="mt-4 border-t border-gray-100 pt-4 text-sm text-gray-700">
            <div>
              Self-test 結果: <span className="font-semibold">{pasteSelfTest.final_status}</span>
            </div>
            <div>選択経路: {pasteSelfTest.selected_path}</div>
            <div>
              Enigo probe: {pasteSelfTest.enigo_result.ok ? "OK" : "NG"} ({pasteSelfTest.enigo_result.message})
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}
