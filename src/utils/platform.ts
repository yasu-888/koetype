/** @format */

export type Platform = "macos" | "windows" | "";

export function getPlatform(): Platform {
  const navPlatform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ||
    navigator.platform ||
    "";
  const lower = navPlatform.toLowerCase();
  if (lower.includes("mac")) return "macos";
  if (lower.includes("win")) return "windows";
  return "";
}
