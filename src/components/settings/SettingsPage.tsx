import { useState, useEffect, useMemo } from 'react';
import { Bell, Download, Gauge, MenuSquare, Monitor, Palette, Power, RefreshCw, Server, Wallet } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import GlassToggle from '@/components/glass/GlassToggle';
import GlassInput from '@/components/glass/GlassInput';
import DiagnosticsPanel from '@/components/states/DiagnosticsPanel';
import ProviderLogo from '@/components/providers/ProviderLogo';
import AboutPanel from '@/components/settings/AboutPanel';
import { Budget, BudgetForecast, ExportFormat, PerProviderThresholds, ProviderId, ProviderStatus, RefreshCadence, TrayTitleMode } from '@/types';
import { useSettingsStore } from '@/stores/settingsStore';
import { useUsageStore } from '@/stores/usageStore';

const providers: ProviderId[] = ['claude', 'codex', 'antigravity', 'cursor'];

/**
 * Estimated Cursor usage. Cursor keeps its quota server-side, so QuotaPulse
 * divides the prompts it sees locally this cycle by a prompt budget. The
 * accurate way to set that budget is to calibrate against the real % shown
 * on cursor.com/dashboard.
 */
const CursorEstimateSection = () => {
  const budget = useSettingsStore((s) => s.cursorPromptBudget);
  const cycleDay = useSettingsStore((s) => s.cursorCycleStartDay);
  const calibratedAt = useSettingsStore((s) => s.cursorCalibratedAt);
  const setEstimate = useSettingsStore((s) => s.setCursorEstimate);
  const [actualPct, setActualPct] = useState('');
  const [message, setMessage] = useState<string>();

  const calibrate = async () => {
    const pct = Number(actualPct.replace(',', '.'));
    try {
      const next = await invoke<number>('calibrate_cursor_estimate', { actualPercent: pct, cycleStartDay: cycleDay });
      setEstimate({ budget: next, calibratedAt: new Date().toISOString() });
      setActualPct('');
      setMessage(`Calibrado: ~${next} prompts por ciclo`);
    } catch (e) {
      setMessage(String(e));
    }
  };

  const calibratedDays = calibratedAt ? Math.floor((Date.now() - Date.parse(calibratedAt)) / 86_400_000) : null;

  return (
    <div className="stg-section">
      <div className="stg-section-head">
        <div className="stg-section-head-left">
          <span className="stg-section-icon"><Gauge size={13} /></span>
          <span className="stg-section-title">Estimación de Cursor</span>
        </div>
        <span className="stg-badge">{budget ? 'Activa' : 'Desactivada'}</span>
      </div>
      <div className="stg-startup-row">
        <span className="stg-startup-text">
          Cursor guarda su cuota en sus servidores. QuotaPulse la estima a partir de los prompts que ve
          en este equipo. Para una cifra cercana, escribe el % que muestra cursor.com/dashboard y pulsa Calibrar.
        </span>
      </div>
      <div className="stg-startup-row">
        <span className="stg-startup-text">Uso actual del panel (%)</span>
        <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
          <GlassInput type="number" min={0.5} max={100} step={0.1} value={actualPct} onChange={setActualPct} placeholder="p. ej. 23" style={{ minWidth: 70 }} />
          <button className="pcard-action-btn" onClick={calibrate} disabled={!actualPct} style={{ width: 'auto', padding: '0 10px' }}>
            Calibrar
          </button>
        </div>
      </div>
      <div className="stg-startup-row">
        <span className="stg-startup-text">Prompts por ciclo de facturación</span>
        <GlassInput
          type="number"
          min={0}
          value={budget ? String(budget) : ''}
          placeholder="Desactivado"
          onChange={(v) => setEstimate({ budget: Number(v) > 0 ? Math.round(Number(v)) : null, calibratedAt: null })}
          style={{ minWidth: 70 }}
        />
      </div>
      <div className="stg-startup-row">
        <span className="stg-startup-text">El ciclo empieza el día</span>
        <GlassInput
          type="number"
          min={1}
          max={28}
          value={String(cycleDay)}
          onChange={(v) => setEstimate({ cycleStartDay: Math.min(28, Math.max(1, Math.round(Number(v)) || 1)) })}
          style={{ minWidth: 70 }}
        />
      </div>
      {(message || calibratedDays != null) && (
        <div className="stg-startup-row">
          <span className="stg-startup-text">
            {message ??
              `Calibrado ${calibratedDays === 0 ? 'hoy' : `hace ${calibratedDays} d`}${
                calibratedDays != null && calibratedDays >= 7 ? ' — recalibra para una cifra más precisa' : ''
              }`}
          </span>
        </div>
      )}
    </div>
  );
};

