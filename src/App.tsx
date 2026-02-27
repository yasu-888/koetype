/** @format */

import { getCurrentWindow } from "@tauri-apps/api/window";
import FloatingStatus from "./FloatingStatus";
import { SettingsShell } from "./settings/SettingsShell";
import "./App.css";

function App() {
  // 現在のウィンドウのラベルを同期取得して、表示するコンポーネントを決定する
  const windowLabel = getCurrentWindow().label;

  if (windowLabel === "settings") {
    return <SettingsShell defaultTab="settings" />;
  } else if (windowLabel === "history") {
    return <SettingsShell defaultTab="history" />;
  } else {
    return <FloatingStatus />;
  }
}

export default App;
