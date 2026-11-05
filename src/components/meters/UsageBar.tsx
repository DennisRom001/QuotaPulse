type UsageBarProps = {
  pct: number;
  label?: string;
  showPct?: boolean;
};

const barClass = (pct: number) => {
  if (pct >= 90) return 'bar-red';
  if (pct >= 70) return 'bar-yellow';
  return 'bar-green';
};

const pctColor = (pct: number) => {
  if (pct >= 90) return 'var(--qp-danger)';
  if (pct >= 70) return 'var(--qp-warn)';
  return 'var(--qp-live)';
};

const UsageBar = ({ pct, label, showPct = true }: UsageBarProps) => {
  const clamped = Math.max(0, Math.min(100, pct));
  return (
    <div className="usage-bar">
      {label && <span className="usage-bar-label">{label}</span>}
      <div className="usage-bar-track">
        <div
          className={`usage-bar-fill ${barClass(clamped)}`}
          style={{ width: `${clamped}%` }}
        />
      </div>
      {showPct && (
        <span className="usage-bar-pct" style={{ color: pctColor(clamped) }}>
          {clamped.toFixed(0)}%
        </span>
      )}
    </div>
  );
};

export default UsageBar;
