import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { Budget, ProviderId, PerProviderThresholds, RefreshCadence, TrayTitleMode, TrendPreset } from '@/types';

type ThemeMode = 'light' | 'dark' | 'system';

type SettingsState = {
  enabledProviders: Record<ProviderId, boolean>;
  refreshCadence: RefreshCadence;
  theme: ThemeMode;
  widgetMode: boolean;
  sidebarCollapsed: boolean;
  launchAtStartup: boolean;
  // Check GitHub Releases for a signed update once per launch.
  autoUpdateCheck: boolean;
  // Global default window used for trend fetches, alerts, and the comparison
  // surface. A preset (7/30/90) can be overridden with a custom day count.
  trendPreset: TrendPreset;
  trendCustom: boolean;
  customDays: number;
  // A1 notifications opt-in + per-provider utilization alert lines.
  notificationsEnabled: boolean;
  perProviderThresholds: Record<ProviderId, PerProviderThresholds>;
  // A3 budgets + A4 per-provider refresh cadences.
  budgets: Record<ProviderId, Budget>;
  perProviderCadence: Record<ProviderId, RefreshCadence>;
  // Text shown next to the macOS menu-bar icon.
  trayTitleMode: TrayTitleMode;
  // Which Gemini source the card shows: "auto" (best available) or a source id.
  geminiSource: string;
  // Persist hydration is tracked separately so side effects only run after the
  // user's saved preferences have been loaded from storage.
  hydrated: boolean;
  setProviderEnabled: (provider: ProviderId, enabled: boolean) => void;
  setRefreshCadence: (cadence: RefreshCadence) => void;
  setTheme: (theme: ThemeMode) => void;
  setWidgetMode: (enabled: boolean) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setLaunchAtStartup: (enabled: boolean) => void;
  setAutoUpdateCheck: (enabled: boolean) => void;
  setTrendPreset: (preset: TrendPreset) => void;
  setTrendCustom: (custom: boolean) => void;
  setCustomDays: (days: number) => void;
  resolveTrendDays: () => number;
  setNotificationsEnabled: (enabled: boolean) => void;
  setProviderThresholds: (provider: ProviderId, thresholds: PerProviderThresholds) => void;
  setBudgets: (provider: ProviderId, budget: Budget) => void;
  setPerProviderCadence: (provider: ProviderId, cadence: RefreshCadence) => void;
  setTrayTitleMode: (mode: TrayTitleMode) => void;
  setGeminiSource: (source: string) => void;
  markHydrated: () => void;
};

