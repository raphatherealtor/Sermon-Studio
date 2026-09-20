'use client';
import React from 'react';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Cell,
} from 'recharts';

interface SermonsByMonthChartProps {
  data: { month: string; count: number }[];
}

interface TooltipPayloadEntry {
  value: number;
}

function CustomTooltip({
  active,
  payload,
  label,
}: {
  active?: boolean;
  payload?: TooltipPayloadEntry[];
  label?: string;
}) {
  if (!active || !payload?.length) return null;
  return (
    <div className="bg-surface-4 border border-border rounded px-3 py-2 shadow-lg">
      <p className="text-xs font-mono-data text-muted-foreground mb-1">{label}</p>
      <p className="text-sm font-600 text-primary">
        {payload[0].value} sermon{payload[0].value !== 1 ? 's' : ''}
      </p>
    </div>
  );
}

export default function SermonsByMonthChart({ data }: SermonsByMonthChartProps) {
  return (
    <ResponsiveContainer width="100%" height={160}>
      <BarChart data={data} margin={{ top: 4, right: 8, left: -20, bottom: 0 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" vertical={false} />
        <XAxis
          dataKey="month"
          tick={{ fontSize: 10, fill: 'var(--muted-foreground)', fontFamily: 'var(--font-mono)' }}
          axisLine={false}
          tickLine={false}
        />
        <YAxis
          allowDecimals={false}
          tick={{ fontSize: 10, fill: 'var(--muted-foreground)', fontFamily: 'var(--font-mono)' }}
          axisLine={false}
          tickLine={false}
        />
        <Tooltip content={<CustomTooltip />} cursor={{ fill: 'var(--surface-3)' }} />
        <Bar dataKey="count" radius={[3, 3, 0, 0]}>
          {data.map((entry, index) => (
            <Cell
              key={`bar-month-${index}`}
              fill={entry.count === 0 ? 'var(--surface-4)' : 'var(--primary)'}
              opacity={entry.count === 0 ? 0.4 : 0.85}
            />
          ))}
        </Bar>
      </BarChart>
    </ResponsiveContainer>
  );
}