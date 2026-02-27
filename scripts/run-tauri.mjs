#!/usr/bin/env node

import { spawn } from "node:child_process";

const args = process.argv.slice(2);

if (process.platform === "win32" && process.env.CARGO_HTTP_CHECK_REVOKE == null) {
  process.env.CARGO_HTTP_CHECK_REVOKE = "false";
  console.log(
    "Windows の Cargo TLS 失効確認エラー回避のため CARGO_HTTP_CHECK_REVOKE=false を適用して実行します",
  );
}

const commandLine = ["pnpm", "exec", "tauri", ...args].join(" ");
const child = spawn(commandLine, {
  stdio: "inherit",
  shell: true,
});

child.on("error", (error) => {
  console.error(`エラー: tauri コマンドの起動に失敗しました: ${error.message}`);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    console.error(`エラー: tauri プロセスがシグナル ${signal} で終了しました`);
    process.exit(1);
  }
  process.exit(code ?? 1);
});