const SETTINGS_KEY = 'quotapulse-settings';
// Carry settings saved under the storage key used by earlier builds.
try {
  const previous = localStorage.getItem('otm-settings-v2');
  if (previous && !localStorage.getItem(SETTINGS_KEY)) localStorage.setItem(SETTINGS_KEY, previous);
} catch {
  // Storage unavailable: start from defaults.
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set, get) => ({
      enabledProviders: { claude: true, codex: true, antigravity: true, cursor: true },
      refreshCadence: 'every2m',
      theme: 'system',
      widgetMode: false,
      sidebarCollapsed: false,
      launchAtStartup: true,
      autoUpdateCheck: true,
      hydrated: false,
      trendPreset: 30,
      trendCustom: false,
      customDays: 14,
      notificationsEnabled: true,
      perProviderThresholds: {
        claude: { warning: 80, high: 90, critical: 97 },
        codex: { warning: 80, high: 90, critical: 97 },
        antigravity: { warning: 80, high: 90, critical: 97 },
        cursor: { warning: 80, high: 90, critical: 97 },
      },
      budgets: {
        claude: { amount_usd: 0, period_days: 30 },
        codex: { amount_usd: 0, period_days: 30 },
        antigravity: { amount_usd: 0, period_days: 30 },
        cursor: { amount_usd: 0, period_days: 30 },
      },
      perProviderCadence: {
        claude: 'every2m',
        codex: 'every2m',
        antigravity: 'every2m',
        cursor: 'every2m',
      },
      trayTitleMode: 'percent',
      geminiSource: 'auto',
      setProviderEnabled: (provider, enabled) =>
        set((state) => ({ enabledProviders: { ...state.enabledProviders, [provider]: enabled } })),
      setRefreshCadence: (cadence) => set({ refreshCadence: cadence }),
      setTheme: (theme) => set({ theme }),
      setWidgetMode: (enabled) => set({ widgetMode: enabled }),
      setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
      setLaunchAtStartup: (enabled) => set({ launchAtStartup: enabled }),
      setAutoUpdateCheck: (enabled) => set({ autoUpdateCheck: enabled }),
      setTrendPreset: (preset) => set({ trendPreset: preset, trendCustom: false }),
      setTrendCustom: (custom) => set({ trendCustom: custom }),
      setCustomDays: (days) =>
        set((state) => ({
          customDays: Math.max(1, Math.min(365, Math.floor(days) || state.customDays)),
          trendCustom: true,
        })),
      resolveTrendDays: () => (get().trendCustom ? get().customDays : get().trendPreset),
      setNotificationsEnabled: (enabled) => set({ notificationsEnabled: enabled }),
      setProviderThresholds: (provider, thresholds) =>
        set((state) => ({
          perProviderThresholds: {
            ...state.perProviderThresholds,
            [provider]: {
              warning: Math.max(0, Math.min(100, thresholds.warning)),
              high: Math.max(0, Math.min(100, thresholds.high)),
              critical: Math.max(0, Math.min(100, thresholds.critical)),
            },
          },
        })),
      setBudgets: (provider, budget) =>
        set((state) => ({
          budgets: {
            ...state.budgets,
            [provider]: {
              amount_usd: Math.max(0, budget.amount_usd),
              period_days: Math.max(1, Math.floor(budget.period_days) || state.budgets[provider].period_days),
            },
          },
        })),
      setPerProviderCadence: (provider, cadence) =>
        set((state) => ({
          perProviderCadence: { ...state.perProviderCadence, [provider]: cadence },
        })),
      setTrayTitleMode: (mode) => set({ trayTitleMode: mode }),
      setGeminiSource: (source) => set({ geminiSource: source }),
      markHydrated: () => set({ hydrated: true }),
    }),
    {
      name: SETTINGS_KEY,
      // Bump when the persisted shape changes so `migrate` can run against
      // older stored payloads. v1 renamed the `gemini` provider to `antigravity`.
      version: 3,
      // Old payloads keyed provider maps by `gemini`. Carry the user's saved
      // enabled/api-key prefs over to `antigravity` and drop the stale key so a
      // returning user doesn't land on an undefined provider tab.
      migrate: (persisted: unknown, fromVersion: number) => {
        const state = (persisted ?? {}) as Record<string, any>;
        if (fromVersion < 1) {
          for (const key of ['enabledProviders', 'apiKeys'] as const) {
            const map = state[key];
            if (map && typeof map === 'object' && 'gemini' in map) {
              if (!('antigravity' in map)) map.antigravity = map.gemini;
              delete map.gemini;
            }
          }
        }
        // v3 (offline-first): the app no longer calls remote usage APIs, so
        // any saved tokens/cookies are dropped from local storage for good.
        if (fromVersion < 3) {
          delete state.apiKeys;
        }
        return state;
      },
      // Backfill any provider keys missing from persisted maps (e.g. a brand-new
      // provider) so lookups like enabledProviders[id] are never undefined.
      merge: (persisted, current) => {
        const p = (persisted ?? {}) as Partial<SettingsState>;
        return {
          ...current,
          ...p,
          enabledProviders: { ...current.enabledProviders, ...(p.enabledProviders ?? {}) },
          perProviderThresholds: {
            ...current.perProviderThresholds,
            ...(p.perProviderThresholds ?? {}),
          },
          budgets: { ...current.budgets, ...(p.budgets ?? {}) },
          perProviderCadence: {
            ...current.perProviderCadence,
            ...(p.perProviderCadence ?? {}),
          },
        };
      },
      // Only persist user-controlled preferences. Runtime bookkeeping like
      // `hydrated` is intentionally excluded.
      partialize: (state) => ({
        enabledProviders: state.enabledProviders,
        refreshCadence: state.refreshCadence,
        theme: state.theme,
        widgetMode: state.widgetMode,
        sidebarCollapsed: state.sidebarCollapsed,
        launchAtStartup: state.launchAtStartup,
        autoUpdateCheck: state.autoUpdateCheck,
        trendPreset: state.trendPreset,
        trendCustom: state.trendCustom,
        customDays: state.customDays,
        notificationsEnabled: state.notificationsEnabled,
        perProviderThresholds: state.perProviderThresholds,
        budgets: state.budgets,
        perProviderCadence: state.perProviderCadence,
        trayTitleMode: state.trayTitleMode,
        geminiSource: state.geminiSource,
      }),
      // Mark the store as ready once Zustand has merged persisted settings.
      onRehydrateStorage: () => (state) => {
        state?.markHydrated();
      },
    }
  )
);
