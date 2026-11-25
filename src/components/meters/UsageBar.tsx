type UsageBarProps = {
  pct: number;
  label?: string;
  showPct?: boolean;
  // Estimates (Cursor) use Estimate Grey and a "~" prefix (DESIGN.md).
  estimate?: boolean;
};

const barClass = (pct: number) => {
  if (pct >= 90) return 'bar-red';
  if (pct >= 70) return 'bar-yellow';
  return 'bar-green';
};

// Ink below 70 %; ochre / red only once a threshold is crossed.
const pctColor = (pct: number) => {
  if (pct >= 90) return 'var(--qp-danger)';
  if (pct >= 70) return 'var(--qp-warn)';
  return 'var(--qp-ink)';
};

const UsageBar = ({ pct, label, showPct = true, estimate = false }: UsageBarProps) => {
  const clamped = Math.max(0, Math.min(100, pct));
  return (
    <div className="usage-bar">
      {label && <span className="usage-bar-label">{label}</span>}
      <div className="usage-bar-track">
        <div
          className={`usage-bar-fill ${estimate ? '' : barClass(clamped)}`}
          style={{ width: `${clamped}%`, background: estimate ? 'var(--qp-estimate)' : undefined }}
        />
      </div>
      {showPct && (
        <span className="usage-bar-pct" style={{ color: estimate ? 'var(--qp-text)' : pctColor(clamped) }}>
          {estimate ? '~' : ''}{clamped.toFixed(0)}%
        </span>
      )}
    </div>
  );
};

export default UsageBar;
