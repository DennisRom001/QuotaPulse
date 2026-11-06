import { UsageSnapshot, UsageWindow } from '@/types';

const compactNumber = (value?: number | null) => {
  if (value == null) return '-';
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return String(value);
};

export const windowLabel = (window?: UsageWindow) => {
  switch (window?.window_type) {
    case 'five_hour':
      return 'Ventana 5 h';
    case 'seven_day':
      return 'Ventana 7 d';
    case 'daily':
      return 'Diaria';
    case 'monthly':
      return 'Mensual';
    case 'weekly':
      return 'Semanal';
    case 'session':
      return 'Sesión';
    default:
      return 'Ventana';
  }
};

export const displayWindows = (snapshot?: UsageSnapshot): [UsageWindow | undefined, UsageWindow | undefined] => {
  if (!snapshot) return [undefined, undefined];

  const [first, second] = snapshot.windows;
  if (!first || !second || snapshot.provider !== 'antigravity') {
    return [first, second];
  }

  const firstReset = first.reset_countdown_secs ?? Number.POSITIVE_INFINITY;
  const secondReset = second.reset_countdown_secs ?? Number.POSITIVE_INFINITY;

  return secondReset < firstReset ? [second, first] : [first, second];
};

export const windowValueLabel = (window?: UsageWindow) => {
  if (!window) return 'Sin datos';
  if (window.accuracy === 'percent_only' || window.used == null || window.limit == null) {
    return '';
  }

  const prefix = window.accuracy === 'approximate' ? '~' : '';
  const base = `${prefix}${compactNumber(window.used)} / ${compactNumber(window.limit)}`;

  switch (window.unit) {
    case 'requests':
      return window.accuracy === 'approximate' ? `${base} prompts (est.)` : `${base} solicitudes`;
    case 'tokens':
      return `${base} tokens`;
    default:
      return base;
  }
};

export const countdownLabel = (window?: UsageWindow) => {
  const secs = window?.reset_countdown_secs;
  if (secs == null || secs <= 0) return 'Reinicio desconocido';
  if (secs < 7_200) return `quedan ${Math.ceil(secs / 60)} min`;
  if (secs < 172_800) return `quedan ${Math.ceil(secs / 3_600)} h`;
  return `quedan ${Math.ceil(secs / 86_400)} d`;
};
