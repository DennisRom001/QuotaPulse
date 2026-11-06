import ResetCountdown from '@/components/meters/ResetCountdown';

type UsageMeterProps = {
  utilization: number;
  label: string;
  detail?: string;
  note?: string;
  resetsAt?: string;
  providerTint: 'claude' | 'codex' | 'antigravity' | 'cursor';
  size?: number;
};

const arcColor = (value: number) => {
  if (value >= 90) return 'var(--qp-danger)';
  if (value >= 70) return 'var(--qp-warn)';
  return 'var(--qp-live)';
};

const UsageMeter = ({ utilization, label, detail, note, resetsAt, providerTint, size = 112 }: UsageMeterProps) => {
  const stroke = 5;
  const radius = (size - stroke) / 2;
  const c = 2 * Math.PI * radius;
  const pct = Math.max(0, Math.min(100, utilization));
  const offset = c - (pct / 100) * c;

  return (
    <div className="glass-panel hover-lift" style={{ padding: 10, width: size + 18 }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        <circle cx={size / 2} cy={size / 2} r={radius} fill="none" stroke="var(--meter-track)" strokeWidth={stroke} />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={offset}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
          style={{ stroke: arcColor(pct), transition: 'stroke-dashoffset .45s cubic-bezier(.22,.9,.24,1)' }}
        />
      </svg>
      <div style={{ marginTop: -(size * 0.71), textAlign: 'center' }}>
        <div className="metric-value" style={{ fontSize: 22 }}>{pct.toFixed(0)}%</div>
        <div className="metric-label">{label}</div>
      </div>
      <div
        className="metric-label"
        style={{ marginTop: size * 0.36, textAlign: 'center', minHeight: size < 100 ? 20 : 30, lineHeight: 1.3, fontSize: size < 100 ? 9 : 11 }}
      >
        {detail ?? 'Sin detalle de ventana'}
      </div>
      {note ? (
        <div
          className="metric-label"
          style={{ marginTop: 6, minHeight: 30, textAlign: 'center', lineHeight: 1.25, fontSize: 10 }}
          title={note}
        >
          {note}
        </div>
      ) : null}
      {resetsAt ? (
        <div style={{ marginTop: 8, textAlign: 'center' }} className={`glass-pill glass-${providerTint}`}>
          <ResetCountdown resetsAt={resetsAt} />
        </div>
      ) : null}
    </div>
  );
};

export default UsageMeter;
