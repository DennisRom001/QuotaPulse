import { CSSProperties, PropsWithChildren } from 'react';

type Tint = 'claude' | 'codex' | 'antigravity' | 'cursor' | 'neutral';

type GlassPanelProps = PropsWithChildren<{
  className?: string;
  // Kept for API compatibility: providers are never colour-coded, so every
  // tint renders the same solid sheet.
  tint?: Tint;
  style?: CSSProperties;
  onClick?: () => void;
  title?: string;
}>;

// A solid sheet card (the name is historical; there is no glass any more).
const GlassPanel = ({ children, className = '', style, onClick, title }: GlassPanelProps) => (
  <div
    className={`glass-panel ${className}`.trim()}
    style={style}
    onClick={onClick}
    title={title}
  >
    {children}
  </div>
);

export default GlassPanel;
