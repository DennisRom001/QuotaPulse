import { useVisibleInterval } from '@/hooks/usePageVisible';
import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { LogIn } from '@/components/icons';
import { isTauriRuntime } from '@/utils/runtime';
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
import { ACTIVE_COLOR, getProviderAccessState } from '@/utils/providerAccess';
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

/** Ink Is Data: plain ink below 70 %, ochre / red only past a threshold, grey for estimates. */
export const barColor = (window?: UsageWindow) => {
  if (!window || window.accuracy === 'approximate') return 'var(--qp-estimate)';
  const pct = clampPct(window.utilization);
  if (pct >= 90) return 'var(--qp-danger)';
  if (pct >= 70) return 'var(--qp-warn)';
  return 'var(--qp-ink)';
};

/** Re-render every 30s so "Resets in" stays current. */
const useNow = (intervalMs = 30_000) => {
  const [now, setNow] = useState(() => Date.now());
  useVisibleInterval(() => setNow(Date.now()), intervalMs);
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

/**
 * "Iniciar sesión en Claude": opens a terminal running `claude auth login`.
 * The card goes back to live quota on its own once Claude Code writes its
 * credentials (the backend watches that file), which also hides this button.
 */
const ClaudeLoginButton = () => {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string>();
  const run = async (event: React.MouseEvent) => {
    event.stopPropagation();
    if (!isTauriRuntime()) return;
    setBusy(true);
    setMessage(undefined);
    try {
      await invoke('trigger_claude_login');
      setMessage('Terminal abierta: completa el inicio de sesión allí.');
    } catch (e) {
      setMessage(String(e));
    } finally {
      // Keep the spinner briefly so a double click doesn't open two terminals.
      window.setTimeout(() => setBusy(false), 2500);
    }
  };
  return (
    <span className="qp-hint-action">
      <button type="button" className="qp-hint-btn" onClick={run} onKeyDown={(e) => e.stopPropagation()} disabled={busy}>
        <LogIn size={12} className={busy ? 'spin-icon' : undefined} />
        {busy ? 'Abriendo terminal…' : 'Iniciar sesión en Claude'}
      </button>
      {message && <span className="qp-hint-msg">{message}</span>}
    </span>
  );
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
  // Gentle advice: from the backend (e.g. Cursor idle for days) or, for the
  // Cursor estimate, a calibration that is more than a week old.
  const calibratedAt = useSettingsStore((s) => s.cursorCalibratedAt);
  const calibrationAgeDays = calibratedAt ? Math.floor((now - Date.parse(calibratedAt)) / 86_400_000) : 0;
  const hint =
    snapshot?.hint ??
    (provider === 'cursor' && primary?.accuracy === 'approximate' && calibrationAgeDays >= 7
      ? `Calibrado hace ${calibrationAgeDays} días: recalibra con cursor.com/dashboard.`
      : undefined);

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
          <ProviderLogo provider={provider} size={18} />
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

      {hint && (
        <span className="qp-pcard-hint" role="note">
          {hint}
          {/* Only when Claude has no live session (the hint comes from the backend). */}
          {provider === 'claude' && snapshot?.hint && snapshot.source === 'local_log' && <ClaudeLoginButton />}
        </span>
      )}

      <div className="qp-pcard-foot">
        <span className="qp-status" style={{ color: access.color }} title={access.label}>
          <span
            className={`qp-status-dot ${access.color === ACTIVE_COLOR ? 'is-hollow' : ''}`}
            style={{ background: access.color === ACTIVE_COLOR ? 'transparent' : access.color, borderColor: access.color }}
          />
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
