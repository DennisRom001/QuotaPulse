import { useEffect, useState } from 'react';
import ProviderLogo from '@/components/providers/ProviderLogo';
import { formatRemaining } from '@/components/meters/ResetCountdown';
import { forecastFor, isLimitImminent } from '@/components/meters/LimitEta';
import {
  ModelBreakdownEntry,
  ProviderId,
  ProviderStatus,
  TrendData,
  UsageAlert,
  UsageSnapshot,
  UsageWindow,
} from '@/types';
import { getProviderAccessState } from '@/utils/providerAccess';
import { displayWindows } from '@/utils/usageWindows';
import { useUsageStore } from '@/stores/usageStore';
import { formatWindowPercent } from '@/utils/percent';
import { useSettingsStore } from '@/stores/settingsStore';
import { SOURCE_COLORS, selectedSource, switchableSources } from '@/utils/sources';

const providerMeta: Record<ProviderId, { label: string; tint: 'claude' | 'codex' | 'antigravity' | 'cursor'; account: string }> = {
  claude: { label: 'Claude', tint: 'claude', account: 'Claude Code' },
  codex: { label: 'ChatGPT', tint: 'codex', account: 'Codex CLI' },
  antigravity: { label: 'Gemini', tint: 'antigravity', account: 'Gemini' },
  cursor: { label: 'Cursor', tint: 'cursor', account: 'Cursor' },
};

const WINDOW_NAMES: Record<string, string> = {
  five_hour: 'Ventana de 5 h',
  seven_day: 'Ventana de 7 días',
  daily: 'Cuota diaria',
  monthly: 'Cuota mensual',
  weekly: 'Cuota semanal',
  session: 'Ventana de sesión',
};

const WINDOW_SHORT: Record<string, string> = {
  five_hour: '5h',
  seven_day: '7d',
  daily: 'Día',
  monthly: 'Mes',
  weekly: 'Sem.',
  session: 'Sesión',
};

const clampPct = (value?: number) => Math.max(0, Math.min(100, value ?? 0));

