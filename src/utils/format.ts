/** @format */

export function formatDate(timestamp: number, includeSeconds = false): string {
  return new Date(timestamp * 1000).toLocaleString("ja-JP", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    ...(includeSeconds ? { second: "2-digit" } : {}),
  });
}

export function formatDuration(ms: number): string {
  return `${(ms / 1000).toFixed(1)}s`;
}

export function formatCalVer(version: string): string {
  const [yy, m, dd] = version.split(".");
  const year = 2000 + parseInt(yy, 10);
  const month = m.padStart(2, "0");
  return `${year}-${month}-${dd}`;
}
