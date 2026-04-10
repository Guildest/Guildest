"use client";

import { useState } from "react";

type Day = { date: string; message_count: number };

function cellColor(count: number, max: number) {
  if (count < 0) return "opacity-0";
  if (count === 0) return "bg-white/[0.08]";
  const level = Math.ceil((count / max) * 4);
  if (level >= 4) return "bg-tan";
  if (level === 3) return "bg-tan/70";
  if (level === 2) return "bg-tan/40";
  return "bg-tan/20";
}

export function Heatmap({ days }: { days: Day[] }) {
  const [tooltip, setTooltip] = useState<{ x: number; y: number; day: Day } | null>(null);

  const base: Day[] =
    days.length > 0
      ? days
      : Array.from({ length: 365 }, (_, i) => ({ date: String(i), message_count: 0 }));

  const cells = [...base];
  while (cells.length % 7 !== 0) cells.push({ date: "", message_count: -1 });

  const max = Math.max(...cells.map((d) => d.message_count), 1);

  return (
    <div className="relative">
      <div
        className="w-full"
        style={{
          display: "grid",
          gridTemplateRows: "repeat(7, 10px)",
          gridAutoFlow: "column",
          gridAutoColumns: "1fr",
          gap: "2px",
        }}
      >
        {cells.map((d, i) => (
          <div
            key={i}
            className={`rounded-[2px] cursor-default ${cellColor(d.message_count, max)}`}
            onMouseEnter={(e) => {
              if (d.date) setTooltip({ x: e.clientX, y: e.clientY, day: d });
            }}
            onMouseMove={(e) => {
              if (d.date) setTooltip({ x: e.clientX, y: e.clientY, day: d });
            }}
            onMouseLeave={() => setTooltip(null)}
          />
        ))}
      </div>

      {tooltip && (
        <div
          className="fixed z-50 pointer-events-none px-2.5 py-1.5 rounded-lg text-xs text-cream bg-surface border border-border shadow-lg whitespace-nowrap"
          style={{ left: tooltip.x + 12, top: tooltip.y - 36 }}
        >
          <span className="font-medium">{tooltip.day.message_count.toLocaleString()} messages</span>
          <span className="text-cream/40 ml-1.5">{tooltip.day.date}</span>
        </div>
      )}
    </div>
  );
}
