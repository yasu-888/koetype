/** @format */

import { listen, type Event } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

/**
 * Tauri イベントを購読し、アンマウント時に確実に解除する hook。
 *
 * handler は ref 経由で常に最新のものを呼ぶため、handler が参照する
 * state/props が変わっても再購読は発生しない（stale closure 対策と
 * 購読のチャタリング防止を両立する）。
 */
export function useTauriListen<T>(eventName: string, handler: (event: Event<T>) => void) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    const unlistenPromise = listen<T>(eventName, (event) => {
      handlerRef.current(event);
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [eventName]);
}
