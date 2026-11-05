import { AuthState, ProviderStatus, UsageSnapshot } from '@/types';

export type ProviderAccessHealth = ProviderStatus['health'] | 'unknown';

export type ProviderAccessState = {
  health: ProviderAccessHealth;
  // A fixed set of generic labels, or a provider-supplied one such as
  // "ACTIVE — LOCAL SESSION" / "ACTIVE — API KEY".
  label: string;
  detail: string;
  color: string;
};

// Status colours come from the design tokens (design-system.css).
export const providerAccessColor = (health: ProviderAccessHealth) => {
  if (health === 'active') return 'var(--qp-live)';
  if (health === 'error') return 'var(--qp-danger)';
  if (health === 'unknown') return 'var(--qp-muted)';
  return 'var(--qp-warn)';
};

export const providerAccessDotClass = (health: ProviderAccessHealth) =>
  health === 'unknown' ? 'health-unknown' : `health-${health}`;

// Keep the UI on a small set of human-facing states even though snapshots can come
// from several backends. The goal is to answer one question consistently:
// "Can the app access live provider usage right now?"
// "ACTIVE" = the tool is present/running or a key is valid, but the provider
// exposes no live quota figure for it; shown in blue to tell it apart from LIVE.
export const ACTIVE_COLOR = 'var(--qp-active)';

export const getProviderAccessState = (
  status?: ProviderStatus,
  snapshot?: UsageSnapshot,
  authState?: AuthState
): ProviderAccessState => {
  if (snapshot?.badge) {
    // "LIVE — …" badges (e.g. LiteLLM metrics) are real figures: green.
    // "ACTIVE — …" badges mean present/running without figures: blue.
    const live = snapshot.badge.toUpperCase().startsWith('LIVE');
    return {
      health: 'active',
      label: snapshot.badge,
      detail: snapshot.plan?.note || status?.message || snapshot.badge,
      color: live ? providerAccessColor('active') : ACTIVE_COLOR,
    };
  }

  if (snapshot?.stale) {
    // Say how old the cached figure is (the backend drops it after 10 min).
    const ageMin = Math.max(0, Math.round((Date.now() - new Date(snapshot.fetched_at).getTime()) / 60000));
    const age = ageMin < 1 ? 'just now' : `${ageMin}m ago`;
    return {
      health: 'waiting',
      label: `Live — cached ${age}`,
      detail: `The last live call failed; showing live data from ${age} until the next refresh.`,
      color: providerAccessColor('waiting'),
    };
  }

  if (snapshot && (snapshot.provenance === 'derived_local' || snapshot.source === 'local_log')) {
    if (authState?.kind === 'none') {
      return {
        health: 'waiting',
        label: 'Local — no auth',
        detail: 'No provider auth was detected; showing usage derived from local session logs.',
        color: providerAccessColor('waiting'),
      };
    }

    if (authState?.kind === 'oauth') {
      const nowUnix = Math.floor(Date.now() / 1000);
      const expired =
        authState.expires_at_unix_secs != null && authState.expires_at_unix_secs <= nowUnix;
      if (expired) {
        return {
          health: 'waiting',
          label: 'Local — auth expired',
          detail: authState.last_error || 'OAuth is expired; click to refresh.',
          color: providerAccessColor('waiting'),
        };
      }
    }

    // Auth handled by a local CLI/server (e.g. Antigravity): a local-log snapshot
    // just means the server isn't running, not that anything failed.
    if (authState?.kind === 'cli') {
      return {
        health: 'waiting',
        label: 'Local',
        detail: authState.last_error || 'Showing local data — start the CLI for live quota.',
        color: providerAccessColor('waiting'),
      };
    }

    return {
      health: 'waiting',
      label: 'Local — fetch failed',
      detail: authState?.last_error || 'Live usage is unavailable right now; showing local data until the next refresh.',
      color: providerAccessColor('waiting'),
    };
  }

  if (snapshot) {
    return {
      health: 'active',
      label: 'Live',
      detail: 'Usage data is accessible.',
      color: providerAccessColor('active'),
    };
  }

  if (status?.health === 'active') {
    return {
      health: 'waiting',
      label: 'Checking',
      detail: 'Provider data was detected; waiting for the first usage snapshot.',
      color: providerAccessColor('waiting'),
    };
  }

  if (status?.message) {
    return {
      health: 'error',
      label: 'No access',
      detail: status.message,
      color: providerAccessColor('error'),
    };
  }

  return {
    health: 'unknown',
    label: 'Checking',
    detail: 'Checking provider access.',
    color: providerAccessColor('unknown'),
  };
};
