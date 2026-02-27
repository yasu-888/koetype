#!/usr/bin/env node
/** @format */

import { spawnSync } from "node:child_process";

const requiredMacTools = ["xcrun", "plutil", "codesign"];

function commandExists(command) {
  const result = spawnSync("bash", ["-lc", `command -v ${command}`], {
    stdio: "pipe",
    encoding: "utf8",
  });
  return result.status === 0;
}

function runBashScript(path) {
  const result = spawnSync("bash", [path], { stdio: "inherit" });
  if (result.error) {
    console.error(`エラー: ${path} の実行に失敗しました: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    console.error(`エラー: ${path} が終了コード ${result.status} で失敗しました`);
    process.exit(result.status ?? 1);
  }
}

if (process.platform !== "darwin") {
  console.log(`sidecar ビルドをスキップしました (platform=${process.platform})`);
  process.exit(0);
}

const missingTools = requiredMacTools.filter((tool) => !commandExists(tool));
if (missingTools.length > 0) {
  console.error(`エラー: macOS sidecar ビルドに必要なコマンドが不足しています: ${missingTools.join(", ")}`);
  process.exit(1);
}

runBashScript("scripts/mac/build-sidecars.sh");
