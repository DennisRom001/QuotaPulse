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
    const age = ageMin < 1 ? 'ahora' : `hace ${ageMin} min`;
    return {
      health: 'waiting',
      label: `Live — caché ${age}`,
      detail: `La última consulta en vivo falló; se muestran los datos en vivo de ${age} hasta la próxima actualización.`,
      color: providerAccessColor('waiting'),
    };
  }

  if (snapshot && (snapshot.provenance === 'derived_local' || snapshot.source === 'local_log')) {
    if (authState?.kind === 'none') {
      return {
        health: 'waiting',
        label: 'Local — sin sesión',
        detail: 'No se detectó ninguna sesión del proveedor; se muestra el uso calculado con los registros locales.',
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
          label: 'Local — sesión caducada',
          detail: authState.last_error || 'La sesión ha caducado; pulsa para actualizar.',
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
        detail: authState.last_error || 'Datos locales — abre la CLI para ver la cuota en vivo.',
        color: providerAccessColor('waiting'),
      };
    }

    return {
      health: 'waiting',
      label: 'Local — error al consultar',
      detail: authState?.last_error || 'El uso en vivo no está disponible ahora; se muestran datos locales hasta la próxima actualización.',
      color: providerAccessColor('waiting'),
    };
  }

  if (snapshot) {
    return {
      health: 'active',
      label: 'Live',
      detail: 'Datos de uso disponibles.',
      color: providerAccessColor('active'),
    };
  }

  if (status?.health === 'active') {
    return {
      health: 'waiting',
      label: 'Comprobando',
      detail: 'Se detectó el proveedor; esperando los primeros datos de uso.',
      color: providerAccessColor('waiting'),
    };
  }

  if (status?.message) {
    return {
      health: 'error',
      label: 'Sin acceso',
      detail: status.message,
      color: providerAccessColor('error'),
    };
  }

  return {
    health: 'unknown',
    label: 'Comprobando',
    detail: 'Comprobando el acceso al proveedor.',
    color: providerAccessColor('unknown'),
  };
};
