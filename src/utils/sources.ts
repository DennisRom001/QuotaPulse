import { SourceState, SourceView, UsageSnapshot } from '@/types';
import { ACTIVE_COLOR } from '@/utils/providerAccess';

export const SOURCE_COLORS: Record<SourceState, string> = {
  live: 'var(--qp-live)',
  active: ACTIVE_COLOR,
  off: 'var(--text-muted)',
};

/** The sources a user can switch between (only when there are at least two). */
export const switchableSources = (snapshot?: UsageSnapshot): SourceView[] =>
  (snapshot?.sources ?? []).length >= 2 ? snapshot!.sources! : [];

/**
 * The source the card should display: the user's pick if that source still
 * exists, otherwise `undefined` (= the backend's best-available snapshot).
 */
export const selectedSource = (snapshot: UsageSnapshot | undefined, choice: string): SourceView | undefined =>
  choice === 'auto' ? undefined : snapshot?.sources?.find((s) => s.id === choice);

/** Next source when cycling from the top-bar chip (auto → each source → auto). */
export const nextSourceChoice = (snapshot: UsageSnapshot | undefined, choice: string): string => {
  const ids = switchableSources(snapshot).map((s) => s.id);
  if (ids.length === 0) return 'auto';
  const order = ['auto', ...ids];
  return order[(order.indexOf(choice) + 1) % order.length] ?? 'auto';
};
