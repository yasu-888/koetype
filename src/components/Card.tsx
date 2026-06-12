/** @format */

import React from "react";

export function Card({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-white rounded-3xl border border-gray-200/70 shadow-[0_14px_36px_rgba(15,23,42,0.08)] overflow-hidden divide-y divide-gray-100/80">
      {children}
    </div>
  );
}

export function CardItem({
  icon,
  label,
  children,
  className = "",
}: {
  icon: React.ReactNode;
  label: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={`px-8 py-6 flex items-center justify-between gap-8 ${className}`}>
      <div className="flex items-center gap-5 flex-1 min-w-0">
        <div className="text-gray-400/80 shrink-0">{icon}</div>
        <div className="font-medium text-gray-900 text-[16px] tracking-tight truncate">{label}</div>
      </div>
      <div className="flex-shrink-0 flex justify-end items-center">{children}</div>
    </div>
  );
}
