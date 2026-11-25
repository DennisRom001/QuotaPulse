import ProviderLogo from '@/components/providers/ProviderLogo';
import { ProviderId } from '@/types';

type WidgetGaugeProps = {
  provider: ProviderId;
  // Undefined = no real figure: no rings are drawn at all (never a fake 0 %).
  primaryPct?: number;
  secondaryPct?: number;
  // Estimates (Cursor) draw in Estimate Grey, never ink or a status colour.
  estimate?: boolean;
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
  return 'var(--qp-ink)';
};

const Ring = ({ r, stroke, pct, estimate }: { r: number; stroke: number; pct?: number; estimate?: boolean }) => {
  if (pct == null) return null;
  const c = 2 * Math.PI * r;
  const value = Math.max(0, Math.min(100, pct));
  return (
    <>
      <circle cx={CENTER} cy={CENTER} r={r} fill="none" style={{ stroke: 'var(--qp-track)' }} strokeWidth={stroke} />
      {value > 0 && (
        <circle
          cx={CENTER}
          cy={CENTER}
          r={r}
          fill="none"
          style={{ stroke: estimate ? 'var(--qp-estimate)' : arcColor(value), transition: 'stroke-dashoffset .45s ease' }}
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

const WidgetGauge = ({ provider, primaryPct, secondaryPct, estimate }: WidgetGaugeProps) => (
  <div style={{ position: 'relative', width: SIZE, height: SIZE, flexShrink: 0 }}>
    <svg width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`} aria-hidden="true">
      <Ring r={OUTER_R} stroke={OUTER_STROKE} pct={primaryPct} estimate={estimate} />
      <Ring r={INNER_R} stroke={INNER_STROKE} pct={secondaryPct} estimate={estimate} />
      <circle cx={CENTER} cy={CENTER} r={16} style={{ fill: 'var(--qp-surface-2)', stroke: 'var(--qp-border)' }} strokeWidth="1" />
    </svg>
    <div style={{ position: 'absolute', inset: 0, display: 'grid', placeItems: 'center', pointerEvents: 'none' }}>
      <ProviderLogo provider={provider} size={18} variant="widget-core" />
    </div>
  </div>
);

export default WidgetGauge;
