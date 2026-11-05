import ProviderLogo from '@/components/providers/ProviderLogo';
import { ProviderId } from '@/types';

type WidgetGaugeProps = {
  provider: ProviderId;
  primaryPct: number;
  secondaryPct?: number;
};

/* Two thin concentric rings (primary outside, secondary inside) around the
   provider logo. Flat colours from the design tokens: no gradients, caps or
   glow. */
const SIZE = 66;
const CENTER = SIZE / 2;
const OUTER_R = 28;
const INNER_R = 22.5;
const OUTER_STROKE = 3;
const INNER_STROKE = 2.4;

/** Usage colour shared by gauges and percentages (tokens in design-system.css). */
export const arcColor = (pct: number) => {
  if (pct >= 90) return 'var(--qp-danger)';
  if (pct >= 70) return 'var(--qp-warn)';
  return 'var(--qp-live)';
};

const Ring = ({ r, stroke, pct }: { r: number; stroke: number; pct?: number }) => {
  const c = 2 * Math.PI * r;
  const value = Math.max(0, Math.min(100, pct ?? 0));
  return (
    <>
      <circle cx={CENTER} cy={CENTER} r={r} fill="none" style={{ stroke: 'var(--qp-track)' }} strokeWidth={stroke} />
      {pct != null && value > 0 && (
        <circle
          cx={CENTER}
          cy={CENTER}
          r={r}
          fill="none"
          style={{ stroke: arcColor(value), transition: 'stroke-dashoffset .45s ease' }}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c - (value / 100) * c}
          transform={`rotate(-90 ${CENTER} ${CENTER})`}
        />
      )}
    </>
  );
};

const WidgetGauge = ({ provider, primaryPct, secondaryPct }: WidgetGaugeProps) => (
  <div style={{ position: 'relative', width: SIZE, height: SIZE, flexShrink: 0 }}>
    <svg width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`} aria-hidden="true">
      <Ring r={OUTER_R} stroke={OUTER_STROKE} pct={primaryPct} />
      <Ring r={INNER_R} stroke={INNER_STROKE} pct={secondaryPct} />
      <circle cx={CENTER} cy={CENTER} r={16} style={{ fill: 'var(--qp-surface-2)', stroke: 'var(--qp-border)' }} strokeWidth="1" />
    </svg>
    <div style={{ position: 'absolute', inset: 0, display: 'grid', placeItems: 'center', pointerEvents: 'none' }}>
      <ProviderLogo provider={provider} size={18} variant="widget-core" />
    </div>
  </div>
);

export default WidgetGauge;
