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

/** ローカルタイムゾーンでの YYYY-MM-DD（日次集計 daily_usage のキーと同じ形式） */
export function formatDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function formatCalVer(version: string): string {
  const [yy, m, dd] = version.split(".");
  const year = 2000 + parseInt(yy, 10);
  const month = m.padStart(2, "0");
  return `${year}-${month}-${dd}`;
}