const providerLabels: Record<ProviderId, string> = {
  claude: 'Claude',
  codex: 'ChatGPT',
  antigravity: 'Gemini',
  cursor: 'Cursor',
};

// Credentials are auto-detected from each tool's own local files; quota is
// read directly from the provider's official host. Nothing is typed or stored.
const localSources: Record<ProviderId, string> = {
  claude: 'Sesión de Claude Code → api.anthropic.com · registros ~/.claude/projects',
  codex: 'Sesión de Codex CLI → chatgpt.com · registros ~/.codex/sessions',
  antigravity: 'Gemini CLI → Google · Antigravity (local) · ~/.gemini',
  cursor: 'Sesión de Cursor → cursor.com · actividad local',
};

const providerAccents: Record<ProviderId, string> = {
  claude: '217 119 87',
  codex: '16 163 127',
  antigravity: '79 107 237',
  cursor: '124 140 255',
};

const themeOptions = [
  { value: 'system', label: 'Sistema', note: 'Sigue el tema del sistema', tag: 'Adaptable' },
  { value: 'dark', label: 'Oscuro', note: 'Contraste alto', tag: 'Menos brillo' },
  { value: 'light', label: 'Claro', note: 'Modo de escritorio luminoso', tag: 'Ligero' },
] as const;

const cadenceOptions: { value: RefreshCadence; label: string; note: string; badge: string; bars: number[] }[] = [
  { value: 'manual', label: 'Manual', note: 'Bajo demanda', badge: 'Pausa', bars: [8, 14, 10, 6] },
  { value: 'every30s', label: '30s', note: 'Lo más rápido', badge: 'Rápido', bars: [28, 38, 30, 20] },
  { value: 'every1m', label: '1m', note: 'Equilibrado', badge: 'En vivo', bars: [24, 34, 28, 18] },
  { value: 'every2m', label: '2m', note: 'Moderado', badge: 'Equilibrado', bars: [18, 28, 22, 14] },
  { value: 'every5m', label: '5m', note: 'Poco frecuente', badge: 'Ligero', bars: [12, 18, 14, 10] },
  { value: 'every15m', label: '15m', note: 'Mínimo', badge: 'Silencioso', bars: [8, 12, 9, 6] },
];

type SettingsView = 'settings' | 'about';

const thresholdLabels = { warning: 'aviso', high: 'alto', critical: 'crítico' } as const;

const healthLabel = (health?: ProviderStatus['health']) => {
  if (health === 'active') return 'activo';
  if (health === 'error') return 'error';
  return 'en espera';
};

const formatCadenceLabel = (cadence: RefreshCadence) =>
  cadenceOptions.find((option) => option.value === cadence)?.label ?? cadence;

const formatThemeLabel = (theme: 'system' | 'dark' | 'light') =>
  themeOptions.find((option) => option.value === theme)?.label ?? theme;

