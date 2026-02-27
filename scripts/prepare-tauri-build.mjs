#!/usr/bin/env node

import { spawn } from "node:child_process";

function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
      shell: process.platform === "win32",
    });
    child.on("error", reject);
    child.on("exit", (code, signal) => {
      if (signal) {
        reject(new Error(`${command} ${args.join(" ")} terminated by signal: ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`${command} ${args.join(" ")} failed with exit code ${code}`));
        return;
      }
      resolve();
    });
  });
}

async function main() {
  await run("pnpm", ["build"]);
  await run("pnpm", ["build:sidecar"]);
  if (process.platform === "darwin") {
    await run("pnpm", ["macos:prepare-whisper-bundle"]);
  }
}

main().catch((error) => {
  console.error(`tauri build preparation failed: ${error.message}`);
  process.exit(1);
});
