import { ReactNode } from 'react';
import { ProviderId } from '@/types';

/*
 * Provider monograms (DESIGN.md → Iconography).
 * QuotaPulse's own geometric marks — not the providers' logos — drawn on a
 * 20 px grid in currentColor, so they follow ink / chalk and stay crisp at
 * 16 and 20 px. Each provider is told apart by shape alone:
 *   Claude  — a ring holding a six-spoke spark
 *   ChatGPT — a terminal prompt (Codex CLI)
 *   Gemini  — a geometric G
 *   Cursor  — a pointer arrow
 *   LiteLLM — a router: one node fanning out to two
 */

export type MarkId = ProviderId | 'litellm';

const marks: Record<MarkId, ReactNode> = {
  claude: (
    <>
      <circle cx="10" cy="10" r="6.75" />
      <path d="M10 6.9v6.2M7.3 8.45l5.4 3.1M7.3 11.55l5.4-3.1" />
    </>
  ),
  codex: (
    <>
      <rect x="2.75" y="3.75" width="14.5" height="12.5" rx="3" />
      <path d="M6.5 8.25 9 10l-2.5 1.75M10.75 12.25h3" />
    </>
  ),
  antigravity: <path d="M15.4 6.6A6 6 0 1 0 16 10h-5.25" />,
  cursor: <path d="M5 3.75 15.5 9.6l-4.85 1.05-2.1 4.6z" />,
  litellm: (
    <>
      <circle cx="5" cy="10" r="1.9" />
      <circle cx="15" cy="5" r="1.9" />
      <circle cx="15" cy="15" r="1.9" />
      <path d="M6.7 9.15 13.3 5.85M6.7 10.85l6.6 3.3" />
    </>
  ),
};

type ProviderMarkProps = {
  provider: MarkId;
  size?: number;
  className?: string;
};

export const ProviderMark = ({ provider, size = 20, className }: ProviderMarkProps) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 20 20"
    fill="none"
    stroke="currentColor"
    // Keep the stroke optically ~1.5 px at small sizes.
    strokeWidth={size <= 14 ? 2 : 1.6}
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
    focusable="false"
    className={['qp-mark', className].filter(Boolean).join(' ')}
    style={{ display: 'block', flexShrink: 0 }}
  >
    {marks[provider]}
  </svg>
);

type ProviderLogoProps = {
  provider: ProviderId;
  size?: number;
  // Kept for API compatibility; every placement uses the same monogram.
  variant?: 'default' | 'widget-core';
};

const ProviderLogo = ({ provider, size = 18 }: ProviderLogoProps) => (
  <ProviderMark provider={provider} size={Math.max(12, size)} />
);

export default ProviderLogo;
