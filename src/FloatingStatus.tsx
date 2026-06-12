/** @format */

import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  currentMonitor,
  getCurrentWindow,
  LogicalPosition,
  LogicalSize,
  primaryMonitor,
} from "@tauri-apps/api/window";
import { FeedbackMode, FeedbackSettings } from "./types";
import { getPlatform } from "./utils/platform";
import { useTauriListen } from "./hooks/useTauriListen";

// アプリケーションの状態
type AppStatus = "idle" | "recording" | "transcribing" | "injecting" | "done" | "error";

// 音声フィードバックの種類
type SoundType = "start" | "stop" | "success" | "error";
const SPECTRUM_BAR_COUNT = 10;
const AUDIO_GATE = 0.02;
const BASE_SPECTRUM_GAIN = 6.5;
const WHISPER_RUNTIME_MISSING_GUIDANCE_MESSAGE =
  "設定のトラブルシューティングからWhisperをインストールしてください";

function FloatingStatus() {
  const [status, setStatus] = useState<AppStatus>("idle");
  const clickCancelCountRef = useRef(0);
  const clickCancelTimerRef = useRef<number | null>(null);
  const activeSinceRef = useRef<number>(0);
  const idleTimerRef = useRef<number | null>(null);
  const lastAudioLogAtRef = useRef<number>(0);
  const lastAudioEventAtRef = useRef<number>(0);
  const absoluteAudioLevelRef = useRef<number>(0);

  // Settings
  const [soundMode, setSoundMode] = useState<FeedbackMode>("FullscreenOnly");
  const isWindows = getPlatform() === "windows";
  const [audioLevelSmooth, setAudioLevelSmooth] = useState(0);
  const [micSensitivity, setMicSensitivity] = useState(1.0);
  const [waveMotionScale, setWaveMotionScale] = useState(1.0);
  const [wavePhase, setWavePhase] = useState(0);
  const [floatingReady, setFloatingReady] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [stickyError, setStickyError] = useState(false);
  const logFrontend = useCallback(
    async (message: string) => {
      if (!import.meta.env.DEV) return;
      try {
        await invoke("log_frontend_event", { message });
      } catch {
        // IPCが無効な場合は黙って無視する
      }
    },
    []
  );
  const clearText = useCallback(() => {
    setErrorMessage(null);
    setStickyError(false);
  }, []);
  const markActive = useCallback(() => {
    activeSinceRef.current = Date.now();
    absoluteAudioLevelRef.current = 0;
    lastAudioEventAtRef.current = 0;
    if (idleTimerRef.current !== null) {
      window.clearTimeout(idleTimerRef.current);
      idleTimerRef.current = null;
    }
  }, []);
  const resetToIdle = useCallback(() => {
    const minVisibleMs = 900;
    const elapsed = Date.now() - activeSinceRef.current;
    const waitMs = Math.max(0, minVisibleMs - elapsed);
    if (idleTimerRef.current !== null) {
      window.clearTimeout(idleTimerRef.current);
      idleTimerRef.current = null;
    }
    idleTimerRef.current = window.setTimeout(() => {
      setStatus("idle");
      clearText();
      idleTimerRef.current = null;
    }, waitMs);
  }, [clearText]);

  const isActive = isWindows && status !== "idle";
  const shouldShowWindow = isActive && floatingReady;

  useEffect(() => {
    return () => {
      if (idleTimerRef.current !== null) {
        window.clearTimeout(idleTimerRef.current);
      }
      if (clickCancelTimerRef.current !== null) {
        window.clearTimeout(clickCancelTimerRef.current);
      }
    };
  }, []);

  // Load Settings
  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings = await invoke<FeedbackSettings>("get_feedback_settings");
        // setNotificationMode(settings.notification_mode);
        setSoundMode(settings.sound_mode);
      } catch (e) {
        console.error("設定読み込みエラー:", e);
        logFrontend(`設定読み込みエラー: ${String(e)}`);
      }
      try {
        const sensitivity = await invoke<number>("get_mic_sensitivity");
        setMicSensitivity(sensitivity);
      } catch (e) {
        logFrontend(`マイク感度設定取得エラー: ${String(e)}`);
      }
      try {
        const scale = await invoke<number>("get_wave_motion_scale");
        setWaveMotionScale(scale);
      } catch (e) {
        logFrontend(`波の動き設定取得エラー: ${String(e)}`);
      }
    };
    loadSettings();
  }, [logFrontend]);

  useTauriListen<FeedbackSettings>("settings-updated", (event) => {
    setSoundMode(event.payload.sound_mode);
  });
  useTauriListen<{ mic_sensitivity: number; wave_motion_scale: number }>(
    "spectrum-ui-settings-updated",
    (event) => {
      setMicSensitivity(event.payload.mic_sensitivity);
      setWaveMotionScale(event.payload.wave_motion_scale);
    }
  );

  // Sound Helper
  const playSound = useCallback(
    async (type: SoundType) => {
      if (soundMode === "AlwaysOff") return;
      if (soundMode === "FullscreenOnly") {
        try {
          const isOccluded = await invoke<boolean>("is_window_occluded");
          if (!isOccluded) return;
        } catch (e) {
          // ignore
        }
      }

      const audioContext = new (window.AudioContext ||
        (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)();
      const oscillator = audioContext.createOscillator();
      const gainNode = audioContext.createGain();

      oscillator.connect(gainNode);
      gainNode.connect(audioContext.destination);

      const now = audioContext.currentTime;

      switch (type) {
        case "start":
          oscillator.frequency.setValueAtTime(440, now);
          oscillator.frequency.linearRampToValueAtTime(880, now + 0.1);
          gainNode.gain.setValueAtTime(0.1, now);
          gainNode.gain.linearRampToValueAtTime(0, now + 0.1);
          oscillator.start(now);
          oscillator.stop(now + 0.1);
          break;
        case "stop":
          oscillator.frequency.setValueAtTime(880, now);
          oscillator.frequency.linearRampToValueAtTime(440, now + 0.1);
          gainNode.gain.setValueAtTime(0.1, now);
          gainNode.gain.linearRampToValueAtTime(0, now + 0.1);
          oscillator.start(now);
          oscillator.stop(now + 0.1);
          break;
        case "success":
          // Pleasant ding
          oscillator.frequency.setValueAtTime(1000, now);
          gainNode.gain.setValueAtTime(0.1, now);
          gainNode.gain.exponentialRampToValueAtTime(0.001, now + 0.5);
          oscillator.start(now);
          oscillator.stop(now + 0.5);
          break;
        case "error":
          oscillator.type = "sawtooth";
          oscillator.frequency.setValueAtTime(200, now);
          gainNode.gain.setValueAtTime(0.1, now);
          gainNode.gain.linearRampToValueAtTime(0, now + 0.3);
          oscillator.start(now);
          oscillator.stop(now + 0.3);
          break;
      }
    },
    [soundMode]
  );

  // Window sizing/positioning logic based on status
  const isErrorWithMessage = status === "error" && Boolean(errorMessage);
  const activeWindowSize = isErrorWithMessage ? { width: 520, height: 92 } : { width: 190, height: 44 };
  const floatingBottomMargin = 56;
  const floatingTopMargin = 12;

  const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

  // Window resize/reposition when visible
  useEffect(() => {
    setFloatingReady(false);
    if (!isActive) return;
    const resize = async () => {
      const window = getCurrentWindow();
      // 追従は行わず、常にメインモニター基準で配置する
      const monitor = (await primaryMonitor()) ?? (await currentMonitor());
      if (!monitor) {
        console.warn("モニター情報の取得に失敗しました。位置合わせをスキップします。");
        return;
      }
      const scaleFactor = monitor.scaleFactor;
      const workAreaSize = monitor.workArea.size.toLogical(scaleFactor);
      const workAreaPos = monitor.workArea.position.toLogical(scaleFactor);
      const maxHeight = Math.max(0, workAreaSize.height - floatingBottomMargin - floatingTopMargin);
      const targetSize = {
        width: activeWindowSize.width,
        height: Math.min(activeWindowSize.height, maxHeight),
      };
      try {
        await window.setSize(new LogicalSize(targetSize.width, targetSize.height));
        const windowSize = (await window.outerSize()).toLogical(scaleFactor);
        const x = workAreaPos.x + (workAreaSize.width - windowSize.width) / 2.0;
        const y = workAreaPos.y + workAreaSize.height - windowSize.height - floatingBottomMargin;
        let target = new LogicalPosition(x, y);
        await window.setPosition(target);

        // macOS でまれに初回 setPosition が無視されるためリトライしながら収束させる
        for (let i = 0; i < 5; i++) {
          const pos = (await window.outerPosition()).toLogical(scaleFactor);
          const dx = Math.abs(pos.x - x);
          const dy = Math.abs(pos.y - y);
          if (dx < 0.5 && dy < 0.5) break;
          target = new LogicalPosition(x, y);
          await window.setPosition(target);
          await sleep(40);
        }

        // 実際の位置もデバッグ出力（開発時のみ）
        if (import.meta.env.DEV) {
          try {
            const pos = (await window.outerPosition()).toLogical(scaleFactor);
            console.info(
              "[floating] target pos",
              { x, y },
              "actual pos",
              { x: pos.x, y: pos.y },
              "delta",
              { x: pos.x - x, y: pos.y - y },
              "workArea",
              { pos: workAreaPos, size: workAreaSize },
              "winSize",
              windowSize
            );
          } catch {
            /* ignore */
          }
        }
        setFloatingReady(true);
      } catch (error) {
        console.warn("Failed to resize/reposition floating window:", error);
        setFloatingReady(true);
      }
    };
    resize();
  }, [isActive, activeWindowSize.width, activeWindowSize.height]);

  // Window visibility control: hide completely while idle / not needed
  useEffect(() => {
    const applyVisibility = async () => {
      const window = getCurrentWindow();
      try {
        if (shouldShowWindow) {
          await window.show();
        } else {
          await window.hide();
        }
      } catch (e) {
        console.warn("floating window visibility error:", e);
        logFrontend(`floating window visibility error: ${String(e)}`);
      }
    };
    applyVisibility();
  }, [shouldShowWindow, logFrontend]);

  // Event Listeners（useTauriListen が解除と最新ハンドラの参照を保証する）
  const showErrorStatus = () => {
    markActive();
    setStatus("error");
    setStickyError(false);
    setErrorMessage(null);
    playSound("error");
  };

  useTauriListen("recording-starting", () => {
    markActive();
    setStatus("recording");
    clearText();
  });
  useTauriListen("recording-started", () => {
    markActive();
    setStatus("recording");
    clearText();
    playSound("start");
  });
  useTauriListen("recording-stopped", () => {
    markActive();
    setStatus("transcribing");
    clearText();
    playSound("stop");
  });
  useTauriListen("recording-skipped", resetToIdle);
  useTauriListen("partial-transcription", markActive);
  useTauriListen<string>("transcription-completed", () => {
    // AI処理や貼り付け完了までローディングを維持する
  });
  useTauriListen("transcription-cancelled", resetToIdle);
  useTauriListen("paste-completed", () => {
    // 完了表示は不要。サウンドのみ鳴らし、すぐ非表示。
    playSound("success");
    resetToIdle();
  });
  useTauriListen("copy-completed", () => {
    playSound("success");
    resetToIdle();
  });
  useTauriListen("recording-error", showErrorStatus);
  useTauriListen("transcription-failed", showErrorStatus);
  useTauriListen("paste-failed", showErrorStatus);
  useTauriListen<string>("whisper-runtime-missing", (event) => {
    markActive();
    setStatus("error");
    setStickyError(true);
    setErrorMessage(event.payload || WHISPER_RUNTIME_MISSING_GUIDANCE_MESSAGE);
    playSound("error");
  });
  useTauriListen<number>("audio-level", (e) => {
    const raw = Math.max(0, Math.min(1, e.payload));
    lastAudioEventAtRef.current = Date.now();
    // 絶対音量をそのまま表示し、小音量だけ固定ゲートで除外する
    const sensitiveRaw = Math.max(0, Math.min(1, raw * micSensitivity));
    const absoluteLevel = sensitiveRaw <= AUDIO_GATE ? 0 : (sensitiveRaw - AUDIO_GATE) / (1 - AUDIO_GATE);
    absoluteAudioLevelRef.current = Math.max(0, Math.min(1, absoluteLevel));

    // 一時調査ログ: 実音量の絶対値が届いているか低頻度で出力
    const now = Date.now();
    if (now - lastAudioLogAtRef.current > 1200) {
      lastAudioLogAtRef.current = now;
      invoke("log_frontend_event", {
        message: `audio-level raw=${raw.toFixed(4)} sensitive=${sensitiveRaw.toFixed(4)} gate=${AUDIO_GATE.toFixed(4)} absolute=${absoluteAudioLevelRef.current.toFixed(4)}`,
      }).catch(() => {});
    }
  });

  useEffect(() => {
    if (!isActive) {
      setAudioLevelSmooth(0);
      setWavePhase(0);
      absoluteAudioLevelRef.current = 0;
      return;
    }
    let rafId = 0;
    const tick = () => {
      const now = Date.now();
      const phase = now / 1000;
      const eventGapMs = now - lastAudioEventAtRef.current;
      let target = eventGapMs > 220 ? 0 : absoluteAudioLevelRef.current;
      if (status === "transcribing") {
        // 文字起こし中は最小値基準で規則的にループ
        const ambient = 0.055 + 0.035 * (Math.sin(phase * 2.2) * 0.5 + 0.5);
        target = ambient;
      }
      setAudioLevelSmooth((prev) => {
        const follow = target > prev ? 0.58 : 0.24;
        const next = prev + (target - prev) * follow;
        return next < 0.0008 ? 0 : next;
      });
      setWavePhase(phase);
      rafId = window.requestAnimationFrame(tick);
    };
    rafId = window.requestAnimationFrame(tick);
    return () => window.cancelAnimationFrame(rafId);
  }, [isActive, status]);

  const handleFloatingClickCancel = useCallback(() => {
    if (!isActive || status !== "transcribing") return;
    clickCancelCountRef.current += 1;
    if (clickCancelTimerRef.current !== null) {
      window.clearTimeout(clickCancelTimerRef.current);
    }
    clickCancelTimerRef.current = window.setTimeout(() => {
      clickCancelCountRef.current = 0;
      clickCancelTimerRef.current = null;
    }, 1800);
    if (clickCancelCountRef.current >= 3) {
      clickCancelCountRef.current = 0;
      if (clickCancelTimerRef.current !== null) {
        window.clearTimeout(clickCancelTimerRef.current);
        clickCancelTimerRef.current = null;
      }
      invoke("cancel_transcription").catch((e) => {
        logFrontend(`3クリックキャンセル要求失敗: ${String(e)}`);
      });
    }
  }, [isActive, status, logFrontend]);

  useEffect(() => {
    if (status !== "error" || stickyError) return;
    const id = window.setTimeout(() => {
      resetToIdle();
    }, 1200);
    return () => window.clearTimeout(id);
  }, [status, stickyError, resetToIdle]);

  // Render Helpers
  const renderContent = () => {
    if (!isActive) return null;

    const baseLevel = Math.max(0, Math.min(1, audioLevelSmooth));

    return (
      <div
        onClick={handleFloatingClickCancel}
        className={`floating-active-card floating-active-card-spectrum floating-show ${
          status === "error" ? "floating-error-ring" : ""
        }`}
      >
        <div
          className={`floating-active-body floating-active-body-spectrum ${isErrorWithMessage ? "with-message" : ""}`}
        >
          {isErrorWithMessage ? <p className="floating-error-message">{errorMessage}</p> : null}
          <div className="floating-spectrum floating-spectrum-center">
            {Array.from({ length: SPECTRUM_BAR_COUNT }).map((_, i) => {
              const center = (SPECTRUM_BAR_COUNT - 1) / 2;
              const normalizedPos = center <= 0 ? 0 : (i - center) / center; // -1..1
              const wideEnvelope = 0.72 + 0.28 * (1 - Math.abs(normalizedPos) * 0.55);
              let movement = 0;
              let tiltGain = 1;
              if (status === "transcribing") {
                // 文字起こし中は左→右へ流れ続ける波をループさせる
                const loop = ((wavePhase * 0.48) % 1 + 1) % 1;
                const headPos = -1 + loop * 2; // -1 -> +1
                const wrappedDistance = (a: number, b: number) => {
                  const d = Math.abs(a - b);
                  return Math.min(d, 2 - d);
                };
                const lead = Math.exp(-Math.pow(wrappedDistance(normalizedPos, headPos), 2) / (2 * 0.2 * 0.2));
                const trail = Math.exp(
                  -Math.pow(wrappedDistance(normalizedPos, headPos - 0.32), 2) / (2 * 0.34 * 0.34)
                );
                const ripple = Math.sin(wavePhase * 4.2 + i * 0.62) * 0.5 + 0.5;
                movement = 0.08 + lead * 0.72 + trail * 0.35 + ripple * 0.1;
                tiltGain = 1;
              } else {
                // 通常時は方向を持たない自然なランダム揺れ
                const randA = Math.sin(wavePhase * 3.9 + i * 1.07 + Math.sin(i * 2.3) * 0.7);
                const randB = Math.sin(wavePhase * 5.6 + i * 0.43 + Math.cos(i * 1.5) * 0.75);
                const randC = Math.sin(wavePhase * 8.2 + i * 1.83);
                movement = 0.16 + 0.84 * ((randA * 0.5 + 0.5) * 0.46 + (randB * 0.5 + 0.5) * 0.34 + (randC * 0.5 + 0.5) * 0.2);
                const tilt = Math.sin(wavePhase * 1.35);
                tiltGain = 1 + tilt * normalizedPos * 0.45;
              }
              const gain = BASE_SPECTRUM_GAIN * waveMotionScale;
              const transcribingGain = (0.22 + waveMotionScale * 0.16) * 0.6;
              const rawLevel =
                status === "transcribing"
                  ? 0.1 + movement * wideEnvelope * transcribingGain
                  : 0.12 + baseLevel * gain * wideEnvelope * movement * tiltGain;
              const maxLevel = status === "transcribing" ? 2.6 : 4.35;
              const level = Math.max(0.1, Math.min(maxLevel, rawLevel));
              return (
                <span
                  key={`spec-${i}`}
                  className="floating-spectrum-bar"
                  style={{
                    transform: `scaleY(${level})`,
                    opacity: 1,
                  }}
                />
              );
            })}
          </div>
        </div>
      </div>
    );
  };

  if (!isActive) return null;

  return (
    <div className={`floating-shell floating-show ${status === "transcribing" ? "floating-shell-interactive" : ""}`}>
      {renderContent()}
    </div>
  );
}

export default FloatingStatus;
