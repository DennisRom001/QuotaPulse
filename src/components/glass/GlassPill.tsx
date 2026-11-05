import { CSSProperties, PropsWithChildren } from 'react';

type GlassPillProps = PropsWithChildren<{
  onClick?: () => void;
  active?: boolean;
  title?: string;
  className?: string;
  style?: CSSProperties;
}>;

/** Segmented-control option (e.g. 7d / 30d / 90d). Styling lives in design-system.css. */
const GlassPill = ({ children, onClick, active = false, title, className = '', style }: GlassPillProps) => (
  <button
    type="button"
    title={title}
    aria-pressed={active}
    className={`glass-pill transition-smooth ${active ? 'is-active' : ''} ${className}`.trim()}
    style={{ cursor: 'pointer', ...style }}
    onClick={onClick}
  >
    {children}
  </button>
);

export default GlassPill;
