/** @format */

import type { KeyboardEvent } from "react";

export const normalizeKey = (key: string): string | null => {
  if (key === "\u00a0") return "Space";
  const upper = key.length === 1 ? key.toUpperCase() : key;
  if (upper === " " || upper === "SPACEBAR" || upper === "SPACE") return "Space";
  if (/^F\\d{1,2}$/i.test(upper)) return upper.toUpperCase();
  switch (upper) {
    case "ENTER":
    case "RETURN":
      return "Enter";
    case "TAB":
      return "Tab";
    case "ESCAPE":
    case "ESC":
      return "Escape";
    case "BACKSPACE":
      return "Backspace";
    case "DELETE":
      return "Delete";
    case "ARROWUP":
      return "ArrowUp";
    case "ARROWDOWN":
      return "ArrowDown";
    case "ARROWLEFT":
      return "ArrowLeft";
    case "ARROWRIGHT":
      return "ArrowRight";
    case "HOME":
      return "Home";
    case "END":
      return "End";
    case "PAGEUP":
      return "PageUp";
    case "PAGEDOWN":
      return "PageDown";
    case "INSERT":
      return "Insert";
    default:
      if (upper.length === 1) return upper;
      return null;
  }
};

export const buildShortcutFromEvent = (event: KeyboardEvent<HTMLInputElement>, isMac: boolean) => {
  event.preventDefault();
  event.stopPropagation();

  // Backspace単独でクリア
  if (
    !event.ctrlKey &&
    !event.metaKey &&
    !event.altKey &&
    !event.shiftKey &&
    (event.key === "Backspace" || event.key === "Delete")
  ) {
    return "";
  }

  const mods: string[] = [];
  if (event.ctrlKey) mods.push("Control");
  if (event.altKey) mods.push(isMac ? "Option" : "Alt");
  if (event.metaKey) mods.push("Super");
  if (event.shiftKey) mods.push("Shift");

  const main = normalizeKey(event.key);
  if (!main || main === "Control" || main === "Shift" || main === "Alt" || main === "Meta") {
    // 単独修飾キーは無視
    return null;
  }

  return [...mods, main].join("+");
};