const formatFetchedAt = (value?: string) => {
  if (!value) return 'Sin datos todavía';
  return `Actualizado ${new Date(value).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;
};

const BudgetForecastSummary = ({ forecast }: { forecast: BudgetForecast }) => {
  const exceed = forecast.projected_exceeds_budget;
  return (
    <div className="stg-pp-forecast-wrap">
      <div className="stg-pp-forecast">
        <div className="stg-pp-forecast-item">
          <span className="stg-pp-forecast-item-label">Gasto</span>
          <span className="stg-pp-forecast-item-value">${forecast.spend_to_date_usd.toFixed(2)}</span>
        </div>
        <div className="stg-pp-forecast-item">
          <span className="stg-pp-forecast-item-label">Proy.</span>
          <span className="stg-pp-forecast-item-value">${forecast.projected_spend_usd.toFixed(2)}</span>
        </div>
        <div className={`stg-pp-forecast-item ${exceed ? 'stg-pp-forecast-exceed' : ''}`}>
          <span className="stg-pp-forecast-item-label">Usado</span>
          <span className="stg-pp-forecast-item-value">{forecast.utilization_percent.toFixed(0)}%</span>
        </div>
        {exceed && <span className="stg-pp-forecast-flag">excede</span>}
      </div>
    </div>
  );
};

const SettingsPage = () => {
  const [view, setView] = useState<SettingsView>('settings');
  const [exportFormat, setExportFormat] = useState<ExportFormat>('csv');
  const [exportDays, setExportDays] = useState(30);

  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);
  const widgetMode = useSettingsStore((s) => s.widgetMode);
  const enabledProviders = useSettingsStore((s) => s.enabledProviders);
  const setProviderEnabled = useSettingsStore((s) => s.setProviderEnabled);
  const refreshCadence = useSettingsStore((s) => s.refreshCadence);
  const setRefreshCadence = useSettingsStore((s) => s.setRefreshCadence);
  const launchAtStartup = useSettingsStore((s) => s.launchAtStartup);
  const setLaunchAtStartup = useSettingsStore((s) => s.setLaunchAtStartup);
  const autoUpdateCheck = useSettingsStore((s) => s.autoUpdateCheck);
  const setAutoUpdateCheck = useSettingsStore((s) => s.setAutoUpdateCheck);

  const notificationsEnabled = useSettingsStore((s) => s.notificationsEnabled);
  const setNotificationsEnabled = useSettingsStore((s) => s.setNotificationsEnabled);
  const perProviderThresholds = useSettingsStore((s) => s.perProviderThresholds);
  const setProviderThresholds = useSettingsStore((s) => s.setProviderThresholds);
  const budgets = useSettingsStore((s) => s.budgets);
  const setBudgets = useSettingsStore((s) => s.setBudgets);
  const perProviderCadence = useSettingsStore((s) => s.perProviderCadence);
  const setPerProviderCadence = useSettingsStore((s) => s.setPerProviderCadence);
  const trayTitleMode = useSettingsStore((s) => s.trayTitleMode);
  const setTrayTitleMode = useSettingsStore((s) => s.setTrayTitleMode);

  const snapshots = useUsageStore((s) => s.snapshots);
  const costHistory = useUsageStore((s) => s.costHistory);
  const statuses = useUsageStore((s) => s.statuses);
  const alerts = useUsageStore((s) => s.alerts);
  const error = useUsageStore((s) => s.error);
  const backendBudgets = useUsageStore((s) => s.backendBudgets);
  const budgetsForecast = useUsageStore((s) => s.budgetsForecast);
  const setCadenceRemote = useUsageStore((s) => s.setCadence);

  const enabledCount = providers.filter((provider) => enabledProviders[provider]).length;
  const activeCount = providers.filter((provider) => statuses[provider]?.health === 'active').length;

  const surfaceCards = [
    { key: 'theme', label: 'Tema', value: formatThemeLabel(theme), badge: theme === 'system' ? 'SO' : 'Fijo', icon: Palette },
    { key: 'mode', label: 'Modo', value: widgetMode ? 'Widget' : 'Panel', badge: widgetMode ? 'Compacto' : 'Completo', icon: Monitor },
    { key: 'refresh', label: 'Actualizar', value: formatCadenceLabel(refreshCadence), badge: refreshCadence === 'manual' ? 'Manual' : 'Auto', icon: RefreshCw },
    { key: 'providers', label: 'Fuentes', value: `${enabledCount}/${providers.length}`, badge: `${activeCount} activas`, icon: Server },
  ] as const;

  // Live previews of the text beside each provider's ring badge, mirroring
  // the backend's build_badges (cost rows are keyed by UTC day).
  const trayPreviews = useMemo(() => {
    const live = providers.filter((p) => snapshots[p]).slice(0, 3);
    const percent = live.map((p) => `${Math.round(snapshots[p]?.windows[0]?.utilization ?? 0)}%`).join(' · ');
    const today = new Date().toISOString().slice(0, 10);
    const cost = live
      .map((p) => {
        const spent = (costHistory[p] ?? [])
          .filter((entry) => entry.date === today)
          .reduce((sum, entry) => sum + entry.estimated_cost_usd, 0);
        return `$${spent.toFixed(0)}`;
      })
      .join(' · ');
    return { off: 'oculto', percent: percent || '0%', cost: cost || '$0' } satisfies Record<TrayTitleMode, string>;
  }, [snapshots, costHistory]);

  const trayTitleOptions: { value: TrayTitleMode; label: string }[] = [
    { value: 'off', label: 'Oculto' },
    { value: 'percent', label: 'Uso %' },
    { value: 'cost', label: 'Coste de hoy' },
  ];

  // A1 — optimistically update the store, then persist the per-provider alert
  // lines. The setter command name is owned by the backend agent; keep working
  // locally if it is unavailable.
  const applyProviderThreshold = async (provider: ProviderId, thresholds: PerProviderThresholds) => {
    setProviderThresholds(provider, thresholds);
    try {
      await invoke('set_thresholds', {
        provider,
        warning: thresholds.warning,
        high: thresholds.high,
        critical: thresholds.critical,
      });
    } catch (err) {
      console.warn('set_thresholds unavailable', err);
    }
  };

  // A3 — persist the per-provider spend cap.
  const applyProviderBudget = async (provider: ProviderId, budget: Budget) => {
    setBudgets(provider, budget);
    try {
      await invoke('set_budget', {
        provider,
        amount_usd: budget.amount_usd,
        period_days: budget.period_days,
      });
    } catch (err) {
      console.warn('set_budget unavailable', err);
    }
  };

  // A4 — persist the per-provider refresh cadence alongside the global one.
  const applyProviderCadence = async (provider: ProviderId, cadence: RefreshCadence) => {
    setPerProviderCadence(provider, cadence);
    try {
      await invoke('set_per_provider_cadence', { provider, cadence });
    } catch (err) {
      console.warn('set_provider_cadence unavailable', err);
    }
  };

  // Load persisted backend budgets once, then (re)forecast each provider
  // whenever the effective budget inputs change so the cards stay current.
  const budgetForecastKey = useMemo(
    () =>
      providers
        .map((provider) => `${provider}:${(backendBudgets[provider] ?? budgets[provider])?.period_days ?? ''}`)
        .join('|'),
    [backendBudgets, budgets]
  );

  useEffect(() => {
    void useUsageStore.getState().fetchBudgets();
  }, []);

  useEffect(() => {
    providers.forEach((provider) => {
      const effective = backendBudgets[provider] ?? budgets[provider];
      if (!effective || effective.amount_usd <= 0) return;
      void useUsageStore.getState().fetchBudgetForecast(provider, effective.period_days);
    });
  }, [budgetForecastKey]);

  return (
    <div className="stg-page">
      {/* Settings / About toggle */}
      <div className="stg-toggle-row">
        <button
          className={`stg-toggle-btn ${view === 'settings' ? 'stg-toggle-active' : ''}`}
          onClick={() => setView('settings')}
        >
          Ajustes
        </button>
        <button
          className={`stg-toggle-btn ${view === 'about' ? 'stg-toggle-active' : ''}`}
          onClick={() => setView('about')}
        >
          Acerca de
        </button>
      </div>

      {view === 'settings' ? (
        <>
          {/* Control surface metrics */}
          <div className="stg-metrics-row">
            {surfaceCards.map((card) => {
              const Icon = card.icon;
              return (
                <div key={card.key} className="stg-metric-card">
                  <span className="stg-metric-icon">
                    <Icon size={13} strokeWidth={2.2} />
                  </span>
                  <div className="stg-metric-copy">
                    <span className="stg-metric-label">{card.label}</span>
                    <span className="stg-metric-value">{card.value}</span>
                  </div>
                  <span className="stg-metric-badge">{card.badge}</span>
                </div>
              );
            })}
          </div>

          {/* Appearance */}
          <div className="stg-section">
            <div className="stg-section-head">
              <div className="stg-section-head-left">
                <span className="stg-section-icon"><Palette size={13} /></span>
                <span className="stg-section-title">Apariencia</span>
              </div>
              <span className="stg-badge">{formatThemeLabel(theme)}</span>
            </div>
            <div className="stg-theme-row">
              {themeOptions.map((option) => {
                const isActive = theme === option.value;
                return (
                  <button
                    key={option.value}
                    className={`stg-theme-card ${isActive ? 'stg-theme-card-active' : ''}`}
                    onClick={() => setTheme(option.value)}
                    title={option.note}
                  >
                    <div className={`stg-theme-preview stg-theme-preview-${option.value}`}>
                      <span className="stg-theme-preview-bar" />
                      <div className="stg-theme-preview-cols">
                        <span className="stg-theme-preview-panel" />
                        <span className="stg-theme-preview-panel stg-theme-preview-side" />
                      </div>
                    </div>
                    <span className="stg-theme-label">{option.label}</span>
                    <span className="stg-theme-tag">{isActive ? 'Activo' : option.tag}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Refresh Cadence */}
          <div className="stg-section">
            <div className="stg-section-head">
              <div className="stg-section-head-left">
                <span className="stg-section-icon"><RefreshCw size={13} /></span>
                <span className="stg-section-title">Actualización</span>
              </div>
              <span className="stg-badge">{formatCadenceLabel(refreshCadence)}</span>
            </div>
            <div className="stg-cadence-row">
              {cadenceOptions.map((option) => {
                const isActive = refreshCadence === option.value;
                return (
                  <button
                    key={option.value}
                    className={`stg-cadence-card ${isActive ? 'stg-cadence-card-active' : ''}`}
                    onClick={() => { setRefreshCadence(option.value); setCadenceRemote(option.value); }}
                    title={option.note}
                  >
                    <div className="stg-cadence-bars">
                      {option.bars.map((h, i) => (
                        <span key={i} className="stg-cadence-bar" style={{ height: h }} />
                      ))}
                    </div>
                    <span className="stg-cadence-label">{option.label}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Desktop Behavior */}
          <div className="stg-section">
            <div className="stg-section-head">
              <div className="stg-section-head-left">
                <span className="stg-section-icon"><Power size={13} /></span>
                <span className="stg-section-title">Inicio</span>
              </div>
              <span className="stg-badge">{launchAtStartup ? 'Automático' : 'Manual'}</span>
            </div>
            <div className="stg-startup-row">
              <span className="stg-startup-text">
                {launchAtStartup ? 'Se abre al iniciar sesión en Windows' : 'Solo se abre manualmente'}
              </span>
              <GlassToggle
                checked={launchAtStartup}
                onChange={setLaunchAtStartup}
                label={launchAtStartup ? 'Sí' : 'No'}
              />
            </div>
            <div className="stg-startup-row">
              <span className="stg-startup-text">
                {autoUpdateCheck
                  ? 'Busca actualizaciones firmadas en GitHub al abrir'
                  : 'Sin búsqueda automática (usa Acerca de → Buscar actualizaciones)'}
              </span>
              <GlassToggle
                checked={autoUpdateCheck}
                onChange={setAutoUpdateCheck}
                label={autoUpdateCheck ? 'Sí' : 'No'}
              />
            </div>
          </div>

          <CursorEstimateSection />

          {/* Providers */}
          <div className="stg-section">
            <div className="stg-section-head">
              <div className="stg-section-head-left">
                <span className="stg-section-icon"><Server size={13} /></span>
                <span className="stg-section-title">Proveedores</span>
              </div>
              <span className="stg-badge">{enabledCount} activos</span>
            </div>
            <div className="stg-providers">
              {providers.map((provider) => {
                const snapshot = snapshots[provider];
                const status = statuses[provider];
                const providerAlertCount = alerts[provider].length;
                const isEnabled = enabledProviders[provider];

                return (
                  <div
                    key={provider}
                    className="stg-provider-card"
                    style={{ '--widget-accent': providerAccents[provider] } as React.CSSProperties}
                  >
                    <div className="stg-provider-top">
                      <ProviderLogo provider={provider} size={16} />
                      <span className="stg-provider-name">{providerLabels[provider]}</span>
                      <span className="stg-badge" style={{ color: status?.health === 'active' ? '#34d399' : status?.health === 'error' ? '#f87171' : '#fbbf24' }}>
                        {healthLabel(status?.health)}
                      </span>
                      <span className="stg-badge">{providerAlertCount} alertas</span>
                      <GlassToggle
                        checked={isEnabled}
                        onChange={(next) => setProviderEnabled(provider, next)}
                        label={isEnabled ? 'Sí' : 'No'}
                      />
                    </div>
                    <div className="stg-provider-key-row">
                      <span className="stg-local-source">{localSources[provider]}</span>
                    </div>
                    <div className="stg-provider-footer">
                      <span>{formatFetchedAt(snapshot?.fetched_at)}</span>
                      <span>{snapshot?.stale ? 'desactualizado' : 'al día'}</span>
                      {snapshot && <span>{snapshot.source}</span>}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Menu bar title */}
          <div className="stg-section">
            <div className="stg-section-head">
              <div className="stg-section-head-left">
                <span className="stg-section-icon"><MenuSquare size={13} /></span>
                <span className="stg-section-title">Bandeja del sistema</span>
              </div>
              <span className="stg-badge">Icono</span>
            </div>
            <div className="stg-cadence-row">
              {trayTitleOptions.map((option) => {
                const isActive = trayTitleMode === option.value;
                return (
                  <button
                    key={option.value}
                    className={`stg-cadence-card ${isActive ? 'stg-cadence-card-active' : ''}`}
                    onClick={() => setTrayTitleMode(option.value)}
                    title="Un icono por proveedor en la bandeja: su logo rodeado por anillos de uso, más este valor"
                  >
                    <span className="stg-tray-preview">{trayPreviews[option.value]}</span>
                    <span className="stg-cadence-label">{option.label}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Notifications + alert thresholds */}
          <div className="stg-section">
            <div className="stg-section-head">
              <div className="stg-section-head-left">
                <span className="stg-section-icon"><Bell size={13} /></span>
                <span className="stg-section-title">Notificaciones</span>
              </div>
              <span className="stg-badge">{notificationsEnabled ? 'Activadas' : 'Desactivadas'}</span>
            </div>
            <div className="stg-pp-toggle-row">
              <span className="stg-pp-toggle-text">
                {notificationsEnabled
                  ? 'Al cruzar cada umbral, cuando el ritmo llevaría al límite antes del reinicio y si se prevé superar el presupuesto'
                  : 'Las notificaciones están desactivadas'}
              </span>
              <GlassToggle
                checked={notificationsEnabled}
                onChange={setNotificationsEnabled}
                label={notificationsEnabled ? 'Sí' : 'No'}
              />
            </div>
            <div className="stg-note">
              <Bell size={11} />
              <span>
                Las notificaciones necesitan el permiso del sistema. Si no aparecen, actívalas para
                QuotaPulse en Configuración de Windows → Sistema → Notificaciones.
              </span>
            </div>
            <div className="stg-pp-grid">
              {providers.map((provider) => {
                const thresholds = perProviderThresholds[provider];
                return (
                  <div
                    key={provider}
                    className="stg-pp-card"
                    style={{ '--widget-accent': providerAccents[provider] } as React.CSSProperties}
                  >
                    <div className="stg-pp-identity">
                      <ProviderLogo provider={provider} size={14} />
                      <span className="stg-pp-name">{providerLabels[provider]}</span>
                    </div>
                    <div className="stg-pp-fields stg-pp-fields-threshold">
                      {(['warning', 'high', 'critical'] as const).map((field) => (
                        <div key={field} className="stg-pp-field">
                          <span className="stg-pp-field-label">{thresholdLabels[field]}</span>
                          <GlassInput
                            type="number"
                            min={0}
                            max={100}
                            value={String(thresholds[field])}
                            onChange={(next) =>
                              applyProviderThreshold(provider, {
                                ...thresholds,
                                [field]: Math.min(100, Math.max(0, Number(next) || thresholds[field])),
                              })
                            }
                            style={{ minWidth: 60 }}
                          />
                        </div>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Budgets */}
          <div className="stg-section">
            <div className="stg-section-head">
              <div className="stg-section-head-left">
                <span className="stg-section-icon"><Wallet size={13} /></span>
                <span className="stg-section-title">Presupuestos</span>
              </div>
              <span className="stg-badge">límite de gasto</span>
            </div>
            <div className="stg-pp-grid">
              {providers.map((provider) => {
                const budget = budgets[provider];
                return (
                  <div
                    key={provider}
                    className="stg-pp-card"
                    style={{ '--widget-accent': providerAccents[provider] } as React.CSSProperties}
                  >
                    <div className="stg-pp-identity">
                      <ProviderLogo provider={provider} size={14} />
                      <span className="stg-pp-name">{providerLabels[provider]}</span>
                    </div>
                    <div className="stg-pp-fields stg-pp-fields-budget">
                      <div className="stg-pp-field">
                        <span className="stg-pp-field-label">Importe ($)</span>
                        <GlassInput
                          type="number"
                          min={0}
                          value={String(budget.amount_usd)}
                          onChange={(next) =>
                            applyProviderBudget(provider, { ...budget, amount_usd: Math.max(0, Number(next) || 0) })
                          }
                          style={{ minWidth: 66 }}
                        />
                      </div>
                      <div className="stg-pp-field">
                        <span className="stg-pp-field-label">Periodo (días)</span>
                        <GlassInput
                          type="number"
                          min={1}
                          value={String(budget.period_days)}
                          onChange={(next) =>
                            applyProviderBudget(provider, { ...budget, period_days: Math.max(1, Number(next) || 1) })
                          }
                          style={{ minWidth: 66 }}
                        />
                      </div>
                    </div>

                    {budgetsForecast[provider] && (
                      <div className="stg-pp-forecast-wrap">
                        <BudgetForecastSummary forecast={budgetsForecast[provider]} />
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          {/* Per-provider refresh cadence */}
          <div className="stg-section">
            <div className="stg-section-head">
              <div className="stg-section-head-left">
                <span className="stg-section-icon"><RefreshCw size={13} /></span>
                <span className="stg-section-title">Frecuencia por proveedor</span>
              </div>
              <span className="stg-badge">por fuente</span>
            </div>
            <div className="stg-pp-grid">
              {providers.map((provider) => {
                const cadence = perProviderCadence[provider];
                return (
                  <div
                    key={provider}
                    className="stg-pp-card"
                    style={{ '--widget-accent': providerAccents[provider] } as React.CSSProperties}
                  >
                    <div className="stg-pp-identity">
                      <ProviderLogo provider={provider} size={14} />
                      <span className="stg-pp-name">{providerLabels[provider]}</span>
                    </div>
                    <div className="stg-pp-fields stg-pp-field-select">
                      <div className="stg-pp-field">
                        <span className="stg-pp-field-label">Actualizar</span>
                        <select
                          className="stg-cadence-select"
                          value={cadence}
                          onChange={(event) => applyProviderCadence(provider, event.target.value as RefreshCadence)}
                        >
                          {cadenceOptions.map((option) => (
                            <option key={option.value} value={option.value}>
                              {option.label}
                            </option>
                          ))}
                        </select>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Export report */}
          <div className="stg-section">
            <div className="stg-section-head">
              <div className="stg-section-head-left">
                <span className="stg-section-icon"><Download size={13} /></span>
                <span className="stg-section-title">Exportar</span>
              </div>
              <span className="stg-badge">CSV · JSON · PDF</span>
            </div>
            <div className="stg-pp-fields stg-pp-fields-budget">
              <div className="stg-pp-field">
                <span className="stg-pp-field-label">Formato</span>
                <select
                  className="stg-cadence-select"
                  value={exportFormat}
                  onChange={(event) => setExportFormat(event.target.value as ExportFormat)}
                >
                  <option value="csv">CSV</option>
                  <option value="json">JSON</option>
                  <option value="pdf">PDF</option>
                </select>
              </div>
              <div className="stg-pp-field">
                <span className="stg-pp-field-label">Días</span>
                <select
                  className="stg-cadence-select"
                  value={String(exportDays)}
                  onChange={(event) => setExportDays(Number(event.target.value))}
                >
                  <option value="7">7</option>
                  <option value="30">30</option>
                  <option value="90">90</option>
                </select>
              </div>
              <div className="stg-pp-field stg-pp-field-btn">
                <button
                  type="button"
                  className="stg-pp-export-btn"
                  onClick={() => void useUsageStore.getState().exportReport(exportFormat, exportDays)}
                >
                  Exportar
                </button>
              </div>
            </div>
          </div>

          <DiagnosticsPanel
            statuses={statuses}
            snapshots={snapshots}
            alerts={alerts}
            globalError={error}
          />
        </>
      ) : (
        <AboutPanel />
      )}
    </div>
  );
};

export default SettingsPage;
