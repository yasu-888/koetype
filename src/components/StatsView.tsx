/** @format */

import { useState, useEffect, useCallback } from "react";
import {
  ChartBarIcon,
  CalendarDotsIcon,
  ChatCircleDotsIcon,
  HourglassHighIcon,
  PlayIcon,
  TimerIcon,
  WallIcon,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DailyUsagePoint } from "../types";

const Icons = {
  Stats: () => <ChartBarIcon className="w-5 h-5" weight="regular" />,
  Calendar: () => <CalendarDotsIcon className="w-5 h-5" weight="regular" />,
  Characters: () => <ChatCircleDotsIcon className="w-5 h-5" weight="regular" />,
  TimeSaved: () => <HourglassHighIcon className="w-5 h-5" weight="regular" />,
  Count: () => <WallIcon className="w-5 h-5" weight="regular" />,
  Play: () => <PlayIcon className="w-5 h-5" weight="regular" />,
  Timer: () => <TimerIcon className="w-5 h-5" weight="regular" />,
};

const formatDateKey = (date: Date) => {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

function ContributionGraph({ dailyUsage }: { dailyUsage: DailyUsagePoint[] }) {
  type ContributionCell =
    | { isPlaceholder: true }
    | { isPlaceholder: false; date: Date; count: number; characters: number };

  const [hoveredDay, setHoveredDay] = useState<{ date: Date; count: number; characters: number } | null>(null);
  const [tooltipPos, setTooltipPos] = useState({ x: 0, y: 0 });

  const today = new Date();
  const rows = 7;
  const columns = 53;
  const totalDays = rows * columns;
  const totalCells = rows * columns;

  const dataMap = dailyUsage.reduce(
    (acc, point) => {
      acc[point.date] = { count: point.entries, characters: point.characters };
      return acc;
    },
    {} as Record<string, { count: number; characters: number }>,
  );

  const getDayColor = (count: number) => {
    if (count === 0) return "bg-[#ebedf0]";
    if (count < 10) return "bg-[#9be9a8]";
    if (count < 30) return "bg-[#40c463]";
    if (count < 50) return "bg-[#30a14e]";
    return "bg-[#216e39]";
  };

  const cells: ContributionCell[] = Array.from({ length: totalCells }, () => ({ isPlaceholder: true }));
  for (let daysAgo = 0; daysAgo < totalDays; daysAgo++) {
    const col = Math.floor(daysAgo / rows);
    const visualCol = columns - 1 - col;
    const rowFromBottom = daysAgo % rows;
    const row = rows - 1 - rowFromBottom;
    const gridIndex = row * columns + visualCol;

    const date = new Date(today);
    date.setDate(today.getDate() - daysAgo);
    const stats = dataMap[formatDateKey(date)] || { count: 0, characters: 0 };
    cells[gridIndex] = { isPlaceholder: false, date, ...stats };
  }

  const handleMouseMove = (e: React.MouseEvent, day: { date: Date; count: number; characters: number }) => {
    setHoveredDay(day);
    setTooltipPos({ x: e.clientX, y: e.clientY });
  };

  return (
    <div className="bg-white/90 rounded-2xl border border-gray-200/70 p-5 md:p-6 shadow-[0_12px_30px_rgba(15,23,42,0.06)] relative">
      <div className="pb-1">
        <div className="grid grid-cols-[repeat(53,minmax(0,1fr))] gap-1 w-full">
          {cells.map((day, i) => (
            <div
              key={i}
              onMouseMove={(e) => {
                if (day.isPlaceholder) return;
                handleMouseMove(e, day);
              }}
              onMouseLeave={() => setHoveredDay(null)}
              className={`w-full aspect-square rounded-[3px] transition-colors duration-200 ${
                day.isPlaceholder
                  ? "bg-transparent"
                  : `hover:ring-2 hover:ring-[#30a14e]/50 cursor-pointer ${getDayColor(day.count)}`
              }`}
            />
          ))}
        </div>
      </div>

      {/* Tooltip */}
      {hoveredDay && (
        <div
          className="fixed z-50 pointer-events-none bg-gray-900 text-white text-[11px] font-bold px-3 py-1.5 rounded-lg shadow-xl shadow-black/20 -translate-x-1/2 -translate-y-full mb-2 whitespace-nowrap animate-in fade-in zoom-in duration-200"
          style={{ left: tooltipPos.x, top: tooltipPos.y - 10 }}
        >
          {hoveredDay.date.getFullYear()}/{String(hoveredDay.date.getMonth() + 1).padStart(2, "0")}/
          {String(hoveredDay.date.getDate()).padStart(2, "0")} {hoveredDay.count.toLocaleString()}回{" "}
          {hoveredDay.characters.toLocaleString()}文字
          <div className="absolute left-1/2 -bottom-1 -translate-x-1/2 w-2 h-2 bg-gray-900 rotate-45" />
        </div>
      )}
    </div>
  );
}

function StatCard({
  icon,
  label,
  value,
  subValue,
  colorClass = "text-gray-900",
}: {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  subValue?: string;
  colorClass?: string;
}) {
  return (
    <div className="bg-white/90 rounded-2xl border border-gray-200/70 p-4 md:p-5 shadow-[0_10px_24px_rgba(15,23,42,0.06)] hover:shadow-[0_16px_32px_rgba(15,23,42,0.1)] transition-all duration-300 group">
      <div className="flex items-center gap-2.5 mb-4">
        <div className="text-gray-400/80 shrink-0 group-hover:scale-110 transition-transform duration-300">{icon}</div>
        <span className="text-[11px] font-black text-gray-500 tracking-wide">{label}</span>
      </div>
      <div className={`text-2xl md:text-3xl font-bold tracking-tight mb-1.5 ${colorClass}`}>{value}</div>
      {subValue && <div className="text-[12px] font-medium text-gray-500/80">{subValue}</div>}
    </div>
  );
}

export function StatsView() {
  const [dailyUsage, setDailyUsage] = useState<DailyUsagePoint[]>([]);

  const loadData = useCallback(async () => {
    try {
      const points = await invoke<DailyUsagePoint[]>("get_daily_usage", { days: 371 });
      setDailyUsage(points);
    } catch (e) {
      console.error("日次使用量取得失敗:", e);
    }
  }, []);

  useEffect(() => {
    loadData();
    const unlistenPromise = listen("transcription-completed", () => {
      loadData();
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [loadData]);
  const today = new Date();
  const todayKey = formatDateKey(today);
  const monthPrefix = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}`;
  const todayPoint = dailyUsage.find((point) => point.date === todayKey);
  const monthlyPoints = dailyUsage.filter((point) => point.date.startsWith(monthPrefix));
  const monthlyCharacters = monthlyPoints.reduce((sum, point) => sum + point.characters, 0);
  const monthlyEntries = monthlyPoints.reduce((sum, point) => sum + point.entries, 0);
  const monthlyUsageDays = monthlyPoints.filter((point) => point.entries > 0).length;
  const todayCharacters = todayPoint?.characters ?? 0;
  const todayEntries = todayPoint?.entries ?? 0;
  const todayTimeSavedMinutes = (todayCharacters / 10000) * 60;
  const todayAvgCharsPerEntry = todayEntries > 0 ? todayCharacters / todayEntries : 0;
  const monthlyTimeSavedMinutes = (monthlyCharacters / 10000) * 60;
  const monthlyTotalEntries = monthlyEntries;
  return (
    <div>
      <header className="mb-10">
        <div>
          <h1 className="text-3xl font-bold text-gray-900 tracking-tight">スタッツ</h1>
          <p className="text-sm text-gray-500 mt-2">キーボードで入力しちゃってない...?</p>
        </div>
      </header>

      <div className="mb-8">
        <ContributionGraph dailyUsage={dailyUsage} />
      </div>

      <div className="mb-8">
        <h3 className="text-[11px] font-black text-gray-500 tracking-[0.18em] mb-3 ml-1 opacity-90">本日</h3>
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <StatCard
            icon={<Icons.Characters />}
            label="入力文字"
            value={todayCharacters.toLocaleString()}
            subValue="文字"
            colorClass="text-[#bc002d]"
          />
          <StatCard
            icon={<Icons.TimeSaved />}
            label="節約時間"
            value={`${todayTimeSavedMinutes.toFixed(1)}`}
            subValue="分 (推定)"
            colorClass="text-black"
          />
          <StatCard icon={<Icons.Count />} label="使用回数" value={todayEntries.toLocaleString()} subValue="回" />
          <StatCard
            icon={<Icons.Stats />}
            label="平均入力文字"
            value={todayAvgCharsPerEntry.toFixed(1)}
            subValue="文字/回"
          />
        </div>
      </div>

      <div className="mb-8">
        <h3 className="text-[11px] font-black text-gray-500 tracking-[0.18em] mb-3 ml-1 opacity-90">月間</h3>
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <StatCard
            icon={<Icons.Characters />}
            label="入力文字"
            value={monthlyCharacters.toLocaleString()}
            subValue="文字"
          />
          <StatCard
            icon={<Icons.TimeSaved />}
            label="節約時間"
            value={`${monthlyTimeSavedMinutes.toFixed(1)}`}
            subValue="分 (推定)"
            colorClass="text-black"
          />
          <StatCard
            icon={<Icons.Count />}
            label="使用回数"
            value={monthlyTotalEntries.toLocaleString()}
            subValue="回"
          />
          <StatCard icon={<Icons.Calendar />} label="使用日数" value={`${monthlyUsageDays}`} subValue="日" />
        </div>
      </div>

    </div>
  );
}
