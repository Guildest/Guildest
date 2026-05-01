"use client";

import { useState, useMemo } from "react";

type Day = { date: string; message_count: number };

const DAY_LABELS = ["Mon", "", "Wed", "", "Fri", "", ""];
const MONTH_NAMES = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const CELL_SIZE = 12;
const CELL_GAP = 3;

function getHeatLevel(count: number, max: number): number {
  if (count <= 0) return 0;
  const ratio = count / max;
  if (ratio > 0.75) return 4;
  if (ratio > 0.5) return 3;
  if (ratio > 0.25) return 2;
  return 1;
}

function cellColorClass(level: number): string {
  const classes: Record<number, string> = {
    0: "bg-cream/[0.04]",
    1: "bg-tan/25",
    2: "bg-tan/50",
    3: "bg-tan/75",
    4: "bg-tan",
  };
  return classes[level] || classes[0];
}

export function Heatmap({ days, servers, members }: { days: Day[]; servers: number; members: number }) {
  const [tooltip, setTooltip] = useState<{ x: number; y: number; day: Day; level: number } | null>(null);

  const base: Day[] =
    days.length > 0
      ? days
      : Array.from({ length: 365 }, (_, i) => ({ date: String(i), message_count: 0 }));

  const cells = [...base];
  while (cells.length % 7 !== 0) cells.push({ date: "", message_count: -1 });

  const max = Math.max(...cells.filter(d => d.message_count >= 0).map((d) => d.message_count), 1);
  const weeks = cells.length / 7;

  const monthLabels = useMemo(() => {
    const labels: { label: string; weekIndex: number }[] = [];
    let lastMonth = -1;
    for (let w = 0; w < weeks; w++) {
      const day = cells[w * 7];
      if (!day?.date) continue;
      const month = new Date(day.date + "T00:00:00Z").getUTCMonth();
      if (!isNaN(month) && month !== lastMonth) {
        labels.push({ label: MONTH_NAMES[month], weekIndex: w });
        lastMonth = month;
      }
    }
    return labels;
  }, [cells, weeks]);

  return (
    <div className="relative flex flex-col">
      <div className="flex gap-2 w-full max-w-full">
        {/* Day labels */}
        <div
          className="flex flex-col shrink-0 justify-end"
          style={{ gap: CELL_GAP, paddingBottom: CELL_GAP }}
        >
          {DAY_LABELS.map((label, i) => (
            <span
              key={i}
              className="text-[11px] text-cream/40 font-medium leading-none flex items-center"
              style={{ height: CELL_SIZE }}
            >
              {label}
            </span>
          ))}
        </div>

        {/* Scrollable grid area */}
        <div
          className="flex-1 overflow-x-auto pb-2 -mb-2 hide-scrollbar"
          style={{ direction: "rtl" }}
        >
          <div className="flex flex-col" style={{ direction: "ltr" }}>
            {/* Month labels */}
            <div className="flex mb-2" style={{ gap: CELL_GAP }}>
              {monthLabels.map((m, i) => {
                const nextWeek = monthLabels[i + 1]?.weekIndex ?? weeks;
                const span = nextWeek - m.weekIndex;
                return (
                  <span
                    key={m.weekIndex}
                    className="text-[11px] text-cream/40 font-medium shrink-0"
                    style={{ width: `calc((100% - ${(weeks - 1) * CELL_GAP}px) / ${weeks} * ${span})` }}
                  >
                    {m.label}
                  </span>
                );
              })}
            </div>

            {/* Grid */}
            <div
              style={{
                display: "grid",
                gridTemplateRows: `repeat(7, ${CELL_SIZE}px)`,
                gridAutoFlow: "column",
                gridAutoColumns: `${CELL_SIZE}px`,
                gap: CELL_GAP,
              }}
            >
              {cells.map((d, i) => {
                const level = d.message_count < 0 ? -1 : getHeatLevel(d.message_count, max);
                return (
                  <div
                    key={i}
                    className={`rounded-[3px] transition-all duration-150 ${
                      level >= 0 ? cellColorClass(level) : "bg-transparent"
                    } ${d.date ? "hover:scale-110 hover:ring-2 hover:ring-tan/30 cursor-pointer" : ""}`}
                    style={{ width: CELL_SIZE, height: CELL_SIZE }}
                    onMouseEnter={(e) => {
                      if (d.date) setTooltip({ x: e.clientX, y: e.clientY, day: d, level });
                    }}
                    onMouseMove={(e) => {
                      if (d.date) setTooltip({ x: e.clientX, y: e.clientY, day: d, level });
                    }}
                    onMouseLeave={() => setTooltip(null)}
                  />
                );
              })}
            </div>
          </div>
        </div>
      </div>

      {/* Legend */}
      <div className="flex items-center gap-2 mt-4 justify-end w-full">
        <span className="text-[11px] text-cream/40 font-medium">Less</span>
        <div className="flex" style={{ gap: CELL_GAP }}>
          {[0, 1, 2, 3, 4].map((level) => (
            <div
              key={level}
              className={`rounded-[3px] ${cellColorClass(level)}`}
              style={{ width: CELL_SIZE, height: CELL_SIZE }}
            />
          ))}
        </div>
        <span className="text-[11px] text-cream/40 font-medium">More</span>
      </div>

      {/* Tooltip */}
      {tooltip && (
        <div
          className="fixed z-50 pointer-events-none px-3 py-2.5 rounded-xl text-sm text-cream bg-surface-light/95 backdrop-blur-sm border border-border-light shadow-xl whitespace-nowrap"
          style={{
            left: tooltip.x + 200 > window.innerWidth ? tooltip.x - 8 : tooltip.x + 8,
            top: tooltip.y - 48,
            transform: tooltip.x + 200 > window.innerWidth ? "translateX(-100%)" : undefined,
          }}
        >
          <div className="flex items-center gap-2">
            <div
              className={`w-2.5 h-2.5 rounded-sm ${cellColorClass(tooltip.level)}`}
            />
            <span className="font-semibold text-tan">{tooltip.day.message_count.toLocaleString()}</span>
            <span className="text-cream/50">messages</span>
          </div>
          <div className="text-cream/30 text-xs mt-1 ml-4.5">
            {tooltip.day.date} · {servers} communities
          </div>
        </div>
      )}
    </div>
  );
}