const compact = (n: number) => {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1).replace(/\.0$/, '')}K`;
  return String(n);
};

/** "15.4M / 20M tokens" when the window reports absolute numbers, else its name. */
const usageLine = (primary?: UsageWindow, secondary?: UsageWindow): string => {
  if (!primary) return '';
  let main: string;
  if (primary.used != null && primary.limit != null && primary.accuracy !== 'percent_only') {
    const unit =
      primary.unit === 'requests'
        ? primary.accuracy === 'approximate' ? 'prompts (est.)' : 'solicitudes'
        : primary.unit === 'tokens' ? 'tokens' : '';
    main = `${primary.accuracy === 'approximate' ? '~' : ''}${compact(primary.used)} / ${compact(primary.limit)} ${unit}`.trim();
  } else {
    main = WINDOW_NAMES[primary.window_type] ?? 'Ventana de uso';
  }
  if (secondary) {
    main += ` · ${WINDOW_SHORT[secondary.window_type] ?? ''} ${formatWindowPercent(secondary)}`;
  }
  return main;
};

/** Soft accent by usage; neutral for local estimates. */
const barColor = (window?: UsageWindow) => {
  if (!window) return 'var(--qp-faint)';
  if (window.accuracy === 'approximate') return 'var(--qp-faint)';
  const pct = clampPct(window.utilization);
  if (pct >= 90) return 'var(--qp-danger)';
  if (pct >= 70) return 'var(--qp-warn)';
  return 'var(--qp-live)';
};

/** Re-render every 30s so "Resets in" stays current. */
const useNow = (intervalMs = 30_000) => {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now;
};

type OverviewCardProps = {
  provider: ProviderId;
  snapshot?: UsageSnapshot;
  trend?: TrendData;
  breakdown?: ModelBreakdownEntry[];
  alerts?: UsageAlert[];
  status?: ProviderStatus;
  onClick: () => void;
};

const OverviewCard = ({ provider, snapshot, status, onClick }: OverviewCardProps) => {
  const meta = providerMeta[provider];
  const now = useNow();
  const geminiSource = useSettingsStore((s) => s.geminiSource);
  const setGeminiSource = useSettingsStore((s) => s.setGeminiSource);
  // Gemini can be viewed per source (Antigravity / Gemini CLI / API key /
  // Gemini App); every other provider has a single source.
  const sources = provider === 'antigravity' ? switchableSources(snapshot) : [];
  const view = provider === 'antigravity' ? selectedSource(snapshot, geminiSource) : undefined;

  const [mainPrimary, mainSecondary] = displayWindows(snapshot);
  const [primary, secondary] = view ? [view.windows[0], view.windows[1]] : [mainPrimary, mainSecondary];
  const authState = useUsageStore((s) => s.authStates[provider]);
  const forecasts = useUsageStore((s) => s.forecasts[provider]);
  const baseAccess = getProviderAccessState(status, snapshot, authState);
  const access = view
    ? { ...baseAccess, label: view.badge, color: SOURCE_COLORS[view.state], detail: view.plan?.note || baseAccess.detail }
    : baseAccess;
  const planInfo = view ? view.plan ?? undefined : snapshot?.plan;
  const plan = planInfo?.tier || meta.account;

  const forecast = forecastFor(forecasts, primary);
  const resetsAt = primary?.resets_at ? new Date(primary.resets_at).getTime() : 0;
  const resetSecs = resetsAt ? Math.round((resetsAt - now) / 1000) : 0;
  const timing = isLimitImminent(forecast)
    ? { text: `Lleno en ${formatRemaining(forecast?.eta_to_full_secs ?? 0)}`, warn: true }
    : resetSecs > 0
      ? { text: `Se reinicia en ${formatRemaining(resetSecs)}`, warn: false }
      : undefined;
  const pct = clampPct(primary?.utilization);
  // Cursor's activity summary sits on its own row under the estimate.
  const showNote = provider === 'cursor' && primary?.accuracy === 'approximate' && !!planInfo?.note;

  return (
    <div
      className={`qp-card qp-pcard glass-${meta.tint}`}
      onClick={onClick}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          onClick();
        }
      }}
      role="button"
      tabIndex={0}
      title={access.detail}
    >
      <div className={`qp-pcard-main ${showNote ? 'has-note' : ''}`}>
        <span className="qp-pcard-logo">
          <ProviderLogo provider={provider} size={22} variant="widget-core" />
        </span>

        <div className="qp-pcard-title">
          <span className="qp-pcard-name">{meta.label}</span>
          {primary && <span className="qp-pcard-pct">{formatWindowPercent(primary)}</span>}
        </div>
        <span className="qp-pcard-plan" title={planInfo?.note ?? plan}>{plan}</span>

        <span className={`qp-pcard-sub ${primary ? '' : 'is-note'}`}>
          {primary ? usageLine(primary, secondary) : planInfo?.note || (access.health === 'error' ? 'No disponible' : 'Esperando datos')}
        </span>
        {showNote && (
          <span className="qp-pcard-note" title={primary?.note ?? undefined}>{planInfo?.note}</span>
        )}
        {timing && (
          <span className={`qp-pcard-reset ${timing.warn ? 'is-warn' : ''}`}>{timing.text}</span>
        )}

        {primary && (
          <div
            className="qp-pcard-bar"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(pct)}
            aria-label={`Uso de ${meta.label}`}
            title={primary.note ?? undefined}
          >
            <span style={{ width: `${pct}%`, background: barColor(primary) }} />
          </div>
        )}
      </div>

      <div className="qp-pcard-foot">
        <span className="qp-status" style={{ color: access.color }}>
          <span className="qp-status-dot" style={{ background: access.color }} />
          {access.label}
        </span>
        {sources.length > 0 && (
          <div className="qp-sources" role="group" aria-label="Fuente de datos de Gemini">
            {[{ id: 'auto', label: 'Auto', state: undefined as undefined | keyof typeof SOURCE_COLORS }, ...sources].map((source) => {
              const selected = (view?.id ?? 'auto') === source.id;
              return (
                <button
                  key={source.id}
                  type="button"
                  className={`qp-source ${selected ? 'is-selected' : ''}`}
                  aria-pressed={selected}
                  onClick={(event) => {
                    event.stopPropagation();
                    setGeminiSource(source.id);
                  }}
                  onKeyDown={(event) => event.stopPropagation()}
                  title={source.id === 'auto' ? 'Mejor fuente disponible' : `${source.label}: ${'badge' in source ? source.badge : ''}`}
                >
                  {source.state && <span className="qp-source-dot" style={{ background: SOURCE_COLORS[source.state] }} />}
                  {source.label}
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

export default OverviewCard;
