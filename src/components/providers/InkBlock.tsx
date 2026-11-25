import { useState } from 'react';
import { useVisibleInterval } from '@/hooks/usePageVisible';
import { ProviderMark } from '@/components/providers/ProviderLogo';
import { formatRemaining } from '@/components/meters/ResetCountdown';
import { ProviderId, UsageSnapshot, UsageWindow } from '@/types';
import { formatWindowPercent } from '@/utils/percent';

/*
 * The ink block (DESIGN.md → The One Block Rule): the single most important
 * real figure on screen — the live quota window closest to its limit.
 *
 * Only live, provider-reported figures qualify. Estimates (Cursor), local-log
 * snapshots and stale caches never reach the block; with nothing live it says
 * so in plain words instead of showing a placeholder number.
 */

const LABELS: Record<ProviderId, string> = {
  claude: 'Claude',
  codex: 'ChatGPT',
  antigravity: 'Gemini',
  cursor: 'Cursor',
};

const WINDOW_NAMES: Record<string, string> = {
  five_hour: 'ventana de 5 h',
  seven_day: 'ventana de 7 días',
  daily: 'cuota diaria',
  monthly: 'cuota mensual',
  weekly: 'cuota semanal',
  session: 'ventana de sesión',
};

type Candidate = { provider: ProviderId; window: UsageWindow };

const isLiveSnapshot = (snapshot?: UsageSnapshot): snapshot is UsageSnapshot =>
  !!snapshot && !snapshot.stale && snapshot.source !== 'local_log' && snapshot.provenance !== 'derived_local';

export const pickHeadline = (
  snapshots: Record<ProviderId, UsageSnapshot | undefined>,
  providers: ProviderId[],
): Candidate | undefined => {
  let best: Candidate | undefined;
  for (const provider of providers) {
    const snapshot = snapshots[provider];
    if (!isLiveSnapshot(snapshot)) continue;
    for (const window of snapshot.windows) {
      if (window.accuracy === 'approximate') continue;
      if (typeof window.utilization !== 'number' || Number.isNaN(window.utilization)) continue;
      if (!best || window.utilization > best.window.utilization) best = { provider, window };
    }
  }
  return best;
};

type InkBlockProps = {
  snapshots: Record<ProviderId, UsageSnapshot | undefined>;
  providers: ProviderId[];
  compact?: boolean;
};

const InkBlock = ({ snapshots, providers, compact = false }: InkBlockProps) => {
  const [now, setNow] = useState(() => Date.now());
  useVisibleInterval(() => setNow(Date.now()), 30_000);

  const headline = pickHeadline(snapshots, providers);

  if (!headline) {
    const withData = providers.filter((id) => snapshots[id]).length;
    return (
      <section className={`qp-ink ${compact ? 'is-compact' : ''}`} aria-label="Cuota más cercana al límite">
        <span className="qp-ink-caption">Más cerca del límite</span>
        <p className="qp-ink-empty">
          Ninguna cuota en vivo ahora
          {withData > 0 ? ` · ${withData} ${withData === 1 ? 'proveedor' : 'proveedores'} con datos locales` : ''}
        </p>
      </section>
    );
  }

  const { provider, window } = headline;
  const pct = Math.max(0, Math.min(100, window.utilization));
  const resetMs = window.resets_at ? Date.parse(window.resets_at) - now : 0;
  const level = pct >= 90 ? 'is-danger' : pct >= 70 ? 'is-warn' : '';

  return (
    <section className={`qp-ink ${level} ${compact ? 'is-compact' : ''}`} aria-label="Cuota más cercana al límite">
      <div className="qp-ink-head">
        <span className="qp-ink-caption">Más cerca del límite</span>
        <span className="qp-ink-live">
          <span className="qp-ink-live-dot" aria-hidden="true" />
          En vivo
        </span>
      </div>
      <div className="qp-ink-body">
        <span className="qp-ink-provider">
          <ProviderMark provider={provider} size={16} />
          <span className="qp-ink-name">{LABELS[provider]}</span>
        </span>
        <span className="qp-ink-figure">{formatWindowPercent(window)}</span>
      </div>
      <div
        className="qp-ink-bar"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(pct)}
        aria-label={`Uso de ${LABELS[provider]}`}
      >
        <span style={{ width: `${pct}%` }} />
      </div>
      <div className="qp-ink-meta">
        <span className="qp-ink-window">{WINDOW_NAMES[window.window_type] ?? 'ventana de uso'}</span>
        {resetMs > 0 && <span className="qp-ink-reset">Se reinicia en {formatRemaining(Math.round(resetMs / 1000))}</span>}
      </div>
    </section>
  );
};

export default InkBlock;
