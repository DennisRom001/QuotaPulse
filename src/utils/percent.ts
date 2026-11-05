import { UsageWindow } from '@/types';

const clamp = (value?: number) => Math.max(0, Math.min(100, value ?? 0));

/**
 * Percent label for a quota window.
 *
 * Values reported by a provider API (`exact` / `percent_only`) are shown as
 * delivered — e.g. 39 stays "39%", 39.5 stays "39.5%" — with no intermediate
 * rounding to whole numbers (only float noise beyond 2 decimals is trimmed).
 * Local-log estimates are approximations, so they are rounded and get a "~".
 */
export const formatWindowPercent = (window?: UsageWindow): string => {
  const value = clamp(window?.utilization);
  if (window?.accuracy === 'approximate') return `~${Math.round(value)}%`;
  return `${Number(value.toFixed(2))}%`;
};
