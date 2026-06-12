/** @format */

import packageJson from "../../package.json";
import { formatCalVer } from "./format";

function resolveRawVersion(): string {
  return typeof packageJson.version === "string" ? packageJson.version : "";
}

export function getDisplayAppVersion(): string {
  const rawVersion = resolveRawVersion();
  if (!rawVersion) {
    return "不明";
  }

  try {
    return formatCalVer(rawVersion);
  } catch {
    return "不明";
  }
}
