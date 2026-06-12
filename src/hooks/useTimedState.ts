/** @format */

import { useCallback, useEffect, useRef, useState } from "react";

/**
 * 値を設定すると一定時間後に自動で null に戻る state。
 * 接続テスト結果などの一時表示メッセージ用。
 * null を設定すると即座にクリアし、タイマーも止める。
 */
export function useTimedState<T>(timeoutMs: number) {
  const [value, setValue] = useState<T | null>(null);
  const timerRef = useRef<number | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const setTimedValue = useCallback(
    (next: T | null) => {
      clearTimer();
      setValue(next);
      if (next === null) return;
      timerRef.current = window.setTimeout(() => {
        setValue(null);
        timerRef.current = null;
      }, timeoutMs);
    },
    [clearTimer, timeoutMs]
  );

  useEffect(() => clearTimer, [clearTimer]);

  return [value, setTimedValue] as const;
}
