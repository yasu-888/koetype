/** @format */

export const inputClass =
  "w-full bg-white/85 border border-gray-200/70 rounded-xl px-4 h-12 text-[15px] text-gray-900 placeholder-gray-400 focus:outline-none focus:border-gray-300 focus:ring-4 focus:ring-gray-200/70 transition-all duration-200 shadow-sm";

export const buttonClass =
  "bg-gray-900 hover:bg-black active:bg-gray-900 text-white text-[14px] font-medium px-4 h-12 rounded-xl transition-all duration-200 shadow-sm disabled:opacity-50 disabled:cursor-not-allowed hover:shadow-md cursor-pointer whitespace-nowrap flex-shrink-0 w-24 flex items-center justify-center gap-2";

export const secondaryButtonClass =
  "bg-white/80 hover:bg-white border border-gray-200/70 text-gray-700 text-[14px] font-medium px-4 h-12 rounded-xl transition-all duration-200 shadow-sm disabled:opacity-50 whitespace-nowrap hover:border-gray-300 hover:text-gray-900 cursor-pointer flex-shrink-0 w-24 flex items-center justify-center";

export const selectContainerClass = "relative w-full max-w-[320px]";

export const selectIconClass = "pointer-events-none absolute inset-y-0 right-0 flex items-center px-4 text-gray-500";

export const selectClass = `${inputClass} appearance-none pr-12 cursor-pointer bg-white/70 hover:bg-white`;

export const saveFeedbackButtonClass = (state: "idle" | "saving" | "success"): string =>
  state === "saving"
    ? "bg-transparent text-gray-500 h-12 flex-shrink-0 w-24 flex items-center justify-center cursor-not-allowed"
    : `${buttonClass} ${state === "success" ? "bg-emerald-600 hover:bg-emerald-600 text-white shadow-emerald-200" : ""}`.trim();
