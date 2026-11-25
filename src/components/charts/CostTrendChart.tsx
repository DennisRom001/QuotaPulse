import { Area, AreaChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';
import { TrendPoint } from '@/types';

type CostTrendChartProps = {
  points: TrendPoint[];
  color: string;
  compact?: boolean;
  days?: number;
};

const CostTrendChart = ({ points, color, compact, days }: CostTrendChartProps) => {
  const height = compact ? 80 : 150;

  return (
    <div style={{ height, padding: compact ? 0 : 8, display: 'flex', flexDirection: 'column', gap: compact ? 0 : 2 }}>
      {days != null && !compact ? (
        <div style={{ paddingRight: 4, paddingBottom: 2, display: 'flex', justifyContent: 'flex-end' }}>
          <span
            style={{
              fontSize: 8,
              fontWeight: 700,
              color: 'var(--text-muted)',
              letterSpacing: '0.03em',
              textTransform: 'uppercase',
            }}
          >
            {days}d
          </span>
        </div>
      ) : null}
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={points} margin={{ top: 4, right: 4, left: compact ? -20 : -16, bottom: 0 }}>
          <XAxis
            dataKey="date"
            tick={{ fill: 'var(--text-muted)', fontSize: compact ? 8 : 10 }}
            tickLine={!compact}
            axisLine={!compact}
          />
          <YAxis
            tick={{ fill: 'var(--text-muted)', fontSize: compact ? 8 : 10 }}
            width={compact ? 30 : 42}
            tickLine={!compact}
            axisLine={!compact}
          />
          <Tooltip
            contentStyle={{
              background: 'var(--qp-surface)',
              border: '1px solid var(--qp-border)',
              color: 'var(--qp-text)',
              borderRadius: 9,
              fontVariantNumeric: 'tabular-nums',
              fontSize: compact ? 10 : 12,
            }}
          />
          {/* Ink line over a flat rail-tinted area: no gradient (DESIGN.md). */}
          <Area
            type="monotone"
            dataKey="cost_usd"
            name="Costo (USD)"
            stroke={color}
            fill="var(--qp-rail)"
            fillOpacity={0.9}
            strokeWidth={compact ? 1.25 : 1.5}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
};

export default CostTrendChart;
