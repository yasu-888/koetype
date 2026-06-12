/** @format */

export interface TestResult {
  success: boolean;
  message: string;
}

/** 接続テストボタンの結果表示用クラス（成功=緑 / 失敗=赤、ホバーでも色を維持） */
export const testButtonStateClass = (result: TestResult | null): string =>
  result
    ? result.success
      ? "bg-emerald-50 border-emerald-200 text-emerald-700 hover:bg-emerald-50 hover:border-emerald-200 hover:text-emerald-700"
      : "bg-[#bc002d]/10 border-[#bc002d]/35 text-[#bc002d] hover:bg-[#bc002d]/10 hover:border-[#bc002d]/35 hover:text-[#bc002d]"
    : "";

export const testButtonLabel = (isTesting: boolean, result: TestResult | null): string =>
  isTesting ? "テスト中" : result ? (result.success ? "成功" : "失敗") : "テスト";
